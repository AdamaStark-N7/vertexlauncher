use egui::{Color32, Context, CornerRadius, Stroke, Style, Visuals};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_THEME_ID: &str = "high_contrast";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub bg_dark: Oklch,
    pub bg: Oklch,
    pub bg_light: Oklch,
    pub text: Oklch,
    pub text_muted: Oklch,
    pub highlight: Oklch,
    pub border: Oklch,
    pub border_muted: Oklch,
    pub primary: Oklch,
    pub secondary: Oklch,
    pub danger: Oklch,
    pub warning: Oklch,
    pub success: Oklch,
    pub info: Oklch,
    #[serde(default)]
    pub motion: ThemeMotion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeMotion {
    #[serde(default = "default_animation_time_seconds")]
    pub animation_time_seconds: f32,
    #[serde(default)]
    pub reduced_motion: bool,
}

impl Default for ThemeMotion {
    fn default() -> Self {
        Self {
            animation_time_seconds: default_animation_time_seconds(),
            reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThemeCatalog {
    themes: Vec<Theme>,
    theme_labels: Vec<String>,
    fallback: Theme,
}

impl ThemeCatalog {
    pub fn load() -> Self {
        let themes_dir = app_paths::themes_dir();
        ensure_themes_dir_and_defaults(&themes_dir);

        let fallback = Theme::matrix_oled();
        let mut themes = load_themes_from_dir(&themes_dir);
        if app_paths::portable_root().is_none() {
            if let Some(legacy_themes_dir) = app_paths::legacy_themes_dir()
                && legacy_themes_dir != themes_dir
            {
                themes.extend(load_themes_from_dir(&legacy_themes_dir));
            }

            let legacy_themes_dir = PathBuf::from("themes");
            if legacy_themes_dir != themes_dir {
                themes.extend(load_themes_from_dir(&legacy_themes_dir));
            }
        }
        let mut seen = HashSet::new();
        themes.retain(|theme| seen.insert(theme.id.clone()));

        if !themes.iter().any(|theme| theme.id == DEFAULT_THEME_ID) {
            themes.push(fallback.clone());
        }

        themes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        let theme_labels = themes.iter().map(|theme| theme.name.clone()).collect();
        Self {
            themes,
            theme_labels,
            fallback,
        }
    }

    pub fn themes(&self) -> &[Theme] {
        &self.themes
    }

    pub fn theme_labels(&self) -> &[String] {
        &self.theme_labels
    }

    pub fn contains(&self, id: &str) -> bool {
        self.themes.iter().any(|theme| theme.id == id)
    }

    pub fn default_theme_id(&self) -> &str {
        DEFAULT_THEME_ID
    }

    pub fn resolve(&self, id: &str) -> &Theme {
        self.themes
            .iter()
            .find(|theme| theme.id == id)
            .or_else(|| {
                self.themes
                    .iter()
                    .find(|theme| theme.id == DEFAULT_THEME_ID)
            })
            .or_else(|| self.themes.first())
            .unwrap_or(&self.fallback)
    }
}

/// How much of the text color is mixed into a widget's idle fill to get its hovered fill.
const HOVER_FILL_TEXT_MIX: f32 = 0.14;

impl Default for Theme {
    fn default() -> Self {
        Self::matrix_oled()
    }
}

impl Theme {
    pub fn apply(&self, ctx: &Context, ui_opacity_percent: u8) {
        let mut style: Style = (*ctx.global_style()).clone();
        let mut visuals = Visuals::dark();

        let bg_dark = self.bg_dark.to_color32();
        let bg = self.bg.to_color32();
        let bg_light = self.bg_light.to_color32();
        let text = self.text.to_color32();
        let text_muted = self.text_muted.to_color32();
        let highlight = self.highlight.to_color32();
        let border = self.border.to_color32();
        let border_muted = self.border_muted.to_color32();

        let alpha_profile = ThemeAlphaProfile::for_ui_opacity(ui_opacity_percent);

        visuals.window_fill = bg_dark;
        visuals.panel_fill = with_alpha(bg, alpha_profile.panel_fill);
        visuals.faint_bg_color = with_alpha(bg_light, alpha_profile.faint_bg);
        visuals.extreme_bg_color = with_alpha(bg_dark, alpha_profile.extreme_bg);
        visuals.code_bg_color = with_alpha(bg_dark, alpha_profile.code_bg);
        visuals.override_text_color = Some(text);
        visuals.weak_text_color = Some(text_muted);
        visuals.window_stroke = Stroke::new(1.0, border_muted);
        visuals.widgets.noninteractive.bg_fill = bg;
        visuals.widgets.noninteractive.weak_bg_fill = bg;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_muted);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border_muted);

        visuals.widgets.inactive.bg_fill = bg_light;
        visuals.widgets.inactive.weak_bg_fill = bg_light;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border_muted);

        // Hover must be visibly different from idle (see `textui_egui::interaction`).
        let bg_hovered = bg_light.lerp_to_gamma(text, HOVER_FILL_TEXT_MIX);
        visuals.widgets.hovered.bg_fill = bg_hovered;
        visuals.widgets.hovered.weak_bg_fill = bg_hovered;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, border);

