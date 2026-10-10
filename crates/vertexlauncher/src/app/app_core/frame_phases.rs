use super::*;

/// Per-frame phases of `VertexApp::update_inner`, in the order they run.
impl VertexApp {
    /// Screen-specific launch auth context for the currently active account, if any.
    fn screen_launch_auth(&self) -> Option<screens::LaunchAuthContext> {
        self.auth
            .active_launch_context()
            .map(|context| screens::LaunchAuthContext {
                account_key: context.account_key,
                player_name: context.player_name,
                player_uuid: context.player_uuid,
                access_token: context.access_token,
                xuid: context.xuid,
                user_type: context.user_type,
            })
    }

    /// Polls the gamepad and forwards its per-screen inputs to the screens that consume them.
    #[allow(deprecated)]
    pub(super) fn update_gamepad_input(&mut self, ctx: &egui::Context) {
        let gamepad_update = if let Some(gamepad) = &mut self.gamepad {
            gamepad.update(ctx, self.config.gamepad_calibrations(), self.active_screen)
        } else {
            gamepad::GamepadUpdate::default()
        };
        screens::set_skins_gamepad_orbit_input(
            ctx,
            if self.active_screen == screens::AppScreen::Skins {
                gamepad_update.skins_preview_orbit
            } else {
                0.0
            },
        );
        let (home_pan, home_zoom) = if self.active_screen == screens::AppScreen::Home {
            (
                gamepad_update.screenshot_viewer_pan,
                gamepad_update.screenshot_viewer_zoom,
            )
        } else {
            (egui::Vec2::ZERO, 0.0)
        };
        screens::set_home_screenshot_viewer_gamepad_input(ctx, home_pan, home_zoom);
        let (instance_pan, instance_zoom) = if self.active_screen == screens::AppScreen::Instance {
            (
                gamepad_update.screenshot_viewer_pan,
                gamepad_update.screenshot_viewer_zoom,
            )
        } else {
            (egui::Vec2::ZERO, 0.0)
        };
        screens::set_instance_screenshot_viewer_gamepad_input(ctx, instance_pan, instance_zoom);
        if let Some(device) = gamepad_update.calibration_requested
            && !self.show_gamepad_calibration_modal
        {
            self.show_gamepad_calibration_modal = true;
            self.gamepad_calibration_state.start(device);
        }
    }

    /// Frame setup: limiter, text/image frame hooks, and the results of finished background work.
    #[allow(deprecated)]
    pub(super) fn begin_frame(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.apply_frame_limiter(ctx);
        ui_foundation::set_drag_scroll_enabled(self.config.drag_scroll_enabled());
        auth::set_insecure_file_storage(self.config.insecure_file_account_storage());
        self.text_ui.set_graphics_config(build_text_graphics_config(
            &self.config,
            self.startup_graphics,
        ));
        let _ = textui_adapter::begin_frame(&mut self.text_ui, ctx, frame.wgpu_render_state());
        launcher_ui::ui::components::image_textures::begin_frame(ctx);
        launcher_ui::ui::components::remote_tiled_image::begin_frame();
        self.poll_link_existing_profile(ctx);
        poll_config_save_results(self);
        poll_instance_store_save_results(self);
        self.auth.poll();
        poll_finished_instance_process_notifications(self);
        let playing_as = if running_instance_roots().is_empty() {
            None
        } else {
            self.screen_launch_auth().map(|context| context.player_name)
        };
        if let Some(screen) = screens::take_requested_screen(ctx) {
            tracing::info!(
                target: "vertexlauncher/navigation",
                from_screen = ?self.active_screen,
                to_screen = ?screen,
                "A UI action requested navigation."
            );
            self.active_screen = screen;
        }
        launcher_ui::screenshot_watch::poll(
            ctx,
            &self.instance_store,
            &self.config,
            playing_as.as_deref(),
        );
        console::prune_instance_tabs(&running_instance_roots());
        apply_install_activity_os_feedback(ctx, frame);
        if self.auth.should_request_repaint() {
            ctx.request_repaint_after(REPAINT_INTERVAL);
        }
    }

    /// Drains create/import/install worker results into app state.
    pub(super) fn poll_task_results(&mut self) {
        poll_create_instance_result(self);
        poll_curseforge_manual_download_preflight(self);
        poll_discover_curseforge_manual_download_preflight(self);
        poll_import_instance_progress(self);
        poll_import_instance_result(self);
        poll_pending_curseforge_manual_download(self);
        poll_discover_install_progress(self);
        poll_discover_install_result(self);
        poll_initial_instance_install_results(self);
    }

