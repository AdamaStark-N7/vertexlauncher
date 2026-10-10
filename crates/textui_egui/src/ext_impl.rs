use super::*;

impl TextUiEguiExt for TextUi {
    fn paint_rich_line<H: Hash>(
        &mut self,
        ui: &Ui,
        painter: &Painter,
        id_source: H,
        pos: egui::Pos2,
        spans: &[RichTextSpan],
        options: &LabelOptions,
    ) -> Vec2 {
        let scale = ui.ctx().pixels_per_point();
        let cache_id = ui.make_persistent_id((&id_source, "textui_rich_line_scene"));
        let mut hasher = DefaultHasher::new();
        "rich_line_scene".hash(&mut hasher);
        for span in spans {
            span.text.hash(&mut hasher);
            span.style.color.hash(&mut hasher);
            span.style.monospace.hash(&mut hasher);
            span.style.italic.hash(&mut hasher);
            span.style.weight.hash(&mut hasher);
        }
        hash_label_options(&mut hasher, options);
        scale.to_bits().hash(&mut hasher);
        let fingerprint = hasher.finish();
        let text_options = options.to_text_label_options();
        let scene = retained_gpu_scene(ui.ctx(), cache_id, fingerprint, || {
            Some(self.prepare_rich_text_gpu_scene_at_scale(
                &id_source,
                spans,
                &text_options,
                None,
                scale,
            ))
        })
        .expect("synchronous textui rich line scene should always be available");
        let size = egui::vec2(scene.size_points[0], scene.size_points[1]);
        paint_gpu_scene_in_rect(
            self,
            painter,
            Rect::from_min_size(pos, size),
            &scene,
            Color32::WHITE,
        );
        size
    }

    fn tooltip_at<H: Hash>(
        &mut self,
        ctx: &Context,
        id_source: H,
        pointer: egui::Pos2,
        text: &str,
        options: &TooltipOptions,
    ) {
        tooltip_at_impl(
            self,
            ctx,
            ctx.content_rect().top(),
            egui::Id::new(id_source),
            pointer,
            text,
            options,
        );
    }

