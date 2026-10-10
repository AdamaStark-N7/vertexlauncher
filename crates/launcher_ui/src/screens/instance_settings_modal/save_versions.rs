use super::*;

pub(super) fn render_save_versions_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instances: &mut InstanceStore,
    config: &Config,
    instance_id: &str,
    styles: &ModalStyles,
    instances_changed: &mut bool,
) {
    ui.add_space(8.0);

    let validation_error = metadata_validation_error(state);
    let can_save_versions = validation_error.is_none();
    if let Some(error) = validation_error.as_deref() {
        let _ = text_ui.label(
            ui,
            ("instance_save_versions_validation_error", instance_id),
            error,
            &style::error_text(ui),
        );
        ui.add_space(6.0);
    }

    let save_versions_clicked = ui
        .add_enabled_ui(can_save_versions, |ui| {
            text_ui.button(
                ui,
                ("instance_save_versions", instance_id),
                "Save metadata & versions",
                &styles.action_button,
            )
        })
        .inner
        .clicked();
    let reinstall_enabled = can_save_versions && !state.runtime_prepare_in_flight && !state.running;
    let reinstall_clicked = ui
        .add_enabled_ui(reinstall_enabled, |ui| {
            text_ui.button(
                ui,
                ("instance_reinstall_profile", instance_id),
                "Reinstall Profile",
                &styles.reinstall_button,
            )
        })
        .inner
        .clicked();
    if save_versions_clicked {
        handle_save_versions_click(state, instance_id, instances, instances_changed);
    }
    if reinstall_clicked {
        handle_reinstall_click(state, instance_id, instances, config, instances_changed);
    }
}

fn metadata_validation_error(state: &InstanceScreenState) -> Option<String> {
    let trimmed_name = state.name_input.trim();
    let requested_modloader = selected_modloader_value(state);
    let requested_game_version = state.game_version_input.trim().to_owned();
    let validation_error = if trimmed_name.is_empty() {
        Some("Name cannot be empty.".to_owned())
    } else if requested_game_version.is_empty() {
        Some("Minecraft game version cannot be empty.".to_owned())
    } else if requested_modloader.trim().is_empty() {
        Some("Modloader cannot be empty.".to_owned())
    } else if support_catalog_ready(state)
        && !state.loader_support.supports_loader(
            requested_modloader.as_str(),
            requested_game_version.as_str(),
        )
        && state.selected_modloader != CUSTOM_MODLOADER_INDEX
    {
        Some(format!(
            "{} is not available for Minecraft {}.",
            requested_modloader, requested_game_version
        ))
    } else {
        resolve_modloader_version_for_settings(
            state,
            requested_modloader.as_str(),
            requested_game_version.as_str(),
        )
        .err()
    };
    validation_error
}

fn handle_save_versions_click(
    state: &mut InstanceScreenState,
    instance_id: &str,
    instances: &mut InstanceStore,
    instances_changed: &mut bool,
) {
    match save_instance_metadata_and_versions(state, instance_id, instances) {
        Ok(()) => {
            *instances_changed = true;
            if let Some(saved) = instances.find(instance_id) {
                tracing::info!(
                    target: "vertexlauncher/ui/instance",
                    instance_id = %instance_id,
                    saved_modloader = %saved.modloader,
                    saved_game_version = %saved.game_version,
                    saved_modloader_version = %saved.modloader_version,
                    "Saved instance metadata and versions."
                );
            }
            state.status_message = Some("Saved metadata and version settings.".to_owned());
        }
        Err(err) => {
            tracing::warn!(
                target: "vertexlauncher/ui/instance",
                instance_id = %instance_id,
                error = %err,
                "Failed to save instance metadata and versions."
            );
            state.status_message = Some(err);
        }
    }
}

fn handle_reinstall_click(
    state: &mut InstanceScreenState,
    instance_id: &str,
    instances: &mut InstanceStore,
    config: &Config,
    instances_changed: &mut bool,
) {
    match save_instance_metadata_and_versions(state, instance_id, instances) {
        Ok(()) => {
            *instances_changed = true;
            let game_version = state.game_version_input.trim().to_owned();
            let modloader = selected_modloader_value(state);
            if let Some(saved_instance) = instances.find(instance_id).cloned() {
                let modloader_version =
                    normalize_optional(saved_instance.modloader_version.as_str());
                let installations_root = config.minecraft_installations_root_path().to_path_buf();
                let instance_root =
                    instances::instance_root_path(&installations_root, &saved_instance);
                let (linux_set_opengl_driver, linux_use_zink_driver) =
                    effective_linux_graphics_settings_for_state(state, config);
                request_runtime_prepare(
                    state,
                    RuntimePrepareOperation::ReinstallProfile,
                    instance_root,
                    game_version.clone(),
                    modloader.clone(),
                    modloader_version,
                    config.effective_required_java_major(game_version.as_str()),
                    config.choose_java_executable(
                        state.java_override_enabled,
                        state.java_override_runtime_major,
                        config.effective_required_java_major(game_version.as_str()),
                    ),
                    config.download_max_concurrent(),
                    config.parsed_download_speed_limit_bps(),
                    linux_set_opengl_driver,
                    linux_use_zink_driver,
                    config.default_instance_max_memory_mib(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                );
            } else {
                state.status_message = Some("Instance was removed before reinstall.".to_owned());
            }
        }
        Err(err) => {
            state.status_message = Some(err);
        }
    }
}
