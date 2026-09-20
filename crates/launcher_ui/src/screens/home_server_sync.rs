//! Per-server sync options: which instances a multiplayer server is kept in.

use std::collections::BTreeSet;

use instances::ServerSyncScope;

use super::*;
use crate::sync_runner::{self, SyncRunConfig};
use crate::ui::components::choice_controls;
use crate::ui::modal;

const REQUEST_KEY: &str = "home_server_sync_modal";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeChoice {
    FollowDefault,
    ThisInstanceOnly,
    AllInstances,
    Selected,
}

impl ScopeChoice {
    fn from_scope(scope: &ServerSyncScope) -> Self {
        match scope {
            ServerSyncScope::FollowDefault => Self::FollowDefault,
            ServerSyncScope::ThisInstanceOnly => Self::ThisInstanceOnly,
            ServerSyncScope::AllInstances => Self::AllInstances,
            ServerSyncScope::Selected(_) => Self::Selected,
        }
    }
}

#[derive(Clone)]
struct ServerSyncModalState {
    address: String,
    name: String,
    choice: ScopeChoice,
    selected: BTreeSet<String>,
}

/// Opens the sync options dialog for `server`, starting from its saved scope.
pub(super) fn request_server_sync_modal(
    ctx: &egui::Context,
    server: &ServerEntry,
    instances: &InstanceStore,
) {
    let scope = instances.server_sync_scope(server.address.as_str());
    let selected = match &scope {
        ServerSyncScope::Selected(ids) => ids.clone(),
        // Start a new selection with the instance the server was found in.
        _ => BTreeSet::from([server.instance_id.clone()]),
    };
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(REQUEST_KEY),
            ServerSyncModalState {
                address: server.address.clone(),
                name: server.server_name.clone(),
                choice: ScopeChoice::from_scope(&scope),
                selected,
            },
        );
    });
}

pub(super) fn render_server_sync_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let key = egui::Id::new(REQUEST_KEY);
    let Some(mut state) = ctx.data(|data| data.get_temp::<ServerSyncModalState>(key)) else {
        return;
    };

    let mut save = false;
    let mut close = false;
    let response = modal::show_window(
        ctx,
        "Server sync options",
        modal::ModalOptions::new(
            egui::Id::new(("home_server_sync_modal_window", state.address.as_str())),
            modal::ModalLayout::centered(
                modal::AxisSizing::new(0.5, 380.0, 620.0),
                modal::AxisSizing::new(0.7, 320.0, 720.0),
            ),
        )
        .with_layer(modal::ModalLayer::Base)
        .with_dismiss_behavior(modal::DismissBehavior::EscapeAndScrim),
        |ui| {
            let muted = style::muted(ui);
            let _ = text_ui.label(
                ui,
                "server_sync_title",
                "Server sync options",
                &style::modal_title(ui),
            );
            let _ = text_ui.label(
                ui,
                "server_sync_target",
                format!("{} ({})", state.name, state.address).as_str(),
                &style::body_strong(ui),
            );
            let _ = text_ui.label(
                ui,
                "server_sync_help",
                "Choose which instances get this server in their server list. Entries that already exist are never changed or removed.",
                &muted,
            );
            if !config.sync_servers_enabled() {
                let _ = text_ui.label(
                    ui,
                    "server_sync_disabled",
                    "Server sync is turned off. Enable it in Settings → Instance Sync for these options to take effect.",
                    &style::warning_text(ui),
                );
            }
            ui.add_space(style::SPACE_LG);

            let default_label = if config.sync_servers_to_all_instances_by_default() {
                "Follow launcher default (currently: every instance)"
            } else {
                "Follow launcher default (currently: only where it already exists)"
            };
            for (choice, label) in [
                (ScopeChoice::FollowDefault, default_label),
                (
                    ScopeChoice::ThisInstanceOnly,
                    "Only where it already exists",
                ),
                (ScopeChoice::AllInstances, "Every instance"),
                (ScopeChoice::Selected, "Selected instances"),
            ] {
                if choice_controls::radio(
                    ui,
                    text_ui,
                    ("server_sync_choice", label),
                    label,
                    state.choice == choice,
                )
                .clicked()
                {
                    state.choice = choice;
                }
                ui.add_space(style::SPACE_XS);
            }

            if state.choice == ScopeChoice::Selected {
                ui.add_space(style::SPACE_MD);
                egui::ScrollArea::vertical()
                    .scroll_source(modal_host::drag_scroll_source())
                    .id_salt("server_sync_instances_scroll")
                    .max_height(260.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for instance in &instances.instances {
                            let mut checked = state.selected.contains(&instance.id);
                            let label =
                                format!("{} (Minecraft {})", instance.name, instance.game_version);
                            if choice_controls::checkbox(
                                ui,
                                text_ui,
                                ("server_sync_instance", instance.id.as_str()),
                                label.as_str(),
                                &mut checked,
                            )
                            .changed()
                            {
                                if checked {
                                    state.selected.insert(instance.id.clone());
                                } else {
                                    state.selected.remove(&instance.id);
                                }
                            }
                            ui.add_space(style::SPACE_XS);
                        }
                    });
            }

            ui.add_space(style::SPACE_XL);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if text_ui
                    .button(
                        ui,
                        "server_sync_save",
                        "Save",
                        &ui_foundation::primary_button(
                            ui,
                            egui::vec2(110.0, style::CONTROL_HEIGHT),
                        ),
                    )
                    .clicked()
                {
                    save = true;
                }
                if text_ui
                    .button(
                        ui,
                        "server_sync_cancel",
                        "Cancel",
                        &ui_foundation::secondary_button(
                            ui,
                            egui::vec2(110.0, style::CONTROL_HEIGHT),
                        ),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        },
    );

    if save {
        let scope = match state.choice {
            ScopeChoice::FollowDefault => ServerSyncScope::FollowDefault,
            ScopeChoice::ThisInstanceOnly => ServerSyncScope::ThisInstanceOnly,
            ScopeChoice::AllInstances => ServerSyncScope::AllInstances,
            ScopeChoice::Selected => ServerSyncScope::Selected(state.selected.clone()),
        };
        instances.set_server_sync_scope(state.address.as_str(), scope);
        // Apply right away instead of waiting for the next launch or exit.
        sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
        close = true;
    }
    if close || response.close_requested {
        ctx.data_mut(|data| data.remove::<ServerSyncModalState>(key));
    } else {
        ctx.data_mut(|data| data.insert_temp(key, state));
    }
}
