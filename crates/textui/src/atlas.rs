use super::*;

#[path = "atlas/dirty_atlas_rect.rs"]
mod dirty_atlas_rect;
#[path = "atlas/field_line_segment.rs"]
mod field_line_segment;
#[path = "atlas/field_raster.rs"]
mod field_raster;
#[path = "atlas/flattened_outline.rs"]
mod flattened_outline;
#[path = "atlas/glyph_atlas_texture.rs"]
mod glyph_atlas_texture;
#[path = "atlas/glyph_atlas_worker_message.rs"]
mod glyph_atlas_worker_message;
#[path = "atlas/glyph_metrics.rs"]
mod glyph_metrics;
#[path = "atlas/glyph_raster.rs"]
mod glyph_raster;
#[path = "atlas/image_upload.rs"]
mod image_upload;
#[path = "atlas/native_glyph_atlas_texture.rs"]
mod native_glyph_atlas_texture;

use self::dirty_atlas_rect::DirtyAtlasRect;
use self::field_line_segment::FieldLineSegment;
use self::field_raster::*;
use self::flattened_outline::FlattenedOutline;
use self::glyph_atlas_texture::GlyphAtlasTexture;
use self::glyph_atlas_worker_message::GlyphAtlasWorkerMessage;
pub(super) use self::glyph_metrics::*;
pub(super) use self::glyph_raster::*;
pub(super) use self::image_upload::*;
use self::native_glyph_atlas_texture::NativeGlyphAtlasTexture;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct GlyphRasterKey {
    cache_key: CacheKey,
    display_scale_bits: u32,
    raster_flags: u8,
    content_mode: GlyphContentMode,
    field_range_bits: u32,
    variation_settings: Arc<[TextVariationSetting]>,
}

impl GlyphRasterKey {
    const STEM_DARKENING: u8 = 1 << 0;

    #[inline]
    pub(super) fn new(
        cache_key: CacheKey,
        display_scale: f32,
        stem_darkening: bool,
        content_mode: GlyphContentMode,
        field_range_px: f32,
        variation_settings: Arc<[TextVariationSetting]>,
    ) -> Self {
        Self {
            cache_key,
            display_scale_bits: display_scale.to_bits(),
            raster_flags: if stem_darkening {
                Self::STEM_DARKENING
            } else {
                0
            },
            content_mode,
            field_range_bits: field_range_px.to_bits(),
            variation_settings,
        }
    }

    #[inline]
    pub(super) fn display_scale(&self) -> f32 {
        f32::from_bits(self.display_scale_bits)
    }

    #[inline]
    pub(super) fn stem_darkening(&self) -> bool {
        self.raster_flags & Self::STEM_DARKENING != 0
    }

    #[inline]
    pub(super) fn content_mode(&self) -> GlyphContentMode {
        self.content_mode
    }

    #[inline]
    pub(super) fn field_range_px(&self) -> f32 {
        f32::from_bits(self.field_range_bits)
    }

    pub(super) fn for_content_mode(
        &self,
        content_mode: GlyphContentMode,
        field_range_px: f32,
    ) -> Self {
        let mut key = self.clone();
        key.content_mode = content_mode;
        key.field_range_bits = field_range_px.to_bits();
        if content_mode != GlyphContentMode::AlphaMask {
            key.cache_key.x_bin = SubpixelBin::Zero;
            key.cache_key.y_bin = SubpixelBin::Zero;
        }
        key
    }
}

