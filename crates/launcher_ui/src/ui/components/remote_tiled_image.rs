use std::hash::{Hash, Hasher};
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

use egui::{TextureOptions, Ui};
use shared_lru::ThreadSafeLru;

use crate::app::tokio_runtime;

use super::{image_memory::compress_rgba_image_for_memory, image_textures};

const TILE_MAX_DIM: u32 = 4096;
const REMOTE_IMAGE_MAX_BYTES: usize = 96 * 1024 * 1024;
const REMOTE_IMAGE_STALE_FRAMES: u64 = 900;

#[derive(Clone)]
enum RemoteImageState {
    Loading,
    Ready(Arc<TiledImage>),
    Failed,
}

#[derive(Clone)]
struct RemoteImageEntry {
    state: RemoteImageState,
    last_touched_frame: u64,
}

#[derive(Clone)]
struct TiledImage {
    width: u32,
    height: u32,
    tiles: Vec<TileImage>,
}

#[derive(Clone)]
struct TileImage {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    uri: String,
    bytes: Arc<[u8]>,
}

struct RemoteImageCache {
    states: ThreadSafeLru<String, RemoteImageEntry>,
    frame_index: u64,
    tx: Option<mpsc::Sender<(String, Result<Arc<TiledImage>, String>)>>,
    rx: Option<Arc<Mutex<mpsc::Receiver<(String, Result<Arc<TiledImage>, String>)>>>>,
}

impl Default for RemoteImageCache {
    fn default() -> Self {
        Self {
            states: ThreadSafeLru::new(REMOTE_IMAGE_MAX_BYTES),
            frame_index: 0,
            tx: None,
            rx: None,
        }
    }
}

fn cache() -> &'static Mutex<RemoteImageCache> {
    static CACHE: OnceLock<Mutex<RemoteImageCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(RemoteImageCache::default()))
}

fn ensure_channel(cache: &mut RemoteImageCache) {
    if cache.tx.is_some() && cache.rx.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<(String, Result<Arc<TiledImage>, String>)>();
    cache.tx = Some(tx);
    cache.rx = Some(Arc::new(Mutex::new(rx)));
}

fn poll_updates(ctx: &egui::Context, cache: &mut RemoteImageCache) -> bool {
    let mut updates = Vec::new();
    let mut should_reset = false;
    if let Some(rx) = cache.rx.as_ref() {
        match rx.lock() {
            Ok(receiver) => loop {
                match receiver.try_recv() {
                    Ok(update) => updates.push(update),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        tracing::error!(
                            target: "vertexlauncher/remote_image",
                            "Remote image worker disconnected unexpectedly."
                        );
                        should_reset = true;
                        break;
                    }
                }
            },
            Err(_) => {
                tracing::error!(
                    target: "vertexlauncher/remote_image",
                    "Remote image receiver mutex was poisoned."
                );
                should_reset = true;
            }
        }
    }

    if should_reset {
        cache.tx = None;
        cache.rx = None;
        // Remove any entries still stuck in Loading state — their in-flight tasks
        // held the old sender and can no longer deliver results. Removing them lets
        // the next show() call re-dispatch a fresh request on the new channel.
        cache.states.write(|state| {
            state.retain(|_, entry| !matches!(entry.value.state, RemoteImageState::Loading))
        });
    }

    let mut did_update = false;
    for (url, result) in updates {
        match result {
            Ok(image) => {
                let approx_bytes = tiled_image_bytes(image.as_ref());
                let evicted = cache.states.write(|state| {
                    state.insert_without_eviction(
                        url,
                        RemoteImageEntry {
                            last_touched_frame: cache.frame_index,
                            state: RemoteImageState::Ready(image),
                        },
                        approx_bytes,
                    );
                    state.evict_to_budget_where(|_, entry| {
                        !matches!(entry.value.state, RemoteImageState::Loading)
                    })
                });
                for (_, entry) in evicted {
                    forget_remote_entry(ctx, &entry);
                }
            }
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/remote_image",
                    url = %url,
                    error = %err,
                    "Remote image load failed."
                );
                let evicted = cache.states.write(|state| {
                    state.insert_without_eviction(
                        url,
                        RemoteImageEntry {
                            last_touched_frame: cache.frame_index,
                            state: RemoteImageState::Failed,
                        },
                        0,
                    );
                    state.evict_to_budget_where(|_, entry| {
                        !matches!(entry.value.state, RemoteImageState::Loading)
                    })
                });
                for (_, entry) in evicted {
                    forget_remote_entry(ctx, &entry);
                }
            }
        }
        did_update = true;
    }
    if did_update {
        trim_remote_cache(ctx, cache);
    }
    did_update
}

