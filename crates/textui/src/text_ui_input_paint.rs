use super::*;

impl TextUi {
    pub(crate) fn paint_editor_gpu(
        &mut self,
        painter: &egui::Painter,
        content_rect: Rect,
        editor: &Editor<'static>,
        options: &InputOptions,
        scale: f32,
        has_focus: bool,
        show_selection_without_focus: bool,
    ) {
        let horizontal_scroll_px = editor.with_buffer(|b| b.scroll().horizontal.max(0.0));
        let selection_visible =
            has_focus || (show_selection_without_focus && editor.selection() != Selection::None);
        let selection_bounds = if selection_visible {
            editor.selection_bounds()
        } else {
            None
        };

        let origin = content_rect.min;
        let painter = painter.with_clip_rect(content_rect);

        struct GlyphCmd {
            cache_key: GlyphRasterKey,
            x_px: f32,
            y_px: f32,
            color: Color32,
        }

        let mut sel_rects: Vec<Rect> = Vec::new();
        let mut cursor_rect: Option<Rect> = None;
        let mut glyph_cmds: Vec<GlyphCmd> = Vec::new();
        let variation_settings = shared_variation_settings(&options.fundamentals);

        editor.with_buffer(|buffer| {
            let buf_width = buffer.size().0.unwrap_or(0.0);

            for run in buffer.layout_runs() {
                let line_i = run.line_i;
                let line_top = run.line_top;
                let line_y = run.line_y;
                let line_height = run.line_height;
                let prefixes = collect_glyph_spacing_prefixes_px(
                    run.text,
                    run.glyphs,
                    &options.fundamentals,
                    scale,
                );

                if let Some((start, end)) = selection_bounds
                    && line_i >= start.line
                    && line_i <= end.line
                {
                    let mut range_opt: Option<(i32, i32)> = None;

                    for (glyph_index, glyph) in run.glyphs.iter().enumerate() {
                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count().max(1);
                        let mut c_x = adjusted_glyph_x_px(glyph, prefixes[glyph_index]);
                        let c_w = glyph.w / total as f32;

                        for (i, c) in cluster.grapheme_indices(true) {
                            let c_start = glyph.start + i;
                            let c_end = glyph.start + i + c.len();
                            if (start.line != line_i || c_end > start.index)
                                && (end.line != line_i || c_start < end.index)
                            {
                                range_opt = match range_opt.take() {
                                    Some((mn, mx)) => {
                                        Some((mn.min(c_x as i32), mx.max((c_x + c_w) as i32)))
                                    }
                                    None => Some((c_x as i32, (c_x + c_w) as i32)),
                                };
                            } else if let Some((mn, mx)) = range_opt.take() {
                                sel_rects.push(editor_sel_rect(
                                    mn,
                                    mx,
                                    line_top,
                                    line_height,
                                    horizontal_scroll_px,
                                    origin,
                                    scale,
                                ));
                            }
                            c_x += c_w;
                        }
                    }

                    if run.glyphs.is_empty() && end.line > line_i {
                        range_opt = Some((0, buf_width as i32));
                    }

                    if let Some((mut mn, mut mx)) = range_opt.take() {
                        if end.line > line_i {
                            if run.rtl {
                                mn = 0;
                            } else {
                                mx = buf_width as i32;
                            }
                        }
                        sel_rects.push(editor_sel_rect(
                            mn,
                            mx,
                            line_top,
                            line_height,
                            horizontal_scroll_px,
                            origin,
                            scale,
                        ));
                    }
                }

                if has_focus
                    && let Some(cx) =
                        editor_cursor_x_in_run(&editor.cursor(), &run, &options.fundamentals, scale)
                {
                    let x_pts = (cx as f32 - horizontal_scroll_px) / scale + origin.x;
                    let y_pts = line_top / scale + origin.y;
                    let h_pts = line_height / scale;
                    cursor_rect = Some(Rect::from_min_size(
                        Pos2::new(x_pts, y_pts),
                        Vec2::new((1.0_f32 / scale).max(0.5), h_pts),
                    ));
                }

                for (glyph_index, glyph) in run.glyphs.iter().enumerate() {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let color = if selection_visible {
                        if let Some((start, end)) = selection_bounds {
                            if line_i >= start.line
                                && line_i <= end.line
                                && (start.line != line_i || glyph.end > start.index)
                                && (end.line != line_i || glyph.start < end.index)
                            {
                                options.selected_text_color
                            } else {
                                glyph
                                    .color_opt
                                    .map_or(options.text_color, cosmic_to_egui_color)
                            }
                        } else {
                            glyph
                                .color_opt
                                .map_or(options.text_color, cosmic_to_egui_color)
                        }
                    } else {
                        glyph
                            .color_opt
                            .map_or(options.text_color, cosmic_to_egui_color)
                    };

                    glyph_cmds.push(GlyphCmd {
                        cache_key: GlyphRasterKey::new(
                            physical.cache_key,
                            scale,
                            options.fundamentals.stem_darkening,
                            GlyphContentMode::AlphaMask,
                            0.0,
                            Arc::clone(&variation_settings),
                        ),
                        x_px: physical.x as f32 + prefixes[glyph_index] - horizontal_scroll_px,
                        y_px: line_y + physical.y as f32,
                        color,
                    });
                }
            }
        });

        for sel in sel_rects {
            painter.add(egui::Shape::rect_filled(
                sel,
                CornerRadius::ZERO,
                options.selection_color,
            ));
        }

        let graphics_config = self.resolved_graphics_config(self.max_texture_side_px.max(1));
        let field_range_px = graphics_config.rasterization.field_range_px.max(1.0);
        let mut quads = Vec::with_capacity(glyph_cmds.len());
        for cmd in glyph_cmds {
            let content_mode = self.resolved_glyph_content_mode(graphics_config, &cmd.cache_key);
            let raster_key = cmd.cache_key.for_content_mode(content_mode, field_range_px);
            let Some(atlas_entry) = self.glyph_atlas.resolve_or_queue(
                painter.ctx(),
                &mut self.font_system,
                &mut self.scale_context,
                raster_key,
                self.current_frame,
            ) else {
                continue;
            };

            let glyph_rect = Rect::from_min_size(
                Pos2::new(
                    (cmd.x_px + atlas_entry.placement_left_px as f32) / scale + origin.x,
                    (cmd.y_px - atlas_entry.placement_top_px as f32) / scale + origin.y,
                ),
                Vec2::new(
                    atlas_entry.size_px[0] as f32 / scale,
                    atlas_entry.size_px[1] as f32 / scale,
                ),
            );

            let tint = if atlas_entry.is_color {
                Color32::WHITE
            } else {
                cmd.color
            };

            quads.push(PaintTextQuad {
                page_index: atlas_entry.page_index,
                positions: quad_positions_from_min_size(glyph_rect.min, glyph_rect.size()),
                uvs: uv_quad_points(atlas_entry.uv),
                tint,
                content_mode: atlas_entry.content_mode,
            });
        }

        self.paint_text_quads(&painter, content_rect, &quads);

        if let Some(cursor_rect) = cursor_rect {
            painter.add(egui::Shape::rect_filled(
                cursor_rect,
                CornerRadius::ZERO,
                options.cursor_color,
            ));
        }
    }
}
