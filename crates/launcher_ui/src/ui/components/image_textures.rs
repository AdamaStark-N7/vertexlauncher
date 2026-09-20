use std::{
    collections::{HashSet, VecDeque},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use shared_lru::ThreadSafeLru;

use crate::app::tokio_runtime;

use super::{
    bc7::{self, Bc7Tile},
    gpu_texture::{self, TiledBc7Texture},
};

const IMAGE_TEXTURE_MAX_BYTES: usize = 128 * 1024 * 1024;
const IMAGE_TEXTURE_STALE_FRAMES: u64 = 600;
const IMAGE_TEXTURE_FETCH_MAX_PER_FRAME: usize = 128;
const IMAGE_TEXTURE_UPLOAD_MAX_PER_FRAME: usize = 32;
const IMAGE_TEXTURE_UPLOAD_MAX_BYTES_PER_FRAME: usize = 16 * 1024 * 1024;
const IMAGE_TEXTURE_EVICT_GRACE_FRAMES: u64 = 2;
/// Largest side kept for viewer textures, before tiling. Images larger than the adapter's
/// texture size limit are split into tiles of exactly that size, compressed or not.
const VIEWER_MAX_EDGE: u32 = 4096 * 4;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct ManagedTextureKey {
    source_key: String,
    options: TextureOptions,
    max_edge: Option<u32>,
    /// Encode to BC7 and keep the texture GPU-compressed.
    compressed: bool,
    /// Split into tiles of at most this many pixels per side; 0 means a single texture.
    tile_max: u32,
}

#[derive(Clone)]
enum ManagedTextureState {
    Loading,
    Ready(TextureHandle),
    ReadyCompressed(Arc<TiledBc7Texture>),
    ReadyTiled(Arc<TiledRgbaTexture>),
    Failed,
}

#[derive(Clone)]
struct ManagedTextureEntry {
    state: ManagedTextureState,
    last_touched_frame: u64,
}

/// An uncompressed image split into tiles that each fit the adapter's texture size limit.
pub struct TiledRgbaTexture {
    width: u32,
    height: u32,
    tiles: Vec<(u32, u32, TextureHandle)>,
}

impl TiledRgbaTexture {
    fn paint(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        uv: egui::Rect,
        corner_radius: egui::CornerRadius,
    ) {
        let tiles: Vec<gpu_texture::TileDraw> = self
            .tiles
            .iter()
            .map(|(x, y, handle)| gpu_texture::TileDraw {
                x: *x,
                y: *y,
                size: handle.size_vec2(),
                id: handle.id(),
                uv_scale: egui::vec2(1.0, 1.0),
            })
            .collect();
        gpu_texture::paint_tiles(
            ui,
            rect,
            uv,
            corner_radius,
            [self.width, self.height],
            &tiles,
        );
    }
}

enum UploadPayload {
    Rgba(ColorImage),
    RgbaTiles {
        width: u32,
        height: u32,
        tiles: Vec<(u32, u32, ColorImage)>,
    },
    Bc7(Vec<Bc7Tile>),
}

struct ReadyTextureUpload {
    key: ManagedTextureKey,
    payload: UploadPayload,
    approx_bytes: usize,
}

struct ManagedTextureCache {
    entries: ThreadSafeLru<ManagedTextureKey, ManagedTextureEntry>,
    frame_index: u64,
    pending: HashSet<ManagedTextureKey>,
    ready: VecDeque<(ManagedTextureKey, Result<ReadyTextureUpload, String>)>,
    results:
        launcher_runtime::WorkerChannel<(ManagedTextureKey, Result<ReadyTextureUpload, String>)>,
}

/// A ready image, either plain RGBA or GPU-compressed (BC7) when that was possible.
#[derive(Clone)]
pub enum ImageTexture {
    Rgba(TextureHandle),
    Compressed(Arc<TiledBc7Texture>),
}

impl ImageTexture {
    /// Drop-in for `egui::Image::from_texture`.
    pub fn image(&self) -> egui::Image<'static> {
        match self {
            Self::Rgba(texture) => egui::Image::from_texture(texture),
            Self::Compressed(texture) => match texture.single() {
                Some((id, size, uv_scale)) => egui::Image::new(egui::load::SizedTexture::new(
                    id, size,
                ))
                .uv(egui::Rect::from_min_max(
                    egui::Pos2::ZERO,
                    egui::pos2(uv_scale.x, uv_scale.y),
                )),
                // Not produced for these requests (they are never tiled).
                None => egui::Image::new(egui::load::SizedTexture::new(
                    egui::TextureId::default(),
                    egui::Vec2::ZERO,
                )),
            },
        }
    }
}

