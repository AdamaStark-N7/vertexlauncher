use super::*;

/// Writes a new value for `def`, formatted the way this version's file expects.
pub(super) fn commit(
    live: &mut LiveOptions,
    def: &OptionDef,
    version: Option<Ver>,
    new_text: &str,
) {
    if let Some(raw) = format_for_def(def, new_text, live.file(), version)
        && live.get(def.key) != Some(raw.as_str())
    {
        live.edit(def.key, Some(&raw));
    }
}

pub(super) fn tooltip_for(def: &OptionDef) -> String {
    mc_options::descriptions::tooltip(def)
}

/// Raw value shown for a key: what's in the file, else the default.
pub(super) fn shown_value<'a>(live: &'a LiveOptions, def: &'a OptionDef) -> &'a str {
    value::unquote(live.get(def.key).unwrap_or(def.default)).0
}

pub(super) fn render_choice_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    live: &mut LiveOptions,
    def: &OptionDef,
    version: Option<Ver>,
    labels: Vec<(String, String)>,
) {
    let current = shown_value(live, def).to_owned();
    let mut labels = labels;
    let mut selected = labels.iter().position(|(stored, _)| *stored == current);
    if selected.is_none() {
        // Show what the file really holds instead of pretending it's a known choice.
        labels.push((current.clone(), format!("{current} (unrecognized)")));
        selected = Some(labels.len() - 1);
    }
    let mut index = selected.unwrap_or(0);
    let before = index;
    let names: Vec<&str> = labels.iter().map(|(_, label)| label.as_str()).collect();
    let tip = tooltip_for(def);
    let _ = settings_widgets::full_width_dropdown_row(
        text_ui,
        ui,
        ("game_setting", def.key),
        def.label,
        Some(tip.as_str()),
        &mut index,
        &names,
    );
    if index != before
        && let Some((stored, _)) = labels.get(index)
    {
        commit(live, def, version, stored);
    }
}

/// Languages for the dropdown. The stored code keeps whatever case the file uses (older versions
/// write `en_US`), so the current language is selected regardless of case.
pub(super) fn language_choices(current: &str) -> Vec<(String, String)> {
    mc_options::languages::LANGUAGES
        .iter()
        .map(|(code, name)| {
            let stored = if code.eq_ignore_ascii_case(current) {
                current.to_owned()
            } else {
                (*code).to_owned()
            };
            (stored, format!("{name} ({code})"))
        })
        .collect()
}

/// Output devices for the dropdown: the system default, every device found, and whatever the file
/// currently holds. Minecraft stores `OpenAL Soft on <name>`; a bare name is kept bare.
pub(super) fn sound_device_choices(
    current: &str,
    devices: Option<&[String]>,
) -> Vec<(String, String)> {
    let bare_current = current
        .strip_prefix(audio_devices::OPENAL_PREFIX)
        .unwrap_or(current);
    let prefixed = current.is_empty() || current.starts_with(audio_devices::OPENAL_PREFIX);
    let mut choices = vec![(String::new(), "System Default".to_owned())];
    let mut found_current = current.is_empty();
    for name in devices.unwrap_or_default() {
        let stored = if name == bare_current {
            found_current = true;
            current.to_owned()
        } else if prefixed {
            format!("{}{name}", audio_devices::OPENAL_PREFIX)
        } else {
            name.clone()
        };
        choices.push((stored, name.clone()));
    }
    if !found_current {
        // Not currently plugged in (or not detectable): keep it selectable.
        choices.push((current.to_owned(), bare_current.to_owned()));
    }
    choices
}
