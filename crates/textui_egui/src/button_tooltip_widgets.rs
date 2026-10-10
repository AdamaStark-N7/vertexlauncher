use super::*;

pub(super) fn selectable_button_impl(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    text: &str,
    selected: bool,
    options: &ButtonOptions,
) -> Response {
    let mut label_style = LabelOptions {
        font_size: options.font_size,
        line_height: options.line_height,
        color: options.text_color,
        wrap: false,
        monospace: false,
        weight: 400,
        italic: false,
        padding: egui::Vec2::ZERO,
        fundamentals: Default::default(),
        ..LabelOptions::default()
    };
    if let Some(typography) = button_options::button_typography(ui.ctx()) {
        // Keep each button's size relative to the default so explicitly sized buttons stay proportional.
        let relative = options.font_size / button_options::DEFAULT_BUTTON_FONT_SIZE;
        label_style.font_size = typography.font_size * relative;
        label_style.line_height = typography.line_height * relative;
        label_style.weight = typography.weight;
        label_style.fundamentals.font_family = typography.font_family;
    }
    let scale = ui.ctx().pixels_per_point();
    let cache_id = ui.make_persistent_id((&id_source, "textui_button_retained_scene"));
    let fingerprint = hash_label_scene_request(text, &label_style, None, scale);
    let scene = retained_gpu_scene(ui.ctx(), cache_id, fingerprint, || {
        Some(text_ui.prepare_label_gpu_scene_at_scale(
            (&id_source, "button_text"),
            text,
            &label_style.to_text_label_options(),
            None,
            scale,
        ))
    })
    .expect("synchronous textui button scene should always be available");
    let text_size = egui::vec2(scene.size_points[0], scene.size_points[1]);
    let desired_size = egui::vec2(
        (text_size.x + options.padding.x * 2.0).max(options.min_size.x),
        (text_size.y + options.padding.y * 2.0).max(options.min_size.y),
    );
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
    let state = interaction::InteractionState::of(ui.ctx(), &response, ui.is_enabled());
    let palette = interaction::FillPalette::new(
        options.fill,
        options.fill_hovered,
        options.fill_active,
        options.fill_selected,
        options.text_color,
    );
    let fill = palette.fill(state, selected);
    let stroke = interaction::focus_stroke(
        state,
        ui.visuals(),
        interaction::hover_stroke(
            state,
            options.stroke,
            ui.visuals().widgets.hovered.bg_stroke,
        ),
    );
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
    interaction::paint_focus_ring(
        ui.painter(),
        ui.visuals(),
        state,
        rect,
        options.corner_radius,
    );
    let text_rect = Rect::from_center_size(rect.center(), text_size);
    let painter = ui.painter().with_clip_rect(ui.clip_rect());
    paint_gpu_scene_in_rect(text_ui, &painter, text_rect, &scene, Color32::WHITE);
    response
}

/// Minimum gap kept between a tooltip and the edge of the window.
pub(super) const TOOLTIP_SCREEN_MARGIN: f32 = 6.0;

pub(super) fn tooltip_impl(
    text_ui: &mut TextUi,
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
    tooltip_at_impl(
        text_ui,
        ui.ctx(),
        ui.clip_rect().top(),
        egui::Id::new(&id_source),
        pointer,
        text,
        options,
    );
}

/// Draws a textui tooltip at `pointer` from a bare [`Context`], for callers that queue
/// tooltips and draw them once per frame.
pub(super) fn tooltip_at_impl(
    text_ui: &mut TextUi,
    ctx: &Context,
    min_y: f32,
    id_source: egui::Id,
    pointer: egui::Pos2,
    text: &str,
    options: &TooltipOptions,
) {
    let scale = ctx.pixels_per_point();
    let width_points_opt = normalize_wrapped_width(
        Some(320.0_f32.min(ctx.input(|i| i.content_rect().width() * 0.35))),
        scale,
    );
    let cache_id = id_source.with("textui_tooltip_retained_scene");
    let fingerprint = hash_label_scene_request(text, &options.text, width_points_opt, scale);
    let scene = retained_gpu_scene(ctx, cache_id, fingerprint, || {
        Some(text_ui.prepare_label_gpu_scene_at_scale(
            (&id_source, "tooltip"),
            text,
            &options.text.to_text_label_options(),
            width_points_opt,
            scale,
        ))
    })
    .expect("synchronous textui tooltip scene should always be available");
    let raster_size = egui::vec2(scene.size_points[0], scene.size_points[1]);
    let size = raster_size + options.padding * 2.0;
    let bounds = ctx.content_rect();
    let bounds = Rect::from_min_max(
        egui::pos2(bounds.min.x, bounds.min.y.max(min_y)),
        bounds.max,
    )
    .shrink(TOOLTIP_SCREEN_MARGIN);
    let mut rect = Rect::from_min_size(pointer + options.offset, size);
    // Flip to the other side of the pointer when the tooltip would run off the screen.
    if rect.max.x > bounds.max.x {
        rect = rect.translate(egui::vec2(-(size.x + options.offset.x * 2.0), 0.0));
    }
    if rect.max.y > bounds.max.y {
        rect = rect.translate(egui::vec2(0.0, -(size.y + options.offset.y * 2.0)));
    }
    // Then hard-clamp so the whole tooltip stays visible even if it does not fit either side.
    let shift = egui::vec2(
        (bounds.min.x - rect.min.x).max(0.0) + (bounds.max.x - rect.max.x).min(0.0),
        (bounds.min.y - rect.min.y).max(0.0) + (bounds.max.y - rect.max.y).min(0.0),
    );
    rect = rect.translate(shift);
    rect = snap_rect_to_pixel_grid(rect, scale);
    let layer_id = egui::LayerId::new(egui::Order::Tooltip, id_source.with("tooltip_layer"));
    let painter = ctx.layer_painter(layer_id);
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
    paint_gpu_scene_in_rect(text_ui, &painter, text_rect, &scene, Color32::WHITE);
}
