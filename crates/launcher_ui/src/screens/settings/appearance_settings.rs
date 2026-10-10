use super::*;

pub(super) fn render_notification_easing_setting(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let mut selected = NotificationEasing::ALL
        .iter()
        .position(|easing| *easing == config.notification_easing())
        .unwrap_or(0);
    let labels: Vec<&str> = NotificationEasing::ALL
        .iter()
        .map(|easing| easing.label())
        .collect();

    let response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "notification_easing",
        "Notification Fade Out Easing",
        Some(
            "The curve of the fade-out animation. Linear fades at a constant rate; Cubic and Exponential ease in, out, or both, and the Exponential curves are the most abrupt. Changes apply immediately. Default: Cubic In/Out.",
        ),
        &mut selected,
        &labels,
    );

    if response.changed() {
        if let Some(next) = NotificationEasing::ALL.get(selected).copied() {
            config.set_notification_easing(next);
        }
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_window_transparency_setting(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let setting = DropdownSettingId::WindowTransparency.spec();
    let options = WindowTransparency::ALL;
    let option_labels: Vec<&str> = options.iter().map(|option| option.label()).collect();
    let mut selected_index = options
        .iter()
        .position(|option| *option == config.window_transparency())
        .unwrap_or(0);
    let response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "window_transparency",
        setting.label,
        setting.info_tooltip,
        &mut selected_index,
        &option_labels,
    );
    if response.changed()
        && let Some(next_mode) = options.get(selected_index).copied()
    {
        config.set_window_transparency(next_mode);
        #[cfg(target_os = "windows")]
        if next_mode.is_translucent()
            && matches!(
                config.graphics_api_preference(),
                GraphicsApiPreference::Vulkan
            )
        {
            config.set_graphics_api_preference(GraphicsApiPreference::Dx12);
        }
    }

    #[cfg(target_os = "windows")]
    if config.window_transparency().is_translucent() {
        let _ = text_ui.label(
            ui,
            "window_transparency_windows_graphics_api_note",
            "Non-opaque window transparency uses the DirectX 12 renderer on Windows; Vulkan is unavailable while transparency is active.",
            &style::muted(ui),
        );
    }

    #[cfg(target_os = "macos")]
    if config.window_transparency().uses_native_blur() {
        let _ = text_ui.label(
            ui,
            "window_transparency_macos_blur_note",
            "Native blur is currently downgraded to transparent mode on macOS at startup; macOS blur parameters are still saved in the macOS section.",
            &style::muted(ui),
        );
    }

    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_ui_opacity_setting(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let opacity_active = ui_opacity_setting_active(config);
    let mut opacity_percent = config.ui_opacity_percent() as u128;
    let response = ui
        .add_enabled_ui(opacity_active, |ui| {
            settings_widgets::u128_slider_with_input_row(
                text_ui,
                ui,
                "ui_opacity_percent",
                "UI Opacity",
                Some(
                    "Only active when Window Transparency is Transparent or Blurred. 100% is fully opaque UI chrome and 0% is fully transparent. Default: 100%.",
                ),
                &mut opacity_percent,
                0,
                100,
                1,
            )
        })
        .inner;
    if response.changed() {
        config.set_ui_opacity_percent(opacity_percent as u8);
    }
    ui.add_space(style::SPACE_MD);
}

pub(super) fn ui_opacity_setting_active(config: &Config) -> bool {
    config.window_transparency().is_translucent()
}
