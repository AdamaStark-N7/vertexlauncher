use super::*;

impl TextUi {
    #[allow(dead_code)]
    pub(super) fn label_impl(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &str,
        options: &LabelOptions,
        sense: Sense,
        async_mode: bool,
    ) -> Response {
        // Apply smart-quote transformation for display when enabled.
        let display_text;
        let text = if options.fundamentals.smart_quotes {
            display_text = apply_smart_quotes(text);
            display_text.as_str()
        } else {
            text
        };

        let scale = ui.ctx().pixels_per_point();
        // Snap available_width to bin boundaries so sub-pixel jitter
        // (scrollbars appearing, fractional DPI) does not bust the cache for
        // every label on screen simultaneously.
        let width_points_opt = if options.wrap {
            Some(snap_width_to_bin(ui.available_width().max(1.0), scale))
        } else {
            None
        };

        let mut hasher = new_fingerprint_hasher();
        "label".hash(&mut hasher);
        text.hash(&mut hasher);
        options.font_size.to_bits().hash(&mut hasher);
        options.line_height.to_bits().hash(&mut hasher);
        options.wrap.hash(&mut hasher);
        options.monospace.hash(&mut hasher);
        options.weight.hash(&mut hasher);
        options.italic.hash(&mut hasher);
        options.color.hash(&mut hasher);
        hash_text_fundamentals(&options.fundamentals, &mut hasher);
        scale.to_bits().hash(&mut hasher);
        width_points_opt
            .map(f32::to_bits)
            .unwrap_or(0)
            .hash(&mut hasher);
        self.hash_typography(&mut hasher);
        let fingerprint = hasher.finish();
        let cache_id = ui.make_persistent_id(id_source).with("textui_label");
        let scene = if async_mode {
            match self.get_or_queue_async_plain_layout(
                fingerprint,
                text.to_owned(),
                options,
                width_points_opt,
                scale,
            ) {
                Some(layout) => self.build_text_scene_from_layout(ui.ctx(), &layout, scale),
                None => {
                    let fallback_height = (options.line_height + options.padding.y * 2.0).max(20.0);
                    let fallback_width =
                        width_points_opt.unwrap_or_else(|| ui.available_width().max(1.0));
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(fallback_width, fallback_height), sense);
                    ui.painter().rect_filled(
                        rect,
                        CornerRadius::same(4),
                        ui.visuals().faint_bg_color,
                    );
                    ui.ctx().request_repaint();
                    return response;
                }
            }
        } else {
            let layout =
                self.get_or_prepare_label_layout(cache_id, text, options, width_points_opt, scale);
            self.build_text_scene_from_layout(ui.ctx(), &layout, scale)
        };
        let scene_size = egui_vec_from_text(scene.size_points);
        if scene_size == Vec2::ZERO {
            let fallback_height = (options.line_height + options.padding.y * 2.0).max(20.0);
            let fallback_width = width_points_opt.unwrap_or_else(|| ui.available_width().max(1.0));
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(fallback_width, fallback_height), sense);
            ui.painter()
                .rect_filled(rect, CornerRadius::same(4), ui.visuals().faint_bg_color);
            ui.ctx().request_repaint();
            return response;
        }

        let desired_size = scene_size + options.padding * 2.0;
        let (rect, response) = ui.allocate_exact_size(desired_size, sense);
        let image_rect = Rect::from_min_size(rect.min + options.padding, scene_size);
        let painter = ui.painter().with_clip_rect(ui.clip_rect());
        self.paint_scene_in_rect(&painter, image_rect, &scene);

        response
    }

    /// Renders a button with text styles from [`ButtonOptions`].
    #[allow(dead_code)]
    pub(super) fn button(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &str,
        options: &ButtonOptions,
    ) -> Response {
        self.button_impl(ui, id_source, text, false, options)
    }

    /// Renders a selectable button variant.
    #[allow(dead_code)]
    pub(super) fn selectable_button(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &str,
        selected: bool,
        options: &ButtonOptions,
    ) -> Response {
        self.button_impl(ui, id_source, text, selected, options)
    }

    #[allow(dead_code)]
    pub(super) fn button_impl(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        text: &str,
        selected: bool,
        options: &ButtonOptions,
    ) -> Response {
        let mut label_style = LabelOptions::default();
        label_style.font_size = options.font_size;
        label_style.line_height = options.line_height;
        label_style.color = options.text_color;
        label_style.wrap = false;

        let scale = ui.ctx().pixels_per_point();
        let text_cache_id = ui.make_persistent_id(id_source).with("button_text");
        let text_layout =
            self.get_or_prepare_label_layout(text_cache_id, text, &label_style, None, scale);
        let text_scene = self.build_text_scene_from_layout(ui.ctx(), &text_layout, scale);
        let text_size = egui_vec_from_text(text_scene.size_points);

        let desired_size = egui::vec2(
            (text_size.x + options.padding.x * 2.0).max(options.min_size.x),
            (text_size.y + options.padding.y * 2.0).max(options.min_size.y),
        );

        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
        let has_focus = response.has_focus();

        let fill = if response.is_pointer_button_down_on() {
            options.fill_active
        } else if response.hovered() {
            options.fill_hovered
        } else if selected || has_focus {
            options.fill_selected
        } else {
            options.fill
        };
        let stroke = if has_focus {
            ui.visuals().selection.stroke
        } else {
            options.stroke
        };

        ui.painter()
            .rect_filled(rect, CornerRadius::same(options.corner_radius), fill);
        if stroke.width > 0.0 {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(options.corner_radius),
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        if has_focus {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(options.corner_radius.saturating_add(2)),
                egui::Stroke::new(
                    (ui.visuals().selection.stroke.width + 1.0).max(2.0),
                    ui.visuals().selection.stroke.color,
                ),
                egui::StrokeKind::Outside,
            );
        }

        let text_rect = Rect::from_center_size(rect.center(), text_size);
        let painter = ui.painter().with_clip_rect(ui.clip_rect());
        self.paint_scene_in_rect(&painter, text_rect, &text_scene);
        apply_gamepad_scroll_if_focused(ui, &response);

        response
    }

    /// Shows a tooltip while the provided response is hovered.
    #[allow(dead_code)]
    pub(super) fn tooltip_for_response(
        &mut self,
        ui: &Ui,
        id_source: impl Hash,
        response: &Response,
        text: &str,
        options: &TooltipOptions,
    ) {
        if !response.hovered() {
            return;
        }

        let pointer = response.hover_pos().unwrap_or(response.rect.right_bottom());
        let scale = ui.ctx().pixels_per_point();
        let width_points_opt = Some(snap_width_to_bin(
            320.0_f32.min(ui.ctx().input(|i| i.content_rect().width() * 0.35)),
            scale,
        ));

        // ── Cache the tooltip texture; rasterize only when content changes ───
        let tooltip_tex_id = ui.make_persistent_id(&id_source).with("tooltip_text");
        let _tooltip_fingerprint = {
            let mut hasher = new_fingerprint_hasher();
            "textui_tooltip".hash(&mut hasher);
            text.hash(&mut hasher);
            options.text.font_size.to_bits().hash(&mut hasher);
            options.text.line_height.to_bits().hash(&mut hasher);
            options.text.color.hash(&mut hasher);
            scale.to_bits().hash(&mut hasher);
            width_points_opt
                .map(f32::to_bits)
                .unwrap_or(0)
                .hash(&mut hasher);
            self.hash_typography(&mut hasher);
            hasher.finish()
        };

        let scene = self.prepare_label_scene(
            ui.ctx(),
            tooltip_tex_id,
            text,
            &options.text,
            width_points_opt,
        );
        let raster_size = egui_vec_from_text(scene.size_points);

        let size = raster_size + options.padding * 2.0;
        let mut rect = Rect::from_min_size(pointer + options.offset, size);
        let min_y = ui.clip_rect().top();
        if rect.min.y < min_y {
            let delta = min_y - rect.min.y;
            rect = rect.translate(egui::vec2(0.0, delta));
        }

        // Keep the tooltip background and its rasterized text on the physical pixel grid.
        // Without this, tiny cursor-position changes can move the textured glyphs onto
        // fractional coordinates, which makes the same cached tooltip look fuzzy.
        rect = snap_rect_to_pixel_grid(rect, scale);

        let layer_id = egui::LayerId::new(
            egui::Order::Tooltip,
            ui.make_persistent_id(&id_source).with("tooltip_layer"),
        );
        let painter = ui.ctx().layer_painter(layer_id);
        painter.rect_filled(
            rect,
            CornerRadius::same(options.corner_radius),
            options.background,
        );
        if options.stroke.width > 0.0 {
            painter.rect_stroke(
                rect,
                CornerRadius::same(options.corner_radius),
                options.stroke,
                egui::StrokeKind::Inside,
            );
        }

        let text_rect = snap_rect_to_pixel_grid(
            Rect::from_min_size(rect.min + options.padding, raster_size),
            scale,
        );
        self.paint_scene_in_rect(&painter, text_rect, &scene);
    }
}
