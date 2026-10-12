use super::*;
use textui_egui::interaction;

pub(in super::super) fn render_install_feedback(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    local_progress: Option<&InstallProgress>,
    external_activity: Option<&install_activity::InstallActivitySnapshot>,
    runtime_prepare_in_flight: bool,
) {
    if let Some(progress) = local_progress
        && matches!(progress.stage, InstallStage::Complete)
        && !runtime_prepare_in_flight
        && external_activity.is_none_or(|activity| matches!(activity.stage, InstallStage::Complete))
    {
        return;
    }

    if let Some(progress) = local_progress {
        ui.add_space(8.0);
        let fraction = progress_fraction(progress);
        let progress_label = if let Some(eta) = progress.eta_seconds {
            format!(
                "{} · {:.1} MiB/s · ETA {}s",
                stage_label(progress.stage),
                progress.bytes_per_second / (1024.0 * 1024.0),
                eta
            )
        } else {
            format!(
                "{} · {:.1} MiB/s",
                stage_label(progress.stage),
                progress.bytes_per_second / (1024.0 * 1024.0)
            )
        };
        ui.add(egui::ProgressBar::new(fraction));
        let _ = text_ui.label(
            ui,
            ("instance_runtime_progress_label", instance_id),
            &format!("{progress_label} · {:.0}%", fraction * 100.0),
            &style::muted(ui),
        );
        let _ = text_ui.label(
            ui,
            ("instance_runtime_progress_message", instance_id),
            progress.message.as_str(),
            &style::muted(ui),
        );
        let bytes_line = if let Some(total) = progress.total_bytes {
            format!(
                "{} / {}",
                format_bytes(progress.downloaded_bytes),
                format_bytes(total)
            )
        } else {
            format!("{} downloaded", format_bytes(progress.downloaded_bytes))
        };
        let _ = text_ui.label(
            ui,
            ("instance_runtime_bytes", instance_id),
            &format!(
                "Files: {}/{} · {}",
                progress.downloaded_files, progress.total_files, bytes_line
            ),
            &style::muted(ui),
        );
        return;
    }

    if runtime_prepare_in_flight {
        ui.add_space(8.0);
        ui.add(egui::ProgressBar::new(0.0).animate(true));
        let _ = text_ui.label(
            ui,
            ("instance_runtime_progress_starting", instance_id),
            "Starting installation...",
            &style::muted(ui),
        );
        return;
    }

    if let Some(activity) = external_activity {
        if matches!(activity.stage, InstallStage::Complete) {
            return;
        }
        ui.add_space(8.0);
        let fraction = progress_fraction_from_activity(activity);
        let progress_label = if let Some(eta) = activity.eta_seconds {
            format!(
                "{} · {:.1} MiB/s · ETA {}s",
                stage_label(activity.stage),
                activity.bytes_per_second / (1024.0 * 1024.0),
                eta
            )
        } else {
            format!(
                "{} · {:.1} MiB/s",
                stage_label(activity.stage),
                activity.bytes_per_second / (1024.0 * 1024.0)
            )
        };
        ui.add(egui::ProgressBar::new(fraction));
        let _ = text_ui.label(
            ui,
            ("instance_runtime_progress_label_external", instance_id),
            &format!("{progress_label} · {:.0}%", fraction * 100.0),
            &style::muted(ui),
        );
        let _ = text_ui.label(
            ui,
            ("instance_runtime_progress_message_external", instance_id),
            activity.message.as_str(),
            &style::muted(ui),
        );
        let bytes_line = if let Some(total) = activity.total_bytes {
            format!(
                "{} / {}",
                format_bytes(activity.downloaded_bytes),
                format_bytes(total)
            )
        } else {
            format!("{} downloaded", format_bytes(activity.downloaded_bytes))
        };
        let _ = text_ui.label(
            ui,
            ("instance_runtime_bytes_external", instance_id),
            &format!(
                "Files: {}/{} · {}",
                activity.downloaded_files, activity.total_files, bytes_line
            ),
            &style::muted(ui),
        );
    }
}

