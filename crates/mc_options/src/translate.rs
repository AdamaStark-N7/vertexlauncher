//! Translating settings between game versions.
//!
//! Both files are read into a version-neutral form (modern key names, unquoted modern
//! values), differences are computed there, and only the changed keys are written back in
//! the target version's own format.

use std::collections::BTreeMap;

use crate::file::OptionsFile;
use crate::keys;
use crate::schema::{Group, Kind, OptionDef, find_def, find_def_any, is_keybind_key};
use crate::value;
use crate::version::Ver;

/// Neutral settings: modern key → modern unquoted value.
pub type Canon = BTreeMap<String, String>;

const V_1_7_2: Ver = Ver(1, 7, 2);
const V_1_11: Ver = Ver(1, 11, 0);
const V_1_13: Ver = Ver(1, 13, 0);
const V_1_14: Ver = Ver(1, 14, 0);
const V_1_16: Ver = Ver(1, 16, 0);

/// Chunk counts for the legacy 4-step render distance (Far, Normal, Short, Tiny).
const LEGACY_VIEW_CHUNKS: [i64; 4] = [16, 8, 4, 2];
/// Framerate caps for the legacy fps modes (Max, Balanced, Power Saver).
const LEGACY_FPS: [i64; 3] = [260, 120, 40];

fn at_least(version: Option<Ver>, bound: Ver) -> bool {
    version.is_none_or(|v| v >= bound)
}

fn below(version: Option<Ver>, bound: Ver) -> bool {
    version.is_some_and(|v| v < bound)
}

fn nearest_index(values: &[i64], target: i64) -> usize {
    values
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| ((**v - target).abs(), **v))
        .map_or(0, |(index, _)| index)
}

/// Renamed key binds, `(old, new)`.
const KEYBIND_RENAMES: &[(&str, &str)] = &[("key_key.swapHands", "key_key.swapOffhand")];

fn canon_keybind_name(key: &str) -> String {
    KEYBIND_RENAMES
        .iter()
        .find(|(old, _)| *old == key)
        .map_or_else(|| key.to_owned(), |(_, new)| (*new).to_owned())
}

fn target_keybind_name(key: &str, version: Option<Ver>) -> String {
    if below(version, V_1_16)
        && let Some((old, _)) = KEYBIND_RENAMES.iter().find(|(_, new)| *new == key)
    {
        return (*old).to_owned();
    }
    key.to_owned()
}

/// Reads a file into neutral form.
pub fn to_canon(file: &OptionsFile, version: Option<Ver>) -> Canon {
    let mut canon = Canon::new();
    for key in file.keys() {
        let Some(raw) = file.get(key) else { continue };
        let text = value::unquote(raw).0.trim().to_owned();
        match key {
            "ao" => {
                let on = value::parse_bool(raw)
                    .or_else(|| value::parse_i64(raw).map(|level| level != 0));
                if let Some(on) = on {
                    canon.insert("ao".into(), on.to_string());
                }
            }
            "fancyGraphics" => {
                if let Some(fancy) = value::parse_bool(raw) {
                    canon.insert(
                        "graphics".into(),
                        if fancy { "fancy" } else { "fast" }.into(),
                    );
                }
            }
            "graphicsMode" => {
                if let Some(mode) = value::parse_i64(raw) {
                    let name = ["fast", "fancy", "fabulous"].get(mode.max(0) as usize);
                    canon.insert("graphics".into(), name.unwrap_or(&"fancy").to_string());
                }
            }
            "graphicsPreset" => {
                // "custom" says nothing about the base look, so it can't be translated.
                if matches!(text.as_str(), "fast" | "fancy" | "fabulous") {
                    canon.insert("graphics".into(), text);
                }
            }
            "viewDistance" => {
                if let Some(step) = value::parse_i64(raw) {
                    let chunks = LEGACY_VIEW_CHUNKS[step.clamp(0, 3) as usize];
                    canon.insert("renderDistance".into(), chunks.to_string());
                }
            }
            "fpsLimit" => {
                if let Some(mode) = value::parse_i64(raw) {
                    let fps = LEGACY_FPS[mode.clamp(0, 2) as usize];
                    canon.insert("maxFps".into(), fps.to_string());
                }
            }
            "clouds" => {
                if let Some(on) = value::parse_bool(raw) {
                    canon.insert("renderClouds".into(), on.to_string());
                }
            }
            "music" => {
                canon.insert("soundCategory_music".into(), text);
            }
            "sound" => {}
            "lang" => {
                canon.insert("lang".into(), text.to_lowercase());
            }
            _ if is_keybind_key(key) => {
                let modern = if below(version, V_1_13) {
                    text.parse::<i32>().ok().and_then(keys::legacy_to_modern)
                } else {
                    Some(text)
                };
                if let Some(modern) = modern {
                    canon.insert(canon_keybind_name(key), modern);
                }
            }
            _ => {
                canon.insert(key.to_owned(), text);
            }
        }
    }
    canon
}

