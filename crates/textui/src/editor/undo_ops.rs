use super::*;

pub(crate) fn classify_modify_op(event: &TextInputEvent) -> UndoOpKind {
    match event {
        TextInputEvent::Text(t) if !t.is_empty() => UndoOpKind::TextInsert,
        TextInputEvent::Paste(p) if !p.is_empty() => UndoOpKind::Paste,
        TextInputEvent::Cut => UndoOpKind::Cut,
        TextInputEvent::Key {
            key,
            pressed: true,
            modifiers,
        } => {
            let word_delete = (modifiers.alt || modifiers.ctrl || modifiers.mac_cmd)
                && matches!(key, TextKey::Backspace | TextKey::Delete);
            let emacs_delete =
                modifiers.ctrl && matches!(key, TextKey::H | TextKey::K | TextKey::U | TextKey::W);
            if matches!(key, TextKey::Backspace | TextKey::Delete) || word_delete || emacs_delete {
                UndoOpKind::Delete
            } else {
                UndoOpKind::None
            }
        }
        TextInputEvent::PointerButton {
            button: TextPointerButton::Middle,
            pressed: true,
            ..
        } => UndoOpKind::Paste,
        _ => UndoOpKind::None,
    }
}

pub(crate) fn is_navigation_event(event: &TextInputEvent) -> bool {
    matches!(
        event,
        TextInputEvent::Key {
            key: TextKey::Left
                | TextKey::Right
                | TextKey::Up
                | TextKey::Down
                | TextKey::Home
                | TextKey::End
                | TextKey::PageUp
                | TextKey::PageDown,
            pressed: true,
            ..
        }
    )
}

pub(crate) fn pending_modify_op(events: &[TextInputEvent]) -> UndoOpKind {
    events
        .iter()
        .map(classify_modify_op)
        .find(|op| *op != UndoOpKind::None)
        .unwrap_or(UndoOpKind::None)
}

pub(crate) fn push_undo(stack: &mut Vec<UndoEntry>, entry: UndoEntry) {
    if stack.len() >= UNDO_STACK_MAX {
        stack.remove(0);
    }
    stack.push(entry);
}