        visuals.widgets.active.bg_fill = highlight;
        visuals.widgets.active.weak_bg_fill = highlight;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, border);

        visuals.widgets.open.bg_fill = bg_hovered;
        visuals.widgets.open.weak_bg_fill = bg_hovered;
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, border);

        visuals.selection.bg_fill = self.primary.to_color32();
        visuals.selection.stroke = Stroke::new(1.0, border);
        visuals.hyperlink_color = self.info.to_color32();
        visuals.warn_fg_color = self.warning.to_color32();
        visuals.error_fg_color = self.danger.to_color32();
        visuals.text_cursor.stroke = Stroke::new(2.0, self.success.to_color32());

        let rounding = CornerRadius::same(14);
        visuals.window_corner_radius = rounding;
        visuals.menu_corner_radius = rounding;
        visuals.widgets.noninteractive.corner_radius = rounding;
        visuals.widgets.inactive.corner_radius = rounding;
        visuals.widgets.hovered.corner_radius = rounding;
        visuals.widgets.active.corner_radius = rounding;
        visuals.widgets.open.corner_radius = rounding;

        style.animation_time = if self.motion.reduced_motion {
            0.0
        } else {
            self.motion.animation_time_seconds.clamp(0.0, 1.0)
        };
        style.visuals = visuals;
        ctx.set_global_style(style);
    }

    fn matrix_oled() -> Self {
        Self {
            id: "matrix_oled".to_owned(),
            name: "Matrix OLED".to_owned(),
            description: "Original dark matrix-inspired theme.".to_owned(),
            bg_dark: Oklch {
                l: 0.10,
                c: 0.0,
                h: 316.0,
            },
            bg: Oklch {
                l: 0.15,
                c: 0.0,
                h: 316.0,
            },
            bg_light: Oklch {
                l: 0.20,
                c: 0.0,
                h: 316.0,
            },
            text: Oklch {
                l: 0.96,
                c: 0.0,
                h: 316.0,
            },
            text_muted: Oklch {
                l: 0.76,
                c: 0.0,
                h: 316.0,
            },
            highlight: Oklch {
                l: 0.50,
                c: 0.0,
                h: 316.0,
            },
            border: Oklch {
                l: 0.40,
                c: 0.0,
                h: 316.0,
            },
            border_muted: Oklch {
                l: 0.30,
                c: 0.0,
                h: 316.0,
            },
            primary: Oklch {
                l: 0.76,
                c: 0.2,
                h: 150.0,
            },
            secondary: Oklch {
                l: 0.76,
                c: 0.2,
                h: 330.0,
            },
            danger: Oklch {
                l: 0.70,
                c: 0.2,
                h: 30.0,
            },
            warning: Oklch {
                l: 0.70,
                c: 0.2,
                h: 100.0,
            },
            success: Oklch {
                l: 0.70,
                c: 0.2,
                h: 160.0,
            },
            info: Oklch {
                l: 0.70,
                c: 0.2,
                h: 260.0,
            },
            motion: ThemeMotion::default(),
        }
    }

    fn default_light() -> Self {
        Self {
            id: "default_light".to_owned(),
            name: "Default Light".to_owned(),
            description: "Light theme from RustServerController.".to_owned(),
            bg_dark: Oklch {
                l: 0.92,
                c: 0.055,
                h: 264.0,
            },
            bg: Oklch {
                l: 0.96,
                c: 0.055,
                h: 264.0,
            },
            bg_light: Oklch {
                l: 1.00,
                c: 0.055,
                h: 264.0,
            },
            text: Oklch {
                l: 0.15,
                c: 0.11,
                h: 264.0,
            },
            text_muted: Oklch {
                l: 0.40,
                c: 0.11,
                h: 264.0,
            },
            highlight: Oklch {
                l: 1.00,
                c: 0.11,
                h: 264.0,
            },
            border: Oklch {
                l: 0.60,
                c: 0.11,
                h: 264.0,
            },
            border_muted: Oklch {
                l: 0.70,
                c: 0.11,
                h: 264.0,
            },
            primary: Oklch {
                l: 0.40,
                c: 0.20,
                h: 264.0,
            },
            secondary: Oklch {
                l: 0.40,
                c: 0.20,
                h: 84.0,
            },
            danger: Oklch {
                l: 0.50,
                c: 0.20,
                h: 30.0,
            },
            warning: Oklch {
                l: 0.50,
                c: 0.20,
                h: 100.0,
            },
            success: Oklch {
                l: 0.50,
                c: 0.20,
                h: 160.0,
            },
            info: Oklch {
                l: 0.50,
                c: 0.20,
                h: 260.0,
            },
            motion: ThemeMotion::default(),
        }
    }

    fn high_contrast() -> Self {
        Self {
            id: "high_contrast".to_owned(),
            name: "High Contrast".to_owned(),
            description: "Accessibility-focused high-contrast theme from RustServerController."
                .to_owned(),
            bg_dark: Oklch {
                l: 0.05,
                c: 0.01,
                h: 256.0,
            },
            bg: Oklch {
                l: 0.08,
                c: 0.01,
                h: 256.0,
            },
            bg_light: Oklch {
                l: 0.12,
                c: 0.01,
                h: 256.0,
            },
            text: Oklch {
                l: 0.99,
                c: 0.03,
                h: 256.0,
            },
            text_muted: Oklch {
                l: 0.90,
                c: 0.03,
                h: 256.0,
            },
            highlight: Oklch {
                l: 0.70,
                c: 0.03,
                h: 256.0,
            },
            border: Oklch {
                l: 0.60,
                c: 0.03,
                h: 256.0,
            },
            border_muted: Oklch {
                l: 0.50,
                c: 0.03,
                h: 256.0,
            },
            primary: Oklch {
                l: 0.85,
                c: 0.25,
                h: 256.0,
            },
            secondary: Oklch {
                l: 0.85,
                c: 0.25,
                h: 76.0,
            },
            danger: Oklch {
                l: 0.80,
                c: 0.25,
                h: 30.0,
            },
            warning: Oklch {
                l: 0.80,
                c: 0.25,
                h: 100.0,
            },
            success: Oklch {
                l: 0.80,
                c: 0.25,
                h: 160.0,
            },
            info: Oklch {
                l: 0.80,
                c: 0.25,
                h: 260.0,
            },
            motion: ThemeMotion::default(),
        }
    }

    fn purple_dream() -> Self {
        Self {
            id: "purple_dream".to_owned(),
            name: "Purple Dream".to_owned(),
            description: "Purple theme from RustServerController.".to_owned(),
            bg_dark: Oklch {
                l: 0.10,
                c: 0.05,
                h: 290.0,
            },
            bg: Oklch {
                l: 0.15,
                c: 0.05,
                h: 290.0,
            },
            bg_light: Oklch {
                l: 0.20,
                c: 0.05,
                h: 290.0,
            },
            text: Oklch {
                l: 0.96,
                c: 0.02,
                h: 290.0,
            },
            text_muted: Oklch {
                l: 0.76,
                c: 0.02,
                h: 290.0,
            },
            highlight: Oklch {
                l: 0.70,
                c: 0.20,
                h: 290.0,
            },
            border: Oklch {
                l: 0.40,
                c: 0.02,
                h: 256.0,
            },
            border_muted: Oklch {
                l: 0.30,
                c: 0.02,
                h: 256.0,
            },
            primary: Oklch {
                l: 0.70,
                c: 0.25,
                h: 290.0,
            },
            secondary: Oklch {
                l: 0.70,
                c: 0.20,
                h: 250.0,
            },
            danger: Oklch {
                l: 0.70,
                c: 0.25,
                h: 30.0,
            },
            warning: Oklch {
                l: 0.70,
                c: 0.20,
                h: 80.0,
            },
            success: Oklch {
                l: 0.70,
                c: 0.20,
                h: 140.0,
            },
            info: Oklch {
                l: 0.70,
                c: 0.20,
                h: 210.0,
            },
            motion: ThemeMotion::default(),
        }
    }
}

