use super::*;

pub(super) fn render_modloader_section(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instance_id: &str,
    styles: &ModalStyles,
) {
    let selected_game_version_for_loader = selected_game_version(state).to_owned();
    ensure_selected_modloader_is_supported(state, selected_game_version_for_loader.as_str());
    let _ = text_ui.label(
        ui,
        ("instance_modloader_label", instance_id),
        "Modloader",
        &styles.body,
    );
    ui.add_space(4.0);
    render_modloader_selector(
        ui,
        text_ui,
        state,
        instance_id,
        selected_game_version_for_loader.as_str(),
    );
    if state.selected_modloader == CUSTOM_MODLOADER_INDEX {
        ui.add_space(6.0);
        let _ = settings_widgets::full_width_text_input_row(
            text_ui,
            ui,
            ("instance_custom_modloader_input", instance_id),
            "Custom modloader id",
            Some("Use any custom modloader name."),
            &mut state.custom_modloader,
        );
    }
    ui.add_space(6.0);

    let selected_modloader_label = selected_modloader_value(state);
    let modloader_versions_key = modloader_versions_cache_key(
        selected_modloader_label.as_str(),
        selected_game_version_for_loader.as_str(),
    );
    let available_modloader_versions =
        selected_modloader_versions(state, selected_game_version_for_loader.as_str()).to_vec();
    if state.selected_modloader == 0 {
        state.modloader_version_input.clear();
    } else {
        let mut resolved_modloader_versions = available_modloader_versions;
        let should_fetch_remote = state.selected_modloader != CUSTOM_MODLOADER_INDEX
            && resolved_modloader_versions.is_empty();
        if should_fetch_remote {
            if let Some(cached) = state.modloader_versions_cache.get(&modloader_versions_key) {
                resolved_modloader_versions = cached.clone();
            } else {
                request_modloader_versions(
                    state,
                    selected_modloader_label.as_str(),
                    selected_game_version_for_loader.as_str(),
                    false,
                );
            }
        }

        let in_flight = state
            .modloader_versions_in_flight
            .contains(&modloader_versions_key);
        if in_flight {
            ui.horizontal(|ui| {
                ui.spinner();
                let _ = text_ui.label(
                    ui,
                    ("instance_modloader_versions_fetching", instance_id),
                    "Fetching modloader versions...",
                    &styles.muted,
                );
            });
        }

        if state.modloader_versions_status_key.as_deref() == Some(modloader_versions_key.as_str())
            && let Some(status) = state.modloader_versions_status.as_deref()
        {
            let is_error = status.starts_with("Failed");
            let _ = text_ui.label(
                ui,
                ("instance_modloader_versions_status", instance_id),
                status,
                &if is_error {
                    style::error_text(ui)
                } else {
                    style::muted(ui)
                },
            );
        }

        let modloader_version_options: Vec<String> = resolved_modloader_versions.clone();

        if state.modloader_version_input.trim().is_empty()
            && let Some(first) = modloader_version_options.first()
        {
            state.modloader_version_input = first.clone();
        }

        let option_refs: Vec<&str> = modloader_version_options
            .iter()
            .map(String::as_str)
            .collect();
        let current_modloader_version = state.modloader_version_input.trim().to_owned();
        let mut selected_index = modloader_version_options
            .iter()
            .position(|entry| entry == &current_modloader_version)
            .unwrap_or(0);
        if settings_widgets::full_width_dropdown_row(
            text_ui,
            ui,
            ("instance_modloader_version_dropdown", instance_id),
            "Modloader version",
            Some("Cataloged by loader+Minecraft compatibility and cached once per day."),
            &mut selected_index,
            &option_refs,
        )
        .changed()
        {
            if let Some(selected) = modloader_version_options.get(selected_index) {
                state.modloader_version_input = selected.clone();
            }
        }

        if state.selected_modloader != CUSTOM_MODLOADER_INDEX {
            let refresh_clicked = ui
                .add_enabled_ui(!in_flight, |ui| {
                    text_ui.button(
                        ui,
                        ("instance_modloader_versions_refresh", instance_id),
                        "Refresh modloader versions",
                        &styles.refresh,
                    )
                })
                .inner
                .clicked();
            if refresh_clicked {
                request_modloader_versions(
                    state,
                    selected_modloader_label.as_str(),
                    selected_game_version_for_loader.as_str(),
                    true,
                );
            }
        }

        if resolved_modloader_versions.is_empty()
            && state.selected_modloader != CUSTOM_MODLOADER_INDEX
        {
            let _ = text_ui.label(
                ui,
                ("instance_modloader_versions_unavailable", instance_id),
                "No cataloged modloader versions were found for this Minecraft version.",
                &styles.muted,
            );
        }
    }
}
