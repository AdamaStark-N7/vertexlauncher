use super::*;

/// How long a requested screenshot is waited for before the request is dropped.
const PENDING_OPEN_TIMEOUT: Duration = Duration::from_secs(15);

fn pending_open_id() -> egui::Id {
    egui::Id::new("home_screenshot_pending_open")
}

/// A request (from outside Home, e.g. a notification) to show one screenshot in the viewer.
#[derive(Clone)]
struct PendingScreenshotOpen {
    path: PathBuf,
    requested_at: Instant,
    /// The gallery was force-rescanned once because the file was not in it yet.
    rescanned: bool,
}

pub(in crate::screens) fn request_open_screenshot(ctx: &egui::Context, path: PathBuf) {
    ctx.data_mut(|data| {
        data.insert_temp(
            pending_open_id(),
            PendingScreenshotOpen {
                path,
                requested_at: Instant::now(),
                rescanned: false,
            },
        );
    });
    crate::screens::request_screen(ctx, AppScreen::Home);
    ctx.request_repaint();
}

pub(in crate::screens) fn has_pending_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<PendingScreenshotOpen>(pending_open_id()))
        .is_some()
}

/// Opens the requested screenshot once the gallery has loaded it. Call after the gallery has
/// been polled for the frame.
pub(in crate::screens) fn resolve_pending_open(
    ctx: &egui::Context,
    state: &mut HomeState,
    instances: &InstanceStore,
    config: &Config,
) {
    let Some(mut pending) =
        ctx.data(|data| data.get_temp::<PendingScreenshotOpen>(pending_open_id()))
    else {
        return;
    };
    let finish = |ctx: &egui::Context| {
        ctx.data_mut(|data| data.remove::<PendingScreenshotOpen>(pending_open_id()));
    };

    if let Some(entry) = state
        .screenshots
        .iter()
        .find(|entry| entry.path == pending.path)
        .cloned()
    {
        state.screenshot_viewer = Some(ScreenshotViewerState {
            screenshot_key: entry.key(),
            entry_snapshot: entry,
            zoom: SCREENSHOT_VIEWER_MIN_ZOOM,
            pan_uv: egui::Vec2::ZERO,
        });
        finish(ctx);
        return;
    }

    if pending.requested_at.elapsed() >= PENDING_OPEN_TIMEOUT {
        finish(ctx);
        return;
    }

    let scan_settled = !state.screenshot_scan_pending && state.last_screenshot_scan_at.is_some();
    if scan_settled {
        if pending.rescanned {
            // A fresh scan finished without the file: it was moved or deleted.
            finish(ctx);
            return;
        }
        pending.rescanned = true;
        refresh_screenshot_state(state, instances, config, true);
        ctx.data_mut(|data| data.insert_temp(pending_open_id(), pending));
    }
    ctx.request_repaint();
}
