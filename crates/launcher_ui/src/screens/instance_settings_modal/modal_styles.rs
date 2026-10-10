use super::*;

/// Label and button styles shared by every section of the instance settings modal.
pub(super) struct ModalStyles {
    pub(super) muted: LabelOptions,
    pub(super) section: LabelOptions,
    pub(super) body: LabelOptions,
    pub(super) action_button: ButtonOptions,
    pub(super) refresh: ButtonOptions,
    pub(super) reinstall_button: ButtonOptions,
}

impl ModalStyles {
    pub(super) fn new(ui: &egui::Ui) -> Self {
        Self {
            muted: style::muted(ui),
            section: style::subtitle(ui),
            body: style::body(ui),
            action_button: ButtonOptions {
                min_size: egui::vec2(220.0, 34.0),
                text_color: ui.visuals().widgets.active.fg_stroke.color,
                fill: ui.visuals().selection.bg_fill,
                fill_hovered: style::hover_fill(ui, ui.visuals().selection.bg_fill),
                fill_active: ui.visuals().selection.bg_fill.gamma_multiply(0.9),
                fill_selected: ui.visuals().selection.bg_fill,
                stroke: ui.visuals().selection.stroke,
                ..ButtonOptions::default()
            },
            refresh: style::neutral_button_with_min_size(ui, egui::vec2(190.0, 30.0)),
            reinstall_button: style::neutral_button_with_min_size(ui, egui::vec2(220.0, 34.0)),
        }
    }
}
