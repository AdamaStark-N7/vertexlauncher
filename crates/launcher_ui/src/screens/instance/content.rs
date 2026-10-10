use super::content_lookup_result::ContentLookupResultEntry;
use super::*;
use content_resolver::detect_installed_content_kind;
use std::collections::{BTreeMap, HashSet};
use textui_egui::truncate_single_line_text_with_ellipsis;
use ui_foundation::tab_button;

#[path = "content_actions.rs"]
mod content_actions;
#[path = "content_hash_cache.rs"]
mod content_hash_cache;
#[path = "content_metadata_lookup.rs"]
mod content_metadata_lookup;
#[path = "content_updates.rs"]
mod content_updates;
#[path = "content_widgets.rs"]
mod content_widgets;
#[path = "installed_entry_display.rs"]
mod installed_entry_display;

use self::content_actions::{
    poll_content_apply_results, refresh_installed_content_state,
    render_joined_content_browser_controls, request_bulk_content_update, request_content_delete,
    request_content_toggle, request_content_update, request_local_content_import,
    request_vtmpatch_apply,
};
use self::content_hash_cache::*;
pub(super) use self::content_metadata_lookup::poll_content_lookup_results;
use self::content_metadata_lookup::*;
use self::content_updates::{
    bulk_update_button_label, bulk_update_button_tooltip, installed_content_lookup_repaint_delay,
    prefetch_bulk_update_metadata, resolve_installed_content_metadata_batch,
    tab_has_known_available_update,
};
use self::content_widgets::{
    cached_tinted_svg, render_bulk_update_button, render_content_thumbnail,
    render_installed_content_action_button, render_installed_content_badge,
    render_installed_content_update_badge, render_installed_content_warning,
    render_mod_enable_toggle,
};
use self::installed_entry_display::InstalledEntryDisplay;

const CONTENT_HASH_CACHE_FLUSH_DEBOUNCE: Duration = Duration::from_millis(750);
const CONTENT_LOOKUP_REPAINT_INTERVAL: Duration = Duration::from_millis(100);
const CONTENT_LOOKUP_BATCH_SIZE: usize = 24;
const CONTENT_UPDATE_PREFETCH_BATCH_SIZE: usize = 4;
const CONTENT_UPDATE_LOG_TARGET: &str = "vertexlauncher/content_update";
const INSTALLED_CONTENT_BADGE_FONT_SIZE: f32 = 13.0;
const INSTALLED_CONTENT_BADGE_LINE_HEIGHT: f32 = 16.0;
const INSTALLED_CONTENT_BADGE_PADDING_X: f32 = 6.0;
const INSTALLED_CONTENT_BADGE_PADDING_Y: f32 = 3.0;
const INSTANCE_CONTENT_TAB_ID_KEY: &str = "instance_content_tab_id";

pub(super) fn installed_content_tab_id(
    ctx: &egui::Context,
    kind: InstalledContentKind,
) -> Option<egui::Id> {
    ctx.data(|data| data.get_temp::<egui::Id>(egui::Id::new((INSTANCE_CONTENT_TAB_ID_KEY, kind))))
}

#[derive(Clone, Debug)]
pub(super) struct ContentApplyResult {
    pub(super) kind: InstalledContentKind,
    pub(super) focus_lookup_keys: Vec<String>,
    pub(super) refresh_all_content: bool,
    pub(super) status_message: Result<String, String>,
}

