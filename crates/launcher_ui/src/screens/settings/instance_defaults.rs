use super::*;

pub(super) fn render_instance_defaults_section(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    render_settings_section(
        ui,
        text_ui,
        "Instance Defaults",
        "Used when creating new instances. You can still override values per instance.",
        |ui, text_ui| render_instance_defaults_contents(ui, text_ui, config),
    );
}

pub(super) fn render_instance_defaults_contents(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let mut body_style = style::muted(ui);
    body_style.wrap = false;

    let mut installations_root = config
        .minecraft_installations_root_path()
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let installations_root_response = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        "instance_defaults_installations_root",
        "Minecraft installations folder",
        Some(
            "New instances are created under this folder as <folder>/<instance.minecraft_root>. Relative paths are resolved from the launcher working directory.",
        ),
        &mut installations_root,
    );
    if installations_root_response.changed() {
        config.set_minecraft_installations_root(installations_root);
    }
    ui.add_space(style::SPACE_MD);

    let cache_button_style = ButtonOptions {
        min_size: egui::vec2(220.0, style::CONTROL_HEIGHT_LG),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().selection.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    };
    static PURGE_CACHE_STATE: OnceLock<Mutex<PurgeCacheState>> = OnceLock::new();
    let purge_state = PURGE_CACHE_STATE.get_or_init(|| Mutex::new(PurgeCacheState { rx: None }));

    let cache_status_id = ui.make_persistent_id("settings_cache_status_message");
    let mut cache_status = ui.ctx().data_mut(|d| d.get_temp::<String>(cache_status_id));

    if let Ok(mut state) = purge_state.lock() {
        if let Some(rx) = state.rx.as_ref() {
            match rx.try_recv() {
                Ok(msg) => {
                    cache_status = Some(msg);
                    state.rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(50));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.rx = None;
                }
            }
        }
    }

    let purge_in_flight = purge_state.lock().is_ok_and(|state| state.rx.is_some());
    let purge_button_enabled = !purge_in_flight;
    let purge_button_label = if purge_in_flight {
        "Purging..."
    } else {
        "Purge metadata cache"
    };
    if ui
        .add_enabled_ui(purge_button_enabled, |ui| {
            text_ui.button(
                ui,
                "instance_defaults_purge_cache",
                purge_button_label,
                &cache_button_style,
            )
        })
        .inner
        .clicked()
    {
        let (tx, rx) = mpsc::channel::<String>();
        if let Ok(mut state) = purge_state.lock() {
            state.rx = Some(rx);
        }
        tokio_runtime::spawn_detached(async move {
            let msg = match purge_installation_cache().await {
                Ok(()) => {
                    "Purged local metadata cache. Version lists will be re-fetched on next refresh."
                        .to_owned()
                }
                Err(err) => format!("Failed to purge metadata cache: {err}"),
            };
            let _ = tx.send(msg);
        });
    }
    if let Some(message) = cache_status.as_deref() {
        let mut status_style = body_style.clone();
        status_style.wrap = true;
        let _ = text_ui.label(ui, "instance_defaults_cache_status", message, &status_style);
    }
    if let Some(message) = cache_status {
        ui.ctx()
            .data_mut(|d| d.insert_temp(cache_status_id, message));
    }
    ui.add_space(style::SPACE_MD);

    let mut max_concurrent_downloads = config.download_max_concurrent() as i32;
    let starts_response = settings_widgets::int_stepper_row(
        text_ui,
        ui,
        "download_max_concurrent",
        "Max concurrent downloads",
        Some(
            "How many files the launcher downloads at the same time (1\u{2013}128) for game files, libraries and mods. Higher can be faster on fast connections but may trigger rate limits from download servers. Default: 8.",
        ),
        &mut max_concurrent_downloads,
        DOWNLOAD_CONCURRENCY_MIN as i32,
        DOWNLOAD_CONCURRENCY_MAX as i32,
        1,
    );
    if starts_response.changed() {
        config.set_download_max_concurrent(max_concurrent_downloads.max(1) as u32);
    }
    ui.add_space(style::SPACE_MD);

    let mut speed_limit_enabled = config.download_speed_limit_enabled();
    let speed_toggle_response = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Enable download speed limiter",
        Some(
            "Limits how fast the launcher downloads, using the value below. If the value is empty or not valid, no limit is applied. Default: Off.",
        ),
        &mut speed_limit_enabled,
    );
    if speed_toggle_response.changed() {
        config.set_download_speed_limit_enabled(speed_limit_enabled);
    }
    ui.add_space(style::SPACE_MD);

    let mut speed_limit = config.download_speed_limit().to_owned();
    let speed_response = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        "download_speed_limit",
        "Download speed limit",
        Some(
            "A number plus a unit in bits per second: Kbps, Mbps, Gbps or Tbps, for example 250Mbps. Decimals are allowed. Note this is bits, so 80Mbps is about 10 MB/s. A missing or unknown unit means no limit. Default: empty (no limit).",
        ),
        &mut speed_limit,
    );
    if speed_response.changed() {
        *config.download_speed_limit_mut() = speed_limit;
    }
    if config.download_speed_limit_enabled() {
        let current_value = config.download_speed_limit().trim();
        let mut validation_style = body_style.clone();
        validation_style.wrap = true;
        if current_value.is_empty() {
            validation_style.color = ui.visuals().weak_text_color();
            let _ = text_ui.label(
                ui,
                "download_speed_limit_hint",
                "Speed limiter enabled, but no value set. Enter something like 250Mbps.",
                &validation_style,
            );
        } else if let Some(bps) = parse_bitrate_to_bps(current_value) {
            let _ = text_ui.label(
                ui,
                "download_speed_limit_ok",
                &format!("Speed limiter active at {bps} bps."),
                &validation_style,
            );
        } else {
            validation_style.color = ui.visuals().error_fg_color;
            let _ = text_ui.label(
                ui,
                "download_speed_limit_invalid",
                "Invalid speed format. Use Kbps, Mbps, Gbps, or Tbps.",
                &validation_style,
            );
        }
        ui.add_space(style::SPACE_MD);
    }

    let mut default_memory = config.default_instance_max_memory_mib();
    let (max_memory_mib, memory_slider_pending) = memory_slider_max_mib();
    if memory_slider_pending {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    if default_memory > max_memory_mib {
        default_memory = max_memory_mib;
        config.set_default_instance_max_memory_mib(default_memory);
    }

    let memory_response = settings_widgets::u128_slider_with_input_row(
        text_ui,
        ui,
        "instance_defaults_memory_mib",
        "Default max memory allocation (MiB)",
        Some(
            "Maximum memory (Java -Xmx) for every instance that does not set its own memory in its instance settings. Changing it affects those instances the next time they launch. The slider stops at the RAM this computer has. Default: 4096 MiB.",
        ),
        &mut default_memory,
        INSTANCE_DEFAULT_MAX_MEMORY_MIB_MIN,
        max_memory_mib,
        INSTANCE_DEFAULT_MAX_MEMORY_MIB_STEP,
    );
    if memory_response.changed() {
        config.set_default_instance_max_memory_mib(default_memory);
    }
    ui.add_space(8.0);

    settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        "instance_defaults_cli_args",
        "Default CLI args",
        Some(
            "Extra JVM arguments for every instance that does not set its own in its instance settings (for example: -XX:+UseZGC). An instance's own arguments replace these rather than adding to them. Applied the next time the instance launches. Default: empty (no extra arguments).",
        ),
        config.default_instance_cli_args_mut(),
    );
    ui.add_space(10.0);
}