/// Whether string enums are quoted in `file`: what the file already does, else the schema.
fn quoting_for(def: &OptionDef, file: &OptionsFile, version: Option<Ver>) -> bool {
    for key in file.keys() {
        if let Some(other) = find_def(key, version)
            && matches!(other.kind, Kind::Choice(_))
            && let Some(raw) = file.get(key)
            && !raw.is_empty()
        {
            return value::unquote(raw).1;
        }
    }
    def.quoted
}

/// Formats a neutral `value` for `def` in the target file's style; `None` if it doesn't fit.
pub fn format_for_def(
    def: &OptionDef,
    value_text: &str,
    file: &OptionsFile,
    version: Option<Ver>,
) -> Option<String> {
    Some(match def.kind {
        Kind::Bool => value::format_bool(value::parse_bool(value_text)?),
        Kind::Int { min, max } => value::parse_f64(value_text)?
            .round()
            .clamp(min as f64, max as f64)
            .to_string()
            .split('.')
            .next()?
            .to_owned(),
        Kind::IntChoice(choices) => {
            let n = value::parse_i64(value_text)?;
            choices
                .iter()
                .any(|(v, _)| *v == n)
                .then(|| n.to_string())?
        }
        Kind::Float { min, max } => {
            value::format_float(value::parse_f64(value_text)?.clamp(min, max))
        }
        Kind::Choice(choices) => {
            let plain = value::unquote(value_text).0;
            choices.iter().any(|(v, _)| *v == plain).then_some(())?;
            if quoting_for(def, file, version) {
                value::quote(plain)
            } else {
                plain.to_owned()
            }
        }
        Kind::Text => {
            let plain = value::unquote(value_text).0;
            if def.quoted {
                value::quote(plain)
            } else {
                plain.to_owned()
            }
        }
        Kind::List => value_text.to_owned(),
        Kind::Keybind => value_text.to_owned(),
    })
}

