//! Turns egui input events into Minecraft key binding names.

use egui::{Event, Key, PointerButton};

/// Minecraft (1.13+) name of an egui key, if it has one.
pub(super) fn key_name(key: Key) -> Option<String> {
    let name = key.name();
    let simple = match key {
        Key::ArrowUp => "up",
        Key::ArrowDown => "down",
        Key::ArrowLeft => "left",
        Key::ArrowRight => "right",
        Key::Space => "space",
        Key::Enter => "enter",
        Key::Tab => "tab",
        Key::Backspace => "backspace",
        Key::Insert => "insert",
        Key::Delete => "delete",
        Key::Home => "home",
        Key::End => "end",
        Key::PageUp => "page.up",
        Key::PageDown => "page.down",
        Key::Minus => "minus",
        Key::Equals => "equal",
        Key::OpenBracket => "left.bracket",
        Key::CloseBracket => "right.bracket",
        Key::Semicolon => "semicolon",
        Key::Quote => "apostrophe",
        Key::Comma => "comma",
        Key::Period => "period",
        Key::Slash => "slash",
        Key::Backslash => "backslash",
        Key::Backtick => "grave.accent",
        _ => "",
    };
    if !simple.is_empty() {
        return Some(format!("key.keyboard.{simple}"));
    }
    // Letters, digits (`Num5`) and function keys (`F5`).
    let lowered = name.to_ascii_lowercase();
    let lowered = lowered.strip_prefix("num").unwrap_or(&lowered);
    let valid = lowered.len() == 1 && lowered.chars().all(|c| c.is_ascii_alphanumeric())
        || (lowered.starts_with('f')
            && lowered[1..].chars().all(|c| c.is_ascii_digit())
            && lowered.len() > 1);
    valid.then(|| format!("key.keyboard.{lowered}"))
}

pub(super) fn mouse_name(button: PointerButton) -> &'static str {
    match button {
        PointerButton::Primary => "key.mouse.left",
        PointerButton::Secondary => "key.mouse.right",
        PointerButton::Middle => "key.mouse.middle",
        PointerButton::Extra1 => "key.mouse.4",
        PointerButton::Extra2 => "key.mouse.5",
    }
}

/// What the user pressed while capturing.
pub(super) enum Captured {
    Binding(String),
    Cancelled,
}

/// The first press in this frame's events, if any. `Escape` cancels.
pub(super) fn poll_capture(ctx: &egui::Context) -> Option<Captured> {
    ctx.input(|input| {
        input.events.iter().find_map(|event| match event {
            Event::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } => Some(Captured::Cancelled),
            Event::Key {
                key, pressed: true, ..
            } => key_name(*key).map(Captured::Binding),
            Event::PointerButton {
                button,
                pressed: true,
                ..
            } => Some(Captured::Binding(mouse_name(*button).to_owned())),
            _ => None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_minecraft() {
        assert_eq!(key_name(Key::W).as_deref(), Some("key.keyboard.w"));
        assert_eq!(key_name(Key::Num5).as_deref(), Some("key.keyboard.5"));
        assert_eq!(key_name(Key::F11).as_deref(), Some("key.keyboard.f11"));
        assert_eq!(
            key_name(Key::PageDown).as_deref(),
            Some("key.keyboard.page.down")
        );
        assert_eq!(
            key_name(Key::Backtick).as_deref(),
            Some("key.keyboard.grave.accent")
        );
        assert_eq!(mouse_name(PointerButton::Extra1), "key.mouse.4");
    }
}
