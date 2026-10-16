//! Home: your most used instances, your worlds and servers across every instance (launch straight into one, mark
//! favorites, see server status), and a gallery of every screenshot. Scanning and server pinging are
//! `launcher_ui`'s `home_api`, shared with the egui screen.

use std::time::Duration;

use instances::InstanceRecord;
use launcher_ui::screens::home_api::{self as api, EntryKind, EntryView, HomeSession};
use launcher_ui::screens::{
    QuickLaunchCommandMode, build_quick_launch_command, build_quick_launch_steam_options,
};
use vertex_shell::sturdy::ui::{
    AlignX, AlignY, Border, ChildAlignment, CornerRadius, Cursor, Direction, ElementDesc, Padding,
    Sizing, TextStyle, TextWrap, Ui,
};
use vertex_shell::sturdy::ui_kit::{self, MenuItem, PopupStyle};

use super::super::icons::Icon;
use super::super::images::ImageSource;
use super::super::kit::{self, ButtonKind, Kit, MenuEntry, TextRole};
use super::super::{AppState, Dialog};
use super::{library, sync_dialogs};

const SCROLL_ID: &str = "home::scroll";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HomeTab {
    #[default]
    Activity,
    Screenshots,
}

pub fn render(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    let _scroll = kit.scroll(ui, SCROLL_ID, kit::SPACE_LG, kit::SPACE_XL);
    let greeting = match state.auth.display_name() {
        Some(name) if !state.streamer_mode() => format!("Welcome back, {name}"),
        _ => "Welcome back".to_owned(),
    };
    kit.text(ui, TextRole::Title, &greeting);
    {
        let _tabs = kit.row(ui, kit::SPACE_SM);
        for (tab, label) in [
            (HomeTab::Activity, "Instances & Worlds"),
            (HomeTab::Screenshots, "Screenshots"),
        ] {
            if kit.tab_sized(
                ui,
                &format!("home::tab::{label}"),
                label,
                state.home_tab == tab,
                Sizing::Grow,
            ) {
                if tab != HomeTab::Screenshots {
                    state.home.forget_screenshots();
                }
                state.home_tab = tab;
            }
        }
    }
    match state.home_tab {
        HomeTab::Activity => activity(ui, kit, state),
        HomeTab::Screenshots => screenshots(ui, kit, state),
    }
}

fn activity(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    if state.home.tick_activity(&state.instances, &state.config) {
        kit.keep_awake_for(Duration::from_millis(100));
    }
    vertex_shell::wake_after(HomeSession::scan_interval());
    usage(ui, kit, state);

    let view = state.home.activity();
    kit.text(ui, TextRole::Heading, "Worlds & Servers");
    if view.favorites.is_empty() && view.recent.is_empty() {
        kit.text(
            ui,
            TextRole::Muted,
            "No worlds or servers found in any instance.",
        );
        return;
    }
    let mut action = None;
    if !view.favorites.is_empty() {
        kit.text(ui, TextRole::Body, "Favorites");
        for entry in &view.favorites {
            if let Some(next) = entry_row(ui, kit, state, entry) {
                action = Some((entry.clone(), next));
            }
        }
        kit.separator(ui);
    }
    if view.recent.is_empty() {
        kit.text(ui, TextRole::Muted, "No recent worlds or servers.");
    } else {
        kit.text(ui, TextRole::Body, "Recent");
        for entry in &view.recent {
            if let Some(next) = entry_row(ui, kit, state, entry) {
                action = Some((entry.clone(), next));
            }
        }
    }
    if let Some((entry, action)) = action {
        match action {
            EntryAction::Favorite => state.home.toggle_favorite(&mut state.instances, &entry),
            EntryAction::Launch(index) => {
                if let Some(target) = entry.targets.get(index) {
                    launch_entry(
                        state,
                        &entry,
                        &target.instance_id.clone(),
                        &target.target.clone(),
                    );
                }
            }
            EntryAction::CopyCommand(steam) => copy_command(ui, state, &entry, steam),
            EntryAction::Sync => match entry.kind {
                EntryKind::World => sync_dialogs::open_world(state, &entry),
                EntryKind::Server => sync_dialogs::open_server(state, &entry),
            },
        }
    }
}

