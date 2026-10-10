use super::*;

pub(super) fn request_screenshot_delete(
    state: &mut HomeState,
    screenshot_key: String,
    path: PathBuf,
    file_name: String,
) {
    if state.delete_screenshot_in_flight {
        return;
    }

    let tx = state.delete_screenshot_results.sender();

    state.delete_screenshot_in_flight = true;
    let _ = tokio_runtime::spawn_detached(async move {
        let result = tokio::fs::remove_file(path.as_path()).await.map_err(|err| {
            tracing::warn!(target: "vertexlauncher/io", op = "remove_file", path = %path.display(), error = %err, context = "delete home screenshot");
            format!("failed to remove {}: {err}", path.display())
        });
        if let Err(err) = tx.send((screenshot_key.clone(), file_name.clone(), result)) {
            tracing::error!(
                target: "vertexlauncher/home",
                screenshot_key = %screenshot_key,
                file_name = %file_name,
                error = %err,
                "Failed to deliver home screenshot-delete result."
            );
        }
    });
}

pub(in crate::screens::home) fn poll_delete_screenshot_results(
    state: &mut HomeState,
    instances: &InstanceStore,
    config: &Config,
) {
    let drained = state.delete_screenshot_results.drain();
    let updates = drained.items;

    if drained.disconnected {
        tracing::error!(target: "vertexlauncher/home", "delete_screenshot_results worker channel stopped unexpectedly.");
        state.delete_screenshot_in_flight = false;
        notification::error!(
            "home/screenshots",
            "Screenshot delete worker stopped unexpectedly."
        );
    }

    for (screenshot_key, file_name, result) in updates {
        state.delete_screenshot_in_flight = false;
        match result {
            Ok(()) => {
                if state
                    .screenshot_viewer
                    .as_ref()
                    .is_some_and(|viewer| viewer.screenshot_key == screenshot_key)
                {
                    tracing::info!(
                        target: "vertexlauncher/screenshots",
                        screenshot_key = screenshot_key.as_str(),
                        "Home screenshot viewer closed because the screenshot was deleted."
                    );
                    state.screenshot_viewer = None;
                }
                state.pending_delete_screenshot_key = None;
                refresh_screenshot_state(state, instances, config, true);
                notification::info!("home/screenshots", "Deleted '{}' from disk.", file_name);
            }
            Err(err) => {
                state.pending_delete_screenshot_key = None;
                notification::error!(
                    "home/screenshots",
                    "Failed to delete '{}': {}",
                    file_name,
                    err
                );
            }
        }
    }
}

pub(in crate::screens::home) fn render_delete_screenshot_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    state: &mut HomeState,
    _instances: &InstanceStore,
    _config: &Config,
    metrics: HomeUiMetrics,
) {
    let Some(screenshot_key) = state.pending_delete_screenshot_key.clone() else {
        return;
    };
    let Some(screenshot) = state
        .screenshots
        .iter()
        .find(|entry| entry.key() == screenshot_key)
        .cloned()
    else {
        state.pending_delete_screenshot_key = None;
        return;
    };

    let danger = ctx.global_style().visuals.error_fg_color;
    let mut cancel_requested = false;
    let mut delete_requested = false;
    let request_cancel_focus = modal_default_focus_requested(
        ctx,
        ("home_delete_screenshot_modal", screenshot_key.as_str()),
    );
    let response = show_dialog(
        ctx,
        dialog_options("home_delete_screenshot_modal", DialogPreset::Confirm),
        |ui| {
            let heading_style = style::heading_color(ui, 28.0, 32.0, danger);
            let body_style = style::body(ui);
            let muted_style = style::muted(ui);

            let _ = text_ui.label(
                ui,
                (
                    "home_delete_screenshot_heading",
                    screenshot.path.display().to_string(),
                ),
                "Delete Screenshot?",
                &heading_style,
            );
            let _ = text_ui.label(
                ui,
                (
                    "home_delete_screenshot_body",
                    screenshot.path.display().to_string(),
                ),
                &format!(
                    "Delete \"{}\" from disk? This permanently removes the screenshot.",
                    screenshot.file_name
                ),
                &body_style,
            );
            let _ = text_ui.label(
                ui,
                (
                    "home_delete_screenshot_instance",
                    screenshot.path.display().to_string(),
                ),
                &format!("Instance: {}", screenshot.instance_name),
                &muted_style,
            );
            let _ = text_ui.label(
                ui,
                (
                    "home_delete_screenshot_path",
                    screenshot.path.display().to_string(),
                ),
                &format!("Path: {}", screenshot.path.display()),
                &muted_style,
            );

            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled_ui(!state.delete_screenshot_in_flight, |ui| {
                        text_ui.button(
                            ui,
                            "home_delete_screenshot_confirm",
                            "Delete",
                            &danger_button(
                                ui,
                                egui::vec2(metrics.action_button_width, style::CONTROL_HEIGHT),
                            ),
                        )
                    })
                    .inner
                    .clicked()
                {
                    delete_requested = true;
                }
                let cancel_response = ui
                    .add_enabled_ui(!state.delete_screenshot_in_flight, |ui| {
                        text_ui.button(
                            ui,
                            "home_delete_screenshot_cancel",
                            "Cancel",
                            &secondary_button(
                                ui,
                                egui::vec2(metrics.action_button_width, style::CONTROL_HEIGHT),
                            ),
                        )
                    })
                    .inner;
                if request_cancel_focus {
                    cancel_response.request_focus();
                }
                if cancel_response.clicked() {
                    cancel_requested = true;
                }
                if state.delete_screenshot_in_flight {
                    ui.spinner();
                }
            });
        },
    );
    cancel_requested |= response.close_requested && !state.delete_screenshot_in_flight;

    if cancel_requested && !state.delete_screenshot_in_flight {
        state.pending_delete_screenshot_key = None;
        return;
    }

    if delete_requested {
        request_screenshot_delete(
            state,
            screenshot_key,
            screenshot.path.clone(),
            screenshot.file_name.clone(),
        );
    }
}
