//! Console: the launcher log and one tab per running instance, from `launcher_ui::console` (the same store the egui
//! console reads). Only the visible lines are built, so huge game logs stay cheap.

use std::time::Duration;

use launcher_ui::console;
use launcher_ui::log_format::{self, LogLevel};
use vertex_shell::sturdy::MouseButton;
use vertex_shell::sturdy::glam::{Vec2, Vec4};
use vertex_shell::sturdy::ui::{
    AlignX, AlignY, ChildAlignment, Clip, CornerRadius, Cursor, Direction, ElementDesc, Padding,
    Sizing, TextStyle, TextWrap, Ui,
};
use vertex_shell::sturdy::ui_kit;

use super::super::AppState;
use super::super::icons::Icon;
use super::super::kit::{self, ButtonKind, Kit, MenuEntry, TextRole};

pub const LINES_ID: &str = "console::lines";

pub struct ConsoleView {
    /// Keep the newest line in view as lines arrive.
    follow: bool,
    last_line_count: usize,
}

impl Default for ConsoleView {
    fn default() -> Self {
        Self {
            follow: true,
            last_line_count: 0,
        }
    }
}

pub fn render(ui: &mut Ui<'_>, kit: &mut Kit, state: &mut AppState) {
    // Console lines arrive constantly while a game runs and do not wake the UI themselves; poll while visible.
    vertex_shell::wake_after(Duration::from_millis(250));
    let snapshot = console::snapshot();
    let _page = ui.element(
        &ElementDesc::new()
            .width(Sizing::Grow)
            .height(Sizing::Grow)
            .direction(Direction::TopToBottom)
            .child_gap(kit::SPACE_MD)
            .padding(Padding::all(kit::SPACE_XL)),
    );
    {
        let _header = kit.row(ui, kit::SPACE_SM);
        kit.fill(ui);
        let mut follow = state.console.follow;
        kit.text(ui, TextRole::Muted, "Follow output");
        if let Some(next) = kit.switch(ui, "console::follow", follow, true) {
            follow = next;
        }
        state.console.follow = follow;
        if kit.button(
            ui,
            "console::copy",
            "Copy all",
            ButtonKind::Neutral,
            !snapshot.active_lines.is_empty(),
        ) {
            let text = snapshot
                .active_lines
                .iter()
                .map(|line| log_format::strip_ansi(line))
                .collect::<Vec<_>>()
                .join("\n");
            let _ = ui
                .engine()
                .primary_window_control()
                .map(|mut window| window.set_clipboard_text(&text));
            launcher_ui::notification::info!(
                "console",
                "Copied {} console lines.",
                snapshot.active_lines.len()
            );
        }
    }
    {
        let _tabs = kit.row(ui, kit::SPACE_XS);
        for tab in &snapshot.tabs {
            let id = format!("console::tab::{}", tab.id);
            let selected = tab.id == snapshot.active_tab_id;
            let background = if selected {
                kit.palette.primary.with_w(0.28)
            } else if ui.hovered(&id) {
                kit.palette.hovered_fill()
            } else {
                kit.palette.bg_light
            };
            {
                let _tab = ui.element(
                    &ElementDesc::new()
                        .id(id.clone())
                        .width(Sizing::Fit)
                        .height(Sizing::Fixed(30.0))
                        .padding(Padding::symmetric(kit::SPACE_MD, 0))
                        .child_gap(kit::SPACE_SM)
                        .align(ChildAlignment::new(AlignX::Left, AlignY::Center))
                        .background(background)
                        .corner_radius(CornerRadius::all(8.0))
                        .cursor(Cursor::Pointer),
                );
                kit.text_clipped(ui, TextRole::Body, &tab.label, 220.0);
                if tab.can_close {
                    let close = format!("{id}::close");
                    {
                        let _close = ui.element(
                            &ElementDesc::new()
                                .id(close.clone())
                                .width(Sizing::Fixed(18.0))
                                .height(Sizing::Fixed(18.0))
                                .align(ChildAlignment::center())
                                .cursor(Cursor::Pointer),
                        );
                        kit.text(ui, TextRole::Small, "✕");
                    }
                    if ui.clicked(&close) {
                        console::close_tab(&tab.id);
                        continue;
                    }
                }
            }
            if kit.clicked(ui, &id) {
                console::set_active_tab(&tab.id);
            }
        }
    }

    let follow = state.console.follow;
    let (count, left_bottom) = log_lines_view(
        ui,
        kit,
        LINES_ID,
        &snapshot.active_lines,
        "Nothing logged yet.",
        follow,
        state.console.last_line_count,
    );
    state.console.last_line_count = count;
    if follow && left_bottom {
        // The user scrolled up to read something; following would drag the view back down under them.
        state.console.follow = false;
    }
}