pub(in super::super) fn render_runtime_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    id: &str,
    instance_root: &Path,
    game_version: &str,
    config: &Config,
    instances: &InstanceStore,
    external_install_active: bool,
    auth: &PlayerAuthContext<'_>,
    streamer_mode: bool,
) {
    let button_style = ButtonOptions {
        min_size: egui::vec2(120.0, 34.0),
        text_color: ui.visuals().widgets.active.fg_stroke.color,
        fill: ui.visuals().selection.bg_fill,
        fill_hovered: style::hover_fill(ui, ui.visuals().selection.bg_fill),
        fill_active: ui.visuals().selection.bg_fill.gamma_multiply(0.9),
        fill_selected: ui.visuals().selection.bg_fill,
        stroke: ui.visuals().selection.stroke,
        ..ButtonOptions::default()
    };
    let mut muted_style = crate::ui::style::body(ui);
    muted_style.color = ui.visuals().weak_text_color();
    muted_style.wrap = false;
    let instance_root_key = normalize_path_key(instance_root);
    let launch_account = auth
        .launch_auth
        .as_ref()
        .map(|a| a.account_key.clone())
        .or_else(|| auth.display_name().map(str::to_owned));
    let launch_display_name = auth
        .launch_auth
        .as_ref()
        .map(|a| a.player_name.clone())
        .or_else(|| auth.display_name().map(str::to_owned));
    let launch_player_uuid = auth.launch_auth.as_ref().map(|a| a.player_uuid.clone());
    let launch_access_token = auth
        .launch_auth
        .as_ref()
        .and_then(|a| a.access_token.clone());
    let launch_xuid = auth.launch_auth.as_ref().and_then(|a| a.xuid.clone());
    let launch_user_type = auth.launch_auth.as_ref().map(|a| a.user_type.clone());
    let runtime_running_for_active_account = launch_account
        .as_deref()
        .is_some_and(|account| is_instance_running_for_account(instance_root, account));
    let account_running_root = launch_account
        .as_deref()
        .and_then(running_instance_for_account);
    let launch_disabled_for_account = !runtime_running_for_active_account
        && account_running_root
            .as_deref()
            .is_some_and(|running_root| running_root != instance_root_key.as_str());
    let launch_disabled_for_missing_ownership =
        !runtime_running_for_active_account && !auth.owns_minecraft();
    let launch_disabled_for_token_refresh =
        !runtime_running_for_active_account && auth.token_refresh_in_progress;
    let launch_disabled = launch_disabled_for_account
        || launch_disabled_for_missing_ownership
        || launch_disabled_for_token_refresh;

    let running_account_key = if runtime_running_for_active_account {
        launch_player_uuid
            .clone()
            .or_else(|| launch_account.clone())
            .or_else(|| state.launch_user_key.clone())
            .map(|value| value.to_ascii_lowercase())
    } else {
        None
    };
    let running_avatar_png = running_account_key
        .as_deref()
        .and_then(|key| auth.account_avatars.get(key))
        .map(Vec::as_slice);
    let runtime_running = runtime_running_for_active_account;
    state.running = runtime_running;

    ui.horizontal(|ui| {
        if !state.runtime_prepare_in_flight && !external_install_active {
            let response = ui
                .add_enabled_ui(!launch_disabled, |ui| {
                    if runtime_running {
                        render_stop_runtime_button(ui, id, &button_style, running_avatar_png)
                    } else {
                        let icon_color = button_style.text_color;
                        let mut paint_play_icon = |ui: &mut Ui, icon_rect: egui::Rect| {
                            let _ = ui.put(
                                icon_rect,
                                egui::Image::from_bytes(
                                    format!(
                                        "bytes://instance/runtime-play/{id}-{:02x}{:02x}{:02x}.svg",
                                        icon_color.r(),
                                        icon_color.g(),
                                        icon_color.b()
                                    ),
                                    crate::ui::svg_tint::tint_svg(assets::PLAY_SVG, icon_color),
                                )
                                .fit_to_exact_size(icon_rect.size()),
                            );
                        };
                        textui_egui::button_with_trailing_icon(
                            text_ui,
                            ui,
                            ("instance_runtime_toggle", id),
                            "Launch",
                            &button_style,
                            textui_egui::TrailingIcon {
                                size: 16.0,
                                gap: style::SPACE_SM,
                                paint: &mut paint_play_icon,
                            },
                        )
                    }
                })
                .inner;
            let toggle_requested = response.clicked();
            if toggle_requested {
                if runtime_running {
                    let stopped = launch_account.as_deref().is_some_and(|account| {
                        stop_running_instance_for_account(instance_root, account)
                    });
                    if stopped {
                        state.running = false;
                        state.status_message = Some("Stopped instance runtime.".to_owned());
                    } else {
                        state.running = false;
                        state.status_message = Some("Instance runtime was not running.".to_owned());
                    }
                } else if game_version.trim().is_empty() {
                    state.status_message =
                        Some("Cannot launch: choose a Minecraft game version first.".to_owned());
                } else {
                    // Launch from the saved record (like the library and home screens do);
                    // edits in the settings modal are autosaved shortly after they are made.
                    let saved = instances.find(id);
                    let InstanceLaunchSettings {
                        max_memory_mib,
                        extra_jvm_args,
                        extra_env_vars,
                        linux_set_opengl_driver,
                        linux_use_zink_driver,
                    } = InstanceLaunchSettings::resolve(
                        config,
                        saved.unwrap_or(&instances::InstanceRecord::default()),
                    );
                    let java_override_enabled =
                        saved.is_some_and(|instance| instance.java_override_enabled);
                    let java_override_runtime_major =
                        saved.and_then(|instance| instance.java_override_runtime_major);
                    // The real name: it is hidden when drawn (see `account_text`), not here.
                    state.launch_username = launch_display_name
                        .clone()
                        .or_else(|| launch_account.clone());
                    state.launch_user_key = launch_player_uuid
                        .clone()
                        .or_else(|| launch_account.clone())
                        .or_else(|| launch_display_name.clone())
                        .and_then(|value| {
                            let trimmed = value.trim();
                            if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed.to_owned())
                            }
                        });
                    request_runtime_prepare(
                        state,
                        RuntimePrepareOperation::Launch,
                        instance_root.to_path_buf(),
                        game_version.trim().to_owned(),
                        selected_modloader_value(state),
                        normalize_optional(state.modloader_version_input.as_str()),
                        config.effective_required_java_major(game_version),
                        config.choose_java_executable(
                            java_override_enabled,
                            java_override_runtime_major,
                            config.effective_required_java_major(game_version),
                        ),
                        config.download_max_concurrent(),
                        config.parsed_download_speed_limit_bps(),
                        linux_set_opengl_driver,
                        linux_use_zink_driver,
                        max_memory_mib,
                        extra_jvm_args,
                        extra_env_vars,
                        state.launch_username.clone(),
                        launch_display_name.clone(),
                        launch_player_uuid.clone(),
                        launch_access_token.clone(),
                        launch_xuid.clone(),
                        launch_user_type.clone(),
                        launch_account.clone(),
                        Some(crate::sync_runner::SyncRunConfig::from_config(config)),
                    );
                }
            }
            ui.add_space(10.0);
        }

        let _ = text_ui.label(
            ui,
            ("instance_runtime_state", id),
            if state.runtime_prepare_in_flight || external_install_active {
                "Installing"
            } else if state.running {
                "Running"
            } else {
                "Instance is not Running"
            },
            &muted_style,
        );
        if state.runtime_prepare_in_flight || external_install_active {
            ui.add_space(8.0);
            ui.spinner();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let settings_button_id = format!("instance_settings_open_{id}");
            let settings_button = icon_button::svg(
                ui,
                settings_button_id.as_str(),
                assets::SETTINGS_SVG,
                "Open instance settings",
                state.show_settings_modal,
                30.0,
            );
            if settings_button.clicked() {
                state.show_settings_modal = true;
            }
            let _ = text_ui.label(
                ui,
                ("instance_settings_hint", id),
                "Open instance settings",
                &muted_style,
            );
        });
    });

    if launch_disabled_for_account {
        let blocked_account = launch_display_name
            .as_deref()
            .map(|name| {
                crate::account_text::mention(launch_player_uuid.as_deref().unwrap_or(""), name)
            })
            .unwrap_or_else(|| "this account".to_owned());
        let _ = crate::ui::account_label::account_label(
            ui,
            text_ui,
            ("instance_runtime_account_locked", id),
            &format!("{blocked_account} is already running another instance."),
            &muted_style,
            streamer_mode,
            false,
        );
    }
    if launch_disabled_for_missing_ownership {
        let _ = text_ui.label(
            ui,
            ("instance_runtime_account_ownership", id),
            "Sign in with a Minecraft account that owns Minecraft to launch.",
            &muted_style,
        );
    }
    if launch_disabled_for_token_refresh {
        let _ = text_ui.label(
            ui,
            ("instance_runtime_token_refresh", id),
            "Waiting for account token refresh...",
            &muted_style,
        );
    }

    if state.running
        && !launch_account
            .as_deref()
            .is_some_and(|account| is_instance_running_for_account(instance_root, account))
    {
        state.running = false;
        state.status_message = Some("Minecraft process exited.".to_owned());
    }
}

