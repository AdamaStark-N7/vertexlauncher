use std::cmp::Ordering;

/// Orders release versions in either scheme (`1.20.4`, `26.3`); `None` for snapshots and
/// anything else that can't be compared reliably.
pub fn compare_game_versions(a: &str, b: &str) -> Option<Ordering> {
    if a.trim() == b.trim() {
        return Some(Ordering::Equal);
    }
    Some(parse_release(a)?.cmp(&parse_release(b)?))
}

fn parse_release(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = match parts.next() {
        Some(part) => part.parse().ok()?,
        None => 0,
    };
    parts.next().is_none().then_some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_both_schemes() {
        assert_eq!(
            compare_game_versions("1.20.1", "1.21"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_game_versions("26.3", "1.21.4"),
            Some(Ordering::Greater)
        );
        assert_eq!(compare_game_versions("26.1", "26.1"), Some(Ordering::Equal));
        assert_eq!(compare_game_versions("24w14a", "1.21"), None);
        assert_eq!(
            compare_game_versions("24w14a", "24w14a"),
            Some(Ordering::Equal)
        );
    }
}
