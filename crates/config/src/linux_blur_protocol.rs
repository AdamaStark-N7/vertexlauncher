use serde::{Deserialize, Serialize};

/// Linux compositor blur protocol preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxBlurProtocol {
    /// Prefer ext_background_effect_v1 blur support, falling back to KDE blur.
    Auto,
    /// Request blur through the standard Wayland ext_background_effect_v1 protocol.
    ExtBackgroundEffect,
    /// Request blur through KDE/KWin blur protocols or X11 blur hints.
    KdeBlur,
}

impl LinuxBlurProtocol {
    pub const ALL: [LinuxBlurProtocol; 3] = [
        LinuxBlurProtocol::Auto,
        LinuxBlurProtocol::ExtBackgroundEffect,
        LinuxBlurProtocol::KdeBlur,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            LinuxBlurProtocol::Auto => "Auto",
            LinuxBlurProtocol::ExtBackgroundEffect => "ext_background_effect_v1",
            LinuxBlurProtocol::KdeBlur => "KDE Blur",
        }
    }
}