pub fn show(
    ui: &mut Ui,
    url: &str,
    desired_size: egui::Vec2,
    id_source: impl Hash,
    placeholder_svg: &[u8],
) {
    let normalized_url = url.trim();
    if normalized_url.is_empty() {
        show_placeholder(ui, desired_size, id_source, placeholder_svg);
        return;
    }

    let mut render_ready: Option<Arc<TiledImage>> = None;
    {
        let Ok(mut cache) = cache().lock() else {
            show_placeholder(ui, desired_size, id_source, placeholder_svg);
            return;
        };
        cache.frame_index = cache.frame_index.saturating_add(1);
        let frame_index = cache.frame_index;
        trim_remote_cache(ui.ctx(), &mut cache);
        let mut request_follow_up_repaint = poll_updates(ui.ctx(), &mut cache);

        if let Some(entry) = cache.states.write(|state| {
            let entry = state.touch(&normalized_url.to_owned())?;
            entry.value.last_touched_frame = frame_index;
            Some(entry.value.clone())
        }) {
            match &entry.state {
                RemoteImageState::Ready(image) => {
                    render_ready = Some(Arc::clone(image));
                }
                RemoteImageState::Loading => {
                    request_follow_up_repaint = true;
                }
                RemoteImageState::Failed => {
                    show_placeholder(ui, desired_size, id_source, placeholder_svg);
                    return;
                }
            }
        } else {
            ensure_channel(&mut cache);
            let Some(tx) = cache.tx.as_ref().cloned() else {
                show_placeholder(ui, desired_size, id_source, placeholder_svg);
                return;
            };
            cache.states.write(|state| {
                state.insert_without_eviction(
                    normalized_url.to_owned(),
                    RemoteImageEntry {
                        state: RemoteImageState::Loading,
                        last_touched_frame: frame_index,
                    },
                    0,
                );
            });
            request_follow_up_repaint = true;
            let url_owned = normalized_url.to_owned();
            tokio_runtime::spawn_blocking_detached(move || {
                let result = fetch_and_tile_remote_image(url_owned.as_str());
                if let Err(err) = tx.send((url_owned.clone(), result)) {
                    tracing::error!(
                        target: "vertexlauncher/remote_image",
                        url = %url_owned,
                        error = %err,
                        "Failed to deliver remote tiled image result."
                    );
                }
            });
        }

        if request_follow_up_repaint {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }

    if let Some(image) = render_ready {
        show_tiled(ui, &image, desired_size);
    } else {
        show_placeholder(ui, desired_size, id_source, placeholder_svg);
    }
}

fn show_tiled(ui: &mut Ui, image: &TiledImage, desired_size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let width = image.width.max(1) as f32;
    let height = image.height.max(1) as f32;

    for tile in &image.tiles {
        let min_x = rect.left() + rect.width() * (tile.x as f32 / width);
        let min_y = rect.top() + rect.height() * (tile.y as f32 / height);
        let max_x = rect.left() + rect.width() * ((tile.x + tile.width) as f32 / width);
        let max_y = rect.top() + rect.height() * ((tile.y + tile.height) as f32 / height);
        let tile_rect =
            egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y));
        if let image_textures::ManagedTextureStatus::Ready(texture) =
            image_textures::request_texture(
                ui.ctx(),
                tile.uri.clone(),
                Arc::clone(&tile.bytes),
                TextureOptions::LINEAR,
            )
        {
            let image = egui::Image::from_texture(&texture).fit_to_exact_size(tile_rect.size());
            let _ = ui.put(tile_rect, image);
        }
    }
}

fn show_placeholder(
    ui: &mut Ui,
    desired_size: egui::Vec2,
    id_source: impl Hash,
    placeholder_svg: &[u8],
) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id_source.hash(&mut hasher);
    ui.add(
        egui::Image::from_bytes(
            format!("bytes://remote-placeholder/{}", hasher.finish()),
            placeholder_svg.to_vec(),
        )
        .fit_to_exact_size(desired_size),
    );
}

fn fetch_and_tile_remote_image(url: &str) -> Result<Arc<TiledImage>, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| format!("failed to fetch remote icon {url}: {err}"))?;
    let (_, body) = response.into_parts();
    let mut reader = body.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read remote icon bytes {url}: {err}"))?;
    let image = image::load_from_memory(bytes.as_slice())
        .map_err(|err| format!("failed to decode remote icon {url}: {err}"))?;

    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err("remote icon has invalid dimensions".to_owned());
    }

    let mut tiles = Vec::new();
    for y in (0..height).step_by(TILE_MAX_DIM as usize) {
        for x in (0..width).step_by(TILE_MAX_DIM as usize) {
            let tile_width = (width - x).min(TILE_MAX_DIM);
            let tile_height = (height - y).min(TILE_MAX_DIM);
            let tile_image = image.crop_imm(x, y, tile_width, tile_height).to_rgba8();
            let tile_bytes = compress_rgba_image_for_memory(tile_image, None);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            url.hash(&mut hasher);
            x.hash(&mut hasher);
            y.hash(&mut hasher);
            tile_width.hash(&mut hasher);
            tile_height.hash(&mut hasher);
            let uri = format!("bytes://remote-tile/{:x}.png", hasher.finish());
            tiles.push(TileImage {
                x,
                y,
                width: tile_width,
                height: tile_height,
                uri,
                bytes: Arc::<[u8]>::from(tile_bytes),
            });
        }
    }

    Ok(Arc::new(TiledImage {
        width,
        height,
        tiles,
    }))
}

fn trim_remote_cache(ctx: &egui::Context, cache: &mut RemoteImageCache) {
    let stale_before = cache.frame_index.saturating_sub(REMOTE_IMAGE_STALE_FRAMES);
    let evicted = cache.states.write(|state| {
        state.retain(|_, entry| {
            matches!(entry.value.state, RemoteImageState::Loading)
                || entry.value.last_touched_frame >= stale_before
        })
    });
    for (_, entry) in evicted {
        forget_remote_entry(ctx, &entry);
    }

    let evicted = cache.states.write(|state| {
        state.evict_to_budget_where(|_, entry| {
            !matches!(entry.value.state, RemoteImageState::Loading)
        })
    });
    for (_, entry) in evicted {
        forget_remote_entry(ctx, &entry);
    }
}

fn forget_remote_entry(_ctx: &egui::Context, entry: &RemoteImageEntry) {
    if let RemoteImageState::Ready(image) = &entry.state {
        for tile in &image.tiles {
            image_textures::evict_source_key(tile.uri.as_str());
        }
    }
}

fn tiled_image_bytes(image: &TiledImage) -> usize {
    image
        .tiles
        .iter()
        .map(|tile| tile.bytes.len() + tile.uri.len())
        .sum()
}