pub(super) fn render_installed_content_section(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    instance_root: &Path,
    download_policy: &DownloadPolicy,
    state: &mut InstanceScreenState,
    external_install_active: bool,
    output: &mut InstanceScreenOutput,
) {
    let content_browser_locked = state.runtime_prepare_in_flight || external_install_active;
    let content_browser_locked_reason =
        "Content Browser is unavailable while instance prep is running.";
    ui.add_space(12.0);
    ui.separator();
    take_content_dialog_results(ui, instance_id, instance_root, state);

    ui.add_space(10.0);
    poll_installed_content_scan_results(state);
    poll_content_hash_cache_load_results(state);
    poll_content_hash_cache_save_results(state);
    poll_content_apply_results(state, instance_root);
    ensure_content_hash_cache_loaded(state, instance_root);

    let (open_browser_response, add_menu_button) =
        render_joined_content_browser_controls(ui, text_ui, instance_id, !content_browser_locked);
    if content_browser_locked {
        let _ = crate::ui::style::hover_tip(
            ui,
            open_browser_response.clone(),
            content_browser_locked_reason,
        );
    }
    if open_browser_response.clicked() {
        output.requested_screen = Some(AppScreen::ContentBrowser);
    }

    render_add_content_popup(
        ui,
        text_ui,
        instance_id,
        instance_root,
        state,
        &add_menu_button,
    );

    ui.add_space(10.0);
    let previous_tab = state.selected_content_tab;
    render_installed_content_tab_row(ui, text_ui, instance_id, &mut state.selected_content_tab);
    if state.selected_content_tab != previous_tab {
        state.installed_content_page = 1;
    }

    ui.add_space(10.0);

    let installed_files =
        installed_content_files_for_tab(state, instance_root, state.selected_content_tab);
    if state
        .installed_content_cache
        .scans_in_flight
        .contains(&state.selected_content_tab)
    {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        let _ = text_ui.label(
            ui,
            (
                "instance_content_loading",
                instance_id,
                state.selected_content_tab.label(),
            ),
            &format!(
                "Loading installed {}...",
                state.selected_content_tab.label().to_lowercase()
            ),
            &style::muted(ui),
        );
        return;
    }
    if installed_files.is_empty() {
        let _ = text_ui.label(
            ui,
            (
                "instance_content_empty",
                instance_id,
                state.selected_content_tab.label(),
            ),
            &format!(
                "No {} installed.",
                state.selected_content_tab.label().to_lowercase()
            ),
            &style::muted(ui),
        );
        return;
    }

    let search_response = themed_text_input(
        text_ui,
        ui,
        (
            "instance_content_search",
            instance_id,
            state.selected_content_tab.label(),
        ),
        &mut state.installed_content_search_query,
        InputOptions {
            desired_width: Some(ui.available_width().max(180.0)),
            placeholder_text: Some(format!(
                "Search installed {} by name, file, version, or provider",
                state.selected_content_tab.label().to_lowercase()
            )),
            ..crate::ui::style::input_options(ui)
        },
    );
    if search_response.changed() {
        state.installed_content_page = 1;
    }
    ui.add_space(8.0);

    // `None` means "no search query": every installed file matches, so no per-frame copy is made.
    let filtered_indices = filter_installed_content_indices(state, installed_files.as_ref());
    let total_items = filtered_indices
        .as_ref()
        .map_or(installed_files.len(), Vec::len);
    if total_items == 0 {
        let query = state.installed_content_search_query.trim();
        let message = if query.is_empty() {
            format!(
                "No {} installed.",
                state.selected_content_tab.label().to_lowercase()
            )
        } else {
            format!(
                "No {} match '{query}'.",
                state.selected_content_tab.label().to_lowercase()
            )
        };
        let _ = text_ui.label(
            ui,
            (
                "instance_content_empty_filtered",
                instance_id,
                state.selected_content_tab.label(),
            ),
            &message,
            &style::muted(ui),
        );
        return;
    }

    let page_size = if INSTALLED_CONTENT_PAGE_SIZES.contains(&state.installed_content_page_size) {
        state.installed_content_page_size
    } else {
        INSTALLED_CONTENT_PAGE_SIZES[1]
    };
    state.installed_content_page_size = page_size;

    let total_pages = total_items.div_ceil(page_size).max(1);
    state.installed_content_page = state.installed_content_page.clamp(1, total_pages);

    let selected_game_version = selected_game_version(state).to_owned();
    let selected_modloader = selected_modloader_value(state).to_owned();
    prefetch_bulk_update_metadata(
        state,
        installed_files.as_ref(),
        state.selected_content_tab,
        selected_game_version.as_str(),
        selected_modloader.as_str(),
    );
    let has_bulk_update_available = tab_has_known_available_update(state, installed_files.as_ref());
    if let Some(delay) = installed_content_lookup_repaint_delay(state, installed_files.as_ref()) {
        ui.ctx().request_repaint_after(delay);
    }

    if has_bulk_update_available {
        render_bulk_update_row(
            ui,
            text_ui,
            instance_id,
            instance_root,
            download_policy,
            state,
            selected_game_version.as_str(),
            selected_modloader.as_str(),
        );
    }

    ui.add_space(8.0);
    render_installed_content_pagination_controls(
        ui,
        text_ui,
        instance_id,
        state,
        total_items,
        installed_files.len(),
        total_pages,
    );

    let start_index = (state.installed_content_page - 1) * state.installed_content_page_size;
    let end_index = (start_index + state.installed_content_page_size).min(total_items);
    let visible_files: std::borrow::Cow<[InstalledContentFile]> = match filtered_indices.as_ref() {
        Some(indices) => std::borrow::Cow::Owned(
            indices[start_index..end_index]
                .iter()
                .map(|&index| installed_files[index].clone())
                .collect(),
        ),
        None => std::borrow::Cow::Borrowed(&installed_files[start_index..end_index]),
    };
    let visible_files = visible_files.as_ref();
    let _ = request_content_metadata_lookup_batch(
        state,
        visible_files,
        state.selected_content_tab,
        selected_game_version.as_str(),
        selected_modloader.as_str(),
        false,
        CONTENT_LOOKUP_BATCH_SIZE,
    );
    let delete_icon_color = ui.visuals().error_fg_color;
    let delete_button_icon_svg =
        cached_tinted_svg(ui.ctx(), "trash_x", assets::TRASH_X_SVG, delete_icon_color);
    let warning_icon_svg = cached_tinted_svg(
        ui.ctx(),
        "warn",
        assets::WARN_SVG,
        ui.visuals().warn_fg_color,
    );

    let mut pending_delete: Option<(PathBuf, String)> = None;
    let mut pending_update: Option<(String, String, PathBuf)> = None;
    let mut pending_toggle: Option<(PathBuf, String, bool)> = None;
    let scroll_height = ui.available_height().max(180.0);
    egui::ScrollArea::vertical()
        .scroll_source(modal_host::drag_scroll_source())
        .id_salt((
            "instance_installed_content_scroll",
            instance_id,
            state.selected_content_tab.label(),
        ))
        .auto_shrink([false, false])
        .max_height(scroll_height)
        .show(ui, |ui| {
            let row_width = (ui.max_rect().width() - INSTALLED_CONTENT_SCROLLBAR_RESERVE).max(1.0);
            ui.set_min_width(row_width);
            ui.set_max_width(row_width);
            for (visible_index, entry) in visible_files.iter().enumerate() {
                let entry_index = start_index + visible_index;
                let metadata = state
                    .content_metadata_cache
                    .get(&entry.lookup_key)
                    .and_then(|meta| meta.as_ref());
                let display = InstalledEntryDisplay::new(entry, metadata);

                let rendered = ui
                    .scope_builder(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(row_width, f32::INFINITY),
                        )),
                        |ui| {
                            ui.set_min_width(row_width);
                            ui.set_max_width(row_width);
                            render_installed_content_entry(
                                ui,
                                text_ui,
                                (instance_id, entry_index),
                                state,
                                entry,
                                display.display_name.as_str(),
                                display.description.as_str(),
                                display.platform_label.as_str(),
                                display.version_label.as_deref(),
                                display.warning_message.as_deref(),
                                display.update_label.as_deref(),
                                display.icon_url.as_deref(),
                                &delete_button_icon_svg,
                                delete_icon_color,
                                &warning_icon_svg,
                                state.selected_content_tab == InstalledContentKind::Mods,
                            )
                        },
                    )
                    .inner;

                if rendered.toggle_clicked {
                    pending_toggle = Some((
                        entry.file_path.clone(),
                        entry.lookup_key.clone(),
                        entry.disabled,
                    ));
                } else if rendered.delete_clicked {
                    pending_delete = Some((entry.file_path.clone(), entry.lookup_key.clone()));
                } else if rendered.update_clicked {
                    if let Some(version_id) = display.update_version_id.as_ref() {
                        pending_update = Some((
                            entry.lookup_key.clone(),
                            version_id.to_owned(),
                            entry.file_path.clone(),
                        ));
                    }
                } else if rendered.open_clicked {
                    open_installed_content_detail(
                        state,
                        output,
                        entry.lookup_key.as_str(),
                        content_browser_locked,
                    );
                }
                ui.add_space(8.0);
            }
        });

    apply_pending_content_actions(
        state,
        instance_root,
        pending_delete,
        pending_toggle,
        pending_update,
        selected_game_version.as_str(),
        selected_modloader.as_str(),
    );
    schedule_content_hash_cache_repaints_and_flush(ui, state, instance_root);
}