#[derive(Clone)]
pub enum ManagedTextureStatus {
    Loading,
    Ready(ImageTexture),
    Failed,
}

impl Default for ManagedTextureCache {
    fn default() -> Self {
        Self {
            entries: ThreadSafeLru::new(IMAGE_TEXTURE_MAX_BYTES),
            frame_index: 0,
            pending: HashSet::new(),
            ready: VecDeque::new(),
            results: Default::default(),
        }
    }
}

fn cache() -> &'static Mutex<ManagedTextureCache> {
    static CACHE: OnceLock<Mutex<ManagedTextureCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ManagedTextureCache::default()))
}

pub fn begin_frame(ctx: &Context) {
    gpu_texture::flush_deferred_frees();
    let Ok(mut cache) = cache().lock() else {
        tracing::error!(
            target: "vertexlauncher/image_textures",
            "Managed image texture cache mutex was poisoned."
        );
        return;
    };

    cache.frame_index = cache.frame_index.saturating_add(1);
    poll_updates(ctx, &mut cache);
    trim_stale(&mut cache);
    trim_to_budget(&mut cache);

    if !cache.pending.is_empty() || !cache.ready.is_empty() {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

pub fn request_texture(
    ctx: &Context,
    source_key: impl Into<String>,
    bytes: Arc<[u8]>,
    options: TextureOptions,
) -> ManagedTextureStatus {
    request_texture_inner(ctx, source_key.into(), bytes, options, None)
}

pub fn request_texture_with_max_edge(
    ctx: &Context,
    source_key: impl Into<String>,
    bytes: Arc<[u8]>,
    options: TextureOptions,
    max_edge: u32,
) -> ManagedTextureStatus {
    request_texture_inner(
        ctx,
        source_key.into(),
        bytes,
        options,
        Some(max_edge.max(1)),
    )
}

/// A screenshot-viewer texture: GPU-compressed when the device supports it, otherwise RGBA.
/// Either way it is tiled to the adapter's texture size limit when the image is larger.
#[derive(Clone)]
pub enum ViewerTexture {
    Compressed(Arc<TiledBc7Texture>),
    Rgba(Arc<TiledRgbaTexture>),
}

impl ViewerTexture {
    /// Paints `uv` (a sub-rect of the whole image, in 0..1) into `rect`.
    pub fn paint(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        uv: egui::Rect,
        corner_radius: egui::CornerRadius,
    ) {
        match self {
            Self::Compressed(texture) => texture.paint(ui, rect, uv, corner_radius),
            Self::Rgba(texture) => texture.paint(ui, rect, uv, corner_radius),
        }
    }
}

pub enum ViewerTextureStatus {
    Loading,
    Ready(ViewerTexture),
    Failed,
}

/// Requests a full-size viewer texture, compressed to BC7 (with mips) when the GPU supports it and
/// plain RGBA otherwise. Images beyond the adapter's texture size limit are tiled in both cases.
pub fn request_viewer_texture(
    ctx: &Context,
    source_key: impl Into<String>,
    bytes: Arc<[u8]>,
) -> ViewerTextureStatus {
    let compressed = gpu_texture::is_available();
    let key = ManagedTextureKey {
        source_key: source_key.into(),
        options: TextureOptions::LINEAR,
        max_edge: Some(VIEWER_MAX_EDGE),
        compressed,
        // egui-wgpu reports the device's real `max_texture_dimension_2d` here.
        tile_max: ctx.input(|input| input.max_texture_side) as u32,
    };
    match request_managed(ctx, key, bytes) {
        ManagedState::Loading => ViewerTextureStatus::Loading,
        ManagedState::Failed => ViewerTextureStatus::Failed,
        ManagedState::ReadyTiled(texture) => {
            ViewerTextureStatus::Ready(ViewerTexture::Rgba(texture))
        }
        // Single-texture results only come from the non-viewer entry points.
        ManagedState::Ready(_) => ViewerTextureStatus::Failed,
        ManagedState::ReadyCompressed(texture) => {
            ViewerTextureStatus::Ready(ViewerTexture::Compressed(texture))
        }
    }
}

fn request_texture_inner(
    ctx: &Context,
    source_key: String,
    bytes: Arc<[u8]>,
    options: TextureOptions,
    max_edge: Option<u32>,
) -> ManagedTextureStatus {
    let key = ManagedTextureKey {
        source_key,
        options,
        max_edge,
        // Compressed when the device supports it and the image suits BC7 (see decode).
        compressed: gpu_texture::is_available(),
        tile_max: 0,
    };
    match request_managed(ctx, key, bytes) {
        ManagedState::Loading => ManagedTextureStatus::Loading,
        ManagedState::Failed => ManagedTextureStatus::Failed,
        ManagedState::Ready(texture) => ManagedTextureStatus::Ready(ImageTexture::Rgba(texture)),
        ManagedState::ReadyCompressed(texture) => {
            ManagedTextureStatus::Ready(ImageTexture::Compressed(texture))
        }
        // Only viewer keys (tile_max > 0) are tiled.
        ManagedState::ReadyTiled(_) => ManagedTextureStatus::Failed,
    }
}

enum ManagedState {
    Loading,
    Ready(TextureHandle),
    ReadyCompressed(Arc<TiledBc7Texture>),
    ReadyTiled(Arc<TiledRgbaTexture>),
    Failed,
}

fn request_managed(ctx: &Context, key: ManagedTextureKey, bytes: Arc<[u8]>) -> ManagedState {
    let Ok(mut cache) = cache().lock() else {
        tracing::error!(
            target: "vertexlauncher/image_textures",
            texture_key = %key.source_key,
            "Managed image texture cache mutex was poisoned."
        );
        return ManagedState::Failed;
    };

    if let Some(entry) = cache.entries.write(|state| {
        let entry = state.touch(&key)?;
        entry.value.last_touched_frame = cache.frame_index;
        Some(entry.value.clone())
    }) {
        return match entry.state {
            ManagedTextureState::Loading => ManagedState::Loading,
            ManagedTextureState::Ready(texture) => ManagedState::Ready(texture),
            ManagedTextureState::ReadyCompressed(texture) => ManagedState::ReadyCompressed(texture),
            ManagedTextureState::ReadyTiled(texture) => ManagedState::ReadyTiled(texture),
            ManagedTextureState::Failed => ManagedState::Failed,
        };
    }

    let tx = cache.results.sender();

    cache.entries.write(|state| {
        state.insert_without_eviction(
            key.clone(),
            ManagedTextureEntry {
                state: ManagedTextureState::Loading,
                last_touched_frame: cache.frame_index,
            },
            0,
        );
    });
    cache.pending.insert(key.clone());

    tokio_runtime::spawn_blocking_detached(move || {
        // A decode briefly holds several full-size copies of the image; cap how many run at once.
        let _slot = decode_gate().acquire();
        let result = decode_ready_texture(key.clone(), bytes.as_ref());
        if let Err(err) = tx.send((key.clone(), result)) {
            tracing::error!(
                target: "vertexlauncher/image_textures",
                texture_key = %key.source_key,
                error = %err,
                "Failed to deliver managed image texture result."
            );
        }
    });
    ctx.request_repaint_after(Duration::from_millis(16));
    ManagedState::Loading
}

fn decode_gate() -> &'static launcher_runtime::BlockingGate {
    static GATE: OnceLock<launcher_runtime::BlockingGate> = OnceLock::new();
    GATE.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        // Leave most cores free for the UI thread; decoding and compressing are CPU-heavy.
        launcher_runtime::BlockingGate::new((cores / 4).clamp(1, 3))
    })
}

