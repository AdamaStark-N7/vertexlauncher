//! Window chrome: the custom title bar (window controls, account menu), the sidebar (screens and instance
//! shortcuts) and the content area that hosts the active screen.

use instances::InstanceRecord;
use launcher_ui::screens::AppScreen;
use vertex_shell::sturdy::glam::Vec4;
use vertex_shell::sturdy::ui::{
    AlignX, AlignY, Border, ChildAlignment, CornerRadius, Cursor, Direction, ElementDesc, Padding,
    Sizing, TextStyle, TextWrap, Ui,
};
use vertex_shell::sturdy::ui_kit::{self, MenuItem, PopupStyle};

use super::icons::Icon;
use super::images::ImageSource;
use super::kit::{self, Kit, MenuEntry, TextRole};
use super::{AppState, Dialog, accounts, screens};

pub const TITLE_BAR_ID: &str = "chrome::title-bar";
const MINIMIZE_ID: &str = "chrome::minimize";
const MAXIMIZE_ID: &str = "chrome::maximize";
const CLOSE_ID: &str = "chrome::close";
const ACCOUNT_ID: &str = "chrome::account";
/// Title-bar elements that must stay clickable instead of dragging the window.
pub const TITLE_BAR_CONTROLS: [&str; 4] = [MINIMIZE_ID, MAXIMIZE_ID, CLOSE_ID, ACCOUNT_ID];
const TITLE_BAR_HEIGHT: f32 = 42.0;
pub const SIDEBAR_WIDTH: f32 = 68.0;

/// The whole window: title bar on top, sidebar and the active screen below.
pub fn window(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState, maximized: bool) {
    // Rounded corners need a see-through window; an opaque one would show square corners behind them.
    let rounded = !maximized && state.config.window_transparency().is_translucent();
    let radius = if rounded {
        f32::from(state.config.window_corner_radius())
    } else {
        0.0
    };
    let _root = ui.element(
        &ElementDesc::new()
            .id("chrome::root".into())
            .width(Sizing::Grow)
            .height(Sizing::Grow)
            .direction(Direction::TopToBottom)
            .background(kit.palette.bg)
            .corner_radius(CornerRadius::all(radius))
            .border(Border::all(
                kit.palette.border_muted,
                if maximized { 0 } else { 1 },
            )),
    );
    title_bar(ui, kit, state, maximized, radius);
    // Fixed to the window's width rather than growing: a fixed size caps the minimum size Clay passes up, so a screen
    // with something too wide cannot stretch the layout (and the title bar's buttons) past the window edge. (Clipping
    // would do that too, but a clipped axis is a scroll container, which stops squeezing its children at all.)
    let _body = ui.element(
        &ElementDesc::new()
            .width(Sizing::Fixed(ui.viewport_size().x))
            .height(Sizing::Grow)
            .direction(Direction::LeftToRight),
    );
    sidebar(ui, kit, state, radius);
    content(ui, kit, state, radius);
}

fn title_bar(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState, maximized: bool, radius: f32) {
    {
        let _bar = ui.element(
            &ElementDesc::new()
                .id(TITLE_BAR_ID.into())
                .width(Sizing::Grow)
                .height(Sizing::Fixed(TITLE_BAR_HEIGHT))
                .padding(Padding {
                    left: kit::SPACE_LG,
                    ..Padding::default()
                })
                .child_gap(kit::SPACE_SM)
                .align(ChildAlignment::new(AlignX::Left, AlignY::Center))
                .direction(Direction::LeftToRight)
                .background(kit.palette.bg_dark.with_w(kit.palette.bg.w))
                .corner_radius(CornerRadius {
                    top_left: radius,
                    top_right: radius,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                }),
        );
        kit.text(ui, TextRole::Body, "Vertex Launcher");
        if let Some(instance) = state
            .screen
            .eq(&AppScreen::Instance)
            .then(|| selected_name(state))
            .flatten()
        {
            kit.text(ui, TextRole::Muted, &format!("· {instance}"));
        }
        kit.fill(ui);
        active_user_button(ui, kit, state);
        account_button(ui, kit, state);
        window_button(ui, kit, MINIMIZE_ID, Some(Icon::Minus), false);
        window_button(
            ui,
            kit,
            MAXIMIZE_ID,
            maximized.then_some(Icon::Restore),
            false,
        );
        window_button(ui, kit, CLOSE_ID, Some(Icon::Close), true);
    }
    for id in [MINIMIZE_ID, MAXIMIZE_ID, CLOSE_ID] {
        if !kit.clicked(ui, id) {
            continue;
        }
        let mut engine = ui.engine();
        if let Some(mut window) = engine.primary_window_control() {
            match id {
                MINIMIZE_ID => {
                    let _ = window.minimize();
                }
                MAXIMIZE_ID => {
                    let _ = window.toggle_maximized();
                }
                _ => window.request_close(),
            }
        }
    }
    kit.tooltip(ui, MINIMIZE_ID, "Minimize");
    kit.tooltip(
        ui,
        MAXIMIZE_ID,
        if maximized {
            "Restore down"
        } else {
            "Maximize"
        },
    );
    kit.tooltip(ui, CLOSE_ID, "Close");
    kit.tooltip(ui, ACCOUNT_ID, "Accounts and sign-in");
    accounts::account_menu(ui, kit, state, ACCOUNT_ID);
}

