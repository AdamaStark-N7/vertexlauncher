use super::*;

pub(super) fn render_def_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    live: &mut LiveOptions,
    def: &OptionDef,
    version: Option<Ver>,
    audio_devices: Option<&Mutex<Option<Vec<String>>>>,
) {
    let shown = shown_value(live, def).to_owned();
    let tip = tooltip_for(def);
    let tip = Some(tip.as_str());
    match def.kind {
        Kind::Bool => {
            let mut on = value::parse_bool(&shown).unwrap_or(def.default == "true");
            let before = on;
            let _ = settings_widgets::toggle_row(text_ui, ui, def.label, tip, &mut on);
            if on != before {
                commit(live, def, version, &on.to_string());
            }
        }
        Kind::Int { min, max } => {
            let mut number = value::parse_i64(&shown)
                .or_else(|| def.default.parse().ok())
                .unwrap_or(min)
                .clamp(min, max) as i32;
            let before = number;
            let _ = settings_widgets::int_stepper_row(
                text_ui,
                ui,
                ("game_setting", def.key),
                def.label,
                tip,
                &mut number,
                min as i32,
                max as i32,
                1,
            );
            if number != before {
                commit(live, def, version, &number.to_string());
            }
        }
        Kind::Float { min, max } => {
            let stored =
                value::parse_f64(&shown).unwrap_or_else(|| def.default.parse().unwrap_or(min));
            let before;
            let mut edited;
            match def.display {
                Display::Degrees { offset, scale } => {
                    before = (offset + stored * scale) as f32;
                    edited = before;
                    let _ = settings_widgets::float_stepper_row(
                        text_ui,
                        ui,
                        ("game_setting", def.key),
                        def.label,
                        tip,
                        &mut edited,
                        (offset + min * scale) as f32,
                        (offset + max * scale) as f32,
                        1.0,
                    );
                    if edited != before {
                        commit(
                            live,
                            def,
                            version,
                            &format!("{}", (f64::from(edited) - offset) / scale),
                        );
                    }
                }
                Display::Percent if min == 0.0 && max == 1.0 => {
                    before = stored as f32;
                    edited = before;
                    let _ = settings_widgets::float_slider_row(
                        text_ui,
                        ui,
                        ("game_setting", def.key),
                        def.label,
                        tip,
                        &mut edited,
                        0.0,
                        1.0,
                        true,
                    );
                    if edited != before {
                        commit(live, def, version, &format!("{edited}"));
                    }
                }
                _ => {
                    before = stored as f32;
                    edited = before;
                    let _ = settings_widgets::float_stepper_row(
                        text_ui,
                        ui,
                        ("game_setting", def.key),
                        def.label,
                        tip,
                        &mut edited,
                        min as f32,
                        max as f32,
                        ((max - min) / 100.0).max(0.01) as f32,
                    );
                    if edited != before {
                        commit(live, def, version, &format!("{edited}"));
                    }
                }
            }
        }
        Kind::Choice(choices) => {
            let labels = choices
                .iter()
                .map(|(stored, label)| ((*stored).to_owned(), (*label).to_owned()))
                .collect();
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::IntChoice(choices) => {
            let labels = choices
                .iter()
                .map(|(stored, label)| (stored.to_string(), (*label).to_owned()))
                .collect();
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::Text if def.key == "lang" => {
            let labels = language_choices(&shown);
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::Text if def.key == "soundDevice" => {
            let devices = audio_devices.and_then(|slot| slot.lock().ok()?.clone());
            let labels = sound_device_choices(&shown, devices.as_deref());
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::Text => {
            let mut text = shown.clone();
            let _ = settings_widgets::text_input_row(
                text_ui,
                ui,
                ("game_setting", def.key),
                def.label,
                tip,
                &mut text,
            );
            if text != shown {
                commit(live, def, version, &text);
            }
        }
        Kind::List => {
            let items = value::parse_list(live.get(def.key).unwrap_or(def.default));
            let summary = if items.is_empty() {
                "none".to_owned()
            } else {
                items.join(", ")
            };
            let _ = text_ui.label(
                ui,
                ("game_setting_list", def.key),
                format!("{}: {summary}", def.label).as_str(),
                &style::muted(ui),
            );
        }
        Kind::Keybind => {}
    }
    ui.add_space(style::SPACE_SM);
}
