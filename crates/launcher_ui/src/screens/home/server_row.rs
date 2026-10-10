use super::*;

pub(super) fn render_server_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    server: &ServerEntry,
    ping: Option<&ServerPingSnapshot>,
    now_ms: u64,
    _streamer_mode: bool,
    id_source: impl std::hash::Hash + Copy,
    instances: &mut InstanceStore,
    output: &mut HomeOutput,
    requested_rescan: &mut bool,
    auth: &PlayerAuthContext<'_>,
    metrics: HomeUiMetrics,
) {
    let server_meta_full = server_meta_line(server, ping, now_ms);
    let player_count = server_player_count_text(ping);
    let name_label_options = activity_entry_name_label_options(ui);
    let meta_label_options = activity_entry_meta_label_options(ui);
    let row_height = activity_entry_min_row_height(
        ui,
        metrics,
        &name_label_options,
        &meta_label_options,
        SERVER_PING_ICON_SIZE,
    );
    let mut star_clicked = false;
    let row_response = render_clickable_entry_row(ui, (id_source, "row"), row_height, |ui| {
        let star_response =
            render_favorite_star_button(ui, (id_source, "server_star"), server.favorite);
        if crate::ui::style::hover_tip(ui, star_response, "Toggle server favorite").clicked() {
            star_clicked = true;
        }
        ui.add_space(ACTIVITY_ENTRY_CONTENT_GAP);
        render_entry_thumbnail(
            ui,
            (id_source, "thumb"),
            server.icon_png.clone().map(|bytes| {
                (
                    home_server_icon_uri(server.instance_id.as_str(), server.favorite_id.as_str()),
                    bytes,
                )
            }),
            assets::TERMINAL_SVG,
            ACTIVITY_ENTRY_THUMBNAIL_SIZE,
            ACTIVITY_ENTRY_THUMBNAIL_SIZE,
        );
        ui.add_space(ACTIVITY_ENTRY_CONTENT_GAP);
        let text_max_width = (ui.available_width() - SERVER_STATUS_GROUP_WIDTH - 8.0).max(80.0);
        let content_height = ui.available_height();
        render_activity_text_stack_sized(
            ui,
            text_ui,
            id_source,
            server.server_name.as_str(),
            server_meta_full.as_str(),
            text_max_width,
            content_height,
            &name_label_options,
            &meta_label_options,
        );
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(
            egui::vec2(SERVER_STATUS_GROUP_WIDTH, content_height),
            Layout::right_to_left(egui::Align::Center),
            |ui| {
                render_server_ping_icon(ui, ping);
                ui.add_space(SERVER_STATUS_GROUP_GAP);
                let players_response = text_ui.label(
                    ui,
                    (id_source, "players"),
                    player_count.as_deref().unwrap_or(""),
                    &meta_label_options,
                );
                if let Some(tip) = server_player_list_tip(ping) {
                    let _ = crate::ui::style::hover_tip(ui, players_response, tip);
                }
            },
        );
    });
    if star_clicked {
        for member in &server.instances {
            let _ = set_server_favorite(
                instances,
                member.instance_id.as_str(),
                server.favorite_id.as_str(),
                !server.favorite,
            );
        }
        *requested_rescan = true;
        return;
    }
    if row_response.clicked() && auth.token_refresh_in_progress {
        crate::notification::info!(
            "home/launch",
            "Waiting for the account token refresh to finish before launching."
        );
    }
    if row_response.clicked() && !auth.token_refresh_in_progress {
        queue_launch_intent(
            ui.ctx(),
            PendingLaunchIntent {
                nonce: current_time_millis(),
                instance_id: server.instance_id.clone(),
                quick_play_singleplayer: None,
                quick_play_multiplayer: Some(server.address.clone()),
            },
        );
        output.selected_instance_id = Some(server.instance_id.clone());
        output.requested_screen = Some(AppScreen::Library);
    }
    let context_id = ui.make_persistent_id((id_source, "server_context"));
    if let Some(anchor) = context_menu::trigger_anchor(ui.ctx(), &row_response) {
        context_menu::request(
            ui.ctx(),
            ContextMenuRequest::new(
                context_id,
                anchor,
                vec![
                    ContextMenuItem::new_with_icon(
                        "copy_server_launch_command",
                        "Copy command line",
                        assets::TERMINAL_SVG,
                    ),
                    ContextMenuItem::new_with_icon(
                        "copy_server_steam_launch_options",
                        "Copy Steam launch options",
                        assets::STEAM_SVG,
                    ),
                    ContextMenuItem::new_with_icon(
                        "server_sync_options",
                        "Sync options…",
                        assets::USERS_GROUP_SVG,
                    ),
                ],
            ),
        );
    }
    match context_menu::take_invocation(ui.ctx(), context_id).as_deref() {
        Some("copy_server_launch_command") => {
            copy_server_launch_command(ui.ctx(), server, auth);
        }
        Some("copy_server_steam_launch_options") => {
            copy_server_steam_launch_options(ui.ctx(), server, auth);
        }
        Some("server_sync_options") => {
            home_server_sync::request_server_sync_modal(ui.ctx(), server, instances);
        }
        _ => {}
    }
}

pub(super) fn server_player_list_tip(ping: Option<&ServerPingSnapshot>) -> Option<String> {
    let snapshot = ping?;
    let online = snapshot.players_online?;
    if snapshot.players_sample.is_empty() {
        return Some(if online == 0 {
            "No players online".to_owned()
        } else {
            "The server did not share which players are online".to_owned()
        });
    }
    let mut tip = format!("Players online ({online}):");
    for name in &snapshot.players_sample {
        tip.push_str("\n");
        tip.push_str(name);
    }
    let hidden = (online as usize).saturating_sub(snapshot.players_sample.len());
    if hidden > 0 {
        tip.push_str(&format!("\n…and {hidden} more"));
    }
    Some(tip)
}

pub(super) fn server_player_count_text(ping: Option<&ServerPingSnapshot>) -> Option<String> {
    let snapshot = ping?;
    Some(format!(
        "{}/{}",
        snapshot.players_online?, snapshot.players_max?
    ))
}
