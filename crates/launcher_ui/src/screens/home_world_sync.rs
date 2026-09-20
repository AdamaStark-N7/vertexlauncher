//! World sync dialog: share a world between instances, change who shares it, or stop.

use std::collections::BTreeSet;

use instance_sync::compare_game_versions;

use super::*;
use crate::sync_runner::{
    self, SyncRunConfig, WorldSyncReceiver, WorldSyncRequest, WorldSyncResult,
};
use crate::ui::components::choice_controls;
use crate::ui::modal;

const REQUEST_KEY: &str = "home_world_sync_modal";
/// Set when a sync finished so Home rescans and shows the new links.
pub(super) const RESCAN_KEY: &str = "home_world_sync_rescan";

#[derive(Clone)]
struct WorldSyncModalState {
    /// Instance the world was picked from; it always stays a member.
    instance_id: String,
    folder: String,
    world_name: String,
    world_version: Option<String>,
    selected: BTreeSet<String>,
    pending: Option<WorldSyncReceiver>,
    /// `(is_error, text)` from the last operation.
    message: Option<(bool, String)>,
    warnings: Vec<String>,
}

pub(super) fn request_world_sync_modal(
    ctx: &egui::Context,
    world: &WorldEntry,
    instances: &InstanceStore,
) {
    let selected =
        match instances.synced_world_for(world.instance_id.as_str(), world.world_id.as_str()) {
            Some(synced) => synced.members.keys().cloned().collect(),
            None => BTreeSet::from([world.instance_id.clone()]),
        };
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(REQUEST_KEY),
            WorldSyncModalState {
                instance_id: world.instance_id.clone(),
                folder: world.world_id.clone(),
                world_name: world.world_name.clone(),
                world_version: world.version_name.clone(),
                selected,
                pending: None,
                message: None,
                warnings: Vec::new(),
            },
        );
    });
}

fn poll_pending(
    state: &mut WorldSyncModalState,
    instances: &mut InstanceStore,
    ctx: &egui::Context,
) {
    let Some(receiver) = state.pending.clone() else {
        return;
    };
    let result = receiver.lock().ok().and_then(|rx| rx.try_recv().ok());
    let Some(WorldSyncResult {
        synced_worlds,
        outcome,
    }) = result
    else {
        ctx.request_repaint_after(Duration::from_millis(100));
        return;
    };
    state.pending = None;
    match outcome {
        Ok(outcome) => {
            instances.synced_worlds = synced_worlds;
            state.warnings = outcome.warnings;
            state.message = Some((false, "Done.".to_owned()));
            // Reflect what actually ended up shared, e.g. members skipped for conflicts.
            state.selected = match instances
                .synced_world_for(state.instance_id.as_str(), state.folder.as_str())
            {
                Some(synced) => synced.members.keys().cloned().collect(),
                None => BTreeSet::from([state.instance_id.clone()]),
            };
            ctx.data_mut(|data| data.insert_temp(egui::Id::new(RESCAN_KEY), true));
        }
        Err(err) => state.message = Some((true, err)),
    }
}