/// The Catppuccin colors a launcher theme is built from. Every flavor shares the same roles, so
/// one mapping (see [`Theme::from_catppuccin`]) covers all four.
struct CatppuccinPalette {
    mantle: u32,
    base: u32,
    surface0: u32,
    surface1: u32,
    surface2: u32,
    overlay0: u32,
    text: u32,
    subtext0: u32,
    mauve: u32,
    blue: u32,
    red: u32,
    yellow: u32,
    green: u32,
    sky: u32,
}

impl Theme {
    fn from_catppuccin(id: &str, flavor: &str, palette: &CatppuccinPalette) -> Self {
        Self {
            id: id.to_owned(),
            name: format!("Catppuccin {flavor}"),
            description: format!("The {flavor} flavor of the Catppuccin palette."),
            bg_dark: Oklch::from_hex(palette.mantle),
            bg: Oklch::from_hex(palette.base),
            bg_light: Oklch::from_hex(palette.surface0),
            text: Oklch::from_hex(palette.text),
            text_muted: Oklch::from_hex(palette.subtext0),
            highlight: Oklch::from_hex(palette.surface2),
            border: Oklch::from_hex(palette.overlay0),
            border_muted: Oklch::from_hex(palette.surface1),
            primary: Oklch::from_hex(palette.mauve),
            secondary: Oklch::from_hex(palette.blue),
            danger: Oklch::from_hex(palette.red),
            warning: Oklch::from_hex(palette.yellow),
            success: Oklch::from_hex(palette.green),
            info: Oklch::from_hex(palette.sky),
            motion: ThemeMotion::default(),
        }
    }

