use super::*;

pub(crate) fn normalize_minecraft_game_version(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if looks_like_minecraft_release_version(trimmed)
        || looks_like_minecraft_pre_release_version(trimmed)
        || looks_like_minecraft_snapshot_version(trimmed)
    {
        return Some(trimmed.to_owned());
    }
    None
}

/// Accepts the legacy `1.x` scheme and the year-based scheme (`26.x` onward).
fn is_minecraft_release_major(major: &str) -> bool {
    major == "1"
        || (major.len() == 2
            && major.chars().all(|ch| ch.is_ascii_digit())
            && major.parse::<u32>().is_ok_and(|year| year >= 26))
}

pub(crate) fn looks_like_minecraft_release_version(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(major) = segments.next() else {
        return false;
    };
    let Some(minor) = segments.next() else {
        return false;
    };
    if !is_minecraft_release_major(major)
        || minor.is_empty()
        || !minor.chars().all(|ch| ch.is_ascii_digit())
    {
        return false;
    }
    match segments.next() {
        Some(patch) if !patch.is_empty() && patch.chars().all(|ch| ch.is_ascii_digit()) => {
            segments.next().is_none()
        }
        None => true,
        _ => false,
    }
}

pub(crate) fn looks_like_minecraft_pre_release_version(value: &str) -> bool {
    for marker in ["-pre", "-rc", "-snapshot"] {
        if let Some((base, suffix)) = value.split_once(marker) {
            let suffix = suffix.strip_prefix('-').unwrap_or(suffix);
            return looks_like_minecraft_release_version(base)
                && !suffix.is_empty()
                && suffix.chars().all(|ch| ch.is_ascii_digit());
        }
    }
    false
}

pub(crate) fn looks_like_minecraft_snapshot_version(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 6
        && bytes.len() <= 7
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'w'
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && bytes[5].is_ascii_lowercase()
        && bytes.get(6).is_none()
}

pub(crate) fn format_loader_label(modloader: &str, version: &str) -> String {
    let version = version.trim();
    if version.is_empty() {
        modloader.trim().to_owned()
    } else {
        format!("{} {}", modloader.trim(), version)
    }
}
