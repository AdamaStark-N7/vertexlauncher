use super::*;

pub(super) fn code_block_impl(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    code: &str,
    options: &CodeBlockOptions,
    async_mode: bool,
) -> Response {
    let scale = ui.ctx().pixels_per_point();
    let width_points_opt = normalize_wrapped_width(
        if options.wrap {
            Some((ui.available_width() - options.padding.x * 2.0).max(1.0))
        } else {
            None
        },
        scale,
    );
    let cache_id = ui.make_persistent_id((&id_source, "textui_code_block_retained_scene"));
    let fingerprint = hash_code_block_scene_request(code, options, width_points_opt, scale);
    let label_options = LabelOptions {
        font_size: options.font_size,
        line_height: options.line_height,
        color: options.text_color,
        wrap: options.wrap,
        monospace: true,
        weight: 400,
        italic: false,
        padding: egui::Vec2::ZERO,
        fundamentals: options.fundamentals.clone(),
        ..LabelOptions::default()
    };
    let scene_opt = retained_gpu_scene(ui.ctx(), cache_id, fingerprint, || {
        let spans = text_ui.highlight_code_spans(
            code,
            options.language.as_deref(),
            options.text_color.into(),
        );
        let text_options = label_options.to_text_label_options();
        if async_mode {
            text_ui.prepare_rich_text_gpu_scene_async_at_scale(
                &id_source,
                &spans,
                &text_options,
                width_points_opt,
                scale,
            )
        } else {
            Some(text_ui.prepare_rich_text_gpu_scene_at_scale(
                &id_source,
                &spans,
                &text_options,
                width_points_opt,
                scale,
            ))
        }
    });

    let Some(scene) = scene_opt else {
        let fallback_height = (options.line_height * 2.0 + options.padding.y * 2.0).max(32.0);
        let desired_size = egui::vec2(
            width_points_opt.unwrap_or_else(|| ui.available_width().max(1.0)),
            fallback_height,
        );
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(options.corner_radius),
            options.background_color,
        );
        ui.ctx().request_repaint();
        return response;
    };

    let scene_size = egui::vec2(scene.size_points[0], scene.size_points[1]);
    let desired_size = scene_size + options.padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(options.corner_radius),
        options.background_color,
    );
    if options.stroke.width > 0.0 {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(options.corner_radius),
            options.stroke,
            egui::StrokeKind::Inside,
        );
    }
    let image_rect = Rect::from_min_size(rect.min + options.padding, scene_size);
    let painter = ui.painter().with_clip_rect(ui.clip_rect());
    paint_gpu_scene_in_rect(text_ui, &painter, image_rect, &scene, Color32::WHITE);
    response
}

pub(super) fn markdown_impl(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    markdown: &str,
    options: &MarkdownOptions,
) {
    let mut hasher = DefaultHasher::new();
    "markdown_blocks".hash(&mut hasher);
    markdown.hash(&mut hasher);
    options.heading_scale.to_bits().hash(&mut hasher);
    options.paragraph_spacing.to_bits().hash(&mut hasher);
    options.body.font_size.to_bits().hash(&mut hasher);
    options.body.line_height.to_bits().hash(&mut hasher);
    options.body.color.hash(&mut hasher);
    options.code.font_size.to_bits().hash(&mut hasher);
    let fingerprint = hasher.finish();
    let blocks = text_ui.parse_markdown_blocks_cached(id_source, markdown, fingerprint);

    ui.push_id("textui_markdown", |ui| {
        for (index, block) in blocks.iter().enumerate() {
            match block {
                TextMarkdownBlock::Heading { level, text } => {
                    let factor = match level {
                        TextMarkdownHeadingLevel::H1 => options.heading_scale + 0.26,
                        TextMarkdownHeadingLevel::H2 => options.heading_scale + 0.12,
                        TextMarkdownHeadingLevel::H3 => options.heading_scale,
                        TextMarkdownHeadingLevel::H4 => options.heading_scale - 0.08,
                        TextMarkdownHeadingLevel::H5 => options.heading_scale - 0.12,
                        TextMarkdownHeadingLevel::H6 => options.heading_scale - 0.16,
                    }
                    .max(1.0);
                    let heading_style = LabelOptions {
                        font_size: options.body.font_size * factor,
                        line_height: options.body.line_height * factor,
                        color: options.body.color,
                        wrap: true,
                        monospace: false,
                        weight: 700,
                        italic: false,
                        padding: egui::Vec2::ZERO,
                        fundamentals: options.body.fundamentals.clone(),
                        ..LabelOptions::default()
                    };
                    let _ = label_impl(
                        text_ui,
                        ui,
                        ("md_h", index),
                        text,
                        &heading_style,
                        Sense::hover(),
                        false,
                    );
                }
                TextMarkdownBlock::Paragraph(text) => {
                    let _ = label_impl(
                        text_ui,
                        ui,
                        ("md_p", index),
                        text,
                        &options.body,
                        Sense::hover(),
                        false,
                    );
                }
                TextMarkdownBlock::Code { language, text } => {
                    let mut code_options = options.code.clone();
                    code_options.language = language.clone();
                    let _ = code_block_impl(
                        text_ui,
                        ui,
                        ("md_code", index),
                        text,
                        &code_options,
                        false,
                    );
                }
            }

            if index + 1 < blocks.len() {
                ui.add_space(options.paragraph_spacing);
            }
        }
    });
}
