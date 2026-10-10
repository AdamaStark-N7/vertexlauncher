use super::*;

/// How long a screen may stay inactive before its state is purged. Zero: as soon as the screen
/// has been off screen for a full frame (see `screen_in_use`).
pub(super) const DISCOVER_STATE_PURGE_DELAY: Duration = Duration::ZERO;

#[derive(Default)]
pub(super) struct ScreenPurgeTimer {
    pub(super) inactive_since: Option<Instant>,
    pub(super) purged: bool,
}

#[derive(Debug)]
pub enum RunError {
    RuntimeBootstrap(launcher_runtime::RuntimeBootstrapError),
    Ui(eframe::Error),
}

pub fn init_tracing() -> Option<PathBuf> {
    launcher_runtime::set_detached_task_reporter(report_detached_task_failure);
    tracing_setup::init_tracing()
}

pub(super) fn report_detached_task_failure(task_kind: &str, error: &launcher_runtime::TaskError) {
    notification::emit_replace(
        notification::Severity::Error,
        "background-task",
        format!("A background task failed ({task_kind}): {error}"),
        format!("background-task/{task_kind}"),
    );
}

pub(super) fn is_discover_screen(screen: screens::AppScreen) -> bool {
    matches!(
        screen,
        screens::AppScreen::Discover | screens::AppScreen::DiscoverDetail
    )
}

pub(super) fn update_screen_purge_timer(
    ctx: &egui::Context,
    timer: &mut ScreenPurgeTimer,
    is_active: bool,
    purge_delay: Duration,
    purge: impl FnOnce(),
) {
    if is_active {
        timer.inactive_since = None;
        timer.purged = false;
        return;
    }

    let inactive_since = timer.inactive_since.get_or_insert_with(Instant::now);
    if timer.purged {
        return;
    }

    let elapsed = inactive_since.elapsed();
    if elapsed >= purge_delay {
        purge();
        timer.purged = true;
    } else {
        ctx.request_repaint_after(purge_delay - elapsed);
    }
}

pub(super) struct VertexApp {
    pub(super) fonts: FontController,
    pub(super) config: Config,
    pub(super) theme_catalog: ui::theme::ThemeCatalog,
    pub(super) theme: ui::theme::Theme,
    pub(super) show_config_format_modal: bool,
    pub(super) selected_config_format: ConfigFormat,
    pub(super) default_config_format: ConfigFormat,
    pub(super) config_creation_error: Option<String>,
    pub(super) active_screen: screens::AppScreen,
    pub(super) instance_shortcuts: Vec<ui::sidebar::ProfileShortcut>,
    pub(super) selected_instance_id: Option<String>,
    pub(super) instance_store: InstanceStore,
    pub(super) content_browser_state: screens::ContentBrowserState,
    pub(super) discover_state: screens::DiscoverState,
    pub(super) home_purge_timer: ScreenPurgeTimer,
    pub(super) library_purge_timer: ScreenPurgeTimer,
    pub(super) instance_purge_timer: ScreenPurgeTimer,
    pub(super) discover_purge_timer: ScreenPurgeTimer,
    pub(super) content_browser_purge_timer: ScreenPurgeTimer,
    pub(super) skins_purge_timer: ScreenPurgeTimer,
    /// The screen that was active at the end of the previous frame. A screen counts as in use for
    /// one extra frame after navigating away, so its last frame is fully painted before its
    /// resources are freed.
    pub(super) last_frame_screen: screens::AppScreen,
    /// Screen and instance whose resources the frame sweep last saw.
    pub(super) resource_scope: (screens::AppScreen, Option<String>),
    /// Set when the scope changed; the sweep runs at the end of the next frame, once the new
    /// screen has drawn and thereby marked everything it needs.
    pub(super) resource_sweep_pending: bool,
    pub(super) show_create_instance_modal: bool,
    pub(super) create_instance_state: create_instance_modal::CreateInstanceState,
    pub(super) show_import_instance_modal: bool,
    pub(super) import_instance_state: import_instance_modal::ImportInstanceState,
    pub(super) show_gamepad_calibration_modal: bool,
    pub(super) gamepad_calibration_state: gamepad_calibration_modal::GamepadCalibrationState,
    pub(super) in_flight_import_request: Option<import_instance_modal::ImportRequest>,
    pub(super) curseforge_manual_download_preflight_request:
        Option<import_instance_modal::ImportRequest>,
    pub(super) curseforge_manual_download_preflight_in_flight: bool,
    pub(super) curseforge_manual_download_preflight_rx: Option<
        mpsc::Receiver<
            Result<Option<Vec<import_instance_modal::CurseForgeManualDownloadRequirement>>, String>,
        >,
    >,
    pub(super) discover_curseforge_manual_download_preflight_request:
        Option<screens::DiscoverInstallRequest>,
    pub(super) discover_curseforge_manual_download_preflight_in_flight: bool,
    pub(super) discover_curseforge_manual_download_preflight: launcher_runtime::WorkerChannel<
        Result<Option<import_instance_modal::CurseForgeManualDownloadRequirement>, String>,
    >,
    pub(super) pending_curseforge_manual_download: Option<PendingCurseForgeManualDownloadState>,
    pub(super) discover_install_progress:
        launcher_runtime::WorkerChannel<import_instance_modal::ImportProgress>,
    pub(super) discover_install_results:
        launcher_runtime::WorkerChannel<import_instance_modal::ImportTaskResult>,
    pub(super) auth: AuthState,
    pub(super) startup_graphics: platform::StartupGraphicsConfig,
    pub(super) text_ui: TextUi,
    pub(super) config_save_in_flight: bool,
    pub(super) pending_config_save: Option<Config>,
    pub(super) config_save_results: launcher_runtime::WorkerChannel<Result<(), String>>,
    pub(super) link_profile_results: launcher_runtime::WorkerChannel<(
        PathBuf,
        Result<screens::DetectedInstanceVersions, String>,
    )>,
    pub(super) instance_store_save_in_flight: bool,
    pub(super) pending_instance_store_save: Option<InstanceStore>,
    pub(super) instance_store_save_results: launcher_runtime::WorkerChannel<Result<(), String>>,
    pub(super) initial_install_results:
        launcher_runtime::WorkerChannel<InitialInstanceInstallResult>,
    pub(super) discord_presence: DiscordPresenceManager,
    pub(super) gamepad: Option<gamepad::GamepadNavigator>,
    pub(super) last_rendered_screen: Option<screens::AppScreen>,
    pub(super) settings_screen_prewarmed: bool,
}

