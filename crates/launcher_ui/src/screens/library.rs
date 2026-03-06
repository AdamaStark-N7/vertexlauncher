use egui::Ui;
use textui::TextUi;

use crate::ui::style;

pub fn render(ui: &mut Ui, text_ui: &mut TextUi, selected_instance_id: Option<&str>) {
    let heading = style::page_heading(ui);
    let body = style::body(ui);

    let _ = text_ui.label(ui, "library_heading", "Library", &heading);
    ui.add_space(style::SPACE_MD);
    let _ = text_ui.label(
        ui,
        "library_desc",
        "Manage installed content and versions here.",
        &body,
    );

    if let Some(instance_id) = selected_instance_id {
        ui.add_space(style::SPACE_MD);
        let _ = text_ui.label(
            ui,
            "library_instance_scope",
            &format!("Selected instance: {instance_id}"),
            &body,
        );
    }
}
