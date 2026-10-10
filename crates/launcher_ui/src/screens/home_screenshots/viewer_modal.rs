use super::*;

pub(in crate::screens::home) fn retain_home_viewer_image(
    state: &mut HomeState,
    retained_image_keys: &mut HashSet<String>,
) {
    let Some(viewer) = state.screenshot_viewer.as_ref() else {
        return;
    };
    let snapshot = viewer.entry_snapshot.clone();
    let image_key = snapshot.uri();
    retained_image_keys.insert(image_key.clone());
    state.screenshot_images.request(image_key, snapshot.path);
}

pub(in crate::screens::home) fn render_screenshot_viewer_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    state: &mut HomeState,
    metrics: HomeUiMetrics,
) {
    let Some(screenshot_key) = state
        .screenshot_viewer
        .as_ref()
        .map(|viewer_state| viewer_state.screenshot_key.clone())
    else {
        return;
    };
    let live_entry = state
        .screenshots
        .iter()
        .find(|entry| entry.key() == screenshot_key)
        .cloned();
    if live_entry.is_none() && !state.screenshot_scan_pending {
        tracing::info!(
            target: "vertexlauncher/screenshots",
            screenshot_key = screenshot_key.as_str(),
            "Home screenshot viewer closed because the screenshot entry was no longer available."
        );
        state.screenshot_viewer = None;
        return;
    }
    let Some(viewer_state) = state.screenshot_viewer.as_mut() else {
        return;
    };
    if let Some(ref entry) = live_entry {
        viewer_state.entry_snapshot = entry.clone();
    }
    let screenshot = live_entry.unwrap_or_else(|| viewer_state.entry_snapshot.clone());
    let image_key = screenshot.uri();
    let image_status = state
        .screenshot_images
        .request(image_key.clone(), screenshot.path.clone());
    let image_bytes = state.screenshot_images.bytes(image_key.as_str());
    let gamepad = ViewerGamepadInput::read(
        ctx,
        "home_screenshot_viewer_gamepad_pan",
        "home_screenshot_viewer_gamepad_zoom",
    );

    let now_ms = current_time_millis();
    let mut close_requested = false;
    let mut delete_requested = false;
    let request_reset_focus = modal_default_focus_requested(
        ctx,
        ("home_screenshot_viewer_window", screenshot_key.as_str()),
    );
    let response = show_dialog(
        ctx,
        dialog_options("home_screenshot_viewer_window", DialogPreset::Viewer),
        |ui| {
            let title_style = style::section_heading(ui);
            let body_style = style::muted_single_line(ui);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let _ = text_ui.label(
                        ui,
                        "home_screenshot_viewer_title",
                        screenshot.file_name.as_str(),
                        &title_style,
                    );
                    let details = format!(
                        "{} | {}x{} | {}",
                        screenshot.instance_name,
                        screenshot.width,
                        screenshot.height,
                        format_time_ago(screenshot.modified_at_ms, now_ms)
                    );
                    let _ = text_ui.label(
                        ui,
                        "home_screenshot_viewer_details",
                        details.as_str(),
                        &body_style,
                    );
                });
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if text_ui
                        .button(
                            ui,
                            "home_screenshot_viewer_close",
                            "Close",
                            &secondary_button(
                                ui,
                                egui::vec2(metrics.action_button_width, style::CONTROL_HEIGHT),
                            ),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                    if text_ui
                        .button(
                            ui,
                            "home_screenshot_viewer_delete",
                            "Delete",
                            &danger_button(
                                ui,
                                egui::vec2(metrics.action_button_width, style::CONTROL_HEIGHT),
                            ),
                        )
                        .clicked()
                    {
                        delete_requested = true;
                    }
                    if ui
                        .add_enabled_ui(image_bytes.is_some(), |ui| {
                            text_ui.button(
                                ui,
                                "home_screenshot_viewer_copy",
                                "Copy",
                                &secondary_button(
                                    ui,
                                    egui::vec2(
                                        (metrics.action_button_width - 10.0).max(80.0),
                                        style::CONTROL_HEIGHT,
                                    ),
                                ),
                            )
                        })
                        .inner
                        .clicked()
                        && let Some(bytes) = image_bytes.as_deref()
                    {
                        copy_screenshot_to_clipboard(
                            ui.ctx(),
                            screenshot.file_name.as_str(),
                            bytes,
                        );
                    }
                    let reset_response = text_ui.button(
                        ui,
                        "home_screenshot_viewer_reset",
                        "Reset",
                        &secondary_button(
                            ui,
                            egui::vec2(
                                (metrics.action_button_width - 10.0).max(80.0),
                                style::CONTROL_HEIGHT,
                            ),
                        ),
                    );
                    if request_reset_focus {
                        reset_response.request_focus();
                    }
                    if reset_response.clicked() {
                        viewer_state.zoom = SCREENSHOT_VIEWER_MIN_ZOOM;
                        viewer_state.pan_uv = egui::Vec2::ZERO;
                    }
                    if text_ui
                        .button(
                            ui,
                            "home_screenshot_viewer_zoom_in",
                            "+",
                            &secondary_button(
                                ui,
                                egui::vec2(
                                    metrics
                                        .screenshot_overlay_button_size
                                        .max(style::CONTROL_HEIGHT),
                                    style::CONTROL_HEIGHT,
                                ),
                            ),
                        )
                        .clicked()
                    {
                        viewer_state.zoom = VIEWER_ZOOM_LIMITS.adjust(viewer_state.zoom, 1.0);
                        VIEWER_ZOOM_LIMITS.clamp_pan(viewer_state.zoom, &mut viewer_state.pan_uv);
                    }
                    if text_ui
                        .button(
                            ui,
                            "home_screenshot_viewer_zoom_out",
                            "-",
                            &secondary_button(
                                ui,
                                egui::vec2(
                                    metrics
                                        .screenshot_overlay_button_size
                                        .max(style::CONTROL_HEIGHT),
                                    style::CONTROL_HEIGHT,
                                ),
                            ),
                        )
                        .clicked()
                    {
                        viewer_state.zoom = VIEWER_ZOOM_LIMITS.adjust(viewer_state.zoom, -1.0);
                        VIEWER_ZOOM_LIMITS.clamp_pan(viewer_state.zoom, &mut viewer_state.pan_uv);
                    }
                });
            });
            ui.add_space(12.0);

            let canvas = ViewerCanvas {
                limits: VIEWER_ZOOM_LIMITS,
                gamepad,
                aspect_ratio: screenshot.aspect_ratio(),
                image_key: image_key.as_str(),
                image_bytes: image_bytes.as_ref(),
                image_status,
                status_salt: "home_screenshot_center_status",
            };
            render_viewer_canvas(
                ui,
                text_ui,
                &canvas,
                &mut viewer_state.zoom,
                &mut viewer_state.pan_uv,
            );
        },
    );
    close_requested |= response.close_requested;

    if delete_requested {
        tracing::info!(
            target: "vertexlauncher/screenshots",
            screenshot_key = screenshot_key.as_str(),
            "Home screenshot viewer requested delete."
        );
        state.pending_delete_screenshot_key = Some(screenshot_key.clone());
    }
    if close_requested {
        tracing::info!(
            target: "vertexlauncher/screenshots",
            screenshot_key = screenshot_key.as_str(),
            "Home screenshot viewer closed by explicit close button."
        );
        state.screenshot_viewer = None;
    }
}