/// Collects results of the add-local-file and apply-patch dialogs. The popup closes as soon as
/// an entry is clicked, so this runs on every frame outside it.
#[allow(clippy::too_many_arguments)]
fn render_bulk_update_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    instance_root: &Path,
    download_policy: &DownloadPolicy,
    state: &mut InstanceScreenState,
    selected_game_version: &str,
    selected_modloader: &str,
) {
    ui.add_space(2.0);
    let bulk_update_label = bulk_update_button_label(state.selected_content_tab);
    let bulk_update_tooltip = bulk_update_button_tooltip(state.selected_content_tab);
    let bulk_update_clicked = render_bulk_update_button(
        ui,
        text_ui,
        (
            "instance_content_bulk_update",
            instance_id,
            state.selected_content_tab.label(),
        ),
        bulk_update_label.as_str(),
        bulk_update_tooltip.as_str(),
        !state.content_apply_in_flight,
    );
    ui.add_space(8.0);

    if bulk_update_clicked {
        request_bulk_content_update(
            state,
            instance_root,
            state.selected_content_tab,
            selected_game_version,
            selected_modloader,
            download_policy,
        );
    }
}

/// Handles a click on an installed row: opens the content browser detail page for the row's
/// resolved metadata, or reports why that is not possible yet.
fn open_installed_content_detail(
    state: &mut InstanceScreenState,
    output: &mut InstanceScreenOutput,
    lookup_key: &str,
    content_browser_locked: bool,
) {
    if content_browser_locked {
        state.status_message =
            Some("Content Browser is unavailable while instance prep is running.".to_owned());
    } else if let Some(metadata) = state
        .content_metadata_cache
        .get(lookup_key)
        .and_then(|meta| meta.clone())
    {
        crate::screens::content_browser::request_open_detail_for_content(metadata.entry);
        output.requested_screen = Some(AppScreen::ContentBrowser);
    } else {
        state.status_message =
            Some("Still loading content metadata. Try again in a moment.".to_owned());
    }
}

