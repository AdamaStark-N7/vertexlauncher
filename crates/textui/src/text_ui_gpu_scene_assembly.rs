use super::*;

/// Accumulates glyph quads and CPU atlas pages while a [`TextGpuScene`] is being built.
///
/// Shared by the straight-line and on-path GPU scene builders, which only differ in how
/// each glyph's quad positions are computed.
pub(super) struct GpuSceneAssembly {
    page_pool: Vec<CpuSceneAtlasPage>,
    pages: Vec<CpuSceneAtlasPage>,
    page_hashers: Vec<FxHasher>,
    quads: Vec<TextGpuQuad>,
    bounds: Option<Rect>,
    target_page_side_px: usize,
    atlas_padding_px: usize,
}

impl GpuSceneAssembly {
    pub(super) fn new(
        page_pool: Vec<CpuSceneAtlasPage>,
        target_page_side_px: usize,
        atlas_padding_px: usize,
        glyph_capacity: usize,
    ) -> Self {
        Self {
            page_pool,
            pages: Vec::new(),
            page_hashers: Vec::new(),
            quads: Vec::with_capacity(glyph_capacity),
            bounds: None,
            target_page_side_px,
            atlas_padding_px,
        }
    }

    /// Packs the glyph bitmap into a scene page, returning its page index and UV rect.
    pub(super) fn place_glyph(
        &mut self,
        glyph: &PreparedGlyph,
        atlas_glyph: &PreparedAtlasGlyph,
    ) -> Option<(usize, Rect)> {
        let allocation_size = size2(
            atlas_glyph.upload_image.size[0] as i32,
            atlas_glyph.upload_image.size[1] as i32,
        );
        let (page_index, allocation) = allocate_cpu_scene_page_slot(
            &mut self.pages,
            &mut self.page_pool,
            self.target_page_side_px,
            allocation_size,
        )?;

        // Initialize hasher for any newly created pages.
        while self.page_hashers.len() < self.pages.len() {
            let new_idx = self.page_hashers.len();
            let mut h = FxHasher::default();
            new_idx.hash(&mut h);
            self.pages[new_idx].size.hash(&mut h);
            self.page_hashers.push(h);
        }

        let pos = [
            allocation.rectangle.min.x.max(0) as usize,
            allocation.rectangle.min.y.max(0) as usize,
        ];
        // Hash glyph identity + position — O(glyphs) instead of O(pixels) at finalisation.
        glyph.cache_key.hash(&mut self.page_hashers[page_index]);
        pos[0].hash(&mut self.page_hashers[page_index]);
        pos[1].hash(&mut self.page_hashers[page_index]);

        let page_size = self.pages[page_index].size;
        blit_to_page(
            &mut self.pages[page_index].rgba8,
            page_size,
            &atlas_glyph.upload_image,
            pos[0],
            pos[1],
        );

        let padding = self.atlas_padding_px;
        let uv = Rect::from_min_max(
            Pos2::new(
                (pos[0] + padding) as f32 / page_size[0] as f32,
                (pos[1] + padding) as f32 / page_size[1] as f32,
            ),
            Pos2::new(
                (pos[0] + padding + atlas_glyph.size_px[0]) as f32 / page_size[0] as f32,
                (pos[1] + padding + atlas_glyph.size_px[1]) as f32 / page_size[1] as f32,
            ),
        );
        Some((page_index, uv))
    }

    /// Records the quad for a glyph already placed with [`Self::place_glyph`].
    pub(super) fn push_quad(
        &mut self,
        page_index: usize,
        uv: Rect,
        positions: [Pos2; 4],
        atlas_glyph: &PreparedAtlasGlyph,
        glyph: &PreparedGlyph,
    ) {
        let quad_bounds = rect_from_points(positions);
        self.bounds = Some(
            self.bounds
                .map_or(quad_bounds, |current| current.union(quad_bounds)),
        );
        let tint = if atlas_glyph.is_color {
            Color32::WHITE
        } else {
            glyph.color
        };
        self.quads.push(TextGpuQuad {
            atlas_page_index: page_index,
            positions: positions.map(|point| [point.x, point.y]),
            uvs: uv_quad_points(uv).map(|point| [point.x, point.y]),
            tint_rgba: [tint.r(), tint.g(), tint.b(), tint.a()],
        });
    }

    /// Finalises the page data, hands reusable page buffers back through `page_pool_out`,
    /// and returns the atlas pages, quads and accumulated bounds.
    pub(super) fn finish(
        self,
        page_pool_out: &mut Vec<CpuSceneAtlasPage>,
    ) -> (Vec<TextAtlasPageData>, Vec<TextGpuQuad>, Option<Rect>) {
        let Self {
            mut page_pool,
            mut pages,
            page_hashers,
            quads,
            bounds,
            ..
        } = self;
        let atlas_pages = pages
            .iter()
            .enumerate()
            .map(|(i, page)| {
                let content_hash = page_hashers.get(i).map(|h| h.finish()).unwrap_or(0);
                cpu_page_to_page_data(page, i, content_hash)
            })
            .collect();

        // Return pages to the pool for reuse next call (pixel buffer allocations survive).
        const CPU_PAGE_POOL_MAX: usize = 4;
        let return_count = CPU_PAGE_POOL_MAX
            .saturating_sub(page_pool.len())
            .min(pages.len());
        page_pool.extend(pages.drain(..return_count));
        *page_pool_out = page_pool;

        (atlas_pages, quads, bounds)
    }
}