    /// Applies theme, streamer mode, fonts and typography from the current config.
    pub(super) fn apply_config_appearance(&mut self, ctx: &egui::Context) {
        self.sync_theme_from_config();
        self.theme
            .apply(ctx, effective_ui_opacity_percent(&self.config));
        self.auth
            .set_streamer_mode(self.config.streamer_mode_enabled());
        notification::set_streamer_mode(self.config.streamer_mode_enabled());
        self.fonts
            .ensure_selected_font_is_available(&mut self.config);
        self.fonts
            .apply_from_config(ctx, &self.config, &mut self.text_ui);
        ui::style::publish_typography(ctx, self.config.typography());
    }

    /// Name of the selected instance when the instance screen is open.
    fn selected_instance_name_on_instance_screen(&self) -> Option<&str> {
        if self.active_screen != screens::AppScreen::Instance {
            return None;
        }
        self.selected_instance_id
            .as_deref()
            .and_then(|id| self.instance_store.find(id))
            .map(|instance| instance.name.as_str())
    }

    /// Renders the top bar and the notification popups.
    pub(super) fn render_top_bar_and_notifications(
        &mut self,
        ctx: &egui::Context,
        streamer_mode: bool,
    ) -> ui::top_bar::TopBarOutput {
        let account_entries = self.auth.account_entries();
        let profile_accounts = account_entries
            .iter()
            .map(|entry| ui::top_bar::ProfileAccountOption {
                profile_id: entry.profile_id.clone(),
                display_name: entry.display_name.clone(),
                is_active: entry.is_active,
                is_failed: entry.is_failed,
            })
            .collect::<Vec<_>>();
        let user_instance_active = self.auth.active_account_key().is_some_and(|account_key| {
            running_instance_for_account(account_key.as_str()).is_some()
        });

        let top_bar_section_label = self
            .selected_instance_name_on_instance_screen()
            .unwrap_or_else(|| self.active_screen.label())
            .to_owned();

        let top_bar_output = ui::top_bar::render(
            ctx,
            top_bar_section_label.as_str(),
            &mut self.text_ui,
            ui::top_bar::ProfileUiModel {
                display_name: self.auth.display_name(),
                avatar_png: self.auth.avatar_png(),
                sign_in_in_progress: self.auth.sign_in_in_progress(),
                auth_busy: self.auth.auth_busy(),
                token_refresh_in_progress: self.auth.token_refresh_in_progress(),
                streamer_mode,
                status_message: self.auth.status_message(),
                accounts: &profile_accounts,
                user_instance_active,
                device_code_prompt: self.auth.device_code_prompt(),
            },
        );
        let suppressed_progress_source = self
            .selected_instance_name_on_instance_screen()
            .map(|name| format!("installation/{name}"));
        notification::render_popups(
            ctx,
            &mut self.text_ui,
            &notification::NotificationDisplaySettings {
                expiry_bars_empty_left: self.config.notification_expiry_bars_empty_left(),
                fade_out: std::time::Duration::from_secs_f32(
                    self.config.notification_fade_out_seconds(),
                ),
                easing: self.config.notification_easing(),
            },
            suppressed_progress_source.as_deref(),
        );
        top_bar_output
    }

    /// Starts or cancels sign-in flows requested from the top bar.
    pub(super) fn apply_top_bar_sign_in_actions(&mut self, output: &ui::top_bar::TopBarOutput) {
        if output.start_webview_sign_in {
            self.auth.start_webview_sign_in();
        }
        if output.start_device_code_sign_in {
            self.auth.start_device_code_sign_in();
        }
        if output.cancel_device_code_sign_in {
            self.auth.cancel_device_code_sign_in();
        }
        if output.open_device_code_browser {
            self.auth.start_system_browser_sign_in(&self.theme);
        }
    }

