use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::Duration,
};

use curseforge::{Client as CurseForgeClient, MINECRAFT_GAME_ID};
use egui::Ui;
use installation::{MinecraftVersionEntry, fetch_version_catalog};
use modrinth::Client as ModrinthClient;
use textui::{LabelOptions, TextUi};

use crate::{
    app::tokio_runtime,
    assets,
    ui::{components::remote_tiled_image, style},
};

const DISCOVER_PROVIDER_LIMIT: u32 = 36;
const DISCOVER_CARD_MIN_WIDTH: f32 = 260.0;
const DISCOVER_CARD_GAP: f32 = 12.0;
const DISCOVER_CARD_IMAGE_HEIGHT: f32 = 124.0;

#[derive(Debug, Clone)]
pub struct DiscoverState {
    query_input: String,
    game_version_filter: String,
    provider_filter: DiscoverProviderFilter,
    loader_filter: DiscoverLoaderFilter,
    sort_mode: DiscoverSortMode,
    page: u32,
    search_in_flight: bool,
    search_request_serial: u64,
    initial_search_requested: bool,
    status_message: Option<String>,
    warnings: Vec<String>,
    entries: Vec<DiscoverEntry>,
    has_more_results: bool,
    cached_snapshots: HashMap<DiscoverSearchRequest, DiscoverSearchSnapshot>,
    available_game_versions: Vec<MinecraftVersionEntry>,
    version_catalog_error: Option<String>,
    version_catalog_in_flight: bool,
    version_catalog_tx: Option<mpsc::Sender<Result<Vec<MinecraftVersionEntry>, String>>>,
    version_catalog_rx:
        Option<Arc<Mutex<mpsc::Receiver<Result<Vec<MinecraftVersionEntry>, String>>>>>,
    search_results_tx: Option<mpsc::Sender<DiscoverSearchResult>>,
    search_results_rx: Option<Arc<Mutex<mpsc::Receiver<DiscoverSearchResult>>>>,
}

