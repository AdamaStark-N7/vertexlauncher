use super::*;

pub(super) fn render_instances_and_worlds_tab(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
    auth: &PlayerAuthContext<'_>,
    streamer_mode: bool,
    state: &mut HomeState,
    output: &mut HomeOutput,
    metrics: HomeUiMetrics,
) {
    poll_home_activity_results(state);
    poll_server_ping_results(state);
    poll_instance_thumbnail_results(ui.ctx(), &mut state.thumbnails);
    let should_scan = state
        .last_scan_at
        .is_none_or(|last| last.elapsed() >= HOME_SCAN_INTERVAL)
        || state.scanned_instance_count != instances.instances.len();
    if should_scan {
        refresh_home_state(state, instances, config, false);
    }
    queue_server_pings(state);
    if state.activity_scan_pending || !state.server_ping_in_flight.is_empty() {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    if !state.thumbnails.cache.is_empty() {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }

    let mut requested_rescan = false;
    render_instance_usage(ui, text_ui, instances, config, auth, state, output, metrics);
    ui.add_space(12.0);
    render_activity_feed(
        ui,
        text_ui,
        instances,
        state,
        auth,
        streamer_mode,
        output,
        &mut requested_rescan,
        metrics,
    );

    let world_sync_finished = ui
        .ctx()
        .data_mut(|data| data.remove_temp::<bool>(egui::Id::new(home_world_sync::RESCAN_KEY)))
        .unwrap_or(false);
    if requested_rescan || world_sync_finished {
        refresh_home_state(state, instances, config, true);
    }

    let mut retained_image_keys = HashSet::new();
    retain_home_viewer_image(state, &mut retained_image_keys);
    state
        .screenshot_images
        .retain_loaded(ui.ctx(), &retained_image_keys);
}

pub(super) fn render_screenshots_tab(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
    state: &mut HomeState,
    metrics: HomeUiMetrics,
) {
    poll_delete_screenshot_results(state, instances, config);
    poll_screenshot_results(state);
    let should_scan = state
        .last_screenshot_scan_at
        .is_none_or(|last| last.elapsed() >= SCREENSHOT_SCAN_INTERVAL)
        || state.scanned_screenshot_instance_count != instances.instances.len();
    if should_scan {
        refresh_screenshot_state(state, instances, config, false);
    }
    if state.screenshot_scan_pending || state.delete_screenshot_in_flight {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    if state.screenshot_viewer.as_ref().is_some_and(|viewer| {
        !state
            .screenshots
            .iter()
            .any(|screenshot| screenshot.key() == viewer.screenshot_key)
    }) && !state.screenshot_scan_pending
    {
        tracing::info!(
            target: "vertexlauncher/screenshots",
            "Home screenshot viewer closed because the selected screenshot disappeared from the gallery state."
        );
        state.screenshot_viewer = None;
    }
    if state
        .pending_delete_screenshot_key
        .as_ref()
        .is_some_and(|pending| {
            !state
                .screenshots
                .iter()
                .any(|screenshot| screenshot.key() == *pending)
        })
    {
        state.pending_delete_screenshot_key = None;
    }
    let mut retained_image_keys = HashSet::new();
    render_screenshot_gallery(ui, text_ui, state, &mut retained_image_keys, metrics);
    retain_home_viewer_image(state, &mut retained_image_keys);
    state
        .screenshot_images
        .retain_loaded(ui.ctx(), &retained_image_keys);
}
