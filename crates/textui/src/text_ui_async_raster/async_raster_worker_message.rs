use super::*;

pub(crate) enum AsyncRasterWorkerMessage {
    RegisterFont(SharedFontBytes),
    Render(AsyncRasterRequest),
}
