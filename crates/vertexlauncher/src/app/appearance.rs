use config::Config;

pub(super) fn transparent_viewport_enabled(config: &Config) -> bool {
    config.window_transparency().is_translucent()
}

/// The UI opacity in effect: the setting only applies while the window is transparent or blurred.
pub(super) fn effective_ui_opacity_percent(config: &Config) -> u8 {
    if transparent_viewport_enabled(config) {
        config.ui_opacity_percent()
    } else {
        100
    }
}