/// A virtualised, colour-coded log viewer: only the visible lines are built. Lines are coloured by their ANSI codes
/// or, failing that, their severity. Returns the number of lines shown.
pub fn log_lines(ui: &mut Ui<'_>, kit: &mut Kit, id: &str, lines: &[String], empty_text: &str) {
    let _ = log_lines_view(ui, kit, id, lines, empty_text, false, lines.len());
}

/// Returns the number of lines shown and whether the view stopped showing the whole bottom line while following.
fn log_lines_view(
    ui: &mut Ui<'_>,
    kit: &mut Kit,
    id: &str,
    lines: &[String],
    empty_text: &str,
    follow: bool,
    last_count: usize,
) -> (usize, bool) {
    let line_style = TextStyle {
        wrap: TextWrap::None,
        ..kit.style(TextRole::Body)
    };
    let row_height = (f32::from(line_style.font_size) * 1.45).ceil();
    let levels = kit.log_levels(id, lines);
    let mut selection = kit.log_selections.remove(id).unwrap_or_default();
    // Keep the selection inside the log when lines are cleared.
    if selection.range().is_some_and(|(_, end)| end >= lines.len()) {
        selection.clear();
    }
    let selected = selection.range();
    let mut hovered_line = None;
    let mut pressed_line = None;
    let mut menu_request: Option<(usize, Vec2)> = None;
    {
        let _list = ui.element(
            &ElementDesc::new()
                .id(id.into())
                .width(Sizing::Grow)
                .height(Sizing::Grow)
                .direction(Direction::TopToBottom)
                .padding(Padding::symmetric(kit::SPACE_MD, kit::SPACE_SM))
                .clip(Clip::both())
                .background(kit.palette.bg_dark)
                .corner_radius(CornerRadius::all(kit::CORNER)),
        );
        kit.scrollbar(ui, id);
        if lines.is_empty() {
            kit.text(ui, TextRole::Muted, empty_text);
        }
        let visible = ui_kit::visible_rows(ui, id, row_height, lines.len(), 8);
        ui_kit::spacer(ui, visible.before);
        for index in visible.rows.clone() {
            let level = levels.get(index).copied().flatten();
            let default_color = match level {
                Some(LogLevel::Fatal | LogLevel::Error) => kit.palette.danger,
                Some(LogLevel::Warn) => kit.palette.warning,
                Some(LogLevel::Info) => kit.palette.info,
                Some(LogLevel::Debug | LogLevel::Trace) => kit.palette.text_muted,
                None => kit.palette.text,
            };
            let row_id = format!("{id}::row::{index}");
            let in_selection = selected.is_some_and(|(start, end)| (start..=end).contains(&index));
            {
                let _row = ui.element(
                    &ElementDesc::new()
                        .id(row_id.clone())
                        .width(Sizing::Grow)
                        .height(Sizing::Fixed(row_height))
                        .direction(Direction::LeftToRight)
                        .align(ChildAlignment::new(AlignX::Left, AlignY::Center))
                        .background(if in_selection {
                            kit.palette.primary.with_w(0.30)
                        } else {
                            Vec4::ZERO
                        }),
                );
                for segment in log_format::ansi_segments(&lines[index]) {
                    let color = segment.foreground.map_or(default_color, |[r, g, b]| {
                        Vec4::new(
                            f32::from(r) / 255.0,
                            f32::from(g) / 255.0,
                            f32::from(b) / 255.0,
                            1.0,
                        )
                    });
                    kit::label(
                        ui,
                        &segment.text,
                        &TextStyle {
                            color,
                            ..line_style
                        },
                    );
                }
            }
            if ui.hovered(&row_id) {
                hovered_line = Some(index);
            }
            if ui_kit::pressed(ui, &row_id, MouseButton::Left) {
                pressed_line = Some(index);
            }
            if let Some(at) = kit.secondary(ui, &row_id) {
                menu_request = Some((index, at));
            }
        }
        ui_kit::spacer(ui, visible.after);
    }

    // Drag over lines to select them; a plain click clears the selection.
    if let Some(line) = pressed_line {
        selection.anchor = Some(line);
        selection.head = Some(line);
        selection.dragging = true;
        selection.moved = false;
    }
    if selection.dragging {
        if ui.pointer_down() {
            if let Some(line) = hovered_line
                && selection.head != Some(line)
            {
                selection.head = Some(line);
                selection.moved = true;
            }
            kit.keep_awake_for(Duration::from_millis(16));
        } else {
            selection.dragging = false;
            if !selection.moved {
                selection.clear();
            }
        }
    }
    if let Some((line, at)) = menu_request {
        selection.menu_line = line;
        selection.menu.open_at(at);
    }
    let menu_id = format!("{id}::menu");
    let has_selection = selection.range().is_some();
    let items: Vec<MenuEntry> = if has_selection {
        vec![
            MenuEntry::new("Copy selection").icon(Icon::Copy),
            MenuEntry::new("Clear selection").icon(Icon::Close),
        ]
    } else {
        vec![MenuEntry::new("Copy line").icon(Icon::Copy)]
    };
    if let Some(choice) = kit.context_menu(ui, &menu_id, &mut selection.menu, &items) {
        let copy = |ui: &mut Ui<'_>, from: usize, to: usize, what: &str| {
            let text = lines[from..=to.min(lines.len().saturating_sub(1))]
                .iter()
                .map(|line| log_format::strip_ansi(line))
                .collect::<Vec<_>>()
                .join("\n");
            let _ = ui
                .engine()
                .primary_window_control()
                .map(|mut window| window.set_clipboard_text(&text));
            launcher_ui::notification::info!("console", "Copied {what}.");
        };
        match (has_selection, choice, selection.range()) {
            (true, 0, Some((start, end))) if !lines.is_empty() => {
                copy(ui, start, end, "the selection")
            }
            (true, _, _) => selection.clear(),
            (false, _, _) if !lines.is_empty() => {
                let line = selection.menu_line.min(lines.len() - 1);
                copy(ui, line, line, "the line");
            }
            _ => {}
        }
    }
    let selecting = selection.active();
    // Following means the last line is fully on screen. If it stays out of view while no lines are arriving, or the
    // wheel scrolls up over the log, the user has moved away.
    let at_bottom = ui.scroll_metrics(id).is_none_or(|m| {
        m.content_size.y <= m.container_size.y + 1.0
            || (-m.offset.y) + m.container_size.y >= m.content_size.y - 2.0
    });
    let wheel_up = ui.hovered(id) && ui.engine().input().wheel_delta().y > 0.0;
    if follow && lines.len() == last_count && (!at_bottom || wheel_up) {
        selection.away_frames += 1;
    } else {
        selection.away_frames = 0;
    }
    let left_bottom = follow && (wheel_up || selection.away_frames >= 4);
    kit.log_selections.insert(id.to_owned(), selection);
    // Following the newest line would scroll the selection away, so it pauses while one exists.
    if follow && !selecting && lines.len() != last_count {
        // Scroll offsets are negative when scrolled down; a huge value clamps to the end.
        ui.set_scroll_offset(id, Vec2::new(0.0, -1.0e9));
    }
    (lines.len(), left_bottom)
}
