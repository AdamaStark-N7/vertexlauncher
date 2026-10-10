use super::*;

impl TextUi {
    pub fn atlas_page_snapshot(&self, page_index: usize) -> Option<TextAtlasPageSnapshot> {
        self.glyph_atlas.page_snapshot(page_index)
    }

    pub fn atlas_page_data(&self, page_index: usize) -> Option<TextAtlasPageData> {
        self.glyph_atlas.page_data(page_index)
    }

    pub fn atlas_page_snapshots_for_scene(
        &self,
        scene: &TextRenderScene,
    ) -> Vec<TextAtlasPageSnapshot> {
        scene
            .atlas_page_indices()
            .into_iter()
            .filter_map(|page_index| self.atlas_page_snapshot(page_index))
            .collect()
    }

    pub fn atlas_page_data_for_scene(&self, scene: &TextRenderScene) -> Vec<TextAtlasPageData> {
        scene
            .atlas_page_indices()
            .into_iter()
            .filter_map(|page_index| self.atlas_page_data(page_index))
            .collect()
    }

    pub fn gpu_scene_for_scene(&self, scene: &TextRenderScene) -> TextGpuScene {
        scene.to_gpu_scene(self.atlas_page_data_for_scene(scene))
    }

    pub fn gpu_scene_page_batches(&self, scene: &TextGpuScene) -> Arc<[TextGpuScenePageBatch]> {
        let fingerprint = if scene.fingerprint != 0 {
            scene.fingerprint
        } else {
            let mut hasher = new_fingerprint_hasher();
            scene.bounds_min[0].to_bits().hash(&mut hasher);
            scene.bounds_min[1].to_bits().hash(&mut hasher);
            scene.bounds_max[0].to_bits().hash(&mut hasher);
            scene.bounds_max[1].to_bits().hash(&mut hasher);
            scene.size_points[0].to_bits().hash(&mut hasher);
            scene.size_points[1].to_bits().hash(&mut hasher);
            for quad in &scene.quads {
                quad.atlas_page_index.hash(&mut hasher);
                for point in quad.positions {
                    point[0].to_bits().hash(&mut hasher);
                    point[1].to_bits().hash(&mut hasher);
                }
                for point in quad.uvs {
                    point[0].to_bits().hash(&mut hasher);
                    point[1].to_bits().hash(&mut hasher);
                }
                quad.tint_rgba.hash(&mut hasher);
            }
            hasher.finish()
        };

        if let Some(batches) = self.gpu_scene_page_batch_cache.write(|state| {
            state
                .touch(&fingerprint)
                .map(|entry| Arc::clone(&entry.value))
        }) {
            return batches;
        }

        let mut grouped = FxHashMap::<usize, Vec<TextGpuQuad>>::default();
        for quad in &scene.quads {
            grouped
                .entry(quad.atlas_page_index)
                .or_default()
                .push(quad.clone());
        }
        let mut page_indices = grouped.keys().copied().collect::<Vec<_>>();
        page_indices.sort_unstable();
        let batches_vec = page_indices
            .into_iter()
            .map(|page_index| TextGpuScenePageBatch {
                page_index,
                quads: Arc::from(
                    grouped
                        .remove(&page_index)
                        .unwrap_or_default()
                        .into_boxed_slice(),
                ),
            })
            .collect::<Vec<_>>();
        let approx_bytes = gpu_scene_page_batches_approx_bytes(&batches_vec);
        let batches = Arc::from(batches_vec.into_boxed_slice());
        self.gpu_scene_page_batch_cache.write(|state| {
            let _ = state.insert(fingerprint, Arc::clone(&batches), approx_bytes);
        });
        batches
    }

