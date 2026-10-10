//! Per-script font choice for CJK text and emoji.
//!
//! A text group (button labels, body text, ...) names one main font, but CJK characters and
//! emoji are usually drawn from a different font. cosmic-text only has one global fallback
//! chain, so a group cannot say "use *this* CJK font" through it. Instead, text is split into
//! runs by script and each CJK or emoji run is shaped with its own family:
//!
//! 1. [`split_runs`] classifies each grapheme cluster as CJK, emoji or neither.
//! 2. [`ScriptFonts`] picks the family for each class: the group's own choice if it has one,
//!    otherwise the launcher-wide preference, otherwise nothing (cosmic-text's fallback).
//! 3. [`ScriptFonts::split_spans`] and [`ScriptFonts::attrs_list`] apply those families to rich
//!    spans and to editor lines.
//!
//! Splitting never changes the text, only the attributes of ranges of it, so cursor positions,
//! selections and IME ranges are unaffected.

use std::ops::Range;

use cosmic_text::{AttrsList, AttrsOwned, FamilyOwned};
use unicode_segmentation::UnicodeSegmentation;

/// A script the launcher can pick a dedicated font for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ScriptClass {
    Cjk,
    Emoji,
}

/// Characters with default emoji presentation (single code points and the blocks that are almost
/// entirely emoji). Text-default symbols such as a bare "✔" are left to the main font unless an
/// emoji variation selector follows them.
const EMOJI_RANGES: &[(u32, u32)] = &[
    (0x231A, 0x231B),
    (0x23E9, 0x23EC),
    (0x23F0, 0x23F0),
    (0x23F3, 0x23F3),
    (0x25FD, 0x25FE),
    (0x2614, 0x2615),
    (0x2648, 0x2653),
    (0x267F, 0x267F),
    (0x2693, 0x2693),
    (0x26A1, 0x26A1),
    (0x26AA, 0x26AB),
    (0x26BD, 0x26BE),
    (0x26C4, 0x26C5),
    (0x26CE, 0x26CE),
    (0x26D4, 0x26D4),
    (0x26EA, 0x26EA),
    (0x26F2, 0x26F3),
    (0x26F5, 0x26F5),
    (0x26FA, 0x26FA),
    (0x26FD, 0x26FD),
    (0x2705, 0x2705),
    (0x270A, 0x270B),
    (0x2728, 0x2728),
    (0x274C, 0x274C),
    (0x274E, 0x274E),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2795, 0x2797),
    (0x27B0, 0x27B0),
    (0x27BF, 0x27BF),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B50),
    (0x2B55, 0x2B55),
    (0x1F004, 0x1F004),
    (0x1F0CF, 0x1F0CF),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F1E6, 0x1F1FF),
    (0x1F300, 0x1F64F),
    (0x1F680, 0x1F6FF),
    (0x1F900, 0x1F9FF),
    (0x1FA70, 0x1FAFF),
];

/// Hangul, kana, Han ideographs, their punctuation, and the full-width forms used alongside them.
const CJK_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x11FF),
    (0x2E80, 0x2FDF),
    (0x3000, 0x303F),
    (0x3040, 0x309F),
    (0x30A0, 0x30FF),
    (0x3100, 0x312F),
    (0x3130, 0x318F),
    (0x3190, 0x31FF),
    (0x3200, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA960, 0xA97F),
    (0xAC00, 0xD7FF),
    (0xF900, 0xFAFF),
    (0xFE30, 0xFE4F),
    (0xFF00, 0xFFEF),
    (0x1B000, 0x1B16F),
    (0x20000, 0x2FA1F),
    (0x30000, 0x323AF),
];

const VARIATION_SELECTOR_EMOJI: char = '\u{FE0F}';
const ZERO_WIDTH_JOINER: char = '\u{200D}';
const COMBINING_KEYCAP: char = '\u{20E3}';

fn in_ranges(ranges: &[(u32, u32)], c: char) -> bool {
    let code = c as u32;
    ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&code))
}

