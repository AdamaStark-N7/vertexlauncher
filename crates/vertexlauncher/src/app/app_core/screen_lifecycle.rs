use super::*;

impl VertexApp {
    /// Releases every cached image that the current screen did not draw this frame, right after a
    /// navigation, so nothing from the previous screen lingers.
    pub(super) fn sweep_screen_resources(&mut self, ctx: &egui::Context) {
        if self.resource_sweep_pending {
            self.resource_sweep_pending = false;
            launcher_ui::ui::components::image_textures::release_untouched();
            textui_egui::release_untouched(ctx);
            self.text_ui.release_layout_caches();
            launcher_ui::ui::components::remote_tiled_image::release_untouched(ctx);
            // Drops egui's own decoded-image and bytes caches (icons, SVGs); they reload on demand.
            ctx.forget_all_images();
            // Hand the freed image buffers back to the OS instead of leaving them in the heap.
            launcher_runtime::spawn_blocking_detached(launcher_runtime::release_memory_to_os);
        }
        let scope = (
            self.active_screen,
            (self.active_screen == screens::AppScreen::Instance)
                .then(|| self.selected_instance_id.clone())
                .flatten(),
        );
        if scope != self.resource_scope {
            self.resource_scope = scope;
            self.resource_sweep_pending = true;
            ctx.request_repaint();
        }
        self.last_frame_screen = self.active_screen;
    }

    pub(super) fn update_discover_lifecycle(
        &mut self,
        ctx: &egui::Context,
        render_state: Option<&eframe::egui_wgpu::RenderState>,
    ) {
        let active = self.active_screen;
        let last = self.last_frame_screen;
        let screen_in_use = |screen: screens::AppScreen| active == screen || last == screen;
        update_screen_purge_timer(
            ctx,
            &mut self.home_purge_timer,
            screen_in_use(screens::AppScreen::Home),
            DISCOVER_STATE_PURGE_DELAY,
            || {
                screens::purge_inactive_home_state(ctx);
                tracing::info!(
                    target: "vertexlauncher/home",
                    "Purged inactive home state after timeout."
                );
            },
        );
        update_screen_purge_timer(
            ctx,
            &mut self.library_purge_timer,
            screen_in_use(screens::AppScreen::Library),
            DISCOVER_STATE_PURGE_DELAY,
            || {
                screens::purge_inactive_library_state(ctx);
                tracing::info!(
                    target: "vertexlauncher/library",
                    "Purged inactive library state after timeout."
                );
            },
        );
        update_screen_purge_timer(
            ctx,
            &mut self.instance_purge_timer,
            screen_in_use(screens::AppScreen::Instance),
            DISCOVER_STATE_PURGE_DELAY,
            || {
                screens::purge_inactive_instance_state(ctx, self.selected_instance_id.as_deref());
                tracing::info!(
                    target: "vertexlauncher/instance",
                    selected_instance_id = self.selected_instance_id.as_deref().unwrap_or(""),
                    "Purged inactive instance state after timeout."
                );
            },
        );
        update_screen_purge_timer(
            ctx,
            &mut self.discover_purge_timer,
            is_discover_screen(active) || is_discover_screen(last),
            DISCOVER_STATE_PURGE_DELAY,
            || {
                self.discover_state.purge_inactive_state();
                tracing::info!(
                    target: "vertexlauncher/discover",
                    "Purged inactive discover state after timeout."
                );
            },
        );
        update_screen_purge_timer(
            ctx,
            &mut self.content_browser_purge_timer,
            screen_in_use(screens::AppScreen::ContentBrowser),
            DISCOVER_STATE_PURGE_DELAY,
            || {
                self.content_browser_state.purge_inactive_state();
                tracing::info!(
                    target: "vertexlauncher/content_browser",
                    "Purged inactive content browser state after timeout."
                );
            },
        );
        update_screen_purge_timer(
            ctx,
            &mut self.skins_purge_timer,
            screen_in_use(screens::AppScreen::Skins),
            DISCOVER_STATE_PURGE_DELAY,
            || {
                screens::purge_inactive_skins_state(ctx);
                if let Some(render_state) = render_state {
                    screens::release_skins_gpu_resources(render_state);
                }
                tracing::info!(
                    target: "vertexlauncher/skins",
                    "Purged inactive skins state after timeout."
                );
            },
        );
    }
}
