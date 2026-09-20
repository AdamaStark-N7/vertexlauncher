//! Minecraft version parsing for feature gating.

/// `(major, minor, patch)` of a release, ordered chronologically across both numbering
/// schemes (`1.21.4` < `26.1`). Alpha/beta/classic versions sort below `1.0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ver(pub u16, pub u16, pub u16);

impl Ver {
    pub const OLDEST: Ver = Ver(0, 0, 0);
    /// Sentinel for "still present" upper bounds.
    pub const NEWEST: Ver = Ver(u16::MAX, 0, 0);
}

/// Parses a game version id. Pre-releases, release candidates and new-style snapshots map to
/// the release they belong to (they already contain that release's options). Returns `None`
/// for old-style weekly snapshots (`24w14a`) and other ids that can't be placed reliably.
pub fn parse_game_version(id: &str) -> Option<Ver> {
    let id = id.trim();
    let first = id.chars().next()?;
    if matches!(first, 'a' | 'b' | 'c') && id[1..].starts_with(|c: char| c.is_ascii_digit()) {
        return Some(Ver::OLDEST);
    }
    if id.starts_with("inf-") || id.starts_with("rd-") {
        return Some(Ver::OLDEST);
    }
    let base = id.split(['-', ' ']).next()?;
    let mut parts = base.split('.');
    let major: u16 = parts.next()?.parse().ok()?;
    let minor: u16 = parts.next()?.parse().ok()?;
    let patch: u16 = match parts.next() {
        Some(part) => part.parse().ok()?,
        None => 0,
    };
    // `1` and `26`+ are the two schemes; anything else (e.g. `2.4.0`) isn't a game version.
    (major == 1 || major >= 26).then_some(Ver(major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_releases_snapshots_and_legacy() {
        assert_eq!(parse_game_version("1.21.4"), Some(Ver(1, 21, 4)));
        assert_eq!(parse_game_version("1.20.5-pre1"), Some(Ver(1, 20, 5)));
        assert_eq!(parse_game_version("26.3-snapshot-10"), Some(Ver(26, 3, 0)));
        assert_eq!(parse_game_version("26.3"), Some(Ver(26, 3, 0)));
        assert_eq!(parse_game_version("b1.7.3"), Some(Ver::OLDEST));
        assert_eq!(parse_game_version("24w14a"), None);
        assert_eq!(parse_game_version("2.4.0"), None);
        assert!(Ver(1, 21, 4) < Ver(26, 1, 0));
    }
}
