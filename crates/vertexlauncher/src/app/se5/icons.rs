//! The launcher's Tabler SVG icons on the engine's SVG pipeline. The engine loads SVGs from files and tints them
//! by multiplying, so each icon is written once to the cache folder with `currentColor` set to white; drawing
//! passes the theme colour as the element's tint.

use std::path::PathBuf;

use launcher_ui::assets;
use vertex_shell::sturdy::glam::Vec4;
use vertex_shell::sturdy::ui::{ElementDesc, Sizing, Ui};

macro_rules! icons {
    ($($variant:ident => ($file:literal, $bytes:expr)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Icon { $($variant),+ }

        impl Icon {
            fn file(self) -> &'static str {
                match self { $(Self::$variant => $file),+ }
            }

            fn svg(self) -> &'static [u8] {
                match self { $(Self::$variant => $bytes),+ }
            }
        }
    };
}

icons! {
    AntennaOff => ("antenna-bars-off", assets::ANTENNA_BARS_OFF_SVG),
    Antenna1 => ("antenna-bars-1", assets::ANTENNA_BARS_1_SVG),
    Antenna2 => ("antenna-bars-2", assets::ANTENNA_BARS_2_SVG),
    Antenna3 => ("antenna-bars-3", assets::ANTENNA_BARS_3_SVG),
    Antenna4 => ("antenna-bars-4", assets::ANTENNA_BARS_4_SVG),
    Antenna5 => ("antenna-bars-5", assets::ANTENNA_BARS_5_SVG),
    Check => ("check", assets::CHECK_SVG),
    ChevronDown => ("chevron-down", assets::CHEVRON_DOWN_SVG),
    ChevronRight => ("chevron-right", assets::CHEVRON_RIGHT_SVG),
    ChevronUp => ("chevron-up", assets::CHEVRON_UP_SVG),
    Copy => ("copy", assets::COPY_SVG),
    Discover => ("discover", assets::DISCOVER_SVG),
    Download => ("download", assets::DOWNLOAD_SVG),
    Edit => ("edit", assets::EDIT_SVG),
    Error => ("error", assets::ERROR_SVG),
    Folder => ("folder", assets::FOLDER_SVG),
    Home => ("home", assets::HOME_SVG),
    Info => ("info-circle", assets::INFO_CIRCLE_SVG),
    Legal => ("legal", assets::LEGAL_SVG),
    Library => ("library", assets::LIBRARY_SVG),
    Log => ("log", assets::LOG_SVG),
    Minus => ("minus", assets::MINUS_SVG),
    Play => ("play", assets::PLAY_SVG),
    Plus => ("plus", assets::PLUS_SVG),
    Refresh => ("refresh", assets::REFRESH_SVG),
    Restore => ("restore", assets::RESTORE_SVG),
    Settings => ("settings", assets::SETTINGS_SVG),
    Share => ("share", assets::SHARE_SVG),
    Skins => ("skin_selector", assets::SKIN_SELECTOR_SVG),
    Star => ("star", assets::STAR_SVG),
    Stop => ("stop", assets::STOP_SVG),
    Steam => ("brand-steam", assets::STEAM_SVG),
    Terminal => ("terminal", assets::TERMINAL_SVG),
    Terminal2 => ("terminal-2", assets::TERMINAL_2_SVG),
    Trash => ("trash-x", assets::TRASH_X_SVG),
    User => ("user", assets::USER_SVG),
    UsersGroup => ("users-group", assets::USERS_GROUP_SVG),
    Warn => ("warn", assets::WARN_SVG),
    Close => ("x", assets::X_SVG),
}

fn icon_dir() -> PathBuf {
    app_paths::cache_root().join("icons")
}

/// Writes the white-on-transparent copy of `icon` if it is not there yet (or the bundled icon changed) and
/// returns its path.
fn ensure_file(icon: Icon) -> Option<PathBuf> {
    let path = icon_dir().join(format!("{}.svg", icon.file()));
    let white = String::from_utf8_lossy(icon.svg())
        .replace("currentColor", "#ffffff")
        .into_bytes();
    if std::fs::read(&path).ok().as_deref() != Some(white.as_slice()) {
        std::fs::create_dir_all(icon_dir()).ok()?;
        std::fs::write(&path, &white).ok()?;
    }
    Some(path)
}

/// Draws `icon` `size` pixels square in `color`. `supersample` renders it larger and scales it down, which is what
/// the "Icon anti-aliasing" setting controls.
pub fn draw(ui: &mut Ui<'_>, icon: Icon, size: f32, color: Vec4, supersample: u32) {
    let Some(path) = ensure_file(icon) else {
        return;
    };
    // Rounded up to a multiple of 4: the engine block-compresses these textures, and WebGPU rejects a BC7 texture
    // whose size is not a whole number of 4x4 blocks (Vulkan tolerates it), which left odd-sized icons invisible.
    let target = ((size * supersample.max(1) as f32 / 4.0).ceil() * 4.0).max(4.0);
    let Ok(texture) = ui.load_svg(&path.to_string_lossy(), target) else {
        return;
    };
    ui.svg(
        &ElementDesc::new()
            .width(Sizing::Fixed(size))
            .height(Sizing::Fixed(size))
            .background(color),
        texture,
    );
}
