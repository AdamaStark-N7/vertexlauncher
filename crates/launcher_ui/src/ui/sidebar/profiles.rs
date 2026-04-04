use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, OnceLock, mpsc},
};

use egui::{Button, Ui};
use shared_lru::ThreadSafeLru;

use crate::{
    app::tokio_runtime,
    assets,
    ui::{
        components::{icon_button, image_memory::load_image_path_for_memory, image_textures},
        instance_context_menu, style,
    },
};

use super::{ProfileShortcut, SidebarOutput};

const SIDEBAR_THUMBNAIL_CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const SIDEBAR_THUMBNAIL_CACHE_STALE_FRAMES: u64 = 900;

#[derive(Clone)]
struct SidebarThumbnailEntry {
    bytes: Option<Arc<[u8]>>,
    last_touched_frame: u64,
}

struct SidebarThumbnailCache {
    entries: ThreadSafeLru<String, SidebarThumbnailEntry>,
    frame_index: u64,
}

impl Default for SidebarThumbnailCache {
    fn default() -> Self {
        Self {
            entries: ThreadSafeLru::new(SIDEBAR_THUMBNAIL_CACHE_MAX_BYTES),
            frame_index: 0,
        }
    }
}

/// Renders the instance shortcut list and emits click or context-menu actions.
pub fn render(
    ui: &mut Ui,
    profile_shortcuts: &[ProfileShortcut],
    output: &mut SidebarOutput,
    max_icon_width: f32,
) {
    if profile_shortcuts.is_empty() {
        return;
    }

    begin_thumbnail_cache_frame(ui.ctx());
    poll_thumbnail_results(ui.ctx());
    let row_height = max_icon_width.max(1.0);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = style::SPACE_SM;

        for profile in profile_shortcuts {
            let icon_id = format!("user_profile_{}", profile.id);
            let context_id =
                ui.make_persistent_id(("sidebar_instance_context", profile.id.as_str()));
            let response = ui
                .allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), row_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let thumbnail = profile
                            .thumbnail_path
                            .as_deref()
                            .filter(|path| !path.as_os_str().is_empty())
                            .and_then(|path| {
                                let key = thumbnail_cache_key(profile.id.as_str(), path);
                                match thumbnail_cache().lock() {
                                    Ok(cache) => {
                                        let frame_index = cache.frame_index;
                                        cache.entries.write(|state| {
                                            let entry = state.touch(&key)?;
                                            entry.value.last_touched_frame = frame_index;
                                            entry.value.bytes.clone()
                                        })
                                    }
                                    Err(_) => None,
                                }
                            });
                        let thumbnail_uri = profile
                            .thumbnail_path
                            .as_deref()
                            .filter(|path| !path.as_os_str().is_empty())
                            .map(|path| {
                                thumbnail_uri_from_key(&thumbnail_cache_key(
                                    profile.id.as_str(),
                                    path,
                                ))
                            });
                        if thumbnail.is_none()
                            && let Some(path) = profile
                                .thumbnail_path
                                .as_deref()
                                .filter(|path| !path.as_os_str().is_empty())
                        {
                            request_thumbnail(profile.id.as_str(), path.to_path_buf());
                        }
                        render_profile_icon(
                            ui,
                            icon_id.as_str(),
                            profile.name.as_str(),
                            max_icon_width,
                            thumbnail_uri,
                            thumbnail,
                        )
                    },
                )
                .inner;

            if response.clicked() {
                output.selected_profile_id = Some(profile.id.clone());
            }

            if response.secondary_clicked() {
                let anchor = response
                    .interact_pointer_pos()
                    .or_else(|| ui.ctx().pointer_latest_pos())
                    .unwrap_or(response.rect.left_bottom());
                instance_context_menu::request_for_instance(ui.ctx(), context_id, anchor, true);
            }

            if let Some(action) = instance_context_menu::take(ui.ctx(), context_id) {
                output
                    .instance_context_actions
                    .push((profile.id.clone(), action));
            }
        }
    });
}

fn render_profile_icon(
    ui: &mut Ui,
    icon_id: &str,
    tooltip: &str,
    max_icon_width: f32,
    thumbnail_uri: Option<String>,
    thumbnail_bytes: Option<Arc<[u8]>>,
) -> egui::Response {
    if let Some(bytes) = thumbnail_bytes {
        let button_size = ui.available_width().min(max_icon_width).max(1.0);
        let icon_size = (button_size - 8.0).clamp(10.0, button_size);
        let uri = thumbnail_uri.unwrap_or_else(|| thumbnail_uri_from_key(icon_id));
        if let image_textures::ManagedTextureStatus::Ready(texture) =
            image_textures::request_texture(ui.ctx(), uri, bytes, egui::TextureOptions::LINEAR)
        {
            let image = egui::Image::from_texture(&texture)
                .fit_to_exact_size(egui::vec2(icon_size, icon_size));
            return ui.add_sized(
                [button_size, button_size],
                Button::image(image)
                    .frame(true)
                    .corner_radius(egui::CornerRadius::same(10))
                    .stroke(egui::Stroke::new(
                        1.0,
                        ui.visuals().widgets.inactive.bg_stroke.color,
                    ))
                    .fill(ui.visuals().widgets.inactive.weak_bg_fill),
            );
        }
    }

    icon_button::svg(
        ui,
        icon_id,
        assets::LIBRARY_SVG,
        tooltip,
        false,
        max_icon_width,
    )
}

