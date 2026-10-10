use super::*;

pub(crate) fn editor_hit_test(
    editor: &Editor<'static>,
    x: i32,
    y: i32,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Option<Cursor> {
    editor.with_buffer(|buffer| {
        hit_buffer_with_fundamentals(buffer, x as f32, y as f32, fundamentals, scale)
    })
}

pub(crate) fn click_editor_to_pointer(
    editor: &mut Editor<'static>,
    x: i32,
    y: i32,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    *preferred_cursor_x_px = None;
    let old_cursor = editor.cursor();
    let old_selection = editor.selection();
    editor.set_selection(Selection::None);
    if let Some(new_cursor) = editor_hit_test(editor, x, y, fundamentals, scale) {
        editor.set_cursor(new_cursor);
    }
    editor.cursor() != old_cursor || editor.selection() != old_selection
}

pub(crate) fn double_click_editor_to_pointer(
    editor: &mut Editor<'static>,
    x: i32,
    y: i32,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    *preferred_cursor_x_px = None;
    let old_cursor = editor.cursor();
    let old_selection = editor.selection();
    editor.set_selection(Selection::None);
    if let Some(new_cursor) = editor_hit_test(editor, x, y, fundamentals, scale) {
        editor.set_cursor(new_cursor);
        editor.set_selection(Selection::Word(editor.cursor()));
    }
    editor.cursor() != old_cursor || editor.selection() != old_selection
}

pub(crate) fn triple_click_editor_to_pointer(
    editor: &mut Editor<'static>,
    x: i32,
    y: i32,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    *preferred_cursor_x_px = None;
    let old_cursor = editor.cursor();
    let old_selection = editor.selection();
    editor.set_selection(Selection::None);
    if let Some(new_cursor) = editor_hit_test(editor, x, y, fundamentals, scale) {
        editor.set_cursor(new_cursor);
        editor.set_selection(Selection::Line(editor.cursor()));
    }
    editor.cursor() != old_cursor || editor.selection() != old_selection
}

pub(crate) fn drag_editor_selection_to_pointer(
    editor: &mut Editor<'static>,
    x: i32,
    y: i32,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    *preferred_cursor_x_px = None;
    let old_cursor = editor.cursor();
    let old_selection = editor.selection();
    if editor.selection() == Selection::None {
        editor.set_selection(Selection::Normal(editor.cursor()));
    }
    if let Some(new_cursor) = editor_hit_test(editor, x, y, fundamentals, scale) {
        editor.set_cursor(new_cursor);
    }
    editor.cursor() != old_cursor || editor.selection() != old_selection
}

pub(crate) fn extend_selection_to_pointer(
    editor: &mut Editor<'static>,
    x: i32,
    y: i32,
    preferred_cursor_x_px: &mut Option<f32>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> bool {
    *preferred_cursor_x_px = None;
    let anchor = selection_anchor(editor.selection()).unwrap_or_else(|| editor.cursor());
    let Some(new_cursor) = editor_hit_test(editor, x, y, fundamentals, scale) else {
        return false;
    };

    editor.set_cursor(new_cursor);
    if new_cursor == anchor {
        editor.set_selection(Selection::None);
    } else {
        editor.set_selection(Selection::Normal(anchor));
    }
    true
}

pub(crate) fn select_all(editor: &mut Editor<'static>) -> bool {
    let end = editor.with_buffer(|buffer| {
        let Some(line) = buffer.lines.len().checked_sub(1) else {
            return Cursor::new(0, 0);
        };
        Cursor::new(line, buffer.lines[line].text().len())
    });
    editor.set_selection(Selection::Normal(Cursor::new(0, 0)));
    editor.set_cursor(end);
    true
}