impl Default for DiscoverState {
    fn default() -> Self {
        Self {
            query_input: String::new(),
            game_version_filter: String::new(),
            provider_filter: DiscoverProviderFilter::default(),
            loader_filter: DiscoverLoaderFilter::default(),
            sort_mode: DiscoverSortMode::default(),
            page: 1,
            search_in_flight: false,
            search_request_serial: 0,
            initial_search_requested: false,
            status_message: None,
            warnings: Vec::new(),
            entries: Vec::new(),
            has_more_results: true,
            cached_snapshots: HashMap::new(),
            available_game_versions: Vec::new(),
            version_catalog_error: None,
            version_catalog_in_flight: false,
            version_catalog_tx: None,
            version_catalog_rx: None,
            search_results_tx: None,
            search_results_rx: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum DiscoverProviderFilter {
    #[default]
    All,
    Modrinth,
    CurseForge,
}

impl DiscoverProviderFilter {
    const ALL: [Self; 3] = [Self::All, Self::Modrinth, Self::CurseForge];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All Sources",
            Self::Modrinth => "Modrinth",
            Self::CurseForge => "CurseForge",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum DiscoverLoaderFilter {
    #[default]
    Any,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

impl DiscoverLoaderFilter {
    const ALL: [Self; 5] = [
        Self::Any,
        Self::Fabric,
        Self::Forge,
        Self::NeoForge,
        Self::Quilt,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Any => "Any Loader",
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Quilt => "Quilt",
        }
    }

    fn modrinth_slug(self) -> Option<&'static str> {
        match self {
            Self::Any => None,
            Self::Fabric => Some("fabric"),
            Self::Forge => Some("forge"),
            Self::NeoForge => Some("neoforge"),
            Self::Quilt => Some("quilt"),
        }
    }

    fn curseforge_mod_loader_type(self) -> Option<u32> {
        match self {
            Self::Any => None,
            Self::Forge => Some(1),
            Self::Fabric => Some(4),
            Self::Quilt => Some(5),
            Self::NeoForge => Some(6),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum DiscoverSortMode {
    #[default]
    Popularity,
    Relevance,
    LastUpdated,
}

impl DiscoverSortMode {
    const ALL: [Self; 3] = [Self::Popularity, Self::Relevance, Self::LastUpdated];

    fn label(self) -> &'static str {
        match self {
            Self::Popularity => "Popularity",
            Self::Relevance => "Relevance",
            Self::LastUpdated => "Last Updated",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DiscoverSource {
    Modrinth,
    CurseForge,
}

impl DiscoverSource {
    fn label(self) -> &'static str {
        match self {
            Self::Modrinth => "Modrinth",
            Self::CurseForge => "CurseForge",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiscoverSearchRequest {
    query: String,
    game_version: Option<String>,
    provider_filter: DiscoverProviderFilter,
    loader_filter: DiscoverLoaderFilter,
    sort_mode: DiscoverSortMode,
    page: u32,
}

#[derive(Clone, Debug)]
struct DiscoverSearchResult {
    request_serial: u64,
    request: DiscoverSearchRequest,
    outcome: Result<DiscoverSearchSnapshot, String>,
}

#[derive(Clone, Debug, Default)]
struct DiscoverSearchSnapshot {
    entries: Vec<DiscoverEntry>,
    warnings: Vec<String>,
    has_more: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchMode {
    Replace,
    Append,
}

#[derive(Clone, Debug)]
struct DiscoverEntry {
    dedupe_key: String,
    name: String,
    summary: String,
    author: Option<String>,
    icon_url: Option<String>,
    primary_url: Option<String>,
    sources: Vec<DiscoverSource>,
    popularity_score: Option<u64>,
    updated_at: Option<String>,
    relevance_rank: u32,
}

#[derive(Clone, Debug)]
struct DiscoverProviderEntry {
    name: String,
    summary: String,
    author: Option<String>,
    icon_url: Option<String>,
    primary_url: Option<String>,
    source: DiscoverSource,
    popularity_score: Option<u64>,
    updated_at: Option<String>,
    relevance_rank: u32,
}

pub fn render(ui: &mut Ui, text_ui: &mut TextUi, state: &mut DiscoverState) {
    let full_width = ui.available_width().max(1.0);
    let full_height = ui.available_height().max(1.0);
    ui.horizontal(|ui| {
        ui.add_space(style::SPACE_XS);
        ui.allocate_ui_with_layout(
            egui::vec2((full_width - style::SPACE_XS * 2.0).max(1.0), full_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_discover_content(ui, text_ui, state),
        );
        ui.add_space(style::SPACE_XS);
    });
}

fn render_discover_content(ui: &mut Ui, text_ui: &mut TextUi, state: &mut DiscoverState) {
    poll_version_catalog(state);
    request_version_catalog(state);
    poll_search_results(state);
    if !state.initial_search_requested {
        state.initial_search_requested = true;
        request_search(state, false, SearchMode::Replace);
    }
    if state.search_in_flight {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }

    let muted_style = style::muted(ui);
    let warning_style = LabelOptions {
        color: ui.visuals().warn_fg_color,
        wrap: true,
        ..LabelOptions::default()
    };

    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_MD))
        .inner_margin(egui::Margin::same(style::SPACE_MD as i8))
        .show(ui, |ui| {
            let old_provider_filter = state.provider_filter;
            let old_loader_filter = state.loader_filter;
            let old_sort_mode = state.sort_mode;
            let old_game_version_filter = state.game_version_filter.clone();
            let search_response = ui.add_sized(
                egui::vec2(ui.available_width(), style::CONTROL_HEIGHT),
                egui::TextEdit::singleline(&mut state.query_input)
                    .hint_text("Search modpacks and press Enter"),
            );
            let search_submitted = search_response.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.add_space(style::SPACE_SM);

            let dropdown_width =
                ((ui.available_width() - (DISCOVER_CARD_GAP * 3.0)) / 4.0).max(120.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = DISCOVER_CARD_GAP;
                sized_combo_box(
                    ui,
                    "discover_provider_filter",
                    dropdown_width,
                    state.provider_filter.label(),
                    |ui| {
                        for provider in DiscoverProviderFilter::ALL {
                            ui.selectable_value(
                                &mut state.provider_filter,
                                provider,
                                provider.label(),
                            );
                        }
                    },
                );
                sized_combo_box(
                    ui,
                    "discover_loader_filter",
                    dropdown_width,
                    state.loader_filter.label(),
                    |ui| {
                        for loader in DiscoverLoaderFilter::ALL {
                            ui.selectable_value(&mut state.loader_filter, loader, loader.label());
                        }
                    },
                );
                sized_combo_box(
                    ui,
                    "discover_sort_mode",
                    dropdown_width,
                    state.sort_mode.label(),
                    |ui| {
                        for sort_mode in DiscoverSortMode::ALL {
                            ui.selectable_value(&mut state.sort_mode, sort_mode, sort_mode.label());
                        }
                    },
                );
                let selected_game_version = selected_game_version_label(
                    state.game_version_filter.as_str(),
                    &state.available_game_versions,
                );
                sized_combo_box(
                    ui,
                    "discover_game_version",
                    dropdown_width,
                    selected_game_version.as_str(),
                    |ui| {
                        ui.selectable_value(
                            &mut state.game_version_filter,
                            String::new(),
                            "Any version",
                        );
                        for version in &state.available_game_versions {
                            ui.selectable_value(
                                &mut state.game_version_filter,
                                version.id.clone(),
                                version.display_label(),
                            );
                        }
                    },
                );
            });
            let filters_changed = state.provider_filter != old_provider_filter
                || state.loader_filter != old_loader_filter
                || state.sort_mode != old_sort_mode
                || state.game_version_filter != old_game_version_filter;
            if search_submitted || filters_changed {
                request_search(state, true, SearchMode::Replace);
            }
        });

    ui.add_space(style::SPACE_MD);
    if let Some(status) = state.status_message.as_deref() {
        let _ = text_ui.label(ui, "discover_status", status, &muted_style);
    }
    for warning in &state.warnings {
        let _ = text_ui.label(ui, ("discover_warning", warning), warning, &warning_style);
    }

    if state.search_in_flight {
        ui.add_space(style::SPACE_SM);
        ui.horizontal(|ui| {
            ui.spinner();
            let _ = text_ui.label(
                ui,
                "discover_search_in_flight",
                "Loading modpacks...",
                &muted_style,
            );
        });
    }

    ui.add_space(style::SPACE_MD);
    let mut should_load_more = false;
    let results_height = ui.available_height().max(1.0);
    egui::ScrollArea::vertical()
        .id_salt("discover_results_scroll")
        .auto_shrink([false, false])
        .max_height(results_height)
        .show_viewport(ui, |ui, viewport| {
            if state.entries.is_empty() && !state.search_in_flight {
                let _ = text_ui.label(
                    ui,
                    "discover_empty",
                    "No modpacks matched the current search and filters.",
                    &muted_style,
                );
                return;
            }
            render_masonry_tiles(ui, text_ui, state.entries.as_slice());
            let content_bottom = ui.min_rect().bottom();
            should_load_more = state.has_more_results
                && !state.search_in_flight
                && viewport.bottom() >= content_bottom - 320.0;
        });

    if should_load_more {
        request_search(state, false, SearchMode::Append);
    }
}

fn render_masonry_tiles(ui: &mut Ui, text_ui: &mut TextUi, entries: &[DiscoverEntry]) {
    let content_width = ui.available_width().max(DISCOVER_CARD_MIN_WIDTH);
    let mut column_count = 1usize;
    for candidate in 1..=4usize {
        let required_width = (DISCOVER_CARD_MIN_WIDTH * candidate as f32)
            + (DISCOVER_CARD_GAP * candidate.saturating_sub(1) as f32);
        if required_width <= content_width {
            column_count = candidate;
        }
    }
    let column_width = (content_width
        - (DISCOVER_CARD_GAP * column_count.saturating_sub(1) as f32))
        / column_count as f32;
    let mut columns = vec![Vec::<usize>::new(); column_count];
    let mut heights = vec![0.0f32; column_count];

    for (index, entry) in entries.iter().enumerate() {
        let summary_lines = (entry.summary.len() as f32 / 46.0).ceil().clamp(2.0, 6.0);
        let estimated_height = 210.0 + (summary_lines * 18.0);
        let target_column = heights
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(index, _)| index)
            .unwrap_or(0);
        columns[target_column].push(index);
        heights[target_column] += estimated_height + DISCOVER_CARD_GAP;
    }

    ui.allocate_ui_with_layout(
        egui::vec2(content_width, 0.0),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.x = DISCOVER_CARD_GAP;
            for column_entries in &columns {
                ui.allocate_ui_with_layout(
                    egui::vec2(column_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(column_width);
                        for (row_index, entry_index) in column_entries.iter().enumerate() {
                            render_discover_tile(ui, text_ui, &entries[*entry_index]);
                            if row_index + 1 < column_entries.len() {
                                ui.add_space(DISCOVER_CARD_GAP);
                            }
                        }
                    },
                );
            }
        },
    );
}

fn render_discover_tile(ui: &mut Ui, text_ui: &mut TextUi, entry: &DiscoverEntry) {
    let heading_style = LabelOptions {
        font_size: 20.0,
        line_height: 24.0,
        weight: 700,
        color: ui.visuals().text_color(),
        wrap: true,
        ..LabelOptions::default()
    };
    let body_style = style::body(ui);
    let muted_style = style::muted(ui);
    let badge_fill = ui.visuals().widgets.inactive.weak_bg_fill;
    let badge_stroke = ui.visuals().widgets.inactive.bg_stroke;

    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_MD))
        .inner_margin(egui::Margin::same(style::SPACE_MD as i8))
        .show(ui, |ui| {
            if let Some(icon_url) = entry.icon_url.as_deref() {
                remote_tiled_image::show(
                    ui,
                    icon_url,
                    egui::vec2(ui.available_width(), DISCOVER_CARD_IMAGE_HEIGHT),
                    ("discover_tile_image", entry.dedupe_key.as_str()),
                    assets::DISCOVER_SVG,
                );
            } else {
                ui.add(
                    egui::Image::from_bytes(
                        format!("bytes://discover/placeholder/{}", entry.dedupe_key),
                        assets::DISCOVER_SVG.to_vec(),
                    )
                    .fit_to_exact_size(egui::vec2(
                        ui.available_width(),
                        DISCOVER_CARD_IMAGE_HEIGHT,
                    )),
                );
            }

            ui.add_space(style::SPACE_SM);
            let _ = text_ui.label(
                ui,
                ("discover_tile_name", entry.dedupe_key.as_str()),
                entry.name.as_str(),
                &heading_style,
            );
            if let Some(author) = entry
                .author
                .as_deref()
                .filter(|author| !author.trim().is_empty())
            {
                let _ = text_ui.label(
                    ui,
                    ("discover_tile_author", entry.dedupe_key.as_str()),
                    &format!("by {author}"),
                    &muted_style,
                );
            }
            ui.add_space(style::SPACE_XS);
            let _ = text_ui.label(
                ui,
                ("discover_tile_summary", entry.dedupe_key.as_str()),
                entry.summary.as_str(),
                &body_style,
            );

            ui.add_space(style::SPACE_SM);
            ui.horizontal_wrapped(|ui| {
                for source in &entry.sources {
                    egui::Frame::new()
                        .fill(badge_fill)
                        .stroke(badge_stroke)
                        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_SM))
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            let _ = text_ui.label(
                                ui,
                                (
                                    "discover_tile_source",
                                    entry.dedupe_key.as_str(),
                                    source.label(),
                                ),
                                source.label(),
                                &LabelOptions {
                                    wrap: false,
                                    color: ui.visuals().text_color(),
                                    ..LabelOptions::default()
                                },
                            );
                        });
                }
            });

            ui.add_space(style::SPACE_SM);
            if let Some(downloads) = entry.popularity_score {
                let _ = text_ui.label(
                    ui,
                    ("discover_tile_downloads", entry.dedupe_key.as_str()),
                    &format!("Downloads: {}", format_compact_number(downloads)),
                    &muted_style,
                );
            }
            if let Some(updated_at) = entry.updated_at.as_deref() {
                let _ = text_ui.label(
                    ui,
                    ("discover_tile_updated", entry.dedupe_key.as_str()),
                    &format!("Updated: {}", format_short_date(updated_at)),
                    &muted_style,
                );
            }
            if let Some(url) = entry.primary_url.as_deref() {
                ui.add_space(style::SPACE_XS);
                ui.hyperlink_to("Open project page", url);
            }
        });
}

fn ensure_search_channel(state: &mut DiscoverState) {
    if state.search_results_tx.is_some() && state.search_results_rx.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<DiscoverSearchResult>();
    state.search_results_tx = Some(tx);
    state.search_results_rx = Some(Arc::new(Mutex::new(rx)));
}

fn request_search(state: &mut DiscoverState, show_cached_status: bool, mode: SearchMode) {
    if state.search_in_flight {
        return;
    }
    ensure_search_channel(state);
    let request = current_request(state, mode);
    if let Some(snapshot) = state.cached_snapshots.get(&request).cloned() {
        apply_search_snapshot(state, &request, snapshot, mode);
        state.search_in_flight = false;
        if show_cached_status {
            state.status_message = Some("Loaded cached discover results.".to_owned());
        }
        return;
    }

    let Some(tx) = state.search_results_tx.as_ref().cloned() else {
        return;
    };
    state.search_request_serial = state.search_request_serial.saturating_add(1);
    let request_serial = state.search_request_serial;
    state.search_in_flight = true;
    if mode == SearchMode::Replace {
        state.page = 1;
        state.has_more_results = true;
    }
    state.status_message = Some(format!(
        "Searching {} for modpacks...",
        request.provider_filter.label()
    ));
    let request_for_task = request.clone();
    let _ = tokio_runtime::spawn_detached(async move {
        let outcome =
            tokio_runtime::spawn_blocking(move || perform_search(&request_for_task)).await;
        let result = match outcome {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => Err(format!("discover search task join error: {error}")),
        };
        let _ = tx.send(DiscoverSearchResult {
            request_serial,
            request,
            outcome: result,
        });
    });
}

fn poll_search_results(state: &mut DiscoverState) {
    let Some(rx) = state.search_results_rx.as_ref().cloned() else {
        return;
    };
    let Ok(receiver) = rx.lock() else {
        return;
    };
    while let Ok(result) = receiver.try_recv() {
        if result.request_serial != state.search_request_serial {
            continue;
        }
        state.search_in_flight = false;
        match result.outcome {
            Ok(snapshot) => {
                let mode = if result.request.page <= 1 {
                    SearchMode::Replace
                } else {
                    SearchMode::Append
                };
                apply_search_snapshot(state, &result.request, snapshot.clone(), mode);
                state.cached_snapshots.insert(result.request, snapshot);
                state.status_message = Some(format!("Showing {} modpacks.", state.entries.len()));
            }
            Err(error) => {
                state.status_message = Some(format!("Discover search failed: {error}"));
                state.entries.clear();
                state.warnings.clear();
            }
        }
    }
}

fn current_request(state: &DiscoverState, mode: SearchMode) -> DiscoverSearchRequest {
    DiscoverSearchRequest {
        query: {
            let trimmed = state.query_input.trim();
            if trimmed.is_empty() {
                "modpack".to_owned()
            } else {
                trimmed.to_owned()
            }
        },
        game_version: non_empty(state.game_version_filter.as_str()),
        provider_filter: state.provider_filter,
        loader_filter: state.loader_filter,
        sort_mode: state.sort_mode,
        page: match mode {
            SearchMode::Replace => 1,
            SearchMode::Append => state.page.saturating_add(1).max(1),
        },
    }
}

fn perform_search(request: &DiscoverSearchRequest) -> DiscoverSearchSnapshot {
    let modrinth = ModrinthClient::default();
    let curseforge = CurseForgeClient::from_env();
    let mut warnings = Vec::new();
    let offset = request
        .page
        .saturating_sub(1)
        .saturating_mul(DISCOVER_PROVIDER_LIMIT);
    let mut provider_entries = Vec::new();
    let mut provider_result_count = 0usize;

    if matches!(
        request.provider_filter,
        DiscoverProviderFilter::All | DiscoverProviderFilter::Modrinth
    ) {
        match modrinth.search_projects_with_filters(
            request.query.as_str(),
            DISCOVER_PROVIDER_LIMIT,
            offset,
            Some("modpack"),
            request.game_version.as_deref(),
            request.loader_filter.modrinth_slug(),
        ) {
            Ok(entries) => {
                provider_result_count += entries.len();
                provider_entries.extend(entries.into_iter().enumerate().map(|(index, entry)| {
                    DiscoverProviderEntry {
                        name: entry.title,
                        summary: entry.description,
                        author: entry.author,
                        icon_url: entry.icon_url,
                        primary_url: Some(entry.project_url),
                        source: DiscoverSource::Modrinth,
                        popularity_score: Some(entry.downloads),
                        updated_at: entry.date_modified,
                        relevance_rank: index as u32,
                    }
                }));
            }
            Err(error) => warnings.push(format!("Modrinth search failed: {error}")),
        }
    }

    if matches!(
        request.provider_filter,
        DiscoverProviderFilter::All | DiscoverProviderFilter::CurseForge
    ) {
        match curseforge {
            Some(client) => {
                let class_id = resolve_curseforge_modpack_class_id_cached(&client, &mut warnings);
                if let Some(class_id) = class_id {
                    match client.search_projects_with_filters(
                        MINECRAFT_GAME_ID,
                        request.query.as_str(),
                        offset,
                        DISCOVER_PROVIDER_LIMIT,
                        Some(class_id),
                        request.game_version.as_deref(),
                        request.loader_filter.curseforge_mod_loader_type(),
                    ) {
                        Ok(entries) => {
                            provider_result_count += entries.len();
                            provider_entries.extend(entries.into_iter().enumerate().map(
                                |(index, entry)| DiscoverProviderEntry {
                                    name: entry.name,
                                    summary: entry.summary,
                                    author: None,
                                    icon_url: entry.icon_url,
                                    primary_url: entry.website_url,
                                    source: DiscoverSource::CurseForge,
                                    popularity_score: Some(entry.download_count),
                                    updated_at: entry.date_modified,
                                    relevance_rank: index as u32,
                                },
                            ));
                        }
                        Err(error) => warnings.push(format!("CurseForge search failed: {error}")),
                    }
                }
            }
            None => warnings.push(
                "CurseForge API key missing in settings. Showing Modrinth results only.".to_owned(),
            ),
        }
    }

    let entries = build_snapshot_entries(provider_entries, request.sort_mode);
    let expected_page_size =
        enabled_provider_count(request).saturating_mul(DISCOVER_PROVIDER_LIMIT as usize);
    DiscoverSearchSnapshot {
        entries,
        warnings,
        has_more: expected_page_size > 0 && provider_result_count >= expected_page_size,
    }
}

fn build_snapshot_entries(
    provider_entries: Vec<DiscoverProviderEntry>,
    sort_mode: DiscoverSortMode,
) -> Vec<DiscoverEntry> {
    let mut deduped = HashMap::<String, DiscoverEntry>::new();
    for entry in provider_entries {
        let dedupe_key = normalize_search_key(entry.name.as_str());
        match deduped.get_mut(&dedupe_key) {
            Some(existing) => {
                if !existing.sources.contains(&entry.source) {
                    existing.sources.push(entry.source);
                }
                if existing.summary.len() < entry.summary.len() {
                    existing.summary = entry.summary.clone();
                }
                if existing.author.is_none() {
                    existing.author = entry.author.clone();
                }
                if existing.icon_url.is_none() {
                    existing.icon_url = entry.icon_url.clone();
                }
                if existing.primary_url.is_none() {
                    existing.primary_url = entry.primary_url.clone();
                }
                existing.popularity_score =
                    match (existing.popularity_score, entry.popularity_score) {
                        (Some(left), Some(right)) => Some(left.max(right)),
                        (None, right) => right,
                        (left, None) => left,
                    };
                existing.updated_at = existing.updated_at.clone().or(entry.updated_at.clone());
                existing.relevance_rank = existing.relevance_rank.min(entry.relevance_rank);
            }
            None => {
                deduped.insert(
                    dedupe_key.clone(),
                    DiscoverEntry {
                        dedupe_key,
                        name: entry.name,
                        summary: entry.summary,
                        author: entry.author,
                        icon_url: entry.icon_url,
                        primary_url: entry.primary_url,
                        sources: vec![entry.source],
                        popularity_score: entry.popularity_score,
                        updated_at: entry.updated_at,
                        relevance_rank: entry.relevance_rank,
                    },
                );
            }
        }
    }

    let mut entries = deduped.into_values().collect::<Vec<_>>();
    entries.sort_by(|left, right| match sort_mode {
        DiscoverSortMode::Popularity => right
            .popularity_score
            .cmp(&left.popularity_score)
            .then_with(|| left.name.cmp(&right.name)),
        DiscoverSortMode::LastUpdated => right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.name.cmp(&right.name)),
        DiscoverSortMode::Relevance => left
            .relevance_rank
            .cmp(&right.relevance_rank)
            .then_with(|| right.popularity_score.cmp(&left.popularity_score))
            .then_with(|| left.name.cmp(&right.name)),
    });
    entries
}