fn take_content_dialog_results(
    ui: &Ui,
    instance_id: &str,
    instance_root: &Path,
    state: &mut InstanceScreenState,
) {
    if let Some(paths) =
        crate::ui::file_dialog::take(ui.ctx(), ("instance_add_local_dialog", instance_id))
        && !paths.is_empty()
    {
        request_local_content_import(state, instance_root, paths);
    }
    if let Some(path) =
        crate::ui::file_dialog::take(ui.ctx(), ("instance_apply_patch_dialog", instance_id))
            .and_then(|paths| paths.into_iter().next())
    {
        request_vtmpatch_apply(state, instance_root, path);
    }
}

fn render_add_content_popup(
    ui: &Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    instance_root: &Path,
    state: &mut InstanceScreenState,
    add_menu_button: &egui::Response,
) {
    let popup_id = ui.id().with(("instance_add_content_popup", instance_id));
    let _ = egui::Popup::menu(add_menu_button)
        .id(popup_id)
        .width(240.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            let popup_button_style = style::neutral_button_with_min_size(
                ui,
                egui::vec2(ui.available_width().max(120.0), style::CONTROL_HEIGHT),
            );
            let add_local_response = ui
                .add_enabled_ui(!state.content_apply_in_flight, |ui| {
                    text_ui.button(
                        ui,
                        ("instance_content_popup_add_local_files", instance_id),
                        "Add local file(s)",
                        &popup_button_style,
                    )
                })
                .inner;
            if add_local_response.clicked() {
                crate::ui::file_dialog::open(
                    ui.ctx(),
                    ("instance_add_local_dialog", instance_id),
                    crate::ui::file_dialog::Pick::Files,
                    crate::ui::file_dialog::Dialog::new().title("Add Local Content"),
                );
            }

            let apply_patch_response = ui
                .add_enabled_ui(!state.content_apply_in_flight, |ui| {
                    text_ui.button(
                        ui,
                        ("instance_content_popup_apply_patch", instance_id),
                        "Apply Patch",
                        &popup_button_style,
                    )
                })
                .inner;
            if apply_patch_response.clicked() {
                crate::ui::file_dialog::open(
                    ui.ctx(),
                    ("instance_apply_patch_dialog", instance_id),
                    crate::ui::file_dialog::Pick::File,
                    crate::ui::file_dialog::Dialog::new()
                        .title("Apply Vertex Patch")
                        .filter("Vertex Modpack Patch", &[VTMPATCH_EXTENSION]),
                );
            }

            if text_ui
                .button(
                    ui,
                    ("instance_content_popup_local", instance_id),
                    "Refresh local files",
                    &popup_button_style,
                )
                .clicked()
            {
                refresh_installed_content_state(state, instance_root);
                state.status_message =
                    Some("Refreshed installed content metadata and hash cache.".to_owned());
            }
        });
}

fn apply_pending_content_actions(
    state: &mut InstanceScreenState,
    instance_root: &Path,
    pending_delete: Option<(PathBuf, String)>,
    pending_toggle: Option<(PathBuf, String, bool)>,
    pending_update: Option<(String, String, PathBuf)>,
    selected_game_version: &str,
    selected_modloader: &str,
) {
    if let Some((path, lookup_key)) = pending_delete {
        request_content_delete(
            state,
            instance_root,
            state.selected_content_tab,
            lookup_key.as_str(),
            path.as_path(),
        );
    }

    if let Some((path, lookup_key, currently_disabled)) = pending_toggle {
        request_content_toggle(
            state,
            state.selected_content_tab,
            lookup_key.as_str(),
            path.as_path(),
            currently_disabled,
        );
    }

    if let Some((lookup_key, version_id, file_path)) = pending_update
        && let Some(metadata) = state
            .content_metadata_cache
            .get(&lookup_key)
            .and_then(|meta| meta.clone())
    {
        request_content_update(
            state,
            instance_root,
            lookup_key.as_str(),
            metadata.entry,
            file_path.as_path(),
            version_id.as_str(),
            selected_game_version,
            selected_modloader,
        );
    }
}