pub(in super::super) fn render_modloader_selector(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    id: &str,
    game_version: &str,
) {
    let style = ButtonOptions {
        min_size: egui::vec2(88.0, 30.0),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().selection.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for (index, option) in MODLOADER_OPTIONS.iter().enumerate() {
            let unavailable_reason = if index == CUSTOM_MODLOADER_INDEX {
                None
            } else {
                state
                    .loader_support
                    .unavailable_reason(option, game_version)
            };
            let available = unavailable_reason.is_none();

            let mut button_style = style.clone();
            if !available {
                button_style.text_color = ui.visuals().weak_text_color();
                button_style.fill = ui.visuals().widgets.noninteractive.bg_fill;
                button_style.fill_hovered = ui.visuals().widgets.noninteractive.bg_fill;
                button_style.fill_active = ui.visuals().widgets.noninteractive.bg_fill;
                button_style.fill_selected = ui.visuals().widgets.noninteractive.bg_fill;
            }

            let response = ui
                .add_enabled_ui(available, |ui| {
                    text_ui.selectable_button(
                        ui,
                        ("instance_modloader_option", id, index),
                        option,
                        state.selected_modloader == index,
                        &button_style,
                    )
                })
                .inner;
            if let Some(reason) = unavailable_reason.as_deref() {
                let tooltip_options = TooltipOptions::default();
                text_ui.tooltip_for_response(
                    ui,
                    ("instance_modloader_unavailable_tooltip", id, index),
                    &response,
                    reason,
                    &tooltip_options,
                );
            }

            if available && response.clicked() && state.selected_modloader != index {
                state.selected_modloader = index;
                state.modloader_version_input.clear();
                state.incompatible_modloader_version_warning_key = None;
            }
        }
    });
}

