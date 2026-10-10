use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use config::Config;
use egui::{Color32, Layout, TextureOptions, Ui};
use flate2::read::GzDecoder;
use instances::{InstanceStore, instance_root_path, set_server_favorite, set_world_favorite};
use launcher_runtime as tokio_runtime;
use textui::TextUi;
use textui_egui::interaction;
use textui_egui::prelude::*;
use ui_foundation::{
    DialogPreset, UiMetrics, danger_button, dialog_options, fill_tab_row, secondary_button,
    show_dialog,
};

use crate::{
    assets, desktop, install_activity, notification,
    screens::{
        PlayerAuthContext, QuickLaunchCommandMode, build_quick_launch_command,
        build_quick_launch_steam_options, selected_quick_launch_user,
    },
    ui::{
        components::{
            image_memory::prepare_owned_image_bytes_for_memory,
            image_textures,
            lazy_image_bytes::{LazyImageBytes, LazyImageBytesStatus},
            virtual_masonry::{
                CachedVirtualMasonryLayout, build_virtual_masonry_layout,
                render_virtualized_masonry,
            },
        },
        context_menu::{self, ContextMenuItem, ContextMenuRequest},
        instance_context_menu::{self, InstanceContextAction},
        style,
    },
};

use super::{AppScreen, PendingLaunchIntent, queue_launch_intent};

#[path = "home_nbt.rs"]
mod home_nbt;
use home_nbt::{parse_servers_dat, parse_world_metadata};

#[path = "home/home_activity_result_channel.rs"]
mod home_activity_result_channel;
#[path = "home/home_activity_scan_instance.rs"]
mod home_activity_scan_instance;
#[path = "home/home_activity_scan_request.rs"]
mod home_activity_scan_request;
#[path = "home/home_activity_scan_result.rs"]
mod home_activity_scan_result;
#[path = "home/home_entry_ref.rs"]
mod home_entry_ref;
#[path = "home/home_output.rs"]
mod home_output;
#[path = "home/home_presence_section.rs"]
mod home_presence_section;
#[path = "home/home_state.rs"]
mod home_state;
#[path = "home/home_tab.rs"]
mod home_tab;
#[path = "home/home_ui_metrics.rs"]
mod home_ui_metrics;
#[path = "home/world_entry.rs"]
mod world_entry;

#[path = "home/activity_feed.rs"]
mod activity_feed;
#[path = "home/activity_layout.rs"]
mod activity_layout;
#[path = "home/activity_scan.rs"]
mod activity_scan;
#[path = "home/entry_widgets.rs"]
mod entry_widgets;
#[path = "home/instance_usage.rs"]
mod instance_usage;
#[path = "home/quick_launch_copy.rs"]
mod quick_launch_copy;
#[path = "home/server_row.rs"]
mod server_row;
#[path = "home/tab_content.rs"]
mod tab_content;
#[path = "home/world_row.rs"]
mod world_row;

use self::activity_feed::*;
use self::activity_layout::*;
use self::activity_scan::*;
use self::entry_widgets::*;
use self::instance_usage::*;
use self::quick_launch_copy::*;
use self::server_row::*;
use self::tab_content::*;
use self::world_row::*;

