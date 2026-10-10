use super::*;

pub(in crate::screens::content_browser) fn open_detail_page(
    state: &mut ContentBrowserState,
    entry: &BrowserProjectEntry,
) {
    let same_entry = state
        .detail_entry
        .as_ref()
        .is_some_and(|current| current.dedupe_key == entry.dedupe_key);
    state.current_view = ContentBrowserPage::Detail;
    if !same_entry {
        state.detail_entry = Some(entry.clone());
        state.detail_tab = ContentDetailTab::Overview;
        state.detail_versions.clear();
        state.detail_versions_project_key = None;
        state.detail_versions_error = None;
        state.detail_versions_in_flight = false;
        state.detail_loader_filter = state.loader;
        state.detail_minecraft_version_filter = state.minecraft_version_filter.clone();
    }
    request_detail_versions(state);
}

pub(in crate::screens::content_browser) fn render_detail_page(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    instance_root: &Path,
    state: &mut ContentBrowserState,
) {
    let Some(entry) = state.detail_entry.clone() else {
        let _ = text_ui.label(
            ui,
            ("content_browser_detail_missing", instance_id),
            "No content item selected.",
            &style::muted(ui),
        );
        return;
    };

    request_detail_versions(state);
    let manifest = super::cached_manifest_for_instance(state, instance_root);
    let installed_project =
        installed_project_for_entry(manifest.as_ref(), &entry).map(|(_, project)| project);

    egui::Frame::new()
        .fill(ui.visuals().widgets.inactive.bg_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                render_browser_thumbnail(
                    ui,
                    ("detail", instance_id, &entry.dedupe_key),
                    &entry,
                    96.0,
                );
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    let _ = text_ui.label(
                        ui,
                        (
                            "content_browser_detail_title",
                            instance_id,
                            &entry.dedupe_key,
                        ),
                        entry.name.as_str(),
                        &LabelOptions {
                            wrap: true,
                            ..style::subtitle(ui)
                        },
                    );
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        render_chip(
                            ui,
                            text_ui,
                            ("detail-type", instance_id, &entry.dedupe_key),
                            entry.content_type.label(),
                        );
                        for source in &entry.sources {
                            render_chip(
                                ui,
                                text_ui,
                                (
                                    "detail-source",
                                    instance_id,
                                    &entry.dedupe_key,
                                    source.label(),
                                ),
                                source.label(),
                            );
                        }
                        if let Some(installed) = installed_project {
                            let installed_label = installed
                                .selected_version_name
                                .as_deref()
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or("Installed");
                            render_chip(
                                ui,
                                text_ui,
                                ("detail-installed", instance_id, &entry.dedupe_key),
                                installed_label,
                            );
                        }
                        ui.add_space(style::SPACE_XS);
                    });
                });
            });
        });

    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        let tab_style = style::neutral_button_with_min_size(ui, egui::vec2(96.0, 30.0));
        for (tab, label) in [
            (ContentDetailTab::Overview, "Overview"),
            (ContentDetailTab::Versions, "Versions"),
        ] {
            let selected = state.detail_tab == tab;
            if text_ui
                .selectable_button(
                    ui,
                    (
                        "content_browser_detail_tab",
                        instance_id,
                        &entry.dedupe_key,
                        label,
                    ),
                    label,
                    selected,
                    &tab_style,
                )
                .clicked()
            {
                state.detail_tab = tab;
            }
        }
    });
    ui.add_space(10.0);

    match state.detail_tab {
        ContentDetailTab::Overview => {
            egui::Frame::new()
                .fill(ui.visuals().widgets.noninteractive.bg_fill)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .scroll_source(modal_host::drag_scroll_source())
                        .id_salt((
                            "content_browser_detail_overview",
                            instance_id,
                            &entry.dedupe_key,
                        ))
                        .max_height(ui.available_height().max(180.0))
                        .show(ui, |ui| {
                            let body = if entry.summary.trim().is_empty() {
                                "No description provided."
                            } else {
                                entry.summary.trim()
                            };
                            let _ = text_ui.label(
                                ui,
                                (
                                    "content_browser_detail_body",
                                    instance_id,
                                    &entry.dedupe_key,
                                ),
                                body,
                                &style::body(ui),
                            );
                        });
                });
        }
        ContentDetailTab::Versions => {
            render_detail_versions_tab(ui, text_ui, instance_id, state, &entry, &manifest);
        }
    }
}

