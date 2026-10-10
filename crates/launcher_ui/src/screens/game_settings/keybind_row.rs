use super::*;

pub(super) fn render_keybind_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
    key: &str,
    label: &str,
    default: &str,
    conflicts: &BTreeMap<String, Vec<String>>,
) {
    let legacy = state.version.is_some_and(|v| v < Ver(1, 13, 0));
    let raw = live.get(key).unwrap_or(default).to_owned();
    let modern = if legacy {
        raw.parse::<i32>()
            .ok()
            .and_then(keys::legacy_to_modern)
            .unwrap_or_else(|| raw.clone())
    } else {
        raw.clone()
    };
    let store = |live: &mut LiveOptions, modern_name: &str| {
        let stored = if legacy {
            match keys::modern_to_legacy(modern_name) {
                Some(code) => code.to_string(),
                None => return,
            }
        } else {
            modern_name.to_owned()
        };
        if live.get(key) != Some(stored.as_str()) {
            live.edit(key, Some(&stored));
        }
    };

    ui.horizontal(|ui| {
        let _ = text_ui.label(
            ui,
            ("keybind_label", key),
            label,
            &LabelOptions {
                wrap: false,
                ..style::body(ui)
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let capturing = state.capturing.as_deref() == Some(key);
            let button_text = if capturing {
                "Press a key…"
            } else {
                "Change"
            };
            if text_ui
                .button(
                    ui,
                    ("keybind_capture", key),
                    button_text,
                    &style::neutral_button_with_min_size(
                        ui,
                        egui::vec2(120.0, style::CONTROL_HEIGHT),
                    ),
                )
                .clicked()
            {
                state.capturing = (!capturing).then(|| key.to_owned());
                state.capture_started_at_frame = ui.ctx().cumulative_frame_nr();
            }
            if text_ui
                .button(
                    ui,
                    ("keybind_unbind", key),
                    "Unbind",
                    &style::neutral_button_with_min_size(
                        ui,
                        egui::vec2(90.0, style::CONTROL_HEIGHT),
                    ),
                )
                .clicked()
            {
                store(live, "key.keyboard.unknown");
            }
            let _ = text_ui.label(
                ui,
                ("keybind_value", key),
                keys::display_name(&modern).as_str(),
                &LabelOptions {
                    wrap: false,
                    ..style::body_strong(ui)
                },
            );
        });
    });
    if let Some(others) = conflicts.get(&modern).filter(|others| others.len() > 1)
        && modern != "key.keyboard.unknown"
    {
        let text = format!(
            "Also used by: {}",
            others
                .iter()
                .filter(|other| other.as_str() != label)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = text_ui.label(
            ui,
            ("keybind_conflict", key),
            text.as_str(),
            &style::warning_text(ui),
        );
    }

    if state.capturing.as_deref() == Some(key)
        && ui.ctx().cumulative_frame_nr() > state.capture_started_at_frame
        && let Some(captured) = key_capture::poll_capture(ui.ctx())
    {
        if let key_capture::Captured::Binding(name) = captured {
            store(live, &name);
        }
        state.capturing = None;
    }
}

/// Key → labels of every binding using it, to flag duplicates.
pub(super) fn keybind_usage(
    live: &LiveOptions,
    version: Option<Ver>,
) -> BTreeMap<String, Vec<String>> {
    let legacy = version.is_some_and(|v| v < Ver(1, 13, 0));
    let mut usage: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in live.file().keys() {
        if !schema::is_keybind_key(key) {
            continue;
        }
        let raw = live.get(key).unwrap_or_default();
        let modern = if legacy {
            raw.parse::<i32>().ok().and_then(keys::legacy_to_modern)
        } else {
            Some(raw.to_owned())
        };
        if let Some(modern) = modern {
            let label = schema::find_def(key, version)
                .map_or_else(|| pretty_key(key), |d| d.label.to_owned());
            usage.entry(modern).or_default().push(label);
        }
    }
    usage
}

pub(super) fn pretty_key(key: &str) -> String {
    key.trim_start_matches("key_key.")
        .trim_start_matches("key_")
        .replace(['.', '_'], " ")
}
