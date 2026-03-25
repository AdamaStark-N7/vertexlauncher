use egui::Ui;

use crate::{
    assets,
    ui::{
        components::icon_button,
        context_menu,
        instance_context_menu,
        style,
    },
};

use super::{ProfileShortcut, SidebarOutput};

/// Renders the instance shortcut list and emits click or context-menu actions.
pub fn render(
    ui: &mut Ui,
    profile_shortcuts: &[ProfileShortcut],
    output: &mut SidebarOutput,
    max_icon_width: f32,
) {
    if profile_shortcuts.is_empty() {
        return;
    }

    let row_height = max_icon_width.max(1.0);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = style::SPACE_SM;

        for profile in profile_shortcuts {
            let icon_id = format!("user_profile_{}", profile.id);
            let context_id = ui.make_persistent_id(("sidebar_instance_context", profile.id.as_str()));
            let response = ui
                .allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), row_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        icon_button::svg(
                            ui,
                            icon_id.as_str(),
                            assets::USER_SVG,
                            profile.name.as_str(),
                            false,
                            max_icon_width,
                        )
                    },
                )
                .inner;

            if response.clicked() {
                output.selected_profile_id = Some(profile.id.clone());
            }

            if response.secondary_clicked() {
                let anchor = response
                    .interact_pointer_pos()
                    .or_else(|| ui.ctx().pointer_latest_pos())
                    .unwrap_or(response.rect.left_bottom());
                instance_context_menu::request_for_instance(ui.ctx(), context_id, anchor, true);
            }

            if let Some(action) = instance_context_menu::take(ui.ctx(), context_id) {
                output.instance_context_actions.push((profile.id.clone(), action));
            }
        }
    });

    context_menu::show(ui.ctx());
}
