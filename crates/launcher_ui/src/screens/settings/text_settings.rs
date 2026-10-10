use super::*;

pub(super) fn render_theme_setting(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    available_themes: &[Theme],
    available_theme_labels: &[String],
) {
    if available_themes.is_empty() {
        return;
    }
    let mut selected_theme_index = available_themes
        .iter()
        .position(|theme| theme.id == config.theme_id())
        .unwrap_or(0);
    let theme_labels: Vec<&str> = available_theme_labels.iter().map(String::as_str).collect();
    let theme_tooltip = format!(
        "Themes are loaded from {} when the launcher starts; add or edit a theme file there and restart to see it.",
        app_paths::themes_dir().display()
    );
    let response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "theme_selector",
        "Theme",
        Some(theme_tooltip.as_str()),
        &mut selected_theme_index,
        &theme_labels,
    );
    let should_force_focus = ui.ctx().data_mut(|data| {
        data.get_temp::<bool>(egui::Id::new(FORCE_THEME_FOCUS_ID))
            .unwrap_or(false)
    });
    if should_force_focus || ui.ctx().memory(|memory| memory.focused().is_none()) {
        response.request_focus();
        ui.ctx()
            .data_mut(|data| data.remove::<bool>(egui::Id::new(FORCE_THEME_FOCUS_ID)));
    }
    if response.changed() {
        if let Some(theme) = available_themes.get(selected_theme_index) {
            config.set_theme_id(theme.id.clone());
        }
    }
    let export_response = text_ui.button(
        ui,
        "theme_export_json",
        "Export theme as JSON",
        &style::neutral_button(ui),
    );
    let export_response = style::hover_tip(
        ui,
        export_response,
        "Saves the selected theme as a JSON file in the themes folder, to use as a starting point for your own theme.",
    );
    if export_response.clicked()
        && let Some(theme) = available_themes.get(selected_theme_index)
    {
        export_theme_json(theme);
    }
    ui.add_space(style::SPACE_MD);
}

fn export_theme_json(theme: &Theme) {
    match theme.export_json_to_themes_dir() {
        Ok(path) => {
            let folder = path.parent().map(Path::to_path_buf);
            crate::notification::emit_with_actions(
                crate::notification::Severity::Info,
                "Settings",
                format!(
                    "Exported {} to {}. Give the copy a new \"id\" and restart to load it as its own theme.",
                    theme.name,
                    path.display()
                ),
                vec![crate::notification::NotificationAction::new(
                    "Open folder",
                    move |_ctx| {
                        if let Some(folder) = folder.as_deref()
                            && let Err(err) = crate::desktop::open_in_file_manager(folder)
                        {
                            crate::notification::error!(
                                "Settings",
                                "Failed to open the themes folder: {}",
                                err
                            );
                        }
                    },
                )],
            );
        }
        Err(err) => {
            crate::notification::error!("Settings", "Failed to export theme: {}", err);
        }
    }
}

pub(super) fn render_ui_font_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    available_ui_fonts: &[UiFontFamily],
    available_ui_font_labels: &[String],
) {
    config.for_each_dropdown_mut(|setting, value| {
        if setting.id != DropdownSettingId::UiFontFamily {
            return;
        }
        ui.push_id(setting.id, |ui| {
            let options: &[UiFontFamily] = available_ui_fonts;
            if options.is_empty() {
                return;
            }
            let selected_option_index = options
                .iter()
                .position(|option| option.matches(value))
                .unwrap_or(0);
            if options[selected_option_index] != *value {
                *value = options[selected_option_index].clone();
            }
            let option_label_refs: Vec<&str> = available_ui_font_labels
                .iter()
                .map(String::as_str)
                .collect();
            let mut selected_index = selected_option_index;
            let response = settings_widgets::searchable_dropdown_row(
                text_ui,
                ui,
                setting.id,
                setting.label,
                setting.info_tooltip,
                &mut selected_index,
                &option_label_refs,
            );
            if response.changed() {
                if let Some(next_value) = options.get(selected_index) {
                    *value = next_value.clone();
                }
            }
        });
        ui.add_space(style::SPACE_MD);
    });

    config.for_each_float_mut(|setting, value| {
        if matches!(
            setting.id,
            FloatSettingId::SkinPreviewMotionBlurAmount
                | FloatSettingId::SkinPreviewMotionBlurShutterFrames
                | FloatSettingId::NotificationFadeOutSeconds
        ) {
            return;
        }
        ui.push_id(setting.id, |ui| {
            settings_widgets::float_stepper_row(
                text_ui,
                ui,
                setting.id,
                setting.label,
                setting.info_tooltip,
                value,
                setting.min,
                setting.max,
                setting.step,
            );
        });
        ui.add_space(style::SPACE_MD);
    });

    render_selected_int_settings(ui, text_ui, config, &[IntSettingId::UiFontWeight]);
}

