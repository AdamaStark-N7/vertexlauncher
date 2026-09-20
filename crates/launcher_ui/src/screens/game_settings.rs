//! In-game settings editor for an instance's `options.txt`.
//!
//! The dialog shows a live view of the file: edits are written a moment after you stop
//! typing (on top of whatever is on disk at that time), and changes made by the game or by
//! settings sync appear in the dialog as soon as they hit the disk.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use config::Config;
use instance_sync::{PackStatus, pack_status, scan_packs};
use instances::{InstanceRecord, InstanceStore, PackOverride, instance_root_path};
use mc_options::packs::PackSupport;
use mc_options::schema::{self, Display, Group, Kind, OptionDef};
use mc_options::translate::format_for_def;
use mc_options::{LiveOptions, Poll, Ver, keys, parse_game_version, value};
use textui::TextUi;
use textui_egui::prelude::*;

use crate::app::tokio_runtime;
use crate::sync_runner::{self, SyncRunConfig};
use crate::ui::components::settings_widgets;
use crate::ui::{modal, style};

#[path = "game_settings/audio_devices.rs"]
mod audio_devices;
#[path = "game_settings/key_capture.rs"]
mod key_capture;

const REQUEST_KEY: &str = "game_settings_modal";
/// Edits are written once they have been idle this long.
const SAVE_DELAY: Duration = Duration::from_millis(250);
/// How often to look for outside changes while the dialog is open.
const WATCH_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Group(Group),
    Packs,
    Sync,
}

#[derive(Clone)]
struct ModalState {
    instance_id: String,
    instance_name: String,
    game_version: String,
    version: Option<Ver>,
    live: Arc<Mutex<LiveOptions>>,
    tab: Tab,
    search: String,
    capturing: Option<String>,
    capture_started_at_frame: u64,
    pack_rows: Arc<Mutex<Option<(Instant, Vec<PackRow>)>>>,
    /// Output devices found by a background lookup; `None` while it runs.
    audio_devices: Arc<Mutex<Option<Vec<String>>>>,
}

/// A resource pack seen in this instance or another syncing instance.
#[derive(Clone)]
struct PackRow {
    name: String,
    support: Option<PackSupport>,
    present_here: bool,
    /// Names of other instances that have it.
    elsewhere: Vec<String>,
}

/// Opens the game settings dialog for `instance`.
pub(super) fn request_modal(
    ctx: &egui::Context,
    instance: &InstanceRecord,
    installations_root: &Path,
) {
    let instance_root = instance_root_path(installations_root, instance);
    let path = instance_root.join("options.txt");
    let audio_devices = Arc::new(Mutex::new(None));
    {
        let audio_devices = Arc::clone(&audio_devices);
        let ctx = ctx.clone();
        tokio_runtime::spawn_blocking_detached(move || {
            let devices = audio_devices::list_output_devices();
            if let Ok(mut slot) = audio_devices.lock() {
                *slot = Some(devices);
            }
            ctx.request_repaint();
        });
    }
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(REQUEST_KEY),
            ModalState {
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                game_version: instance.game_version.clone(),
                version: parse_game_version(&instance.game_version),
                live: Arc::new(Mutex::new(open_live_options(
                    path,
                    instance_root.as_path(),
                    &instance.game_version,
                ))),
                tab: Tab::Group(Group::Video),
                search: String::new(),
                capturing: None,
                capture_started_at_frame: 0,
                pack_rows: Arc::new(Mutex::new(None)),
                audio_devices,
            },
        );
    });
}

/// Opens the live view of `options.txt`. If the game hasn't recorded its data version there yet
/// (fresh instance), records it now, as if that version of the game had written the file. The
/// value is never shown in the UI.
fn open_live_options(
    path: std::path::PathBuf,
    instance_root: &Path,
    game_version: &str,
) -> LiveOptions {
    let mut live = LiveOptions::open(path);
    if live.get("version").is_none()
        && let Some(world_version) = mc_options::data_version::read_world_version(
            &mc_options::data_version::client_jar_path(instance_root, game_version),
        )
    {
        live.edit("version", Some(&world_version.to_string()));
    }
    live
}

/// Writes a new value for `def`, formatted the way this version's file expects.
fn commit(live: &mut LiveOptions, def: &OptionDef, version: Option<Ver>, new_text: &str) {
    if let Some(raw) = format_for_def(def, new_text, live.file(), version)
        && live.get(def.key) != Some(raw.as_str())
    {
        live.edit(def.key, Some(&raw));
    }
}

