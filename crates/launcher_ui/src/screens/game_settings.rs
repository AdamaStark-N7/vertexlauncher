//! In-game settings editor for an instance's `options.txt`.
//!
//! The dialog shows a live view of the file: edits are written a moment after you stop
//! typing (on top of whatever is on disk at that time), and changes made by the game or by
//! settings sync appear in the dialog as soon as they hit the disk.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use config::Config;
use instance_sync::{PackStatus, pack_status, scan_packs};
use instances::{InstanceRecord, InstanceStore, PackOverride, instance_root_path};
use mc_options::packs::PackSupport;
use mc_options::schema::{self, Display, Group, Kind, OptionDef};
use mc_options::translate::format_for_def;
use mc_options::{LiveOptions, Poll, Ver, keys, parse_game_version, value};
use textui::TextUi;
use textui_egui::prelude::*;

use crate::app::tokio_runtime;
use crate::sync_runner::{self, SyncRunConfig};
use crate::ui::components::settings_widgets;
use crate::ui::{modal, style};

#[path = "game_settings/audio_devices.rs"]
mod audio_devices;
#[path = "game_settings/def_row.rs"]
mod def_row;
#[path = "game_settings/group.rs"]
mod group;
#[path = "game_settings/key_capture.rs"]
mod key_capture;
#[path = "game_settings/keybind_row.rs"]
mod keybind_row;
#[path = "game_settings/modal_sections.rs"]
mod modal_sections;
#[path = "game_settings/option_rows.rs"]
mod option_rows;
#[path = "game_settings/packs_tab.rs"]
mod packs_tab;
#[path = "game_settings/search.rs"]
mod search;
#[path = "game_settings/sync_tab.rs"]
mod sync_tab;

use self::def_row::*;
use self::group::*;
use self::keybind_row::*;
use self::modal_sections::*;
use self::option_rows::*;
use self::packs_tab::*;
use self::search::*;
use self::sync_tab::*;

const REQUEST_KEY: &str = "game_settings_modal";
/// Edits are written once they have been idle this long.
const SAVE_DELAY: Duration = Duration::from_millis(250);
/// How often to look for outside changes while the dialog is open.
const WATCH_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Group(Group),
    Packs,
    Sync,
}

#[derive(Clone)]
struct ModalState {
    instance_id: String,
    instance_name: String,
    game_version: String,
    version: Option<Ver>,
    live: Arc<Mutex<LiveOptions>>,
    tab: Tab,
    search: String,
    capturing: Option<String>,
    capture_started_at_frame: u64,
    pack_rows: Arc<Mutex<Option<(Instant, Arc<[PackRow]>)>>>,
    /// Output devices found by a background lookup; `None` while it runs.
    audio_devices: Arc<Mutex<Option<Vec<String>>>>,
}

/// A resource pack seen in this instance or another syncing instance.
#[derive(Clone)]
struct PackRow {
    name: String,
    support: Option<PackSupport>,
    present_here: bool,
    /// Names of other instances that have it.
    elsewhere: Vec<String>,
}

/// Opens the game settings dialog for `instance`.
pub(super) fn request_modal(
    ctx: &egui::Context,
    instance: &InstanceRecord,
    installations_root: &Path,
) {
    let instance_root = instance_root_path(installations_root, instance);
    let path = instance_root.join("options.txt");
    let audio_devices = Arc::new(Mutex::new(None));
    {
        let audio_devices = Arc::clone(&audio_devices);
        let ctx = ctx.clone();
        tokio_runtime::spawn_blocking_detached(move || {
            let devices = audio_devices::list_output_devices();
            if let Ok(mut slot) = audio_devices.lock() {
                *slot = Some(devices);
            }
            ctx.request_repaint();
        });
    }
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(REQUEST_KEY),
            ModalState {
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                game_version: instance.game_version.clone(),
                version: parse_game_version(&instance.game_version),
                live: Arc::new(Mutex::new(open_live_options(
                    path,
                    instance_root.as_path(),
                    &instance.game_version,
                ))),
                tab: Tab::Group(Group::Video),
                search: String::new(),
                capturing: None,
                capture_started_at_frame: 0,
                pack_rows: Arc::new(Mutex::new(None)),
                audio_devices,
            },
        );
    });
}

/// Opens the live view of `options.txt`. If the game hasn't recorded its data version there yet
/// (fresh instance), records it now, as if that version of the game had written the file. The
/// value is never shown in the UI.
fn open_live_options(
    path: std::path::PathBuf,
    instance_root: &Path,
    game_version: &str,
) -> LiveOptions {
    let mut live = LiveOptions::open(path);
    if live.get("version").is_none()
        && let Some(world_version) = mc_options::data_version::read_world_version(
            &mc_options::data_version::client_jar_path(instance_root, game_version),
        )
    {
        live.edit("version", Some(&world_version.to_string()));
    }
    live
}

/// Renders the dialog if open, keeping it in step with the file.
pub(super) fn render_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let key = egui::Id::new(REQUEST_KEY);
    let Some(mut state) = ctx.data(|data| data.get_temp::<ModalState>(key)) else {
        return;
    };
    let live_handle = Arc::clone(&state.live);
    let Ok(mut live) = live_handle.lock() else {
        return;
    };

    // Follow the file, and flush edits that have been idle long enough.
    let polled = live.poll(SAVE_DELAY);
    if polled == Poll::Saved
        && instances
            .game_settings_sync
            .instances
            .contains(&state.instance_id)
    {
        // Propagate right away instead of waiting for the next launch or exit.
        sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
    }
    ctx.request_repaint_after(
        live.time_until_save(SAVE_DELAY)
            .map_or(WATCH_INTERVAL, |wait| wait.min(WATCH_INTERVAL)),
    );

    let running = instances
        .find(&state.instance_id)
        .map(|instance| instance_root_path(config.minecraft_installations_root_path(), instance))
        .is_some_and(|root| installation::is_instance_running(root.as_path()));

    let mut close = false;
    let response = modal::show_window(
        ctx,
        "Game settings",
        modal::ModalOptions::new(
            egui::Id::new(("game_settings_modal_window", state.instance_id.as_str())),
            modal::ModalLayout::centered(
                // No upper cap: it grows with the window, keeping the host's minimum padding.
                modal::AxisSizing::new(0.92, 520.0, f32::INFINITY),
                modal::AxisSizing::new(0.92, 420.0, f32::INFINITY),
            ),
        )
        .with_layer(modal::ModalLayer::Base)
        // The body has its own vertical scroll; an outer one would fight it for the wheel.
        .with_content_scroll(false)
        .with_dismiss_behavior(modal::DismissBehavior::EscapeAndScrim),
        |ui| {
            render_modal_header(ui, text_ui, &state, &live, running);
            render_tab_strip(ui, text_ui, &mut state);
            render_search_box(ui, text_ui, &mut state);
            render_tab_body(ui, text_ui, &mut state, &mut live, instances, config);
            close = render_close_row(ui, text_ui);
        },
    );

    let done = close || response.close_requested;
    if done {
        // Save anything still pending before the view goes away.
        let _ = live.flush();
    }
    drop(live);
    if done {
        ctx.data_mut(|data| data.remove::<ModalState>(key));
    } else {
        ctx.data_mut(|data| data.insert_temp(key, state));
    }
}
