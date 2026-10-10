use super::*;

pub(super) fn dropdown_popup_height_limit(
    max_popup_height: f32,
    row_height: f32,
    row_count: usize,
) -> f32 {
    if row_count == 0 {
        return max_popup_height;
    }

    let content_height = row_height * row_count as f32;
    let fit_allowance = row_height * 0.35;
    (content_height + fit_allowance).min(max_popup_height)
}

pub(super) fn response_will_open(ui: &Ui) -> bool {
    ui.input(|input| {
        input.pointer.primary_clicked()
            || input.key_pressed(egui::Key::Enter)
            || input.key_pressed(egui::Key::Space)
    })
}

pub(super) fn dropdown_text_budget(metrics: ControlMetrics) -> f32 {
    let left_padding = 8.0;
    let right_padding = 8.0;
    let icon_gap = 6.0;
    (metrics.dropdown_width - metrics.icon_size - left_padding - right_padding - icon_gap).max(0.0)
}

/// Inputs for painting the closed dropdown button shared by both dropdown kinds.
pub(super) struct DropdownButtonPaint<'a> {
    pub(super) response: &'a Response,
    pub(super) button_rect: egui::Rect,
    pub(super) is_open: bool,
    pub(super) metrics: ControlMetrics,
    pub(super) chevron_icon_id: &'static str,
    pub(super) selected_text_id: &'static str,
}

/// Paints the dropdown button frame, chevron and selected text.
pub(super) fn paint_dropdown_button(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    paint: DropdownButtonPaint<'_>,
    label_style: &mut LabelOptions,
    selected_text: &str,
) {
    let DropdownButtonPaint {
        response,
        button_rect,
        is_open,
        metrics,
        chevron_icon_id,
        selected_text_id,
    } = paint;

    let interacted = if is_open {
        &ui.visuals().widgets.open
    } else if response.has_focus() {
        &ui.visuals().widgets.active
    } else {
        &ui.visuals().widgets.inactive
    };
    let text_color = interacted.text_color();

    let dropdown_state = interaction::InteractionState::of(ui.ctx(), response, true);
    let (dropdown_fill, dropdown_stroke) = if is_open || response.has_focus() {
        (interacted.bg_fill, interacted.bg_stroke)
    } else {
        (
            interaction::FillPalette::from_visuals(ui.visuals()).fill(dropdown_state, false),
            interaction::hover_stroke(
                dropdown_state,
                ui.visuals().widgets.inactive.bg_stroke,
                ui.visuals().widgets.hovered.bg_stroke,
            ),
        )
    };
    ui.painter().rect(
        button_rect,
        6.0,
        dropdown_fill,
        dropdown_stroke,
        egui::StrokeKind::Inside,
    );

    let icon_bytes = if is_open {
        assets::CHEVRON_UP_SVG
    } else {
        assets::CHEVRON_DOWN_SVG
    };
    let icon = themed_svg_image(chevron_icon_id, icon_bytes, metrics.icon_size, text_color)
        .fit_to_exact_size(egui::vec2(metrics.icon_size, metrics.icon_size));
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(
            button_rect.right() - metrics.icon_size - 8.0,
            button_rect.center().y - metrics.icon_size * 0.5,
        ),
        egui::vec2(metrics.icon_size, metrics.icon_size),
    );
    ui.put(icon_rect, icon);

    label_style.color = text_color;
    let parent_clip_rect = ui.clip_rect();
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(button_rect.left() + 8.0, button_rect.top()),
        egui::pos2(icon_rect.left() - 6.0, button_rect.bottom()),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
        ui.set_clip_rect(text_rect.intersect(parent_clip_rect));
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let _ = text_ui.label(ui, selected_text_id, selected_text, label_style);
        });
    });
}