    fn catppuccin_latte() -> Self {
        Self::from_catppuccin(
            "catppuccin_latte",
            "Latte",
            &CatppuccinPalette {
                mantle: 0xe6e9ef,
                base: 0xeff1f5,
                surface0: 0xccd0da,
                surface1: 0xbcc0cc,
                surface2: 0xacb0be,
                overlay0: 0x9ca0b0,
                text: 0x4c4f69,
                subtext0: 0x6c6f85,
                mauve: 0x8839ef,
                blue: 0x1e66f5,
                red: 0xd20f39,
                yellow: 0xdf8e1d,
                green: 0x40a02b,
                sky: 0x04a5e5,
            },
        )
    }

    fn catppuccin_frappe() -> Self {
        Self::from_catppuccin(
            "catppuccin_frappe",
            "Frappé",
            &CatppuccinPalette {
                mantle: 0x292c3c,
                base: 0x303446,
                surface0: 0x414559,
                surface1: 0x51576d,
                surface2: 0x626880,
                overlay0: 0x737994,
                text: 0xc6d0f5,
                subtext0: 0xa5adce,
                mauve: 0xca9ee6,
                blue: 0x8caaee,
                red: 0xe78284,
                yellow: 0xe5c890,
                green: 0xa6d189,
                sky: 0x99d1db,
            },
        )
    }

