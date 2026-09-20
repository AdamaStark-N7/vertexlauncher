//! Every known Java Edition `options.txt` key, with type, range, default and the versions
//! it exists in. Sourced from the Minecraft Wiki's options.txt page and checked against real
//! 26.3 option files; unknown keys (mods, future versions) are handled generically.

use crate::version::Ver;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Video,
    Audio,
    Controls,
    Keybinds,
    Chat,
    Accessibility,
    Skin,
    Language,
    Online,
    /// Bookkeeping the game manages itself (shown only under Advanced).
    Internal,
}

impl Group {
    pub const ALL: [Group; 10] = [
        Group::Video,
        Group::Audio,
        Group::Controls,
        Group::Keybinds,
        Group::Chat,
        Group::Accessibility,
        Group::Skin,
        Group::Language,
        Group::Online,
        Group::Internal,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Group::Video => "Video",
            Group::Audio => "Music & Sounds",
            Group::Controls => "Controls",
            Group::Keybinds => "Key Binds",
            Group::Chat => "Chat",
            Group::Accessibility => "Accessibility",
            Group::Skin => "Skin",
            Group::Language => "Language & Fonts",
            Group::Online => "Online & Misc",
            Group::Internal => "Advanced",
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Group::Video => "video",
            Group::Audio => "audio",
            Group::Controls => "controls",
            Group::Keybinds => "keybinds",
            Group::Chat => "chat",
            Group::Accessibility => "accessibility",
            Group::Skin => "skin",
            Group::Language => "language",
            Group::Online => "online",
            Group::Internal => "internal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    Float {
        min: f64,
        max: f64,
    },
    /// String enum: `(stored value, label)`.
    Choice(&'static [(&'static str, &'static str)]),
    /// Integer enum: `(stored value, label)`.
    IntChoice(&'static [(i64, &'static str)]),
    Text,
    /// `["a","b"]` list.
    List,
    /// Key or mouse button, in the version's own format.
    Keybind,
}

