//! Controller input for the SE5 front end. The device layer (`app::gamepad`) reads the controllers and applies
//! the user's stick calibration; this turns what it saw into the kit's navigation input each frame, plus the
//! per-screen inputs (skin preview orbit, screenshot viewer pan and zoom).

use vertex_shell::sturdy::glam::Vec2;
use vertex_shell::sturdy::ui::Ui;
use vertex_shell::sturdy::ui_kit::{NavDirection, NavInput};

use super::super::gamepad::{GamepadNavigator, NavDir};
use super::kit::Kit;
use super::{AppState, Dialog};
use launcher_ui::screens::AppScreen;

/// Controller state kept between frames.
pub struct Gamepad {
    navigator: Option<GamepadNavigator>,
    /// Right stick pan and trigger zoom for the screenshot viewers, in -1..1.
    pub viewer_pan: Vec2,
    pub viewer_zoom: f32,
}

impl Default for Gamepad {
    fn default() -> Self {
        Self {
            navigator: GamepadNavigator::new(),
            viewer_pan: Vec2::ZERO,
            viewer_zoom: 0.0,
        }
    }
}

impl Gamepad {
    pub fn navigator(&self) -> Option<&GamepadNavigator> {
        self.navigator.as_ref()
    }

    pub fn navigator_mut(&mut self) -> Option<&mut GamepadNavigator> {
        self.navigator.as_mut()
    }
}

fn direction(dir: NavDir) -> NavDirection {
    match dir {
        NavDir::Up => NavDirection::Up,
        NavDir::Down => NavDirection::Down,
        NavDir::Left => NavDirection::Left,
        NavDir::Right => NavDirection::Right,
    }
}

/// Reads the controllers and the keyboard and starts the kit's input frame. Call once per frame before drawing.
pub fn frame(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    let AppState {
        gamepad,
        config,
        screen,
        skins,
        dialog,
        calibration,
        console,
        ..
    } = state;
    let Some(navigator) = gamepad.navigator.as_mut() else {
        kit.begin_input_frame(ui, NavInput::default(), 0);
        return;
    };
    let slider_focused = kit.focused_is_slider();
    let poll = navigator.poll(config.gamepad_calibrations(), *screen, slider_focused);

    let mut input = NavInput {
        direction: poll.nav.first().copied().map(direction),
        activate: poll.south,
        back: poll.back,
        scroll: Vec2::ZERO,
    };
    if *screen == AppScreen::Console {
        // The right stick always drives the console log, whatever has focus.
        let lines_id = console.active_lines_id().to_owned();
        if poll.scroll.y != 0.0
            && let Some(metrics) = ui.scroll_metrics(&lines_id)
        {
            ui.set_scroll_offset(&lines_id, metrics.offset + Vec2::new(0.0, poll.scroll.y));
        }
    } else {
        input.scroll = Vec2::new(poll.scroll.x, poll.scroll.y);
    }
    if poll.input_seen {
        kit.controller_active = true;
    }
    kit.begin_input_frame(ui, input, poll.slider_step);
    kit.set_context_pressed(poll.context_menu);
    if poll.home_focus {
        // The left stick click jumps to the sidebar.
        kit.focus
            .set_focus(Some(&format!("nav::{}", AppScreen::Home.label())));
    }

    skins.set_gamepad_orbit(poll.skins_preview_orbit);
    gamepad.viewer_pan = Vec2::new(poll.screenshot_viewer_pan.x, poll.screenshot_viewer_pan.y);
    gamepad.viewer_zoom = poll.screenshot_viewer_zoom;
    if let Some(device) = poll.calibration_requested
        && dialog.is_none()
    {
        calibration.start(device);
        *dialog = Some(Dialog::GamepadCalibration);
    }
    // Controllers do not wake the window; keep looking while one is connected, and now and then for new ones.
    let connected = gamepad
        .navigator
        .as_ref()
        .is_some_and(|navigator| navigator.connected() > 0);
    vertex_shell::wake_after(std::time::Duration::from_millis(if connected {
        16
    } else {
        1000
    }));
}