    fn catppuccin_macchiato() -> Self {
        Self::from_catppuccin(
            "catppuccin_macchiato",
            "Macchiato",
            &CatppuccinPalette {
                mantle: 0x1e2030,
                base: 0x24273a,
                surface0: 0x363a4f,
                surface1: 0x494d64,
                surface2: 0x5b6078,
                overlay0: 0x6e738d,
                text: 0xcad3f5,
                subtext0: 0xa5adcb,
                mauve: 0xc6a0f6,
                blue: 0x8aadf4,
                red: 0xed8796,
                yellow: 0xeed49f,
                green: 0xa6da95,
                sky: 0x91d7e3,
            },
        )
    }

    fn catppuccin_mocha() -> Self {
        Self::from_catppuccin(
            "catppuccin_mocha",
            "Mocha",
            &CatppuccinPalette {
                mantle: 0x181825,
                base: 0x1e1e2e,
                surface0: 0x313244,
                surface1: 0x45475a,
                surface2: 0x585b70,
                overlay0: 0x6c7086,
                text: 0xcdd6f4,
                subtext0: 0xa6adc8,
                mauve: 0xcba6f7,
                blue: 0x89b4fa,
                red: 0xf38ba8,
                yellow: 0xf9e2af,
                green: 0xa6e3a1,
                sky: 0x89dceb,
            },
        )
    }

    /// Serializes this theme as pretty JSON, a starting point for writing a new theme.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Writes this theme as `<id>.json` into the themes directory and returns the file path.
    ///
    /// Theme files in that directory are loaded at startup, so editing the exported file (and
    /// giving it a new `id`) is enough to add a theme of your own.
    pub fn export_json_to_themes_dir(&self) -> Result<PathBuf, String> {
        let dir = app_paths::themes_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        let file_name: String = self
            .id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = dir.join(format!("{file_name}.json"));
        let json = self.to_json().map_err(|err| err.to_string())?;
        std::fs::write(&path, json)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
        Ok(path)
    }
}

impl Oklch {
    /// Converts a `0xRRGGBB` sRGB color to Oklch.
    pub fn from_hex(rgb: u32) -> Self {
        let channel = |shift: u32| srgb_to_linear(((rgb >> shift) & 0xff) as f32 / 255.0);
        let (r, g, b) = (channel(16), channel(8), channel(0));

        let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();

        let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
        let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
        let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;

        Self {
            l: lightness,
            c: a.hypot(b),
            h: b.atan2(a).to_degrees().rem_euclid(360.0),
        }
    }
}

impl Oklch {
    pub fn to_color32(self) -> Color32 {
        let h_rad = self.h.to_radians();
        let a = self.c * h_rad.cos();
        let b = self.c * h_rad.sin();

        let l_ = self.l + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = self.l - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = self.l - 0.089_484_18 * a - 1.291_485_5 * b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        let r_lin = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
        let g_lin = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s;
        let b_lin = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;

        Color32::from_rgb(
            linear_to_srgb_u8(r_lin),
            linear_to_srgb_u8(g_lin),
            linear_to_srgb_u8(b_lin),
        )
    }
}

fn ensure_themes_dir_and_defaults(dir: &Path) {
    tracing::debug!(target: "vertexlauncher/io", op = "create_dir_all", path = %dir.display(), context = "ensure themes dir");
    if let Err(err) = std::fs::create_dir_all(dir) {
        tracing::warn!(target: "vertexlauncher/io", op = "create_dir_all", path = %dir.display(), error = %err, context = "ensure themes dir");
        return;
    }

    for theme in [
        Theme::matrix_oled(),
        Theme::default_light(),
        Theme::high_contrast(),
        Theme::purple_dream(),
        Theme::catppuccin_latte(),
        Theme::catppuccin_frappe(),
        Theme::catppuccin_macchiato(),
        Theme::catppuccin_mocha(),
    ] {
        let path = dir.join(format!("{}.toml", theme.id));
        if path.exists() {
            continue;
        }

        if let Ok(contents) = toml::to_string_pretty(&theme) {
            tracing::debug!(target: "vertexlauncher/io", op = "write", path = %path.display(), context = "write default theme");
            if let Err(err) = std::fs::write(&path, contents) {
                tracing::warn!(target: "vertexlauncher/io", op = "write", path = %path.display(), error = %err, context = "write default theme");
            }
        }
    }
}