/// Writes for one neutral setting into the target's format. Empty when the target has no
/// equivalent (or the value doesn't fit it).
fn from_canon_key(
    key: &str,
    canon_value: &str,
    target: &OptionsFile,
    version: Option<Ver>,
) -> Vec<(String, String)> {
    let one = |k: &str, v: Option<String>| -> Vec<(String, String)> {
        v.map(|v| vec![(k.to_owned(), v)]).unwrap_or_default()
    };
    let with_def = |k: &str, text: &str| -> Vec<(String, String)> {
        find_def(k, version)
            .and_then(|def| format_for_def(def, text, target, version))
            .map(|formatted| vec![(k.to_owned(), formatted)])
            .unwrap_or_default()
    };
    match key {
        "ao" => {
            let on = value::parse_bool(canon_value).unwrap_or(true);
            match find_def("ao", version).map(|d| d.kind) {
                Some(Kind::IntChoice(_)) => {
                    // Keep an existing "minimum" level rather than forcing maximum.
                    let existing = target.get("ao").and_then(value::parse_i64).unwrap_or(0);
                    one(
                        "ao",
                        Some(
                            if on {
                                existing.max(1).max(if existing == 0 { 2 } else { 0 })
                            } else {
                                0
                            }
                            .to_string(),
                        ),
                    )
                }
                Some(_) => with_def("ao", canon_value),
                None => Vec::new(),
            }
        }
        "graphics" => {
            let idx = ["fast", "fancy", "fabulous"]
                .iter()
                .position(|name| *name == canon_value)
                .unwrap_or(1);
            if find_def("graphicsPreset", version).is_some() {
                with_def("graphicsPreset", canon_value)
            } else if find_def("graphicsMode", version).is_some() {
                one("graphicsMode", Some(idx.to_string()))
            } else if find_def("fancyGraphics", version).is_some() {
                one("fancyGraphics", Some((idx >= 1).to_string()))
            } else {
                Vec::new()
            }
        }
        "renderDistance" => {
            if below(version, V_1_7_2) {
                let chunks = value::parse_i64(canon_value).unwrap_or(8);
                one(
                    "viewDistance",
                    Some(nearest_index(&LEGACY_VIEW_CHUNKS, chunks).to_string()),
                )
            } else {
                with_def("renderDistance", canon_value)
            }
        }
        "maxFps" => {
            if below(version, V_1_7_2) {
                let fps = value::parse_i64(canon_value).unwrap_or(120);
                one(
                    "fpsLimit",
                    Some(nearest_index(&LEGACY_FPS, fps).to_string()),
                )
            } else {
                with_def("maxFps", canon_value)
            }
        }
        "renderClouds" => {
            if find_def("clouds", version).is_some() {
                one("clouds", Some((canon_value != "false").to_string()))
            } else {
                with_def("renderClouds", canon_value)
            }
        }
        "soundCategory_music" if below(version, V_1_7_2) => with_def("music", canon_value),
        "lang" => {
            if at_least(version, V_1_11) {
                with_def("lang", &canon_value.to_lowercase())
            } else {
                let mut parts = canon_value.splitn(2, '_');
                let text = match (parts.next(), parts.next()) {
                    (Some(l), Some(r)) => format!("{l}_{}", r.to_uppercase()),
                    _ => canon_value.to_owned(),
                };
                with_def("lang", &text)
            }
        }
        "difficulty" => {
            if below(version, V_1_14) {
                with_def("difficulty", canon_value)
            } else {
                Vec::new()
            }
        }
        _ if is_keybind_key(key) => {
            let target_key = target_keybind_name(key, version);
            // Only bind keys the target knows about, or already has (mod binds).
            if find_def(&target_key, version).is_none() && !target.contains(&target_key) {
                return Vec::new();
            }
            if below(version, V_1_13) {
                keys::modern_to_legacy(canon_value)
                    .map(|code| vec![(target_key, code.to_string())])
                    .unwrap_or_default()
            } else {
                vec![(target_key, canon_value.to_owned())]
            }
        }
        _ => {
            match find_def(key, version) {
                Some(_) => with_def(key, canon_value),
                // Unknown to the schema: keep mod/future keys in step only where they already exist.
                None if target.contains(key) => vec![(key.to_owned(), canon_value.to_owned())],
                None => Vec::new(),
            }
        }
    }
}

/// Which neutral keys a sync may touch: not machine-specific, and in an allowed group.
pub fn key_is_syncable(canon_key: &str, allowed: &dyn Fn(Group) -> bool) -> bool {
    if is_keybind_key(canon_key) {
        return allowed(Group::Keybinds);
    }
    match canon_key {
        // Neutral-only keys.
        "graphics" => allowed(Group::Video),
        _ => match find_def_any(canon_key) {
            Some(def) => {
                !def.machine_specific && def.group != Group::Internal && allowed(def.group)
            }
            None => false,
        },
    }
}