/// The script of one grapheme cluster, or `None` for anything the main font should draw.
fn classify_cluster(cluster: &str) -> Option<ScriptClass> {
    let first = cluster.chars().next()?;
    // Emoji forms that start with an ASCII character: keycaps ("1️⃣") and "©️"-style sequences.
    let marks_emoji = cluster
        .chars()
        .any(|c| matches!(c, VARIATION_SELECTOR_EMOJI | COMBINING_KEYCAP));
    if first.is_ascii() {
        return (cluster.len() > 1 && marks_emoji).then_some(ScriptClass::Emoji);
    }
    if marks_emoji || cluster.contains(ZERO_WIDTH_JOINER) || in_ranges(EMOJI_RANGES, first) {
        return Some(ScriptClass::Emoji);
    }
    in_ranges(CJK_RANGES, first).then_some(ScriptClass::Cjk)
}

/// Covers all of `text` with consecutive runs, merging neighbouring clusters of the same script.
pub(crate) fn split_runs(text: &str) -> Vec<(Range<usize>, Option<ScriptClass>)> {
    let mut runs: Vec<(Range<usize>, Option<ScriptClass>)> = Vec::new();
    for (start, cluster) in text.grapheme_indices(true) {
        let class = classify_cluster(cluster);
        let end = start + cluster.len();
        match runs.last_mut() {
            Some((range, last_class)) if *last_class == class => range.end = end,
            _ => runs.push((start..end, class)),
        }
    }
    runs
}

/// Whether `text` has anything that could need a script-specific font. ASCII never does, which
/// keeps the common case free of any splitting work.
pub(crate) fn may_need_script_fonts(text: &str) -> bool {
    !text.is_ascii()
}

/// The family to use for each script, already resolved from the group's choice and the
/// launcher-wide preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScriptFonts<'a> {
    pub(crate) cjk: Option<&'a str>,
    pub(crate) emoji: Option<&'a str>,
}

impl<'a> ScriptFonts<'a> {
    /// The group's own choice, falling back to the launcher-wide preference.
    pub(crate) fn resolve(
        group_cjk: Option<&'a str>,
        group_emoji: Option<&'a str>,
        global_cjk: Option<&'a str>,
        global_emoji: Option<&'a str>,
    ) -> Self {
        Self {
            cjk: group_cjk.or(global_cjk),
            emoji: group_emoji.or(global_emoji),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.cjk.is_none() && self.emoji.is_none()
    }

    fn family_for(&self, class: ScriptClass) -> Option<&'a str> {
        match class {
            ScriptClass::Cjk => self.cjk,
            ScriptClass::Emoji => self.emoji,
        }
    }

    /// `base` with the family for `class` swapped in, when that script has a chosen font.
    pub(crate) fn attrs_for(&self, base: &AttrsOwned, class: Option<ScriptClass>) -> AttrsOwned {
        let mut attrs = base.clone();
        if let Some(family) = class.and_then(|class| self.family_for(class)) {
            attrs.family_owned = FamilyOwned::Name(family.into());
        }
        attrs
    }

