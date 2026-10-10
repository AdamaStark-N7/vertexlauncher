use super::*;

pub(in crate::screens::content_browser) fn render_controls(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    state: &mut ContentBrowserState,
) {
    let frame = egui::Frame::new()
        .fill(ui.visuals().widgets.inactive.bg_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(style::SPACE_XL as i8));
    frame.show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(style::SPACE_SM, style::SPACE_MD);
        let response = themed_text_input(
            text_ui,
            ui,
            ("content_browser_query", instance_id),
            &mut state.query_input,
            InputOptions {
                desired_width: Some(ui.available_width().max(160.0)),
                placeholder_text: Some(
                    "Search project names, summaries, and tags. Press Enter to search".to_owned(),
                ),
                ..crate::ui::style::input_options(ui)
            },
        );
        let enter_pressed = ui.input(|input| input.key_pressed(egui::Key::Enter));
        let submit_pressed = enter_pressed && (response.has_focus() || response.lost_focus());
        if submit_pressed {
            if ui.input(|input| input.modifiers.shift) {
                if add_search_tag(&mut state.search_tags, state.query_input.as_str()) {
                    state.query_input.clear();
                    if !state.search_in_flight {
                        request_search_for_current_filters(state, true);
                    }
                }
            } else if !state.search_in_flight {
                request_search_for_current_filters(state, true);
            }
        }

        if !state.search_tags.is_empty() {
            ui.add_space(style::SPACE_SM);
            if render_search_tag_chips(ui, text_ui, &mut state.search_tags, SEARCH_TAG_CHIP_IDS) {
                request_search_for_current_filters(state, true);
            }
        }
        ui.add_space(style::SPACE_MD);
        let gap = style::SPACE_MD;
        let column_width = ((ui.available_width() - gap * 3.0) / 4.0).max(1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;

            ui.allocate_ui_with_layout(
                egui::vec2(column_width, style::CONTROL_HEIGHT),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let version_option_refs = state
                        .game_version_options
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    let mut selected_version_index = 0_usize;
                    if !state.minecraft_version_filter.trim().is_empty()
                        && let Some(found_index) = state
                            .available_game_versions
                            .iter()
                            .position(|version| version.id == state.minecraft_version_filter)
                    {
                        selected_version_index = found_index + 1;
                    }
                    let response = settings_widgets::dropdown_picker(
                        text_ui,
                        ui,
                        ("content_browser_minecraft_version", instance_id),
                        &mut selected_version_index,
                        &version_option_refs,
                        Some(column_width),
                    );
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new(CONTENT_BROWSER_VERSION_DROPDOWN_ID_KEY),
                            response.id,
                        )
                    });
                    if response.changed() {
                        state.minecraft_version_filter = if selected_version_index == 0 {
                            String::new()
                        } else {
                            state.available_game_versions[selected_version_index - 1]
                                .id
                                .clone()
                        };
                        request_search_for_current_filters(state, true);
                    }
                },
            );

            if let Some(scope) = render_enum_filter_column(
                ui,
                text_ui,
                column_width,
                ("content_browser_scope", instance_id),
                CONTENT_BROWSER_SCOPE_DROPDOWN_ID_KEY,
                &ContentScope::ALL,
                &ContentScope::ALL.map(ContentScope::label),
                state.content_scope,
            ) {
                state.content_scope = scope;
                request_search_for_current_filters(state, true);
            }

            if let Some(mode) = render_enum_filter_column(
                ui,
                text_ui,
                column_width,
                ("content_browser_mod_sort", instance_id),
                CONTENT_BROWSER_SORT_DROPDOWN_ID_KEY,
                &ModSortMode::ALL,
                &ModSortMode::ALL.map(ModSortMode::label),
                state.mod_sort_mode,
            ) {
                state.mod_sort_mode = mode;
                request_search_for_current_filters(state, true);
            }

            if let Some(loader) = render_enum_filter_column(
                ui,
                text_ui,
                column_width,
                ("content_browser_loader", instance_id),
                CONTENT_BROWSER_LOADER_DROPDOWN_ID_KEY,
                &BrowserLoader::ALL,
                &BrowserLoader::ALL.map(BrowserLoader::label),
                state.loader,
            ) {
                state.loader = loader;
                request_search_for_current_filters(state, true);
            }
        });

        ui.add_space(style::SPACE_MD);
        let identify_response = ui.add_enabled_ui(!state.identify_in_flight, |ui| {
            settings_widgets::full_width_button(
                text_ui,
                ui,
                ("content_browser_identify_file_button", instance_id),
                "Identify Content File",
                ui.available_width(),
                false,
            )
        });
        let identify_slot = ("content_browser_identify_dialog", instance_id);
        if identify_response.inner.clicked() {
            crate::ui::file_dialog::open(
                ui.ctx(),
                identify_slot,
                crate::ui::file_dialog::Pick::File,
                crate::ui::file_dialog::Dialog::new()
                    .title("Identify Content File")
                    .filter("Minecraft Content", &["jar", "zip"])
                    .filter("Mods", &["jar"])
                    .filter("Packs", &["zip"]),
            );
        }
        if let Some(selected_path) = crate::ui::file_dialog::take(ui.ctx(), identify_slot)
            .and_then(|paths| paths.into_iter().next())
        {
            request_identify_file(state, selected_path);
        }

        let queue_status = if state.download_in_flight {
            format!("Downloads: active, {} queued", state.download_queue.len())
        } else if state.download_queue.is_empty() {
            "Downloads: idle".to_owned()
        } else {
            format!("Downloads: idle, {} queued", state.download_queue.len())
        };
        let _ = text_ui.label(
            ui,
            ("content_browser_queue", instance_id),
            queue_status.as_str(),
            &style::muted(ui),
        );
    });
}

/// One fixed-width dropdown column over an enum's `ALL` list. Publishes the dropdown's widget id
/// under `id_key` (for gamepad navigation) and returns the newly chosen value when it changed.
fn render_enum_filter_column<T: Copy + PartialEq>(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    column_width: f32,
    id_source: impl std::hash::Hash,
    id_key: &'static str,
    all: &[T],
    labels: &[&str],
    current: T,
) -> Option<T> {
    ui.allocate_ui_with_layout(
        egui::vec2(column_width, style::CONTROL_HEIGHT),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            let mut selected_index = all.iter().position(|value| *value == current).unwrap_or(0);
            let response = settings_widgets::dropdown_picker(
                text_ui,
                ui,
                id_source,
                &mut selected_index,
                labels,
                Some(column_width),
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new(id_key), response.id));
            response.changed().then(|| all[selected_index])
        },
    )
    .inner
}