#[inline]
pub(super) fn glyph_logical_font_size_points(glyph: &GlyphRasterKey) -> f32 {
    let ppem = f32::from_bits(glyph.cache_key.font_size_bits);
    let display_scale = glyph.display_scale().max(1.0);
    (ppem / display_scale).max(1.0)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum GlyphContentMode {
    AlphaMask,
    Sdf,
    Msdf,
}

pub(super) struct GlyphAtlas {
    entries: ThreadSafeLru<GlyphRasterKey, GlyphAtlasEntry>,
    pages: Vec<GlyphAtlasPage>,
    page_side_px: usize,
    padding_px: usize,
    sampling: TextAtlasSampling,
    rasterization: TextRasterizationConfig,
    /// Kept to coordinate atlas recreation when the broader text rendering
    /// pipeline switches modes. Atlas pages themselves stay FP16.
    linear_pipeline: bool,
    pub(super) wgpu_render_state: Option<EguiWgpuRenderState>,
    pub(super) pending: FxHashSet<GlyphRasterKey>,
    ready: VecDeque<GlyphAtlasWorkerResponse>,
    generation: u64,
    tx: Option<mpsc::Sender<GlyphAtlasWorkerMessage>>,
    rx: Option<mpsc::Receiver<GlyphAtlasWorkerResponse>>,
}

pub(super) struct GlyphAtlasPage {
    allocator: AtlasAllocator,
    content_mode: GlyphContentMode,
    texture: GlyphAtlasTexture,
    backing: ColorImage,
    cached_page_data: Mutex<Option<TextAtlasPageData>>,
    dirty_rect: Option<DirtyAtlasRect>,
    live_glyphs: usize,
}

#[derive(Clone, Debug)]
pub(super) struct GlyphAtlasEntry {
    page_index: usize,
    allocation_id: AllocId,
    atlas_min_px: [usize; 2],
    size_px: [usize; 2],
    placement_left_px: i32,
    placement_top_px: i32,
    is_color: bool,
    content_mode: GlyphContentMode,
    last_used_frame: u64,
    approx_bytes: usize,
}

#[derive(Clone)]
pub(super) struct ResolvedGlyphAtlasEntry {
    pub(super) page_index: usize,
    pub(super) uv: Rect,
    pub(super) size_px: [usize; 2],
    pub(super) placement_left_px: i32,
    pub(super) placement_top_px: i32,
    pub(super) is_color: bool,
    pub(super) content_mode: GlyphContentMode,
}

#[derive(Clone)]
pub(super) struct PreparedAtlasGlyph {
    pub(super) upload_image: ColorImage,
    pub(super) size_px: [usize; 2],
    pub(super) placement_left_px: i32,
    pub(super) placement_top_px: i32,
    pub(super) is_color: bool,
    pub(super) content_mode: GlyphContentMode,
    pub(super) approx_bytes: usize,
}

#[derive(Clone, Debug)]
pub(super) struct PaintTextQuad {
    pub(super) page_index: usize,
    pub(super) positions: [Pos2; 4],
    pub(super) uvs: [Pos2; 4],
    pub(super) tint: Color32,
    pub(super) content_mode: GlyphContentMode,
}

pub(super) struct GlyphAtlasWorkerResponse {
    generation: u64,
    cache_key: GlyphRasterKey,
    glyph: Option<PreparedAtlasGlyph>,
}

/// High-level text rendering engine built on cosmic-text + Swash.

impl GlyphAtlas {
    pub(super) fn new() -> Self {
        let (tx, rx) = mpsc::channel::<GlyphAtlasWorkerMessage>();
        let (result_tx, result_rx) = mpsc::channel::<GlyphAtlasWorkerResponse>();
        let _ =
            tokio_runtime::spawn_blocking_detached(move || glyph_atlas_worker_loop(rx, result_tx));
        Self {
            entries: ThreadSafeLru::new(GLYPH_ATLAS_MAX_BYTES),
            pages: Vec::new(),
            page_side_px: GLYPH_ATLAS_PAGE_TARGET_PX,
            padding_px: GLYPH_ATLAS_PADDING_PX.max(0) as usize,
            sampling: TextAtlasSampling::Linear,
            rasterization: TextRasterizationConfig::default(),
            linear_pipeline: false,
            wgpu_render_state: None,
            pending: FxHashSet::default(),
            ready: VecDeque::new(),
            generation: 0,
            tx: Some(tx),
            rx: Some(result_rx),
        }
    }

    pub(super) fn set_render_state(&mut self, render_state: Option<&EguiWgpuRenderState>) {
        let render_state_changed = match (&self.wgpu_render_state, render_state) {
            (Some(current), Some(next)) => {
                !Arc::ptr_eq(&current.renderer, &next.renderer)
                    || current.target_format != next.target_format
            }
            (None, None) => false,
            _ => true,
        };
        if render_state_changed {
            self.generation = self.generation.saturating_add(1);
            self.pending.clear();
            self.ready.clear();
            let _ = self.entries.write(|state| state.clear());
            self.free_all_pages();
        }
        self.wgpu_render_state = render_state.cloned();
    }

    pub(super) fn set_page_side(&mut self, page_side_px: usize) {
        self.page_side_px = page_side_px.max(1);
    }

    pub(super) fn set_sampling(&mut self, sampling: TextAtlasSampling) {
        self.sampling = sampling;
    }

    pub(super) fn set_padding(&mut self, padding_px: usize) {
        self.padding_px = padding_px;
    }

    pub(super) fn set_rasterization(&mut self, rasterization: TextRasterizationConfig) {
        self.rasterization = rasterization;
    }

    /// Enables or disables the linear pipeline.
    ///
    /// When changed, all atlas pages are cleared so the atlas contents are
    /// rebuilt against the current text pipeline assumptions.
    pub(super) fn set_linear_pipeline(&mut self, linear_pipeline: bool) {
        if self.linear_pipeline == linear_pipeline {
            return;
        }
        self.linear_pipeline = linear_pipeline;
        self.generation = self.generation.saturating_add(1);
        self.pending.clear();
        self.ready.clear();
        let _ = self.entries.write(|state| state.clear());
        self.free_all_pages();
    }

    pub(super) fn register_font(&self, bytes: Vec<u8>) {
        if let Some(tx) = self.tx.as_ref() {
            let _ = tx.send(GlyphAtlasWorkerMessage::RegisterFont(bytes));
        }
    }

    pub(super) fn clear(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.pending.clear();
        self.ready.clear();
        let _ = self.entries.write(|state| state.clear());
        self.free_all_pages();
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn poll_ready(&mut self, ctx: &Context, current_frame: u64) {
        let Some(rx) = self.rx.as_ref() else {
            return;
        };
        let mut worker_disconnected = false;
        for _ in 0..GLYPH_ATLAS_FETCH_MAX_PER_FRAME {
            match rx.try_recv() {
                Ok(response) => self.ready.push_back(response),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    worker_disconnected = true;
                    break;
                }
            }
        }

        let mut uploaded_glyphs = 0usize;
        let mut uploaded_bytes = 0usize;
        while uploaded_glyphs < GLYPH_ATLAS_UPLOAD_MAX_GLYPHS_PER_FRAME
            && uploaded_bytes < GLYPH_ATLAS_UPLOAD_MAX_BYTES_PER_FRAME
        {
            let Some(response) = self.ready.pop_front() else {
                break;
            };
            if response.generation != self.generation {
                continue;
            }
            self.pending.remove(&response.cache_key);
            if self
                .entries
                .read(|state| state.contains_key(&response.cache_key))
            {
                continue;
            }
            if let Some(glyph) = response.glyph {
                uploaded_glyphs = uploaded_glyphs.saturating_add(1);
                uploaded_bytes = uploaded_bytes.saturating_add(glyph.approx_bytes);
                self.insert_prepared_glyph(ctx, response.cache_key, glyph, current_frame, false);
            }
        }

        self.flush_dirty_pages();

        if worker_disconnected {
            self.tx = None;
            self.rx = None;
            self.pending.clear();
            self.ready.clear();
        }
    }

    pub(super) fn trim_stale(&mut self, current_frame: u64) {
        let stale_before = current_frame.saturating_sub(GLYPH_ATLAS_STALE_FRAMES);
        let evicted = self
            .entries
            .write(|state| state.retain(|_, entry| entry.value.last_used_frame >= stale_before));
        for (_, entry) in evicted {
            self.deallocate_entry(entry);
        }
    }

    fn free_all_pages(&mut self) {
        if let Some(render_state) = self.wgpu_render_state.as_ref() {
            let mut renderer = render_state.renderer.write();
            for page in self.pages.drain(..) {
                if let GlyphAtlasTexture::Wgpu(texture) = page.texture {
                    renderer.free_texture(&texture.id);
                }
            }
        } else {
            self.pages.clear();
        }
    }

    pub(super) fn resolve_or_queue(
        &mut self,
        ctx: &Context,
        font_system: &mut FontSystem,
        scale_context: &mut ScaleContext,
        cache_key: GlyphRasterKey,
        current_frame: u64,
    ) -> Option<ResolvedGlyphAtlasEntry> {
        if let Some(entry) = self.entries.write(|state| {
            let entry = state.touch(&cache_key)?;
            entry.value.last_used_frame = current_frame;
            Some(entry.value.clone())
        }) {
            return Some(self.resolve_entry(&entry));
        }

        if !self.pending.contains(&cache_key) {
            let queued = self.tx.as_ref().is_some_and(|tx| {
                tx.send(GlyphAtlasWorkerMessage::Rasterize {
                    generation: self.generation,
                    cache_key: cache_key.clone(),
                    rasterization: self.rasterization,
                    padding_px: self.padding_px,
                })
                .is_ok()
            });
            if queued {
                self.pending.insert(cache_key);
                ctx.request_repaint();
                return None;
            }
        }

        let glyph = rasterize_atlas_glyph(
            font_system,
            scale_context,
            &cache_key,
            self.rasterization,
            self.padding_px,
        )?;
        self.insert_prepared_glyph(ctx, cache_key, glyph, current_frame, true)
    }

    pub(super) fn resolve_sync(
        &mut self,
        ctx: &Context,
        font_system: &mut FontSystem,
        scale_context: &mut ScaleContext,
        cache_key: GlyphRasterKey,
        current_frame: u64,
    ) -> Option<ResolvedGlyphAtlasEntry> {
        if let Some(entry) = self.entries.write(|state| {
            let entry = state.touch(&cache_key)?;
            entry.value.last_used_frame = current_frame;
            Some(entry.value.clone())
        }) {
            return Some(self.resolve_entry(&entry));
        }

        let glyph = rasterize_atlas_glyph(
            font_system,
            scale_context,
            &cache_key,
            self.rasterization,
            self.padding_px,
        )?;
        self.insert_prepared_glyph(ctx, cache_key, glyph, current_frame, true)
    }

    fn insert_prepared_glyph(
        &mut self,
        ctx: &Context,
        cache_key: GlyphRasterKey,
        glyph: PreparedAtlasGlyph,
        current_frame: u64,
        flush_immediately: bool,
    ) -> Option<ResolvedGlyphAtlasEntry> {
        let allocation_size = size2(
            glyph.upload_image.size[0] as i32,
            glyph.upload_image.size[1] as i32,
        );
        if allocation_size.width > self.page_side_px as i32
            || allocation_size.height > self.page_side_px as i32
        {
            return None;
        }

        let (page_index, allocation) = loop {
            if let Some(found) = self.try_allocate(allocation_size, glyph.content_mode) {
                break found;
            }
            if self.try_add_page(ctx, glyph.content_mode) {
                continue;
            }
            if !self.evict_one_lru() {
                return None;
            }
        };

        self.write_glyph(page_index, allocation, &glyph.upload_image);

        let entry = GlyphAtlasEntry {
            page_index,
            allocation_id: allocation.id,
            atlas_min_px: [
                (allocation.rectangle.min.x + self.padding_px as i32) as usize,
                (allocation.rectangle.min.y + self.padding_px as i32) as usize,
            ],
            size_px: glyph.size_px,
            placement_left_px: glyph.placement_left_px,
            placement_top_px: glyph.placement_top_px,
            is_color: glyph.is_color,
            content_mode: glyph.content_mode,
            last_used_frame: current_frame,
            approx_bytes: glyph.approx_bytes,
        };
        let resolved = self.resolve_entry(&entry);
        let approx_bytes = entry.approx_bytes;
        self.entries.write(|state| {
            state.insert_without_eviction(cache_key, entry, approx_bytes);
        });
        if flush_immediately {
            self.flush_page_upload(page_index);
        }
        Some(resolved)
    }

    fn try_allocate(
        &mut self,
        size: etagere::Size,
        content_mode: GlyphContentMode,
    ) -> Option<(usize, Allocation)> {
        for (page_index, page) in self.pages.iter_mut().enumerate() {
            if page.content_mode != content_mode {
                continue;
            }
            if let Some(allocation) = page.allocator.allocate(size) {
                return Some((page_index, allocation));
            }
        }
        None
    }

    fn try_add_page(&mut self, ctx: &Context, content_mode: GlyphContentMode) -> bool {
        let side = self.page_side_px;
        let side_i = side as i32;

        // Reuse any page that has been fully evicted — reset its allocator in place.
        // The GPU texture is kept as-is; stale pixels at unreachable UVs are harmless.
        for page in &mut self.pages {
            if page.live_glyphs == 0 && page.content_mode == content_mode {
                page.allocator = AtlasAllocator::new(size2(side_i, side_i));
                return true;
            }
        }

        // No reusable page; allocate a fresh GPU texture.
        let texture = self.allocate_page_texture(ctx, side);
        self.pages.push(GlyphAtlasPage {
            allocator: AtlasAllocator::new(size2(side_i, side_i)),
            content_mode,
            texture,
            backing: ColorImage::filled([side, side], Color32::TRANSPARENT),
            cached_page_data: Mutex::new(None),
            dirty_rect: None,
            live_glyphs: 0,
        });
        true
    }

    fn allocate_page_texture(&mut self, ctx: &Context, side: usize) -> GlyphAtlasTexture {
        if let Some(render_state) = self.wgpu_render_state.as_ref() {
            let atlas_format = wgpu::TextureFormat::Rgba16Float;
            let texture = render_state
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("textui_glyph_atlas"),
                    size: wgpu::Extent3d {
                        width: side as u32,
                        height: side as u32,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: atlas_format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[atlas_format],
                });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let id = render_state.renderer.write().register_native_texture(
                &render_state.device,
                &view,
                wgpu_filter_mode_for_sampling(self.sampling),
            );
            GlyphAtlasTexture::Wgpu(NativeGlyphAtlasTexture { id, texture })
        } else {
            GlyphAtlasTexture::Egui(ctx.load_texture(
                format!("textui_glyph_atlas_{}", self.pages.len()),
                ColorImage::filled([side, side], Color32::TRANSPARENT),
                texture_options_for_sampling(self.sampling),
            ))
        }
    }

    fn evict_one_lru(&mut self) -> bool {
        let removed = self.entries.write(|state| state.pop_lru());
        if let Some((_, entry)) = removed {
            self.deallocate_entry(entry);
            true
        } else {
            false
        }
    }

    fn deallocate_entry(&mut self, entry: GlyphAtlasEntry) {
        let Some(page) = self.pages.get_mut(entry.page_index) else {
            return;
        };
        page.allocator.deallocate(entry.allocation_id);
        page.live_glyphs = page.live_glyphs.saturating_sub(1);
        // Empty pages are reclaimed by try_add_page on the next allocation demand;
        // we do not remove them here so that existing page_index values stay valid.
    }

    fn resolve_entry(&self, entry: &GlyphAtlasEntry) -> ResolvedGlyphAtlasEntry {
        let side = self.page_side_px as f32;
        let uv = Rect::from_min_max(
            Pos2::new(
                entry.atlas_min_px[0] as f32 / side,
                entry.atlas_min_px[1] as f32 / side,
            ),
            Pos2::new(
                (entry.atlas_min_px[0] + entry.size_px[0]) as f32 / side,
                (entry.atlas_min_px[1] + entry.size_px[1]) as f32 / side,
            ),
        );

        ResolvedGlyphAtlasEntry {
            page_index: entry.page_index,
            uv,
            size_px: entry.size_px,
            placement_left_px: entry.placement_left_px,
            placement_top_px: entry.placement_top_px,
            is_color: entry.is_color,
            content_mode: entry.content_mode,
        }
    }

    pub(super) fn page_snapshot(&self, page_index: usize) -> Option<TextAtlasPageSnapshot> {
        let page = self.pages.get(page_index)?;
        // Safety: Color32 is repr(C) struct([u8; 4]) — identical layout to 4 contiguous u8 bytes.
        let rgba8 = unsafe {
            std::slice::from_raw_parts(
                page.backing.pixels.as_ptr() as *const u8,
                page.backing.pixels.len() * 4,
            )
        }
        .to_vec();
        Some(TextAtlasPageSnapshot {
            page_index,
            size_px: page.backing.size,
            rgba8,
        })
    }

    pub(super) fn page_data(&self, page_index: usize) -> Option<TextAtlasPageData> {
        let page = self.pages.get(page_index)?;
        if let Ok(cached) = page.cached_page_data.lock()
            && let Some(data) = cached.as_ref()
        {
            return Some(data.clone());
        }

        let data = color_image_to_page_data(page_index, &page.backing);
        if let Ok(mut cached) = page.cached_page_data.lock() {
            *cached = Some(data.clone());
        }
        Some(data)
    }

    pub(super) fn texture_id_for_page(&self, page_index: usize) -> Option<TextureId> {
        self.pages.get(page_index).map(|page| page.texture.id())
    }

    pub(super) fn native_texture_for_page(&self, page_index: usize) -> Option<wgpu::Texture> {
        let page = self.pages.get(page_index)?;
        match &page.texture {
            GlyphAtlasTexture::Wgpu(texture) => Some(texture.texture.clone()),
            GlyphAtlasTexture::Egui(_) => None,
        }
    }

    fn write_glyph(&mut self, page_index: usize, allocation: Allocation, glyph: &ColorImage) {
        if glyph.size[0] == 0 || glyph.size[1] == 0 {
            return;
        }
        let Some(page) = self.pages.get_mut(page_index) else {
            return;
        };

        let pos = [
            allocation.rectangle.min.x.max(0) as usize,
            allocation.rectangle.min.y.max(0) as usize,
        ];
        blit_color_image(&mut page.backing, glyph, pos[0], pos[1]);
        if let Ok(mut cached) = page.cached_page_data.lock() {
            *cached = None;
        }
        let dirty = DirtyAtlasRect::new(pos, glyph.size);
        page.dirty_rect = Some(
            page.dirty_rect
                .map_or(dirty, |existing| existing.union(dirty)),
        );
        page.live_glyphs = page.live_glyphs.saturating_add(1);
    }

    fn flush_dirty_pages(&mut self) {
        for page_index in 0..self.pages.len() {
            self.flush_page_upload(page_index);
        }
    }

    fn flush_page_upload(&mut self, page_index: usize) {
        let Some(page) = self.pages.get_mut(page_index) else {
            return;
        };
        let Some(dirty_rect) = page.dirty_rect.take() else {
            return;
        };
        let image = color_image_sub_image(&page.backing, dirty_rect);
        match &mut page.texture {
            GlyphAtlasTexture::Egui(texture) => {
                texture.set_partial(
                    dirty_rect.min,
                    egui::ImageData::Color(image.into()),
                    texture_options_for_sampling(self.sampling),
                );
            }
            GlyphAtlasTexture::Wgpu(texture) => {
                if let Some(render_state) = self.wgpu_render_state.as_ref() {
                    write_color_image_to_wgpu_texture(
                        &render_state.queue,
                        &texture.texture,
                        dirty_rect.min,
                        &image,
                    );
                }
            }
        }
    }
}

impl Drop for GlyphAtlas {
    fn drop(&mut self) {
        self.pending.clear();
        self.ready.clear();
        self.free_all_pages();
    }
}
