use std::borrow::Cow;
use std::ops::Range;
use std::sync::{Arc, Mutex};

pub const REDACTED_ACCOUNT_LABEL: &str = "Hidden Account";
pub const REDACTED_IDENTIFIER_LABEL: &str = "[redacted]";
const MAX_LOG_TEXT_CHARS: usize = 240;

pub fn redact_account_label<'a>(streamer_mode: bool, label: &'a str) -> Cow<'a, str> {
    if streamer_mode {
        Cow::Borrowed(REDACTED_ACCOUNT_LABEL)
    } else {
        Cow::Borrowed(label)
    }
}

pub fn redact_sensitive_text<'a>(streamer_mode: bool, text: &'a str) -> Cow<'a, str> {
    if !streamer_mode {
        return Cow::Borrowed(text);
    }

    let mut changed = false;
    let mut sanitized_parts = Vec::new();
    for part in text.split_whitespace() {
        let redacted = redact_identifier_token(part);
        if redacted != part {
            changed = true;
        }
        sanitized_parts.push(redacted);
    }

    if !changed {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(sanitized_parts.join(" "))
    }
}

pub fn sanitize_text_for_log(text: &str) -> String {
    log_redaction::sanitize_message_for_log(text, MAX_LOG_TEXT_CHARS)
}

fn redact_identifier_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return token.to_owned();
    }

    let prefix_len = trimmed
        .find(|ch: char| ch.is_ascii_alphanumeric())
        .unwrap_or(trimmed.len());
    let suffix_len = trimmed
        .chars()
        .rev()
        .take_while(|ch| !ch.is_ascii_alphanumeric())
        .count();
    let core_end = trimmed.len().saturating_sub(suffix_len);
    if prefix_len >= core_end {
        return token.to_owned();
    }

    let prefix = &trimmed[..prefix_len];
    let core = &trimmed[prefix_len..core_end];
    let suffix = &trimmed[core_end..];

    if looks_like_sensitive_identifier(core) {
        format!("{prefix}{REDACTED_IDENTIFIER_LABEL}{suffix}")
    } else {
        token.to_owned()
    }
}

fn looks_like_sensitive_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("profile_id=")
        || lower.starts_with("player_uuid=")
        || lower.starts_with("xuid=")
        || lower.starts_with("account_key=")
        || lower.starts_with("display_name=")
        || lower.starts_with("player_name=")
    {
        return true;
    }

    is_hex_identifier(&lower) || is_uuid_identifier(&lower)
}

fn is_hex_identifier(value: &str) -> bool {
    value.len() >= 16 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_uuid_identifier(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 5 {
        return false;
    }
    let expected = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected)
        .all(|(part, len)| part.len() == len && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

/// Whether `ch` can be part of a name (so "Steve" is not found inside "Steven" or "Steve_2").
fn is_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// The byte ranges in `text` that are one of `names_lowercase` as a whole word, ignoring case,
/// with overlaps resolved in favor of the earlier and longer match.
fn name_ranges(text: &str, names_lowercase: &[String]) -> Vec<Range<usize>> {
    // ASCII lowercasing keeps every byte offset the same as in `text`.
    let lowered = text.to_ascii_lowercase();
    let mut found = Vec::new();
    for name in names_lowercase {
        if name.is_empty() {
            continue;
        }
        for (start, _) in lowered.match_indices(name.as_str()) {
            let end = start + name.len();
            let before = text[..start].chars().next_back();
            let after = text[end..].chars().next();
            if !before.is_some_and(is_name_char) && !after.is_some_and(is_name_char) {
                found.push(start..end);
            }
        }
    }
    found.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut kept: Vec<Range<usize>> = Vec::with_capacity(found.len());
    for range in found {
        if kept.last().is_none_or(|last| range.start >= last.end) {
            kept.push(range);
        }
    }
    kept
}

/// `text` with every whole-word occurrence of one of `names_lowercase` (lowercase, ASCII) replaced
/// by the redaction label, or `None` if there is none. The redaction label itself is left alone,
/// so an account named like one of its words cannot garble it.
pub fn hide_account_names(text: &str, names_lowercase: &[String]) -> Option<String> {
    if names_lowercase.is_empty() || text.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for (index, part) in text.split(REDACTED_ACCOUNT_LABEL).enumerate() {
        if index > 0 {
            out.push_str(REDACTED_ACCOUNT_LABEL);
        }
        let mut copied = 0;
        for range in name_ranges(part, names_lowercase) {
            out.push_str(&part[copied..range.start]);
            out.push_str(REDACTED_ACCOUNT_LABEL);
            copied = range.end;
            changed = true;
        }
        out.push_str(&part[copied..]);
    }
    changed.then_some(out)
}

/// Logs, once, that a `without_name_scrubbing` scope drew an account name. The text is not logged.
fn warn_exempt_text_has_name() {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        tracing::warn!(
            target: "vertexlauncher/privacy",
            "Text drawn inside a name-exempt UI scope contained an account name; that scope is not static."
        );
    }
}

/// What the UI is currently hiding: streamer mode and the lowercase names to hide with it.
static HIDDEN_NAMES: Mutex<Option<(bool, Vec<String>)>> = Mutex::new(None);

/// Sets up the last-chance filter that every piece of text drawn by the UI passes through
/// (see `textui_egui::set_text_scrubber`), so account names cannot show in streamer mode
/// whatever produced the text: log lines, error messages, anything.
///
/// Call it every frame with the current streamer mode and the names of every account the
/// launcher knows. It does nothing unless something changed. Stray mention markup (see
/// [`crate::account_text`]) that reaches a plain widget is resolved here as well, to the name or
/// to the redaction label, so the markup itself is never shown.
pub fn update_hidden_names<'a>(streamer_mode: bool, names: impl IntoIterator<Item = &'a str>) {
    let mut names_lowercase: Vec<String> = names
        .into_iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect();
    // Longest first, so "steve_2" is matched before "steve" could claim part of it.
    names_lowercase.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    names_lowercase.dedup();

    let Ok(mut current) = HIDDEN_NAMES.lock() else {
        return;
    };
    let unchanged = current
        .as_ref()
        .is_some_and(|(mode, held)| *mode == streamer_mode && *held == names_lowercase);
    if unchanged {
        return;
    }
    *current = Some((streamer_mode, names_lowercase.clone()));
    drop(current);

    let hidden = Arc::new(names_lowercase);
    textui_egui::set_text_scrubber(Some(Arc::new(move |text: &str| {
        let resolved = crate::account_text::plain(text, streamer_mode);
        if streamer_mode {
            if textui_egui::name_scrubbing_exempt() {
                // The exemption is the caller's promise that this text holds no account name.
                // Debug builds check it, so a wrong exemption is noticed.
                if cfg!(debug_assertions) && hide_account_names(&resolved, &hidden).is_some() {
                    warn_exempt_text_has_name();
                }
            } else if let Some(hidden_text) = hide_account_names(&resolved, &hidden) {
                return Some(hidden_text);
            }
        }
        match resolved {
            Cow::Owned(resolved) => Some(resolved),
            Cow::Borrowed(_) => None,
        }
    })));
}