fn schedule_content_hash_cache_repaints_and_flush(
    ui: &Ui,
    state: &mut InstanceScreenState,
    instance_root: &Path,
) {
    if state.content_hash_cache_dirty {
        let repaint_after = state
            .content_hash_cache_dirty_since
            .map(|dirty_since| {
                CONTENT_HASH_CACHE_FLUSH_DEBOUNCE.saturating_sub(dirty_since.elapsed())
            })
            .unwrap_or(CONTENT_HASH_CACHE_FLUSH_DEBOUNCE);
        ui.ctx().request_repaint_after(repaint_after);
    }
    if state.content_hash_cache_load_in_flight || state.content_hash_cache_save_in_flight {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
    flush_content_hash_cache(state, instance_root);
}

fn render_installed_content_tab_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    active_tab: &mut InstalledContentKind,
) {
    let tabs = InstalledContentKind::ALL.map(|tab| (tab, tab.label()));
    let spacing = 6.0;
    let height = 30.0;
    let width =
        ((ui.available_width() - spacing * (tabs.len() as f32 - 1.0)) / tabs.len() as f32).max(0.0);
    ui.push_id(("instance_content_tab", instance_id), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for &(tab, label) in &tabs {
                let selected = *active_tab == tab;
                let response = text_ui.selectable_button(
                    ui,
                    ("fill_tab_row", label),
                    label,
                    selected,
                    &tab_button(ui, selected, egui::vec2(width, height)),
                );
                ui.ctx().data_mut(|data| {
                    data.insert_temp(
                        egui::Id::new((INSTANCE_CONTENT_TAB_ID_KEY, tab)),
                        response.id,
                    )
                });
                if response.clicked() {
                    *active_tab = tab;
                }
            }
        });
    });
}

fn render_installed_content_pagination_controls(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instance_id: &str,
    state: &mut InstanceScreenState,
    total_items: usize,
    total_installed: usize,
    total_pages: usize,
) {
    ui.horizontal(|ui| {
        ui.set_min_height(style::CONTROL_HEIGHT);
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        let label_style = LabelOptions {
            line_height: style::CONTROL_HEIGHT - 6.0,
            wrap: false,
            ..style::body(ui)
        };
        let muted_label_style = LabelOptions {
            color: ui.visuals().weak_text_color(),
            ..label_style.clone()
        };

        let _ = text_ui.label(
            ui,
            ("instance_content_page_size_label", instance_id),
            "Items per page",
            &label_style,
        );
        let page_size_labels = INSTALLED_CONTENT_PAGE_SIZES
            .iter()
            .map(|page_size| page_size.to_string())
            .collect::<Vec<_>>();
        let page_size_options = page_size_labels
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut selected_page_size_index = INSTALLED_CONTENT_PAGE_SIZES
            .iter()
            .position(|candidate| *candidate == state.installed_content_page_size)
            .unwrap_or(0);
        if settings_widgets::dropdown_picker(
            text_ui,
            ui,
            ("instance_content_page_size", instance_id),
            &mut selected_page_size_index,
            &page_size_options,
            Some(92.0),
        )
        .changed()
        {
            state.installed_content_page_size =
                INSTALLED_CONTENT_PAGE_SIZES[selected_page_size_index];
            state.installed_content_page = 1;
        }

        let _ = text_ui.label(
            ui,
            ("instance_content_page_label", instance_id),
            "Page",
            &label_style,
        );
        let page_labels = (1..=total_pages)
            .map(|page| {
                if page == state.installed_content_page {
                    format!("{page} / {total_pages}")
                } else {
                    format!("Page {page}")
                }
            })
            .collect::<Vec<_>>();
        let page_options = page_labels.iter().map(String::as_str).collect::<Vec<_>>();
        let mut selected_page_index = state.installed_content_page.saturating_sub(1);
        if settings_widgets::dropdown_picker(
            text_ui,
            ui,
            ("instance_content_page", instance_id),
            &mut selected_page_index,
            &page_options,
            Some(112.0),
        )
        .changed()
        {
            state.installed_content_page = selected_page_index + 1;
        }

        let _ = text_ui.label(
            ui,
            ("instance_content_page_total", instance_id),
            &installed_content_count_label(
                total_items,
                total_installed,
                state.installed_content_search_query.as_str(),
            ),
            &muted_label_style,
        );
    });
}

