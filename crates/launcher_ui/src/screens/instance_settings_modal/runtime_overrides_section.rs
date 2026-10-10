use super::*;

pub(super) fn render_runtime_overrides_section(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instances: &mut InstanceStore,
    config: &Config,
    instance_id: &str,
    styles: &ModalStyles,
    instances_changed: &mut bool,
) {
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(10.0);

    let _ = text_ui.label(
        ui,
        ("instance_settings_heading", instance_id),
        "Runtime Overrides",
        &styles.section,
    );
    let _ = text_ui.label(
        ui,
        ("instance_runtime_overrides_description", instance_id),
        "Per-instance overrides for memory, JVM arguments, and Java runtime selection.",
        &styles.muted,
    );
    ui.add_space(8.0);
    let memory_slider_max = render_memory_override(ui, text_ui, state, instance_id);
    render_launch_option_overrides(ui, text_ui, state, instance_id);
    let java_options = render_java_override(ui, text_ui, state, instance_id, config);
    ui.add_space(8.0);

    if text_ui
        .button(
            ui,
            ("instance_save_settings", instance_id),
            "Save instance settings",
            &styles.action_button,
        )
        .clicked()
    {
        match save_runtime_overrides(
            state,
            instance_id,
            instances,
            &java_options,
            memory_slider_max,
        ) {
            Ok(()) => {
                *instances_changed = true;
                state.status_message = Some("Saved instance settings.".to_owned());
            }
            Err(err) => state.status_message = Some(err),
        }
    }

    render_platform_specific_instance_settings_section(
        ui,
        text_ui,
        state,
        instance_id,
        &styles.section,
        &styles.muted,
    );
    *instances_changed |= autosave_runtime_overrides(
        ui.ctx(),
        state,
        instance_id,
        instances,
        &java_options,
        memory_slider_max,
    );
}

fn render_memory_override(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instance_id: &str,
) -> u128 {
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Override max memory for this instance",
        Some("When disabled, launcher instance default memory is used."),
        &mut state.memory_override_enabled,
    );
    ui.add_space(6.0);

    let (memory_slider_max, memory_slider_pending) = memory_slider_max_mib();
    if memory_slider_pending {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    if state.memory_override_enabled {
        let mut memory_mib = state
            .memory_override_mib
            .clamp(INSTANCE_DEFAULT_MAX_MEMORY_MIB_MIN, memory_slider_max);
        let response = settings_widgets::u128_slider_with_input_row(
            text_ui,
            ui,
            ("instance_memory_override", instance_id),
            "Max memory allocation (MiB)",
            Some("Per-instance memory limit."),
            &mut memory_mib,
            INSTANCE_DEFAULT_MAX_MEMORY_MIB_MIN,
            memory_slider_max,
            INSTANCE_DEFAULT_MAX_MEMORY_MIB_STEP,
        );
        if response.changed() {
            state.memory_override_mib = memory_mib;
        }
        ui.add_space(6.0);
    }
    memory_slider_max
}

fn render_launch_option_overrides(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instance_id: &str,
) {
    let _ = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        ("instance_cli_args_override", instance_id),
        "JVM args override (optional)",
        Some("Leave blank to use launcher instance default JVM args."),
        &mut state.cli_args_input,
    );
    ui.add_space(6.0);
    let _ = settings_widgets::full_width_multiline_text_input_row(
        text_ui,
        ui,
        ("instance_env_vars_override", instance_id),
        "Environment variables (optional)",
        Some(
            "One KEY=value per line. Blank lines and # comments are ignored. Values are not written to launch logs.",
        ),
        &mut state.env_vars_input,
        4,
        Some("VK_INSTANCE_LAYERS=\nMESA_VK_WSI_PRESENT_MODE=mailbox"),
    );
    ui.add_space(6.0);
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "This modpack has a rich presence mod",
        Some(
            "When enabled, Vertex clears its own Discord Rich Presence for this instance after launch so the mod inside Minecraft can take over.",
        ),
        &mut state.discord_rich_presence_mod_installed,
    );
    ui.add_space(8.0);

    ui.add_space(8.0);
}

fn render_java_override(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instance_id: &str,
    config: &Config,
) -> Vec<(u8, String)> {
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Override Java runtime for this instance",
        Some("When enabled, this instance will use the selected configured global Java path."),
        &mut state.java_override_enabled,
    );
    ui.add_space(6.0);

    let java_options = configured_java_path_options(config);
    if state.java_override_enabled {
        if java_options.is_empty() {
            let _ = text_ui.label(
                ui,
                ("instance_java_override_no_options", instance_id),
                "No configured global Java paths found. Add at least one Java path in Settings first.",
                &style::error_text(ui),
            );
        } else {
            if state
                .java_override_runtime_major
                .is_none_or(|major| !java_options.iter().any(|(m, _)| *m == major))
            {
                state.java_override_runtime_major = java_options.first().map(|(major, _)| *major);
            }
            let option_labels: Vec<&str> = java_options
                .iter()
                .map(|(_, label)| label.as_str())
                .collect();
            let mut selected_index = java_options
                .iter()
                .position(|(major, _)| Some(*major) == state.java_override_runtime_major)
                .unwrap_or(0);
            if settings_widgets::full_width_dropdown_row(
                text_ui,
                ui,
                ("instance_java_override_runtime", instance_id),
                "Java path override",
                Some("Select which configured Java path this instance should use."),
                &mut selected_index,
                &option_labels,
            )
            .changed()
            {
                state.java_override_runtime_major =
                    java_options.get(selected_index).map(|(major, _)| *major);
            }
        }
    }
    java_options
}
