//! Plain-language explanations of every known option, shown in the settings tooltips.

use crate::schema;

/// What the option does, or `None` for keys this launcher doesn't know.
pub fn description(key: &str) -> Option<&'static str> {
    Some(match key {
        // ── Video ──
        "ao" => {
            "Softens the shadows where blocks meet. Higher settings look nicer but cost performance."
        }
        "biomeBlendRadius" => {
            "How many blocks around you are averaged when tinting grass, leaves and water by biome. Higher blends transitions more smoothly."
        }
        "chunkSectionFadeInTime" => {
            "Seconds that newly loaded chunks take to fade in. 0 shows them instantly."
        }
        "cutoutLeaves" => {
            "Draws leaves with see-through gaps (fancy) instead of solid blocks (fast)."
        }
        "enableVsync" => {
            "Locks the frame rate to your monitor's refresh rate to prevent screen tearing."
        }
        "entityDistanceScaling" => {
            "How far away mobs, items and other entities are still drawn. Higher values cost performance."
        }
        "entityShadows" => "Draws a soft shadow under mobs and other entities.",
        "fov" => {
            "Field of view in degrees: how wide the game world appears. Higher shows more but distorts the edges."
        }
        "fovEffectScale" => {
            "How strongly sprinting, speed effects and bows change your field of view."
        }
        "screenEffectScale" => {
            "Strength of the wobble when you use a Nether portal or have nausea."
        }
        "darknessEffectScale" => {
            "How strongly the screen pulses when a Warden or Darkness effect is nearby."
        }
        "glintSpeed" => "How fast the shimmer on enchanted items moves.",
        "glintStrength" => "How bright the shimmer on enchanted items is.",
        "preferredGraphicsBackend" => {
            "Which graphics API the game renders with. Default lets Minecraft choose."
        }
        "fancyGraphics" => {
            "Fancy draws more detail (leaves, water, clouds); fast is quicker on weak hardware."
        }
        "graphicsMode" => {
            "Overall graphics quality: Fast, Fancy, or Fabulous! (adds transparency and weather effects)."
        }
        "graphicsPreset" => {
            "A bundle of graphics options. Changing individual options switches this to Custom."
        }
        "prioritizeChunkUpdates" => {
            "How eagerly the game rebuilds chunks after a block changes. Higher options prevent visual lag when building but can lower frame rate."
        }
        "fullscreen" => "Runs the game in fullscreen instead of a window.",
        "exclusiveFullscreen" => {
            "Takes full control of the display in fullscreen. Can lower latency but makes alt-tabbing slower."
        }
        "macFullscreenMenuVisibility" => {
            "Whether the macOS menu bar can appear while the game is fullscreen."
        }
        "gamma" => {
            "Brightness of dark areas. Moody is the default; Bright makes caves easier to see."
        }
        "guiScale" => "Size of menus and the HUD. 0 picks the largest size that fits your window.",
        "debugGuiScale" => "Scale used for the debug screen text.",
        "maxAnisotropyBit" => {
            "Keeps textures sharp on surfaces seen at a steep angle. Higher values are sharper but cost performance."
        }
        "textureFiltering" => {
            "How textures are smoothed: none, smooth, or anisotropic for crisper distant surfaces."
        }
        "maxFps" | "fpsLimit" => {
            "Caps the frame rate. Lower it to save power and heat; the top value is unlimited."
        }
        "improvedTransparency" => {
            "Sorts see-through things (water, stained glass, particles) correctly. Needs a capable GPU."
        }
        "inactivityFpsLimit" => {
            "Lowers the frame rate when you are away from the keyboard or the window is minimized."
        }
        "mipmapLevels" => "Blends textures at a distance to reduce shimmering. 0 turns it off.",
        "clouds" | "renderClouds" => "Cloud rendering: off, fast (flat), or fancy (3D).",
        "cloudRange" => "How far away clouds are drawn, in blocks.",
        "viewDistance" | "renderDistance" => {
            "How many chunks around you are drawn. Higher lets you see farther but costs performance."
        }
        "simulationDistance" => {
            "How many chunks around you the game keeps active (mobs move, crops grow). Lower saves performance."
        }
        "vignette" => "Darkens the screen edges slightly.",
        "weatherRadius" => "How far from you rain and snow are drawn, in blocks.",
        "bobView" => "Makes the camera sway while you walk.",
        "particles" => "How many particles (smoke, sparks, bubbles) are shown.",
        "reducedDebugInfo" => "Hides coordinates and other details on the debug screen.",
        "attackIndicator" => "Where the attack cooldown meter is shown.",
        "anaglyph3d" => "Red/cyan 3D effect for anaglyph glasses.",
        "advancedOpengl" => "Uses occlusion culling to skip drawing hidden chunks.",
        "useVbo" => "Uses vertex buffer objects for faster rendering.",
        "fboEnable" => "Renders through framebuffer objects; needed for some effects.",
        "overrideWidth" => "Forces the game window width in pixels. 0 uses the default size.",
        "overrideHeight" => "Forces the game window height in pixels. 0 uses the default size.",
        "fullscreenResolution" => "Video mode used in exclusive fullscreen, as chosen in the game.",
        // ── Audio ──
        "music" | "soundCategory_music" => "Volume of background music.",
        "sound" => "Volume of sound effects.",
        "soundCategory_master" => "Overall volume for everything.",
        "soundCategory_record" => "Volume of jukeboxes and note blocks.",
        "soundCategory_weather" => "Volume of rain and thunder.",
        "soundCategory_block" => "Volume of sounds from blocks, like digging and placing.",
        "soundCategory_hostile" => "Volume of hostile mobs.",
        "soundCategory_neutral" => "Volume of passive and neutral mobs.",
        "soundCategory_player" => "Volume of sounds made by players.",
        "soundCategory_ambient" => "Volume of ambient and environmental sounds, like caves.",
        "soundCategory_voice" => "Volume of speech and voice sounds.",
        "soundCategory_ui" => "Volume of menu and interface sounds.",
        "soundDevice" => {
            "The audio output device the game plays through. System Default follows your operating system's choice."
        }
        "showSubtitles" => "Shows captions for sounds, useful for accessibility or when muted.",
        "directionalAudio" => {
            "Uses a 3D audio effect so sounds seem to come from their direction. Best with headphones."
        }
        "showNowPlayingToast" | "musicToast" => {
            "Shows a pop-up naming the music track that starts playing."
        }
        "musicFrequency" => "How often background music plays.",
        "narrator" => "Reads chat and menus aloud. Choose what it reads, or turn it off.",
        // ── Controls ──
        "mouseSensitivity" => "How far the view turns for a given mouse movement.",
        "invertYMouse" => "Reverses vertical look, so moving the mouse up looks down.",
        "invertXMouse" => "Reverses horizontal look.",
        "mouseWheelSensitivity" => {
            "How much each scroll notch changes the selected hotbar slot or menu."
        }
        "discrete_mouse_scroll" => {
            "Treats each scroll as exactly one step, ignoring high-resolution scrolling. Helps with free-spinning wheels."
        }
        "touchscreen" => "Uses touchscreen controls instead of a mouse and keyboard.",
        "rawMouseInput" => {
            "Reads the mouse directly, ignoring operating-system acceleration and settings."
        }
        "autoJump" => "Jumps automatically over single blocks while you move.",
        "toggleCrouch" => "Sneak stays on after one press instead of while held.",
        "toggleSprint" => "Sprint stays on after one press instead of while held.",
        "toggleAttack" => "Attack stays active after one press instead of while held.",
        "toggleUse" => "Use item stays active after one press instead of while held.",
        "sprintWindow" => "How many ticks you have to tap forward twice to start sprinting.",
        "rotateWithMinecart" => "The camera turns with the minecart on curves.",
        "allowCursorChanges" => "Lets the game change the mouse cursor's shape.",
        "quitShortcuts" => "Enables keyboard shortcuts for quitting the game.",
        "ctrlClickEmulatesRightClick" => "Treats Ctrl+click as a right click (macOS).",
        "operatorItemsTab" => "Shows the operator-only items tab in creative mode.",
        // ── Chat ──
        "chatVisibility" => "Which messages appear in chat: everything, only commands, or nothing.",
        "chatColors" => "Shows colored text in chat.",
        "chatLinks" => "Makes web links in chat clickable.",
        "chatLinksPrompt" => "Asks for confirmation before opening a link from chat.",
        "chatOpacity" => "How opaque the chat text is.",
        "chatLineSpacing" => "Space between chat lines.",
        "textBackgroundOpacity" => "Opacity of the dark box behind chat text.",
        "backgroundForChatOnly" => "Only draws the text background behind chat, not other text.",
        "chatHeightFocused" => "Chat height while you have chat open.",
        "chatHeightUnfocused" => "Chat height while chat is closed.",
        "chatScale" => "Size of chat text.",
        "chatWidth" => "Width of the chat window.",
        "chatDelay" => "Seconds to delay incoming chat messages.",
        "notificationDisplayTime" => "How long pop-up notifications and toasts stay on screen.",
        "autoSuggestions" => "Suggests commands as you type in chat.",
        "hideMatchedNames" => "Hides player names that match a chat filter.",
        "onlyShowSecureChat" => "Only shows chat messages that carry a valid signature.",
        "saveChatDrafts" => "Remembers unsent chat text when you close chat.",
        "hideServerAddress" => "Hides the server address in the multiplayer list and pause screen.",
        "advancedItemTooltips" => "Adds item IDs and durability to item tooltips (same as F3+H).",
        // ── Accessibility ──
        "highContrast" => "Uses higher-contrast menus and button outlines.",
        "highContrastBlockOutline" => {
            "Draws a stronger outline around the block you're looking at."
        }
        "narratorHotkey" => "Lets you toggle the narrator with its keyboard shortcut.",
        "darkMojangStudiosBackground" => "Uses a dark background on the loading screen.",
        "hideLightningFlashes" => "Stops the sky flashing during thunderstorms.",
        "hideSplashTexts" => "Hides the yellow splash text on the title screen.",
        "damageTiltStrength" => "How much the view shakes when you take damage.",
        "panoramaScrollSpeed" => "Speed of the rotating title-screen panorama. 0 stops it.",
        "menuBackgroundBlurriness" => "How blurred the world behind menus looks.",
        "showAutosaveIndicator" => "Shows an icon while the world is saving.",
        "onboardAccessibility" => {
            "Whether the accessibility setup screen still needs to be shown at first launch."
        }
        // ── Skin ──
        "modelPart_cape" => "Shows your cape.",
        "modelPart_jacket" => "Shows the jacket layer of your skin.",
        "modelPart_left_sleeve" => "Shows the left sleeve layer of your skin.",
        "modelPart_right_sleeve" => "Shows the right sleeve layer of your skin.",
        "modelPart_left_pants_leg" => "Shows the left pants layer of your skin.",
        "modelPart_right_pants_leg" => "Shows the right pants layer of your skin.",
        "modelPart_hat" => "Shows the hat layer of your skin.",
        "mainHand" => "Which hand is your main hand.",
        // ── Language ──
        "lang" => "The language the game's menus and text are shown in.",
        "forceUnicodeFont" => {
            "Uses the Unicode font for all text, which supports more characters but looks different."
        }
        "japaneseGlyphVariants" => "Uses Japanese forms of shared CJK characters.",
        // ── Online & misc ──
        "lastServer" => "The last server address you connected to with Direct Connect.",
        "resourcePacks" => {
            "Resource packs the game has enabled, in load order. Manage them on the Resource Packs tab."
        }
        "incompatibleResourcePacks" => {
            "Resource packs the game skipped because they target another version."
        }
        "difficulty" => "Default difficulty for new worlds.",
        "pauseOnLostFocus" => "Pauses the game when the window loses focus.",
        "realmsNotifications" => "Shows Realms invites and news on the title screen.",
        "skipMultiplayerWarning" => "Skips the multiplayer safety warning.",
        "allowServerListing" => "Lets your name appear in the player list shown by servers.",
        "useNativeTransport" => "Uses the operating system's faster networking when available.",
        "telemetryOptInExtra" => "Sends extra optional diagnostic data to Mojang.",
        "sharePresence" => "Lets friends see what you're playing.",
        "inGameNotification" => "Shows in-game pop-up notifications.",
        // ── Advanced / internal ──
        "version" => "Data version of the game that last wrote this file. Managed by the game.",
        "tutorialStep" => "Progress through the in-game tutorial hints. Managed by the game.",
        "joinedFirstServer" => "Whether you have joined a server before. Managed by the game.",
        "syncChunkWrites" => {
            "Writes world data to disk synchronously. Safer against crashes, but slower."
        }
        "startedCleanly" => "Set by the game to detect a crash on the previous run.",
        "glDebugVerbosity" => "How much OpenGL debug output the game prints to the log.",
        _ => return None,
    })
}

/// Tooltip text for an option: what it does, plus its raw `options.txt` key.
pub fn tooltip(def: &schema::OptionDef) -> String {
    let mut text = String::new();
    if let Some(desc) = description(def.key) {
        text.push_str(desc);
    }
    if !def.note.is_empty() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(def.note);
    }
    if text.is_empty() {
        format!("Option: {}", def.key)
    } else {
        format!("{text}\n\nOption: {}", def.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Kind;

    #[test]
    fn every_editable_option_is_explained() {
        let missing: Vec<&str> = schema::OPTIONS
            .iter()
            .filter(|def| !matches!(def.kind, Kind::Keybind))
            .filter(|def| description(def.key).is_none())
            .map(|def| def.key)
            .collect();
        assert!(
            missing.is_empty(),
            "options without a description: {missing:?}"
        );
    }
}