fn tooltip_for(def: &OptionDef) -> String {
    mc_options::descriptions::tooltip(def)
}

/// Raw value shown for a key: what's in the file, else the default.
fn shown_value<'a>(live: &'a LiveOptions, def: &'a OptionDef) -> &'a str {
    value::unquote(live.get(def.key).unwrap_or(def.default)).0
}

fn render_choice_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    live: &mut LiveOptions,
    def: &OptionDef,
    version: Option<Ver>,
    labels: Vec<(String, String)>,
) {
    let current = shown_value(live, def).to_owned();
    let mut labels = labels;
    let mut selected = labels.iter().position(|(stored, _)| *stored == current);
    if selected.is_none() {
        // Show what the file really holds instead of pretending it's a known choice.
        labels.push((current.clone(), format!("{current} (unrecognized)")));
        selected = Some(labels.len() - 1);
    }
    let mut index = selected.unwrap_or(0);
    let before = index;
    let names: Vec<&str> = labels.iter().map(|(_, label)| label.as_str()).collect();
    let tip = tooltip_for(def);
    let _ = settings_widgets::full_width_dropdown_row(
        text_ui,
        ui,
        ("game_setting", def.key),
        def.label,
        Some(tip.as_str()),
        &mut index,
        &names,
    );
    if index != before
        && let Some((stored, _)) = labels.get(index)
    {
        commit(live, def, version, stored);
    }
}

