use super::*;

pub(super) fn render_sync_tab(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &ModalState,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let _ = text_ui.label(
        ui,
        "settings_sync_intro",
        "Keep these settings in step across instances. The most recently changed instance wins; its settings are translated to each other instance's Minecraft version (renamed options, key codes, value formats). Settings that belong to this machine, like the window size or audio device, are never copied, and running instances are left alone until they close.",
        &style::muted(ui),
    );
    ui.add_space(style::SPACE_LG);

    let mut member = instances
        .game_settings_sync
        .instances
        .contains(&state.instance_id);
    let before = member;
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Sync this instance's settings",
        Some("Instances with this turned on share the settings chosen below."),
        &mut member,
    );
    if member != before {
        if member {
            instances
                .game_settings_sync
                .instances
                .insert(state.instance_id.clone());
        } else {
            instances
                .game_settings_sync
                .instances
                .remove(&state.instance_id);
        }
    }
    let participants = instances.game_settings_sync.instances.len();
    let _ = text_ui.label(
        ui,
        "settings_sync_count",
        if participants < 2 {
            "Turn this on for at least two instances for anything to sync.".to_owned()
        } else {
            format!("{participants} instances take part.")
        }
        .as_str(),
        &style::muted(ui),
    );

    ui.add_space(style::SPACE_LG);
    let _ = text_ui.label(
        ui,
        "settings_sync_groups",
        "Settings to sync",
        &style::subtitle(ui),
    );
    for group in Group::ALL
        .into_iter()
        .filter(|g| !matches!(g, Group::Online | Group::Internal))
    {
        let mut on = instances
            .game_settings_sync
            .categories
            .contains(group.slug());
        let before = on;
        let _ = settings_widgets::toggle_row(text_ui, ui, group.label(), None, &mut on);
        if on != before {
            if on {
                instances
                    .game_settings_sync
                    .categories
                    .insert(group.slug().to_owned());
            } else {
                instances.game_settings_sync.categories.remove(group.slug());
            }
        }
    }

    ui.add_space(style::SPACE_LG);
    let _ = text_ui.label(
        ui,
        "settings_sync_mods",
        "Mod settings to sync",
        &style::subtitle(ui),
    );
    let _ = text_ui.label(
        ui,
        "settings_sync_mods_help",
        "Only instances that already have the mod's config file take part. Thread counts, server identity and other machine-specific keys are skipped.",
        &style::muted(ui),
    );
    for (id, name) in instance_sync::available_mod_configs() {
        let mut on = instances.game_settings_sync.mod_configs.contains(id);
        let before = on;
        let _ = settings_widgets::toggle_row(text_ui, ui, name, None, &mut on);
        if on != before {
            if on {
                instances
                    .game_settings_sync
                    .mod_configs
                    .insert(id.to_owned());
            } else {
                instances.game_settings_sync.mod_configs.remove(id);
            }
        }
    }

    ui.add_space(style::SPACE_LG);
    let mut packs = instances.game_settings_sync.resource_packs;
    let before = packs;
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Sync resource packs",
        Some(
            "Shares packs between these instances where their metadata says they work (or you've said they do), and keeps which packs are enabled in step. Manage per-pack exceptions on the Resource Packs tab.",
        ),
        &mut packs,
    );
    if packs != before {
        instances.game_settings_sync.resource_packs = packs;
    }

    ui.add_space(style::SPACE_LG);
    if text_ui
        .button(
            ui,
            "settings_sync_now",
            "Sync now",
            &style::neutral_button_with_min_size(ui, egui::vec2(140.0, style::CONTROL_HEIGHT)),
        )
        .clicked()
    {
        sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
    }
}
