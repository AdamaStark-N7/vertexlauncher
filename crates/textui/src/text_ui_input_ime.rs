//! Inline IME composition for text inputs.
//!
//! While an IME is composing, its in-progress ("pre-edit") text is kept in the editor buffer, as
//! egui's own text edit does, so it lays out, scrolls and wraps like any other text. This module
//! tracks exactly where that text sits so it can be replaced on every update, underlined while
//! painted, removed if the composition is abandoned, and kept out of the string the application
//! sees until the IME commits it.

use super::*;

/// Where the IME's pre-edit text currently sits in the editor buffer (always within one line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreeditRange {
    pub(crate) start: Cursor,
    pub(crate) end: Cursor,
}

impl PreeditRange {
    pub(crate) fn bounds(self) -> (Cursor, Cursor) {
        (self.start, self.end)
    }
}

/// Removes the pre-edit text from the editor, leaving the cursor where the composition began.
/// Returns whether anything was removed.
pub(crate) fn clear_preedit(
    editor: &mut Editor<'static>,
    preedit: &mut Option<PreeditRange>,
) -> bool {
    let Some(range) = preedit.take() else {
        return false;
    };
    let start = clamp_cursor_to_editor(editor, range.start);
    let end = clamp_cursor_to_editor(editor, range.end);
    editor.set_selection(Selection::Normal(start));
    editor.set_cursor(end);
    editor.delete_selection();
    true
}

/// Replaces the pre-edit text with `text`. The first update of a composition also replaces
/// whatever was selected, like typing would. Newlines are never part of a composition.
pub(crate) fn set_preedit(
    editor: &mut Editor<'static>,
    preedit: &mut Option<PreeditRange>,
    text: &str,
) -> bool {
    let mut changed = clear_preedit(editor, preedit);
    let text = text.replace(['\n', '\r'], "");
    if text.is_empty() {
        return changed;
    }
    editor.delete_selection();
    let start = editor.cursor();
    editor.insert_string(&text, None);
    *preedit = Some(PreeditRange {
        start,
        end: editor.cursor(),
    });
    changed = true;
    changed
}

/// Ends the composition with its final `text`, which replaces the pre-edit text. Newlines are
/// dropped from single-line inputs.
pub(crate) fn commit_preedit(
    editor: &mut Editor<'static>,
    preedit: &mut Option<PreeditRange>,
    text: &str,
    multiline: bool,
) -> bool {
    let mut changed = clear_preedit(editor, preedit);
    let text = if multiline {
        text.to_owned()
    } else {
        text.replace(['\n', '\r'], "")
    };
    if !text.is_empty() {
        editor.insert_string(&text, None);
        changed = true;
    }
    changed
}

/// The editor's text with any pre-edit text left out: the text the application should see.
pub(crate) fn text_without_preedit(
    editor: &Editor<'static>,
    preedit: Option<PreeditRange>,
) -> String {
    let Some(range) = preedit else {
        return editor_to_string(editor);
    };
    let mut out = String::new();
    editor.with_buffer(|buffer| {
        for (line_index, line) in buffer.lines.iter().enumerate() {
            let text = line.text();
            if line_index == range.start.line
                && range.start.index <= range.end.index
                && range.end.index <= text.len()
                && text.is_char_boundary(range.start.index)
                && text.is_char_boundary(range.end.index)
            {
                out.push_str(&text[..range.start.index]);
                out.push_str(&text[range.end.index..]);
            } else {
                out.push_str(text);
            }
            out.push_str(line.ending().as_str());
        }
    });
    out
}

