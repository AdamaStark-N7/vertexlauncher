use super::*;

/// Title, file path, running/error warnings and the save status line.
pub(super) fn render_modal_header(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &ModalState,
    live: &LiveOptions,
    running: bool,
) {
    let _ = text_ui.label(
        ui,
        "game_settings_title",
        "Game settings",
        &style::modal_title(ui),
    );
    let subtitle = format!(
        "{} · Minecraft {} · {}",
        state.instance_name,
        state.game_version,
        live.path().display()
    );
    let _ = text_ui.label(
        ui,
        "game_settings_path",
        subtitle.as_str(),
        &style::muted(ui),
    );
    if running {
        let _ = text_ui.label(
            ui,
            "game_settings_running",
            "Minecraft is running. The game rewrites options.txt when you leave its options screens, so changes here can be overwritten; changes made in-game appear here as they are saved.",
            &style::warning_text(ui),
        );
    }
    if let Some(error) = live.last_error() {
        let _ = text_ui.label(
            ui,
            "game_settings_error",
            format!("Couldn't save: {error}. Will retry.").as_str(),
            &style::error_text(ui),
        );
    }
    let status = if live.has_pending() {
        "Saving…"
    } else {
        "All changes saved"
    };
    let _ = text_ui.label(ui, "game_settings_status", status, &style::caption(ui));
    ui.add_space(style::SPACE_MD);
}

/// Tabs: one row in its own horizontally scrolling strip, so a narrow dialog scrolls the
/// tabs instead of forcing the whole dialog to be as wide as the row.
pub(super) fn render_tab_strip(ui: &mut egui::Ui, text_ui: &mut TextUi, state: &mut ModalState) {
    egui::ScrollArea::horizontal()
        .scroll_source(modal_host::drag_scroll_source())
        .id_salt("game_settings_tabs_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for group in Group::ALL {
                    let selected = state.tab == Tab::Group(group);
                    if settings_widgets::selectable_chip_button(
                        text_ui,
                        ui,
                        ("game_settings_tab", group.slug()),
                        group.label(),
                        selected,
                        118.0,
                        true,
                    )
                    .clicked()
                    {
                        state.tab = Tab::Group(group);
                    }
                }
                if settings_widgets::selectable_chip_button(
                    text_ui,
                    ui,
                    "game_settings_tab_packs",
                    "Resource Packs",
                    state.tab == Tab::Packs,
                    150.0,
                    true,
                )
                .clicked()
                {
                    state.tab = Tab::Packs;
                }
                if settings_widgets::selectable_chip_button(
                    text_ui,
                    ui,
                    "game_settings_tab_sync",
                    "Sync",
                    state.tab == Tab::Sync,
                    118.0,
                    true,
                )
                .clicked()
                {
                    state.tab = Tab::Sync;
                }
            });
        });
    ui.add_space(style::SPACE_MD);
}

pub(super) fn render_search_box(ui: &mut egui::Ui, text_ui: &mut TextUi, state: &mut ModalState) {
    let mut search = state.search.clone();
    let _ = text_ui.singleline_input(ui, "game_settings_search", &mut search, &{
        let mut options = style::input_options(ui);
        options.placeholder_text = Some("Search all settings".to_owned());
        options.desired_width = Some(ui.available_width());
        options.desired_rows = 1;
        options
    });
    state.search = search;
    ui.add_space(style::SPACE_MD);
}

/// The vertically scrolling body: search results when searching, else the selected tab.
pub(super) fn render_tab_body(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
    instances: &mut InstanceStore,
    config: &Config,
) {
    // Leave room for the Close row below; otherwise the scroll area claims all the
    // remaining height and pushes Close out of the window.
    let footer_height = style::SPACE_MD * 2.0 + style::CONTROL_HEIGHT;
    egui::ScrollArea::vertical()
        .scroll_source(modal_host::drag_scroll_source())
        .id_salt("game_settings_scroll")
        .max_height((ui.available_height() - footer_height).max(80.0))
        .auto_shrink([false, false])
        .show(ui, |ui| match state.tab {
            _ if !state.search.trim().is_empty() => {
                let mut tab_state = state.clone();
                render_search_results(ui, text_ui, &mut tab_state, live);
                state.capturing = tab_state.capturing;
                state.capture_started_at_frame = tab_state.capture_started_at_frame;
            }
            Tab::Sync => render_sync_tab(ui, text_ui, state, instances, config),
            Tab::Packs => render_packs_tab(ui, text_ui, state, instances, config),
            Tab::Group(group) => {
                let mut tab_state = state.clone();
                render_group(ui, text_ui, &mut tab_state, live, group, None);
                state.capturing = tab_state.capturing;
                state.capture_started_at_frame = tab_state.capture_started_at_frame;
            }
        });
}

/// Right-aligned Close button; returns whether it was clicked.
pub(super) fn render_close_row(ui: &mut egui::Ui, text_ui: &mut TextUi) -> bool {
    let mut close = false;
    ui.add_space(style::SPACE_MD);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if text_ui
            .button(
                ui,
                "game_settings_close",
                "Close",
                &ui_foundation::secondary_button(ui, egui::vec2(110.0, style::CONTROL_HEIGHT)),
            )
            .clicked()
        {
            close = true;
        }
    });
    close
}