fn load_themes_from_dir(dir: &Path) -> Vec<Theme> {
    tracing::debug!(target: "vertexlauncher/io", op = "read_dir", path = %dir.display(), context = "load themes");
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(target: "vertexlauncher/io", op = "read_dir", path = %dir.display(), error = %err, context = "load themes");
            return Vec::new();
        }
    };

    let mut themes = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if ext != "toml" && ext != "json" {
            continue;
        }

        tracing::debug!(target: "vertexlauncher/io", op = "read_to_string", path = %path.display(), context = "load theme file");
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                tracing::warn!(target: "vertexlauncher/io", op = "read_to_string", path = %path.display(), error = %err, context = "load theme file");
                continue;
            }
        };
        let parsed = if ext == "json" {
            serde_json::from_str::<Theme>(&contents).ok()
        } else {
            toml::from_str::<Theme>(&contents).ok()
        };
        let Some(mut theme) = parsed else {
            continue;
        };

        let file_stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(DEFAULT_THEME_ID);
        if theme.id.trim().is_empty() {
            theme.id = file_stem.to_owned();
        }
        if theme.name.trim().is_empty() {
            theme.name = file_stem.replace('_', " ");
        }

        themes.push(theme);
    }

    themes
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(value: f32) -> u8 {
    let clamped = value.clamp(0.0, 1.0);
    let srgb = if clamped <= 0.0031308 {
        12.92 * clamped
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn default_animation_time_seconds() -> f32 {
    0.15
}

#[derive(Clone, Copy)]
struct ThemeAlphaProfile {
    panel_fill: u8,
    faint_bg: u8,
    extreme_bg: u8,
    code_bg: u8,
}

impl ThemeAlphaProfile {
    const OPAQUE: Self = Self {
        panel_fill: 255,
        faint_bg: 255,
        extreme_bg: 255,
        code_bg: 255,
    };

    fn for_ui_opacity(ui_opacity_percent: u8) -> Self {
        Self::OPAQUE.scaled(ui_opacity_percent)
    }

    fn scaled(self, ui_opacity_percent: u8) -> Self {
        Self {
            panel_fill: scale_alpha(self.panel_fill, ui_opacity_percent),
            faint_bg: scale_alpha(self.faint_bg, ui_opacity_percent),
            extreme_bg: scale_alpha(self.extreme_bg, ui_opacity_percent),
            code_bg: scale_alpha(self.code_bg, ui_opacity_percent),
        }
    }
}

fn scale_alpha(alpha: u8, ui_opacity_percent: u8) -> u8 {
    ((alpha as u32).saturating_mul(ui_opacity_percent as u32) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_oklch_round_trips_through_color32() {
        for hex in [
            0x1e1e2e_u32,
            0xcdd6f4,
            0xf38ba8,
            0xeff1f5,
            0x8839ef,
            0x000000,
            0xffffff,
        ] {
            let color = Oklch::from_hex(hex).to_color32();
            let expected = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
            for (got, want) in [color.r(), color.g(), color.b()].into_iter().zip(expected) {
                assert!(got.abs_diff(want) <= 1, "{hex:06x}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn exported_json_loads_back_as_the_same_theme() {
        let theme = Theme::catppuccin_mocha();
        let json = theme.to_json().unwrap();
        let loaded: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, theme.id);
        assert_eq!(loaded.text.to_color32(), theme.text.to_color32());
    }
}