#[cfg(test)]
mod hidden_name_tests {
    use super::*;

    fn names(values: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = values.iter().map(|v| v.to_ascii_lowercase()).collect();
        out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        out
    }

    #[test]
    fn names_are_hidden_as_whole_words_ignoring_case() {
        let hidden = names(&["Steve"]);
        assert_eq!(
            hide_account_names("Setting user: Steve", &hidden).as_deref(),
            Some("Setting user: Hidden Account")
        );
        assert_eq!(
            hide_account_names("steve and STEVE", &hidden).as_deref(),
            Some("Hidden Account and Hidden Account")
        );
        assert_eq!(
            hide_account_names("(Steve), Steve: Steve's", &hidden).as_deref(),
            Some("(Hidden Account), Hidden Account: Hidden Account's")
        );
    }

    #[test]
    fn longer_names_and_lookalike_words_are_left_alone() {
        let hidden = names(&["Steve"]);
        for text in [
            "Steven", "Steve_2", "mySteve", "Steve2", "Stevé", "no names",
        ] {
            assert_eq!(hide_account_names(text, &hidden), None, "{text}");
        }
    }

    #[test]
    fn overlapping_names_prefer_the_longer_one() {
        let hidden = names(&["Steve", "Steve_2"]);
        assert_eq!(
            hide_account_names("Steve_2 and Steve", &hidden).as_deref(),
            Some("Hidden Account and Hidden Account")
        );
    }

    #[test]
    fn unicode_text_around_names_is_preserved() {
        let hidden = names(&["Alex"]);
        assert_eq!(
            hide_account_names("日本語 Alex ✓ ünï", &hidden).as_deref(),
            Some("日本語 Hidden Account ✓ ünï")
        );
        // Non-ASCII letters next to a name make it part of a longer word.
        assert_eq!(hide_account_names("Alexé", &hidden), None);
    }

    #[test]
    fn the_redaction_label_is_never_garbled() {
        let hidden = names(&["Hidden", "Account"]);
        assert_eq!(hide_account_names("Hidden Account", &hidden), None);
        assert_eq!(
            hide_account_names("Hidden Account met Hidden", &hidden).as_deref(),
            Some("Hidden Account met Hidden Account")
        );
    }

    /// The whole path: the installed rule is what the UI text API draws.
    #[test]
    fn installed_rule_hides_names_and_resolves_stray_markup() {
        use crate::account_text::mention;
        let marked = format!("Hello {}!", mention("id", "Steve"));

        update_hidden_names(true, ["Steve", "Alex_9"]);
        assert_eq!(
            textui_egui::scrubbed("Setting user: Steve (alex_9)"),
            "Setting user: Hidden Account (Hidden Account)"
        );
        assert_eq!(textui_egui::scrubbed(&marked), "Hello Hidden Account!");
        assert_eq!(textui_egui::scrubbed("no names here"), "no names here");

        // Inside an exempt scope names are not guessed at, but markup is still resolved.
        textui_egui::without_name_scrubbing(|| {
            assert_eq!(textui_egui::scrubbed("Play as Steve"), "Play as Steve");
            assert_eq!(textui_egui::scrubbed(&marked), "Hello Hidden Account!");
        });
        assert_eq!(
            textui_egui::scrubbed("Play as Steve"),
            "Play as Hidden Account"
        );

        update_hidden_names(false, ["Steve"]);
        assert_eq!(
            textui_egui::scrubbed("Setting user: Steve"),
            "Setting user: Steve"
        );
        // Not hidden, but the markup is still never shown raw.
        assert_eq!(textui_egui::scrubbed(&marked), "Hello Steve!");
        textui_egui::set_text_scrubber(None);
    }

    #[test]
    fn no_names_means_no_change() {
        assert_eq!(hide_account_names("Steve", &[]), None);
        assert_eq!(hide_account_names("", &names(&["Steve"])), None);
    }
}
