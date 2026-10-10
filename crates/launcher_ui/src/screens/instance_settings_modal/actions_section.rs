use super::*;

pub(super) fn render_actions_section(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    instances: &InstanceStore,
    config: &Config,
    instance_id: &str,
    styles: &ModalStyles,
) -> bool {
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(10.0);

    let _ = text_ui.label(
        ui,
        ("instance_actions_heading", instance_id),
        "Maintenance & Actions",
        &styles.section,
    );
    let _ = text_ui.label(
        ui,
        ("instance_actions_description", instance_id),
        "Open the instance folder and commit any metadata or runtime changes.",
        &styles.muted,
    );
    ui.add_space(8.0);

    if text_ui
        .button(
            ui,
            ("instance_open_folder", instance_id),
            "Open Instance Folder",
            &styles.refresh,
        )
        .clicked()
    {
        open_instance_folder(state, instances, config, instance_id);
    }
    ui.add_space(6.0);
    if text_ui
        .button(
            ui,
            ("instance_export_vtmpack", instance_id),
            "Export .vtmpack...",
            &styles.refresh,
        )
        .clicked()
    {
        state.export_vtmpack_patch_mode = false;
        state.show_export_vtmpack_modal = true;
    }
    ui.add_space(6.0);
    if text_ui
        .button(
            ui,
            ("instance_generate_vtmpatch", instance_id),
            "Generate Patch...",
            &styles.refresh,
        )
        .clicked()
    {
        state.export_vtmpack_patch_mode = true;
        state.show_export_vtmpack_modal = true;
    }
    ui.add_space(6.0);
    if text_ui
        .button(
            ui,
            ("instance_export_server_zip", instance_id),
            "Auto-generate server zip...",
            &styles.refresh,
        )
        .clicked()
    {
        state.show_export_server_modal = true;
    }
    ui.add_space(6.0);
    if text_ui
        .button(
            ui,
            ("instance_move_instance", instance_id),
            "Move Instance...",
            &styles.refresh,
        )
        .clicked()
    {
        prepare_move_instance_modal(state, instances, config, instance_id);
    }
    ui.add_space(8.0);

    let mut close_requested = false;
    ui.horizontal(|ui| {
        if text_ui
            .button(
                ui,
                ("instance_settings_close", instance_id),
                "Done",
                &styles.action_button,
            )
            .clicked()
        {
            close_requested = true;
        }
    });
    close_requested
}

fn open_instance_folder(
    state: &mut InstanceScreenState,
    instances: &InstanceStore,
    config: &Config,
    instance_id: &str,
) {
    if let Some(instance) = instances.find(instance_id) {
        let installations_root = config.minecraft_installations_root_path().to_path_buf();
        let instance_root = instances::instance_root_path(&installations_root, instance);
        match desktop::open_in_file_manager(instance_root.as_path()) {
            Ok(()) => {
                state.status_message = Some(format!(
                    "Opened instance folder: {}",
                    instance_root.display()
                ));
            }
            Err(err) => {
                state.status_message = Some(format!("Failed to open instance folder: {err}"));
            }
        }
    } else {
        state.status_message = Some("Instance was removed before opening its folder.".to_owned());
    }
}

fn prepare_move_instance_modal(
    state: &mut InstanceScreenState,
    instances: &InstanceStore,
    config: &Config,
    instance_id: &str,
) {
    if let Some(instance) = instances.find(instance_id) {
        let installations_root = config.minecraft_installations_root_path().to_path_buf();
        let current_root = instances::instance_root_path(&installations_root, instance);
        state.move_instance_dest_input = current_root.display().to_string();
    } else {
        state.move_instance_dest_input = String::new();
    }
    state.move_instance_dest_valid = false;
    state.move_instance_dest_error = None;
    state.show_move_instance_modal = true;
}