const ACTIVE_USER_ID: &str = "chrome::active-user";

/// While the signed-in account is playing, a button that opens that game's console.
fn active_user_button(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    let Some(account) = state
        .auth
        .active_launch_context()
        .map(|context| context.account_key)
    else {
        return;
    };
    if installation::running_instance_for_account(&account).is_none() {
        return;
    }
    let hovered = ui.hovered(ACTIVE_USER_ID);
    {
        let _button = ui.element(
            &ElementDesc::new()
                .id(ACTIVE_USER_ID.into())
                .width(Sizing::Fit)
                .height(Sizing::Fixed(30.0))
                .padding(Padding::symmetric(kit::SPACE_MD, 0))
                .child_gap(kit::SPACE_SM)
                .align(ChildAlignment::center())
                .background(if hovered {
                    kit.palette.hovered_fill()
                } else {
                    kit.palette.bg_light
                })
                .corner_radius(CornerRadius::all(8.0))
                .cursor(Cursor::Pointer),
        );
        kit.icon(ui, Icon::Terminal2, 16.0, kit.palette.text);
        launcher_ui::privacy::without_name_scrubbing(|| {
            kit.text(ui, TextRole::Small, "player active")
        });
    }
    kit.tooltip(
        ui,
        ACTIVE_USER_ID,
        "Open the console of the game you are playing",
    );
    if kit.clicked(ui, ACTIVE_USER_ID) {
        state.open(AppScreen::Console);
        let name = state.auth.display_name().map(str::to_owned);
        let _ = launcher_ui::console::activate_tab_for_user(Some(&account), name.as_deref());
    }
}

fn screen_icon(screen: AppScreen) -> Icon {
    match screen {
        AppScreen::Home => Icon::Home,
        AppScreen::Library | AppScreen::Instance => Icon::Library,
        AppScreen::Discover | AppScreen::DiscoverDetail | AppScreen::ContentBrowser => {
            Icon::Discover
        }
        AppScreen::Skins => Icon::Skins,
        AppScreen::Settings => Icon::Settings,
        AppScreen::Legal => Icon::Legal,
        AppScreen::Console => Icon::Terminal,
    }
}

fn selected_name(state: &AppState) -> Option<String> {
    let id = state.selected_instance_id.as_deref()?;
    state
        .instances
        .find(id)
        .map(|instance| instance.name.clone())
}

/// `None` draws the plain square of a maximize button.
fn window_button(ui: &mut Ui<'_>, kit: &Kit, id: &str, icon: Option<Icon>, danger: bool) {
    let hovered = ui.hovered(id);
    let background = match (hovered, danger) {
        (true, true) => kit.palette.danger,
        (true, false) => kit.palette.hovered_fill(),
        _ => Vec4::ZERO,
    };
    let _button = ui.element(
        &ElementDesc::new()
            .id(id.into())
            .width(Sizing::Fixed(46.0))
            .height(Sizing::Grow)
            .align(ChildAlignment::center())
            .background(background)
            .cursor(Cursor::Pointer),
    );
    let color = if hovered && danger {
        kit.palette.on_primary
    } else {
        kit.palette.text
    };
    match icon {
        Some(icon) => kit.icon(ui, icon, 14.0, color),
        None => drop(
            ui.element(
                &ElementDesc::new()
                    .width(Sizing::Fixed(11.0))
                    .height(Sizing::Fixed(11.0))
                    .border(Border::all(color, 1))
                    .corner_radius(CornerRadius::all(2.0)),
            ),
        ),
    }
}

