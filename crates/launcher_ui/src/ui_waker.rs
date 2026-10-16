//! Lets background code wake a UI that only renders on demand. The front end installs its wake function once;
//! notification emitters and the console call [`wake`] whenever they publish something the user should see.

use std::sync::OnceLock;

static WAKER: OnceLock<fn()> = OnceLock::new();

/// Installs the function that schedules a UI frame. Only the first call has an effect.
pub fn install(waker: fn()) {
    let _ = WAKER.set(waker);
}

/// Asks the UI to render a frame soon. A no-op until a waker is installed.
pub fn wake() {
    if let Some(waker) = WAKER.get() {
        waker();
    }
}

/// Passes a channel send result through, waking the UI once the message is queued.
pub(crate) fn wake_after_send<T, E>(result: Result<T, E>) -> Result<T, E> {
    if result.is_ok() {
        wake();
    }
    result
}