#[derive(Debug)]
pub(super) enum InitialInstanceInstallResult {
    Failed {
        instance_id: String,
        instance_name: String,
        error: String,
    },
}

/// File-dialog slot for "Link existing profile".

#[path = "app_core/frame_phases.rs"]
mod frame_phases;
#[path = "app_core/link_profile.rs"]
mod link_profile;
#[path = "app_core/screen_lifecycle.rs"]
mod screen_lifecycle;

impl VertexApp {
    pub(super) fn new(cc: &eframe::CreationContext<'_>, config_state: LoadConfigResult) -> Self {
        tracing::info!(
            target: "vertexlauncher/app/startup",
            "VertexApp::new entered."
        );
        egui_extras::install_image_loaders(&cc.egui_ctx);
        #[cfg(target_os = "macos")]
        app_icon::apply_macos_dock_icon();

        let (
            mut config,
            config_loaded_from_disk,
            show_config_format_modal,
            selected_config_format,
            default_config_format,
        ) = match config_state {
            LoadConfigResult::Loaded(config) => {
                (config, true, false, ConfigFormat::Json, ConfigFormat::Json)
            }
            LoadConfigResult::Missing { default_format } => (
                Config::default(),
                false,
                true,
                default_format,
                default_format,
            ),
        };

        config.normalize();
        #[cfg(target_os = "macos")]
        if config.window_transparency().uses_native_blur() {
            downgrade_window_blur_for_startup(
                cc,
                &mut config,
                config_loaded_from_disk,
                "Window blur is temporarily downgraded to transparent mode on macOS to keep launcher startup on the stable path.".to_owned(),
                "macOS safety fallback",
            );
        }
        if let Err(error) = window_effects::apply(
            cc,
            config.window_transparency(),
            config.windows_backdrop_type(),
            config.linux_blur_protocol(),
            config.macos_visual_effect_material(),
            config.macos_visual_effect_blending_mode(),
            config.macos_visual_effect_state(),
            config.macos_visual_effect_emphasized(),
        ) {
            downgrade_window_blur_for_startup(
                cc,
                &mut config,
                config_loaded_from_disk,
                format!(
                    "Window blur is unsupported here and has been downgraded to transparent mode. Restart may be required to fully apply the change. {error}"
                ),
                "unsupported platform check",
            );
        }

        let theme_catalog = ui::theme::ThemeCatalog::load();
        if !theme_catalog.contains(config.theme_id()) {
            config.set_theme_id(theme_catalog.default_theme_id().to_owned());
        }
        let theme = theme_catalog.resolve(config.theme_id()).clone();
        let startup_graphics = platform::startup_graphics_config(
            transparent_viewport_enabled(&config),
            config.graphics_api_preference(),
        );

        let mut text_ui =
            TextUi::new_with_graphics_config(build_text_graphics_config(&config, startup_graphics));
        let _ =
            textui_adapter::begin_frame(&mut text_ui, &cc.egui_ctx, cc.wgpu_render_state.as_ref());
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            launcher_ui::ui::components::gpu_texture::install(render_state);
        }
        FontController::register_included_fonts(&mut text_ui);