fn resolve_curseforge_modpack_class_id_cached(
    client: &CurseForgeClient,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    static CACHE: OnceLock<Mutex<Option<Option<u32>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(cache) = cache.lock()
        && let Some(class_id) = *cache
    {
        return class_id;
    }

    let class_id = resolve_curseforge_modpack_class_id(client, warnings);
    if let Ok(mut cache) = cache.lock() {
        *cache = Some(class_id);
    }
    class_id
}

fn resolve_curseforge_modpack_class_id(
    client: &CurseForgeClient,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    match client.list_content_classes(MINECRAFT_GAME_ID) {
        Ok(classes) => classes
            .into_iter()
            .find(|class_entry| {
                let normalized = normalize_search_key(class_entry.name.as_str());
                normalized.contains("modpack") || normalized.contains("mod pack")
            })
            .map(|class_entry| class_entry.id),
        Err(error) => {
            warnings.push(format!(
                "CurseForge modpack class discovery failed: {error}"
            ));
            None
        }
    }
}

fn normalize_search_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace())
        .collect::<String>()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn sized_combo_box(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    width: f32,
    selected_text: &str,
    add_contents: impl FnOnce(&mut Ui),
) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, style::CONTROL_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            egui::ComboBox::from_id_salt(id)
                .width(width)
                .selected_text(selected_text)
                .show_ui(ui, add_contents);
        },
    );
}

