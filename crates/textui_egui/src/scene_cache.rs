use super::*;

pub(super) const GPU_SCENE_TEXTURE_CACHE_ID: &str = "textui_egui_gpu_scene_texture_cache";
pub(super) const GPU_SCENE_TEXTURE_CACHE_STALE_FRAMES: u64 = 600;
/// Memory ceiling for the GPU textures made from text atlas pages.
pub(super) const GPU_SCENE_TEXTURE_CACHE_MAX_BYTES: usize = 160 * 1024 * 1024;
/// When a cache goes over its ceiling it evicts down to this fraction of it, so the eviction pass
/// (which frees many entries at once) is rare instead of running on every insert.
pub(super) const CACHE_EVICT_TARGET_PERCENT: usize = 80;
pub(super) const RETAINED_GPU_SCENE_CACHE_ID: &str = "textui_egui_retained_gpu_scene_cache";
pub(super) const RETAINED_GPU_SCENE_CACHE_STALE_FRAMES: u64 = 600;
/// Memory ceiling for the atlas page pixels kept alive by retained scenes.
pub(super) const RETAINED_GPU_SCENE_CACHE_MAX_BYTES: usize = 160 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct CachedGpuSceneTexture {
    pub(super) handle: TextureHandle,
    pub(super) last_used_frame: u64,
}

#[derive(Default)]
pub(super) struct GpuSceneTextureCacheState {
    pub(super) entries: HashMap<u64, CachedGpuSceneTexture>,
    /// Running total of the textures' sizes, kept up to date on insert and removal so no pass
    /// over the cache is needed to know it.
    pub(super) bytes: usize,
    /// Frame on which `entries.retain` was last run. Ensures the O(N) eviction
    /// scan runs at most once per frame rather than once per text element.
    pub(super) last_eviction_frame: u64,
}

pub(super) type GpuSceneTextureCache = Arc<Mutex<GpuSceneTextureCacheState>>;

#[derive(Clone)]
pub(super) struct CachedRetainedGpuScene {
    pub(super) fingerprint: u64,
    pub(super) scene: Arc<TextGpuScene>,
    pub(super) last_used_frame: u64,
}

#[derive(Default)]
pub(super) struct RetainedGpuSceneCacheState {
    pub(super) entries: HashMap<Id, CachedRetainedGpuScene>,
    pub(super) last_eviction_frame: u64,
    /// Page snapshots shared by retained scenes: pointer -> (bytes, scenes using it). Their bytes
    /// are counted once however many scenes share them.
    pub(super) pages: HashMap<usize, (usize, usize)>,
    pub(super) bytes: usize,
}

pub(super) type RetainedGpuSceneCache = Arc<Mutex<RetainedGpuSceneCacheState>>;

pub(super) fn texture_options_for_sampling(sampling: TextAtlasSampling) -> TextureOptions {
    match sampling {
        TextAtlasSampling::Linear => TextureOptions::LINEAR,
        TextAtlasSampling::Nearest => TextureOptions::NEAREST,
    }
}

pub(super) fn gpu_scene_texture_cache(ctx: &Context) -> GpuSceneTextureCache {
    ctx.data_mut(|data| {
        let id = Id::new(GPU_SCENE_TEXTURE_CACHE_ID);
        if let Some(cache) = data.get_temp::<GpuSceneTextureCache>(id) {
            cache
        } else {
            let cache = Arc::new(Mutex::new(GpuSceneTextureCacheState::default()));
            data.insert_temp(id, Arc::clone(&cache));
            cache
        }
    })
}

pub(super) fn retained_gpu_scene_cache(ctx: &Context) -> RetainedGpuSceneCache {
    ctx.data_mut(|data| {
        let id = Id::new(RETAINED_GPU_SCENE_CACHE_ID);
        if let Some(cache) = data.get_temp::<RetainedGpuSceneCache>(id) {
            cache
        } else {
            let cache = Arc::new(Mutex::new(RetainedGpuSceneCacheState::default()));
            data.insert_temp(id, Arc::clone(&cache));
            cache
        }
    })
}

pub(super) fn retained_gpu_scene(
    ctx: &Context,
    cache_id: Id,
    fingerprint: u64,
    build_scene: impl FnOnce() -> Option<Arc<TextGpuScene>>,
) -> Option<Arc<TextGpuScene>> {
    let current_frame = ctx.cumulative_frame_nr();
    let cache = retained_gpu_scene_cache(ctx);
    {
        let mut cache_guard = cache
            .lock()
            .expect("textui_egui retained scene cache poisoned");
        if current_frame > cache_guard.last_eviction_frame {
            cache_guard.last_eviction_frame = current_frame;
            cache_guard.retain_scenes(|entry| {
                current_frame.saturating_sub(entry.last_used_frame)
                    <= RETAINED_GPU_SCENE_CACHE_STALE_FRAMES
            });
        }
        if let Some(entry) = cache_guard.entries.get_mut(&cache_id)
            && entry.fingerprint == fingerprint
        {
            entry.last_used_frame = current_frame;
            return Some(Arc::clone(&entry.scene));
        }
    }

    let scene = build_scene()?;
    let mut cache_guard = cache
        .lock()
        .expect("textui_egui retained scene cache poisoned");
    cache_guard.insert_scene(
        cache_id,
        CachedRetainedGpuScene {
            fingerprint,
            scene: Arc::clone(&scene),
            last_used_frame: current_frame,
        },
        current_frame,
    );
    Some(scene)
}

