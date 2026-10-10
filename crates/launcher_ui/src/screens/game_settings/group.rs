use super::*;

pub(super) fn render_group(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
    group: Group,
    mut heading: Option<&str>,
) -> usize {
    let version = state.version;
    let search = state.search.clone();
    let defs: Vec<&'static OptionDef> = schema::defs_for(version)
        .filter(|def| !def.hidden && def.group == group && def_matches_search(&search, def))
        .collect();
    let mut rendered = 0usize;

    if group == Group::Keybinds {
        let usage = keybind_usage(live, version);
        let mut shown = std::collections::HashSet::new();
        for def in &defs {
            shown.insert(def.key);
            heading_once(ui, text_ui, &mut heading);
            rendered += 1;
            render_keybind_row(
                ui,
                text_ui,
                state,
                live,
                def.key,
                def.label,
                def.default,
                &usage,
            );
        }
        // Binds from mods (and other versions) that the file already contains.
        let extra: Vec<String> = live
            .file()
            .keys()
            .into_iter()
            .filter(|key| schema::is_keybind_key(key) && !shown.contains(key))
            .filter(|key| matches_search(&search, &pretty_key(key), key))
            .map(str::to_owned)
            .collect();
        if !extra.is_empty() {
            heading_once(ui, text_ui, &mut heading);
            rendered += extra.len();
            ui.add_space(style::SPACE_LG);
            let _ = text_ui.label(
                ui,
                "keybinds_modded",
                "Mod & other key binds",
                &style::subtitle(ui),
            );
            for key in extra {
                let label = pretty_key(&key);
                render_keybind_row(
                    ui,
                    text_ui,
                    state,
                    live,
                    &key,
                    &label,
                    "key.keyboard.unknown",
                    &usage,
                );
            }
        }
        return rendered;
    }

    let audio_devices = Arc::clone(&state.audio_devices);
    for def in defs {
        heading_once(ui, text_ui, &mut heading);
        rendered += 1;
        render_def_row(ui, text_ui, live, def, version, Some(&*audio_devices));
    }

    if group == Group::Internal {
        // Keys this launcher has no definition for: shown raw so nothing is hidden.
        let unknown: Vec<String> = live
            .file()
            .keys()
            .into_iter()
            .filter(|key| !schema::is_keybind_key(key) && schema::find_def(key, version).is_none())
            .filter(|key| !key.starts_with("soundCategory_") || schema::find_def_any(key).is_none())
            .filter(|key| matches_search(&search, key, key))
            .map(str::to_owned)
            .collect();
        if !unknown.is_empty() {
            heading_once(ui, text_ui, &mut heading);
            rendered += unknown.len();
            ui.add_space(style::SPACE_LG);
            let _ = text_ui.label(
                ui,
                "other_keys_title",
                "Other keys (mods and other versions)",
                &style::subtitle(ui),
            );
            for key in unknown {
                let current = live.get(&key).unwrap_or_default().to_owned();
                let mut text = current.clone();
                let _ = settings_widgets::text_input_row(
                    text_ui,
                    ui,
                    ("game_setting_raw", key.as_str()),
                    key.as_str(),
                    Some("Not a setting this launcher knows; edited as raw text."),
                    &mut text,
                );
                if text != current {
                    live.edit(&key, Some(&text));
                }
                ui.add_space(style::SPACE_SM);
            }
        }
    }
    rendered
}

/// Everything matching the search, across every tab, with a heading per section.
pub(super) fn render_search_results(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
) {
    let mut total = 0usize;
    for group in Group::ALL {
        total += render_group(ui, text_ui, state, live, group, Some(group.label()));
    }
    if total == 0 {
        let _ = text_ui.label(
            ui,
            "game_settings_no_matches",
            "No settings match your search.",
            &style::muted(ui),
        );
    }
}