        let instance_store = match load_store() {
            Ok(store) => store,
            Err(err) => {
                notification::error!("instance_store", "Failed to load instance store: {err}");
                InstanceStore::default()
            }
        };
        let streamer_mode_enabled = config.streamer_mode_enabled();
        if app_metadata::try_settings_info().is_none() {
            let _ = tokio_runtime::spawn_blocking_detached(app_metadata::preload_settings_info);
        }

        let mut app = Self {
            fonts: FontController::new(config.ui_font_family()),
            config,
            theme_catalog,
            theme,
            show_config_format_modal,
            selected_config_format,
            default_config_format,
            config_creation_error: None,
            active_screen: screens::AppScreen::Home,
            instance_shortcuts: Vec::new(),
            selected_instance_id: None,
            instance_store,
            content_browser_state: screens::ContentBrowserState::default(),
            discover_state: screens::DiscoverState::default(),
            home_purge_timer: ScreenPurgeTimer::default(),
            library_purge_timer: ScreenPurgeTimer::default(),
            instance_purge_timer: ScreenPurgeTimer::default(),
            discover_purge_timer: ScreenPurgeTimer::default(),
            content_browser_purge_timer: ScreenPurgeTimer::default(),
            skins_purge_timer: ScreenPurgeTimer::default(),
            last_frame_screen: screens::AppScreen::Home,
            resource_scope: (screens::AppScreen::Home, None),
            resource_sweep_pending: false,
            show_create_instance_modal: false,
            create_instance_state: create_instance_modal::CreateInstanceState::default(),
            show_import_instance_modal: false,
            import_instance_state: import_instance_modal::ImportInstanceState::default(),
            show_gamepad_calibration_modal: false,
            gamepad_calibration_state: gamepad_calibration_modal::GamepadCalibrationState::default(
            ),
            in_flight_import_request: None,
            curseforge_manual_download_preflight_request: None,
            curseforge_manual_download_preflight_in_flight: false,
            curseforge_manual_download_preflight_rx: None,
            discover_curseforge_manual_download_preflight_request: None,
            discover_curseforge_manual_download_preflight_in_flight: false,
            discover_curseforge_manual_download_preflight: Default::default(),
            pending_curseforge_manual_download: None,
            discover_install_progress: Default::default(),
            discover_install_results: Default::default(),
            auth: AuthState::load(streamer_mode_enabled),
            startup_graphics,
            text_ui,
            config_save_in_flight: false,
            pending_config_save: None,
            config_save_results: Default::default(),
            link_profile_results: Default::default(),
            instance_store_save_in_flight: false,
            pending_instance_store_save: None,
            instance_store_save_results: Default::default(),
            initial_install_results: Default::default(),
            discord_presence: DiscordPresenceManager::default(),
            gamepad: gamepad::GamepadNavigator::new(),
            last_rendered_screen: None,
            settings_screen_prewarmed: false,
        };

        app.refresh_instance_shortcuts();
        if let Some(first) = app.instance_shortcuts.first() {
            app.selected_instance_id = Some(first.id.clone());
        }

