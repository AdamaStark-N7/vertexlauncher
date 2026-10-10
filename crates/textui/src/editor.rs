use super::*;

#[path = "editor/keys.rs"]
mod keys;
#[path = "editor/pointer.rs"]
mod pointer;
#[path = "editor/scroll_metrics.rs"]
mod scroll_metrics;
#[path = "editor/undo_ops.rs"]
mod undo_ops;

pub(super) use self::keys::*;
pub(super) use self::pointer::*;
pub(super) use self::scroll_metrics::*;
pub(super) use self::undo_ops::*;

/// Coarse operation kind used to group consecutive edits into a single undo
/// entry (so typing a word is one undo step rather than per-char).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum UndoOpKind {
    #[default]
    None,
    TextInsert,
    Delete,
    Paste,
    Cut,
}

#[derive(Clone, Debug)]
pub(super) struct UndoEntry {
    pub(super) text: String,
    pub(super) cursor: Cursor,
    pub(super) selection: Selection,
}

#[derive(Debug)]
pub(super) struct InputState {
    pub(super) editor: Editor<'static>,
    pub(super) last_text: String,
    pub(super) attrs_fingerprint: u64,
    pub(super) multiline: bool,
    pub(super) preferred_cursor_x_px: Option<f32>,
    pub(super) scroll_metrics: EditorScrollMetrics,
    pub(super) last_used_frame: u64,
    pub(super) undo_stack: Vec<UndoEntry>,
    pub(super) redo_stack: Vec<UndoEntry>,
    pub(super) last_undo_op: UndoOpKind,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct EditorScrollMetrics {
    pub(super) current_horizontal_scroll_px: f32,
    pub(super) max_horizontal_scroll_px: f32,
    pub(super) current_vertical_scroll_px: f32,
    pub(super) max_vertical_scroll_px: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ViewerScrollbarTracks {
    pub(super) horizontal: Option<Rect>,
    pub(super) vertical: Option<Rect>,
}

impl ViewerScrollbarTracks {
    pub(super) fn contains(self, pos: Pos2) -> bool {
        self.horizontal.is_some_and(|rect| rect.contains(pos))
            || self.vertical.is_some_and(|rect| rect.contains(pos))
    }
}

pub(super) fn editor_to_string(editor: &Editor<'static>) -> String {
    let mut out = String::new();
    editor.with_buffer(|buffer| {
        for line in &buffer.lines {
            out.push_str(line.text());
            out.push_str(line.ending().as_str());
        }
    });
    out
}

pub(super) fn editor_horizontal_scroll(editor: &Editor<'static>) -> f32 {
    editor.with_buffer(|buffer| buffer.scroll().horizontal.max(0.0))
}

pub(super) fn clamp_cursor_to_editor(editor: &Editor<'static>, cursor: Cursor) -> Cursor {
    editor.with_buffer(|buffer| {
        let line_index = cursor.line.min(buffer.lines.len().saturating_sub(1));
        let line = &buffer.lines[line_index];
        Cursor::new_with_affinity(
            line_index,
            cursor.index.min(line.text().len()),
            cursor.affinity,
        )
    })
}

pub(super) fn clamp_selection_to_editor(
    editor: &Editor<'static>,
    selection: Selection,
) -> Selection {
    match selection {
        Selection::None => Selection::None,
        Selection::Normal(cursor) => Selection::Normal(clamp_cursor_to_editor(editor, cursor)),
        Selection::Line(cursor) => Selection::Line(clamp_cursor_to_editor(editor, cursor)),
        Selection::Word(cursor) => Selection::Word(clamp_cursor_to_editor(editor, cursor)),
    }
}

pub(super) fn selection_anchor(selection: Selection) -> Option<Cursor> {
    match selection {
        Selection::None => None,
        Selection::Normal(cursor) | Selection::Line(cursor) | Selection::Word(cursor) => {
            Some(cursor)
        }
    }
}

pub(super) fn cursor_x_for_layout_cursor(
    buffer: &mut Buffer,
    font_system: &mut FontSystem,
    cursor: Cursor,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Option<f32> {
    let layout_cursor = buffer.layout_cursor(font_system, cursor)?;
    let line_text = buffer.lines.get(layout_cursor.line)?.text().to_owned();
    let layout = buffer.line_layout(font_system, layout_cursor.line)?;
    let layout_line = layout.get(layout_cursor.layout).or_else(|| layout.last())?;
    let stops = cursor_stops_for_glyphs(
        layout_cursor.line,
        &line_text,
        &layout_line.glyphs,
        fundamentals,
        scale,
    );
    stops
        .into_iter()
        .find(|(stop_cursor, _)| {
            stop_cursor.line == cursor.line && stop_cursor.index == cursor.index
        })
        .map(|(_, x)| x)
        .or_else(|| layout_line.glyphs.is_empty().then_some(0.0))
}

pub(super) fn cursor_for_layout_line_x(
    buffer: &mut Buffer,
    font_system: &mut FontSystem,
    line_i: usize,
    layout_i: usize,
    desired_x: f32,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Option<Cursor> {
    let line_text = buffer.lines.get(line_i)?.text().to_owned();
    let layout = buffer.line_layout(font_system, line_i)?;
    let layout_line = layout.get(layout_i).or_else(|| layout.last())?;
    let stops =
        cursor_stops_for_glyphs(line_i, &line_text, &layout_line.glyphs, fundamentals, scale);

    if let Some((first_cursor, first_x)) = stops.first().copied()
        && desired_x <= first_x
    {
        return Some(first_cursor);
    }

    for window in stops.windows(2) {
        let (left_cursor, left_x) = window[0];
        let (right_cursor, right_x) = window[1];
        let mid_x = (left_x + right_x) * 0.5;
        if desired_x <= mid_x {
            return Some(left_cursor);
        }
        if desired_x <= right_x {
            return Some(right_cursor);
        }
    }

    stops
        .last()
        .map(|(cursor, _)| *cursor)
        .or_else(|| Some(Cursor::new_with_affinity(line_i, 0, Affinity::After)))
}

pub(super) fn adjacent_visual_layout_position(
    buffer: &mut Buffer,
    font_system: &mut FontSystem,
    cursor: Cursor,
    direction: i32,
) -> Option<(usize, usize)> {
    let mut layout_cursor = buffer.layout_cursor(font_system, cursor)?;
    match direction.cmp(&0) {
        Ordering::Less => {
            if layout_cursor.layout > 0 {
                layout_cursor.layout -= 1;
            } else if layout_cursor.line > 0 {
                layout_cursor.line -= 1;
                let layout_count = buffer.line_layout(font_system, layout_cursor.line)?.len();
                layout_cursor.layout = layout_count.saturating_sub(1);
            } else {
                return None;
            }
        }
        Ordering::Greater => {
            let layout_count = buffer.line_layout(font_system, layout_cursor.line)?.len();
            if layout_cursor.layout + 1 < layout_count {
                layout_cursor.layout += 1;
            } else if layout_cursor.line + 1 < buffer.lines.len() {
                layout_cursor.line += 1;
                layout_cursor.layout = 0;
            } else {
                return None;
            }
        }
        Ordering::Equal => return Some((layout_cursor.line, layout_cursor.layout)),
    }
    Some((layout_cursor.line, layout_cursor.layout))
}

pub(super) fn move_cursor_one_visual_line(
    font_system: &mut FontSystem,
    editor: &mut Editor<'static>,
    direction: i32,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    let current_cursor = editor.cursor();
    let desired_x = preferred_cursor_x_px.unwrap_or_else(|| {
        editor
            .with_buffer_mut(|buffer| {
                cursor_x_for_layout_cursor(buffer, font_system, current_cursor, fundamentals, scale)
            })
            .unwrap_or(0.0)
    });
    *preferred_cursor_x_px = Some(desired_x);

    let Some((target_line, target_layout)) = editor.with_buffer_mut(|buffer| {
        adjacent_visual_layout_position(buffer, font_system, current_cursor, direction)
    }) else {
        return false;
    };

    let Some(new_cursor) = editor.with_buffer_mut(|buffer| {
        cursor_for_layout_line_x(
            buffer,
            font_system,
            target_line,
            target_layout,
            desired_x,
            fundamentals,
            scale,
        )
    }) else {
        return false;
    };

    if new_cursor != current_cursor {
        editor.set_cursor(new_cursor);
        true
    } else {
        false
    }
}

pub(super) fn handle_spacing_aware_vertical_motion(
    font_system: &mut FontSystem,
    editor: &mut Editor<'static>,
    motion: Motion,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    match motion {
        Motion::Up => move_cursor_one_visual_line(
            font_system,
            editor,
            -1,
            preferred_cursor_x_px,
            fundamentals,
            scale,
        ),
        Motion::Down => move_cursor_one_visual_line(
            font_system,
            editor,
            1,
            preferred_cursor_x_px,
            fundamentals,
            scale,
        ),
        Motion::PageUp | Motion::PageDown | Motion::Vertical(_) => {
            let step_count = editor.with_buffer(|buffer| match motion {
                Motion::PageUp => buffer
                    .size()
                    .1
                    .map(|height| -(height as i32 / buffer.metrics().line_height as i32))
                    .unwrap_or(0),
                Motion::PageDown => buffer
                    .size()
                    .1
                    .map(|height| height as i32 / buffer.metrics().line_height as i32)
                    .unwrap_or(0),
                Motion::Vertical(px) => px / buffer.metrics().line_height as i32,
                _ => 0,
            });
            let direction = step_count.signum();
            let mut moved = false;
            for _ in 0..step_count.unsigned_abs() {
                if !move_cursor_one_visual_line(
                    font_system,
                    editor,
                    direction,
                    preferred_cursor_x_px,
                    fundamentals,
                    scale,
                ) {
                    break;
                }
                moved = true;
            }
            moved
        }
        _ => false,
    }
}