enum EntryAction {
    Favorite,
    Launch(usize),
    CopyCommand(bool),
    Sync,
}

fn launch_entry(state: &mut AppState, entry: &EntryView, instance_id: &str, target: &str) {
    match entry.kind {
        EntryKind::World => library::launch_with(state, instance_id, Some(target.to_owned()), None),
        EntryKind::Server => {
            library::launch_with(state, instance_id, None, Some(target.to_owned()))
        }
    }
    state.open_instance(instance_id);
}

/// Puts a command line (or Steam launch options) for the entry's primary instance on the clipboard.
fn copy_command(ui: &mut Ui<'_>, state: &mut AppState, entry: &EntryView, steam: bool) {
    let user = state
        .auth
        .active_launch_context()
        .map(|context| context.player_uuid)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| state.auth.display_name().map(str::to_owned));
    let Some(user) = user else {
        launcher_ui::notification::warn!(
            "home/quick_launch",
            "Sign in before copying a command line."
        );
        return;
    };
    let Some(target) = entry.targets.first() else {
        return;
    };
    let (mode, world, server) = match entry.kind {
        EntryKind::World => (
            QuickLaunchCommandMode::World,
            Some(target.target.as_str()),
            None,
        ),
        EntryKind::Server => (
            QuickLaunchCommandMode::Server,
            None,
            Some(target.target.as_str()),
        ),
    };
    let text = if steam {
        build_quick_launch_steam_options(mode, &target.instance_id, &user, world, server)
    } else {
        build_quick_launch_command(mode, &target.instance_id, &user, world, server)
    };
    let _ = ui
        .engine()
        .primary_window_control()
        .map(|mut window| window.set_clipboard_text(&text));
    launcher_ui::notification::info!(
        "home/quick_launch",
        "{}",
        if steam {
            "Copied Steam launch options to clipboard."
        } else {
            "Copied command line to clipboard."
        }
    );
}

fn usage(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    kit.text(ui, TextRole::Heading, "Most Used Instances");
    let mut items: Vec<InstanceRecord> = state.instances.instances.clone();
    items.sort_by(|a, b| {
        b.launch_count
            .cmp(&a.launch_count)
            .then_with(|| b.last_launched_at_ms.cmp(&a.last_launched_at_ms))
            .then_with(|| a.name.cmp(&b.name))
    });
    if items.is_empty() {
        kit.text(ui, TextRole::Muted, "No instances yet.");
        return;
    }
    for instance in items.iter().take(8) {
        let id = format!("home::usage::{}", instance.id);
        let hovered = ui.hovered(&id);
        {
            let _row = ui.element(
                &ElementDesc::new()
                    .id(id.clone())
                    .width(Sizing::Grow)
                    .height(Sizing::Fixed(52.0))
                    .direction(Direction::LeftToRight)
                    .child_gap(kit::SPACE_MD)
                    .padding(Padding::symmetric(kit::SPACE_MD, 6))
                    .align(ChildAlignment::new(AlignX::Left, AlignY::Center))
                    .background(if hovered {
                        kit.palette.bg_light
                    } else {
                        kit.palette.bg
                    })
                    .corner_radius(CornerRadius::all(12.0))
                    .cursor(Cursor::Pointer),
            );
            library::thumbnail(ui, kit, instance, &format!("{id}::thumb"), 40.0);
            kit.text_clipped(ui, TextRole::Body, &instance.name, 420.0);
            kit.fill(ui);
            kit.text(
                ui,
                TextRole::Small,
                &format!(
                    "{} launches | {}",
                    instance.launch_count,
                    api::time_ago(instance.last_launched_at_ms)
                ),
            );
        }
        if kit.clicked(ui, &id) {
            state.open_instance(&instance.id);
        }
    }
}