    /// Drops focus and asks the settings screen to focus its theme control on first open.
    pub(super) fn focus_settings_on_open(&mut self, ctx: &egui::Context) {
        if self.active_screen == screens::AppScreen::Settings
            && self.last_rendered_screen != Some(screens::AppScreen::Settings)
        {
            if let Some(focused_id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(focused_id));
            }
            screens::request_settings_theme_focus(ctx);
        }
    }

    /// Applies account selection/removal/refresh and terminal requests from the top bar.
    /// Returns whether the active account was switched.
    pub(super) fn apply_top_bar_account_actions(
        &mut self,
        output: &ui::top_bar::TopBarOutput,
    ) -> bool {
        let mut account_switched = false;
        if let Some(profile_id) = output.select_account_id.as_deref() {
            self.auth.select_account(profile_id);
            account_switched = true;
        }
        if let Some(profile_id) = output.remove_account_id.as_deref() {
            self.auth.remove_account(profile_id);
        }
        if let Some(profile_id) = output.refresh_account_id.as_deref() {
            self.auth.refresh_account_token(profile_id);
        }
        if output.open_active_user_terminal {
            tracing::info!(
                target: "vertexlauncher/navigation",
                from_screen = ?self.active_screen,
                to_screen = ?screens::AppScreen::Console,
                "Top bar opened the active user terminal."
            );
            self.active_screen = screens::AppScreen::Console;
            let active_launch_auth = self.screen_launch_auth();
            let _ = console::activate_tab_for_user(
                active_launch_auth
                    .as_ref()
                    .map(|context| context.account_key.as_str()),
                self.auth.display_name(),
            );
        }
        account_switched
    }

    /// Renders the sidebar and applies its navigation, context-menu and modal requests.
    pub(super) fn render_sidebar(&mut self, ctx: &egui::Context) {
        let sidebar_output = ui::sidebar::render(
            ctx,
            self.active_screen,
            &self.instance_shortcuts,
            &mut self.text_ui,
        );

        if let Some(next_screen) = sidebar_output.selected_screen {
            tracing::info!(
                target: "vertexlauncher/navigation",
                from_screen = ?self.active_screen,
                to_screen = ?next_screen,
                "Sidebar selected a screen."
            );
            self.active_screen = next_screen;
        }
        if let Some(instance_id) = sidebar_output.selected_profile_id {
            tracing::info!(
                target: "vertexlauncher/navigation",
                instance_id = %instance_id,
                "Sidebar selected an instance profile."
            );
            self.selected_instance_id = Some(instance_id);
            self.active_screen = screens::AppScreen::Instance;
        }
        for (instance_id, action) in sidebar_output.instance_context_actions {
            self.apply_instance_context_action(ctx, instance_id, action);
        }
        if sidebar_output.create_instance_clicked {
            self.show_create_instance_modal = true;
            self.create_instance_state.error = None;
        }
        if sidebar_output.import_instance_clicked {
            self.show_import_instance_modal = true;
            self.import_instance_state.error = None;
        }
        if sidebar_output.link_existing_profile_clicked {
            self.link_existing_profile_folder(ctx);
        }
    }

    fn apply_instance_context_action(
        &mut self,
        ctx: &egui::Context,
        instance_id: String,
        action: InstanceContextAction,
    ) {
        match action {
            InstanceContextAction::OpenInstance => {
                tracing::info!(
                    target: "vertexlauncher/navigation",
                    instance_id = %instance_id,
                    "Instance context menu opened an instance."
                );
                self.selected_instance_id = Some(instance_id);
                self.active_screen = screens::AppScreen::Instance;
            }
            InstanceContextAction::OpenFolder => {
                self.open_instance_folder(&instance_id);
            }
            InstanceContextAction::CopyLaunchCommand => {
                self.copy_quick_launch_text(
                    ctx,
                    instance_id.as_str(),
                    screens::build_quick_launch_command,
                    "Copied instance command line to clipboard.",
                    "Sign in before copying an instance command line.",
                );
            }
            InstanceContextAction::CopySteamLaunchOptions => {
                self.copy_quick_launch_text(
                    ctx,
                    instance_id.as_str(),
                    screens::build_quick_launch_steam_options,
                    "Copied Steam launch options to clipboard.",
                    "Sign in before copying Steam launch options.",
                );
            }
            InstanceContextAction::Delete => {
                tracing::info!(
                    target: "vertexlauncher/navigation",
                    instance_id = %instance_id,
                    "Instance context menu requested delete; redirecting to Library."
                );
                self.selected_instance_id = Some(instance_id.clone());
                self.active_screen = screens::AppScreen::Library;
                screens::request_delete_instance(ctx, &instance_id);
            }
        }
    }

    /// Copies a quick-launch command or Steam options string for `instance_id` to the clipboard.
    fn copy_quick_launch_text(
        &self,
        ctx: &egui::Context,
        instance_id: &str,
        build: fn(
            screens::QuickLaunchCommandMode,
            &str,
            &str,
            Option<&str>,
            Option<&str>,
        ) -> String,
        success_message: &'static str,
        sign_in_message: &'static str,
    ) {
        let quick_auth = screens::PlayerAuthContext {
            launch_auth: self.screen_launch_auth(),
            token_refresh_in_progress: false,
            account_avatars: self.auth.account_avatars_by_key(),
        };
        if let Some(user) = screens::selected_quick_launch_user(&quick_auth) {
            let text = build(
                screens::QuickLaunchCommandMode::Pack,
                instance_id,
                user.as_str(),
                None,
                None,
            );
            ctx.copy_text(text);
            notification::info!("sidebar/quick_launch", "{success_message}");
        } else {
            notification::warn!("sidebar/quick_launch", "{sign_in_message}");
        }
    }

    /// Renders the active screen inside the central panel and returns what it requested.
    #[allow(deprecated)]
    pub(super) fn render_central_panel(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        account_switched: bool,
        streamer_mode: bool,
    ) -> screens::ScreenOutput {
        let mut screen_output = screens::ScreenOutput::default();
        let wgpu_target_format = frame.wgpu_render_state().map(|state| state.target_format);
        let skin_preview_msaa_samples = 4;
        if app_metadata::try_settings_info().is_none() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let settings_info = app_metadata::try_settings_info().unwrap_or_default();
        let options = screens::AvailableOptions {
            ui_fonts: self.fonts.available_ui_fonts(),
            ui_font_labels: self.fonts.available_ui_font_labels(),
            emoji_fonts: self.fonts.available_emoji_fonts(),
            emoji_font_labels: self.fonts.available_emoji_font_labels(),
            cjk_fonts: self.fonts.available_cjk_fonts(),
            cjk_font_labels: self.fonts.available_cjk_font_labels(),
            themes: self.theme_catalog.themes(),
            theme_labels: self.theme_catalog.theme_labels(),
            settings_info: &settings_info,
        };
        if !self.settings_screen_prewarmed {
            screens::prewarm_settings(ctx, &mut self.text_ui, &self.config, &options);
            self.settings_screen_prewarmed = true;
        }
        let skin_manager = screens::SkinManagerContext {
            opened: self.active_screen == screens::AppScreen::Skins
                && self.last_rendered_screen != Some(screens::AppScreen::Skins),
            account_switched: self.active_screen == screens::AppScreen::Skins && account_switched,
            wgpu_target_format,
            msaa_samples: skin_preview_msaa_samples,
        };
        let auth = screens::PlayerAuthContext {
            launch_auth: self.screen_launch_auth(),
            token_refresh_in_progress: self.auth.token_refresh_in_progress(),
            account_avatars: self.auth.account_avatars_by_key(),
        };
        CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ctx.global_style().visuals.panel_fill)
                    .inner_margin(egui::Margin::ZERO)
                    .outer_margin(egui::Margin::ZERO)
                    .stroke(egui::Stroke::new(
                        1.0,
                        ctx.global_style()
                            .visuals
                            .widgets
                            .noninteractive
                            .bg_stroke
                            .color,
                    )),
            )
            .show(ctx, |ui| {
                let content_rect = ui
                    .max_rect()
                    .shrink2(egui::vec2(ui::style::SPACE_MD, ui::style::SPACE_SM));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(content_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        screen_output = screens::render(
                            ui,
                            self.active_screen,
                            skin_manager,
                            self.selected_instance_id.as_deref(),
                            &auth,
                            streamer_mode,
                            &mut self.config,
                            &mut self.instance_store,
                            &options,
                            &mut self.content_browser_state,
                            &mut self.discover_state,
                            &mut self.text_ui,
                        );
                    },
                );
            });
        self.last_rendered_screen = Some(self.active_screen);
        screen_output
    }

    /// Applies what the rendered screen requested. Returns its menu presence context, if any.
    pub(super) fn apply_screen_output(
        &mut self,
        ctx: &egui::Context,
        screen_output: screens::ScreenOutput,
    ) -> Option<screens::MenuPresenceContext> {
        if screen_output.instances_changed {
            self.refresh_instance_shortcuts();
        }
        if let Some(instance_id) = screen_output.selected_instance_id {
            tracing::info!(
                target: "vertexlauncher/navigation",
                selected_instance_id = %instance_id,
                "Screen output selected an instance."
            );
            self.selected_instance_id = Some(instance_id);
        }
        if let Some(request) = screen_output.discover_install_requested {
            start_discover_install_task(self, request);
        }
        if let Some(instance_id) = screen_output.delete_requested_instance_id {
            tracing::info!(
                target: "vertexlauncher/navigation",
                instance_id = %instance_id,
                "Screen output requested instance deletion flow; redirecting to Library."
            );
            self.selected_instance_id = Some(instance_id.clone());
            self.active_screen = screens::AppScreen::Library;
            screens::request_delete_instance(ctx, &instance_id);
        }
        if let Some(requested_screen) = screen_output.requested_screen {
            tracing::info!(
                target: "vertexlauncher/navigation",
                from_screen = ?self.active_screen,
                requested_screen = ?requested_screen,
                "Screen output requested navigation."
            );
            self.active_screen = requested_screen;
        }
        screen_output.menu_presence_context
    }

    /// Renders every open modal dialog and applies its result.
    pub(super) fn render_modals(&mut self, ctx: &egui::Context) {
        self.render_config_format_modal(ctx);
        self.render_create_instance_modal(ctx);
        self.render_import_instance_modal(ctx);
        self.render_gamepad_calibration_modal(ctx);
        self.render_curseforge_manual_download_modal(ctx);
    }

    fn render_config_format_modal(&mut self, ctx: &egui::Context) {
        if !self.show_config_format_modal {
            return;
        }
        match config_format_modal::render(
            ctx,
            &mut self.text_ui,
            &mut self.selected_config_format,
            self.config_creation_error.as_deref(),
        ) {
            ModalAction::None => {}
            ModalAction::Cancel => self.create_config_with_choice(self.default_config_format),
            ModalAction::Create(choice) => self.create_config_with_choice(choice),
        }
    }

    fn render_create_instance_modal(&mut self, ctx: &egui::Context) {
        if !self.show_create_instance_modal {
            return;
        }
        if self.create_instance_state.create_in_flight {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        match create_instance_modal::render(
            ctx,
            &mut self.text_ui,
            &mut self.create_instance_state,
            installation::VersionCatalogFilter {
                include_snapshots_and_betas: self.config.include_snapshots_and_betas(),
                include_alpha: self.config.include_alpha_versions(),
                include_experimental: self.config.include_experimental_versions(),
            },
        ) {
            create_instance_modal::ModalAction::None => {}
            create_instance_modal::ModalAction::Cancel => {
                self.show_create_instance_modal = false;
                self.create_instance_state.reset();
            }
            create_instance_modal::ModalAction::Create(draft) => {
                start_create_instance_task(self, draft);
            }
        }
    }

    fn render_import_instance_modal(&mut self, ctx: &egui::Context) {
        if !self.show_import_instance_modal || self.pending_curseforge_manual_download.is_some() {
            return;
        }
        if self.import_instance_state.import_in_flight {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        match import_instance_modal::render(
            ctx,
            &mut self.text_ui,
            &mut self.import_instance_state,
            !self.config.curseforge_api_key().trim().is_empty(),
        ) {
            import_instance_modal::ModalAction::None => {}
            import_instance_modal::ModalAction::Cancel => {
                self.show_import_instance_modal = false;
                self.import_instance_state.reset();
            }
            import_instance_modal::ModalAction::Import(request) => {
                start_import_instance_task(self, request);
            }
        }
    }

    fn render_gamepad_calibration_modal(&mut self, ctx: &egui::Context) {
        if !self.show_gamepad_calibration_modal {
            return;
        }
        let live_sample = self.gamepad.as_ref().and_then(|gamepad| {
            self.gamepad_calibration_state
                .device_key()
                .and_then(|device_key| gamepad.current_left_stick(device_key))
        });
        match gamepad_calibration_modal::render(
            ctx,
            &mut self.text_ui,
            &mut self.gamepad_calibration_state,
            live_sample,
        ) {
            gamepad_calibration_modal::ModalAction::None => {}
            gamepad_calibration_modal::ModalAction::Cancel => {
                self.show_gamepad_calibration_modal = false;
                self.gamepad_calibration_state.reset();
            }
            gamepad_calibration_modal::ModalAction::Save {
                device_key,
                calibration,
            } => {
                self.config
                    .set_gamepad_calibration(device_key.clone(), calibration);
                if let Some(gamepad) = self.gamepad.as_mut() {
                    gamepad.reset_navigation_state(device_key.as_str());
                }
                self.show_gamepad_calibration_modal = false;
                self.gamepad_calibration_state.reset();
                notification::info!("gamepad", "Saved gamepad calibration.");
            }
        }
    }

    fn render_curseforge_manual_download_modal(&mut self, ctx: &egui::Context) {
        let Some(pending) = self
            .pending_curseforge_manual_download
            .as_mut()
            .filter(|pending| !pending.pending_files.is_empty())
        else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(200));
        match render_curseforge_manual_download_modal(ctx, &mut self.text_ui, pending) {
            ManualCurseForgeDownloadAction::None => {}
            ManualCurseForgeDownloadAction::Cancel => {
                cancel_pending_curseforge_manual_download(self);
            }
            ManualCurseForgeDownloadAction::OpenDownloadsFolder => {
                if let Err(err) =
                    launcher_ui::desktop::open_in_file_manager(pending.downloads_dir.as_path())
                {
                    pending.error = Some(format!("Failed to open downloads folder: {err}"));
                }
            }
        }
    }

    /// Normalizes the config and, if it differs from `previous`, queues a save and re-applies fonts.
    pub(super) fn commit_config_changes(&mut self, ctx: &egui::Context, previous: &Config) {
        self.config.normalize();
        self.fonts
            .ensure_selected_font_is_available(&mut self.config);
        if self.config != *previous {
            if transparent_viewport_enabled(&self.config) != transparent_viewport_enabled(previous)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(
                    transparent_viewport_enabled(&self.config),
                ));
            }
            queue_config_save(self);
            self.fonts
                .apply_from_config(ctx, &self.config, &mut self.text_ui);
        }
    }

    /// Normalizes the instance store and, if it differs from `previous`, fixes the selection,
    /// refreshes sidebar shortcuts and queues a save.
    pub(super) fn commit_instance_store_changes(&mut self, previous: &InstanceStore) {
        self.instance_store.normalize();
        if self.instance_store != *previous {
            if self
                .selected_instance_id
                .as_deref()
                .is_some_and(|id| self.instance_store.find(id).is_none())
            {
                self.selected_instance_id =
                    self.instance_store.instances.first().map(|i| i.id.clone());
                if self.selected_instance_id.is_none()
                    && self.active_screen == screens::AppScreen::Instance
                {
                    self.active_screen = screens::AppScreen::Library;
                }
            }
            self.refresh_instance_shortcuts();
            queue_instance_store_save(self);
        }
    }

    /// Logs navigation changes and frees screenshot/instance state of the screen just left.
    pub(super) fn finish_navigation(
        &mut self,
        ctx: &egui::Context,
        previous_active_screen: screens::AppScreen,
        previous_selected_instance_id: Option<&str>,
        menu_presence_context: screens::MenuPresenceContext,
    ) {
        if self.active_screen != previous_active_screen
            || self.selected_instance_id.as_deref() != previous_selected_instance_id
        {
            tracing::info!(
                target: "vertexlauncher/navigation",
                from_screen = ?previous_active_screen,
                to_screen = ?self.active_screen,
                previous_selected_instance_id = previous_selected_instance_id.unwrap_or(""),
                selected_instance_id = self.selected_instance_id.as_deref().unwrap_or(""),
                menu_presence_context = ?menu_presence_context,
                "Launcher navigation state changed."
            );
        }

        if previous_active_screen == screens::AppScreen::Home
            && self.active_screen != screens::AppScreen::Home
        {
            screens::purge_home_screenshot_state(ctx);
        }
        if previous_active_screen == screens::AppScreen::Instance
            && (self.active_screen != screens::AppScreen::Instance
                || self.selected_instance_id.as_deref() != previous_selected_instance_id)
        {
            screens::purge_instance_screenshot_state(ctx, previous_selected_instance_id);
            // Also drop the rest of that instance's screen state (content lists, caches, ...);
            // the inactivity purge only ever sees the currently selected instance.
            screens::purge_inactive_instance_state(ctx, previous_selected_instance_id);
        }
    }
}