pub(super) fn memory_slider_max_mib() -> (u128, bool) {
    static CACHED: OnceLock<Mutex<MemorySliderMaxState>> = OnceLock::new();
    let cache = CACHED.get_or_init(|| Mutex::new(MemorySliderMaxState::default()));
    let mut total_mib = None;
    let mut pending = false;

    if let Ok(mut state) = cache.lock() {
        if !state.load_complete {
            if let Some(rx) = state.rx.as_ref() {
                match rx.try_recv() {
                    Ok(result) => {
                        state.detected_total_mib = result;
                        state.load_complete = true;
                        state.rx = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        pending = true;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        tracing::error!(
                            target: "vertexlauncher/settings",
                            "Memory slider max probe worker disconnected unexpectedly."
                        );
                        state.load_complete = true;
                        state.rx = None;
                    }
                }
            }

            if !state.load_complete && state.rx.is_none() {
                let (tx, rx) = mpsc::channel::<Option<u128>>();
                state.rx = Some(rx);
                pending = true;
                let _ = tokio_runtime::spawn_blocking_detached(move || {
                    let result = platform::detect_total_memory_mib();
                    if let Err(err) = tx.send(result) {
                        tracing::error!(
                            target: "vertexlauncher/settings",
                            error = %err,
                            "Failed to deliver memory slider max probe result."
                        );
                    }
                });
            }
        }
        total_mib = state.detected_total_mib;
        pending |= !state.load_complete;
    }

    let max_mib = total_mib
        .unwrap_or(FALLBACK_TOTAL_MEMORY_MIB)
        .saturating_sub(RESERVED_SYSTEM_MEMORY_MIB)
        .max(INSTANCE_DEFAULT_MAX_MEMORY_MIB_MIN);
    (max_mib, pending)
}
