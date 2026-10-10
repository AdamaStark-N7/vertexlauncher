use super::*;
use crate::ui::color::lerp_color32_oklab;
use crate::ui::toggle_anim::ToggleAnimState;
use textui_egui::interaction;

pub(super) fn render_content_thumbnail(
    ui: &mut Ui,
    id_source: impl Hash,
    icon_url: Option<&str>,
    size: f32,
) {
    let size = egui::vec2(size, size);
    if let Some(icon_url) = icon_url {
        remote_tiled_image::show(
            ui,
            icon_url,
            size,
            (id_source, "remote-icon"),
            assets::LIBRARY_SVG,
        );
    } else {
        let mut hasher = DefaultHasher::new();
        id_source.hash(&mut hasher);
        ui.add(
            egui::Image::from_bytes(
                format!(
                    "bytes://instance/default-content-icon/{}.svg",
                    hasher.finish()
                ),
                assets::LIBRARY_SVG,
            )
            .fit_to_exact_size(size),
        );
    }
}

/// Tints `svg` with `color`, memoized in egui's temp data so the bytes are built once per
/// `(icon, color)` instead of every frame. `icon` must uniquely identify `svg`.
pub(super) fn cached_tinted_svg(
    ctx: &egui::Context,
    icon: &'static str,
    svg: &'static [u8],
    color: egui::Color32,
) -> Arc<[u8]> {
    let id = egui::Id::new(("instance_content_tinted_svg", icon, color));
    if let Some(bytes) = ctx.data(|data| data.get_temp::<Arc<[u8]>>(id)) {
        return bytes;
    }
    let bytes: Arc<[u8]> = crate::ui::svg_tint::tint_svg(svg, color).into();
    ctx.data_mut(|data| data.insert_temp(id, Arc::clone(&bytes)));
    bytes
}

/// Paints the shared rounded fill, hover/focus stroke and focus ring used by the icon-style buttons.
fn paint_rounded_button_frame(
    ui: &Ui,
    rect: egui::Rect,
    state: interaction::InteractionState,
    palette: interaction::FillPalette,
    base_stroke: egui::Stroke,
) {
    let visuals = ui.visuals();
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(8),
        palette.fill(state, false),
    );
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(8),
        interaction::focus_stroke(
            state,
            visuals,
            interaction::hover_stroke(state, base_stroke, visuals.widgets.hovered.bg_stroke),
        ),
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), visuals, state, rect, 8);
}

pub(super) fn render_installed_content_action_button(
    ui: &mut Ui,
    icon_id: &str,
    themed_svg: &Arc<[u8]>,
    icon_color: egui::Color32,
    tooltip: &str,
    width: f32,
    height: f32,
) -> bool {
    let uri = format!(
        "bytes://instance-installed-content-action/{icon_id}-{:02x}{:02x}{:02x}.svg",
        icon_color.r(),
        icon_color.g(),
        icon_color.b()
    );
    let button_size = egui::vec2(width, height);
    let icon_size = (height - 10.0).max(12.0);
    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
    let visuals = ui.visuals();
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    paint_rounded_button_frame(
        ui,
        rect,
        state,
        interaction::FillPalette::new(
            visuals.extreme_bg_color,
            visuals.widgets.hovered.bg_fill,
            visuals.widgets.active.bg_fill,
            visuals.selection.bg_fill,
            visuals.text_color(),
        ),
        visuals.widgets.inactive.bg_stroke,
    );

    let image = egui::Image::from_bytes(uri, Arc::clone(themed_svg))
        .fit_to_exact_size(egui::vec2(icon_size, icon_size));
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
    let _ = ui.put(icon_rect, image);

    crate::ui::style::hover_tip(ui, response, tooltip).clicked()
}

// ── Animated toggle widget ────────────────────────────────────────────────────