    pub fn prepare_gpu_scene_draw_batches(
        &self,
        scene: &TextGpuScene,
        options: TextGpuSceneDrawOptions,
    ) -> Arc<[TextGpuScenePageBatch]> {
        let scene_fingerprint = if scene.fingerprint != 0 {
            scene.fingerprint
        } else {
            let mut hasher = new_fingerprint_hasher();
            scene.bounds_min[0].to_bits().hash(&mut hasher);
            scene.bounds_min[1].to_bits().hash(&mut hasher);
            scene.bounds_max[0].to_bits().hash(&mut hasher);
            scene.bounds_max[1].to_bits().hash(&mut hasher);
            scene.size_points[0].to_bits().hash(&mut hasher);
            scene.size_points[1].to_bits().hash(&mut hasher);
            hasher.finish()
        };
        let mut hasher = new_fingerprint_hasher();
        scene_fingerprint.hash(&mut hasher);
        options.offset.x.to_bits().hash(&mut hasher);
        options.offset.y.to_bits().hash(&mut hasher);
        options.scale.x.to_bits().hash(&mut hasher);
        options.scale.y.to_bits().hash(&mut hasher);
        options.tint.to_array().hash(&mut hasher);
        let draw_fingerprint = hasher.finish();

        if let Some(batches) = self.gpu_scene_draw_batch_cache.write(|state| {
            state
                .touch(&draw_fingerprint)
                .map(|entry| Arc::clone(&entry.value))
        }) {
            return batches;
        }

        let source_batches = self.gpu_scene_page_batches(scene);
        let transformed_batches = source_batches
            .iter()
            .map(|batch| {
                let quads = batch
                    .quads
                    .iter()
                    .map(|quad| {
                        let positions = quad.positions.map(|point| {
                            [
                                options.offset.x + point[0] * options.scale.x,
                                options.offset.y + point[1] * options.scale.y,
                            ]
                        });
                        let tint = multiply_color32(
                            Color32::from_rgba_premultiplied(
                                quad.tint_rgba[0],
                                quad.tint_rgba[1],
                                quad.tint_rgba[2],
                                quad.tint_rgba[3],
                            ),
                            options.tint.into(),
                        );
                        TextGpuQuad {
                            atlas_page_index: quad.atlas_page_index,
                            positions,
                            uvs: quad.uvs,
                            tint_rgba: tint.to_array(),
                        }
                    })
                    .collect::<Vec<_>>();
                TextGpuScenePageBatch {
                    page_index: batch.page_index,
                    quads: Arc::from(quads.into_boxed_slice()),
                }
            })
            .collect::<Vec<_>>();
        let approx_bytes = gpu_scene_page_batches_approx_bytes(&transformed_batches);
        let batches = Arc::from(transformed_batches.into_boxed_slice());
        self.gpu_scene_draw_batch_cache.write(|state| {
            let _ = state.insert(draw_fingerprint, Arc::clone(&batches), approx_bytes);
        });
        batches
    }

    pub(super) fn paint_scene_in_rect(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        scene: &TextRenderScene,
    ) {
        self.paint_scene_in_rect_tinted(painter, rect, scene, Color32::WHITE);
    }

    pub(super) fn paint_scene_in_rect_tinted(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        scene: &TextRenderScene,
        tint: Color32,
    ) {
        let rect = snap_rect_to_pixel_grid(rect, painter.pixels_per_point());
        let quads = map_scene_quads_to_rect(
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui_vec_from_text(scene.size_points),
            &scene.quads,
            tint,
        );
        self.paint_text_quads(painter, rect, &quads);
    }

    pub(super) fn paint_text_quads(
        &mut self,
        painter: &egui::Painter,
        bounds: Rect,
        quads: &[PaintTextQuad],
    ) {
        if quads.is_empty() {
            return;
        }

        if let Some(callback) = self.build_text_wgpu_scene_callback(quads) {
            let callback_rect = bounds.intersect(painter.clip_rect());
            if callback_rect.is_positive() {
                painter.add(egui_wgpu::Callback::new_paint_callback(
                    callback_rect,
                    callback,
                ));
                return;
            }
        }

        paint_text_quads_fallback(&self.glyph_atlas, painter, quads);
    }
}
