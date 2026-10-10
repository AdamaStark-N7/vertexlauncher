use super::*;

fn text_modifiers(modifiers: egui::Modifiers) -> TextModifiers {
    TextModifiers {
        alt: modifiers.alt,
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        command: modifiers.command,
        mac_cmd: modifiers.mac_cmd,
    }
}

fn text_key(key: egui::Key) -> Option<TextKey> {
    Some(match key {
        egui::Key::A => TextKey::A,
        egui::Key::ArrowDown => TextKey::Down,
        egui::Key::ArrowLeft => TextKey::Left,
        egui::Key::ArrowRight => TextKey::Right,
        egui::Key::ArrowUp => TextKey::Up,
        egui::Key::B => TextKey::B,
        egui::Key::Backspace => TextKey::Backspace,
        egui::Key::Delete => TextKey::Delete,
        egui::Key::E => TextKey::E,
        egui::Key::End => TextKey::End,
        egui::Key::Enter => TextKey::Enter,
        egui::Key::Escape => TextKey::Escape,
        egui::Key::F => TextKey::F,
        egui::Key::H => TextKey::H,
        egui::Key::Home => TextKey::Home,
        egui::Key::K => TextKey::K,
        egui::Key::N => TextKey::N,
        egui::Key::P => TextKey::P,
        egui::Key::PageDown => TextKey::PageDown,
        egui::Key::PageUp => TextKey::PageUp,
        egui::Key::Tab => TextKey::Tab,
        egui::Key::U => TextKey::U,
        egui::Key::W => TextKey::W,
        egui::Key::Y => TextKey::Y,
        egui::Key::Z => TextKey::Z,
        _ => return None,
    })
}

fn text_pointer_button(button: egui::PointerButton) -> TextPointerButton {
    match button {
        egui::PointerButton::Primary => TextPointerButton::Primary,
        egui::PointerButton::Secondary => TextPointerButton::Secondary,
        egui::PointerButton::Middle => TextPointerButton::Middle,
        egui::PointerButton::Extra1 => TextPointerButton::Extra1,
        egui::PointerButton::Extra2 => TextPointerButton::Extra2,
    }
}

pub(super) fn translate_input_events(events: &[egui::Event]) -> Vec<TextInputEvent> {
    events
        .iter()
        .filter_map(|event| match event {
            egui::Event::Text(text) => Some(TextInputEvent::Text(text.clone())),
            egui::Event::Ime(ime) => match ime {
                egui::ImeEvent::Enabled => None,
                egui::ImeEvent::Preedit(text) => Some(TextInputEvent::ImePreedit(text.clone())),
                egui::ImeEvent::Commit(text) => Some(TextInputEvent::ImeCommit(text.clone())),
                egui::ImeEvent::Disabled => Some(TextInputEvent::ImeDisabled),
            },
            egui::Event::Copy => Some(TextInputEvent::Copy),
            egui::Event::Cut => Some(TextInputEvent::Cut),
            egui::Event::Paste(text) => Some(TextInputEvent::Paste(text.clone())),
            egui::Event::Key {
                key,
                pressed,
                modifiers,
                ..
            } => text_key(*key).map(|key| TextInputEvent::Key {
                key,
                pressed: *pressed,
                modifiers: text_modifiers(*modifiers),
            }),
            egui::Event::PointerButton {
                button,
                pressed,
                modifiers,
                ..
            } => Some(TextInputEvent::PointerButton {
                button: text_pointer_button(*button),
                pressed: *pressed,
                modifiers: text_modifiers(*modifiers),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ime_events_become_composition_events() {
        let events = [
            egui::Event::Ime(egui::ImeEvent::Enabled),
            egui::Event::Ime(egui::ImeEvent::Preedit("にほ".to_owned())),
            egui::Event::Ime(egui::ImeEvent::Commit("日本".to_owned())),
            egui::Event::Ime(egui::ImeEvent::Disabled),
        ];
        assert_eq!(
            translate_input_events(&events),
            vec![
                TextInputEvent::ImePreedit("にほ".to_owned()),
                TextInputEvent::ImeCommit("日本".to_owned()),
                TextInputEvent::ImeDisabled,
            ],
            "Enabled carries no text and is dropped"
        );
    }
}
