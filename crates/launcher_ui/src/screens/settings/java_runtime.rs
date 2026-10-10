use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JavaRuntimeActionKind {
    Download,
    Validate,
}

struct JavaRuntimeActionResult {
    kind: JavaRuntimeActionKind,
    outcome: Result<JavaRuntimeActionOutcome, String>,
}

enum JavaRuntimeActionOutcome {
    Downloaded(PathBuf),
    Validated(String),
}

#[derive(Default)]
struct JavaRuntimeActionState {
    in_flight: Option<JavaRuntimeActionKind>,
    rx: Option<mpsc::Receiver<JavaRuntimeActionResult>>,
    message: Option<String>,
}

pub(super) fn render_java_runtime_settings(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let heading_style = style::section_heading(ui);
    let mut body_style = style::muted(ui);
    body_style.wrap = false;

    let _ = text_ui.label(
        ui,
        "java_paths_heading",
        "Java JVM Paths (Optional)",
        &heading_style,
    );
    let _ = text_ui.label(
        ui,
        "java_paths_description",
        "Leave blank for None. Configure only versions you actually use.",
        &body_style,
    );
    ui.add_space(8.0);

    for runtime in JavaRuntimeVersion::ALL {
        render_java_runtime_path_row(ui, text_ui, config, runtime);
    }
}

fn render_java_runtime_path_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    runtime: JavaRuntimeVersion,
) {
    update_java_runtime_action_state(ui, config, runtime);

    let mut path_value = config
        .java_runtime_path_ref(runtime)
        .map(|path| path.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    let response = settings_widgets::full_width_text_input_row(
        text_ui,
        ui,
        ("instance_java_path", runtime.major()),
        runtime.label(),
        Some(runtime.info_tooltip()),
        &mut path_value,
    );
    if response.changed() {
        config.set_java_runtime_path_ref(
            runtime,
            normalize_optional_input(&path_value)
                .as_deref()
                .map(Path::new),
        );
    }

    render_java_runtime_path_actions(ui, text_ui, config, runtime, path_value.as_str());
    ui.add_space(8.0);
}

fn java_runtime_action_states() -> &'static Mutex<HashMap<u8, JavaRuntimeActionState>> {
    static STATES: OnceLock<Mutex<HashMap<u8, JavaRuntimeActionState>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn update_java_runtime_action_state(ui: &mut Ui, config: &mut Config, runtime: JavaRuntimeVersion) {
    let mut completed = None;
    if let Ok(mut states) = java_runtime_action_states().lock() {
        let state = states.entry(runtime.major()).or_default();
        if let Some(rx) = state.rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => {
                    state.in_flight = None;
                    state.rx = None;
                    completed = Some(result);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(50));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.in_flight = None;
                    state.rx = None;
                    state.message =
                        Some(format!("{} action worker disconnected.", runtime.label()));
                }
            }
        }
    }

    let Some(result) = completed else {
        return;
    };

    let message = match result.outcome {
        Ok(JavaRuntimeActionOutcome::Downloaded(path)) => {
            config.set_java_runtime_path_ref(runtime, Some(path.as_path()));
            format!(
                "Downloaded {} and selected {}.",
                runtime.label(),
                path.display()
            )
        }
        Ok(JavaRuntimeActionOutcome::Validated(version)) => {
            format!("Java validation passed: {version}")
        }
        Err(err) => match result.kind {
            JavaRuntimeActionKind::Download => {
                format!("Failed to download {}: {err}", runtime.label())
            }
            JavaRuntimeActionKind::Validate => format!("Java validation failed: {err}"),
        },
    };

    if let Ok(mut states) = java_runtime_action_states().lock() {
        states.entry(runtime.major()).or_default().message = Some(message);
    }
}