/// How a numeric value is shown to the user.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Plain,
    /// Shown as `value * 100` percent.
    Percent,
    /// Shown as `offset + value * scale` degrees (FOV is stored normalized).
    Degrees {
        offset: f64,
        scale: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct OptionDef {
    pub key: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub kind: Kind,
    pub default: &'static str,
    /// First version with this key/format (inclusive).
    pub since: Ver,
    /// First version without it (exclusive).
    pub until: Ver,
    /// String values are wrapped in quotes in this version's file format.
    pub quoted: bool,
    pub display: Display,
    /// Tied to this machine or session; never synced between instances.
    pub machine_specific: bool,
    /// Bookkeeping the game or launcher manages; never shown in the settings UI. The value is
    /// still preserved in the file.
    pub hidden: bool,
    pub note: &'static str,
}

impl OptionDef {
    const fn new(
        key: &'static str,
        label: &'static str,
        group: Group,
        kind: Kind,
        default: &'static str,
    ) -> Self {
        Self {
            key,
            label,
            group,
            kind,
            default,
            since: Ver::OLDEST,
            until: Ver::NEWEST,
            quoted: false,
            display: Display::Plain,
            machine_specific: false,
            hidden: false,
            note: "",
        }
    }

    const fn since(mut self, major: u16, minor: u16, patch: u16) -> Self {
        self.since = Ver(major, minor, patch);
        self
    }

    /// Exclusive upper bound.
    const fn until(mut self, major: u16, minor: u16, patch: u16) -> Self {
        self.until = Ver(major, minor, patch);
        self
    }

    const fn quoted(mut self) -> Self {
        self.quoted = true;
        self
    }

    const fn display(mut self, display: Display) -> Self {
        self.display = display;
        self
    }

    const fn machine(mut self) -> Self {
        self.machine_specific = true;
        self
    }

    const fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    const fn note(mut self, note: &'static str) -> Self {
        self.note = note;
        self
    }

    pub fn applies_to(&self, version: Option<Ver>) -> bool {
        match version {
            Some(v) => v >= self.since && v < self.until,
            // Unknown version: consider only what is current.
            None => self.until == Ver::NEWEST,
        }
    }
}

use Display::Percent;
use Group::*;

const OFF_ON_NARRATOR: &[(i64, &str)] = &[(0, "Off"), (1, "All"), (2, "Chat"), (3, "System")];
const PARTICLES: &[(i64, &str)] = &[(0, "All"), (1, "Decreased"), (2, "Minimal")];
const CHAT_VISIBILITY: &[(i64, &str)] = &[(0, "Shown"), (1, "Commands Only"), (2, "Hidden")];
const ATTACK_INDICATOR: &[(i64, &str)] = &[(0, "Off"), (1, "Crosshair"), (2, "Hotbar")];
const GRAPHICS_MODE: &[(i64, &str)] = &[(0, "Fast"), (1, "Fancy"), (2, "Fabulous!")];
const GRAPHICS_PRESET: &[(&str, &str)] = &[
    ("fast", "Fast"),
    ("fancy", "Fancy"),
    ("fabulous", "Fabulous!"),
    ("custom", "Custom"),
];
const CLOUDS: &[(&str, &str)] = &[("false", "Off"), ("fast", "Fast"), ("true", "Fancy")];
const AO_LEVELS: &[(i64, &str)] = &[(0, "Off"), (1, "Minimum"), (2, "Maximum")];
const CHUNK_UPDATES: &[(i64, &str)] = &[(0, "None"), (1, "Player Affected"), (2, "Nearby")];
const HAND: &[(&str, &str)] = &[("right", "Right"), ("left", "Left")];
const BACKENDS: &[(&str, &str)] = &[
    ("default", "Default"),
    ("opengl", "OpenGL"),
    ("vulkan", "Vulkan"),
];
const INACTIVITY: &[(&str, &str)] = &[("afk", "When AFK"), ("minimized", "When Minimized")];
const MUSIC_TOAST: &[(&str, &str)] = &[
    ("never", "Never"),
    ("pause", "On Pause"),
    ("pause_and_toast", "Pause and Toast"),
];
const MUSIC_FREQUENCY: &[(&str, &str)] = &[
    ("DEFAULT", "Default"),
    ("FREQUENT", "Frequent"),
    ("CONSTANT", "Constant"),
];
const TUTORIAL: &[(&str, &str)] = &[
    ("movement", "Movement"),
    ("find_tree", "Find Tree"),
    ("punch_tree", "Punch Tree"),
    ("open_inventory", "Open Inventory"),
    ("craft_planks", "Craft Planks"),
    ("none", "None"),
];
const SHARE_PRESENCE: &[(&str, &str)] = &[("all", "All"), ("limited", "Limited"), ("none", "None")];
const DIFFICULTY: &[(i64, &str)] = &[(0, "Peaceful"), (1, "Easy"), (2, "Normal"), (3, "Hard")];
const FPS_LIMIT_LEGACY: &[(i64, &str)] = &[(0, "Max"), (1, "Balanced"), (2, "Power Saver")];
const VIEW_DISTANCE_LEGACY: &[(i64, &str)] =
    &[(0, "Far"), (1, "Normal"), (2, "Short"), (3, "Tiny")];

const fn unit_float() -> Kind {
    Kind::Float { min: 0.0, max: 1.0 }
}

const fn toggle(
    key: &'static str,
    label: &'static str,
    group: Group,
    default: &'static str,
) -> OptionDef {
    OptionDef::new(key, label, group, Kind::Bool, default)
}

const fn slider(
    key: &'static str,
    label: &'static str,
    group: Group,
    default: &'static str,
) -> OptionDef {
    OptionDef::new(key, label, group, unit_float(), default).display(Percent)
}

const fn sound(key: &'static str, label: &'static str) -> OptionDef {
    slider(key, label, Audio, "1.0").since(1, 7, 2)
}

const fn part(key: &'static str, label: &'static str) -> OptionDef {
    toggle(key, label, Skin, "true").since(1, 8, 0)
}

const fn bind(key: &'static str, label: &'static str, default: &'static str) -> OptionDef {
    OptionDef::new(key, label, Keybinds, Kind::Keybind, default)
}

/// Every known key. The same key may appear more than once when its type changed.
pub static OPTIONS: &[OptionDef] = &[
    // ── Video ──────────────────────────────────────────────────────────────────────────
    OptionDef::new(
        "ao",
        "Smooth Lighting",
        Video,
        Kind::IntChoice(AO_LEVELS),
        "2",
    )
    .until(1, 14, 0),
    toggle("ao", "Smooth Lighting", Video, "true").since(1, 14, 0),
    OptionDef::new(
        "biomeBlendRadius",
        "Biome Blend",
        Video,
        Kind::Int { min: 0, max: 7 },
        "2",
    )
    .since(1, 15, 0),
    OptionDef::new(
        "chunkSectionFadeInTime",
        "Chunk Fade-In Time",
        Video,
        Kind::Float { min: 0.0, max: 2.0 },
        "0.75",
    )
    .since(1, 21, 11),
    toggle("cutoutLeaves", "Cutout Leaves", Video, "true").since(1, 21, 11),
    toggle("enableVsync", "VSync", Video, "true").since(1, 3, 1),
    OptionDef::new(
        "entityDistanceScaling",
        "Entity Distance",
        Video,
        Kind::Float { min: 0.5, max: 5.0 },
        "1.0",
    )
    .since(1, 16, 0)
    .display(Percent),
    toggle("entityShadows", "Entity Shadows", Video, "true").since(1, 8, 1),
    OptionDef::new(
        "fov",
        "Field of View",
        Video,
        Kind::Float {
            min: -1.0,
            max: 1.0,
        },
        "0.0",
    )
    .display(Display::Degrees {
        offset: 70.0,
        scale: 40.0,
    })
    .note("Stored normalized: 0.0 is 70°."),
    slider("fovEffectScale", "FOV Effects", Accessibility, "1.0").since(1, 16, 2),
    slider(
        "screenEffectScale",
        "Distortion Effects",
        Accessibility,
        "1.0",
    )
    .since(1, 16, 2),
    slider(
        "darknessEffectScale",
        "Darkness Pulsing",
        Accessibility,
        "1.0",
    )
    .since(1, 19, 0),
    slider("glintSpeed", "Glint Speed", Video, "0.5").since(1, 19, 4),
    slider("glintStrength", "Glint Strength", Video, "0.75").since(1, 19, 4),
    OptionDef::new(
        "preferredGraphicsBackend",
        "Graphics Backend",
        Video,
        Kind::Choice(BACKENDS),
        "default",
    )
    .since(26, 1, 0)
    .quoted()
    .machine(),
    toggle("fancyGraphics", "Fancy Graphics", Video, "true").until(1, 16, 0),
    OptionDef::new(
        "graphicsMode",
        "Graphics",
        Video,
        Kind::IntChoice(GRAPHICS_MODE),
        "1",
    )
    .since(1, 16, 0)
    .until(1, 21, 11),
    OptionDef::new(
        "graphicsPreset",
        "Graphics Preset",
        Video,
        Kind::Choice(GRAPHICS_PRESET),
        "fancy",
    )
    .since(1, 21, 11)
    .quoted(),
    OptionDef::new(
        "prioritizeChunkUpdates",
        "Chunk Builder",
        Video,
        Kind::IntChoice(CHUNK_UPDATES),
        "0",
    )
    .since(1, 18, 0),
    toggle("fullscreen", "Fullscreen", Video, "false")
        .since(1, 3, 1)
        .machine(),
    toggle(
        "exclusiveFullscreen",
        "Exclusive Fullscreen",
        Video,
        "false",
    )
    .since(26, 1, 0)
    .machine(),
    toggle(
        "macFullscreenMenuVisibility",
        "Mac Fullscreen Menu Bar",
        Video,
        "false",
    )
    .since(26, 3, 0)
    .machine(),
    slider("gamma", "Brightness", Video, "0.5"),
    OptionDef::new(
        "guiScale",
        "GUI Scale (0 = Auto)",
        Video,
        Kind::Int { min: 0, max: 10 },
        "0",
    ),
    OptionDef::new(
        "debugGuiScale",
        "Debug GUI Scale",
        Video,
        Kind::Int { min: -1, max: 10 },
        "0",
    )
    .since(26, 3, 0),
    OptionDef::new(
        "maxAnisotropyBit",
        "Anisotropic Filtering (bits)",
        Video,
        Kind::Int { min: 1, max: 3 },
        "2",
    )
    .since(1, 21, 11),
    OptionDef::new(
        "textureFiltering",
        "Texture Filtering",
        Video,
        Kind::Int { min: 0, max: 2 },
        "0",
    )
    .since(1, 21, 11),
    OptionDef::new(
        "maxFps",
        "Max Framerate",
        Video,
        Kind::Int { min: 10, max: 260 },
        "120",
    )
    .since(1, 7, 2),
    OptionDef::new(
        "fpsLimit",
        "Max Framerate",
        Video,
        Kind::IntChoice(FPS_LIMIT_LEGACY),
        "0",
    )
    .until(1, 7, 2),
    toggle(
        "improvedTransparency",
        "Improved Transparency",
        Video,
        "false",
    )
    .since(1, 21, 11),
    OptionDef::new(
        "inactivityFpsLimit",
        "Inactivity FPS Limit",
        Video,
        Kind::Choice(INACTIVITY),
        "afk",
    )
    .since(1, 21, 2)
    .quoted(),
    OptionDef::new(
        "mipmapLevels",
        "Mipmap Levels",
        Video,
        Kind::Int { min: 0, max: 4 },
        "4",
    )
    .since(1, 7, 2),
    toggle("clouds", "Clouds", Video, "true").until(1, 8, 0),
    OptionDef::new(
        "renderClouds",
        "Clouds",
        Video,
        Kind::Choice(CLOUDS),
        "true",
    )
    .since(1, 8, 0)
    .quoted(),
    OptionDef::new(
        "cloudRange",
        "Cloud Distance",
        Video,
        Kind::Int { min: 2, max: 128 },
        "128",
    )
    .since(1, 21, 6),
    OptionDef::new(
        "viewDistance",
        "Render Distance",
        Video,
        Kind::IntChoice(VIEW_DISTANCE_LEGACY),
        "1",
    )
    .until(1, 7, 2),
    OptionDef::new(
        "renderDistance",
        "Render Distance (chunks)",
        Video,
        Kind::Int { min: 2, max: 32 },
        "12",
    )
    .since(1, 7, 2),
    OptionDef::new(
        "simulationDistance",
        "Simulation Distance (chunks)",
        Video,
        Kind::Int { min: 5, max: 32 },
        "12",
    )
    .since(1, 18, 0),
    toggle("vignette", "Vignette", Video, "true").since(1, 21, 11),
    OptionDef::new(
        "weatherRadius",
        "Weather Radius",
        Video,
        Kind::Int { min: 3, max: 10 },
        "10",
    )
    .since(1, 21, 11),
    toggle("bobView", "View Bobbing", Video, "true"),
    OptionDef::new(
        "particles",
        "Particles",
        Video,
        Kind::IntChoice(PARTICLES),
        "0",
    ),
    toggle("reducedDebugInfo", "Reduced Debug Info", Video, "false").since(1, 8, 0),
    OptionDef::new(
        "attackIndicator",
        "Attack Indicator",
        Video,
        Kind::IntChoice(ATTACK_INDICATOR),
        "1",
    )
    .since(1, 9, 0),
    toggle("anaglyph3d", "3D Anaglyph", Video, "false").until(1, 13, 0),
    toggle("advancedOpengl", "Advanced OpenGL", Video, "false").until(1, 14, 0),
    toggle("useVbo", "Use VBOs", Video, "true")
        .since(1, 1, 0)
        .until(1, 14, 0),
    toggle("fboEnable", "Framebuffer Objects", Video, "true")
        .since(1, 1, 0)
        .until(1, 14, 0),
    OptionDef::new(
        "overrideWidth",
        "Window Width Override (0 = none)",
        Video,
        Kind::Int { min: 0, max: 16384 },
        "0",
    )
    .since(1, 4, 6)
    .machine(),
    OptionDef::new(
        "overrideHeight",
        "Window Height Override (0 = none)",
        Video,
        Kind::Int { min: 0, max: 16384 },
        "0",
    )
    .since(1, 4, 6)
    .machine(),
    OptionDef::new(
        "fullscreenResolution",
        "Fullscreen Resolution",
        Video,
        Kind::Text,
        "",
    )
    .since(1, 13, 0)
    .machine(),
    // ── Audio ──────────────────────────────────────────────────────────────────────────
    slider("music", "Music", Audio, "1.0").until(1, 7, 2),
    slider("sound", "Sound", Audio, "1.0").until(1, 7, 2),
    sound("soundCategory_master", "Master Volume"),
    sound("soundCategory_music", "Music"),
    sound("soundCategory_record", "Jukebox/Note Blocks"),
    sound("soundCategory_weather", "Weather"),
    sound("soundCategory_block", "Blocks"),
    sound("soundCategory_hostile", "Hostile Creatures"),
    sound("soundCategory_neutral", "Friendly Creatures"),
    sound("soundCategory_player", "Players"),
    sound("soundCategory_ambient", "Ambient/Environment"),
    sound("soundCategory_voice", "Voice/Speech").since(1, 9, 0),
    sound("soundCategory_ui", "UI").since(1, 21, 6),
    OptionDef::new("soundDevice", "Output Device", Audio, Kind::Text, "")
        .since(1, 18, 0)
        .quoted()
        .machine(),
    toggle("showSubtitles", "Show Subtitles", Audio, "false").since(1, 9, 0),
    toggle("directionalAudio", "Directional Audio", Audio, "false").since(1, 19, 0),
    toggle(
        "showNowPlayingToast",
        "Show Now Playing Toast",
        Audio,
        "true",
    )
    .since(1, 21, 5)
    .until(1, 21, 11),
    OptionDef::new(
        "musicToast",
        "Music Toast",
        Audio,
        Kind::Choice(MUSIC_TOAST),
        "never",
    )
    .since(1, 21, 11)
    .quoted(),
    OptionDef::new(
        "musicFrequency",
        "Music Frequency",
        Audio,
        Kind::Choice(MUSIC_FREQUENCY),
        "DEFAULT",
    )
    .since(1, 21, 6)
    .quoted(),
    OptionDef::new(
        "narrator",
        "Narrator",
        Audio,
        Kind::IntChoice(OFF_ON_NARRATOR),
        "0",
    )
    .since(1, 12, 0),
    // ── Controls ───────────────────────────────────────────────────────────────────────
    OptionDef::new(
        "mouseSensitivity",
        "Mouse Sensitivity",
        Controls,
        unit_float(),
        "0.5",
    )
    .display(Percent),
    toggle("invertYMouse", "Invert Mouse Y", Controls, "false"),
    toggle("invertXMouse", "Invert Mouse X", Controls, "false").since(1, 21, 9),
    OptionDef::new(
        "mouseWheelSensitivity",
        "Scroll Sensitivity",
        Controls,
        Kind::Float {
            min: 1.0,
            max: 10.0,
        },
        "1.0",
    )
    .since(1, 13, 0),
    toggle(
        "discrete_mouse_scroll",
        "Discrete Scrolling",
        Controls,
        "false",
    )
    .since(1, 14, 0),
    toggle("touchscreen", "Touchscreen Mode", Controls, "false")
        .since(1, 14, 0)
        .until(1, 21, 11),
    toggle("rawMouseInput", "Raw Input", Controls, "true")
        .since(1, 15, 0)
        .until(1, 21, 11),
    toggle("autoJump", "Auto-Jump", Controls, "false").since(1, 10, 0),
    toggle("toggleCrouch", "Toggle Crouch", Controls, "false").since(1, 15, 0),
    toggle("toggleSprint", "Toggle Sprint", Controls, "false").since(1, 15, 0),
    toggle("toggleAttack", "Toggle Attack", Controls, "false").since(1, 21, 9),
    toggle("toggleUse", "Toggle Use", Controls, "false").since(1, 21, 9),
    OptionDef::new(
        "sprintWindow",
        "Sprint Window (ticks)",
        Controls,
        Kind::Int { min: 0, max: 10 },
        "7",
    )
    .since(1, 21, 9),
    toggle(
        "rotateWithMinecart",
        "Rotate With Minecart",
        Controls,
        "false",
    )
    .since(1, 21, 2),
    toggle(
        "allowCursorChanges",
        "Allow Cursor Changes",
        Controls,
        "true",
    )
    .since(1, 21, 9),
    toggle("quitShortcuts", "Quit Shortcuts", Controls, "true").since(26, 3, 0),
    toggle(
        "ctrlClickEmulatesRightClick",
        "Ctrl-Click Is Right-Click",
        Controls,
        "false",
    )
    .since(26, 3, 0),
    toggle("operatorItemsTab", "Operator Items Tab", Controls, "false").since(1, 19, 3),
    // ── Chat ───────────────────────────────────────────────────────────────────────────
    OptionDef::new(
        "chatVisibility",
        "Chat",
        Chat,
        Kind::IntChoice(CHAT_VISIBILITY),
        "0",
    )
    .since(1, 3, 1),
    toggle("chatColors", "Colors", Chat, "true").since(1, 3, 1),
    toggle("chatLinks", "Web Links", Chat, "true").since(1, 3, 1),
    toggle("chatLinksPrompt", "Prompt on Links", Chat, "true").since(1, 3, 1),
    slider("chatOpacity", "Chat Text Opacity", Chat, "1.0").since(1, 3, 1),
    slider("chatLineSpacing", "Line Spacing", Chat, "0.0").since(1, 16, 0),
    slider(
        "textBackgroundOpacity",
        "Text Background Opacity",
        Chat,
        "0.5",
    )
    .since(1, 14, 0),
    toggle(
        "backgroundForChatOnly",
        "Background for Chat Only",
        Chat,
        "true",
    )
    .since(1, 14, 0),
    slider("chatHeightFocused", "Focused Height", Chat, "1.0").since(1, 5, 1),
    slider("chatHeightUnfocused", "Unfocused Height", Chat, "0.4375").since(1, 5, 1),
    slider("chatScale", "Chat Text Size", Chat, "1.0").since(1, 5, 1),
    slider("chatWidth", "Chat Width", Chat, "1.0").since(1, 5, 1),
    OptionDef::new(
        "chatDelay",
        "Chat Delay (seconds)",
        Chat,
        Kind::Float { min: 0.0, max: 6.0 },
        "0.0",
    )
    .since(1, 16, 0),
    OptionDef::new(
        "notificationDisplayTime",
        "Notification Time",
        Chat,
        Kind::Float {
            min: 0.0,
            max: 10.0,
        },
        "1.0",
    )
    .since(1, 19, 4),
    toggle("autoSuggestions", "Command Suggestions", Chat, "true").since(1, 13, 0),
    toggle("hideMatchedNames", "Hide Matched Names", Chat, "true").since(1, 16, 4),
    toggle("onlyShowSecureChat", "Only Show Secure Chat", Chat, "false").since(1, 19, 0),
    toggle("saveChatDrafts", "Save Chat Drafts", Chat, "false").since(1, 21, 9),
    toggle("hideServerAddress", "Hide Server Address", Chat, "false").since(1, 3, 2),
    toggle("advancedItemTooltips", "Advanced Tooltips", Chat, "false").since(1, 4, 2),
    // ── Accessibility ──────────────────────────────────────────────────────────────────
    toggle("highContrast", "High Contrast", Accessibility, "false").since(1, 19, 4),
    toggle(
        "highContrastBlockOutline",
        "High Contrast Block Outline",
        Accessibility,
        "false",
    )
    .since(1, 21, 2),
    toggle("narratorHotkey", "Narrator Hotkey", Accessibility, "true").since(1, 20, 2),
    toggle(
        "darkMojangStudiosBackground",
        "Dark Loading Background",
        Accessibility,
        "false",
    )
    .since(1, 17, 0),
    toggle(
        "hideLightningFlashes",
        "Hide Lightning Flashes",
        Accessibility,
        "false",
    )
    .since(1, 18, 0),
    toggle(
        "hideSplashTexts",
        "Hide Splash Texts",
        Accessibility,
        "false",
    )
    .since(1, 20, 3),
    slider("damageTiltStrength", "Damage Tilt", Accessibility, "1.0").since(1, 19, 4),
    slider(
        "panoramaScrollSpeed",
        "Panorama Scroll Speed",
        Accessibility,
        "1.0",
    )
    .since(1, 19, 3),
    OptionDef::new(
        "menuBackgroundBlurriness",
        "Menu Background Blur",
        Accessibility,
        Kind::Int { min: 0, max: 10 },
        "5",
    )
    .since(1, 20, 5),
    toggle(
        "showAutosaveIndicator",
        "Autosave Indicator",
        Accessibility,
        "true",
    )
    .since(1, 18, 0),
    toggle(
        "onboardAccessibility",
        "Accessibility Onboarding",
        Internal,
        "true",
    )
    .since(1, 19, 4),
    // ── Skin ───────────────────────────────────────────────────────────────────────────
    part("modelPart_cape", "Cape"),
    part("modelPart_jacket", "Jacket"),
    part("modelPart_left_sleeve", "Left Sleeve"),
    part("modelPart_right_sleeve", "Right Sleeve"),
    part("modelPart_left_pants_leg", "Left Pants Leg"),
    part("modelPart_right_pants_leg", "Right Pants Leg"),
    part("modelPart_hat", "Hat"),
    OptionDef::new("mainHand", "Main Hand", Skin, Kind::Choice(HAND), "right")
        .since(1, 9, 0)
        .quoted(),
    // ── Language & fonts ───────────────────────────────────────────────────────────────
    OptionDef::new("lang", "Language", Language, Kind::Text, "en_us").since(1, 1, 0),
    toggle("forceUnicodeFont", "Force Unicode Font", Language, "false").since(1, 7, 2),
    toggle(
        "japaneseGlyphVariants",
        "Japanese Glyph Variants",
        Language,
        "false",
    )
    .since(1, 20, 5),
    // ── Online & misc ──────────────────────────────────────────────────────────────────
    OptionDef::new("lastServer", "Last Server", Online, Kind::Text, "")
        .machine()
        .hidden(),
    OptionDef::new(
        "resourcePacks",
        "Enabled Resource Packs",
        Online,
        Kind::List,
        "[]",
    )
    .since(1, 7, 2),
    OptionDef::new(
        "incompatibleResourcePacks",
        "Incompatible Resource Packs",
        Online,
        Kind::List,
        "[]",
    )
    .since(1, 8, 8),
    OptionDef::new(
        "difficulty",
        "Difficulty",
        Online,
        Kind::IntChoice(DIFFICULTY),
        "2",
    )
    .until(1, 14, 0),
    toggle("pauseOnLostFocus", "Pause on Lost Focus", Online, "true").since(1, 4, 2),
    toggle(
        "realmsNotifications",
        "Realms Notifications",
        Online,
        "true",
    )
    .since(1, 8, 9),
    toggle(
        "skipMultiplayerWarning",
        "Skip Multiplayer Warning",
        Online,
        "false",
    )
    .since(1, 15, 2),
    toggle("allowServerListing", "Allow Server Listing", Online, "true").since(1, 18, 0),
    toggle("useNativeTransport", "Native Networking", Online, "true").since(1, 8, 1),
    toggle("telemetryOptInExtra", "Extra Telemetry", Online, "false").since(1, 19, 3),
    OptionDef::new(
        "sharePresence",
        "Share Presence",
        Online,
        Kind::Choice(SHARE_PRESENCE),
        "all",
    )
    .since(26, 2, 0)
    .quoted(),
    toggle(
        "inGameNotification",
        "In-Game Notifications",
        Online,
        "false",
    )
    .since(26, 2, 0),
    // ── Internal bookkeeping ───────────────────────────────────────────────────────────
    OptionDef::new(
        "version",
        "Options Version",
        Internal,
        Kind::Int {
            min: 0,
            max: 1_000_000,
        },
        "0",
    )
    .since(1, 10, 0)
    .machine()
    .hidden(),
    OptionDef::new(
        "tutorialStep",
        "Tutorial Step",
        Internal,
        Kind::Choice(TUTORIAL),
        "movement",
    )
    .since(1, 12, 0)
    .machine(),
    toggle(
        "joinedFirstServer",
        "Joined First Server",
        Internal,
        "false",
    )
    .since(1, 16, 4)
    .machine()
    .hidden(),
    toggle("syncChunkWrites", "Sync Chunk Writes", Internal, "false")
        .since(1, 16, 0)
        .machine(),
    toggle("startedCleanly", "Started Cleanly", Internal, "true")
        .since(1, 21, 5)
        .machine()
        .hidden(),
    OptionDef::new(
        "glDebugVerbosity",
        "GL Debug Verbosity",
        Internal,
        Kind::Int { min: 0, max: 4 },
        "1",
    )
    .since(1, 13, 0)
    .machine(),
    // ── Key binds ──────────────────────────────────────────────────────────────────────
    bind("key_key.attack", "Attack/Destroy", "key.mouse.left"),
    bind("key_key.use", "Use Item/Place Block", "key.mouse.right"),
    bind("key_key.forward", "Walk Forwards", "key.keyboard.w"),
    bind("key_key.left", "Strafe Left", "key.keyboard.a"),
    bind("key_key.back", "Walk Backwards", "key.keyboard.s"),
    bind("key_key.right", "Strafe Right", "key.keyboard.d"),
    bind("key_key.jump", "Jump", "key.keyboard.space"),
    bind("key_key.sneak", "Sneak", "key.keyboard.left.shift"),
    bind("key_key.sprint", "Sprint", "key.keyboard.left.control").since(1, 3, 1),
    bind("key_key.drop", "Drop Selected Item", "key.keyboard.q"),
    bind(
        "key_key.inventory",
        "Open/Close Inventory",
        "key.keyboard.e",
    ),
    bind("key_key.chat", "Open Chat", "key.keyboard.t"),
    bind("key_key.playerlist", "List Players", "key.keyboard.tab"),
    bind("key_key.pickItem", "Pick Block", "key.mouse.middle"),
    bind("key_key.command", "Open Command", "key.keyboard.slash").since(1, 3, 1),
    bind("key_key.friends", "Friends", "key.keyboard.o").since(26, 2, 0),
    bind(
        "key_key.socialInteractions",
        "Social Interactions Screen",
        "key.keyboard.p",
    )
    .since(1, 16, 4),
    bind("key_key.toggleGui", "Hide GUI", "key.keyboard.f1").since(1, 21, 11),
    bind(
        "key_key.toggleSpectatorShaderEffects",
        "Toggle Spectator Shader Effects",
        "key.keyboard.f4",
    )
    .since(1, 21, 11),
    bind("key_key.screenshot", "Take Screenshot", "key.keyboard.f2").since(1, 7, 2),
    bind(
        "key_key.togglePerspective",
        "Toggle Perspective",
        "key.keyboard.f5",
    )
    .since(1, 7, 2),
    bind(
        "key_key.smoothCamera",
        "Cinematic Camera",
        "key.keyboard.unknown",
    )
    .since(1, 7, 2),
    bind(
        "key_key.fullscreen",
        "Toggle Fullscreen",
        "key.keyboard.f11",
    )
    .since(1, 7, 4),
    bind(
        "key_key.spectatorOutlines",
        "Spectator Outlines",
        "key.keyboard.unknown",
    )
    .since(1, 8, 0),
    bind(
        "key_key.spectatorHotbar",
        "Spectator Hotbar",
        "key.mouse.middle",
    )
    .since(1, 21, 9),
    bind(
        "key_key.swapHands",
        "Swap Item With Offhand",
        "key.keyboard.f",
    )
    .since(1, 9, 0)
    .until(1, 16, 0),
    bind(
        "key_key.swapOffhand",
        "Swap Item With Offhand",
        "key.keyboard.f",
    )
    .since(1, 16, 0),
    bind(
        "key_key.saveToolbarActivator",
        "Save Hotbar Activator",
        "key.keyboard.c",
    )
    .since(1, 12, 0),
    bind(
        "key_key.loadToolbarActivator",
        "Load Hotbar Activator",
        "key.keyboard.x",
    )
    .since(1, 12, 0),
    bind("key_key.advancements", "Advancements", "key.keyboard.l").since(1, 12, 0),
    bind("key_key.quickActions", "Quick Actions", "key.keyboard.g").since(1, 21, 6),
    bind("key_key.debug.overlay", "Debug Overlay", "key.keyboard.f3").since(1, 21, 11),
    bind(
        "key_key.debug.modifier",
        "Debug Shortcut Modifier",
        "key.keyboard.f3",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.reloadChunk",
        "Debug: Reload Chunks",
        "key.keyboard.a",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.showHitboxes",
        "Debug: Show Hitboxes",
        "key.keyboard.b",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.clearChat",
        "Debug: Clear Chat",
        "key.keyboard.d",
    )
    .since(1, 21, 11),
    bind("key_key.debug.crash", "Debug: Crash", "key.keyboard.c").since(1, 21, 11),
    bind(
        "key_key.debug.showChunkBorders",
        "Debug: Chunk Borders",
        "key.keyboard.g",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.showAdvancedTooltips",
        "Debug: Advanced Tooltips",
        "key.keyboard.h",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.copyRecreateCommand",
        "Debug: Copy Recreate Command",
        "key.keyboard.i",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.spectate",
        "Debug: Spectate",
        "key.keyboard.n",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.switchGameMode",
        "Debug: Switch Game Mode",
        "key.keyboard.f4",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.debugOptions",
        "Debug: Options Screen",
        "key.keyboard.f6",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.focusPause",
        "Debug: Pause on Focus Loss",
        "key.keyboard.p",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.dumpDynamicTextures",
        "Debug: Dump Textures",
        "key.keyboard.s",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.reloadResourcePacks",
        "Debug: Reload Resource Packs",
        "key.keyboard.t",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.profiling",
        "Debug: Profiling",
        "key.keyboard.l",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.copyLocation",
        "Debug: Copy Location",
        "key.keyboard.c",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.dumpVersion",
        "Debug: Dump Version",
        "key.keyboard.v",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.profilingChart",
        "Debug: Profiling Chart",
        "key.keyboard.1",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.fpsCharts",
        "Debug: FPS Charts",
        "key.keyboard.2",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.networkCharts",
        "Debug: Network Charts",
        "key.keyboard.3",
    )
    .since(1, 21, 11),
    bind(
        "key_key.debug.lightmapTexture",
        "Debug: Lightmap Texture",
        "key.keyboard.4",
    )
    .since(26, 1, 0),
    bind(
        "key_key.debug.improvedTransparency",
        "Debug: Improved Transparency",
        "key.keyboard.x",
    )
    .since(26, 3, 0),
    bind("key_key.hotbar.1", "Hotbar Slot 1", "key.keyboard.1"),
    bind("key_key.hotbar.2", "Hotbar Slot 2", "key.keyboard.2"),
    bind("key_key.hotbar.3", "Hotbar Slot 3", "key.keyboard.3"),
    bind("key_key.hotbar.4", "Hotbar Slot 4", "key.keyboard.4"),
    bind("key_key.hotbar.5", "Hotbar Slot 5", "key.keyboard.5"),
    bind("key_key.hotbar.6", "Hotbar Slot 6", "key.keyboard.6"),
    bind("key_key.hotbar.7", "Hotbar Slot 7", "key.keyboard.7"),
    bind("key_key.hotbar.8", "Hotbar Slot 8", "key.keyboard.8"),
    bind("key_key.hotbar.9", "Hotbar Slot 9", "key.keyboard.9"),
];

/// The definition of `key` for `version` (`None` = unknown version, current definitions only).
pub fn find_def(key: &str, version: Option<Ver>) -> Option<&'static OptionDef> {
    OPTIONS
        .iter()
        .find(|def| def.key == key && def.applies_to(version))
}