fn selected_game_version_label(
    selected_filter: &str,
    available_game_versions: &[MinecraftVersionEntry],
) -> String {
    let selected = selected_filter.trim();
    if selected.is_empty() {
        return "Any version".to_owned();
    }

    available_game_versions
        .iter()
        .find(|version| version.id == selected)
        .map(MinecraftVersionEntry::display_label)
        .unwrap_or_else(|| selected.to_owned())
}

fn request_version_catalog(state: &mut DiscoverState) {
    if state.version_catalog_in_flight
        || !state.available_game_versions.is_empty()
        || state.version_catalog_error.is_some()
    {
        return;
    }

    ensure_version_catalog_channel(state);
    let Some(tx) = state.version_catalog_tx.as_ref().cloned() else {
        return;
    };

    state.version_catalog_in_flight = true;
    let _ = tokio_runtime::spawn_detached(async move {
        let result = tokio_runtime::spawn_blocking(move || {
            fetch_version_catalog(false)
                .map(|catalog| catalog.game_versions)
                .map_err(|err| err.to_string())
        })
        .await
        .map_err(|err| format!("version catalog task join error: {err}"))
        .and_then(|inner| inner);
        let _ = tx.send(result);
    });
}

fn ensure_version_catalog_channel(state: &mut DiscoverState) {
    if state.version_catalog_tx.is_some() && state.version_catalog_rx.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<Result<Vec<MinecraftVersionEntry>, String>>();
    state.version_catalog_tx = Some(tx);
    state.version_catalog_rx = Some(Arc::new(Mutex::new(rx)));
}