/// Computes the writes that bring `target` in line with `source` for the syncable keys.
/// Only settings whose neutral value differs are returned, as `(target key, raw value)`.
pub fn plan_sync(
    source: &OptionsFile,
    source_version: Option<Ver>,
    target: &OptionsFile,
    target_version: Option<Ver>,
    allowed: &dyn Fn(Group) -> bool,
) -> Vec<(String, String)> {
    let wanted = to_canon(source, source_version);
    let current = to_canon(target, target_version);
    let mut writes = Vec::new();
    for (key, new_value) in &wanted {
        if !key_is_syncable(key, allowed) || current.get(key) == Some(new_value) {
            continue;
        }
        // A "custom" graphics preset is the user's own mix; don't flatten it.
        if key == "graphics"
            && target
                .get("graphicsPreset")
                .is_some_and(|p| value::unquote(p).0 == "custom")
        {
            continue;
        }
        writes.extend(from_canon_key(key, new_value, target, target_version));
    }
    // Drop writes that don't actually change the raw file.
    writes.retain(|(key, raw)| target.get(key) != Some(raw.as_str()));
    writes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: Group) -> bool {
        true
    }

    fn apply(target: &mut OptionsFile, writes: &[(String, String)]) {
        for (key, raw) in writes {
            target.set(key, raw);
        }
    }

    #[test]
    fn translates_a_1_12_file_into_modern_format() {
        let old = OptionsFile::parse(
            "ao:2\nfancyGraphics:false\nviewDistance:0\nfpsLimit:1\nclouds:false\nkey_key.forward:30\nkey_key.swapHands:34\nlang:en_US\nmusic:0.3\nguiScale:3\nfov:0.25\n",
        );
        let mut modern = OptionsFile::parse(
            "ao:false\ngraphicsMode:1\nrenderDistance:12\nmaxFps:260\nrenderClouds:\"true\"\nkey_key.forward:key.keyboard.w\nkey_key.swapOffhand:key.keyboard.f\nlang:en_us\nsoundCategory_music:1.0\nguiScale:0\nfov:0.0\n",
        );
        let writes = plan_sync(
            &old,
            Some(Ver(1, 12, 2)),
            &modern,
            Some(Ver(1, 21, 4)),
            &all,
        );
        apply(&mut modern, &writes);
        assert_eq!(modern.get("ao"), Some("true"));
        assert_eq!(modern.get("graphicsMode"), Some("0"));
        assert_eq!(modern.get("renderDistance"), Some("16"));
        assert_eq!(modern.get("maxFps"), Some("120"));
        assert_eq!(modern.get("renderClouds"), Some("\"false\""));
        assert_eq!(modern.get("key_key.forward"), Some("key.keyboard.a"));
        assert_eq!(modern.get("key_key.swapOffhand"), Some("key.keyboard.g"));
        assert_eq!(modern.get("soundCategory_music"), Some("0.3"));
        assert_eq!(modern.get("guiScale"), Some("3"));
        assert_eq!(modern.get("fov"), Some("0.25"));
        // Nothing was invented that the target doesn't know.
        assert!(!modern.contains("music") && !modern.contains("fpsLimit"));
    }

    #[test]
    fn translates_modern_into_legacy_and_keeps_unrelated_keys() {
        let modern = OptionsFile::parse(
            "ao:true\ngraphicsPreset:\"fabulous\"\nrenderDistance:12\nmaxFps:100\nrenderClouds:\"fast\"\nkey_key.forward:key.keyboard.up\nkey_key.jump:key.mouse.4\nkey_key.debug.overlay:key.keyboard.f3\nlang:en_us\nmouseSensitivity:0.75\n",
        );
        let mut old = OptionsFile::parse(
            "ao:0\nfancyGraphics:false\nviewDistance:2\nfpsLimit:0\nclouds:false\nkey_key.forward:17\nkey_key.jump:57\nlang:en_US\nmouseSensitivity:0.5\nmodded:keep\n",
        );
        let writes = plan_sync(&modern, Some(Ver(1, 21, 4)), &old, Some(Ver(1, 6, 4)), &all);
        apply(&mut old, &writes);
        assert_eq!(old.get("ao"), Some("2"));
        assert_eq!(old.get("fancyGraphics"), Some("true"));
        assert_eq!(old.get("viewDistance"), Some("1"));
        assert_eq!(old.get("fpsLimit"), Some("1"));
        assert_eq!(old.get("clouds"), Some("true"));
        assert_eq!(old.get("key_key.forward"), Some("200"));
        assert_eq!(old.get("key_key.jump"), Some("-97"));
        assert_eq!(old.get("mouseSensitivity"), Some("0.75"));
        assert_eq!(old.get("modded"), Some("keep"));
        assert!(!old.contains("key_key.debug.overlay"));
    }

    #[test]
    fn respects_machine_specific_keys_groups_and_custom_graphics() {
        let source = OptionsFile::parse(
            "fullscreen:true\nsoundDevice:\"Speakers\"\ngraphicsPreset:\"fabulous\"\nfov:0.5\nchatScale:0.5\n",
        );
        let target = OptionsFile::parse(
            "fullscreen:false\nsoundDevice:\"\"\ngraphicsPreset:\"custom\"\nfov:0.0\nchatScale:1.0\n",
        );
        let only_video = |g: Group| g == Group::Video;
        let writes = plan_sync(
            &source,
            Some(Ver(26, 3, 0)),
            &target,
            Some(Ver(26, 3, 0)),
            &only_video,
        );
        let keys: Vec<&str> = writes.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["fov"], "{writes:?}");
    }

    #[test]
    fn no_writes_when_already_equal_and_idempotent() {
        let a = OptionsFile::parse("ao:true\nrenderDistance:12\nfov:0.5\nrenderClouds:\"true\"\n");
        let mut b =
            OptionsFile::parse("ao:false\nrenderDistance:8\nfov:0.0\nrenderClouds:\"false\"\n");
        let writes = plan_sync(&a, Some(Ver(1, 21, 4)), &b, Some(Ver(1, 21, 4)), &all);
        apply(&mut b, &writes);
        assert!(plan_sync(&a, Some(Ver(1, 21, 4)), &b, Some(Ver(1, 21, 4)), &all).is_empty());
    }
}