/// The definition of `key` from any version; used to describe keys present in a file whose
/// version gating doesn't match (for example when the game version is unknown).
pub fn find_def_any(key: &str) -> Option<&'static OptionDef> {
    OPTIONS.iter().rev().find(|def| def.key == key)
}

/// Every definition available in `version`.
pub fn defs_for(version: Option<Ver>) -> impl Iterator<Item = &'static OptionDef> {
    OPTIONS.iter().filter(move |def| def.applies_to(version))
}

/// True for `key_*` entries, including ones this launcher doesn't know (mods).
pub fn is_keybind_key(key: &str) -> bool {
    key.starts_with("key_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_key_never_overlaps_between_versions() {
        for (i, a) in OPTIONS.iter().enumerate() {
            for b in &OPTIONS[i + 1..] {
                if a.key == b.key {
                    assert!(
                        a.until <= b.since || b.until <= a.since,
                        "{} has overlapping definitions",
                        a.key
                    );
                }
            }
        }
    }

    #[test]
    fn defaults_parse_for_their_kind() {
        for def in OPTIONS {
            match def.kind {
                Kind::Bool => assert!(
                    def.default == "true" || def.default == "false",
                    "{}",
                    def.key
                ),
                Kind::Int { .. } | Kind::IntChoice(_) => {
                    assert!(def.default.parse::<i64>().is_ok(), "{}", def.key)
                }
                Kind::Float { min, max } => {
                    let value: f64 = def
                        .default
                        .parse()
                        .unwrap_or_else(|_| panic!("{}", def.key));
                    assert!(
                        value >= min && value <= max,
                        "{} default out of range",
                        def.key
                    );
                }
                Kind::Choice(choices) => assert!(
                    choices.iter().any(|(value, _)| *value == def.default),
                    "{} default not a choice",
                    def.key
                ),
                Kind::Text | Kind::List | Kind::Keybind => {}
            }
        }
    }

    #[test]
    fn gating_picks_the_right_format() {
        assert!(matches!(
            find_def("ao", Some(Ver(1, 12, 2))).unwrap().kind,
            Kind::IntChoice(_)
        ));
        assert!(matches!(
            find_def("ao", Some(Ver(1, 21, 4))).unwrap().kind,
            Kind::Bool
        ));
        assert!(find_def("graphicsPreset", Some(Ver(1, 20, 4))).is_none());
        assert!(find_def("macFullscreenMenuVisibility", Some(Ver(26, 3, 0))).is_some());
    }
}

