use super::*;

const LINK_PROFILE_DIALOG: &str = "link_existing_profile";

impl VertexApp {
    /// Asks the user for a profile folder to link. The dialog runs without blocking the UI;
    /// [`Self::poll_link_existing_profile`] continues once it closes.
    pub(super) fn link_existing_profile_folder(&mut self, ctx: &egui::Context) {
        launcher_ui::ui::file_dialog::open(
            ctx,
            LINK_PROFILE_DIALOG,
            launcher_ui::ui::file_dialog::Pick::Folder,
            launcher_ui::ui::file_dialog::Dialog::new().title("Link Existing Vertex Profile"),
        );
    }

    /// Advances linking: picked folder -> version detection on a worker -> registration.
    pub(super) fn poll_link_existing_profile(&mut self, ctx: &egui::Context) {
        if let Some(path) = launcher_ui::ui::file_dialog::take(ctx, LINK_PROFILE_DIALOG)
            .and_then(|paths| paths.into_iter().next())
        {
            if self.instance_store.instances.iter().any(|instance| {
                instance.instance_root_override.as_deref() == Some(path.as_path())
                    || instance_root_path(self.config.minecraft_installations_root_path(), instance)
                        == path
            }) {
                notification::warn!(
                    "instance_link",
                    "That profile folder is already registered in Vertex."
                );
            } else {
                // Reading the profile's version files is filesystem work; keep it off the UI thread.
                let tx = self.link_profile_results.sender();
                let ctx = ctx.clone();
                tokio_runtime::spawn_blocking_detached(move || {
                    let detected = screens::detect_instance_versions(path.as_path())
                        .map_err(|err| err.to_string());
                    let _ = tx.send((path, detected));
                    ctx.request_repaint();
                });
            }
        }
        for (path, detected) in self.link_profile_results.drain().items {
            self.finish_link_existing_profile(path, detected);
        }
    }

    fn finish_link_existing_profile(
        &mut self,
        path: PathBuf,
        detect_result: Result<screens::DetectedInstanceVersions, String>,
    ) {
        let fallback_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Linked Profile")
            .to_owned();
        let detected_versions = match detect_result {
            Ok(detected) => Some(detected),
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/instances",
                    root = %path.display(),
                    error = %err,
                    "Failed to auto-detect linked instance versions; using fallback metadata."
                );
                notification::warn!(
                    "instance_link",
                    "Linked profile version detection failed; using fallback version metadata. You can run auto-detect from Instance Settings."
                );
                None
            }
        };
        let detected_summary = detected_versions.as_ref().map(|detected| {
            if detected.modloader_version.trim().is_empty() {
                format!(
                    "{} for Minecraft {}",
                    detected.modloader, detected.game_version
                )
            } else {
                format!(
                    "{} {} for Minecraft {}",
                    detected.modloader, detected.modloader_version, detected.game_version
                )
            }
        });
        let (modloader, game_version, modloader_version) = detected_versions
            .map(|detected| {
                (
                    detected.modloader,
                    detected.game_version,
                    detected.modloader_version,
                )
            })
            .unwrap_or_else(|| ("Vanilla".to_owned(), "latest".to_owned(), String::new()));
        let installations_root = self
            .config
            .minecraft_installations_root_path()
            .to_path_buf();
        match link_instance_root(
            &mut self.instance_store,
            installations_root.as_path(),
            LinkedInstanceSpec {
                name: fallback_name,
                root_path: path.clone(),
                description: Some(format!(
                    "Linked existing profile at {}.",
                    display_user_path(path.as_path())
                )),
                thumbnail_path: None,
                modloader,
                game_version,
                modloader_version,
            },
        ) {
            Ok(instance) => {
                self.selected_instance_id = Some(instance.id.clone());
                self.active_screen = screens::AppScreen::Instance;
                self.refresh_instance_shortcuts();
                queue_instance_store_save(self);
                if let Some(summary) = detected_summary.as_deref() {
                    notification::info!(
                        "instance_link",
                        "Linked existing profile '{}' and detected {summary}.",
                        instance.name
                    );
                } else {
                    notification::info!(
                        "instance_link",
                        "Linked existing profile '{}'. You can adjust Minecraft/version settings from the profile screen.",
                        instance.name
                    );
                }
            }
            Err(err) => {
                notification::error!(
                    "instance_link",
                    "Failed to link existing profile folder: {err}"
                );
            }
        }
    }
}
