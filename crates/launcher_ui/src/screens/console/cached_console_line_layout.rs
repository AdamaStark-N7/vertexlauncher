use super::*;

#[derive(Clone)]
pub(super) struct CachedConsoleLineLayout {
    pub(super) fingerprint: u64,
    pub(super) layout: Arc<ConsoleLineLayout>,
    pub(super) last_used_frame: u64,
}