/// Frees every managed texture that was not requested during the current frame. Call at the end
/// of a frame after switching screens so the old screen's images are released immediately rather
/// than waiting for the stale timer or the memory budget.
pub fn release_untouched() {
    let Ok(mut cache) = cache().lock() else {
        return;
    };
    let frame = cache.frame_index;
    let removed = cache
        .entries
        .write(|state| state.retain(|_, entry| entry.value.last_touched_frame >= frame));
    if removed.is_empty() {
        return;
    }
    let removed_keys: HashSet<ManagedTextureKey> =
        removed.into_iter().map(|(key, _)| key).collect();
    cache.pending.retain(|key| !removed_keys.contains(key));
    cache.ready.retain(|(key, _)| !removed_keys.contains(key));
}

pub fn evict_source_key(source_key: &str) {
    let Ok(mut cache) = cache().lock() else {
        tracing::error!(
            target: "vertexlauncher/image_textures",
            texture_key = %source_key,
            "Managed image texture cache mutex was poisoned."
        );
        return;
    };

    cache.pending.retain(|key| key.source_key != source_key);
    cache.ready.retain(|(key, _)| key.source_key != source_key);
    let _ = cache
        .entries
        .write(|state| state.retain(|key, _| key.source_key != source_key));
}

fn poll_updates(ctx: &Context, cache: &mut ManagedTextureCache) {
    let drained = cache.results.drain_up_to(IMAGE_TEXTURE_FETCH_MAX_PER_FRAME);
    let channel_broke = drained.disconnected;
    // Decoded images wait in `ready`; uploads to the GPU are rate-limited below.
    cache.ready.extend(drained.items);

    let mut uploaded_count = 0usize;
    let mut uploaded_bytes = 0usize;
    while uploaded_count < IMAGE_TEXTURE_UPLOAD_MAX_PER_FRAME
        && uploaded_bytes < IMAGE_TEXTURE_UPLOAD_MAX_BYTES_PER_FRAME
    {
        let Some((key, result)) = cache.ready.pop_front() else {
            break;
        };
        cache.pending.remove(&key);
        let is_loading = cache.entries.read(|state| {
            matches!(
                state.get(&key).map(|entry| &entry.value.state),
                Some(ManagedTextureState::Loading)
            )
        });
        if !is_loading {
            continue;
        }

        match result {
            Ok(upload) => {
                uploaded_count = uploaded_count.saturating_add(1);
                uploaded_bytes = uploaded_bytes.saturating_add(upload.approx_bytes);
                let name = managed_texture_name(&upload.key);
                let (ready_state, approx_bytes) = match upload.payload {
                    UploadPayload::Rgba(image) => {
                        let texture = ctx.load_texture(name, image, upload.key.options);
                        let bytes = texture.byte_size().max(upload.approx_bytes);
                        (ManagedTextureState::Ready(texture), bytes)
                    }
                    UploadPayload::RgbaTiles {
                        width,
                        height,
                        tiles,
                    } => {
                        let tiles: Vec<(u32, u32, TextureHandle)> = tiles
                            .into_iter()
                            .map(|(x, y, image)| {
                                let handle = ctx.load_texture(
                                    format!("{name}_{x}_{y}"),
                                    image,
                                    upload.key.options,
                                );
                                (x, y, handle)
                            })
                            .collect();
                        let bytes = tiles.iter().map(|(_, _, t)| t.byte_size()).sum();
                        (
                            ManagedTextureState::ReadyTiled(Arc::new(TiledRgbaTexture {
                                width,
                                height,
                                tiles,
                            })),
                            bytes,
                        )
                    }
                    UploadPayload::Bc7(image) => {
                        match TiledBc7Texture::upload(name.as_str(), &image) {
                            Some(texture) => {
                                let bytes = texture.gpu_bytes();
                                (
                                    ManagedTextureState::ReadyCompressed(Arc::new(texture)),
                                    bytes,
                                )
                            }
                            None => (ManagedTextureState::Failed, 0),
                        }
                    }
                };
                let frame_index = cache.frame_index;
                let evicted = cache.entries.write(|state| {
                    state.insert_without_eviction(
                        upload.key.clone(),
                        ManagedTextureEntry {
                            state: ready_state,
                            last_touched_frame: cache.frame_index,
                        },
                        approx_bytes,
                    );
                    state.evict_to_budget_where(|_, entry| {
                        can_evict_budget_entry(entry, frame_index)
                    })
                });
                drop(evicted);
            }
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/image_textures",
                    texture_key = %key.source_key,
                    error = %err,
                    "Failed to decode managed image texture."
                );
                let frame_index = cache.frame_index;
                let evicted = cache.entries.write(|state| {
                    state.insert_without_eviction(
                        key,
                        ManagedTextureEntry {
                            state: ManagedTextureState::Failed,
                            last_touched_frame: cache.frame_index,
                        },
                        0,
                    );
                    state.evict_to_budget_where(|_, entry| {
                        can_evict_budget_entry(entry, frame_index)
                    })
                });
                drop(evicted);
            }
        }
    }

    if channel_broke {
        cache.pending.clear();
        cache.ready.clear();
        let _ = cache.entries.write(|state| {
            state.retain(|_, entry| !matches!(entry.value.state, ManagedTextureState::Loading))
        });
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn trim_stale(cache: &mut ManagedTextureCache) {
    let stale_before = cache.frame_index.saturating_sub(IMAGE_TEXTURE_STALE_FRAMES);
    let evicted = cache.entries.write(|state| {
        state.retain(|_, entry| {
            matches!(entry.value.state, ManagedTextureState::Loading)
                || entry.value.last_touched_frame >= stale_before
        })
    });
    drop(evicted);
}

fn trim_to_budget(cache: &mut ManagedTextureCache) {
    let frame_index = cache.frame_index;
    let evicted = cache.entries.write(|state| {
        state.evict_to_budget_where(|_, entry| can_evict_budget_entry(entry, frame_index))
    });
    drop(evicted);
}

fn can_evict_budget_entry(
    entry: &shared_lru::LruEntry<ManagedTextureEntry>,
    frame_index: u64,
) -> bool {
    !matches!(entry.value.state, ManagedTextureState::Loading)
        && frame_index.saturating_sub(entry.value.last_touched_frame)
            > IMAGE_TEXTURE_EVICT_GRACE_FRAMES
}

fn decode_ready_texture(
    key: ManagedTextureKey,
    bytes: &[u8],
) -> Result<ReadyTextureUpload, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| format!("failed to decode '{}': {err}", key.source_key))?;
    let image = if let Some(max_edge) = key.max_edge {
        let width = image.width();
        let height = image.height();
        if width.max(height) > max_edge {
            image.resize(max_edge, max_edge, image::imageops::FilterType::Triangle)
        } else {
            image
        }
    } else {
        image
    }
    .to_rgba8();
    let normalized_image = if image.width() == 0 || image.height() == 0 {
        image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]))
    } else {
        image
    };
    let size = [
        normalized_image.width() as usize,
        normalized_image.height() as usize,
    ];
    let viewer = key.tile_max > 0;
    if key.compressed && (viewer || suits_bc7(&normalized_image, key.options)) {
        let tile_max = if viewer {
            key.tile_max.max(4) as usize
        } else {
            usize::MAX / 8
        };
        let compressed =
            bc7::compress_rgba_tiled(normalized_image.as_raw(), size[0], size[1], tile_max);
        let approx_bytes = compressed.iter().map(|tile| tile.image.byte_size()).sum();
        return Ok(ReadyTextureUpload {
            key,
            payload: UploadPayload::Bc7(compressed),
            approx_bytes,
        });
    }
    // Viewer keys always take the tiled path (a single tile when the image fits).
    if key.tile_max > 0 {
        let tile = key.tile_max as usize;
        let raw = normalized_image.as_raw();
        let mut tiles = Vec::new();
        for y in (0..size[1]).step_by(tile) {
            let th = tile.min(size[1] - y);
            for x in (0..size[0]).step_by(tile) {
                let tw = tile.min(size[0] - x);
                let mut sub = Vec::with_capacity(tw * th * 4);
                for row in y..y + th {
                    let start = (row * size[0] + x) * 4;
                    sub.extend_from_slice(&raw[start..start + tw * 4]);
                }
                tiles.push((
                    x as u32,
                    y as u32,
                    ColorImage::from_rgba_unmultiplied([tw, th], &sub),
                ));
            }
        }
        let approx_bytes = size[0]
            .saturating_mul(size[1])
            .saturating_mul(std::mem::size_of::<egui::Color32>());
        return Ok(ReadyTextureUpload {
            key,
            payload: UploadPayload::RgbaTiles {
                width: size[0] as u32,
                height: size[1] as u32,
                tiles,
            },
            approx_bytes,
        });
    }
    let color_image = ColorImage::from_rgba_unmultiplied(size, normalized_image.as_raw());
    let approx_bytes = size[0]
        .saturating_mul(size[1])
        .saturating_mul(std::mem::size_of::<egui::Color32>());
    Ok(ReadyTextureUpload {
        key,
        payload: UploadPayload::Rgba(color_image),
        approx_bytes,
    })
}

/// Our BC7 encoding is opaque-only and works on 4x4 blocks, so it only suits photo-like images that
/// are drawn smoothly. Icons, avatars, skins and pixel art stay plain RGBA.
fn suits_bc7(image: &image::RgbaImage, options: TextureOptions) -> bool {
    const MIN_SIDE: u32 = 64;
    options.magnification == egui::TextureFilter::Linear
        && image.width() >= MIN_SIDE
        && image.height() >= MIN_SIDE
        && image.pixels().all(|pixel| pixel.0[3] == 255)
}

fn managed_texture_name(key: &ManagedTextureKey) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    format!("managed_image_texture_{:016x}", hasher.finish())
}
