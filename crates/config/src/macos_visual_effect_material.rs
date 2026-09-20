use serde::{Deserialize, Serialize};

/// NSVisualEffectView material selection for macOS blur/backdrop effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacosVisualEffectMaterial {
    AppearanceBased,
    Light,
    Dark,
    Titlebar,
    Selection,
    Menu,
    Popover,
    Sidebar,
    HeaderView,
    Sheet,
    WindowBackground,
    HudWindow,
    FullScreenUi,
    Tooltip,
    ContentBackground,
    UnderWindowBackground,
    UnderPageBackground,
}

impl MacosVisualEffectMaterial {
    pub const ALL: [MacosVisualEffectMaterial; 17] = [
        MacosVisualEffectMaterial::AppearanceBased,
        MacosVisualEffectMaterial::Light,
        MacosVisualEffectMaterial::Dark,
        MacosVisualEffectMaterial::Titlebar,
        MacosVisualEffectMaterial::Selection,
        MacosVisualEffectMaterial::Menu,
        MacosVisualEffectMaterial::Popover,
        MacosVisualEffectMaterial::Sidebar,
        MacosVisualEffectMaterial::HeaderView,
        MacosVisualEffectMaterial::Sheet,
        MacosVisualEffectMaterial::WindowBackground,
        MacosVisualEffectMaterial::HudWindow,
        MacosVisualEffectMaterial::FullScreenUi,
        MacosVisualEffectMaterial::Tooltip,
        MacosVisualEffectMaterial::ContentBackground,
        MacosVisualEffectMaterial::UnderWindowBackground,
        MacosVisualEffectMaterial::UnderPageBackground,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            MacosVisualEffectMaterial::AppearanceBased => "Appearance Based",
            MacosVisualEffectMaterial::Light => "Light",
            MacosVisualEffectMaterial::Dark => "Dark",
            MacosVisualEffectMaterial::Titlebar => "Titlebar",
            MacosVisualEffectMaterial::Selection => "Selection",
            MacosVisualEffectMaterial::Menu => "Menu",
            MacosVisualEffectMaterial::Popover => "Popover",
            MacosVisualEffectMaterial::Sidebar => "Sidebar",
            MacosVisualEffectMaterial::HeaderView => "Header View",
            MacosVisualEffectMaterial::Sheet => "Sheet",
            MacosVisualEffectMaterial::WindowBackground => "Window Background",
            MacosVisualEffectMaterial::HudWindow => "HUD Window",
            MacosVisualEffectMaterial::FullScreenUi => "Full Screen UI",
            MacosVisualEffectMaterial::Tooltip => "Tooltip",
            MacosVisualEffectMaterial::ContentBackground => "Content Background",
            MacosVisualEffectMaterial::UnderWindowBackground => "Under Window Background",
            MacosVisualEffectMaterial::UnderPageBackground => "Under Page Background",
        }
    }
}