fn render_keybind_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
    key: &str,
    label: &str,
    default: &str,
    conflicts: &BTreeMap<String, Vec<String>>,
) {
    let legacy = state.version.is_some_and(|v| v < Ver(1, 13, 0));
    let raw = live.get(key).unwrap_or(default).to_owned();
    let modern = if legacy {
        raw.parse::<i32>()
            .ok()
            .and_then(keys::legacy_to_modern)
            .unwrap_or_else(|| raw.clone())
    } else {
        raw.clone()
    };
    let store = |live: &mut LiveOptions, modern_name: &str| {
        let stored = if legacy {
            match keys::modern_to_legacy(modern_name) {
                Some(code) => code.to_string(),
                None => return,
            }
        } else {
            modern_name.to_owned()
        };
        if live.get(key) != Some(stored.as_str()) {
            live.edit(key, Some(&stored));
        }
    };

    ui.horizontal(|ui| {
        let _ = text_ui.label(
            ui,
            ("keybind_label", key),
            label,
            &LabelOptions {
                wrap: false,
                ..style::body(ui)
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let capturing = state.capturing.as_deref() == Some(key);
            let button_text = if capturing {
                "Press a key…"
            } else {
                "Change"
            };
            if text_ui
                .button(
                    ui,
                    ("keybind_capture", key),
                    button_text,
                    &style::neutral_button_with_min_size(
                        ui,
                        egui::vec2(120.0, style::CONTROL_HEIGHT),
                    ),
                )
                .clicked()
            {
                state.capturing = (!capturing).then(|| key.to_owned());
                state.capture_started_at_frame = ui.ctx().cumulative_frame_nr();
            }
            if text_ui
                .button(
                    ui,
                    ("keybind_unbind", key),
                    "Unbind",
                    &style::neutral_button_with_min_size(
                        ui,
                        egui::vec2(90.0, style::CONTROL_HEIGHT),
                    ),
                )
                .clicked()
            {
                store(live, "key.keyboard.unknown");
            }
            let _ = text_ui.label(
                ui,
                ("keybind_value", key),
                keys::display_name(&modern).as_str(),
                &LabelOptions {
                    wrap: false,
                    ..style::body_strong(ui)
                },
            );
        });
    });
    if let Some(others) = conflicts.get(&modern).filter(|others| others.len() > 1)
        && modern != "key.keyboard.unknown"
    {
        let text = format!(
            "Also used by: {}",
            others
                .iter()
                .filter(|other| other.as_str() != label)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = text_ui.label(
            ui,
            ("keybind_conflict", key),
            text.as_str(),
            &style::warning_text(ui),
        );
    }

    if state.capturing.as_deref() == Some(key)
        && ui.ctx().cumulative_frame_nr() > state.capture_started_at_frame
        && let Some(captured) = key_capture::poll_capture(ui.ctx())
    {
        if let key_capture::Captured::Binding(name) = captured {
            store(live, &name);
        }
        state.capturing = None;
    }
}

fn render_def_row(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    live: &mut LiveOptions,
    def: &OptionDef,
    version: Option<Ver>,
    audio_devices: Option<&Mutex<Option<Vec<String>>>>,
) {
    let shown = shown_value(live, def).to_owned();
    let tip = tooltip_for(def);
    let tip = Some(tip.as_str());
    match def.kind {
        Kind::Bool => {
            let mut on = value::parse_bool(&shown).unwrap_or(def.default == "true");
            let before = on;
            let _ = settings_widgets::toggle_row(text_ui, ui, def.label, tip, &mut on);
            if on != before {
                commit(live, def, version, &on.to_string());
            }
        }
        Kind::Int { min, max } => {
            let mut number = value::parse_i64(&shown)
                .or_else(|| def.default.parse().ok())
                .unwrap_or(min)
                .clamp(min, max) as i32;
            let before = number;
            let _ = settings_widgets::int_stepper_row(
                text_ui,
                ui,
                ("game_setting", def.key),
                def.label,
                tip,
                &mut number,
                min as i32,
                max as i32,
                1,
            );
            if number != before {
                commit(live, def, version, &number.to_string());
            }
        }
        Kind::Float { min, max } => {
            let stored =
                value::parse_f64(&shown).unwrap_or_else(|| def.default.parse().unwrap_or(min));
            let before;
            let mut edited;
            match def.display {
                Display::Degrees { offset, scale } => {
                    before = (offset + stored * scale) as f32;
                    edited = before;
                    let _ = settings_widgets::float_stepper_row(
                        text_ui,
                        ui,
                        ("game_setting", def.key),
                        def.label,
                        tip,
                        &mut edited,
                        (offset + min * scale) as f32,
                        (offset + max * scale) as f32,
                        1.0,
                    );
                    if edited != before {
                        commit(
                            live,
                            def,
                            version,
                            &format!("{}", (f64::from(edited) - offset) / scale),
                        );
                    }
                }
                Display::Percent if min == 0.0 && max == 1.0 => {
                    before = stored as f32;
                    edited = before;
                    let _ = settings_widgets::float_slider_row(
                        text_ui,
                        ui,
                        ("game_setting", def.key),
                        def.label,
                        tip,
                        &mut edited,
                        0.0,
                        1.0,
                        true,
                    );
                    if edited != before {
                        commit(live, def, version, &format!("{edited}"));
                    }
                }
                _ => {
                    before = stored as f32;
                    edited = before;
                    let _ = settings_widgets::float_stepper_row(
                        text_ui,
                        ui,
                        ("game_setting", def.key),
                        def.label,
                        tip,
                        &mut edited,
                        min as f32,
                        max as f32,
                        ((max - min) / 100.0).max(0.01) as f32,
                    );
                    if edited != before {
                        commit(live, def, version, &format!("{edited}"));
                    }
                }
            }
        }
        Kind::Choice(choices) => {
            let labels = choices
                .iter()
                .map(|(stored, label)| ((*stored).to_owned(), (*label).to_owned()))
                .collect();
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::IntChoice(choices) => {
            let labels = choices
                .iter()
                .map(|(stored, label)| (stored.to_string(), (*label).to_owned()))
                .collect();
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::Text if def.key == "lang" => {
            let labels = language_choices(&shown);
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::Text if def.key == "soundDevice" => {
            let devices = audio_devices.and_then(|slot| slot.lock().ok()?.clone());
            let labels = sound_device_choices(&shown, devices.as_deref());
            render_choice_row(ui, text_ui, live, def, version, labels);
        }
        Kind::Text => {
            let mut text = shown.clone();
            let _ = settings_widgets::text_input_row(
                text_ui,
                ui,
                ("game_setting", def.key),
                def.label,
                tip,
                &mut text,
            );
            if text != shown {
                commit(live, def, version, &text);
            }
        }
        Kind::List => {
            let items = value::parse_list(live.get(def.key).unwrap_or(def.default));
            let summary = if items.is_empty() {
                "none".to_owned()
            } else {
                items.join(", ")
            };
            let _ = text_ui.label(
                ui,
                ("game_setting_list", def.key),
                format!("{}: {summary}", def.label).as_str(),
                &style::muted(ui),
            );
        }
        Kind::Keybind => {}
    }
    ui.add_space(style::SPACE_SM);
}

/// Languages for the dropdown. The stored code keeps whatever case the file uses (older versions
/// write `en_US`), so the current language is selected regardless of case.
fn language_choices(current: &str) -> Vec<(String, String)> {
    mc_options::languages::LANGUAGES
        .iter()
        .map(|(code, name)| {
            let stored = if code.eq_ignore_ascii_case(current) {
                current.to_owned()
            } else {
                (*code).to_owned()
            };
            (stored, format!("{name} ({code})"))
        })
        .collect()
}

/// Output devices for the dropdown: the system default, every device found, and whatever the file
/// currently holds. Minecraft stores `OpenAL Soft on <name>`; a bare name is kept bare.
fn sound_device_choices(current: &str, devices: Option<&[String]>) -> Vec<(String, String)> {
    let bare_current = current
        .strip_prefix(audio_devices::OPENAL_PREFIX)
        .unwrap_or(current);
    let prefixed = current.is_empty() || current.starts_with(audio_devices::OPENAL_PREFIX);
    let mut choices = vec![(String::new(), "System Default".to_owned())];
    let mut found_current = current.is_empty();
    for name in devices.unwrap_or_default() {
        let stored = if name == bare_current {
            found_current = true;
            current.to_owned()
        } else if prefixed {
            format!("{}{name}", audio_devices::OPENAL_PREFIX)
        } else {
            name.clone()
        };
        choices.push((stored, name.clone()));
    }
    if !found_current {
        // Not currently plugged in (or not detectable): keep it selectable.
        choices.push((current.to_owned(), bare_current.to_owned()));
    }
    choices
}

/// Key → labels of every binding using it, to flag duplicates.
fn keybind_usage(live: &LiveOptions, version: Option<Ver>) -> BTreeMap<String, Vec<String>> {
    let legacy = version.is_some_and(|v| v < Ver(1, 13, 0));
    let mut usage: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in live.file().keys() {
        if !schema::is_keybind_key(key) {
            continue;
        }
        let raw = live.get(key).unwrap_or_default();
        let modern = if legacy {
            raw.parse::<i32>().ok().and_then(keys::legacy_to_modern)
        } else {
            Some(raw.to_owned())
        };
        if let Some(modern) = modern {
            let label = schema::find_def(key, version)
                .map_or_else(|| pretty_key(key), |d| d.label.to_owned());
            usage.entry(modern).or_default().push(label);
        }
    }
    usage
}

fn pretty_key(key: &str) -> String {
    key.trim_start_matches("key_key.")
        .trim_start_matches("key_")
        .replace(['.', '_'], " ")
}

fn matches_search(search: &str, label: &str, key: &str) -> bool {
    let needle = search.trim().to_lowercase();
    needle.is_empty()
        || label.to_lowercase().contains(&needle)
        || key.to_lowercase().contains(&needle)
}

/// Like [`matches_search`], but also looks in the option's description, so people can search for
/// what a setting does ("shadows", "gamma") and not only for its name.
fn def_matches_search(search: &str, def: &OptionDef) -> bool {
    let needle = search.trim().to_lowercase();
    needle.is_empty()
        || matches_search(search, def.label, def.key)
        || mc_options::descriptions::description(def.key)
            .is_some_and(|text| text.to_lowercase().contains(&needle))
}

/// Draws a group's heading the first time something under it is shown.
fn heading_once(ui: &mut egui::Ui, text_ui: &mut TextUi, heading: &mut Option<&str>) {
    if let Some(text) = heading.take() {
        ui.add_space(style::SPACE_MD);
        let _ = text_ui.label(
            ui,
            ("game_settings_heading", text),
            text,
            &style::subtitle(ui),
        );
        ui.add_space(style::SPACE_SM);
    }
}

fn render_group(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
    group: Group,
    mut heading: Option<&str>,
) -> usize {
    let version = state.version;
    let search = state.search.clone();
    let defs: Vec<&'static OptionDef> = schema::defs_for(version)
        .filter(|def| !def.hidden && def.group == group && def_matches_search(&search, def))
        .collect();
    let mut rendered = 0usize;

    if group == Group::Keybinds {
        let usage = keybind_usage(live, version);
        let mut shown = std::collections::HashSet::new();
        for def in &defs {
            shown.insert(def.key);
            heading_once(ui, text_ui, &mut heading);
            rendered += 1;
            render_keybind_row(
                ui,
                text_ui,
                state,
                live,
                def.key,
                def.label,
                def.default,
                &usage,
            );
        }
        // Binds from mods (and other versions) that the file already contains.
        let extra: Vec<String> = live
            .file()
            .keys()
            .into_iter()
            .filter(|key| schema::is_keybind_key(key) && !shown.contains(key))
            .filter(|key| matches_search(&search, &pretty_key(key), key))
            .map(str::to_owned)
            .collect();
        if !extra.is_empty() {
            heading_once(ui, text_ui, &mut heading);
            rendered += extra.len();
            ui.add_space(style::SPACE_LG);
            let _ = text_ui.label(
                ui,
                "keybinds_modded",
                "Mod & other key binds",
                &style::subtitle(ui),
            );
            for key in extra {
                let label = pretty_key(&key);
                render_keybind_row(
                    ui,
                    text_ui,
                    state,
                    live,
                    &key,
                    &label,
                    "key.keyboard.unknown",
                    &usage,
                );
            }
        }
        return rendered;
    }

    let audio_devices = Arc::clone(&state.audio_devices);
    for def in defs {
        heading_once(ui, text_ui, &mut heading);
        rendered += 1;
        render_def_row(ui, text_ui, live, def, version, Some(&*audio_devices));
    }

    if group == Group::Internal {
        // Keys this launcher has no definition for: shown raw so nothing is hidden.
        let unknown: Vec<String> = live
            .file()
            .keys()
            .into_iter()
            .filter(|key| !schema::is_keybind_key(key) && schema::find_def(key, version).is_none())
            .filter(|key| !key.starts_with("soundCategory_") || schema::find_def_any(key).is_none())
            .filter(|key| matches_search(&search, key, key))
            .map(str::to_owned)
            .collect();
        if !unknown.is_empty() {
            heading_once(ui, text_ui, &mut heading);
            rendered += unknown.len();
            ui.add_space(style::SPACE_LG);
            let _ = text_ui.label(
                ui,
                "other_keys_title",
                "Other keys (mods and other versions)",
                &style::subtitle(ui),
            );
            for key in unknown {
                let current = live.get(&key).unwrap_or_default().to_owned();
                let mut text = current.clone();
                let _ = settings_widgets::text_input_row(
                    text_ui,
                    ui,
                    ("game_setting_raw", key.as_str()),
                    key.as_str(),
                    Some("Not a setting this launcher knows; edited as raw text."),
                    &mut text,
                );
                if text != current {
                    live.edit(&key, Some(&text));
                }
                ui.add_space(style::SPACE_SM);
            }
        }
    }
    rendered
}

/// Everything matching the search, across every tab, with a heading per section.
fn render_search_results(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &mut ModalState,
    live: &mut LiveOptions,
) {
    let mut total = 0usize;
    for group in Group::ALL {
        total += render_group(ui, text_ui, state, live, group, Some(group.label()));
    }
    if total == 0 {
        let _ = text_ui.label(
            ui,
            "game_settings_no_matches",
            "No settings match your search.",
            &style::muted(ui),
        );
    }
}

fn render_sync_tab(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &ModalState,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let _ = text_ui.label(
        ui,
        "settings_sync_intro",
        "Keep these settings in step across instances. The most recently changed instance wins; its settings are translated to each other instance's Minecraft version (renamed options, key codes, value formats). Settings that belong to this machine, like the window size or audio device, are never copied, and running instances are left alone until they close.",
        &style::muted(ui),
    );
    ui.add_space(style::SPACE_LG);

    let mut member = instances
        .game_settings_sync
        .instances
        .contains(&state.instance_id);
    let before = member;
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Sync this instance's settings",
        Some("Instances with this turned on share the settings chosen below."),
        &mut member,
    );
    if member != before {
        if member {
            instances
                .game_settings_sync
                .instances
                .insert(state.instance_id.clone());
        } else {
            instances
                .game_settings_sync
                .instances
                .remove(&state.instance_id);
        }
    }
    let participants = instances.game_settings_sync.instances.len();
    let _ = text_ui.label(
        ui,
        "settings_sync_count",
        if participants < 2 {
            "Turn this on for at least two instances for anything to sync.".to_owned()
        } else {
            format!("{participants} instances take part.")
        }
        .as_str(),
        &style::muted(ui),
    );

    ui.add_space(style::SPACE_LG);
    let _ = text_ui.label(
        ui,
        "settings_sync_groups",
        "Settings to sync",
        &style::subtitle(ui),
    );
    for group in Group::ALL
        .into_iter()
        .filter(|g| !matches!(g, Group::Online | Group::Internal))
    {
        let mut on = instances
            .game_settings_sync
            .categories
            .contains(group.slug());
        let before = on;
        let _ = settings_widgets::toggle_row(text_ui, ui, group.label(), None, &mut on);
        if on != before {
            if on {
                instances
                    .game_settings_sync
                    .categories
                    .insert(group.slug().to_owned());
            } else {
                instances.game_settings_sync.categories.remove(group.slug());
            }
        }
    }

    ui.add_space(style::SPACE_LG);
    let _ = text_ui.label(
        ui,
        "settings_sync_mods",
        "Mod settings to sync",
        &style::subtitle(ui),
    );
    let _ = text_ui.label(
        ui,
        "settings_sync_mods_help",
        "Only instances that already have the mod's config file take part. Thread counts, server identity and other machine-specific keys are skipped.",
        &style::muted(ui),
    );
    for (id, name) in instance_sync::available_mod_configs() {
        let mut on = instances.game_settings_sync.mod_configs.contains(id);
        let before = on;
        let _ = settings_widgets::toggle_row(text_ui, ui, name, None, &mut on);
        if on != before {
            if on {
                instances
                    .game_settings_sync
                    .mod_configs
                    .insert(id.to_owned());
            } else {
                instances.game_settings_sync.mod_configs.remove(id);
            }
        }
    }

    ui.add_space(style::SPACE_LG);
    let mut packs = instances.game_settings_sync.resource_packs;
    let before = packs;
    let _ = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Sync resource packs",
        Some(
            "Shares packs between these instances where their metadata says they work (or you've said they do), and keeps which packs are enabled in step. Manage per-pack exceptions on the Resource Packs tab.",
        ),
        &mut packs,
    );
    if packs != before {
        instances.game_settings_sync.resource_packs = packs;
    }

    ui.add_space(style::SPACE_LG);
    if text_ui
        .button(
            ui,
            "settings_sync_now",
            "Sync now",
            &style::neutral_button_with_min_size(ui, egui::vec2(140.0, style::CONTROL_HEIGHT)),
        )
        .clicked()
    {
        sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
    }
}

/// Renders the dialog if open, keeping it in step with the file.
pub(super) fn render_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let key = egui::Id::new(REQUEST_KEY);
    let Some(mut state) = ctx.data(|data| data.get_temp::<ModalState>(key)) else {
        return;
    };
    let live_handle = Arc::clone(&state.live);
    let Ok(mut live) = live_handle.lock() else {
        return;
    };

    // Follow the file, and flush edits that have been idle long enough.
    let polled = live.poll(SAVE_DELAY);
    if polled == Poll::Saved
        && instances
            .game_settings_sync
            .instances
            .contains(&state.instance_id)
    {
        // Propagate right away instead of waiting for the next launch or exit.
        sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
    }
    ctx.request_repaint_after(
        live.time_until_save(SAVE_DELAY)
            .map_or(WATCH_INTERVAL, |wait| wait.min(WATCH_INTERVAL)),
    );

    let running = instances
        .find(&state.instance_id)
        .map(|instance| instance_root_path(config.minecraft_installations_root_path(), instance))
        .is_some_and(|root| installation::is_instance_running(root.as_path()));

    let mut close = false;
    let response = modal::show_window(
        ctx,
        "Game settings",
        modal::ModalOptions::new(
            egui::Id::new(("game_settings_modal_window", state.instance_id.as_str())),
            modal::ModalLayout::centered(
                // No upper cap: it grows with the window, keeping the host's minimum padding.
                modal::AxisSizing::new(0.92, 520.0, f32::INFINITY),
                modal::AxisSizing::new(0.92, 420.0, f32::INFINITY),
            ),
        )
        .with_layer(modal::ModalLayer::Base)
        // The body has its own vertical scroll; an outer one would fight it for the wheel.
        .with_content_scroll(false)
        .with_dismiss_behavior(modal::DismissBehavior::EscapeAndScrim),
        |ui| {
            let _ = text_ui.label(
                ui,
                "game_settings_title",
                "Game settings",
                &style::modal_title(ui),
            );
            let subtitle = format!(
                "{} · Minecraft {} · {}",
                state.instance_name,
                state.game_version,
                live.path().display()
            );
            let _ = text_ui.label(
                ui,
                "game_settings_path",
                subtitle.as_str(),
                &style::muted(ui),
            );
            if running {
                let _ = text_ui.label(
                    ui,
                    "game_settings_running",
                    "Minecraft is running. The game rewrites options.txt when you leave its options screens, so changes here can be overwritten; changes made in-game appear here as they are saved.",
                    &style::warning_text(ui),
                );
            }
            if let Some(error) = live.last_error() {
                let _ = text_ui.label(
                    ui,
                    "game_settings_error",
                    format!("Couldn't save: {error}. Will retry.").as_str(),
                    &style::error_text(ui),
                );
            }
            let status = if live.has_pending() {
                "Saving…"
            } else {
                "All changes saved"
            };
            let _ = text_ui.label(ui, "game_settings_status", status, &style::caption(ui));
            ui.add_space(style::SPACE_MD);

            // Tabs: one row in its own horizontally scrolling strip, so a narrow dialog scrolls the
            // tabs instead of forcing the whole dialog to be as wide as the row.
            egui::ScrollArea::horizontal()
                .scroll_source(modal_host::drag_scroll_source())
                .id_salt("game_settings_tabs_scroll")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for group in Group::ALL {
                            let selected = state.tab == Tab::Group(group);
                            if settings_widgets::selectable_chip_button(
                                text_ui,
                                ui,
                                ("game_settings_tab", group.slug()),
                                group.label(),
                                selected,
                                118.0,
                                true,
                            )
                            .clicked()
                            {
                                state.tab = Tab::Group(group);
                            }
                        }
                        if settings_widgets::selectable_chip_button(
                            text_ui,
                            ui,
                            "game_settings_tab_packs",
                            "Resource Packs",
                            state.tab == Tab::Packs,
                            150.0,
                            true,
                        )
                        .clicked()
                        {
                            state.tab = Tab::Packs;
                        }
                        if settings_widgets::selectable_chip_button(
                            text_ui,
                            ui,
                            "game_settings_tab_sync",
                            "Sync",
                            state.tab == Tab::Sync,
                            118.0,
                            true,
                        )
                        .clicked()
                        {
                            state.tab = Tab::Sync;
                        }
                    });
                });
            ui.add_space(style::SPACE_MD);

            {
                let mut search = state.search.clone();
                let _ = text_ui.singleline_input(ui, "game_settings_search", &mut search, &{
                    let mut options = style::input_options(ui);
                    options.placeholder_text = Some("Search all settings".to_owned());
                    options.desired_width = Some(ui.available_width());
                    options.desired_rows = 1;
                    options
                });
                state.search = search;
                ui.add_space(style::SPACE_MD);
            }

            // Leave room for the Close row below; otherwise the scroll area claims all the
            // remaining height and pushes Close out of the window.
            let footer_height = style::SPACE_MD * 2.0 + style::CONTROL_HEIGHT;
            egui::ScrollArea::vertical()
                .scroll_source(modal_host::drag_scroll_source())
                .id_salt("game_settings_scroll")
                .max_height((ui.available_height() - footer_height).max(80.0))
                .auto_shrink([false, false])
                .show(ui, |ui| match state.tab {
                    _ if !state.search.trim().is_empty() => {
                        let mut tab_state = state.clone();
                        render_search_results(ui, text_ui, &mut tab_state, &mut live);
                        state.capturing = tab_state.capturing;
                        state.capture_started_at_frame = tab_state.capture_started_at_frame;
                    }
                    Tab::Sync => render_sync_tab(ui, text_ui, &state, instances, config),
                    Tab::Packs => render_packs_tab(ui, text_ui, &state, instances, config),
                    Tab::Group(group) => {
                        let mut tab_state = state.clone();
                        render_group(ui, text_ui, &mut tab_state, &mut live, group, None);
                        state.capturing = tab_state.capturing;
                        state.capture_started_at_frame = tab_state.capture_started_at_frame;
                    }
                });

            ui.add_space(style::SPACE_MD);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if text_ui
                    .button(
                        ui,
                        "game_settings_close",
                        "Close",
                        &ui_foundation::secondary_button(
                            ui,
                            egui::vec2(110.0, style::CONTROL_HEIGHT),
                        ),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        },
    );

    let done = close || response.close_requested;
    if done {
        // Save anything still pending before the view goes away.
        let _ = live.flush();
    }
    drop(live);
    if done {
        ctx.data_mut(|data| data.remove::<ModalState>(key));
    } else {
        ctx.data_mut(|data| data.insert_temp(key, state));
    }
}