const HOME_SCAN_INTERVAL: Duration = Duration::from_secs(3);
const SERVER_PING_REFRESH_INTERVAL: Duration = Duration::from_secs(20);
const SERVER_PING_CONNECT_TIMEOUT: Duration = Duration::from_millis(350);
const SERVER_PINGS_PER_SCAN: usize = 3;
const ENTRY_ICON_SIZE: f32 = 14.0;
const SERVER_PING_ICON_SIZE: f32 = 24.0;
const SERVER_STATUS_GROUP_WIDTH: f32 = 86.0;
const SERVER_STATUS_GROUP_GAP: f32 = 6.0;
const FAVORITE_STAR_BUTTON_SIZE: f32 = 20.0;
const FAVORITE_STAR_ICON_SIZE: f32 = 14.0;
const ACTIVITY_ENTRY_THUMBNAIL_SIZE: f32 = 34.0;
const ACTIVITY_ENTRY_CONTENT_GAP: f32 = 8.0;
const ACTIVITY_ENTRY_ROW_VERTICAL_PADDING: f32 = 5.0;
const ACTIVITY_ENTRY_ROW_HORIZONTAL_PADDING: f32 = 8.0;
const SCREENSHOT_SCAN_INTERVAL: Duration = Duration::from_secs(10);
const SCREENSHOT_PAGE_SIZE: usize = 30;
const SCREENSHOT_TILE_GAP: f32 = 10.0;
const SCREENSHOT_VIEWER_MIN_ZOOM: f32 = 1.0;
const SCREENSHOT_VIEWER_MAX_ZOOM: f32 = 8.0;
const SCREENSHOT_VIEWER_ZOOM_STEP: f32 = 0.12;
const SCREENSHOT_VIEWER_SCROLL_ZOOM_SENSITIVITY: f32 = 0.0015;
const HOME_SCREENSHOT_OVERSCAN: f32 = 420.0;
const HOME_THUMBNAIL_CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const HOME_THUMBNAIL_CACHE_STALE_FRAMES: u64 = 900;

#[path = "home_grouping.rs"]
mod home_grouping;
#[path = "home_screenshots.rs"]
mod home_screenshots;
#[path = "home_server_ping.rs"]
mod home_server_ping;
#[path = "home_server_sync.rs"]
mod home_server_sync;
#[path = "home_support.rs"]
mod home_support;
#[path = "home_thumbnails.rs"]
mod home_thumbnails;
#[path = "home_world_sync.rs"]
mod home_world_sync;
#[path = "screenshot_common.rs"]
pub(in crate::screens) mod screenshot_common;

use self::home_activity_result_channel::HomeActivityResultChannel;
use self::home_activity_scan_instance::HomeActivityScanInstance;
use self::home_activity_scan_request::HomeActivityScanRequest;
use self::home_activity_scan_result::HomeActivityScanResult;
use self::home_entry_ref::HomeEntryRef;
use self::home_grouping::EntryInstance;
pub use self::home_output::HomeOutput;
pub use self::home_presence_section::HomePresenceSection;
use self::home_screenshots::{
    ScreenshotCandidate, ScreenshotEntry, ScreenshotViewerState,
    handle_escape as handle_screenshot_escape, poll_delete_screenshot_results,
    poll_screenshot_results, purge_screenshot_state as purge_home_screenshot_feature_state,
    refresh_screenshot_state, render_delete_screenshot_modal, render_screenshot_gallery,
    render_screenshot_viewer_modal, retain_home_viewer_image,
};
use self::home_server_ping::{
    ServerEntry, ServerPingSnapshot, collect_servers_from_request, home_server_icon_uri,
    normalize_server_address, poll_server_ping_results, queue_server_pings,
    render_server_ping_icon, retain_known_server_pings, server_meta_line,
};
use self::home_state::HomeState;
use self::home_support::{
    current_time_millis, format_time_ago, modified_millis, open_home_instance,
    open_home_instance_folder,
};
use self::home_tab::HomeTab;
use self::home_thumbnails::{
    HomeThumbnailState, home_instance_thumbnail_uri, home_world_thumbnail_uri,
    instance_thumbnail_cache_key, poll_instance_thumbnail_results,
    purge_activity_image_state as purge_home_activity_thumbnail_state, request_instance_thumbnail,
    trim_home_thumbnail_cache,
};
use self::home_ui_metrics::HomeUiMetrics;
use self::world_entry::WorldEntry;

fn home_state_id() -> egui::Id {
    egui::Id::new("home_screen_state")
}

pub fn purge_inactive_state(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        if let Some(mut state) = data.get_temp::<HomeState>(home_state_id()) {
            state.purge_screenshot_state(ctx);
            state.purge_activity_image_state(ctx);
        }
        data.insert_temp(home_state_id(), HomeState::default());
    });
}

