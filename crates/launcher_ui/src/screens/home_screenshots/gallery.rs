use super::*;

pub(in crate::screens::home) fn render_screenshot_gallery(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    state: &mut HomeState,
    retained_image_keys: &mut HashSet<String>,
    metrics: HomeUiMetrics,
) {
    let title_style = style::body_strong(ui);
    let body_style = style::muted(ui);
    let _ = text_ui.label(ui, "home_screenshots_title", "Screenshots", &title_style);
    let total_candidates = state.screenshot_candidates.len();
    let loaded_count = state.screenshots.len();
    let summary = if state.screenshot_scan_pending && state.screenshots.is_empty() {
        "Loading screenshots...".to_owned()
    } else if state.screenshots.is_empty() {
        "No screenshots found in any instance.".to_owned()
    } else if state.screenshot_loaded_count < total_candidates {
        format!(
            "Showing {loaded_count} of {total_candidates} screenshots — scroll down to load more."
        )
    } else {
        format!("{loaded_count} screenshots across your instances.")
    };
    let _ = text_ui.label(
        ui,
        "home_screenshots_summary",
        summary.as_str(),
        &body_style,
    );
    ui.add_space(style::SPACE_SM);

    if state.screenshots.is_empty() {
        return;
    }

    let layout = build_virtual_masonry_layout(
        ui,
        metrics.screenshot_min_column_width,
        SCREENSHOT_TILE_GAP,
        3,
        state.screenshots.len(),
        state.screenshot_layout_revision,
        &mut state.screenshot_masonry_layout_cache,
        |index, column_width| screenshot_tile_height(&state.screenshots[index], column_width),
    );

    let mut open_screenshot_key = None;
    let mut delete_screenshot_key = None;
    let mut should_load_more = false;
    let viewer_open = state.screenshot_viewer.is_some();
    egui::ScrollArea::vertical()
        .scroll_source(modal_host::drag_scroll_source())
        .id_salt("home_screenshots_scroll")
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            ui.add_space(style::SPACE_SM);
            let screenshots = &state.screenshots;
            let screenshot_images = &mut state.screenshot_images;
            render_virtualized_masonry(
                ui,
                &layout,
                SCREENSHOT_TILE_GAP,
                viewport,
                HOME_SCREENSHOT_OVERSCAN,
                |column_ui, index, tile_height| {
                    let action = render_screenshot_tile(
                        column_ui,
                        text_ui,
                        screenshot_images,
                        &screenshots[index],
                        tile_height,
                        retained_image_keys,
                        metrics,
                        viewer_open,
                    );
                    if action.open_viewer {
                        open_screenshot_key = Some(screenshots[index].key());
                    }
                    if action.request_delete {
                        delete_screenshot_key = Some(screenshots[index].key());
                    }
                },
            );
            should_load_more = state.screenshot_loaded_count < state.screenshot_candidates.len()
                && viewport.bottom() >= ui.min_rect().bottom() - viewport.height().max(320.0);
        });

    if should_load_more {
        let request_id = state.latest_requested_screenshot_scan_id;
        spawn_screenshot_load_page(state, request_id, SCREENSHOT_PAGE_SIZE);
    }

    if let Some(screenshot_key) = open_screenshot_key {
        if let Some(entry) = state
            .screenshots
            .iter()
            .find(|e| e.key() == screenshot_key)
            .cloned()
        {
            state.screenshot_viewer = Some(ScreenshotViewerState {
                screenshot_key,
                entry_snapshot: entry,
                zoom: SCREENSHOT_VIEWER_MIN_ZOOM,
                pan_uv: egui::Vec2::ZERO,
            });
        }
    }
    if let Some(screenshot_key) = delete_screenshot_key {
        state.pending_delete_screenshot_key = Some(screenshot_key);
    }
}

pub(super) fn screenshot_tile_height(screenshot: &ScreenshotEntry, column_width: f32) -> f32 {
    column_width / screenshot.aspect_ratio().max(0.01)
}

