use super::*;

impl UndoEntry {
    /// Snapshots the editor's current text, cursor and selection.
    pub(super) fn capture(editor: &Editor<'static>) -> Self {
        Self {
            text: editor_to_string(editor),
            cursor: editor.cursor(),
            selection: editor.selection(),
        }
    }
}

/// Which stack an undo/redo step pops from.
#[derive(Clone, Copy)]
pub(super) enum UndoDirection {
    Undo,
    Redo,
}

impl TextUi {
    /// Pops one entry from the undo (or redo) stack and restores it into the editor.
    /// Returns true when an entry was applied.
    pub(super) fn apply_undo_step(
        &mut self,
        state: &mut InputState,
        direction: UndoDirection,
        options: &InputOptions,
        multiline: bool,
        content_size_px: [f32; 2],
        scale: f32,
    ) -> bool {
        let popped = match direction {
            UndoDirection::Undo => state.undo_stack.pop(),
            UndoDirection::Redo => state.redo_stack.pop(),
        };
        let Some(UndoEntry {
            text: restored_text,
            cursor: restored_cursor,
            selection: restored_selection,
        }) = popped
        else {
            return false;
        };

        let snap = UndoEntry::capture(&state.editor);
        match direction {
            UndoDirection::Undo => state.redo_stack.push(snap),
            UndoDirection::Redo => push_undo(&mut state.undo_stack, snap),
        }
        state.scroll_metrics = self.replace_editor_text(
            &mut state.editor,
            &restored_text,
            options,
            multiline,
            content_size_px[0],
            content_size_px[1],
            scale,
        );
        state
            .editor
            .set_cursor(clamp_cursor_to_editor(&state.editor, restored_cursor));
        state
            .editor
            .set_selection(clamp_selection_to_editor(&state.editor, restored_selection));
        state.last_text = restored_text;
        state.preedit = None;
        state.last_undo_op = UndoOpKind::None;
        state.preferred_cursor_x_px = None;
        true
    }
}
