use serde::{Deserialize, Serialize};

/// App-wide window translucency mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowTransparency {
    /// Fully opaque window with no transparent viewport or native blur.
    Opaque,
    /// Alpha-capable transparent viewport without requesting compositor blur.
    Transparent,
    /// Alpha-capable transparent viewport plus platform blur/backdrop where supported.
    Blurred,
}

impl WindowTransparency {
    pub const ALL: [WindowTransparency; 3] = [
        WindowTransparency::Opaque,
        WindowTransparency::Transparent,
        WindowTransparency::Blurred,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            WindowTransparency::Opaque => "Opaque",
            WindowTransparency::Transparent => "Transparent",
            WindowTransparency::Blurred => "Blurred",
        }
    }

    pub const fn is_translucent(self) -> bool {
        !matches!(self, WindowTransparency::Opaque)
    }

    pub const fn uses_native_blur(self) -> bool {
        matches!(self, WindowTransparency::Blurred)
    }
}