fn collect_pack_rows(
    state: &ModalState,
    instances: &InstanceStore,
    config: &Config,
) -> Vec<PackRow> {
    let root_of = |instance: &InstanceRecord| {
        instance_root_path(config.minecraft_installations_root_path(), instance)
    };
    let mut rows: BTreeMap<String, PackRow> = BTreeMap::new();
    let Some(this) = instances.find(&state.instance_id) else {
        return Vec::new();
    };
    for pack in scan_packs(&root_of(this)) {
        rows.insert(
            pack.name.clone(),
            PackRow {
                name: pack.name,
                support: pack.support,
                present_here: true,
                elsewhere: Vec::new(),
            },
        );
    }
    // Packs other syncing instances have, so they can be vouched for before they arrive.
    for other in &instances.instances {
        if other.id == state.instance_id
            || !instances.game_settings_sync.instances.contains(&other.id)
        {
            continue;
        }
        for pack in scan_packs(&root_of(other)) {
            let row = rows.entry(pack.name.clone()).or_insert_with(|| PackRow {
                name: pack.name.clone(),
                support: pack.support,
                present_here: false,
                elsewhere: Vec::new(),
            });
            row.elsewhere.push(other.name.clone());
        }
    }
    let mut rows: Vec<PackRow> = rows.into_values().collect();
    rows.sort_by_key(|row| row.name.to_lowercase());
    rows
}

