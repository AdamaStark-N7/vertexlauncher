use std::hash::Hash;

use egui::{Rect, Response, Sense, Stroke, Ui, UiBuilder, Vec2, pos2};
use textui::TextUi;
use textui_egui::prelude::*;

use crate::ui::style;

const GAP: f32 = style::SPACE_MD;

/// Radio button with a textui label, so it follows the launcher font settings.
pub fn radio(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl Hash,
    label: &str,
    selected: bool,
) -> Response {
    choice_row(ui, text_ui, id_source, label, |ui, rect, response| {
        let visuals = ui.style().interact_selectable(response, selected);
        let center = rect.center();
        let radius = rect.width() * 0.5 - 1.0;
        ui.painter()
            .circle(center, radius, visuals.bg_fill, visuals.bg_stroke);
        if selected {
            ui.painter()
                .circle_filled(center, radius * 0.5, ui.visuals().selection.stroke.color);
        }
    })
}

/// Checkbox with a textui label, so it follows the launcher font settings.
/// Marks the response as changed when the value flips.
pub fn checkbox(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl Hash,
    label: &str,
    checked: &mut bool,
) -> Response {
    let mut response = choice_row(ui, text_ui, id_source, label, |ui, rect, response| {
        let visuals = ui.style().interact_selectable(response, *checked);
        ui.painter().rect(
            rect.shrink(1.0),
            2.0,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        if *checked {
            let stroke = Stroke::new(2.0, ui.visuals().selection.stroke.color);
            let w = rect.width();
            let start = rect.min + Vec2::new(w * 0.24, w * 0.52);
            let mid = rect.min + Vec2::new(w * 0.43, w * 0.72);
            let end = rect.min + Vec2::new(w * 0.76, w * 0.30);
            ui.painter().line_segment([start, mid], stroke);
            ui.painter().line_segment([mid, end], stroke);
        }
    });
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    response
}

fn choice_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl Hash,
    label: &str,
    paint_indicator: impl FnOnce(&Ui, Rect, &Response),
) -> Response {
    let label_style = style::body(ui);
    let label_style = textui_egui::LabelOptions {
        wrap: false,
        ..label_style
    };
    let label_size = text_ui.measure_text_size(ui, label, &label_style);
    let indicator = label_size.y.clamp(16.0, 28.0);
    let size = Vec2::new(indicator + GAP + label_size.x, label_size.y.max(indicator));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let indicator_rect = Rect::from_min_size(
        pos2(rect.min.x, rect.center().y - indicator * 0.5),
        Vec2::splat(indicator),
    );
    paint_indicator(ui, indicator_rect, &response);
    let label_rect = Rect::from_min_size(
        pos2(indicator_rect.max.x + GAP, rect.min.y),
        Vec2::new(label_size.x, rect.height()),
    );
    ui.scope_builder(UiBuilder::new().max_rect(label_rect), |ui| {
        let _ = text_ui.label(ui, (id_source, "choice_label"), label, &label_style);
    });
    response
}
