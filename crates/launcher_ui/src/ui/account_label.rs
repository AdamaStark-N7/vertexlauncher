//! The renderer for text that mentions accounts (see [`crate::account_text`]).
//!
//! Use [`account_label`] anywhere text that may contain a mention is shown. Text without a
//! mention is drawn as one ordinary label, so it costs nothing to use it everywhere.

use egui::{Color32, Layout, Response, Ui};
use textui::TextUi;
use textui_egui::prelude::*;

use crate::account_text::{self, Segment};
use crate::ui::account_color::{highlighted_name_label, name_highlight};

/// A piece of a chunk: a run of ordinary text without spaces, or a mentioned account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece<'a> {
    Word(&'a str),
    Account { id: &'a str, name: &'a str },
}

/// Splits `text` into chunks that must not be broken across lines. Spaces end a chunk, so
/// words wrap normally; a name and the punctuation touching it stay together ("Steve's",
/// "(Steve)", "Steve: error"), and a name is never split even when it contains a space.
fn chunks(text: &str) -> Vec<Vec<Piece<'_>>> {
    let mut chunks: Vec<Vec<Piece>> = Vec::new();
    let mut current: Vec<Piece> = Vec::new();
    for segment in account_text::segments(text) {
        match segment {
            Segment::Account { id, name } => current.push(Piece::Account { id, name }),
            Segment::Text(text) => {
                let mut rest = text;
                while !rest.is_empty() {
                    let is_space = rest.starts_with(char::is_whitespace);
                    let end = rest
                        .find(|ch: char| ch.is_whitespace() != is_space)
                        .unwrap_or(rest.len());
                    let (run, tail) = rest.split_at(end);
                    if is_space {
                        if !current.is_empty() {
                            chunks.push(std::mem::take(&mut current));
                        }
                    } else {
                        current.push(Piece::Word(run));
                    }
                    rest = tail;
                }
            }
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Draws `text`, which may mention accounts. In `streamer_mode`, mentions read "Hidden
/// Account" highlighted in the account's chosen color; otherwise they read as the name.
///
/// Long text wraps within the available width. With `clickable`, the pieces sense clicks, and
/// the returned response is the union of all of them.
pub fn account_label(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash,
    text: &str,
    options: &LabelOptions,
    streamer_mode: bool,
    clickable: bool,
) -> Response {
    if !account_text::has_mentions(text) {
        return if clickable {
            text_ui.clickable_label(ui, id_source, text, options)
        } else {
            text_ui.label(ui, id_source, text, options)
        };
    }

    // Each piece is its own single-line label, flowed and wrapped by egui.
    let mut options = options.clone();
    options.wrap = false;
    let space_width = space_width(ui, text_ui, &options);
    let id = ui.make_persistent_id(id_source);
    let mut union: Option<Response> = None;
    let flow = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = space_width;
        for (chunk_index, chunk) in chunks(text).iter().enumerate() {
            let response = if let [piece] = chunk.as_slice() {
                piece_label(
                    ui,
                    text_ui,
                    (id, chunk_index, 0usize),
                    *piece,
                    &options,
                    streamer_mode,
                    clickable,
                )
            } else {
                glued_chunk(
                    ui,
                    text_ui,
                    (id, chunk_index),
                    chunk,
                    &options,
                    streamer_mode,
                    clickable,
                )
            };
            union = Some(match union.take() {
                Some(previous) => previous.union(response),
                None => response,
            });
        }
    });
    match union {
        Some(union) => union.union(flow.response),
        None => flow.response,
    }
}

/// Several pieces drawn edge to edge as one unit. Its width is measured first so the wrapping
/// parent moves the whole unit to the next line instead of overflowing.
fn glued_chunk(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id: impl std::hash::Hash + Copy,
    pieces: &[Piece<'_>],
    options: &LabelOptions,
    streamer_mode: bool,
    clickable: bool,
) -> Response {
    let sizes: Vec<egui::Vec2> = pieces
        .iter()
        .map(|piece| text_ui.measure_text_size(ui, piece_text(*piece, streamer_mode), options))
        .collect();
    let width: f32 = sizes.iter().map(|size| size.x).sum();
    let height = sizes.iter().map(|size| size.y).fold(0.0, f32::max);
    ui.allocate_ui_with_layout(
        egui::vec2(width.max(1.0), height.max(1.0)),
        Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let mut union: Option<Response> = None;
            for (index, piece) in pieces.iter().enumerate() {
                let response = piece_label(
                    ui,
                    text_ui,
                    (id, index),
                    *piece,
                    options,
                    streamer_mode,
                    clickable,
                );
                union = Some(match union.take() {
                    Some(previous) => previous.union(response),
                    None => response,
                });
            }
            union
        },
    )
    .inner
    .unwrap_or_else(|| ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover()))
}