/// The signal-strength icon, its colour and its tooltip for a server's last ping.
fn ping_indicator(
    kit: &Kit,
    ping: Option<&api::PingView>,
) -> (Icon, vertex_shell::sturdy::glam::Vec4, String) {
    let p = &kit.palette;
    match ping {
        None => (
            Icon::AntennaOff,
            p.text_muted.with_w(0.9),
            "Ping unknown".to_owned(),
        ),
        Some(ping) => match ping.latency_ms.filter(|_| ping.online) {
            None => (Icon::AntennaOff, p.danger, "Server offline".to_owned()),
            Some(ms) => {
                let (icon, color) = if ms <= 80 {
                    (Icon::Antenna5, p.success)
                } else if ms <= 140 {
                    (Icon::Antenna4, p.success * 0.9)
                } else if ms <= 220 {
                    (Icon::Antenna3, p.warning * 0.85)
                } else if ms <= 320 {
                    (Icon::Antenna2, p.warning)
                } else {
                    (Icon::Antenna1, p.danger * 0.92)
                };
                (icon, color.with_w(1.0), format!("Latency: {ms}ms"))
            }
        },
    }
}

fn entry_row(
    ui: &mut Ui<'_>,
    kit: &mut Kit,
    state: &mut AppState,
    entry: &EntryView,
) -> Option<EntryAction> {
    let id = format!("home::entry::{}", entry.key);
    let star_id = format!("{id}::star");
    let hovered = ui.hovered(&id);
    let mut result = None;
    {
        let _row = ui.element(
            &ElementDesc::new()
                .id(id.clone())
                .width(Sizing::Grow)
                .height(Sizing::Fit)
                .direction(Direction::TopToBottom)
                .child_gap(kit::SPACE_SM)
                .padding(Padding::symmetric(kit::SPACE_MD, kit::SPACE_SM))
                .background(if hovered {
                    kit.palette.bg_light
                } else {
                    kit.palette.bg
                })
                .corner_radius(CornerRadius::all(12.0))
                .border(Border::all(kit.palette.border_muted, 1))
                .cursor(Cursor::Pointer),
        );
        {
            let _top = ui.element(
                &ElementDesc::new()
                    .width(Sizing::Grow)
                    .height(Sizing::Fit)
                    .direction(Direction::LeftToRight)
                    .child_gap(kit::SPACE_MD)
                    .align(ChildAlignment::new(AlignX::Left, AlignY::Center)),
            );
            {
                let star_hover = ui.hovered(&star_id);
                let _star = ui.element(
                    &ElementDesc::new()
                        .id(star_id.clone())
                        .width(Sizing::Fixed(26.0))
                        .height(Sizing::Fixed(26.0))
                        .align(ChildAlignment::center())
                        .background(if star_hover {
                            kit.palette.hovered_fill()
                        } else {
                            vertex_shell::sturdy::glam::Vec4::ZERO
                        })
                        .corner_radius(CornerRadius::all(8.0))
                        .cursor(Cursor::Pointer),
                );
                kit.icon(
                    ui,
                    Icon::Star,
                    16.0,
                    if entry.favorite {
                        kit.palette.warning
                    } else {
                        kit.palette.text_muted
                    },
                );
            }
            kit.tooltip(
                ui,
                &star_id,
                if entry.kind == EntryKind::World {
                    "Toggle world favorite"
                } else {
                    "Toggle server favorite"
                },
            );
            match &entry.icon_png {
                Some(png) => {
                    let bytes = png.clone();
                    let key = format!("entry-icon::{}", entry.key);
                    kit.image(
                        ui,
                        &format!("{id}::icon"),
                        &key,
                        move || ImageSource::Bytes(bytes),
                        34.0,
                        34.0,
                        6.0,
                    );
                }
                None => {
                    let _box = ui.element(
                        &ElementDesc::new()
                            .width(Sizing::Fixed(34.0))
                            .height(Sizing::Fixed(34.0))
                            .align(ChildAlignment::center())
                            .background(kit.palette.bg_light)
                            .corner_radius(CornerRadius::all(6.0)),
                    );
                    kit.icon(
                        ui,
                        if entry.kind == EntryKind::World {
                            Icon::Home
                        } else {
                            Icon::Terminal
                        },
                        16.0,
                        kit.palette.text_muted,
                    );
                }
            }
            {
                let _text = ui.element(
                    &ElementDesc::new()
                        .width(Sizing::Grow)
                        .height(Sizing::Fit)
                        .direction(Direction::TopToBottom)
                        .child_gap(2),
                );
                kit.text_clipped(ui, TextRole::Body, &entry.title, 560.0);
                kit.text_clipped(ui, TextRole::Small, &entry.meta, 560.0);
            }
            if entry.kind == EntryKind::Server {
                let (icon, color, tip) = ping_indicator(kit, entry.ping.as_ref());
                let ping_id = format!("{id}::ping");
                {
                    let _ping = ui.element(
                        &ElementDesc::new()
                            .id(ping_id.clone())
                            .width(Sizing::Fixed(24.0))
                            .height(Sizing::Fixed(24.0))
                            .align(ChildAlignment::center()),
                    );
                    kit.icon(ui, icon, 18.0, color);
                }
                kit.tooltip(ui, &ping_id, &tip);
                if let Some(players) = entry.ping.as_ref().and_then(|ping| ping.players.as_ref()) {
                    kit.text(ui, TextRole::Small, players);
                }
            }
        }
        if entry.targets.len() > 1 {
            let _buttons = kit.row(ui, kit::SPACE_SM);
            for (index, target) in entry.targets.iter().enumerate() {
                if kit.button(
                    ui,
                    &format!("{id}::launch::{index}"),
                    &target.instance_name,
                    ButtonKind::Neutral,
                    true,
                ) {
                    result = Some(EntryAction::Launch(index));
                }
            }
        }
    }
    if result.is_some() {
        return result;
    }
    if ui.clicked(&star_id) {
        return Some(EntryAction::Favorite);
    }
    if entry.targets.len() == 1 && kit.clicked(ui, &id) {
        return Some(EntryAction::Launch(0));
    }
    if let Some(tip) = entry
        .ping
        .as_ref()
        .and_then(|ping| ping.players_tip.as_deref())
    {
        kit.tooltip(ui, &id, tip);
    }
    if let Some(at) = kit.secondary(ui, &id) {
        state.home_menu_target = Some(entry.key.clone());
        state.home_menu.open_at(at);
    }
    if state.home_menu_target.as_deref() == Some(entry.key.as_str()) {
        let sync_label = if entry.kind == EntryKind::World {
            "Sync world…"
        } else {
            "Sync options…"
        };
        let items = [
            MenuEntry::new("Copy command line").icon(Icon::Terminal),
            MenuEntry::new("Copy Steam launch options").icon(Icon::Steam),
            MenuEntry::new(sync_label).icon(Icon::UsersGroup),
        ];
        if let Some(choice) = kit.context_menu(ui, "home::entry-menu", &mut state.home_menu, &items)
        {
            result = Some(if choice == 2 {
                EntryAction::Sync
            } else {
                EntryAction::CopyCommand(choice == 1)
            });
        }
        // Keep the target while the menu folds away, or it would vanish instead of animating closed.
        if !state.home_menu.is_open() && !kit.menu_visible("home::entry-menu") {
            state.home_menu_target = None;
        }
    }
    result
}

