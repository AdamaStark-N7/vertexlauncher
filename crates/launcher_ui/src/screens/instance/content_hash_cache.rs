use super::*;

pub(super) fn poll_content_hash_cache_save_results(state: &mut InstanceScreenState) {
    let drained = state.content_hash_cache_save_results.drain();
    if drained.disconnected {
        tracing::error!(target: "vertexlauncher/instance_content", "content_hash_cache_save_results receiver was poisoned; channel reset.");
    }
    for (saved_serial, result) in drained.items {
        state.content_hash_cache_save_in_flight = false;
        match result {
            Ok(()) => {
                if saved_serial == state.content_hash_cache_serial {
                    state.content_hash_cache_dirty = false;
                    state.content_hash_cache_dirty_since = None;
                }
            }
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/instance_content",
                    serial = saved_serial,
                    error = %err,
                    "Failed to save installed-content hash cache."
                );
                if state.content_hash_cache_dirty_since.is_none() {
                    state.content_hash_cache_dirty_since = Some(Instant::now());
                }
            }
        }
    }
}

pub(super) fn poll_content_hash_cache_load_results(state: &mut InstanceScreenState) {
    let drained = state.content_hash_cache_load_results.drain();
    if drained.disconnected {
        tracing::error!(target: "vertexlauncher/instance_content", "content_hash_cache_load_results receiver was poisoned; channel reset.");
    }
    for result in drained.items {
        state.content_hash_cache_load_in_flight = false;
        if state.content_hash_cache.is_some() {
            continue;
        }
        match result {
            Ok(cache) => state.content_hash_cache = Some(cache),
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/instance_content",
                    error = %err,
                    "Failed to load installed-content hash cache; using an empty cache."
                );
                state.content_hash_cache = Some(InstalledContentHashCache::default());
            }
        }
    }
}

pub(super) fn ensure_content_hash_cache_loaded(
    state: &mut InstanceScreenState,
    instance_root: &Path,
) {
    if state.content_hash_cache.is_some() || state.content_hash_cache_load_in_flight {
        return;
    }
    let tx = state.content_hash_cache_load_results.sender();

    let instance_root = instance_root.to_path_buf();
    state.content_hash_cache_load_in_flight = true;
    let _ = tokio_runtime::spawn_detached(async move {
        let result = tokio_runtime::spawn_blocking(move || {
            Ok::<_, String>(InstalledContentResolver::load_hash_cache(
                instance_root.as_path(),
            ))
        })
        .await
        .map_err(|err| err.to_string())
        .and_then(|result| result);
        let _ = tx.send(result);
    });
}

pub(super) fn flush_content_hash_cache(state: &mut InstanceScreenState, instance_root: &Path) {
    if !state.content_hash_cache_dirty || state.content_hash_cache_save_in_flight {
        return;
    }
    let Some(dirty_since) = state.content_hash_cache_dirty_since else {
        state.content_hash_cache_dirty_since = Some(Instant::now());
        return;
    };
    if dirty_since.elapsed() < CONTENT_HASH_CACHE_FLUSH_DEBOUNCE {
        return;
    }

    let Some(cache) = state.content_hash_cache.clone() else {
        return;
    };
    let tx = state.content_hash_cache_save_results.sender();
    let instance_root = instance_root.to_path_buf();
    let serial = state.content_hash_cache_serial;
    state.content_hash_cache_save_in_flight = true;
    let _ = tokio_runtime::spawn_detached(async move {
        let result = tokio_runtime::spawn_blocking(move || {
            InstalledContentResolver::save_hash_cache(instance_root.as_path(), &cache)
                .map_err(|err| err.to_string())
        })
        .await;
        let result = match result {
            Ok(result) => result,
            Err(err) => Err(err.to_string()),
        };
        let _ = tx.send((serial, result));
    });
}

pub(super) fn clear_content_hash_cache(state: &mut InstanceScreenState, instance_root: &Path) {
    state.content_hash_cache = Some(InstalledContentHashCache::default());
    state.content_hash_cache_load_in_flight = false;
    state.content_hash_cache_dirty = false;
    state.content_hash_cache_dirty_since = None;
    state.content_hash_cache_serial = state.content_hash_cache_serial.saturating_add(1);
    state.content_hash_cache_save_in_flight = false;
    let instance_root = instance_root.to_path_buf();
    let _ = tokio_runtime::spawn_detached(async move {
        let _ = tokio_runtime::spawn_blocking(move || {
            InstalledContentResolver::clear_hash_cache(instance_root.as_path())
        })
        .await;
    });
}
