use super::*;

impl TextUi {
    /// Advances the engine-side frame state without requiring an [`egui::Context`].
    ///
    /// Consumers that use the context-free scene/export APIs can drive frame maintenance
    /// through this method and inject any relevant input/render state separately.
    pub fn begin_frame_info(&mut self, frame_info: TextFrameInfo) {
        self.current_frame = frame_info.frame_number;
        let current_frame = self.current_frame;
        let max_texture_side_px = frame_info.max_texture_side_px.max(1);
        let graphics_config = self.resolved_graphics_config(max_texture_side_px);
        self.frame_events.clear();
        self.glyph_atlas
            .set_page_side(graphics_config.atlas_page_target_px);
        self.glyph_atlas
            .set_sampling(graphics_config.atlas_sampling);
        self.glyph_atlas
            .set_padding(graphics_config.atlas_padding_px);
        self.glyph_atlas
            .set_rasterization(graphics_config.rasterization);
        self.glyph_atlas
            .set_linear_pipeline(self.graphics_config.linear_pipeline);
        if self.max_texture_side_px != max_texture_side_px {
            self.max_texture_side_px = max_texture_side_px;
            self.invalidate_text_caches(false);
        }
        self.prepared_texts.write(|state| {
            state.retain(|_, entry| {
                current_frame.saturating_sub(entry.value.last_used_frame)
                    <= INPUT_STATE_STALE_FRAMES
            });
        });
        self.markdown_cache.retain(|_, (_, last_used_frame, _)| {
            current_frame.saturating_sub(*last_used_frame) <= INPUT_STATE_STALE_FRAMES
        });
        self.input_states.retain(|_, state| {
            current_frame.saturating_sub(state.last_used_frame) <= INPUT_STATE_STALE_FRAMES
        });
        self.glyph_atlas.trim_stale(current_frame);
        self.enforce_prepared_text_cache_budget();
        self.enforce_async_raster_cache_budget();
        self.enforce_gpu_scene_cache_budget();
        self.poll_async_raster_results();
    }

    /// Replaces the per-frame input event buffer used by interactive widgets.
    pub fn set_frame_input_events(&mut self, frame_events: Vec<TextInputEvent>) {
        self.frame_events = frame_events;
    }

    /// Drops the rebuildable layout, scene and markdown caches (not the glyph atlas), so memory
    /// used by a screen's text is returned when that screen goes away. Everything is rebuilt on
    /// demand the next time the text is drawn.
    pub fn release_layout_caches(&mut self) {
        let _ = self.prepared_texts.write(|cache| cache.clear());
        let _ = self.gpu_scene_cache.write(|cache| cache.clear());
        let _ = self.gpu_scene_page_batch_cache.write(|cache| cache.clear());
        let _ = self.gpu_scene_draw_batch_cache.write(|cache| cache.clear());
        let _ = self.gpu_scene_glyph_cache.write(|cache| cache.clear());
        self.markdown_cache.clear();
        self.cpu_page_pool = Vec::new();
    }

    /// Clears any per-frame input events previously set by [`Self::set_frame_input_events`].
    pub fn clear_frame_input_events(&mut self) {
        self.frame_events.clear();
    }

    #[doc(hidden)]
    /// Updates the optional native WGPU render state used by the atlas renderer.
    ///
    /// When using native atlas pages, set this before [`Self::begin_frame_info`] so any
    /// frame-start invalidation can release old textures through the current renderer.
    pub fn egui_set_render_state(&mut self, render_state: Option<&EguiWgpuRenderState>) {
        self.glyph_atlas.set_render_state(render_state);
    }

    #[doc(hidden)]
    /// Flushes pending atlas work that still needs an [`egui::Context`] for texture uploads.
    pub fn egui_flush_frame(&mut self, ctx: &Context) -> TextFrameOutput {
        self.glyph_atlas.poll_ready(ctx, self.current_frame);
        let needs_repaint = !self.glyph_atlas.pending.is_empty();
        if needs_repaint {
            ctx.request_repaint();
        }
        TextFrameOutput { needs_repaint }
    }