fn status_text(status: PackStatus, support: Option<PackSupport>, version: Option<Ver>) -> String {
    let format = version.and_then(mc_options::packs::resource_format_for);
    let format_note = format.map(|f| {
        format!(
            "Minecraft {} uses format {}.{}",
            version.map_or(String::new(), |v| format!("{}.{}", v.0, v.1)),
            f.major,
            f.minor
        )
    });
    match status {
        PackStatus::Compatible => "Compatible with this version".to_owned(),
        PackStatus::Incompatible => format!(
            "Not declared for this version{}",
            format_note.map(|n| format!(" ({n})")).unwrap_or_default()
        ),
        PackStatus::Unknown => {
            if support.is_none() {
                "No readable pack.mcmeta; compatibility unknown".to_owned()
            } else {
                "Compatibility unknown for this version".to_owned()
            }
        }
        PackStatus::ForcedAllow => "Marked as working here".to_owned(),
        PackStatus::ForcedDeny => "Marked as not working here".to_owned(),
    }
}

fn render_packs_tab(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &ModalState,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let _ = text_ui.label(
        ui,
        "packs_intro",
        "Each pack declares which Minecraft versions it supports. You know your setup better than the metadata: mark a pack as working (or not) for this instance and sync will follow that instead. Overrides only affect sync; they don't change the pack or what the game allows.",
        &style::muted(ui),
    );
    if !instances.game_settings_sync.resource_packs {
        let _ = text_ui.label(
            ui,
            "packs_sync_off",
            "Resource pack sync is off. Turn it on in the Sync tab; overrides are remembered either way.",
            &style::warning_text(ui),
        );
    }
    ui.add_space(style::SPACE_LG);

    let rows = {
        let mut cache = state.pack_rows.lock().expect("pack row cache poisoned");
        let stale = cache
            .as_ref()
            .is_none_or(|(when, _)| when.elapsed() > Duration::from_secs(1));
        if stale {
            *cache = Some((Instant::now(), collect_pack_rows(state, instances, config)));
        }
        cache
            .as_ref()
            .map(|(_, rows)| rows.clone())
            .unwrap_or_default()
    };
    if rows.is_empty() {
        let _ = text_ui.label(
            ui,
            "packs_empty",
            "No resource packs found.",
            &style::muted(ui),
        );
        return;
    }

    let mut changed = false;
    for row in rows {
        let status = pack_status(
            instances,
            &state.instance_id,
            &state.game_version,
            &row.name,
            row.support,
        );
        let verdict = instances.pack_override(&row.name, &state.instance_id);
        let _ = text_ui.label(
            ui,
            ("pack_name", row.name.as_str()),
            row.name.as_str(),
            &style::body_strong(ui),
        );
        let declared = row
            .support
            .map_or_else(|| "none".to_owned(), |s| s.serialize());
        let location = if row.present_here {
            "In this instance".to_owned()
        } else {
            format!("Only in {}", row.elsewhere.join(", "))
        };
        let detail = format!(
            "{location} · declared formats: {declared} · {}",
            status_text(status, row.support, state.version)
        );
        let _ = text_ui.label(
            ui,
            ("pack_detail", row.name.as_str()),
            detail.as_str(),
            &style::muted(ui),
        );
        ui.horizontal(|ui| {
            for (label, target) in [
                ("Works here", Some(PackOverride::Allow)),
                ("Doesn't work here", Some(PackOverride::Deny)),
                ("Use pack's metadata", None),
            ] {
                let selected = verdict == target;
                if settings_widgets::selectable_chip_button(
                    text_ui,
                    ui,
                    ("pack_override", row.name.as_str(), label),
                    label,
                    selected,
                    170.0,
                    true,
                )
                .clicked()
                    && !selected
                {
                    instances.set_pack_override(&row.name, &state.instance_id, target);
                    changed = true;
                }
            }
        });
        ui.add_space(style::SPACE_MD);
    }
    if changed {
        *state.pack_rows.lock().expect("pack row cache poisoned") = None;
        if instances.game_settings_sync.resource_packs {
            sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
        }
    }
}