fn account_button(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    let label = accounts::active_account_label(state);
    let hovered = ui.hovered(ACCOUNT_ID);
    {
        let _button = ui.element(
            &ElementDesc::new()
                .id(ACCOUNT_ID.into())
                .width(Sizing::Fit)
                .height(Sizing::Fixed(30.0))
                .padding(Padding::symmetric(kit::SPACE_MD, 0))
                .align(ChildAlignment::center())
                .background(if hovered {
                    kit.palette.hovered_fill()
                } else {
                    kit.palette.bg_light
                })
                .corner_radius(CornerRadius::all(10.0))
                .cursor(Cursor::Pointer),
        );
        if let Some(avatar) = state
            .auth
            .avatar_png()
            .filter(|_| !state.streamer_mode())
            .map(std::sync::Arc::<[u8]>::from)
        {
            let key = format!(
                "avatar::{}",
                state.auth.active_account_key().unwrap_or_default()
            );
            kit.image(
                ui,
                "chrome::avatar",
                &key,
                || ImageSource::Bytes(avatar),
                22.0,
                22.0,
                5.0,
            );
        }
        kit.text_clipped(ui, TextRole::Body, &label, 220.0);
    }
    if kit.clicked(ui, ACCOUNT_ID) {
        state.account_menu.toggle();
    }
}

// -- Sidebar ------------------------------------------------------------------------------------------------------

fn sidebar(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState, radius: f32) {
    {
        let _sidebar = ui.element(
            &ElementDesc::new()
                .id("chrome::sidebar".into())
                .width(Sizing::Fixed(SIDEBAR_WIDTH))
                .height(Sizing::Grow)
                .padding(Padding::all(kit::SPACE_MD))
                .child_gap(kit::SPACE_XS)
                .direction(Direction::TopToBottom)
                .background(kit.palette.bg_dark.with_w(kit.palette.bg.w))
                .corner_radius(CornerRadius {
                    bottom_left: radius,
                    ..CornerRadius::all(0.0)
                })
                .border(Border {
                    right: 1,
                    ..Border::all(kit.palette.border_muted, 0)
                }),
        );
        for screen in AppScreen::FIXED_NAV {
            let selected = state.screen == screen
                || (screen == AppScreen::Discover && state.screen == AppScreen::DiscoverDetail);
            let id = format!("nav::{}", screen.label());
            if kit.nav_icon(ui, &id, screen_icon(screen), selected) {
                state.open(screen);
            }
            kit.tooltip(ui, &id, screen.label());
        }
        kit.separator(ui);
        {
            let _create = ui.element(
                &ElementDesc::new()
                    .width(Sizing::Grow)
                    .height(Sizing::Fit)
                    .align(ChildAlignment::center()),
            );
            if kit.nav_icon(
                ui,
                "nav::create-instance",
                Icon::Plus,
                state.profile_menu.is_open(),
            ) {
                state.profile_menu.toggle();
            }
        }
        kit.tooltip(ui, "nav::create-instance", "Profile actions");
        {
            let _list = ui.element(
                &ElementDesc::new()
                    .id("nav::instances".into())
                    .width(Sizing::Grow)
                    .height(Sizing::Grow)
                    .direction(Direction::TopToBottom)
                    .child_gap(kit::SPACE_XS)
                    .clip(vertex_shell::sturdy::ui::Clip::vertical()),
            );
            let shortcuts: Vec<InstanceRecord> = state.instances.instances.clone();
            for instance in shortcuts {
                let id = instance.id.clone();
                let element_id = format!("nav::instance::{id}");
                let selected = state.screen == AppScreen::Instance
                    && state.selected_instance_id.as_deref() == Some(id.as_str());
                if kit.nav_thumbnail(ui, &element_id, selected, |ui, kit| {
                    screens::library::thumbnail(
                        ui,
                        kit,
                        &instance,
                        &format!("{element_id}::thumb"),
                        34.0,
                    )
                }) {
                    state.open_instance(&id);
                }
                kit.tooltip(ui, &element_id, &instance.name);
                if let Some(at) = kit.secondary(ui, &element_id) {
                    state.instance_menu_target = Some(id.clone());
                    state.instance_menu.open_at(at);
                }
            }
        }
    }
    profile_actions_menu(ui, kit, state);
    instance_context_menu(ui, kit, state);
}