/// The font choices offered by each text group's dropdowns.
pub(super) struct TypographyFonts<'a> {
    pub(super) ui_fonts: &'a [UiFontFamily],
    pub(super) ui_font_labels: &'a [String],
    pub(super) cjk_fonts: &'a [UiCjkFontFamily],
    pub(super) cjk_font_labels: &'a [String],
    pub(super) emoji_fonts: &'a [UiEmojiFontFamily],
    pub(super) emoji_font_labels: &'a [String],
}

/// Index into a "use the general setting" dropdown for `current`: 0 for no choice (or a font that
/// is no longer installed), otherwise one past its position in `family_names`.
fn script_font_index(current: Option<&str>, family_names: &[&str]) -> usize {
    current
        .and_then(|current| {
            family_names
                .iter()
                .position(|name| name.eq_ignore_ascii_case(current))
        })
        .map_or(0, |position| position + 1)
}

/// The family chosen by a "use the general setting" dropdown index; `None` for index 0.
fn script_font_choice(index: usize, family_names: &[&str]) -> Option<String> {
    index
        .checked_sub(1)
        .and_then(|position| family_names.get(position))
        .map(|name| (*name).to_owned())
}

/// Per-role size, weight and font rows. Values equal to the built-in default are stored as
/// "no override", so only changed roles end up in the config file.
pub(super) fn render_typography_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    fonts: &TypographyFonts<'_>,
) {
    let available_ui_fonts = fonts.ui_fonts;
    let available_ui_font_labels = fonts.ui_font_labels;
    let cjk_names: Vec<&str> = fonts
        .cjk_fonts
        .iter()
        .map(UiCjkFontFamily::family_name)
        .collect();
    let emoji_names: Vec<&str> = fonts
        .emoji_fonts
        .iter()
        .map(UiEmojiFontFamily::family_name)
        .collect();
    let mut cjk_labels: Vec<&str> = vec!["Use general CJK font"];
    cjk_labels.extend(fonts.cjk_font_labels.iter().map(String::as_str));
    let mut emoji_labels: Vec<&str> = vec!["Use general emoji font"];
    emoji_labels.extend(fonts.emoji_font_labels.iter().map(String::as_str));

    render_settings_subgroup(
        ui,
        text_ui,
        "Text styles",
        "Size, weight, and font for each kind of text. These stack with the UI font size and weight above.",
    );
    let mut font_labels: Vec<&str> = vec!["Launcher UI font (default)"];
    font_labels.extend(available_ui_font_labels.iter().map(String::as_str));

    for role in config::TextRole::ALL {
        let defaults = role.defaults();
        let current = config.typography().override_for(role);
        let mut size = current.size.unwrap_or(defaults.size);
        let mut weight = current.weight.unwrap_or(defaults.weight);
        let mut font_index = current
            .font_family
            .as_deref()
            .and_then(|name| {
                available_ui_fonts
                    .iter()
                    .position(|font| font.matches_name(name))
                    .map(|index| index + 1)
            })
            .unwrap_or(0);
        let previous_font_index = font_index;
        let mut cjk_index = script_font_index(current.cjk_font_family.as_deref(), &cjk_names);
        let previous_cjk_index = cjk_index;
        let mut emoji_index = script_font_index(current.emoji_font_family.as_deref(), &emoji_names);
        let previous_emoji_index = emoji_index;

        let mut heading_style = style::body(ui);
        heading_style.weight = 700;
        heading_style.wrap = false;
        let _ = text_ui.label(
            ui,
            ("typography_role_heading", role.slug()),
            role.label(),
            &heading_style,
        );
        let mut changed = false;
        ui.push_id(("typography_role", role.slug()), |ui| {
            changed |= settings_widgets::float_stepper_row(
                text_ui,
                ui,
                "size",
                "Size",
                None,
                &mut size,
                config::ROLE_FONT_SIZE_MIN,
                config::ROLE_FONT_SIZE_MAX,
                config::ROLE_FONT_SIZE_STEP,
            )
            .changed();
            changed |= settings_widgets::int_stepper_row(
                text_ui,
                ui,
                "weight",
                "Weight",
                Some("Font weight from 100 (thin) to 900 (black), in steps of 100; 400 is regular and 700 is bold. Fonts that lack the chosen weight may look different. Default: 400."),
                &mut weight,
                config::UI_FONT_WEIGHT_MIN,
                config::UI_FONT_WEIGHT_MAX,
                config::UI_FONT_WEIGHT_STEP,
            )
            .changed();
            changed |= settings_widgets::searchable_dropdown_row(
                text_ui,
                ui,
                "font",
                "Font",
                None,
                &mut font_index,
                &font_labels,
            )
.changed();
            changed |= settings_widgets::searchable_dropdown_row(
                text_ui,
                ui,
                "cjk_font",
                "CJK font",
                Some("Font for Chinese, Japanese and Korean characters in this kind of text. Leave on the general setting to follow the CJK Font choice above."),
                &mut cjk_index,
                &cjk_labels,
            )
            .changed();
            changed |= settings_widgets::searchable_dropdown_row(
                text_ui,
                ui,
                "emoji_font",
                "Emoji font",
                Some("Font for emoji in this kind of text. Leave on the general setting to follow the Emoji Font choice above."),
                &mut emoji_index,
                &emoji_labels,
            )
            .changed();
            if text_ui
                .button(ui, "reset", "Reset to default", &style::neutral_button(ui))
                .clicked()
            {
                config.typography_mut().reset(role);
                changed = false;
            }
        });
        if changed
            || font_index != previous_font_index
            || cjk_index != previous_cjk_index
            || emoji_index != previous_emoji_index
        {
            let font_family = font_index
                .checked_sub(1)
                .and_then(|index| available_ui_fonts.get(index))
                .map(|font| font.label().to_owned());
            config.typography_mut().set_override(
                role,
                config::RoleOverride {
                    size: (size != defaults.size).then_some(size),
                    weight: (weight != defaults.weight).then_some(weight),
                    font_family,
                    // Only follow the dropdown when it moved, so a font that is no longer
                    // installed is kept rather than silently reset by an unrelated edit.
                    cjk_font_family: if cjk_index != previous_cjk_index {
                        script_font_choice(cjk_index, &cjk_names)
                    } else {
                        current.cjk_font_family.clone()
                    },
                    emoji_font_family: if emoji_index != previous_emoji_index {
                        script_font_choice(emoji_index, &emoji_names)
                    } else {
                        current.emoji_font_family.clone()
                    },
                },
            );
        }
        ui.add_space(style::SPACE_MD);
    }
}

