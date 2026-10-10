use super::*;

pub(super) fn label_impl(
    text_ui: &mut TextUi,
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
        display_text = textui::apply_smart_quotes(text);
        display_text.as_str()
    } else {
        text
    };

    let scale = ui.ctx().pixels_per_point();
    let width_points_opt =
        normalize_wrapped_width(options.wrap.then(|| ui.available_width().max(1.0)), scale);
    let cache_id = ui.make_persistent_id((&id_source, "textui_label_retained_scene"));
    let fingerprint = hash_label_scene_request(text, options, width_points_opt, scale);
    let text_options = options.to_text_label_options();
    let scene_opt = retained_gpu_scene(ui.ctx(), cache_id, fingerprint, || {
        if async_mode {
            text_ui.prepare_label_gpu_scene_async_at_scale(
                &id_source,
                text,
                &text_options,
                width_points_opt,
                scale,
            )
        } else {
            Some(text_ui.prepare_label_gpu_scene_at_scale(
                &id_source,
                text,
                &text_options,
                width_points_opt,
                scale,
            ))
        }
    });

    let Some(scene) = scene_opt else {
        let fallback_height = (options.line_height + options.padding.y * 2.0).max(20.0);
        let fallback_width = width_points_opt.unwrap_or_else(|| ui.available_width().max(1.0));
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(fallback_width, fallback_height), sense);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), ui.visuals().faint_bg_color);
        ui.ctx().request_repaint();
        return response;
    };

    let scene_size = egui::vec2(scene.size_points[0], scene.size_points[1]);
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
    paint_gpu_scene_in_rect(text_ui, &painter, image_rect, &scene, Color32::WHITE);
    response
}