pub(super) fn render_world_sync_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let key = egui::Id::new(REQUEST_KEY);
    let Some(mut state) = ctx.data(|data| data.get_temp::<WorldSyncModalState>(key)) else {
        return;
    };
    poll_pending(&mut state, instances, ctx);

    let synced_id = instances
        .synced_world_for(state.instance_id.as_str(), state.folder.as_str())
        .map(|world| world.id.clone());
    let busy = state.pending.is_some();
    let mut request: Option<WorldSyncRequest> = None;
    let mut close = false;

    let response = modal::show_window(
        ctx,
        "Sync world",
        modal::ModalOptions::new(
            egui::Id::new(("home_world_sync_modal_window", state.folder.as_str())),
            modal::ModalLayout::centered(
                modal::AxisSizing::new(0.5, 400.0, 640.0),
                modal::AxisSizing::new(0.75, 360.0, 760.0),
            ),
        )
        .with_layer(modal::ModalLayer::Base)
        .with_dismiss_behavior(modal::DismissBehavior::EscapeAndScrim),
        |ui| {
            let _ = text_ui.label(
                ui,
                "world_sync_title",
                "Sync world",
                &style::modal_title(ui),
            );
            let _ = text_ui.label(
                ui,
                "world_sync_target",
                state.world_name.as_str(),
                &style::body_strong(ui),
            );
            let help = if synced_id.is_some() {
                "This world is stored once and linked into each instance below, so progress made in any of them appears in all of them. Removing an instance gives it its own copy."
            } else {
                "The world moves to Vertex's shared worlds folder and each instance you pick gets a link to it (or a copy that refreshes at launch and exit where links aren't possible). Progress made in any of them appears in all of them. Close the game in every picked instance first."
            };
            let _ = text_ui.label(ui, "world_sync_help", help, &style::muted(ui));
            ui.add_space(style::SPACE_LG);

            egui::ScrollArea::vertical()
                .scroll_source(modal_host::drag_scroll_source())
                .id_salt("world_sync_instances_scroll")
                .max_height(280.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for instance in &instances.instances {
                        let mut checked = state.selected.contains(&instance.id);
                        let label =
                            format!("{} (Minecraft {})", instance.name, instance.game_version);
                        let changed = ui
                            .add_enabled_ui(!busy, |ui| {
                                choice_controls::checkbox(
                                    ui,
                                    text_ui,
                                    ("world_sync_instance", instance.id.as_str()),
                                    label.as_str(),
                                    &mut checked,
                                )
                                .changed()
                            })
                            .inner;
                        if changed {
                            if checked {
                                state.selected.insert(instance.id.clone());
                            } else {
                                state.selected.remove(&instance.id);
                            }
                        }
                        ui.add_space(style::SPACE_XS);
                    }
                });
            if synced_id.is_none() {
                // The instance the world lives in is always part of the group.
                state.selected.insert(state.instance_id.clone());
            }

            if let Some(world_version) = state.world_version.as_deref() {
                for instance in &instances.instances {
                    if state.selected.contains(&instance.id)
                        && compare_game_versions(&instance.game_version, world_version)
                            == Some(std::cmp::Ordering::Less)
                    {
                        let _ = text_ui.label(
                            ui,
                            ("world_sync_version_warning", instance.id.as_str()),
                            format!(
                                "{} runs Minecraft {}, older than this world ({}). Opening it there can corrupt the world.",
                                instance.name, instance.game_version, world_version
                            )
                            .as_str(),
                            &style::warning_text(ui),
                        );
                    }
                }
            }
            for (index, warning) in state.warnings.iter().enumerate() {
                let _ = text_ui.label(
                    ui,
                    ("world_sync_warning", index),
                    warning.as_str(),
                    &style::warning_text(ui),
                );
            }
            if busy {
                let _ = text_ui.label(
                    ui,
                    "world_sync_busy",
                    "Working… moving or copying a large world can take a while.",
                    &style::muted(ui),
                );
            } else if let Some((is_error, message)) = &state.message {
                let style = if *is_error {
                    style::error_text(ui)
                } else {
                    style::muted(ui)
                };
                let _ = text_ui.label(ui, "world_sync_message", message.as_str(), &style);
            }
            let _ = config;

            ui.add_space(style::SPACE_XL);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let size = egui::vec2(150.0, style::CONTROL_HEIGHT);
                if text_ui
                    .button(
                        ui,
                        "world_sync_close",
                        "Close",
                        &ui_foundation::secondary_button(
                            ui,
                            egui::vec2(100.0, style::CONTROL_HEIGHT),
                        ),
                    )
                    .clicked()
                {
                    close = true;
                }
                ui.add_enabled_ui(!busy, |ui| {
                    if let Some(world_id) = synced_id.clone() {
                        if text_ui
                            .button(
                                ui,
                                "world_sync_apply",
                                "Apply changes",
                                &ui_foundation::primary_button(ui, size),
                            )
                            .clicked()
                        {
                            request = Some(WorldSyncRequest::SetMembers {
                                world_id: world_id.clone(),
                                members: state.selected.clone(),
                            });
                        }
                        if text_ui
                            .button(
                                ui,
                                "world_sync_stop",
                                "Stop syncing",
                                &ui_foundation::danger_button(ui, size),
                            )
                            .clicked()
                        {
                            request = Some(WorldSyncRequest::Disable { world_id });
                        }
                    } else if state.selected.len() >= 2
                        && text_ui
                            .button(
                                ui,
                                "world_sync_start",
                                "Start syncing",
                                &ui_foundation::primary_button(ui, size),
                            )
                            .clicked()
                    {
                        request = Some(WorldSyncRequest::Enable {
                            source_instance_id: state.instance_id.clone(),
                            folder: state.folder.clone(),
                            members: state.selected.clone(),
                        });
                    }
                });
            });
        },
    );

    if let Some(request) = request {
        state.message = None;
        state.warnings.clear();
        state.pending = Some(sync_runner::spawn_world_sync(
            instances.clone(),
            &SyncRunConfig::from_config(config),
            request,
        ));
    }
    if (close || response.close_requested) && state.pending.is_none() {
        ctx.data_mut(|data| data.remove::<WorldSyncModalState>(key));
    } else {
        ctx.data_mut(|data| data.insert_temp(key, state));
    }
}