pub(super) fn render_mod_enable_toggle(
    ui: &mut Ui,
    id: &str,
    enabled: bool,
    width: f32,
    height: f32,
) -> bool {
    let size = egui::vec2(width, height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    // Load, advance, and persist animation state.
    let anim_id = ui.make_persistent_id(("mod_toggle_anim", id));
    let target = if enabled { 1.0f32 } else { 0.0f32 };
    // Cap dt so a long frame doesn't teleport the knob.
    let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);

    let mut anim: ToggleAnimState = ui
        .ctx()
        .data_mut(|d| d.get_temp::<ToggleAnimState>(anim_id))
        .unwrap_or_else(|| ToggleAnimState::settled(target));

    anim.redirect(target);
    anim.advance(dt);
    let pos = anim.visual_pos(); // 0.0 = off, 1.0 = on

    if !anim.is_done() {
        ui.ctx().request_repaint();
    }
    ui.ctx().data_mut(|d| d.insert_temp(anim_id, anim));

    // Geometry
    let track_h = (height * 0.68).max(14.0);
    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(width - 6.0, track_h));
    let cr = egui::CornerRadius::same((track_h * 0.5) as u8);
    let visuals = ui.visuals();
    let hover = interaction::InteractionState::of(ui.ctx(), &response, true);

    // Track fill and stroke — interpolated in OKLab, then lifted toward the text color on hover.
    let fill = lerp_color32_oklab(
        visuals.widgets.inactive.bg_fill,
        visuals.selection.bg_fill,
        pos,
    )
    .lerp_to_gamma(visuals.text_color(), 0.14 * hover.hover);
    let stroke_color = lerp_color32_oklab(
        visuals.widgets.inactive.bg_stroke.color,
        visuals.selection.stroke.color,
        pos,
    )
    .lerp_to_gamma(visuals.widgets.hovered.bg_stroke.color, hover.hover);
    let stroke_w = visuals.widgets.inactive.bg_stroke.width
        + (visuals.selection.stroke.width - visuals.widgets.inactive.bg_stroke.width) * pos;

    ui.painter().rect_filled(track_rect, cr, fill);
    ui.painter().rect_stroke(
        track_rect,
        cr,
        egui::Stroke::new(stroke_w, stroke_color),
        egui::StrokeKind::Inside,
    );

    // Knob position — driven by the same eased pos.
    let knob_r = (track_h * 0.33).max(4.0);
    let margin = knob_r + 2.5;
    let travel = track_rect.width() - margin * 2.0;
    let knob_x = track_rect.left() + margin + travel * pos;
    let knob_color = lerp_color32_oklab(
        visuals.widgets.noninteractive.fg_stroke.color,
        visuals.widgets.active.fg_stroke.color,
        pos,
    );
    ui.painter().circle_filled(
        egui::pos2(knob_x, track_rect.center().y),
        knob_r,
        knob_color,
    );

    let tooltip = if enabled { "Disable mod" } else { "Enable mod" };
    crate::ui::style::hover_tip(ui, response, tooltip).clicked()
}

pub(super) fn render_bulk_update_button(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    label: &str,
    tooltip: &str,
    enabled: bool,
) -> bool {
    ui.push_id(id_source, |ui| {
        let text_color = if enabled {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        };
        let fill = if enabled {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };
        let sense = if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let button_size = egui::vec2(ui.available_width().max(220.0), 34.0);
        let (rect, response) = ui.allocate_exact_size(button_size, sense);
        let state = interaction::InteractionState::of(ui.ctx(), &response, enabled);
        paint_rounded_button_frame(
            ui,
            rect,
            state,
            interaction::FillPalette::new(
                fill,
                fill.lerp_to_gamma(ui.visuals().text_color(), 0.14),
                fill.gamma_multiply(0.9),
                fill,
                ui.visuals().text_color(),
            ),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        let icon_size = 16.0;
        let icon_uri = format!(
            "bytes://instance-content-bulk-update/{:02x}{:02x}{:02x}.svg",
            text_color.r(),
            text_color.g(),
            text_color.b()
        );
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + 12.0, rect.center().y - (icon_size * 0.5)),
            egui::vec2(icon_size, icon_size),
        );
        let refresh_icon_svg =
            cached_tinted_svg(ui.ctx(), "refresh", assets::REFRESH_SVG, text_color);
        let _ = ui.put(
            icon_rect,
            egui::Image::from_bytes(icon_uri, refresh_icon_svg)
                .fit_to_exact_size(egui::vec2(icon_size, icon_size)),
        );
        let label_rect = egui::Rect::from_min_max(
            egui::pos2(icon_rect.max.x + 8.0, rect.top()),
            egui::pos2(rect.right() - 10.0, rect.bottom()),
        );
        let label_style = LabelOptions {
            color: text_color,
            ..style::stat_label(ui)
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(label_rect), |ui| {
            ui.set_clip_rect(label_rect.intersect(ui.clip_rect()));
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let _ = text_ui.label(
                    ui,
                    ("instance_content_bulk_update_label", label),
                    label,
                    &label_style,
                );
            });
        });

        crate::ui::style::hover_tip(
            ui,
            response,
            if enabled {
                tooltip
            } else {
                "A content operation is already in progress."
            },
        )
        .clicked()
    })
    .inner
}

