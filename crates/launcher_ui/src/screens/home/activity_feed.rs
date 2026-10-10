use super::*;

pub(super) fn render_activity_feed(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    state: &HomeState,
    auth: &PlayerAuthContext<'_>,
    streamer_mode: bool,
    output: &mut HomeOutput,
    requested_rescan: &mut bool,
    metrics: HomeUiMetrics,
) {
    let mut title_style = crate::ui::style::body_strong(ui);
    title_style.wrap = false;
    let _ = text_ui.label(ui, "home_activity_title", "Worlds & Servers", &title_style);
    ui.add_space(6.0);

    if state.worlds.is_empty() && state.servers.is_empty() {
        let _ = text_ui.label(
            ui,
            "home_activity_empty",
            "No worlds or servers found in any instance.",
            &style::muted(ui),
        );
        return;
    }

    let now_ms = current_time_millis();
    let mut favorites: Vec<HomeEntryRef<'_>> = state
        .worlds
        .iter()
        .filter(|world| world.favorite)
        .map(HomeEntryRef::World)
        .collect();
    favorites.extend(
        state
            .servers
            .iter()
            .filter(|server| server.favorite)
            .map(HomeEntryRef::Server),
    );
    favorites.sort_by(|a, b| {
        b.last_used_at_ms()
            .unwrap_or(0)
            .cmp(&a.last_used_at_ms().unwrap_or(0))
            .then_with(|| a.primary_label().cmp(b.primary_label()))
    });

    let mut entries: Vec<HomeEntryRef<'_>> = state
        .worlds
        .iter()
        .filter(|world| !world.favorite)
        .map(HomeEntryRef::World)
        .collect();
    entries.extend(
        state
            .servers
            .iter()
            .filter(|server| !server.favorite)
            .map(HomeEntryRef::Server),
    );
    entries.sort_by(|a, b| {
        b.last_used_at_ms()
            .unwrap_or(0)
            .cmp(&a.last_used_at_ms().unwrap_or(0))
            .then_with(|| a.primary_label().cmp(b.primary_label()))
    });

    let scroll_output = egui::ScrollArea::vertical()
        .scroll_source(modal_host::drag_scroll_source())
        .id_salt("home_activity_scroll")
        .max_height(ui.available_height().max(180.0))
        .show_viewport(ui, |ui, viewport| {
            if !favorites.is_empty() {
                let _ = text_ui.label(
                    ui,
                    "home_activity_favorites_title",
                    "Favorites",
                    &style::body_strong(ui),
                );
                ui.add_space(4.0);
                for (index, entry) in favorites.into_iter().enumerate() {
                    render_activity_entry_row_if_visible(
                        ui,
                        text_ui,
                        entry,
                        state,
                        now_ms,
                        streamer_mode,
                        ("home_favorite", index),
                        instances,
                        output,
                        requested_rescan,
                        auth,
                        metrics,
                        viewport,
                    );
                }
                ui.separator();
                ui.add_space(8.0);
            }

            if entries.is_empty() {
                let _ = text_ui.label(
                    ui,
                    "home_activity_recent_empty",
                    "No recent worlds or servers.",
                    &style::muted(ui),
                );
                return;
            }

            let _ = text_ui.label(
                ui,
                "home_activity_recent_title",
                "Recent",
                &style::body_strong(ui),
            );
            ui.add_space(4.0);
            for (index, entry) in entries.into_iter().enumerate() {
                render_activity_entry_row_if_visible(
                    ui,
                    text_ui,
                    entry,
                    state,
                    now_ms,
                    streamer_mode,
                    ("home_recent", index),
                    instances,
                    output,
                    requested_rescan,
                    auth,
                    metrics,
                    viewport,
                );
            }
        });
    textui_egui::make_gamepad_scrollable(ui.ctx(), &scroll_output);
}

pub(super) fn render_activity_entry_row_if_visible(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    entry: HomeEntryRef<'_>,
    state: &HomeState,
    now_ms: u64,
    streamer_mode: bool,
    id_source: impl std::hash::Hash + Copy,
    instances: &mut InstanceStore,
    output: &mut HomeOutput,
    requested_rescan: &mut bool,
    auth: &PlayerAuthContext<'_>,
    metrics: HomeUiMetrics,
    viewport: egui::Rect,
) {
    const ROW_GAP: f32 = 2.0;
    let row_height = activity_entry_row_height(ui, metrics, &entry);
    let group = home_grouping::group_instances(&entry);
    let buttons_layout =
        home_grouping::instance_buttons_layout(ui, text_ui, group, ui.available_width());
    let total_height = row_height + ROW_GAP + buttons_layout.total_height;
    let is_world = matches!(entry, HomeEntryRef::World(_));
    let row_top = ui.cursor().top();
    let row_bottom = row_top + total_height;
    if row_bottom < viewport.top() || row_top > viewport.bottom() {
        ui.add_space(total_height);
        return;
    }

    match entry {
        HomeEntryRef::World(world) => render_world_row(
            ui,
            text_ui,
            world,
            now_ms,
            (id_source, "world"),
            instances,
            output,
            requested_rescan,
            auth,
            metrics,
        ),
        HomeEntryRef::Server(server) => render_server_row(
            ui,
            text_ui,
            server,
            state
                .server_pings
                .get(&normalize_server_address(&server.address)),
            now_ms,
            streamer_mode,
            (id_source, "server"),
            instances,
            output,
            requested_rescan,
            auth,
            metrics,
        ),
    }
    if let Some(chosen) = home_grouping::render_instance_buttons(
        ui,
        text_ui,
        id_source,
        group,
        &buttons_layout,
        !auth.token_refresh_in_progress,
    ) {
        queue_launch_intent(
            ui.ctx(),
            PendingLaunchIntent {
                nonce: current_time_millis(),
                instance_id: chosen.instance_id.clone(),
                quick_play_singleplayer: is_world.then(|| chosen.target.clone()),
                quick_play_multiplayer: (!is_world).then(|| chosen.target.clone()),
            },
        );
        output.selected_instance_id = Some(chosen.instance_id);
        output.requested_screen = Some(AppScreen::Library);
    }
    ui.add_space(ROW_GAP);
}