fn screenshots(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    if state.home.tick_screenshots(&state.instances, &state.config) {
        kit.keep_awake_for(Duration::from_millis(100));
    }
    vertex_shell::wake_after(HomeSession::screenshot_scan_interval());
    let shots = state.home.screenshots();
    if shots.is_empty() {
        kit.text(
            ui,
            TextRole::Muted,
            if state.home.screenshots_loading() {
                "Scanning for screenshots..."
            } else {
                "No screenshots found in any instance."
            },
        );
        return;
    }
    kit.text(
        ui,
        TextRole::Heading,
        &format!("Screenshots ({})", shots.len()),
    );
    let width = kit::content_width_in(ui, SCROLL_ID);
    let gap = f32::from(kit::SPACE_MD);
    let columns = kit::masonry_columns(width);
    let tile_width = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    let mut heights = vec![0.0_f32; columns];
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); columns];
    for (index, shot) in shots.iter().enumerate() {
        let shortest = heights
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(column, _)| column);
        heights[shortest] +=
            tile_width * shot.height as f32 / shot.width.max(1) as f32 + 24.0 + gap;
        buckets[shortest].push(index);
    }
    let mut open = None;
    let mut delete = None;
    {
        let _grid = ui.element(
            &ElementDesc::new()
                .width(Sizing::Grow)
                .height(Sizing::Fit)
                .direction(Direction::LeftToRight)
                .child_gap(kit::SPACE_MD),
        );
        for bucket in &buckets {
            let _column = ui.element(
                &ElementDesc::new()
                    .width(Sizing::Grow)
                    .height(Sizing::Fit)
                    .direction(Direction::TopToBottom)
                    .child_gap(kit::SPACE_MD),
            );
            for &index in bucket {
                let shot = &shots[index];
                let tile_id = format!("home::shot::{index}");
                let height = tile_width * shot.height as f32 / shot.width.max(1) as f32;
                let key = format!("shot::{}::{:?}", shot.path.display(), shot.modified_at_ms);
                if kit.near_viewport(ui, SCROLL_ID, &tile_id, 800.0) {
                    let path = shot.path.clone();
                    kit.image(
                        ui,
                        &tile_id,
                        &key,
                        || ImageSource::File(path),
                        tile_width,
                        height,
                        10.0,
                    );
                } else {
                    drop(
                        ui.element(
                            &ElementDesc::new()
                                .id(tile_id.clone())
                                .width(Sizing::Grow)
                                .height(Sizing::Fixed(height)),
                        ),
                    );
                }
                kit::label(
                    ui,
                    &shot.instance_name,
                    &TextStyle {
                        wrap: TextWrap::None,
                        ..kit.style(TextRole::Small)
                    },
                );
                match super::viewer::tile_actions(ui, kit, &tile_id) {
                    Some(super::viewer::TileAction::Copy) => {
                        launcher_ui::app_commands::push(
                            launcher_ui::app_commands::AppCommand::CopyImage(shot.path.clone()),
                        );
                    }
                    Some(super::viewer::TileAction::Delete) => {
                        delete = Some(shot.key.clone());
                    }
                    None => {
                        if kit.clicked(ui, &tile_id) {
                            open = Some(shot.key.clone());
                        }
                    }
                }
            }
        }
    }
    if let Some(key) = delete {
        state.confirming_screenshot_delete = Some(key.clone());
        state.dialog = Some(Dialog::HomeScreenshot(key));
    } else if let Some(key) = open {
        state.dialog = Some(Dialog::HomeScreenshot(key));
    }
}