    fn label_async<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &LabelOptions,
    ) -> Response {
        label_impl(self, ui, id_source, text, options, Sense::hover(), true)
    }

    fn code_block_async<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        code: &str,
        options: &CodeBlockOptions,
    ) -> Response {
        code_block_impl(self, ui, id_source, code, options, true)
    }

    fn label<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &LabelOptions,
    ) -> Response {
        label_impl(self, ui, id_source, text, options, Sense::hover(), false)
    }

    fn clickable_label<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &LabelOptions,
    ) -> Response {
        label_impl(self, ui, id_source, text, options, Sense::click(), false)
    }

    fn measure_text_size_ctx(&mut self, ctx: &Context, text: &str, options: &LabelOptions) -> Vec2 {
        self.measure_text_size_at_scale(
            ctx.pixels_per_point(),
            text,
            &options.to_text_label_options(),
        )
        .into()
    }

    fn measure_text_size(&mut self, ui: &Ui, text: &str, options: &LabelOptions) -> Vec2 {
        self.measure_text_size_at_scale(
            ui.ctx().pixels_per_point(),
            text,
            &options.to_text_label_options(),
        )
        .into()
    }

    fn prepare_label_texture<H: Hash>(
        &mut self,
        ctx: &Context,
        id_source: H,
        text: &str,
        options: &LabelOptions,
        width_points_opt: Option<f32>,
    ) -> TextTextureHandle {
        let scale = ctx.pixels_per_point();
        let width_points_opt = normalize_wrapped_width(width_points_opt, scale);
        let fingerprint = hash_label_scene_request(text, options, width_points_opt, scale);
        let scene = retained_gpu_scene(
            ctx,
            Id::new((&id_source, "textui_prepare_label_texture_scene")),
            fingerprint,
            || {
                Some(self.prepare_label_gpu_scene_at_scale(
                    &id_source,
                    text,
                    &options.to_text_label_options(),
                    width_points_opt,
                    scale,
                ))
            },
        )
        .expect("synchronous label texture scene should always be available");
        TextTextureHandle {
            size_points: egui::vec2(scene.size_points[0], scene.size_points[1]),
            scene,
        }
    }

    fn prepare_rich_text_texture<H: Hash>(
        &mut self,
        ctx: &Context,
        id_source: H,
        spans: &[RichTextSpan],
        options: &LabelOptions,
        width_points_opt: Option<f32>,
    ) -> TextTextureHandle {
        let scale = ctx.pixels_per_point();
        let width_points_opt = normalize_wrapped_width(width_points_opt, scale);
        let fingerprint = hash_rich_text_scene_request(spans, options, width_points_opt, scale);
        let scene = retained_gpu_scene(
            ctx,
            Id::new((&id_source, "textui_prepare_rich_texture_scene")),
            fingerprint,
            || {
                Some(self.prepare_rich_text_gpu_scene_at_scale(
                    &id_source,
                    spans,
                    &options.to_text_label_options(),
                    width_points_opt,
                    scale,
                ))
            },
        )
        .expect("synchronous rich text texture scene should always be available");
        TextTextureHandle {
            size_points: egui::vec2(scene.size_points[0], scene.size_points[1]),
            scene,
        }
    }

    fn paint_label_on_path<H: Hash>(
        &mut self,
        painter: &Painter,
        id_source: H,
        text: &str,
        options: &LabelOptions,
        width_points_opt: Option<f32>,
        path: &TextPath,
        path_options: &TextPathOptions,
    ) -> Result<TextPathLayout, TextPathError> {
        let scene = self.prepare_label_path_gpu_scene_at_scale(
            &id_source,
            text,
            &options.to_text_label_options(),
            width_points_opt,
            painter.pixels_per_point(),
            path,
            path_options,
        )?;
        let layout = self.prepare_label_path_layout(
            text,
            &options.to_text_label_options(),
            width_points_opt,
            path,
            path_options,
        )?;
        paint_gpu_scene_absolute(self, painter, &scene, Color32::WHITE);
        Ok(layout)
    }

    fn paint_rich_text_on_path<H: Hash>(
        &mut self,
        painter: &Painter,
        id_source: H,
        spans: &[RichTextSpan],
        options: &LabelOptions,
        width_points_opt: Option<f32>,
        path: &TextPath,
        path_options: &TextPathOptions,
    ) -> Result<TextPathLayout, TextPathError> {
        let scene = self.prepare_rich_text_path_gpu_scene_at_scale(
            &id_source,
            spans,
            &options.to_text_label_options(),
            width_points_opt,
            painter.pixels_per_point(),
            path,
            path_options,
        )?;
        let layout = self.prepare_rich_text_path_layout(
            spans,
            &options.to_text_label_options(),
            width_points_opt,
            path,
            path_options,
        )?;
        paint_gpu_scene_absolute(self, painter, &scene, Color32::WHITE);
        Ok(layout)
    }

    fn paint_scene_in_rect(&mut self, painter: &Painter, rect: Rect, scene: &TextRenderScene) {
        let gpu_scene = retained_gpu_scene_for_render_scene(self, painter.ctx(), scene);
        paint_gpu_scene_in_rect(self, painter, rect, &gpu_scene, Color32::WHITE)
    }

    fn paint_scene_in_rect_tinted(
        &mut self,
        painter: &Painter,
        rect: Rect,
        scene: &TextRenderScene,
        tint: egui::Color32,
    ) {
        let gpu_scene = retained_gpu_scene_for_render_scene(self, painter.ctx(), scene);
        paint_gpu_scene_in_rect(self, painter, rect, &gpu_scene, tint)
    }

    fn button<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        options: &ButtonOptions,
    ) -> Response {
        selectable_button_impl(self, ui, id_source, text, false, options)
    }

    fn selectable_button<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &str,
        selected: bool,
        options: &ButtonOptions,
    ) -> Response {
        selectable_button_impl(self, ui, id_source, text, selected, options)
    }

    fn tooltip_for_response<H: Hash>(
        &mut self,
        ui: &Ui,
        id_source: H,
        response: &Response,
        text: &str,
        options: &TooltipOptions,
    ) {
        tooltip_impl(self, ui, id_source, response, text, options)
    }

    fn code_block<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        code: &str,
        options: &CodeBlockOptions,
    ) -> Response {
        code_block_impl(self, ui, id_source, code, options, false)
    }

    fn markdown<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        markdown: &str,
        options: &MarkdownOptions,
    ) {
        markdown_impl(self, ui, id_source, markdown, options)
    }

    fn singleline_input<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &mut String,
        options: &InputOptions,
    ) -> Response {
        TextUi::egui_singleline_input(self, ui, id_source, text, &options.to_core_input_options())
    }

    fn multiline_input<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        text: &mut String,
        options: &InputOptions,
    ) -> Response {
        TextUi::egui_multiline_input(self, ui, id_source, text, &options.to_core_input_options())
    }

    fn multiline_rich_viewer<H: Hash>(
        &mut self,
        ui: &mut Ui,
        id_source: H,
        spans: &[RichTextSpan],
        options: &InputOptions,
        stick_to_bottom: bool,
        wrap: bool,
    ) -> Response {
        TextUi::egui_multiline_rich_viewer(
            self,
            ui,
            id_source,
            spans,
            &options.to_core_input_options(),
            stick_to_bottom,
            wrap,
        )
    }
}