pub(super) fn render_emoji_font_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    available_emoji_fonts: &[UiEmojiFontFamily],
    available_emoji_font_labels: &[String],
) {
    if available_emoji_fonts.is_empty() {
        return;
    }
    let current = config.ui_emoji_font_family();
    let selected_option_index = available_emoji_fonts
        .iter()
        .position(|option| option.matches(&current))
        .unwrap_or(0);
    let option_label_refs: Vec<&str> = available_emoji_font_labels
        .iter()
        .map(String::as_str)
        .collect();
    let mut selected_index = selected_option_index;
    let response = settings_widgets::searchable_dropdown_row(
        text_ui,
        ui,
        egui::Id::new("emoji_font_family_dropdown"),
        "Emoji Font",
        Some("Font used for emoji characters. Default: Noto Color Emoji (Included)."),
        &mut selected_index,
        &option_label_refs,
    );
    if response.changed() {
        if let Some(next_value) = available_emoji_fonts.get(selected_index) {
            config.set_ui_emoji_font_family(next_value.clone());
        }
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_cjk_font_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    available_cjk_fonts: &[UiCjkFontFamily],
    available_cjk_font_labels: &[String],
) {
    if available_cjk_fonts.is_empty() {
        return;
    }
    let current = config.ui_cjk_font_family();
    let selected_option_index = available_cjk_fonts
        .iter()
        .position(|option| option.matches(&current))
        .unwrap_or(0);
    let option_label_refs: Vec<&str> = available_cjk_font_labels
        .iter()
        .map(String::as_str)
        .collect();
    let mut selected_index = selected_option_index;
    let response = settings_widgets::searchable_dropdown_row(
        text_ui,
        ui,
        egui::Id::new("cjk_font_family_dropdown"),
        "CJK Font",
        Some(
            "Font used for Chinese, Japanese and Korean characters. Each kind of text below can pick its own. The included Maple Mono NF CN covers Chinese and Japanese kana but not Korean Hangul, so pick another font for Korean text. Default: Maple Mono NF CN (Included).",
        ),
        &mut selected_index,
        &option_label_refs,
    );
    if response.changed() {
        if let Some(next_value) = available_cjk_fonts.get(selected_index) {
            config.set_ui_cjk_font_family(next_value.clone());
        }
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_text_rendering_path_setting(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let options = TextRenderingPath::ALL;
    let option_labels: Vec<&str> = options.iter().map(|option| option.label()).collect();
    let selected_index = options
        .iter()
        .position(|option| *option == config.text_rendering_path())
        .unwrap_or(0);
    let mut selected_index = selected_index;
    let response = settings_widgets::full_width_dropdown_row(
        text_ui,
        ui,
        "text_rendering_path",
        "Text Rendering Path",
        Some(
            "Controls how glyphs are rasterized for the launcher UI. Auto keeps the default path. SDF/MSDF are mainly useful for testing the new text pipeline. Default: Auto.",
        ),
        &mut selected_index,
        &option_labels,
    );
    if response.changed() {
        if let Some(next) = options.get(selected_index).copied() {
            config.set_text_rendering_path(next);
        }
    }
    ui.add_space(style::SPACE_MD);
}