fn filter_installed_content_indices(
    state: &InstanceScreenState,
    files: &[InstalledContentFile],
) -> Option<Vec<usize>> {
    let query = state
        .installed_content_search_query
        .trim()
        .to_ascii_lowercase();
    if query.is_empty() {
        return None;
    }

    Some(
        files
            .iter()
            .enumerate()
            .filter(|(_, file)| installed_content_file_matches_search(state, file, query.as_str()))
            .map(|(index, _)| index)
            .collect(),
    )
}

fn installed_content_file_matches_search(
    state: &InstanceScreenState,
    file: &InstalledContentFile,
    query: &str,
) -> bool {
    let mut haystack = vec![
        file.file_name.as_str(),
        file.lookup_query.as_str(),
        file.lookup_key.as_str(),
    ];
    if let Some(value) = file.fallback_lookup_query.as_deref() {
        haystack.push(value);
    }
    if let Some(value) = file.fallback_lookup_key.as_deref() {
        haystack.push(value);
    }
    if let Some(identity) = file.managed_identity.as_ref() {
        haystack.push(identity.name.as_str());
        haystack.push(identity.selected_version_id.as_str());
    }
    if let Some(metadata) = state
        .content_metadata_cache
        .get(&file.lookup_key)
        .and_then(|metadata| metadata.as_ref())
    {
        haystack.push(metadata.entry.name.as_str());
        haystack.push(metadata.entry.summary.as_str());
        haystack.push(metadata.entry.source.label());
        if let Some(version) = metadata.installed_version_label.as_deref() {
            haystack.push(version);
        }
        if let Some(warning) = metadata.warning_message.as_deref() {
            haystack.push(warning);
        }
        if let Some(update) = metadata.update.as_ref() {
            haystack.push(update.latest_version_label.as_str());
        }
    }

    haystack
        .iter()
        .any(|value| contains_ignore_ascii_case(value, query))
}

/// Equivalent to `value.to_ascii_lowercase().contains(query)` for an already lowercased
/// `query`, without allocating.
fn contains_ignore_ascii_case(value: &str, query: &str) -> bool {
    let query = query.as_bytes();
    if query.is_empty() {
        return true;
    }
    value
        .as_bytes()
        .windows(query.len())
        .any(|window| window.eq_ignore_ascii_case(query))
}

fn installed_content_count_label(matching: usize, total: usize, query: &str) -> String {
    if query.trim().is_empty() || matching == total {
        format!("{total} installed")
    } else {
        format!("{matching} matching / {total} installed")
    }
}

fn poll_installed_content_scan_results(state: &mut InstanceScreenState) {
    let drained = state.installed_content_cache.scan_results.drain();
    if drained.disconnected {
        tracing::error!(target: "vertexlauncher/instance_content", "scan_results receiver was poisoned; channel reset.");
    }
    for result in drained.items {
        if result.generation != state.installed_content_cache.scan_generation {
            continue;
        }
        state
            .installed_content_cache
            .scans_in_flight
            .remove(&result.kind);
        state.installed_content_cache.managed_identities = Some(result.managed_identities);
        state
            .installed_content_cache
            .files_by_tab
            .insert(result.kind, result.files);
    }
}

fn request_installed_content_scan(
    state: &mut InstanceScreenState,
    instance_root: &Path,
    tab: InstalledContentKind,
) {
    if state.installed_content_cache.scans_in_flight.contains(&tab) {
        return;
    }
    let tx = state.installed_content_cache.scan_results.sender();

    let generation = state.installed_content_cache.scan_generation;
    let instance_root = instance_root.to_path_buf();
    state.installed_content_cache.scans_in_flight.insert(tab);
    let _ = tokio_runtime::spawn_detached(async move {
        let join = tokio_runtime::spawn_blocking(move || {
            let managed_identities = load_managed_content_identities(instance_root.as_path());
            let files: Arc<[InstalledContentFile]> =
                InstalledContentResolver::scan_installed_content_files(
                    instance_root.as_path(),
                    tab,
                    &managed_identities,
                )
                .into();
            InstalledContentScanResult {
                generation,
                kind: tab,
                managed_identities,
                files,
            }
        });
        match join.await {
            Ok(result) => {
                let _ = tx.send(result);
            }
            Err(err) => {
                tracing::error!(
                    target: "vertexlauncher/instance_content",
                    kind = %tab.folder_name(),
                    error = %err,
                    "Installed-content scan worker panicked."
                );
            }
        }
    });
}