pub(in super::super) fn render_stop_runtime_button(
    ui: &mut Ui,
    id: &str,
    style: &ButtonOptions,
    avatar_png: Option<&[u8]>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(style.min_size, egui::Sense::click());
    let error_color = ui.visuals().error_fg_color;
    let fill_base = egui::Color32::from_rgba_premultiplied(
        error_color.r(),
        error_color.g(),
        error_color.b(),
        36,
    );
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    let fill = interaction::FillPalette::new(
        fill_base,
        fill_base.gamma_multiply(1.25),
        fill_base.gamma_multiply(0.85),
        fill_base,
        egui::Color32::WHITE,
    )
    .fill(state, false);
    let idle_stroke = egui::Stroke::new(style.stroke.width.max(1.0), error_color);
    let stroke = interaction::focus_stroke(
        state,
        ui.visuals(),
        interaction::hover_stroke(
            state,
            idle_stroke,
            interaction::emphasized_stroke(idle_stroke, egui::Color32::WHITE),
        ),
    );

    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(style.corner_radius), fill);
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(style.corner_radius),
        stroke,
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), ui.visuals(), state, rect, style.corner_radius);

    let inner_rect = rect.shrink2(style.padding);
    let avatar_size = (inner_rect.height() - 2.0).clamp(12.0, 20.0);
    let avatar_rect =
        egui::Rect::from_min_size(inner_rect.min, egui::vec2(avatar_size, avatar_size));
    render_runtime_avatar(ui, id, avatar_rect, avatar_png, error_color);

    let icon_lane = egui::Rect::from_min_max(
        egui::pos2(
            (avatar_rect.max.x + 8.0).min(inner_rect.max.x),
            inner_rect.min.y,
        ),
        inner_rect.max,
    );
    let icon_size = (icon_lane.height() - 4.0).clamp(12.0, 18.0);
    let stop_icon_rect =
        egui::Rect::from_center_size(icon_lane.center(), egui::vec2(icon_size, icon_size));
    let stop_icon_color = egui::Color32::WHITE;
    let stop_icon = egui::Image::from_bytes(
        format!(
            "bytes://instance/runtime-stop/{id}-{:02x}{:02x}{:02x}.svg",
            stop_icon_color.r(),
            stop_icon_color.g(),
            stop_icon_color.b()
        ),
        crate::ui::svg_tint::tint_svg(assets::STOP_SVG, stop_icon_color),
    )
    .fit_to_exact_size(egui::vec2(icon_size, icon_size));
    let _ = ui.put(stop_icon_rect, stop_icon);

    response
}

pub(in super::super) fn render_runtime_avatar(
    ui: &mut Ui,
    id: &str,
    rect: egui::Rect,
    avatar_png: Option<&[u8]>,
    color: egui::Color32,
) {
    if let Some(bytes) = avatar_png {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        bytes.hash(&mut hasher);
        let key = format!("bytes://instance/runtime-avatar/{}", hasher.finish());
        if let crate::ui::components::image_textures::ManagedTextureStatus::Ready(texture) =
            crate::ui::components::image_textures::request_texture(
                ui.ctx(),
                key,
                Arc::<[u8]>::from(bytes.to_vec().into_boxed_slice()),
                egui::TextureOptions::LINEAR,
            )
        {
            let image = texture.image().fit_to_exact_size(rect.size());
            let _ = ui.put(rect, image);
        }
        return;
    }

    let fallback = egui::Image::from_bytes(
        format!("bytes://instance/runtime-avatar-fallback/{id}.svg"),
        crate::ui::svg_tint::tint_svg(assets::USER_SVG, color),
    )
    .fit_to_exact_size(rect.size());
    let _ = ui.put(rect, fallback);
}
