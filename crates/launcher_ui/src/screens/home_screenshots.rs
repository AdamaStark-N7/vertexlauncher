use super::*;
use textui_egui::interaction;

use super::screenshot_common::{
    ViewerCanvas, ViewerGamepadInput, ViewerZoomLimits, decode_color_image,
    is_supported_screenshot_path, paint_tile_border, paint_tile_caption, paint_tile_image,
    render_viewer_canvas, ui_pointer_over_rect,
};

#[path = "home_screenshots/delete.rs"]
mod delete;
#[path = "home_screenshots/gallery.rs"]
mod gallery;
#[path = "home_screenshots/pending_open.rs"]
mod pending_open;
#[path = "home_screenshots/scan.rs"]
mod scan;
#[path = "home_screenshots/screenshot_candidate.rs"]
mod screenshot_candidate;
#[path = "home_screenshots/screenshot_entry.rs"]
mod screenshot_entry;
#[path = "home_screenshots/screenshot_overlay_action.rs"]
mod screenshot_overlay_action;
#[path = "home_screenshots/screenshot_overlay_button_result.rs"]
mod screenshot_overlay_button_result;
#[path = "home_screenshots/screenshot_overlay_result.rs"]
mod screenshot_overlay_result;
#[path = "home_screenshots/screenshot_result_channel.rs"]
mod screenshot_result_channel;
#[path = "home_screenshots/screenshot_scan_instance.rs"]
mod screenshot_scan_instance;
#[path = "home_screenshots/screenshot_scan_message.rs"]
mod screenshot_scan_message;
#[path = "home_screenshots/screenshot_scan_request.rs"]
mod screenshot_scan_request;
#[path = "home_screenshots/screenshot_tile_action.rs"]
mod screenshot_tile_action;
#[path = "home_screenshots/screenshot_viewer_state.rs"]
mod screenshot_viewer_state;
#[path = "home_screenshots/viewer_modal.rs"]
mod viewer_modal;

pub(super) use self::delete::*;
pub(super) use self::gallery::*;
pub(super) use self::pending_open::*;
pub(super) use self::scan::*;
pub(super) use self::screenshot_candidate::ScreenshotCandidate;
pub(super) use self::screenshot_entry::ScreenshotEntry;
use self::screenshot_overlay_action::ScreenshotOverlayAction;
use self::screenshot_overlay_button_result::ScreenshotOverlayButtonResult;
use self::screenshot_overlay_result::ScreenshotOverlayResult;
use self::screenshot_result_channel::ScreenshotResultChannel;
pub(super) use self::screenshot_scan_instance::ScreenshotScanInstance;
use self::screenshot_scan_message::ScreenshotScanMessage;
pub(super) use self::screenshot_scan_request::ScreenshotScanRequest;
use self::screenshot_tile_action::ScreenshotTileAction;
pub(super) use self::screenshot_viewer_state::ScreenshotViewerState;
pub(super) use self::viewer_modal::*;

const VIEWER_ZOOM_LIMITS: ViewerZoomLimits = ViewerZoomLimits {
    min: SCREENSHOT_VIEWER_MIN_ZOOM,
    max: SCREENSHOT_VIEWER_MAX_ZOOM,
    step: SCREENSHOT_VIEWER_ZOOM_STEP,
    scroll_sensitivity: SCREENSHOT_VIEWER_SCROLL_ZOOM_SENSITIVITY,
};

pub(super) fn purge_screenshot_state(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        let Some(mut state) = data.get_temp::<HomeState>(home_state_id()) else {
            return;
        };
        state.purge_screenshot_state(ctx);
        data.insert_temp(home_state_id(), state);
    });
}

pub(super) fn set_gamepad_screenshot_viewer_input(ctx: &egui::Context, pan: egui::Vec2, zoom: f32) {
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new("home_screenshot_viewer_gamepad_pan"), pan);
        data.insert_temp(egui::Id::new("home_screenshot_viewer_gamepad_zoom"), zoom);
    });
}

pub(super) fn handle_escape(ctx: &egui::Context) -> bool {
    let state_id = home_state_id();
    let mut handled = false;
    ctx.data_mut(|data| {
        let Some(mut state) = data.get_temp::<HomeState>(state_id) else {
            return;
        };
        if state.pending_delete_screenshot_key.is_some() {
            if !state.delete_screenshot_in_flight {
                tracing::info!(
                    target: "vertexlauncher/screenshots",
                    "Home screenshot delete confirmation closed by escape."
                );
                state.pending_delete_screenshot_key = None;
            }
            data.insert_temp(state_id, state);
            handled = true;
            return;
        }
        if state.screenshot_viewer.take().is_some() {
            tracing::info!(
                target: "vertexlauncher/screenshots",
                "Home screenshot viewer closed by escape."
            );
            data.insert_temp(state_id, state);
            handled = true;
        }
    });
    handled
}
