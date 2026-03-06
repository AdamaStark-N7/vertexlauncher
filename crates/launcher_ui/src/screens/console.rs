use egui::Ui;
use textui::{LabelOptions, TextUi};

use crate::{console, ui::style};

pub fn render(ui: &mut Ui, text_ui: &mut TextUi) {
    let lines = console::snapshot();
    let viewport_size = egui::vec2(
        ui.available_width().max(1.0),
        ui.available_height().max(1.0),
    );
    ui.allocate_ui_with_layout(
        viewport_size,
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.add_space(style::SPACE_LG);
            let inner_width = (ui.available_width() - style::SPACE_LG).max(1.0);
            ui.allocate_ui_with_layout(
                egui::vec2(inner_width, ui.available_height().max(1.0)),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let viewport_height = ui.available_height().max(1.0);
                    ui.set_min_height(viewport_height);
                    egui::ScrollArea::both()
                        .id_salt("console_scroll_area")
                        .auto_shrink([false, false])
                        .max_height(viewport_height)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if lines.is_empty() {
                                let mut empty_style = style::muted(ui);
                                empty_style.wrap = false;
                                let _ = text_ui.label(
                                    ui,
                                    "console_empty",
                                    "No log entries yet.",
                                    &empty_style,
                                );
                                let _ = ui.allocate_exact_size(
                                    egui::vec2(1.0, (viewport_height - 24.0).max(1.0)),
                                    egui::Sense::hover(),
                                );
                                return;
                            }

                            for (index, line) in lines.iter().enumerate() {
                                let mut line_style = LabelOptions {
                                    font_size: 14.0,
                                    line_height: 18.0,
                                    color: color_for_line(ui, line),
                                    wrap: false,
                                    monospace: true,
                                    weight: 400,
                                    italic: false,
                                    padding: egui::Vec2::ZERO,
                                };
                                if line.contains("][ERROR][") {
                                    line_style.weight = 700;
                                }
                                let _ = text_ui.label_async(
                                    ui,
                                    ("console_line", index),
                                    line,
                                    &line_style,
                                );
                            }
                        });
                },
            );
            ui.add_space(style::SPACE_LG);
        },
    );
}

fn color_for_line(ui: &Ui, line: &str) -> egui::Color32 {
    if line.contains("][ERROR][") {
        ui.visuals().error_fg_color
    } else if line.contains("][WARN][") {
        ui.visuals().warn_fg_color
    } else if line.contains("][INFO][") {
        ui.visuals().hyperlink_color
    } else if line.contains("][DEBUG][") || line.contains("][TRACE][") {
        ui.visuals().weak_text_color()
    } else {
        ui.visuals().text_color()
    }
}