pub(super) fn render_screenshot_tile(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    screenshot_images: &mut LazyImageBytes,
    screenshot: &ScreenshotEntry,
    tile_height: f32,
    retained_image_keys: &mut HashSet<String>,
    metrics: HomeUiMetrics,
    viewer_open: bool,
) -> ScreenshotTileAction {
    let width = ui.available_width().max(1.0);
    let tile_size = egui::vec2(width, tile_height);
    let (rect, _) = ui.allocate_exact_size(tile_size, egui::Sense::hover());
    let mut image_response = ui.interact(
        rect,
        ui.id().with(("home_screenshot_tile", screenshot.key())),
        egui::Sense::click(),
    );
    let image_has_focus = image_response.has_focus();
    let image_key = screenshot.uri();
    retained_image_keys.insert(image_key.clone());
    let image_status = screenshot_images.request(image_key.clone(), screenshot.path.clone());
    let image_bytes = screenshot_images.bytes(image_key.as_str());
    paint_tile_image(
        ui,
        text_ui,
        rect,
        image_key.as_str(),
        image_bytes.as_ref(),
        image_status,
        "home_screenshot_placeholder",
    );

    let tile_contains_pointer = !viewer_open && ui_pointer_over_rect(ui, rect);
    let overlay_memory_id = image_response.id.with("home_screenshot_overlay_active");
    let overlay_was_active = !viewer_open
        && ui
            .ctx()
            .data_mut(|data| data.get_temp::<bool>(overlay_memory_id))
            .unwrap_or(false);
    let mut overlay_clicked = false;
    let mut action = ScreenshotTileAction::default();
    let mut overlay_result = ScreenshotOverlayResult::default();
    if tile_contains_pointer || overlay_was_active || image_has_focus {
        overlay_result = render_screenshot_overlay_action(
            ui,
            rect,
            "home_gallery",
            screenshot,
            image_bytes.as_deref(),
            image_status == LazyImageBytesStatus::Loading,
            metrics,
        );
        match overlay_result.action {
            Some(ScreenshotOverlayAction::Copy) => {
                overlay_clicked = true;
            }
            Some(ScreenshotOverlayAction::Delete) => {
                overlay_clicked = true;
                action.request_delete = true;
            }
            None => {}
        }
    }
    // Keep the overlay up while the tile or one of its buttons has keyboard/controller focus.
    let overlay_active =
        tile_contains_pointer || overlay_result.contains_pointer || image_has_focus;
    ui.ctx()
        .data_mut(|data| data.insert_temp(overlay_memory_id, overlay_active));
    let mut state = interaction::InteractionState {
        hover: ui
            .ctx()
            .animate_bool(overlay_memory_id.with("hover_fade"), overlay_active),
        pressed: false,
        focused: false,
        enabled: !viewer_open,
    };
    state.focused = image_has_focus;
    if overlay_active && !viewer_open {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    paint_tile_border(ui, rect, state);

    let age_label = format_time_ago(screenshot.modified_at_ms, current_time_millis());
    let label = format!("{} | {}", screenshot.instance_name, age_label);
    paint_tile_caption(
        ui,
        text_ui,
        rect,
        ("home_screenshot_tile_label", screenshot.key()),
        label.as_str(),
    );

    if !overlay_result.contains_pointer {
        image_response = crate::ui::style::hover_tip(
            ui,
            image_response,
            format!(
                "{}\n{}\n{}",
                screenshot.instance_name,
                screenshot.file_name,
                screenshot.path.display()
            ),
        );
    }
    action.open_viewer = image_response.clicked() && !overlay_clicked;
    action
}

pub(super) fn render_screenshot_overlay_action(
    ui: &mut Ui,
    tile_rect: egui::Rect,
    scope: &str,
    screenshot: &ScreenshotEntry,
    copy_bytes: Option<&[u8]>,
    copy_loading: bool,
    metrics: HomeUiMetrics,
) -> ScreenshotOverlayResult {
    let screenshot_key = screenshot.key();
    let mut result = ScreenshotOverlayResult::default();
    let copy_result = render_screenshot_overlay_button(
        ui,
        tile_rect,
        scope,
        screenshot_key.as_str(),
        "home_screenshot_copy_button",
        assets::COPY_SVG,
        ui.visuals().text_color(),
        if copy_loading {
            "Image is still loading"
        } else {
            "Copy image to clipboard"
        },
        8.0,
        copy_bytes.is_some(),
        metrics,
    );
    result.contains_pointer |= copy_result.contains_pointer;
    if copy_result.clicked {
        let Some(bytes) = copy_bytes else {
            return result;
        };
        copy_screenshot_to_clipboard(ui.ctx(), screenshot.file_name.as_str(), bytes);
        result.action = Some(ScreenshotOverlayAction::Copy);
        return result;
    }
    let delete_result = render_screenshot_overlay_button(
        ui,
        tile_rect,
        scope,
        screenshot_key.as_str(),
        "home_screenshot_delete_button",
        assets::TRASH_X_SVG,
        ui.visuals().error_fg_color,
        "Delete screenshot",
        8.0 + metrics.screenshot_overlay_button_size + 6.0,
        true,
        metrics,
    );
    result.contains_pointer |= delete_result.contains_pointer;
    if delete_result.clicked {
        result.action = Some(ScreenshotOverlayAction::Delete);
    }
    result
}

pub(super) fn render_screenshot_overlay_button(
    ui: &mut Ui,
    tile_rect: egui::Rect,
    scope: &str,
    screenshot_key: &str,
    id_source: &str,
    icon_svg: &[u8],
    icon_color: Color32,
    tooltip: &str,
    x_offset: f32,
    enabled: bool,
    metrics: HomeUiMetrics,
) -> ScreenshotOverlayButtonResult {
    let button_rect = egui::Rect::from_min_size(
        tile_rect.min + egui::vec2(x_offset, 8.0),
        egui::vec2(
            metrics.screenshot_overlay_button_size,
            metrics.screenshot_overlay_button_size,
        ),
    );
    let themed_svg = crate::ui::svg_tint::tint_svg(icon_svg, icon_color);
    let icon_color_key = format!(
        "{:02x}{:02x}{:02x}",
        icon_color.r(),
        icon_color.g(),
        icon_color.b()
    );
    let uri = format!("bytes://home/{id_source}/{scope}-{screenshot_key}-{icon_color_key}.svg");
    let response = ui.interact(
        button_rect,
        ui.id().with((id_source, scope, screenshot_key)),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let button_contains_pointer = ui_pointer_over_rect(ui, button_rect);
    let button_state =
        interaction::InteractionState::of_covered(ui.ctx(), &response, button_rect, enabled);
    let overlay_idle = egui::Color32::from_rgba_premultiplied(12, 16, 24, 210);
    let fill = interaction::FillPalette::new(
        overlay_idle,
        ui.visuals().widgets.hovered.bg_fill,
        ui.visuals().widgets.active.bg_fill,
        ui.visuals().selection.bg_fill,
        ui.visuals().text_color(),
    )
    .fill(button_state, false);
    ui.painter()
        .rect_filled(button_rect, egui::CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        button_rect,
        egui::CornerRadius::same(8),
        interaction::focus_stroke(
            button_state,
            ui.visuals(),
            interaction::hover_stroke(
                button_state,
                ui.visuals().widgets.inactive.bg_stroke,
                ui.visuals().widgets.hovered.bg_stroke,
            ),
        ),
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), ui.visuals(), button_state, button_rect, 8);
    let icon_size = (metrics.screenshot_overlay_button_size * 0.5).clamp(12.0, 16.0);
    let icon_rect =
        egui::Rect::from_center_size(button_rect.center(), egui::vec2(icon_size, icon_size));
    egui::Image::from_bytes(uri, themed_svg)
        .fit_to_exact_size(icon_rect.size())
        .tint(if enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(120)
        })
        .paint_at(ui, icon_rect);
    let clicked = response.clicked();
    if button_contains_pointer {
        let _ = crate::ui::style::hover_tip(ui, response.clone(), tooltip);
    }
    ScreenshotOverlayButtonResult {
        clicked,
        contains_pointer: button_contains_pointer
            || response.is_pointer_button_down_on()
            || response.has_focus(),
    }
}

pub(super) fn copy_screenshot_to_clipboard(ctx: &egui::Context, label: &str, bytes: &[u8]) {
    match decode_color_image(bytes) {
        Ok(image) => {
            ctx.copy_image(image);
            notification::info!("home/screenshots", "Copied '{}' to clipboard.", label);
        }
        Err(err) => {
            notification::error!(
                "home/screenshots",
                "Failed to copy '{}' to clipboard: {}",
                label,
                err
            );
        }
    }
}