    pub fn set_graphics_config(&mut self, graphics_config: TextGraphicsConfig) {
        if self.graphics_config != graphics_config {
            self.graphics_config = graphics_config;
            self.invalidate_text_caches(false);
        }
    }

    pub fn graphics_config(&self) -> TextGraphicsConfig {
        self.graphics_config
    }

    pub fn set_gpu_instancing_enabled(&mut self, enabled: bool) {
        let mut graphics_config = self.graphics_config;
        graphics_config.renderer_backend = if enabled {
            TextRendererBackend::WgpuInstanced
        } else {
            TextRendererBackend::EguiMesh
        };
        self.set_graphics_config(graphics_config);
    }

    pub fn gpu_instancing_enabled(&self) -> bool {
        !matches!(
            self.resolved_graphics_config(self.max_texture_side_px.max(1))
                .renderer_backend,
            ResolvedTextRendererBackend::EguiMesh
        )
    }

    pub(super) fn resolved_graphics_config(
        &self,
        max_texture_side_px: usize,
    ) -> ResolvedTextGraphicsConfig {
        let renderer_backend = match self.graphics_config.renderer_backend {
            TextRendererBackend::Auto => ResolvedTextRendererBackend::WgpuInstanced,
            TextRendererBackend::EguiMesh => ResolvedTextRendererBackend::EguiMesh,
            TextRendererBackend::WgpuInstanced => ResolvedTextRendererBackend::WgpuInstanced,
        };
        ResolvedTextGraphicsConfig {
            renderer_backend,
            atlas_sampling: self.graphics_config.atlas_sampling,
            atlas_page_target_px: self
                .graphics_config
                .atlas_page_target_px
                .max(256)
                .min(max_texture_side_px.max(1)),
            atlas_padding_px: self.graphics_config.atlas_padding_px,
            rasterization: self.graphics_config.rasterization,
            output_is_hdr: self.graphics_config.output_is_hdr,
        }
    }

    pub(super) fn resolved_glyph_content_mode(
        &self,
        graphics_config: ResolvedTextGraphicsConfig,
        glyph: &GlyphRasterKey,
    ) -> GlyphContentMode {
        if graphics_config.renderer_backend != ResolvedTextRendererBackend::WgpuInstanced {
            return GlyphContentMode::AlphaMask;
        }
        if self.glyph_atlas.wgpu_render_state.is_none() {
            return GlyphContentMode::AlphaMask;
        }

        match graphics_config.rasterization.glyph_raster_mode {
            TextGlyphRasterMode::Auto => {
                let logical_font_size = glyph_logical_font_size_points(glyph);
                if logical_font_size >= AUTO_MSDF_MIN_LOGICAL_FONT_SIZE_PT {
                    GlyphContentMode::Msdf
                } else {
                    GlyphContentMode::Sdf
                }
            }
            mode => glyph_content_mode_from_rasterization(mode),
        }
    }

    pub(super) fn invalidate_text_caches(&mut self, clear_input_states: bool) {
        let _ = self.prepared_texts.write(|state| state.clear());
        let _ = self.async_raster.cache.write(|state| state.clear());
        self.async_raster.pending.clear();
        self.glyph_atlas.clear();
        self.markdown_cache.clear();
        let _ = self.gpu_scene_cache.write(|state| state.clear());
        let _ = self.gpu_scene_page_batch_cache.write(|state| state.clear());
        let _ = self.gpu_scene_draw_batch_cache.write(|state| state.clear());
        let _ = self.gpu_scene_glyph_cache.write(|state| state.clear());
        if clear_input_states {
            self.input_states.clear();
        }
    }

    pub(super) fn enforce_prepared_text_cache_budget(&mut self) {
        self.prepared_texts.write(|state| {
            let _ = state.evict_to_budget();
        });
    }

    pub(super) fn enforce_gpu_scene_cache_budget(&mut self) {
        self.gpu_scene_cache.write(|state| {
            let _ = state.evict_to_budget();
        });
        self.gpu_scene_glyph_cache.write(|state| {
            let _ = state.evict_to_budget();
        });
    }
}