fn thumbnail_cache() -> &'static Mutex<SidebarThumbnailCache> {
    static CACHE: OnceLock<Mutex<SidebarThumbnailCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(SidebarThumbnailCache::default()))
}

fn thumbnail_in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn thumbnail_results_channel() -> &'static (
    mpsc::Sender<(String, Option<Arc<[u8]>>)>,
    Arc<Mutex<mpsc::Receiver<(String, Option<Arc<[u8]>>)>>>,
) {
    static CHANNEL: OnceLock<(
        mpsc::Sender<(String, Option<Arc<[u8]>>)>,
        Arc<Mutex<mpsc::Receiver<(String, Option<Arc<[u8]>>)>>>,
    )> = OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(String, Option<Arc<[u8]>>)>();
        (tx, Arc::new(Mutex::new(rx)))
    })
}

fn thumbnail_cache_key(instance_id: &str, path: &std::path::Path) -> String {
    format!("{instance_id}\n{}", path.display())
}

fn request_thumbnail(instance_id: &str, path: std::path::PathBuf) {
    let key = thumbnail_cache_key(instance_id, path.as_path());
    if let Ok(cache) = thumbnail_cache().lock()
        && cache.entries.read(|state| state.contains_key(&key))
    {
        return;
    }
    if let Ok(mut in_flight) = thumbnail_in_flight().lock() {
        if in_flight.contains(key.as_str()) {
            return;
        }
        in_flight.insert(key.clone());
    }
    let tx = thumbnail_results_channel().0.clone();
    tokio_runtime::spawn_detached(async move {
        let bytes = match load_image_path_for_memory(path.clone()).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/sidebar",
                    thumbnail_key = %key,
                    path = %path.display(),
                    error = %err,
                    "Failed to read sidebar thumbnail."
                );
                None
            }
        };
        if let Err(err) = tx.send((key.clone(), bytes)) {
            tracing::error!(
                target: "vertexlauncher/sidebar",
                thumbnail_key = %key,
                path = %path.display(),
                error = %err,
                "Failed to deliver sidebar thumbnail result."
            );
        }
    });
}

fn poll_thumbnail_results(ctx: &egui::Context) {
    let rx = thumbnail_results_channel().1.clone();
    let Ok(receiver) = rx.lock() else {
        tracing::error!(
            target: "vertexlauncher/sidebar",
            "Sidebar thumbnail receiver mutex was poisoned."
        );
        return;
    };
    let mut updates = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(update) => updates.push(update),
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
                tracing::error!(
                    target: "vertexlauncher/sidebar",
                    "Sidebar thumbnail worker disconnected unexpectedly."
                );
                break;
            }
        }
    }
    if updates.is_empty() {
        return;
    }
    if let Ok(mut cache) = thumbnail_cache().lock()
        && let Ok(mut in_flight) = thumbnail_in_flight().lock()
    {
        let frame_index = cache.frame_index;
        for (key, bytes) in updates {
            in_flight.remove(key.as_str());
            let approx_bytes = bytes.as_ref().map_or(0, |bytes| bytes.len());
            let evicted = cache.entries.write(|state| {
                state.insert_without_eviction(
                    key,
                    SidebarThumbnailEntry {
                        bytes,
                        last_touched_frame: frame_index,
                    },
                    approx_bytes,
                );
                state.evict_to_budget()
            });
            for (key, _) in evicted {
                image_textures::evict_source_key(thumbnail_uri_from_key(&key).as_str());
            }
        }
        trim_thumbnail_cache(ctx, &mut cache);
    }
}

fn begin_thumbnail_cache_frame(ctx: &egui::Context) {
    if let Ok(mut cache) = thumbnail_cache().lock() {
        cache.frame_index = cache.frame_index.saturating_add(1);
        trim_thumbnail_cache(ctx, &mut cache);
    }
}

fn trim_thumbnail_cache(_ctx: &egui::Context, cache: &mut SidebarThumbnailCache) {
    let stale_before = cache
        .frame_index
        .saturating_sub(SIDEBAR_THUMBNAIL_CACHE_STALE_FRAMES);
    let evicted = cache
        .entries
        .write(|state| state.retain(|_, entry| entry.value.last_touched_frame >= stale_before));
    for (key, _) in evicted {
        image_textures::evict_source_key(thumbnail_uri_from_key(&key).as_str());
    }

    let evicted = cache.entries.write(|state| state.evict_to_budget());
    for (key, _) in evicted {
        image_textures::evict_source_key(thumbnail_uri_from_key(&key).as_str());
    }
}

fn thumbnail_uri_from_key(key: &str) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("bytes://sidebar/profile-thumb/{}", hasher.finish())
}