fn render_java_runtime_path_actions(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &Config,
    runtime: JavaRuntimeVersion,
    edited_path_value: &str,
) {
    let (in_flight, message) = java_runtime_action_states()
        .lock()
        .ok()
        .and_then(|states| {
            states
                .get(&runtime.major())
                .map(|state| (state.in_flight, state.message.clone()))
        })
        .unwrap_or((None, None));

    let button_style = ButtonOptions {
        min_size: egui::vec2(180.0, 30.0),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().widgets.active.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    };

    let current_path = normalize_optional_input(edited_path_value).or_else(|| {
        config
            .java_runtime_path_ref(runtime)
            .map(|path| path.as_os_str().to_string_lossy().into_owned())
    });
    let busy = in_flight.is_some();
    let download_label = match in_flight {
        Some(JavaRuntimeActionKind::Download) => "Downloading...".to_owned(),
        _ => format!("Download OpenJDK {}", runtime.major()),
    };
    let validate_label = match in_flight {
        Some(JavaRuntimeActionKind::Validate) => "Testing...".to_owned(),
        _ => "Test Java".to_owned(),
    };

    let has_current_path = current_path.is_some();
    ui.horizontal_wrapped(|ui| {
        let download_clicked = ui
            .add_enabled_ui(!busy, |ui| {
                text_ui.button(
                    ui,
                    ("settings_java_download", runtime.major()),
                    download_label.as_str(),
                    &button_style,
                )
            })
            .inner
            .clicked();
        if download_clicked {
            request_java_runtime_download(runtime);
        }

        let validate_clicked = ui
            .add_enabled_ui(!busy && has_current_path, |ui| {
                text_ui.button(
                    ui,
                    ("settings_java_validate", runtime.major()),
                    validate_label.as_str(),
                    &button_style,
                )
            })
            .inner
            .clicked();
        if validate_clicked {
            if let Some(path) = current_path.clone() {
                request_java_runtime_validation(runtime, path);
            }
        }
    });

    if !has_current_path && in_flight != Some(JavaRuntimeActionKind::Download) {
        let mut hint_style = style::muted(ui);
        hint_style.wrap = true;
        let _ = text_ui.label(
            ui,
            ("settings_java_validate_hint", runtime.major()),
            "Set a Java executable path before testing, or download OpenJDK.",
            &hint_style,
        );
    }

    if let Some(message) = message {
        let mut status_style = style::muted(ui);
        status_style.wrap = true;
        let _ = text_ui.label(
            ui,
            ("settings_java_action_status", runtime.major()),
            message.as_str(),
            &status_style,
        );
    }
}

fn request_java_runtime_download(runtime: JavaRuntimeVersion) {
    let (tx, rx) = mpsc::channel();
    if let Ok(mut states) = java_runtime_action_states().lock() {
        let state = states.entry(runtime.major()).or_default();
        state.in_flight = Some(JavaRuntimeActionKind::Download);
        state.rx = Some(rx);
        state.message = Some(format!("Downloading {}...", runtime.label()));
    }

    let runtime_major = runtime.major();
    tokio_runtime::spawn_detached(async move {
        let outcome = ensure_openjdk_runtime_async(runtime_major)
            .await
            .map(JavaRuntimeActionOutcome::Downloaded)
            .map_err(|err| err.to_string());
        let _ = tx.send(JavaRuntimeActionResult {
            kind: JavaRuntimeActionKind::Download,
            outcome,
        });
    });
}

fn request_java_runtime_validation(runtime: JavaRuntimeVersion, path: String) {
    let (tx, rx) = mpsc::channel();
    if let Ok(mut states) = java_runtime_action_states().lock() {
        let state = states.entry(runtime.major()).or_default();
        state.in_flight = Some(JavaRuntimeActionKind::Validate);
        state.rx = Some(rx);
        state.message = Some(format!("Testing {}...", path));
    }

    tokio_runtime::spawn_detached(async move {
        let outcome = validate_java_executable(path.as_str())
            .await
            .map(JavaRuntimeActionOutcome::Validated);
        let _ = tx.send(JavaRuntimeActionResult {
            kind: JavaRuntimeActionKind::Validate,
            outcome,
        });
    });
}

async fn validate_java_executable(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("No Java executable path is configured.".to_owned());
    }
    let path = Path::new(trimmed);
    tokio::fs::metadata(path)
        .await
        .map_err(|err| format!("Path is not readable: {trimmed}: {err}"))?;

    let output = tokio::process::Command::new(path)
        .arg("-version")
        .output()
        .await
        .map_err(|err| format!("Could not run {trimmed}: {err}"))?;
    let stdout = String::from_utf8_lossy(output.stdout.as_slice());
    let stderr = String::from_utf8_lossy(output.stderr.as_slice());
    let combined = format!("{stdout}\n{stderr}");
    let first_line = combined
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("<no version output>");
    let combined_lower = combined.to_ascii_lowercase();

    if !output.status.success() {
        return Err(format!(
            "`{trimmed} -version` exited with {}; output: {first_line}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_owned())
        ));
    }

    let looks_like_java = combined_lower.contains("openjdk")
        || combined_lower.contains("java version")
        || combined_lower.contains("runtime environment")
        || combined_lower.contains("java(tm)");
    if !looks_like_java {
        return Err(format!(
            "`{trimmed} -version` ran, but output did not look like Java: {first_line}"
        ));
    }

    Ok(first_line.to_owned())
}

fn normalize_optional_input(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