    /// Splits each `(text, attrs)` span at script boundaries and gives CJK and emoji runs their
    /// chosen family. Returns the spans unchanged (and `false`) when nothing needed to change.
    pub(crate) fn split_spans<'t>(
        &self,
        spans: &[(&'t str, AttrsOwned)],
    ) -> (Vec<(&'t str, AttrsOwned)>, bool) {
        if self.is_empty() || spans.iter().all(|(text, _)| !may_need_script_fonts(text)) {
            return (spans.to_vec(), false);
        }
        let mut out = Vec::with_capacity(spans.len());
        let mut changed = false;
        for (text, base) in spans {
            if !may_need_script_fonts(text) {
                out.push((*text, base.clone()));
                continue;
            }
            for (range, class) in split_runs(text) {
                let attrs = self.attrs_for(base, class);
                changed |= attrs != *base;
                out.push((&text[range], attrs));
            }
        }
        if changed {
            (out, true)
        } else {
            // Every run kept its font, so splitting would only fragment the layout.
            (spans.to_vec(), false)
        }
    }

    /// The attribute list for one editor line: `base` everywhere, with CJK and emoji runs
    /// switched to their chosen families.
    pub(crate) fn attrs_list(&self, line_text: &str, base: &AttrsOwned) -> AttrsList {
        let mut list = AttrsList::new(&base.as_attrs());
        if self.is_empty() || !may_need_script_fonts(line_text) {
            return list;
        }
        for (range, class) in split_runs(line_text) {
            let attrs = self.attrs_for(base, class);
            if attrs != *base {
                list.add_span(range, &attrs.as_attrs());
            }
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Attrs, Family};

    fn classes(text: &str) -> Vec<(&str, Option<ScriptClass>)> {
        split_runs(text)
            .into_iter()
            .map(|(range, class)| (&text[range], class))
            .collect()
    }

    fn base() -> AttrsOwned {
        AttrsOwned::new(&Attrs::new().family(Family::Name("Main")))
    }

    fn family_of(attrs: &AttrsOwned) -> String {
        match &attrs.family_owned {
            FamilyOwned::Name(name) => name.to_string(),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn plain_ascii_is_one_unclassified_run() {
        assert_eq!(classes("hello world"), vec![("hello world", None)]);
        assert!(!may_need_script_fonts("hello world"));
        assert_eq!(classes(""), vec![]);
    }

    #[test]
    fn cjk_text_is_split_from_latin_text() {
        assert_eq!(
            classes("Play 日本語 now"),
            vec![
                ("Play ", None),
                ("日本語", Some(ScriptClass::Cjk)),
                (" now", None)
            ]
        );
        assert_eq!(classes("こんにちは")[0].1, Some(ScriptClass::Cjk));
        assert_eq!(classes("한국어")[0].1, Some(ScriptClass::Cjk));
        // Full-width punctuation and digits belong with the CJK text around them.
        assert_eq!(classes("「中文」")[0].1, Some(ScriptClass::Cjk));
    }

    #[test]
    fn emoji_are_recognised_including_sequences() {
        assert_eq!(classes("😀")[0].1, Some(ScriptClass::Emoji));
        assert_eq!(classes("👨‍👩‍👧")[0].1, Some(ScriptClass::Emoji), "ZWJ family");
        assert_eq!(classes("👍🏽")[0].1, Some(ScriptClass::Emoji), "skin tone");
        assert_eq!(classes("🇯🇵")[0].1, Some(ScriptClass::Emoji), "flag");
        assert_eq!(classes("1️⃣")[0].1, Some(ScriptClass::Emoji), "keycap");
        assert_eq!(
            classes("✔️")[0].1,
            Some(ScriptClass::Emoji),
            "with emoji selector"
        );
        assert_eq!(classes("⭐")[0].1, Some(ScriptClass::Emoji));
    }

    #[test]
    fn text_symbols_and_plain_digits_stay_with_the_main_font() {
        assert_eq!(classes("✔")[0].1, None, "no emoji selector");
        assert_eq!(classes("12")[0].1, None);
        assert_eq!(classes("é")[0].1, None);
        assert_eq!(classes("→")[0].1, None);
    }

    #[test]
    fn mixed_text_alternates_runs_in_order() {
        assert_eq!(
            classes("a日b😀c"),
            vec![
                ("a", None),
                ("日", Some(ScriptClass::Cjk)),
                ("b", None),
                ("😀", Some(ScriptClass::Emoji)),
                ("c", None),
            ]
        );
    }

    #[test]
    fn runs_cover_the_whole_text_without_gaps() {
        let text = "ab日本👍🏽語c ";
        let runs = split_runs(text);
        assert_eq!(runs.first().unwrap().0.start, 0);
        assert_eq!(runs.last().unwrap().0.end, text.len());
        for pair in runs.windows(2) {
            assert_eq!(pair[0].0.end, pair[1].0.start);
        }
    }

    #[test]
    fn a_groups_choice_beats_the_global_preference() {
        let fonts = ScriptFonts::resolve(
            Some("Group CJK"),
            None,
            Some("Global CJK"),
            Some("Global Emoji"),
        );
        assert_eq!(fonts.cjk, Some("Group CJK"));
        assert_eq!(fonts.emoji, Some("Global Emoji"));
        assert!(ScriptFonts::default().is_empty());
    }

    #[test]
    fn spans_get_per_script_families_and_keep_the_rest() {
        let fonts = ScriptFonts {
            cjk: Some("CJK Font"),
            emoji: Some("Emoji Font"),
        };
        let (spans, changed) = fonts.split_spans(&[("Hi 日本 😀!", base())]);
        assert!(changed);
        let pieces: Vec<(&str, String)> = spans
            .iter()
            .map(|(text, attrs)| (*text, family_of(attrs)))
            .collect();
        assert_eq!(
            pieces,
            vec![
                ("Hi ", "Main".to_owned()),
                ("日本", "CJK Font".to_owned()),
                (" ", "Main".to_owned()),
                ("😀", "Emoji Font".to_owned()),
                ("!", "Main".to_owned()),
            ]
        );
    }

    #[test]
    fn unchanged_text_is_not_split() {
        let fonts = ScriptFonts {
            cjk: Some("CJK Font"),
            emoji: None,
        };
        // No CJK in the text, and emoji has no chosen font, so nothing changes.
        let (spans, changed) = fonts.split_spans(&[("Hello 😀", base())]);
        assert!(!changed);
        assert_eq!(spans.len(), 1);
        let (spans, changed) = ScriptFonts::default().split_spans(&[("日本", base())]);
        assert!(!changed);
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn editor_lines_get_attribute_spans_only_where_needed() {
        let fonts = ScriptFonts {
            cjk: Some("CJK Font"),
            emoji: None,
        };
        let line = "ab日本cd";
        let list = fonts.attrs_list(line, &base());
        assert_eq!(family_of(&AttrsOwned::new(&list.get_span(0))), "Main");
        let cjk_start = "ab".len();
        assert_eq!(
            family_of(&AttrsOwned::new(&list.get_span(cjk_start))),
            "CJK Font"
        );
        let after = "ab日本".len();
        assert_eq!(family_of(&AttrsOwned::new(&list.get_span(after))), "Main");
    }

    const MAIN_FONT: &[u8] =
        include_bytes!("../../vertexlauncher/src/included_fonts/MapleMono-NF-Regular.ttf");
    const CJK_FONT: &[u8] =
        include_bytes!("../../vertexlauncher/src/included_fonts/MapleMono-NF-CN-Regular.ttf");

    /// Shapes `text` with the real embedded fonts and returns the font that drew each character.
    fn shaped_fonts(fonts: ScriptFonts<'_>, text: &str) -> Vec<(char, String)> {
        use cosmic_text::{Buffer, FontSystem, Metrics, Shaping};

        let mut system = FontSystem::new_with_locale_and_db(
            "en-US".to_owned(),
            cosmic_text::fontdb::Database::new(),
        );
        system.db_mut().load_font_data(MAIN_FONT.to_vec());
        system.db_mut().load_font_data(CJK_FONT.to_vec());

        let base = AttrsOwned::new(&Attrs::new().family(Family::Name("Maple Mono NF")));
        let (spans, _) = fonts.split_spans(&[(text, base.clone())]);
        let rich: Vec<(&str, Attrs)> = spans
            .iter()
            .map(|(text, attrs)| (*text, attrs.as_attrs()))
            .collect();

        let mut buffer = Buffer::new(&mut system, Metrics::new(16.0, 22.0));
        buffer.set_rich_text(&mut system, rich, &base.as_attrs(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut system, true);

        let mut drawn = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let family = system
                    .db()
                    .face(glyph.font_id)
                    .map(|face| face.families[0].0.clone())
                    .unwrap_or_default();
                let character = run.text[glyph.start..glyph.end].chars().next().unwrap();
                drawn.push((character, family));
            }
        }
        drawn
    }

    #[test]
    fn cjk_runs_are_drawn_by_the_chosen_cjk_font_and_the_rest_by_the_main_font() {
        let drawn = shaped_fonts(
            ScriptFonts {
                cjk: Some("Maple Mono NF CN"),
                emoji: None,
            },
            "A中あB",
        );
        let family_of_char = |wanted: char| {
            drawn
                .iter()
                .find(|(character, _)| *character == wanted)
                .map(|(_, family)| family.as_str())
        };
        assert_eq!(family_of_char('A'), Some("Maple Mono NF"));
        assert_eq!(family_of_char('B'), Some("Maple Mono NF"));
        assert_eq!(family_of_char('中'), Some("Maple Mono NF CN"));
        assert_eq!(family_of_char('あ'), Some("Maple Mono NF CN"));
    }

    #[test]
    fn the_embedded_cjk_font_covers_chinese_and_japanese_but_not_hangul() {
        let font = skrifa::FontRef::new(CJK_FONT).expect("embedded CJK font parses");
        let charmap = skrifa::MetadataProvider::charmap(&font);
        assert!(charmap.map('中').is_some(), "Han");
        assert!(charmap.map('あ').is_some(), "hiragana");
        assert!(charmap.map('ア').is_some(), "katakana");
        assert!(charmap.map('한').is_none(), "Hangul is not in the CN build");
    }
}