fn installed_content_files_for_tab(
    state: &mut InstanceScreenState,
    instance_root: &Path,
    tab: InstalledContentKind,
) -> Arc<[InstalledContentFile]> {
    if let Some(files) = state.installed_content_cache.files_by_tab.get(&tab) {
        return files.clone();
    }

    request_installed_content_scan(state, instance_root, tab);
    Arc::from([])
}

fn render_installed_content_entry(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    state: &mut InstanceScreenState,
    entry: &InstalledContentFile,
    display_name: &str,
    description: &str,
    platform_label: &str,
    version_label: Option<&str>,
    warning_message: Option<&str>,
    update_label: Option<&str>,
    icon_url: Option<&str>,
    delete_button_icon_svg: &Arc<[u8]>,
    delete_icon_color: egui::Color32,
    warning_icon_svg: &Arc<[u8]>,
    is_mod_tab: bool,
) -> InstalledEntryRenderResult {
    const INSTALLED_TILE_GAP: f32 = 8.0;
    const INSTALLED_TILE_THUMBNAIL_FRAME_PADDING: f32 = 8.0;
    let available_width = ui.available_width().max(1.0);
    let tile_width = (available_width - (style::SPACE_XS * 2.0)).max(1.0);
    let side_padding = ((available_width - tile_width) * 0.5).max(0.0);

    let (delete_clicked, update_clicked, toggle_clicked, open_clicked) = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if side_padding > 0.0 {
                ui.add_space(side_padding);
            }

            let frame_response = egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_MD))
                .inner_margin(egui::Margin::same(style::SPACE_MD as i8))
                .show(ui, |ui| {
                    ui.set_min_width(tile_width);
                    ui.set_max_width(tile_width);

                    let mut delete_clicked = false;
                    let mut update_clicked = false;
                    let mut toggle_clicked = false;
                    let action_button_width = 28.0;
                    let content_width = ui.available_width().max(1.0);
                    let thumbnail_size = ((content_width - 52.0) * 0.14).clamp(32.0, 48.0);
                    let thumbnail_frame_size =
                        thumbnail_size + INSTALLED_TILE_THUMBNAIL_FRAME_PADDING;
                    let thumbnail_lane_height = 92.0_f32.max(thumbnail_frame_size);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let delete_button_id =
                            format!("instance-content-delete-{}", entry.lookup_key);
                        if render_installed_content_action_button(
                            ui,
                            delete_button_id.as_str(),
                            delete_button_icon_svg,
                            delete_icon_color,
                            "Delete this content",
                            action_button_width,
                            action_button_width,
                        ) {
                            delete_clicked = true;
                        }

                        if is_mod_tab {
                            ui.add_space(INSTALLED_TILE_GAP);
                            // Width scales with height using the same 11:7 ratio a typical
                            // pill switch uses — both dimensions now track action_button_width
                            // so they scale consistently with the delete button.
                            let toggle_width = (action_button_width * 1.57).ceil();
                            if render_mod_enable_toggle(
                                ui,
                                entry.lookup_key.as_str(),
                                !entry.disabled,
                                toggle_width,
                                action_button_width,
                            ) {
                                toggle_clicked = true;
                            }
                        }

                        ui.add_space(INSTALLED_TILE_GAP);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width().max(1.0), 0.0),
                            egui::Layout::left_to_right(egui::Align::TOP),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.allocate_ui_with_layout(
                                    egui::vec2(thumbnail_frame_size, thumbnail_lane_height),
                                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                                    |ui| {
                                        egui::Frame::new()
                                            .fill(ui.visuals().extreme_bg_color)
                                            .stroke(egui::Stroke::NONE)
                                            .corner_radius(egui::CornerRadius::same(
                                                style::CORNER_RADIUS_SM,
                                            ))
                                            .inner_margin(egui::Margin::same(4))
                                            .show(ui, |ui| {
                                                render_content_thumbnail(
                                                    ui,
                                                    id_source,
                                                    icon_url,
                                                    thumbnail_size,
                                                );
                                            });
                                    },
                                );

                                ui.add_space(INSTALLED_TILE_GAP);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width().max(1.0), 0.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.set_width(ui.available_width().max(1.0));
                                        let _ = text_ui.label(
                                            ui,
                                            (id_source, "name"),
                                            display_name,
                                            &LabelOptions {
                                                wrap: true,
                                                ..style::stat_label(ui)
                                            },
                                        );

                                        ui.add_space(4.0);
                                        if render_installed_content_badge_row(
                                            ui,
                                            text_ui,
                                            id_source,
                                            state.content_apply_in_flight,
                                            platform_label,
                                            version_label,
                                            update_label,
                                            warning_message,
                                            warning_icon_svg,
                                        ) {
                                            update_clicked = true;
                                        }

                                        ui.add_space(4.0);
                                        render_installed_content_description(
                                            ui,
                                            text_ui,
                                            state,
                                            id_source,
                                            entry.lookup_key.as_str(),
                                            description,
                                        );
                                    },
                                );
                            },
                        );
                    });

                    (delete_clicked, update_clicked, toggle_clicked)
                });

            if side_padding > 0.0 {
                ui.add_space(side_padding);
            }

            (
                frame_response.inner.0,
                frame_response.inner.1,
                frame_response.inner.2,
                frame_response.response.clicked(),
            )
        })
        .inner;

    InstalledEntryRenderResult {
        open_clicked: open_clicked && !delete_clicked && !update_clicked && !toggle_clicked,
        delete_clicked,
        update_clicked,
        toggle_clicked,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_installed_content_badge_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    content_apply_in_flight: bool,
    platform_label: &str,
    version_label: Option<&str>,
    update_label: Option<&str>,
    warning_message: Option<&str>,
    warning_icon_svg: &Arc<[u8]>,
) -> bool {
    let mut update_clicked = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        render_installed_content_badge(
            ui,
            text_ui,
            (id_source, "platform_badge"),
            platform_label,
            ui.visuals().selection.bg_fill,
            ui.visuals().selection.stroke.color,
        );
        if let Some(version_label) = version_label {
            render_installed_content_badge(
                ui,
                text_ui,
                (id_source, "version_badge"),
                version_label,
                ui.visuals().widgets.inactive.bg_fill,
                ui.visuals().text_color(),
            );
        }
        if let Some(update) = update_label
            && render_installed_content_update_badge(
                ui,
                text_ui,
                (id_source, "update_badge"),
                update,
                !content_apply_in_flight,
            )
        {
            update_clicked = true;
        }
        if let Some(warning_message) = warning_message {
            render_installed_content_warning(ui, warning_message, id_source, warning_icon_svg);
        }
    });
    update_clicked
}