/// The Home gallery's viewer: the screenshot at full size with copy, folder and (confirmed) delete. Returns false
/// once it should close.
pub fn viewer(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState, key: &str) -> bool {
    let Some(shot) = state
        .home
        .screenshots()
        .into_iter()
        .find(|shot| shot.key == key)
    else {
        return false;
    };
    let viewport = ui.viewport_size();
    let max_width = (viewport.x - 160.0).clamp(320.0, 1500.0);
    let max_height = (viewport.y - 300.0).clamp(200.0, 900.0);
    let style = ui_kit::ModalStyle {
        background: kit.palette.bg_dark,
        corner_radius: kit::CORNER,
        width: Sizing::Fit,
        ..ui_kit::ModalStyle::default()
    };
    let image_key = format!("viewer::{}", shot.path.display());
    let path = shot.path.clone();
    let picture = kit.images.get(&image_key, || ImageSource::File(path), 0);
    let confirming = state.confirming_screenshot_delete.as_deref() == Some(key);
    let deleting = state.home.delete_in_flight();
    let (mut close, mut copy, mut ask, mut delete, mut cancel) =
        (false, false, false, false, false);
    let controller = (state.gamepad.viewer_pan, state.gamepad.viewer_zoom);
    let viewer = &mut state.viewer;
    let response = ui_kit::modal(ui, "home-screenshot", &style, !deleting, |ui| {
        let _column = ui.element(
            &ElementDesc::new()
                .width(Sizing::Fit)
                .height(Sizing::Fit)
                .direction(Direction::TopToBottom)
                .child_gap(kit::SPACE_MD),
        );
        match picture {
            Some(picture) => {
                super::viewer::picture(
                    ui,
                    kit,
                    viewer,
                    controller,
                    "home-screenshot::image",
                    &image_key,
                    picture,
                    max_width,
                    max_height,
                );
            }
            None if kit.images.failed(&image_key) => {
                kit.text(ui, TextRole::Danger, "This screenshot could not be opened.")
            }
            None => kit.text(ui, TextRole::Muted, "Loading…"),
        }
        kit.text(
            ui,
            TextRole::Small,
            &format!("{} · {}", shot.instance_name, shot.file_name),
        );
        if confirming {
            kit::label(
                ui,
                "Delete this screenshot from disk? This permanently removes it.",
                &TextStyle {
                    wrap: TextWrap::Words,
                    ..kit.style(TextRole::Danger)
                },
            );
            let _row = kit.row(ui, kit::SPACE_SM);
            kit.fill(ui);
            cancel = kit.button(
                ui,
                "home-screenshot::no",
                "Cancel",
                ButtonKind::Neutral,
                !deleting,
            );
            delete = kit.button(
                ui,
                "home-screenshot::yes",
                if deleting { "Deleting…" } else { "Delete" },
                ButtonKind::Danger,
                !deleting,
            );
        } else {
            let _row = kit.row(ui, kit::SPACE_SM);
            copy = kit.button(
                ui,
                "home-screenshot::copy",
                "Copy",
                ButtonKind::Neutral,
                true,
            );
            if kit.button(
                ui,
                "home-screenshot::folder",
                "Show in folder",
                ButtonKind::Neutral,
                true,
            ) && let Some(parent) = shot.path.parent()
                && let Err(err) = launcher_ui::desktop::open_in_file_manager(parent)
            {
                launcher_ui::notification::error!(
                    "screenshots",
                    "Failed to open the folder: {}",
                    err
                );
            }
            kit.fill(ui);
            ask = kit.button(
                ui,
                "home-screenshot::delete",
                "Delete",
                ButtonKind::Danger,
                true,
            );
            close = kit.button(
                ui,
                "home-screenshot::close",
                "Close",
                ButtonKind::Neutral,
                true,
            );
        }
    });
    if copy {
        launcher_ui::app_commands::push(launcher_ui::app_commands::AppCommand::CopyImage(
            shot.path.clone(),
        ));
    }
    if ask {
        state.confirming_screenshot_delete = Some(key.to_owned());
    }
    if cancel {
        state.confirming_screenshot_delete = None;
    }
    if delete {
        state.home.delete_screenshot(key);
    }
    if close || ((response.dismissed || kit.back_pressed()) && !deleting) {
        state.confirming_screenshot_delete = None;
        return false;
    }
    true
}