        app.fonts.ensure_selected_font_is_available(&mut app.config);
        app.fonts
            .apply_from_config(&cc.egui_ctx, &app.config, &mut app.text_ui);
        app
    }

    #[allow(deprecated)]
    pub(super) fn update_inner(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update_gamepad_input(ctx);
        self.begin_frame(ctx, frame);

        let previous_config = self.config.clone();
        let previous_instance_store = self.instance_store.clone();
        self.poll_task_results();
        self.apply_config_appearance(ctx);
        let _ = self.handle_escape(ctx);

        let streamer_mode = self.config.streamer_mode_enabled();
        let top_bar_output = self.render_top_bar_and_notifications(ctx, streamer_mode);
        self.apply_top_bar_sign_in_actions(&top_bar_output);
        let previous_active_screen = self.active_screen;
        self.focus_settings_on_open(ctx);
        let previous_selected_instance_id = self.selected_instance_id.clone();
        let account_switched = self.apply_top_bar_account_actions(&top_bar_output);

        svg_aa::set_svg_aa_mode(self.config.svg_aa_mode());

        self.render_sidebar(ctx);
        let screen_output = self.render_central_panel(ctx, frame, account_switched, streamer_mode);
        let menu_presence_context = self.apply_screen_output(ctx, screen_output);

        self.update_discover_lifecycle(ctx, frame.wgpu_render_state());
        self.render_modals(ctx);

        self.commit_config_changes(ctx, &previous_config);
        self.commit_instance_store_changes(&previous_instance_store);

        let menu_presence_context = menu_presence_context
            .unwrap_or(screens::MenuPresenceContext::Screen(self.active_screen));
        self.finish_navigation(
            ctx,
            previous_active_screen,
            previous_selected_instance_id.as_deref(),
            menu_presence_context,
        );

        self.discord_presence.update(
            &self.config,
            &self.instance_store,
            self.config.minecraft_installations_root_path(),
            menu_presence_context,
            self.selected_instance_id.as_deref(),
        );

        ui::style::flush_tooltips(&mut self.text_ui, ctx);
        ui::top_bar::handle_window_resize(ctx);
        self.sweep_screen_resources(ctx);
    }

    pub(super) fn create_config_with_choice(&mut self, choice: ConfigFormat) {
        match create_default_config(choice) {
            Ok(config) => {
                self.config = config;
                self.config.normalize();
                self.fonts
                    .ensure_selected_font_is_available(&mut self.config);
                self.show_config_format_modal = false;
                self.config_creation_error = None;
            }
            Err(err) => {
                self.config_creation_error = Some(format!("Failed to create config: {err}"));
            }
        }
    }

    pub(super) fn sync_theme_from_config(&mut self) {
        if !self.theme_catalog.contains(self.config.theme_id()) {
            self.config
                .set_theme_id(self.theme_catalog.default_theme_id().to_owned());
        }

        let resolved = self.theme_catalog.resolve(self.config.theme_id());
        if self.theme.id != resolved.id {
            self.theme = resolved.clone();
        }
    }

    pub(super) fn refresh_instance_shortcuts(&mut self) {
        self.instance_shortcuts = self
            .instance_store
            .instances
            .iter()
            .map(|instance| ui::sidebar::ProfileShortcut {
                id: instance.id.clone(),
                name: instance.name.clone(),
                thumbnail_path: instance.thumbnail_path.clone(),
            })
            .collect();
    }

    pub(super) fn open_instance_folder(&mut self, instance_id: &str) {
        let Some(instance) = self.instance_store.find(instance_id).cloned() else {
            notification::error!(
                "instance_context_menu",
                "Could not find the selected instance to open its folder."
            );
            return;
        };

        let installations_root = self
            .config
            .minecraft_installations_root_path()
            .to_path_buf();
        let instance_root = instance_root_path(installations_root.as_path(), &instance);
        if let Err(err) = launcher_ui::desktop::open_in_file_manager(&instance_root) {
            notification::error!(
                "instance_context_menu",
                "Failed to open instance folder: {err}"
            );
        }
    }

    pub(super) fn apply_frame_limiter(&mut self, ctx: &egui::Context) {
        if !self.config.frame_limiter_enabled() {
            return;
        }

        let fps = self.config.frame_limit_fps().clamp(30, 240) as u32;
        let frame_time = Duration::from_secs_f64(1.0 / fps as f64);
        ctx.request_repaint_after(frame_time);
    }

    pub(super) fn handle_escape(&mut self, ctx: &egui::Context) -> bool {
        if !ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            return false;
        }
        if egui::Popup::is_any_open(ctx) {
            egui::Popup::close_all(ctx);
            return true;
        }
        if self.pending_curseforge_manual_download.is_some() {
            if self.import_instance_state.import_in_flight
                || self.curseforge_manual_download_preflight_in_flight
                || self.discover_curseforge_manual_download_preflight_in_flight
            {
                return true;
            }
            cancel_pending_curseforge_manual_download(self);
            return true;
        }
        if self.show_import_instance_modal {
            if self.import_instance_state.import_in_flight {
                return true;
            }
            self.show_import_instance_modal = false;
            self.import_instance_state.reset();
            return true;
        }
        if self.show_create_instance_modal {
            if self.create_instance_state.create_in_flight {
                return true;
            }
            self.show_create_instance_modal = false;
            self.create_instance_state.reset();
            return true;
        }
        if self.show_config_format_modal {
            self.create_config_with_choice(self.default_config_format);
            return true;
        }
        if self.active_screen == screens::AppScreen::DiscoverDetail {
            self.active_screen = screens::AppScreen::Discover;
            return true;
        }
        screens::handle_escape(
            ctx,
            self.active_screen,
            self.selected_instance_id.as_deref(),
        )
    }
}