fn render_installed_content_description(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    state: &mut InstanceScreenState,
    id_source: impl std::hash::Hash + Copy,
    lookup_key: &str,
    description: &str,
) {
    const INSTALLED_DESCRIPTION_LINE_HEIGHT: f32 = 20.0;
    const INSTALLED_DESCRIPTION_FRAME_Y_PADDING: i8 = 3;
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_SM))
        .inner_margin(egui::Margin::symmetric(
            6,
            INSTALLED_DESCRIPTION_FRAME_Y_PADDING,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width().max(1.0));
            let description_style = LabelOptions {
                line_height: INSTALLED_DESCRIPTION_LINE_HEIGHT,
                wrap: false,
                ..style::body(ui)
            };
            let truncated_description = cached_truncated_description(
                state,
                text_ui,
                ui,
                lookup_key,
                description,
                ui.available_width().max(1.0),
                &description_style,
            );
            ui.allocate_ui_with_layout(
                egui::vec2(
                    ui.available_width().max(1.0),
                    INSTALLED_DESCRIPTION_LINE_HEIGHT,
                ),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(INSTALLED_DESCRIPTION_LINE_HEIGHT);
                    ui.set_max_height(INSTALLED_DESCRIPTION_LINE_HEIGHT);
                    ui.set_width(ui.available_width().max(1.0));
                    let _ = text_ui.label(
                        ui,
                        (id_source, "description"),
                        truncated_description.as_str(),
                        &description_style,
                    );
                },
            );
        });
}

fn cached_truncated_description(
    state: &mut InstanceScreenState,
    text_ui: &mut TextUi,
    ui: &Ui,
    lookup_key: &str,
    description: &str,
    max_width: f32,
    label_options: &LabelOptions,
) -> String {
    let width_bucket = (max_width.max(0.0) / 8.0).round() as u32;
    if let Some(cache_entry) = state.installed_content_entry_ui_cache.get(lookup_key)
        && cache_entry.description_source == description
        && cache_entry.description_width_bucket == width_bucket
    {
        return cache_entry.truncated_description.clone();
    }

    let truncated_description =
        truncate_single_line_text_with_ellipsis(text_ui, ui, description, max_width, label_options);
    state.installed_content_entry_ui_cache.insert(
        lookup_key.to_owned(),
        InstalledContentEntryUiCache {
            description_source: description.to_owned(),
            description_width_bucket: width_bucket,
            truncated_description: truncated_description.clone(),
        },
    );
    truncated_description
}
