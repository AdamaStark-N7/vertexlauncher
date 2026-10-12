//! Account names inside text, as *mentions* that are resolved when the text is drawn.
//!
//! Anywhere an account's name goes into a string that will be shown to the user, write it with
//! [`mention`] instead of formatting the name in. A mention carries the account id and the real
//! name through any string handling (notification stacks, sanitizing, console labels) and is
//! only turned into what the user sees by the renderer (`ui::account_label`):
//!
//! - normally, the account's name;
//! - in streamer mode, "Hidden Account", highlighted with the account's chosen color.
//!
//! Because the choice is made at draw time, a name can never be shown by a call site that forgot
//! to redact it, and text created before streamer mode was switched on is still hidden.
//! For anything that cannot draw a mention (logs, plain widgets), use [`plain`].

use std::borrow::Cow;

use crate::privacy::REDACTED_ACCOUNT_LABEL;

// Private-use characters never occur in real names, ids, or ordinary text.
const OPEN: char = '\u{E000}';
const SEPARATOR: char = '\u{E001}';
const CLOSE: char = '\u{E002}';

fn is_marker(ch: char) -> bool {
    matches!(ch, OPEN | SEPARATOR | CLOSE)
}

/// An account to mention: its id (the player UUID, used to find its color) and its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountMention<'a> {
    pub id: &'a str,
    pub name: &'a str,
}

impl AccountMention<'_> {
    /// This mention as text for embedding in a string.
    pub fn token(&self) -> String {
        mention(self.id, self.name)
    }
}

/// Writes an account into a string. `account_id` may be empty when it is not known; the name is
/// still hidden in streamer mode, it just has no color to highlight with.
pub fn mention(account_id: &str, name: &str) -> String {
    let clean = |value: &str| -> String { value.chars().filter(|ch| !is_marker(*ch)).collect() };
    format!(
        "{OPEN}{}{SEPARATOR}{}{CLOSE}",
        clean(account_id),
        clean(name)
    )
}

/// One piece of text: ordinary text, or a mentioned account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    Text(&'a str),
    Account { id: &'a str, name: &'a str },
}

/// Whether `text` contains any mention.
pub fn has_mentions(text: &str) -> bool {
    text.contains(OPEN)
}

/// Splits `text` into text and mentions. Damaged markup (a marker with no partner) never
/// panics: the stray markers are dropped and what remains is treated as ordinary text.
pub fn segments(text: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open_at) = rest.find(OPEN) {
        push_text(&mut out, &rest[..open_at]);
        let after_open = &rest[open_at + OPEN.len_utf8()..];
        let parsed = after_open.find(SEPARATOR).and_then(|separator_at| {
            let id = &after_open[..separator_at];
            let after_separator = &after_open[separator_at + SEPARATOR.len_utf8()..];
            let close_at = after_separator.find(CLOSE)?;
            // A marker inside the id or name means this was not a mention after all.
            let name = &after_separator[..close_at];
            (!id.contains(is_marker) && !name.contains(is_marker)).then_some((
                id,
                name,
                &after_separator[close_at + CLOSE.len_utf8()..],
            ))
        });
        match parsed {
            Some((id, name, remainder)) => {
                out.push(Segment::Account { id, name });
                rest = remainder;
            }
            None => rest = after_open,
        }
    }
    push_text(&mut out, rest);
    out
}

fn push_text<'a>(out: &mut Vec<Segment<'a>>, text: &'a str) {
    // Stray separators and closers from damaged markup are removed by splitting around them.
    for piece in text.split(is_marker).filter(|piece| !piece.is_empty()) {
        out.push(Segment::Text(piece));
    }
}

/// What a mentioned account is called to the user right now.
pub fn display_name(name: &str, streamer_mode: bool) -> &str {
    if streamer_mode {
        REDACTED_ACCOUNT_LABEL
    } else {
        name
    }
}

/// `text` as plain text with every mention resolved: the name normally, "Hidden Account" in
/// streamer mode. For logs and widgets that cannot draw a highlight.
pub fn plain(text: &str, streamer_mode: bool) -> Cow<'_, str> {
    if !text.contains(is_marker) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        segments(text)
            .into_iter()
            .map(|segment| match segment {
                Segment::Text(text) => text,
                Segment::Account { name, .. } => display_name(name, streamer_mode),
            })
            .collect(),
    )
}