fn poll_version_catalog(state: &mut DiscoverState) {
    let mut should_reset_channel = false;
    let mut updates = Vec::new();

    if let Some(rx) = state.version_catalog_rx.as_ref() {
        match rx.lock() {
            Ok(receiver) => loop {
                match receiver.try_recv() {
                    Ok(update) => updates.push(update),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        should_reset_channel = true;
                        break;
                    }
                }
            },
            Err(_) => should_reset_channel = true,
        }
    }

    if should_reset_channel {
        state.version_catalog_tx = None;
        state.version_catalog_rx = None;
        state.version_catalog_in_flight = false;
    }

    for update in updates {
        state.version_catalog_in_flight = false;
        match update {
            Ok(versions) => {
                state.available_game_versions = versions;
                state.version_catalog_error = None;
            }
            Err(err) => {
                state.version_catalog_error = Some(err);
            }
        }
    }
}

fn apply_search_snapshot(
    state: &mut DiscoverState,
    request: &DiscoverSearchRequest,
    snapshot: DiscoverSearchSnapshot,
    mode: SearchMode,
) {
    match mode {
        SearchMode::Replace => {
            state.entries = snapshot.entries;
            state.page = request.page.max(1);
        }
        SearchMode::Append => {
            for entry in snapshot.entries {
                if !state
                    .entries
                    .iter()
                    .any(|existing| existing.dedupe_key == entry.dedupe_key)
                {
                    state.entries.push(entry);
                }
            }
            state.page = request.page.max(state.page);
        }
    }
    state.warnings = snapshot.warnings;
    state.has_more_results = snapshot.has_more;
}

fn enabled_provider_count(request: &DiscoverSearchRequest) -> usize {
    match request.provider_filter {
        DiscoverProviderFilter::All => 1 + usize::from(CurseForgeClient::from_env().is_some()),
        DiscoverProviderFilter::Modrinth => 1,
        DiscoverProviderFilter::CurseForge => usize::from(CurseForgeClient::from_env().is_some()),
    }
}

fn format_compact_number(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.1}B", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn format_short_date(value: &str) -> String {
    value.get(0..10).unwrap_or(value).to_owned()
}
