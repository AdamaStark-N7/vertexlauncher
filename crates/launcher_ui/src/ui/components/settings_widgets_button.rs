use super::*;

pub fn full_width_button(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    text: &str,
    width: f32,
    primary: bool,
) -> Response {
    let mut style = ButtonOptions {
        min_size: egui::vec2(width.max(1.0), 30.0),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().widgets.active.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    };
    if primary {
        style.stroke = ui.visuals().selection.stroke;
        style.fill = ui.visuals().selection.bg_fill;
        style.fill_hovered = crate::ui::style::hover_fill(ui, ui.visuals().selection.bg_fill);
        style.fill_active = ui.visuals().selection.bg_fill.gamma_multiply(0.9);
        style.fill_selected = ui.visuals().selection.bg_fill;
        style.text_color = ui.visuals().widgets.active.fg_stroke.color;
    }
    text_ui.button(ui, id_source, text, &style)
}

pub fn selectable_chip_button(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    text: &str,
    selected: bool,
    width: f32,
    enabled: bool,
) -> Response {
    let mut style = ButtonOptions {
        min_size: egui::vec2(width.max(1.0), 30.0),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().widgets.active.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    };

    if selected {
        style.fill = ui.visuals().widgets.active.bg_fill;
        style.fill_hovered = crate::ui::style::hover_fill(ui, ui.visuals().widgets.active.bg_fill);
        style.fill_active = ui.visuals().widgets.active.bg_fill.gamma_multiply(0.92);
        style.text_color = ui.visuals().widgets.active.fg_stroke.color;
    }

    if !enabled {
        style.text_color = ui.visuals().weak_text_color();
        style.fill = ui.visuals().widgets.noninteractive.bg_fill;
        style.fill_hovered = ui.visuals().widgets.noninteractive.bg_fill;
        style.fill_active = ui.visuals().widgets.noninteractive.bg_fill;
        style.fill_selected = ui.visuals().widgets.noninteractive.bg_fill;
    }

    ui.add_enabled_ui(enabled, |ui| {
        text_ui.selectable_button(ui, id_source, text, selected, &style)
    })
    .inner
}