/// Applies `transform` to the ordinary text of `text` and leaves every mention untouched, so
/// text sanitizers can run without seeing (and mangling) account ids.
pub fn map_text(text: &str, mut transform: impl FnMut(&str) -> String) -> String {
    if !has_mentions(text) {
        return transform(text);
    }
    let mut out = String::with_capacity(text.len());
    for segment in segments(text) {
        match segment {
            Segment::Text(text) => out.push_str(&transform(text)),
            Segment::Account { id, name } => out.push_str(&mention(id, name)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";

    #[test]
    fn mentions_round_trip_through_segments() {
        let text = format!("Renewing login for {} now", mention(ID, "Steve"));
        assert_eq!(
            segments(&text),
            vec![
                Segment::Text("Renewing login for "),
                Segment::Account {
                    id: ID,
                    name: "Steve"
                },
                Segment::Text(" now"),
            ]
        );
    }

    #[test]
    fn text_without_mentions_is_a_single_segment() {
        assert_eq!(segments("plain text"), vec![Segment::Text("plain text")]);
        assert_eq!(segments(""), Vec::new());
        assert!(!has_mentions("plain text"));
    }

    #[test]
    fn adjacent_and_edge_mentions_parse() {
        let text = format!("{}{}", mention("a", "One"), mention("b", "Two"));
        assert_eq!(
            segments(&text),
            vec![
                Segment::Account {
                    id: "a",
                    name: "One"
                },
                Segment::Account {
                    id: "b",
                    name: "Two"
                },
            ]
        );
    }

    #[test]
    fn plain_resolves_names_or_the_redaction_label() {
        let text = format!("{} took a screenshot!", mention(ID, "Steve"));
        assert_eq!(plain(&text, false), "Steve took a screenshot!");
        assert_eq!(plain(&text, true), "Hidden Account took a screenshot!");
        assert!(matches!(plain("no mentions", true), Cow::Borrowed(_)));
    }

    #[test]
    fn a_name_never_survives_in_streamer_plain_text() {
        let text = format!("{} and {}", mention("1", "Alpha"), mention("2", "Beta"));
        let shown = plain(&text, true);
        assert!(
            !shown.contains("Alpha") && !shown.contains("Beta"),
            "{shown}"
        );
        assert_eq!(shown, "Hidden Account and Hidden Account");
    }

    #[test]
    fn an_unknown_id_still_hides_the_name() {
        let text = format!("Hi {}", mention("", "Steve"));
        assert_eq!(plain(&text, true), "Hi Hidden Account");
        assert_eq!(plain(&text, false), "Hi Steve");
    }

    #[test]
    fn damaged_markup_is_tolerated() {
        for damaged in [
            format!("{OPEN}no separator"),
            format!("{OPEN}id{SEPARATOR}no close"),
            format!("stray {CLOSE} closer"),
            format!("stray {SEPARATOR} separator"),
            format!("{OPEN}{OPEN}id{SEPARATOR}name{CLOSE}"),
        ] {
            let shown = plain(&damaged, false);
            assert!(!shown.contains(is_marker), "{damaged:?} -> {shown:?}");
        }
        // The valid mention inside nested damage is still found.
        let nested = format!("{OPEN}{OPEN}id{SEPARATOR}name{CLOSE}");
        assert_eq!(plain(&nested, false), "name");
    }

    #[test]
    fn markers_in_inputs_cannot_break_the_token() {
        let token = mention(&format!("i{OPEN}d"), &format!("Na{CLOSE}me"));
        assert_eq!(
            segments(&token),
            vec![Segment::Account {
                id: "id",
                name: "Name"
            }]
        );
    }

    #[test]
    fn map_text_leaves_mentions_alone() {
        let text = format!("abc {} def", mention(ID, "Steve"));
        let shouted = map_text(&text, |part| part.to_ascii_uppercase());
        assert_eq!(
            segments(&shouted),
            vec![
                Segment::Text("ABC "),
                Segment::Account {
                    id: ID,
                    name: "Steve"
                },
                Segment::Text(" DEF"),
            ]
        );
        assert_eq!(
            map_text("no mentions", |part| part.to_uppercase()),
            "NO MENTIONS"
        );
    }

    #[test]
    fn account_mention_token_matches_mention() {
        let account = AccountMention {
            id: ID,
            name: "Steve",
        };
        assert_eq!(account.token(), mention(ID, "Steve"));
    }
}
