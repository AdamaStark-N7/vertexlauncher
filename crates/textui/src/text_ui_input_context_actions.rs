/// Actions requested from the text input context menu during one frame.
#[derive(Clone, Copy, Default)]
pub(crate) struct InputContextActions {
    pub(crate) cut: bool,
    pub(crate) copy: bool,
    pub(crate) paste: bool,
    pub(crate) select_all: bool,
}
