use super::*;

pub(super) fn render_instance_usage(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &InstanceStore,
    config: &Config,
    auth: &PlayerAuthContext<'_>,
    state: &mut HomeState,
    output: &mut HomeOutput,
    metrics: HomeUiMetrics,
) {
    let mut title_style = crate::ui::style::body_strong(ui);
    title_style.wrap = false;
    let _ = text_ui.label(ui, "home_usage_title", "Most Used Instances", &title_style);
    ui.add_space(6.0);

    let mut items = instances.instances.iter().collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.launch_count
            .cmp(&a.launch_count)
            .then_with(|| b.last_launched_at_ms.cmp(&a.last_launched_at_ms))
            .then_with(|| a.name.cmp(&b.name))
    });
    if items.is_empty() {
        let _ = text_ui.label(
            ui,
            "home_usage_empty",
            "No instances yet.",
            &style::muted(ui),
        );
        return;
    }

    let max_height = (ui.available_height() * (1.0 / 3.0)).clamp(140.0, 340.0);
    let now_ms = current_time_millis();
    let scroll_output = egui::ScrollArea::vertical()
        .scroll_source(modal_host::drag_scroll_source())
        .id_salt("home_instances_scroll")
        .max_height(max_height)
        .show(ui, |ui| {
            for (index, instance) in items.into_iter().enumerate() {
                let thumbnail = instance
                    .thumbnail_path
                    .as_deref()
                    .filter(|path| !path.as_os_str().is_empty())
                    .and_then(|path| {
                        let key = instance_thumbnail_cache_key(instance.id.as_str(), path);
                        match state.thumbnails.cache.get_mut(&key) {
                            Some(entry) => {
                                entry.last_touched_frame = state.thumbnails.cache_frame_index;
                                entry.bytes.clone().map(|bytes| {
                                    (
                                        home_instance_thumbnail_uri(instance.id.as_str(), path),
                                        bytes,
                                    )
                                })
                            }
                            None => {
                                request_instance_thumbnail(
                                    &mut state.thumbnails,
                                    instance.id.as_str(),
                                    path.to_path_buf(),
                                );
                                None
                            }
                        }
                    });
                let row_response = render_clickable_entry_row(
                    ui,
                    ("home_instance_row", index),
                    metrics.instance_row_height,
                    |ui| {
                        render_entry_thumbnail(
                            ui,
                            ("home_instance_thumb", instance.id.as_str()),
                            thumbnail.clone(),
                            assets::LIBRARY_SVG,
                            40.0,
                            18.0,
                        );
                        ui.add_space(8.0);
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            let usage_line = format!(
                                "{} launches | {}",
                                instance.launch_count,
                                format_time_ago(instance.last_launched_at_ms, now_ms)
                            );
                            let _ = text_ui.label(
                                ui,
                                ("home_usage_count", index),
                                usage_line.as_str(),
                                &style::muted_single_line(ui),
                            );
                            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                                let _ = text_ui.label(
                                    ui,
                                    ("home_usage_name", index),
                                    instance.name.as_str(),
                                    &style::body_strong(ui),
                                );
                            });
                        });
                    },
                );
                let context_id =
                    ui.make_persistent_id(("home_instance_context", instance.id.as_str()));

                if row_response.clicked() {
                    if install_activity::is_instance_installing(instance.id.as_str()) {
                        output.selected_instance_id = Some(instance.id.clone());
                        output.requested_screen = Some(AppScreen::Library);
                    } else if !auth.token_refresh_in_progress {
                        queue_launch_intent(
                            ui.ctx(),
                            PendingLaunchIntent {
                                nonce: current_time_millis(),
                                instance_id: instance.id.clone(),
                                quick_play_singleplayer: None,
                                quick_play_multiplayer: None,
                            },
                        );
                        output.selected_instance_id = Some(instance.id.clone());
                        output.requested_screen = Some(AppScreen::Library);
                    }
                }

                if let Some(anchor) = context_menu::trigger_anchor(ui.ctx(), &row_response) {
                    instance_context_menu::request_for_instance(ui.ctx(), context_id, anchor, true);
                }

                if let Some(action) = instance_context_menu::take(ui.ctx(), context_id) {
                    match action {
                        InstanceContextAction::OpenInstance => {
                            open_home_instance(output, instance.id.as_str());
                        }
                        InstanceContextAction::OpenFolder => {
                            if let Err(err) =
                                open_home_instance_folder(instance.id.as_str(), instances, config)
                            {
                                notification::emit_replace(
                                    notification::Severity::Error,
                                    format!("home-instance-folder-{}", instance.id),
                                    format!("Failed to open instance folder: {err}"),
                                    format!("home-instance-folder/{}/error", instance.id),
                                );
                            }
                        }
                        InstanceContextAction::CopyLaunchCommand => {
                            copy_instance_launch_command(ui.ctx(), instance.id.as_str(), auth);
                        }
                        InstanceContextAction::CopySteamLaunchOptions => {
                            copy_instance_steam_launch_options(
                                ui.ctx(),
                                instance.id.as_str(),
                                auth,
                            );
                        }
                        InstanceContextAction::Delete => {
                            output.delete_requested_instance_id = Some(instance.id.clone());
                        }
                    }
                }

                ui.add_space(3.0);
            }
        });
    textui_egui::make_gamepad_scrollable(ui.ctx(), &scroll_output);
}