fn render_detail_versions_tab(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    state: &mut ContentBrowserState,
    entry: &BrowserProjectEntry,
    manifest: &ContentInstallManifest,
) {
    egui::Frame::new()
        .fill(ui.visuals().widgets.noninteractive.bg_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            let loader_options = BrowserLoader::ALL.map(BrowserLoader::label);
            let mut detail_loader_index = BrowserLoader::ALL
                .iter()
                .position(|loader| *loader == state.detail_loader_filter)
                .unwrap_or(0);
            let detail_loader_response = settings_widgets::full_width_dropdown_row(
                text_ui,
                ui,
                ("detail_loader_filter", instance_id, &entry.dedupe_key),
                "Loader",
                None,
                &mut detail_loader_index,
                &loader_options,
            );
            if detail_loader_response.changed() {
                state.detail_loader_filter = BrowserLoader::ALL[detail_loader_index];
            }

            ui.add_space(style::SPACE_SM);
            let version_option_refs = state
                .game_version_options
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut selected_version_index = 0_usize;
            if !state.detail_minecraft_version_filter.trim().is_empty()
                && let Some(found_index) = state
                    .available_game_versions
                    .iter()
                    .position(|version| version.id == state.detail_minecraft_version_filter)
            {
                selected_version_index = found_index + 1;
            }
            let detail_version_response = settings_widgets::full_width_dropdown_row(
                text_ui,
                ui,
                (
                    "detail_minecraft_version_filter",
                    instance_id,
                    &entry.dedupe_key,
                ),
                "Minecraft Version",
                None,
                &mut selected_version_index,
                &version_option_refs,
            );
            if detail_version_response.changed() {
                state.detail_minecraft_version_filter = if selected_version_index == 0 {
                    String::new()
                } else {
                    state.available_game_versions[selected_version_index - 1]
                        .id
                        .clone()
                };
            }

            if let Some(error) = state.detail_versions_error.as_deref() {
                ui.add_space(8.0);
                let _ = text_ui.label(
                    ui,
                    ("detail_versions_error", instance_id, &entry.dedupe_key),
                    error,
                    &style::warning_text(ui),
                );
            }

            if state.detail_versions_in_flight {
                ui.add_space(8.0);
                let _ = text_ui.label(
                    ui,
                    ("detail_versions_loading", instance_id, &entry.dedupe_key),
                    "Loading versions...",
                    &style::muted(ui),
                );
            }

            ui.add_space(8.0);
            let filtered_versions: Vec<&BrowserVersionEntry> = state
                .detail_versions
                .iter()
                .filter(|version| version_matches_loader(version, state.detail_loader_filter))
                .filter(|version| {
                    version_matches_game_version(
                        version,
                        state.detail_minecraft_version_filter.as_str(),
                    )
                })
                .collect();

            if filtered_versions.is_empty() && !state.detail_versions_in_flight {
                let _ = text_ui.label(
                    ui,
                    ("detail_versions_empty", instance_id, &entry.dedupe_key),
                    "No versions match the current filters.",
                    &style::muted(ui),
                );
                return;
            }

            egui::ScrollArea::vertical()
                .scroll_source(modal_host::drag_scroll_source())
                .id_salt(("detail_versions_scroll", instance_id, &entry.dedupe_key))
                .max_height(ui.available_height().max(180.0))
                .show(ui, |ui| {
                    for version in filtered_versions {
                        let action = version_row_action(manifest, entry, version);
                        egui::Frame::new()
                            .fill(ui.visuals().widgets.inactive.bg_fill)
                            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin::same(8))
                            .show(ui, |ui| {
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(
                                            (ui.available_width() - TILE_ACTION_BUTTON_WIDTH - 8.0)
                                                .max(160.0),
                                            ui.available_height(),
                                        ),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            let _ = text_ui.label(
                                                ui,
                                                (
                                                    "detail_version_name",
                                                    instance_id,
                                                    &entry.dedupe_key,
                                                    &version.version_id,
                                                ),
                                                version.version_name.as_str(),
                                                &LabelOptions {
                                                    ..style::stat_label(ui)
                                                },
                                            );
                                            ui.add_space(2.0);
                                            ui.horizontal_wrapped(|ui| {
                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                render_chip(
                                                    ui,
                                                    text_ui,
                                                    (
                                                        "detail_version_source",
                                                        instance_id,
                                                        &entry.dedupe_key,
                                                        &version.version_id,
                                                    ),
                                                    version.source.label(),
                                                );
                                                for loader in &version.loaders {
                                                    render_chip(
                                                        ui,
                                                        text_ui,
                                                        (
                                                            "detail_version_loader",
                                                            instance_id,
                                                            &entry.dedupe_key,
                                                            &version.version_id,
                                                            loader,
                                                        ),
                                                        loader.as_str(),
                                                    );
                                                }
                                                for game_version in
                                                    version.game_versions.iter().take(3)
                                                {
                                                    render_chip(
                                                        ui,
                                                        text_ui,
                                                        (
                                                            "detail_version_mc",
                                                            instance_id,
                                                            &entry.dedupe_key,
                                                            &version.version_id,
                                                            game_version,
                                                        ),
                                                        game_version.as_str(),
                                                    );
                                                }
                                            });
                                            ui.add_space(4.0);
                                            let _ = text_ui.label(
                                                ui,
                                                (
                                                    "detail_version_file",
                                                    instance_id,
                                                    &entry.dedupe_key,
                                                    &version.version_id,
                                                ),
                                                &format!(
                                                    "{} | {}",
                                                    version.file_name, version.published_at
                                                ),
                                                &style::muted(ui),
                                            );
                                        },
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Min),
                                        |ui| {
                                            let (icon, tooltip, fill, enabled) = match action {
                                                VersionRowAction::Installed => (
                                                    assets::CHECK_SVG,
                                                    "Installed version",
                                                    ui.visuals().selection.bg_fill,
                                                    false,
                                                ),
                                                VersionRowAction::Switch => (
                                                    assets::REFRESH_SVG,
                                                    "Switch to this version",
                                                    ui.visuals().warn_fg_color,
                                                    true,
                                                ),
                                                VersionRowAction::Download => (
                                                    assets::DOWNLOAD_SVG,
                                                    "Install this version",
                                                    ui.visuals().selection.bg_fill,
                                                    true,
                                                ),
                                            };
                                            let mut hasher =
                                                std::collections::hash_map::DefaultHasher::new();
                                            (
                                                &entry.dedupe_key,
                                                &version.version_id,
                                                "detail_version_action",
                                            )
                                                .hash(&mut hasher);
                                            let action_id = format!(
                                                "detail-version-action-{}",
                                                hasher.finish()
                                            );
                                            if render_rounded_icon_button(
                                                ui,
                                                action_id.as_str(),
                                                icon,
                                                tooltip,
                                                fill,
                                                TILE_ACTION_BUTTON_WIDTH,
                                                TILE_ACTION_BUTTON_HEIGHT,
                                                enabled,
                                            )
                                            .clicked
                                                && enabled
                                            {
                                                let requested_game_version = if state
                                                    .detail_minecraft_version_filter
                                                    .trim()
                                                    .is_empty()
                                                {
                                                    state.minecraft_version_filter.clone()
                                                } else {
                                                    state.detail_minecraft_version_filter.clone()
                                                };
                                                state.download_queue.push_back(
                                                    QueuedContentDownload {
                                                        request: ContentInstallRequest::Exact {
                                                            entry: entry.clone(),
                                                            version: version.clone(),
                                                            game_version: requested_game_version,
                                                            loader: state.detail_loader_filter,
                                                        },
                                                    },
                                                );
                                                state.status_message = Some(format!(
                                                    "Queued {} {}.",
                                                    match action {
                                                        VersionRowAction::Switch => "switch for",
                                                        VersionRowAction::Installed => "installed",
                                                        VersionRowAction::Download => "install for",
                                                    },
                                                    entry.name
                                                ));
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(8.0);
                    }
                });
        });
}

fn version_row_action(
    manifest: &ContentInstallManifest,
    entry: &BrowserProjectEntry,
    version: &BrowserVersionEntry,
) -> VersionRowAction {
    let Some((_, installed)) = installed_project_for_entry(manifest, entry) else {
        return VersionRowAction::Download;
    };
    if installed.selected_source == Some(version.source.into())
        && installed.selected_version_id.as_deref() == Some(version.version_id.as_str())
    {
        VersionRowAction::Installed
    } else {
        VersionRowAction::Switch
    }
}
