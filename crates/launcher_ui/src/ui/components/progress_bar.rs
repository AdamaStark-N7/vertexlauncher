use std::hash::Hash;

use egui::{Direction, Layout, Response, Ui, UiBuilder, Vec2};
use textui::TextUi;
use textui_egui::prelude::*;

use crate::ui::style;

/// What a progress bar shows on top of its fill.
pub enum ProgressLabel<'a> {
    None,
    Percentage,
    Text(&'a str),
}

/// Progress bar whose label is drawn with textui, so it follows the launcher font settings
/// (egui's own `ProgressBar::text` / `show_percentage` use egui's built-in fonts).
///
/// `size` is `(width, height)`; `None` for either axis uses the available width and a
/// height that fits the label.
pub fn progress_bar(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl Hash,
    fraction: f32,
    animate: bool,
    size: (Option<f32>, Option<f32>),
    label: ProgressLabel<'_>,
) -> Response {
    let fraction = fraction.clamp(0.0, 1.0);
    let label_style = style::caption(ui);
    let label_style = textui_egui::LabelOptions {
        color: ui.visuals().text_color(),
        ..label_style
    };
    let height = size.1.unwrap_or_else(|| {
        ui.spacing()
            .interact_size
            .y
            .max(label_style.line_height + 4.0)
    });
    let width = size.0.unwrap_or_else(|| ui.available_width());
    let bar = egui::ProgressBar::new(fraction).animate(animate);
    let response = ui.add_sized(Vec2::new(width, height), bar);

    let text = match label {
        ProgressLabel::None => return response,
        ProgressLabel::Percentage => format!("{:.0}%", fraction * 100.0),
        ProgressLabel::Text(text) => text.to_owned(),
    };
    ui.scope_builder(UiBuilder::new().max_rect(response.rect), |ui| {
        ui.with_layout(
            Layout::centered_and_justified(Direction::LeftToRight),
            |ui| {
                let _ = text_ui.label(ui, (id_source, "progress_bar_label"), &text, &label_style);
            },
        );
    });
    response
}