fn piece_text<'a>(piece: Piece<'a>, streamer_mode: bool) -> &'a str {
    match piece {
        Piece::Word(word) => word,
        Piece::Account { name, .. } => account_text::display_name(name, streamer_mode),
    }
}

fn piece_label(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id: impl std::hash::Hash,
    piece: Piece<'_>,
    options: &LabelOptions,
    streamer_mode: bool,
    clickable: bool,
) -> Response {
    match piece {
        Piece::Word(word) => {
            if clickable {
                text_ui.clickable_label(ui, id, word, options)
            } else {
                text_ui.label(ui, id, word, options)
            }
        }
        Piece::Account {
            id: account_id,
            name,
        } => {
            let highlight: Option<Color32> = name_highlight(streamer_mode, account_id);
            highlighted_name_label(
                ui,
                text_ui,
                id,
                account_text::display_name(name, streamer_mode),
                options,
                highlight,
                clickable,
            )
        }
    }
}

/// The width of one space in `options`' font, measured from a string that has one in the
/// middle because trailing spaces are not measured.
fn space_width(ui: &Ui, text_ui: &mut TextUi, options: &LabelOptions) -> f32 {
    let with_space = text_ui.measure_text_size(ui, "a a", options).x;
    let without = text_ui.measure_text_size(ui, "aa", options).x;
    let width = with_space - without;
    if width.is_finite() && width > 0.5 {
        width
    } else {
        // Fall back to a typical space (about a third of the line height).
        (options.line_height * 0.3).max(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_text::mention;

    fn words(chunk: &[Piece<'_>]) -> Vec<String> {
        chunk
            .iter()
            .map(|piece| match piece {
                Piece::Word(word) => (*word).to_owned(),
                Piece::Account { name, .. } => format!("<{name}>"),
            })
            .collect()
    }

    fn flat(text: &str) -> Vec<Vec<String>> {
        chunks(text).iter().map(|chunk| words(chunk)).collect()
    }

    #[test]
    fn words_wrap_and_a_name_is_one_unbreakable_chunk() {
        let text = format!("Renewing login for {} now", mention("id", "Hidden Account"));
        assert_eq!(
            flat(&text),
            vec![
                vec!["Renewing"],
                vec!["login"],
                vec!["for"],
                vec!["<Hidden Account>"],
                vec!["now"],
            ]
        );
    }

    #[test]
    fn punctuation_touching_a_name_stays_glued_to_it() {
        let text = format!(
            "({}): {}'s turn",
            mention("1", "Steve"),
            mention("2", "Alex")
        );
        assert_eq!(
            flat(&text),
            vec![
                vec!["(", "<Steve>", "):"],
                vec!["<Alex>", "'s"],
                vec!["turn"],
            ]
        );
    }

    #[test]
    fn leading_and_repeated_whitespace_makes_no_empty_chunks() {
        let text = format!("  {}   took\n a  shot ", mention("1", "Steve"));
        assert_eq!(
            flat(&text),
            vec![vec!["<Steve>"], vec!["took"], vec!["a"], vec!["shot"]]
        );
    }

    #[test]
    fn adjacent_names_are_glued_together() {
        let text = format!("{}{}", mention("1", "A"), mention("2", "B"));
        assert_eq!(flat(&text), vec![vec!["<A>", "<B>"]]);
    }

    #[test]
    fn text_without_mentions_has_ordinary_chunks() {
        assert_eq!(
            flat("just some words"),
            vec![vec!["just"], vec!["some"], vec!["words"]]
        );
        assert!(chunks("   ").is_empty());
    }
}