pub fn purge_screenshot_state(ctx: &egui::Context) {
    purge_home_screenshot_feature_state(ctx);
}

pub fn set_gamepad_screenshot_viewer_input(ctx: &egui::Context, pan: egui::Vec2, zoom: f32) {
    home_screenshots::set_gamepad_screenshot_viewer_input(ctx, pan, zoom);
}

pub fn presence_section(ctx: &egui::Context) -> HomePresenceSection {
    let state_id = home_state_id();
    let state = ctx.data_mut(|data| data.get_temp::<HomeState>(state_id));
    state
        .map(|state| state.active_tab.presence_section())
        .unwrap_or(HomePresenceSection::Activity)
}

pub(super) fn handle_escape(ctx: &egui::Context) -> bool {
    handle_screenshot_escape(ctx)
}

pub fn render(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
    auth: &PlayerAuthContext<'_>,
    streamer_mode: bool,
) -> HomeOutput {
    let mut output = HomeOutput::default();
    ui.ctx()
        .options_mut(|options| options.reduce_texture_memory = true);
    let metrics = HomeUiMetrics::from_ui(ui);
    let state_id = home_state_id();
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<HomeState>(state_id))
        .unwrap_or_default();
    let previous_tab = state.active_tab;

    render_home_tab_row(ui, text_ui, &mut state.active_tab, metrics);
    if previous_tab == HomeTab::Screenshots && state.active_tab != HomeTab::Screenshots {
        state.purge_screenshot_state(ui.ctx());
    }
    if previous_tab == HomeTab::InstancesAndWorlds
        && state.active_tab != HomeTab::InstancesAndWorlds
    {
        state.purge_activity_image_state(ui.ctx());
    }
    ui.ctx().request_repaint_after(Duration::from_millis(250));
    state.screenshot_images.begin_frame(ui.ctx());
    state.thumbnails.cache_frame_index = state.thumbnails.cache_frame_index.saturating_add(1);
    trim_home_thumbnail_cache(ui.ctx(), &mut state.thumbnails);
    let screenshot_images_updated = state.screenshot_images.poll(ui.ctx());
    ui.add_space(14.0);

    match state.active_tab {
        HomeTab::InstancesAndWorlds => render_instances_and_worlds_tab(
            ui,
            text_ui,
            instances,
            config,
            auth,
            streamer_mode,
            &mut state,
            &mut output,
            metrics,
        ),
        HomeTab::Screenshots => {
            render_screenshots_tab(ui, text_ui, instances, config, &mut state, metrics)
        }
    }

    if screenshot_images_updated
        || (state.screenshot_images.has_in_flight()
            && (state.active_tab == HomeTab::Screenshots || state.screenshot_viewer.is_some()))
    {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }

    render_screenshot_viewer_modal(ui.ctx(), text_ui, &mut state, metrics);
    render_delete_screenshot_modal(ui.ctx(), text_ui, &mut state, instances, config, metrics);
    home_server_sync::render_server_sync_modal(ui.ctx(), text_ui, instances, config);
    home_world_sync::render_world_sync_modal(ui.ctx(), text_ui, instances, config);
    output.presence_section = state.active_tab.presence_section();
    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
    output
}

fn render_home_tab_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    active_tab: &mut HomeTab,
    metrics: HomeUiMetrics,
) {
    fill_tab_row(
        text_ui,
        ui,
        "home_tab_row",
        active_tab,
        &[
            (
                HomeTab::InstancesAndWorlds,
                HomeTab::InstancesAndWorlds.label(),
            ),
            (HomeTab::Screenshots, HomeTab::Screenshots.label()),
        ],
        metrics.tab_height,
        style::SPACE_MD,
    );
}

fn modal_default_focus_requested(ctx: &egui::Context, id_source: impl std::hash::Hash) -> bool {
    let key = egui::Id::new(("modal_default_focus_frame", id_source));
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| {
        let last_seen = data.get_temp::<u64>(key);
        data.insert_temp(key, frame);
        !matches!(last_seen, Some(previous) if previous.saturating_add(1) >= frame)
    })
}
