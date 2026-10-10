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
    ui.add_space(style::SPACE_MD);
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

/// Per-role size, weight and font rows. Values equal to the built-in default are stored as
/// "no override", so only changed roles end up in the config file.
pub(super) fn render_typography_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    available_ui_fonts: &[UiFontFamily],
    available_ui_font_labels: &[String],
) {
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
            if text_ui
                .button(ui, "reset", "Reset to default", &style::neutral_button(ui))
                .clicked()
            {
                config.typography_mut().reset(role);
                changed = false;
            }
        });
        if changed || font_index != previous_font_index {
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