/// The menu under the sidebar's "+" button.
fn profile_actions_menu(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    let style = PopupStyle {
        background: kit.palette.bg_dark,
        border: kit.palette.border_muted,
        corner_radius: 10.0,
        padding: Padding::all(kit::SPACE_SM),
        ..PopupStyle::default()
    };
    let mut menu = std::mem::take(&mut state.profile_menu);
    let mut choice = None;
    ui_kit::popup(
        ui,
        "chrome::profile-menu",
        "nav::create-instance",
        &mut menu,
        &style,
        |ui, popup| {
            let _column = ui.element(
                &ElementDesc::new()
                    .width(Sizing::Fit)
                    .height(Sizing::Fit)
                    .direction(Direction::TopToBottom)
                    .child_gap(kit::SPACE_XS),
            );
            // At least this wide; longer entries widen the menu.
            drop(
                ui.element(
                    &ElementDesc::new()
                        .width(Sizing::Fixed(200.0))
                        .height(Sizing::Fixed(0.0)),
                ),
            );
            for (index, label) in [
                "Create from scratch",
                "Import profile",
                "Link existing Vertex profile",
            ]
            .into_iter()
            .enumerate()
            {
                if kit.nav_item_ex(
                    ui,
                    &format!("chrome::profile-menu::{index}"),
                    Some(Icon::Plus),
                    label,
                    false,
                    false,
                ) {
                    choice = Some(index);
                    popup.close();
                }
            }
        },
    );
    state.profile_menu = menu;
    match choice {
        Some(0) => state.dialog = Some(Dialog::CreateInstance),
        Some(1) => state.dialog = Some(Dialog::Import),
        Some(2) => super::link_profile::start(state),
        _ => {}
    }
}

fn instance_context_menu(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    let Some(target) = state.instance_menu_target.clone() else {
        return;
    };
    let items = [
        MenuEntry::new("Open instance menu").icon(Icon::Library),
        MenuEntry::new("Open folder").icon(Icon::Folder),
        MenuEntry::new("Copy command line").icon(Icon::Terminal),
        MenuEntry::new("Copy Steam launch options").icon(Icon::Steam),
        MenuEntry::new("Delete instance").icon(Icon::Trash).danger(),
    ];
    if let Some(choice) = kit.context_menu(
        ui,
        "chrome::instance-menu",
        &mut state.instance_menu,
        &items,
    ) {
        match choice {
            0 => state.open_instance(&target),
            1 => screens::library::open_folder(state, &target),
            2 => screens::library::copy_quick_launch(ui, state, &target, false),
            3 => screens::library::copy_quick_launch(ui, state, &target, true),
            _ => state.dialog = Some(Dialog::ConfirmDelete(target.clone())),
        }
    }
    // Keep the target while the menu folds away, or it would vanish instead of animating closed.
    if !state.instance_menu.is_open() && !kit.menu_visible("chrome::instance-menu") {
        state.instance_menu_target = None;
    }
}

// -- Content ------------------------------------------------------------------------------------------------------

fn content(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState, radius: f32) {
    // Clipping is rectangular, so scrolled content would poke out past the window's rounded bottom corners; stop it
    // above the corner band instead.
    let _content = ui.element(
        &ElementDesc::new()
            .id("chrome::content".into())
            .width(Sizing::Fixed(
                (ui.viewport_size().x - SIDEBAR_WIDTH).max(0.0),
            ))
            .height(Sizing::Grow)
            .padding(Padding {
                bottom: radius as u16,
                ..Padding::default()
            })
            .direction(Direction::TopToBottom),
    );
    if state.screen != AppScreen::Skins {
        // The preview texture is only kept while the screen is showing.
        state.skins_view.hidden(ui);
    }
    match state.screen {
        AppScreen::Library => screens::library::render(ui, kit, state),
        AppScreen::Instance => screens::instance::render(ui, kit, state),
        // Static launcher text only; nothing here can show an account name.
        AppScreen::Settings => launcher_ui::privacy::without_name_scrubbing(|| {
            screens::settings::render(ui, kit, state)
        }),
        AppScreen::Console => screens::console::render(ui, kit, state),
        AppScreen::Legal => {
            launcher_ui::privacy::without_name_scrubbing(|| screens::legal::render(ui, kit))
        }
        AppScreen::Home => screens::home::render(ui, kit, state),
        AppScreen::Discover => screens::discover::browse(ui, kit, state),
        AppScreen::DiscoverDetail => screens::discover::detail(ui, kit, state),
        AppScreen::ContentBrowser => screens::content_browser::render(ui, kit, state),
        AppScreen::Skins => screens::skins::render(ui, kit, state),
    }
}