#[cfg(test)]
mod real_file_tests {
    use super::*;
    use crate::{OptionsFile, value};

    /// Checks the schema against real option files: set `VERTEX_OPTIONS_SAMPLES` to a
    /// `path[:path...]` list, each with the game version as `path=version`.
    #[test]
    fn schema_understands_real_option_files() {
        let Ok(samples) = std::env::var("VERTEX_OPTIONS_SAMPLES") else {
            return;
        };
        for sample in samples.split(',') {
            let (path, version) = sample.split_once('=').expect("path=version");
            let file = OptionsFile::read(std::path::Path::new(path)).expect("readable sample");
            let version = crate::parse_game_version(version);
            let mut problems = Vec::new();
            for key in file.keys() {
                let raw = file.get(key).unwrap();
                let Some(def) = find_def(key, version) else {
                    // Unknown keys are legal (mods add their own); report, don't fail.
                    if !is_keybind_key(key) && !key.starts_with("soundCategory_") {
                        eprintln!("{path}: key not in schema: {key}");
                    }
                    continue;
                };
                let ok = match def.kind {
                    Kind::Bool => value::parse_bool(raw).is_some(),
                    Kind::Int { min, max } => {
                        value::parse_i64(raw).is_some_and(|v| v >= min && v <= max)
                    }
                    Kind::IntChoice(c) => {
                        value::parse_i64(raw).is_some_and(|v| c.iter().any(|(k, _)| *k == v))
                    }
                    Kind::Float { min, max } => {
                        value::parse_f64(raw).is_some_and(|v| v >= min && v <= max)
                    }
                    Kind::Choice(c) => c.iter().any(|(k, _)| *k == value::unquote(raw).0),
                    Kind::Text | Kind::List | Kind::Keybind => true,
                };
                if !ok {
                    problems.push(format!("{key}:{raw} does not fit its definition"));
                }
                if def.quoted && matches!(def.kind, Kind::Choice(_)) && !value::unquote(raw).1 {
                    problems.push(format!("{key} expected quoted"));
                }
            }
            // Keys the version should have but the file lacks (informational).
            let missing: Vec<_> = defs_for(version)
                .filter(|d| !file.contains(d.key) && d.group != Group::Keybinds)
                .map(|d| d.key)
                .collect();
            eprintln!("{path}: missing-from-file {missing:?}");
            assert!(problems.is_empty(), "{path}: {problems:#?}");
        }
    }
}
