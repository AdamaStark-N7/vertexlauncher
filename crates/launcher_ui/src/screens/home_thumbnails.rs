use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    app::tokio_runtime,
    ui::components::{image_memory::load_image_path_for_memory, image_textures},
};

use super::{HOME_THUMBNAIL_CACHE_MAX_BYTES, HOME_THUMBNAIL_CACHE_STALE_FRAMES};

#[path = "home_thumbnails/home_thumbnail_state.rs"]
mod home_thumbnail_state;
#[path = "home_thumbnails/thumbnail_cache_entry.rs"]
mod thumbnail_cache_entry;

pub(super) use self::home_thumbnail_state::HomeThumbnailState;
use self::thumbnail_cache_entry::ThumbnailCacheEntry;

pub(super) fn purge_activity_image_state(ctx: &egui::Context, state: &mut HomeThumbnailState) {
    for key in state.cache.keys() {
        forget_home_thumbnail(ctx, key);
    }
    state.cache.clear();
    state.in_flight.clear();
    state.results.reset();
}

pub(super) fn instance_thumbnail_cache_key(instance_id: &str, path: &Path) -> String {
    format!("{instance_id}\n{}", path.display())
}

pub(super) fn home_instance_thumbnail_uri(instance_id: &str, path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    instance_id.hash(&mut hasher);
    path.hash(&mut hasher);
    format!("bytes://home/instance-thumbnail/{:016x}", hasher.finish())
}

pub(super) fn home_world_thumbnail_uri(instance_id: &str, world_id: &str) -> String {
    format!("bytes://home/world-thumbnail/{instance_id}/{world_id}")
}

pub(super) fn request_instance_thumbnail(
    state: &mut HomeThumbnailState,
    instance_id: &str,
    path: PathBuf,
) {
    let key = instance_thumbnail_cache_key(instance_id, path.as_path());
    if state.in_flight.contains(key.as_str()) {
        return;
    }
    let tx = state.results.sender();
    state.in_flight.insert(key.clone());
    tokio_runtime::spawn_detached(async move {
        let bytes = match load_image_path_for_memory(path.clone()).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/home",
                    thumbnail_key = %key,
                    path = %path.display(),
                    error = %err,
                    "Failed to read home instance thumbnail."
                );
                None
            }
        };
        if let Err(err) = tx.send((key.clone(), bytes)) {
            tracing::error!(
                target: "vertexlauncher/home",
                thumbnail_key = %key,
                path = %path.display(),
                error = %err,
                "Failed to deliver home instance thumbnail result."
            );
        }
    });
}

pub(super) fn poll_instance_thumbnail_results(ctx: &egui::Context, state: &mut HomeThumbnailState) {
    let drained = state.results.drain();
    let updates = drained.items;
    if drained.disconnected {
        tracing::error!(target: "vertexlauncher/home", "results worker channel stopped unexpectedly.");
        state.in_flight.clear();
    }
    for (key, bytes) in updates {
        state.in_flight.remove(key.as_str());
        state.cache.insert(
            key,
            ThumbnailCacheEntry {
                approx_bytes: bytes.as_ref().map_or(0, |bytes| bytes.len()),
                bytes,
                last_touched_frame: state.cache_frame_index,
            },
        );
    }
    trim_home_thumbnail_cache(ctx, state);
}

pub(super) fn trim_home_thumbnail_cache(ctx: &egui::Context, state: &mut HomeThumbnailState) {
    let stale_before = state
        .cache_frame_index
        .saturating_sub(HOME_THUMBNAIL_CACHE_STALE_FRAMES);
    state.cache.retain(|key, entry| {
        let keep =
            state.in_flight.contains(key.as_str()) || entry.last_touched_frame >= stale_before;
        if !keep {
            forget_home_thumbnail(ctx, key);
        }
        keep
    });

    let mut total_bytes = state
        .cache
        .values()
        .map(|entry| entry.approx_bytes)
        .sum::<usize>();
    if total_bytes <= HOME_THUMBNAIL_CACHE_MAX_BYTES {
        return;
    }

    let mut eviction_order = state
        .cache
        .iter()
        .filter(|(key, _)| !state.in_flight.contains(key.as_str()))
        .map(|(key, entry)| (key.clone(), entry.last_touched_frame, entry.approx_bytes))
        .collect::<Vec<_>>();
    eviction_order.sort_by_key(|(_, last_touched_frame, _)| *last_touched_frame);

    for (key, _, approx_bytes) in eviction_order {
        if total_bytes <= HOME_THUMBNAIL_CACHE_MAX_BYTES {
            break;
        }
        if state.cache.remove(key.as_str()).is_some() {
            forget_home_thumbnail(ctx, key.as_str());
            total_bytes = total_bytes.saturating_sub(approx_bytes);
        }
    }
}

fn forget_home_thumbnail(ctx: &egui::Context, key: &str) {
    let Some((instance_id, path)) = key.split_once('\n') else {
        return;
    };
    let _ = ctx;
    image_textures::evict_source_key(
        home_instance_thumbnail_uri(instance_id, Path::new(path)).as_str(),
    );
}