impl GpuSceneTextureCacheState {
    pub(super) fn retain_textures(&mut self, mut keep: impl FnMut(&CachedGpuSceneTexture) -> bool) {
        let bytes = &mut self.bytes;
        self.entries.retain(|_, entry| {
            let keep = keep(entry);
            if !keep {
                *bytes = bytes.saturating_sub(entry.handle.byte_size());
            }
            keep
        });
    }

    /// Called after an insert. Does nothing until the ceiling is crossed, then drops the least
    /// recently used textures (never ones used this frame) down to a margin below it.
    pub(super) fn evict_textures_over_budget(&mut self, current_frame: u64) {
        if self.bytes <= GPU_SCENE_TEXTURE_CACHE_MAX_BYTES {
            return;
        }
        let target = GPU_SCENE_TEXTURE_CACHE_MAX_BYTES / 100 * CACHE_EVICT_TARGET_PERCENT;
        let mut by_age: Vec<(u64, u64)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_used_frame < current_frame)
            .map(|(key, entry)| (entry.last_used_frame, *key))
            .collect();
        by_age.sort_unstable();
        for (_, key) in by_age {
            if self.bytes <= target {
                break;
            }
            if let Some(entry) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(entry.handle.byte_size());
            }
        }
    }
}

impl RetainedGpuSceneCacheState {
    pub(super) fn add_pages(&mut self, scene: &TextGpuScene) {
        for page in &scene.atlas_pages {
            let slot = self
                .pages
                .entry(page.rgba8.as_ptr() as usize)
                .or_insert((page.rgba8.len(), 0));
            if slot.1 == 0 {
                self.bytes += slot.0;
            }
            slot.1 += 1;
        }
    }

    pub(super) fn remove_pages(&mut self, scene: &TextGpuScene) {
        for page in &scene.atlas_pages {
            let ptr = page.rgba8.as_ptr() as usize;
            if let Some(slot) = self.pages.get_mut(&ptr) {
                slot.1 = slot.1.saturating_sub(1);
                if slot.1 == 0 {
                    self.bytes = self.bytes.saturating_sub(slot.0);
                    self.pages.remove(&ptr);
                }
            }
        }
    }

    pub(super) fn retain_scenes(&mut self, mut keep: impl FnMut(&CachedRetainedGpuScene) -> bool) {
        let mut dropped = Vec::new();
        self.entries.retain(|_, entry| {
            let keep = keep(entry);
            if !keep {
                dropped.push(Arc::clone(&entry.scene));
            }
            keep
        });
        for scene in dropped {
            self.remove_pages(&scene);
        }
    }

    pub(super) fn insert_scene(
        &mut self,
        id: Id,
        entry: CachedRetainedGpuScene,
        current_frame: u64,
    ) {
        self.add_pages(&entry.scene);
        if let Some(old) = self.entries.insert(id, entry) {
            self.remove_pages(&old.scene);
        }
        self.evict_scenes_over_budget(current_frame);
    }

    /// Called after an insert; see [`GpuSceneTextureCacheState::evict_textures_over_budget`].
    pub(super) fn evict_scenes_over_budget(&mut self, current_frame: u64) {
        if self.bytes <= RETAINED_GPU_SCENE_CACHE_MAX_BYTES {
            return;
        }
        let target = RETAINED_GPU_SCENE_CACHE_MAX_BYTES / 100 * CACHE_EVICT_TARGET_PERCENT;
        let mut by_age: Vec<(u64, Id)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_used_frame < current_frame)
            .map(|(id, entry)| (entry.last_used_frame, *id))
            .collect();
        by_age.sort_unstable_by_key(|(frame, _)| *frame);
        for (_, id) in by_age {
            if self.bytes <= target {
                break;
            }
            if let Some(entry) = self.entries.remove(&id) {
                self.remove_pages(&entry.scene);
            }
        }
    }
}

/// Drops every cached text scene and page texture that was not used this frame. Call at the end
/// of a frame after switching screens; text for the old screen's widgets is no longer needed.
pub fn release_untouched(ctx: &Context) {
    let frame = ctx.cumulative_frame_nr();
    if let Ok(mut cache) = gpu_scene_texture_cache(ctx).lock() {
        cache.retain_textures(|entry| entry.last_used_frame >= frame);
    }
    if let Ok(mut cache) = retained_gpu_scene_cache(ctx).lock() {
        cache.retain_scenes(|entry| entry.last_used_frame >= frame);
    }
}