pub(super) fn render_installed_content_badge(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    label: &str,
    fill: egui::Color32,
    text_color: egui::Color32,
) {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(
            INSTALLED_CONTENT_BADGE_PADDING_X as i8,
            INSTALLED_CONTENT_BADGE_PADDING_Y as i8,
        ))
        .show(ui, |ui| {
            let _ = text_ui.label(
                ui,
                id_source,
                label,
                &LabelOptions {
                    font_size: INSTALLED_CONTENT_BADGE_FONT_SIZE,
                    line_height: INSTALLED_CONTENT_BADGE_LINE_HEIGHT,
                    color: text_color,
                    wrap: false,
                    ..crate::ui::style::body(ui)
                },
            );
        });
}

pub(super) fn render_installed_content_update_badge(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    label: &str,
    enabled: bool,
) -> bool {
    ui.push_id(id_source, |ui| {
        let text_color = if enabled {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().weak_text_color()
        };
        let fill = if enabled {
            ui.visuals().warn_fg_color.gamma_multiply(0.16)
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };
        let stroke_color = if enabled {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                text_ui.button(
                    ui,
                    ("installed_content_update_badge", label),
                    label,
                    &ButtonOptions {
                        font_size: INSTALLED_CONTENT_BADGE_FONT_SIZE,
                        line_height: INSTALLED_CONTENT_BADGE_LINE_HEIGHT,
                        padding: egui::vec2(
                            INSTALLED_CONTENT_BADGE_PADDING_X,
                            INSTALLED_CONTENT_BADGE_PADDING_Y,
                        ),
                        min_size: egui::vec2(0.0, 0.0),
                        corner_radius: 6,
                        text_color,
                        fill,
                        fill_hovered: style::hover_fill(ui, fill),
                        fill_active: fill.gamma_multiply(0.92),
                        fill_selected: fill,
                        stroke: egui::Stroke::new(1.0, stroke_color),
                        ..ButtonOptions::default()
                    },
                )
            })
            .inner;
        crate::ui::style::hover_tip(
            ui,
            response,
            if enabled {
                "Update to this version"
            } else {
                "A content operation is already in progress"
            },
        )
        .clicked()
    })
    .inner
}

pub(super) fn render_installed_content_warning(
    ui: &mut Ui,
    warning_message: &str,
    id_source: impl std::hash::Hash + Copy,
    warning_icon_svg: &Arc<[u8]>,
) {
    let mut hasher = DefaultHasher::new();
    id_source.hash(&mut hasher);
    let uri = format!(
        "bytes://instance-installed-content-warning/{}.svg",
        hasher.finish()
    );
    let response = ui.add(
        egui::Image::from_bytes(uri, Arc::clone(warning_icon_svg))
            .fit_to_exact_size(egui::vec2(16.0, 16.0)),
    );
    crate::ui::style::hover_tip(ui, response, warning_message);
}