/// The cursor to report for undo while a composition is active: where it began.
pub(crate) fn cursor_without_preedit(
    editor: &Editor<'static>,
    preedit: Option<PreeditRange>,
) -> Cursor {
    preedit.map_or_else(|| editor.cursor(), |range| range.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An editor over `text` with the cursor at `cursor_index`. The buffer is filled directly:
    /// composition only edits text, so nothing here needs fonts or shaping.
    fn editor_at(text: &str, cursor_index: usize) -> Editor<'static> {
        let mut buffer = Buffer::new_empty(Metrics::new(16.0, 22.0));
        buffer.lines = text
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                let ending = if text.split('\n').count() > index + 1 {
                    cosmic_text::LineEnding::Lf
                } else {
                    cosmic_text::LineEnding::None
                };
                cosmic_text::BufferLine::new(
                    line,
                    ending,
                    cosmic_text::AttrsList::new(&Attrs::new()),
                    Shaping::Advanced,
                )
            })
            .collect();
        let mut editor = Editor::new(buffer);
        editor.set_cursor(Cursor::new(0, cursor_index));
        editor
    }

    #[test]
    fn each_update_replaces_the_previous_preedit_text() {
        let mut editor = editor_at("ab", 2);
        let mut preedit = None;

        assert!(set_preedit(&mut editor, &mut preedit, "に"));
        assert_eq!(editor_to_string(&editor), "abに");
        assert_eq!(text_without_preedit(&editor, preedit), "ab");

        assert!(set_preedit(&mut editor, &mut preedit, "日本"));
        assert_eq!(editor_to_string(&editor), "ab日本");
        assert_eq!(text_without_preedit(&editor, preedit), "ab");
        assert_eq!(
            preedit.map(|range| (range.start.index, range.end.index)),
            Some((2, 2 + "日本".len()))
        );
    }

    #[test]
    fn commit_replaces_the_preedit_with_the_final_text() {
        let mut editor = editor_at("ab", 2);
        let mut preedit = None;
        set_preedit(&mut editor, &mut preedit, "にほん");

        assert!(commit_preedit(&mut editor, &mut preedit, "日本", false));
        assert!(preedit.is_none());
        assert_eq!(editor_to_string(&editor), "ab日本");
        assert_eq!(editor.cursor().index, 2 + "日本".len());
    }

    #[test]
    fn an_abandoned_composition_leaves_the_text_untouched() {
        let mut editor = editor_at("ab", 2);
        let mut preedit = None;
        set_preedit(&mut editor, &mut preedit, "x");

        assert!(clear_preedit(&mut editor, &mut preedit));
        assert_eq!(editor_to_string(&editor), "ab");
        assert_eq!(editor.cursor().index, 2);
        assert!(
            !clear_preedit(&mut editor, &mut preedit),
            "nothing left to clear"
        );
    }

    #[test]
    fn an_empty_preedit_clears_the_composition() {
        let mut editor = editor_at("ab", 2);
        let mut preedit = None;
        set_preedit(&mut editor, &mut preedit, "x");

        assert!(set_preedit(&mut editor, &mut preedit, ""));
        assert!(preedit.is_none());
        assert_eq!(editor_to_string(&editor), "ab");
    }

    #[test]
    fn composition_in_the_middle_keeps_the_surrounding_text() {
        let mut editor = editor_at("abcd", 2);
        let mut preedit = None;

        set_preedit(&mut editor, &mut preedit, "漢字");
        assert_eq!(editor_to_string(&editor), "ab漢字cd");
        assert_eq!(text_without_preedit(&editor, preedit), "abcd");

        commit_preedit(&mut editor, &mut preedit, "漢字", false);
        assert_eq!(editor_to_string(&editor), "ab漢字cd");
    }

    #[test]
    fn starting_a_composition_replaces_the_selection() {
        let mut editor = editor_at("hello", 5);
        editor.set_selection(Selection::Normal(Cursor::new(0, 0)));
        let mut preedit = None;

        set_preedit(&mut editor, &mut preedit, "あ");
        assert_eq!(editor_to_string(&editor), "あ");
        assert_eq!(text_without_preedit(&editor, preedit), "");
    }

    #[test]
    fn newlines_never_enter_a_composition_and_are_dropped_from_single_line_commits() {
        let mut editor = editor_at("", 0);
        let mut preedit = None;

        set_preedit(&mut editor, &mut preedit, "a\nb");
        assert_eq!(editor_to_string(&editor), "ab");

        commit_preedit(&mut editor, &mut preedit, "x\ny", false);
        assert_eq!(editor_to_string(&editor), "xy");
    }

    #[test]
    fn multiline_commits_keep_their_newlines() {
        let mut editor = editor_at("", 0);
        let mut preedit = None;
        commit_preedit(&mut editor, &mut preedit, "x\ny", true);
        assert_eq!(editor_to_string(&editor), "x\ny");
    }
}
