use super::*;

pub(super) fn render_metadata_section(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instance_id: &str,
    styles: &ModalStyles,
) {
    let _ = text_ui.label(
        ui,
        ("instance_settings_modal_heading", instance_id),
        "Instance Settings",
        &styles.section,
    );
    let _ = text_ui.label(
        ui,
        ("instance_settings_modal_description", instance_id),
        "Manage this profile's metadata, version stack, runtime overrides, and maintenance actions.",
        &styles.muted,
    );
    ui.add_space(8.0);

    let _ = text_ui.label(
        ui,
        ("instance_versions_heading", instance_id),
        "Metadata & Versions",
        &styles.section,
    );
    let _ = text_ui.label(
        ui,
        ("instance_versions_description", instance_id),
        "Display info, Minecraft version, and modloader selection for this instance.",
        &styles.muted,
    );
    ui.add_space(8.0);

    let _ = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        ("instance_name_input", instance_id),
        "Name",
        Some("Display name shown in the sidebar."),
        &mut state.name_input,
    );
    ui.add_space(6.0);
    let _ = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        ("instance_description_input", instance_id),
        "Description (optional)",
        Some("Optional note shown in library instance tiles."),
        &mut state.description_input,
    );
    ui.add_space(6.0);

    let mut thumbnail_input = state
        .thumbnail_input
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let thumbnail_changed = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        ("instance_thumbnail_input", instance_id),
        "Thumbnail path (optional)",
        Some("Local image path for this instance."),
        &mut thumbnail_input,
    )
    .changed();
    if thumbnail_changed {
        let trimmed = thumbnail_input.trim();
        state.thumbnail_input = if trimmed.is_empty() {
            PathBuf::new()
        } else {
            PathBuf::from(trimmed)
        };
    }
    ui.add_space(6.0);
}

pub(super) fn render_version_actions_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instances: &mut InstanceStore,
    config: &Config,
    instance_id: &str,
    instance_root_for_detection: &Option<PathBuf>,
    styles: &ModalStyles,
    instances_changed: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        if text_ui
            .button(
                ui,
                ("instance_refresh_versions", instance_id),
                "Refresh version list",
                &styles.refresh,
            )
            .clicked()
        {
            sync_version_catalog(
                state,
                installation::VersionCatalogFilter {
                    include_snapshots_and_betas: config.include_snapshots_and_betas(),
                    include_alpha: config.include_alpha_versions(),
                    include_experimental: config.include_experimental_versions(),
                },
                true,
            );
            state.modloader_versions_cache.clear();
            state.modloader_versions_status = None;
            state.modloader_versions_status_key = None;
        }

        let detect_clicked = ui
            .add_enabled_ui(instance_root_for_detection.is_some(), |ui| {
                text_ui.button(
                    ui,
                    ("instance_detect_versions", instance_id),
                    "Auto-detect installed versions",
                    &styles.refresh,
                )
            })
            .inner
            .clicked();
        if detect_clicked {
            if let Some(instance_root) = instance_root_for_detection.as_deref() {
                match detect_instance_versions(instance_root) {
                    Ok(detected) => {
                        apply_detected_instance_versions(state, &detected);
                        match set_instance_versions(
                            instances,
                            instance_id,
                            detected.modloader.clone(),
                            detected.game_version.clone(),
                            detected.modloader_version.clone(),
                        ) {
                            Ok(()) => {
                                *instances_changed = true;
                                state.status_message = Some(format!(
                                    "Detected and saved {} from profile '{}'.",
                                    detected_versions_summary(&detected),
                                    detected.source_profile_id
                                ));
                            }
                            Err(err) => {
                                state.status_message = Some(format!(
                                    "Detected versions, but failed to save them: {err}"
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        state.status_message =
                            Some(format!("Could not auto-detect installed versions: {err}"));
                    }
                }
            }
        }
    });
}

pub(super) fn render_version_catalog_controls(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instance_id: &str,
    styles: &ModalStyles,
) {
    if state.version_catalog_in_flight {
        ui.horizontal(|ui| {
            ui.spinner();
            let _ = text_ui.label(
                ui,
                ("instance_versions_loading", instance_id),
                "Fetching version catalog...",
                &styles.muted,
            );
        });
    }

    if let Some(catalog_error) = state.version_catalog_error.as_deref() {
        let _ = text_ui.label(
            ui,
            ("instance_version_catalog_error", instance_id),
            catalog_error,
            &style::error_text(ui),
        );
    }

    let version_labels: Vec<String> = state
        .available_game_versions
        .iter()
        .map(MinecraftVersionEntry::display_label)
        .collect();
    let version_refs: Vec<&str> = version_labels.iter().map(String::as_str).collect();
    if !version_refs.is_empty() {
        let mut selected_index = state
            .selected_game_version_index
            .min(version_refs.len().saturating_sub(1));
        let response = settings_widgets::dropdown_row(
            text_ui,
            ui,
            ("instance_game_version_dropdown", instance_id),
            "Minecraft game version",
            Some("Pick from available Minecraft versions."),
            &mut selected_index,
            &version_refs,
        );
        if response.changed() {
            state.selected_game_version_index = selected_index;
            if let Some(version) = state.available_game_versions.get(selected_index) {
                state.game_version_input = version.id.clone();
            }
        }
    } else {
        let _ = text_ui.label(
            ui,
            ("instance_game_version_empty", instance_id),
            "No game versions available yet.",
            &styles.muted,
        );
    }
    ui.add_space(6.0);
}
