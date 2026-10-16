//! The SturdyEngine5 host for Vertex: opens the window, owns the engine loop and exposes the pieces the rest of the
//! launcher needs (a way to wake the UI from any thread, and the per-frame draw hook).
//!
//! The UI is immediate-mode. The caller hands [`run`] one draw closure that rebuilds the whole frame, and the engine
//! draws continuously ([`sturdy::app::FrameMode::Continuous`]): as fast as the GPU allows, or exactly at the frame
//! limit when one is set. `wake` is therefore only an extra nudge.

mod logging;
pub mod palette;
pub mod settings_apply;

pub use settings_apply::Applied;
use settings_apply::Appliers;

use std::process::ExitCode;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use config::{Config, SettingsDiff, SettingsDomain};

use sturdy::app::{FrameMode, RedrawHandle};
use sturdy::prelude::*;
use sturdy::ui::{self, Ui};

pub use sturdy;

static REDRAW: OnceLock<RedrawHandle> = OnceLock::new();

/// Wakes the UI so it renders another frame. Safe from any thread; a no-op until the engine has started.
pub fn wake() {
    if let Some(handle) = REDRAW.get() {
        handle.request_redraw();
    }
}

/// Wakes the UI after `delay` (the earliest pending request wins).
pub fn wake_after(delay: std::time::Duration) {
    if let Some(handle) = REDRAW.get() {
        handle.request_redraw_after(delay);
    }
}

/// Window-level startup options. Everything here can also change at runtime through the settings appliers.
#[derive(Clone, Debug)]
pub struct ShellConfig {
    pub title: String,
    pub app_id: String,
    pub size: (u32, u32),
    pub decorated: bool,
    pub transparent: bool,
}

impl ShellConfig {
    pub fn new(title: impl Into<String>, app_id: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: app_id.into(),
            size: (1280, 800),
            decorated: false,
            transparent: true,
        }
    }
}

/// How the UI hands config changes to the engine thread and learns what the appliers made of them.
///
/// `submit` is cheap and callable from any thread (including the draw hook); the next frame applies it. Only the
/// latest pending config matters, so rapid changes coalesce.
#[derive(Default)]
pub struct SettingsBridge {
    pending: Mutex<Option<Config>>,
    results: Mutex<Vec<(SettingsDomain, Applied)>>,
}

impl SettingsBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn submit(&self, config: Config) {
        *lock(&self.pending) = Some(config);
        wake();
    }

    /// Failures (the caller restores the old value and notifies) and degradations (it just notifies), oldest first.
    pub fn take_results(&self) -> Vec<(SettingsDomain, Applied)> {
        std::mem::take(&mut *lock(&self.results))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Shell<F> {
    draw: Option<F>,
    bridge: Arc<SettingsBridge>,
    /// The config the engine currently reflects; `None` until the first frame applies the initial one.
    applied: Option<Config>,
    appliers: Option<Appliers>,
    effect_polls_left: u8,
}

impl<F> Shell<F> {
    fn apply_pending(&mut self, engine: &mut Engine<'_>, window_id: u64) {
        let appliers = self.appliers.get_or_insert_with(|| Appliers::new(window_id));
        if let Some(new) = lock(&self.bridge.pending).take() {
            let diff = match &self.applied {
                Some(old) => old.diff(&new),
                None => SettingsDiff::all(),
            };
            if diff.contains(SettingsDomain::WindowEffect) {
                self.effect_polls_left = 40;
            }
            let results = appliers.apply(engine, &new, diff);
            if !results.is_empty() {
                lock(&self.bridge.results).extend(results);
            }
            self.applied = Some(new);
        }
        if appliers.poll_hdr_metadata(engine) {
            wake_after(Duration::from_millis(50));
        }
        if self.effect_polls_left > 0 {
            // The platform reports blur's verdict a frame or two after the request.
            self.effect_polls_left -= 1;
            if let Some(verdict) = appliers.poll_effect_status(engine) {
                self.effect_polls_left = 0;
                if verdict != Applied::Ok {
                    lock(&self.bridge.results).push((SettingsDomain::WindowEffect, verdict));
                }
            } else if self.effect_polls_left > 0 {
                wake_after(Duration::from_millis(50));
            }
        }
    }
}

impl<F> GameLogic for Shell<F>
where
    F: FnMut(&mut Ui<'_>) + Send + 'static,
{
    fn init(&mut self, engine: &mut Engine<'_>) -> Result<(), Error> {
        engine.set_frame_mode(FrameMode::Continuous);
        let _ = REDRAW.set(engine.redraw_handle());
        if let Some(draw) = self.draw.take() {
            ui::install_draw_hook(draw);
        }
        Ok(())
    }

    fn frame(&mut self, engine: &mut Engine<'_>, frame: &Frame) -> Option<FrameDescription> {
        self.apply_pending(engine, frame.window_id);
        Some(FrameDescription::ui_only())
    }
}

/// Runs the launcher window until it closes. Call after the CLI, webview-helper and single-instance checks.
///
/// `initial` is applied on the first frame, so every setting takes effect without a restart, including the ones the
/// window itself is created with.
pub fn run<F>(config: ShellConfig, initial: Config, bridge: Arc<SettingsBridge>, draw: F) -> ExitCode
where
    F: FnMut(&mut Ui<'_>) + Send + 'static,
{
    logging::forward_engine_logs();
    let translucent = initial.window_transparency().is_translucent();
    bridge.submit(initial);
    let runtime = RuntimeConfig::new(config.title)
        .app_id(config.app_id)
        .size(config.size.0, config.size.1)
        .decorated(config.decorated)
        // The window is created able to become translucent; the applier then sets the real state live.
        .transparent(config.transparent || translucent)
        .transparent_composition(config.transparent || translucent);
    sturdy::run(runtime, Shell { draw: Some(draw), bridge, applied: None, appliers: None, effect_polls_left: 0 })
}
