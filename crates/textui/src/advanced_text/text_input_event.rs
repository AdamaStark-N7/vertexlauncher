use super::*;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TextInputEvent {
    Text(String),
    /// The in-progress IME composition (pre-edit text) changed. An empty string clears it.
    ImePreedit(String),
    /// The IME finished composing and produced this final text.
    ImeCommit(String),
    /// The IME was switched off, abandoning any composition in progress.
    ImeDisabled,
    Copy,
    Cut,
    Paste(String),
    Key {
        key: TextKey,
        pressed: bool,
        modifiers: TextModifiers,
    },
    PointerButton {
        button: TextPointerButton,
        pressed: bool,
        modifiers: TextModifiers,
    },
}
