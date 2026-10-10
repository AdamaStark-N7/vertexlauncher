use super::*;

pub(super) fn render_world_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    world: &WorldEntry,
    now_ms: u64,
    id_source: impl std::hash::Hash + Copy,
    instances: &mut InstanceStore,
    output: &mut HomeOutput,
    requested_rescan: &mut bool,
    auth: &PlayerAuthContext<'_>,
    metrics: HomeUiMetrics,
) {
    let name_label_options = activity_entry_name_label_options(ui);
    let meta_label_options = activity_entry_meta_label_options(ui);
    let row_height =
        activity_entry_min_row_height(ui, metrics, &name_label_options, &meta_label_options, 0.0);
    let mut star_clicked = false;
    let row_response = render_clickable_entry_row(ui, (id_source, "row"), row_height, |ui| {
        let star_response =
            render_favorite_star_button(ui, (id_source, "world_star"), world.favorite);
        if crate::ui::style::hover_tip(ui, star_response, "Toggle world favorite").clicked() {
            star_clicked = true;
        }
        ui.add_space(ACTIVITY_ENTRY_CONTENT_GAP);
        render_entry_thumbnail(
            ui,
            (id_source, "thumb"),
            world.thumbnail_png.clone().map(|bytes| {
                (
                    home_world_thumbnail_uri(world.instance_id.as_str(), world.world_id.as_str()),
                    bytes,
                )
            }),
            assets::HOME_SVG,
            ACTIVITY_ENTRY_THUMBNAIL_SIZE,
            ACTIVITY_ENTRY_THUMBNAIL_SIZE,
        );
        ui.add_space(ACTIVITY_ENTRY_CONTENT_GAP);
        let text_max_width = ui.available_width().max(80.0);
        render_activity_text_stack(
            ui,
            text_ui,
            id_source,
            world.world_name.as_str(),
            world_meta_line(world, now_ms).as_str(),
            text_max_width,
            &name_label_options,
            &meta_label_options,
        );
    });
    if star_clicked {
        // A favorite applies to every instance the world appears in.
        for member in &world.instances {
            let _ = set_world_favorite(
                instances,
                member.instance_id.as_str(),
                member.target.as_str(),
                !world.favorite,
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
                instance_id: world.instance_id.clone(),
                quick_play_singleplayer: Some(world.world_id.clone()),
                quick_play_multiplayer: None,
            },
        );
        output.selected_instance_id = Some(world.instance_id.clone());
        output.requested_screen = Some(AppScreen::Library);
    }
    let context_id = ui.make_persistent_id((id_source, "world_context"));
    if let Some(anchor) = context_menu::trigger_anchor(ui.ctx(), &row_response) {
        context_menu::request(
            ui.ctx(),
            ContextMenuRequest::new(
                context_id,
                anchor,
                vec![
                    ContextMenuItem::new_with_icon(
                        "copy_world_launch_command",
                        "Copy command line",
                        assets::TERMINAL_SVG,
                    ),
                    ContextMenuItem::new_with_icon(
                        "copy_world_steam_launch_options",
                        "Copy Steam launch options",
                        assets::STEAM_SVG,
                    ),
                    ContextMenuItem::new_with_icon(
                        "world_sync_options",
                        "Sync world…",
                        assets::USERS_GROUP_SVG,
                    ),
                ],
            ),
        );
    }
    match context_menu::take_invocation(ui.ctx(), context_id).as_deref() {
        Some("copy_world_launch_command") => {
            copy_world_launch_command(ui.ctx(), world, auth);
        }
        Some("copy_world_steam_launch_options") => {
            copy_world_steam_launch_options(ui.ctx(), world, auth);
        }
        Some("world_sync_options") => {
            home_world_sync::request_world_sync_modal(ui.ctx(), world, instances);
        }
        _ => {}
    }
}

pub(super) fn world_meta_line(world: &WorldEntry, now_ms: u64) -> String {
    let mut parts = vec![
        format!("instance {}", world.instance_name),
        format!("folder {}", world.world_id),
        format!(
            "last used {}",
            format_time_ago(world.last_used_at_ms, now_ms)
        ),
    ];
    if let Some(game_mode) = world.game_mode.as_deref() {
        parts.push(game_mode.to_owned());
    }
    if let Some(difficulty) = world.difficulty.as_deref() {
        parts.push(format!("difficulty {difficulty}"));
    }
    if let Some(hardcore) = world.hardcore {
        parts.push(if hardcore {
            "hardcore".to_owned()
        } else {
            "non-hardcore".to_owned()
        });
    }
    if let Some(cheats_enabled) = world.cheats_enabled {
        parts.push(if cheats_enabled {
            "cheats on".to_owned()
        } else {
            "cheats off".to_owned()
        });
    }
    if let Some(version_name) = world.version_name.as_deref() {
        parts.push(format!("version {version_name}"));
    }
    parts.join(" | ")
}
