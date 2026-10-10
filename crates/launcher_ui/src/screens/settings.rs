use config::{
    Config, DOWNLOAD_CONCURRENCY_MAX, DOWNLOAD_CONCURRENCY_MIN, DropdownSettingId, FloatSettingId,
    GraphicsAdapterPreferenceType, GraphicsAdapterProfile, GraphicsApiPreference,
    INSTANCE_DEFAULT_MAX_MEMORY_MIB_MIN, INSTANCE_DEFAULT_MAX_MEMORY_MIB_STEP, IntSettingId,
    JavaRuntimeVersion, NotificationEasing, SkinPreviewAaMode, SkinPreviewTexelAaMode, SvgAaMode,
    TextRenderingPath, UiEmojiFontFamily, UiFontFamily, WindowTransparency, parse_bitrate_to_bps,
};
use egui::Ui;
use installation::{ensure_openjdk_runtime_async, purge_cache_async as purge_installation_cache};
use launcher_runtime as tokio_runtime;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::Duration;

struct PurgeCacheState {
    rx: Option<mpsc::Receiver<String>>,
}

use textui::TextUi;
use textui_egui::{gamepad_scroll, prelude::*};

use super::{SettingsInfo, platform};
use crate::ui::{components::settings_widgets, style, theme::Theme};

const RESERVED_SYSTEM_MEMORY_MIB: u128 = 4 * 1024;
const FALLBACK_TOTAL_MEMORY_MIB: u128 = 20 * 1024;
const FORCE_THEME_FOCUS_ID: &str = "settings_force_theme_focus";

#[path = "settings/memory_slider_max_state.rs"]
mod memory_slider_max_state;

#[path = "settings/java_runtime.rs"]
mod java_runtime;

#[path = "settings/text_settings.rs"]
mod text_settings;

#[path = "settings/graphics_settings.rs"]
mod graphics_settings;

#[path = "settings/appearance_settings.rs"]
mod appearance_settings;

#[path = "settings/privacy_settings.rs"]
mod privacy_settings;

#[path = "settings/instance_defaults.rs"]
mod instance_defaults;

use self::appearance_settings::*;
use self::graphics_settings::*;
use self::instance_defaults::*;
use self::java_runtime::render_java_runtime_settings;
use self::memory_slider_max_state::MemorySliderMaxState;
use self::privacy_settings::*;
use self::text_settings::*;

pub fn render(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    options: &super::AvailableOptions<'_>,
) {
    gamepad_scroll(
        egui::ScrollArea::vertical()
            .scroll_source(modal_host::drag_scroll_source())
            .id_salt("settings_page_scroll")
            .auto_shrink([false, false]),
        ui,
        |ui| {
            render_settings_contents(ui, text_ui, config, options);
        },
    );
    render_insecure_storage_confirm(ui.ctx(), text_ui, config);
}

pub fn prewarm(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    config: &Config,
    options: &super::AvailableOptions<'_>,
) {
    let mut prewarm_config = config.clone();
    let width = ctx.content_rect().width().max(960.0);
    egui::Area::new(egui::Id::new("settings_screen_prewarm"))
        .fixed_pos(egui::pos2(-20_000.0, -20_000.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.set_min_width(width);
            render_settings_contents(ui, text_ui, &mut prewarm_config, options);
        });
}

pub fn request_theme_focus(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(FORCE_THEME_FOCUS_ID), true));
    settings_widgets::request_default_focus(ctx, true);
}

fn render_settings_contents(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    options: &super::AvailableOptions<'_>,
) {
    let super::AvailableOptions {
        ui_fonts: available_ui_fonts,
        ui_font_labels: available_ui_font_labels,
        emoji_fonts: available_emoji_fonts,
        emoji_font_labels: available_emoji_font_labels,
        themes: available_themes,
        theme_labels: available_theme_labels,
        settings_info,
    } = options;
    render_settings_section(
        ui,
        text_ui,
        "Appearance",
        "Theme and launcher chrome settings.",
        |ui, text_ui| {
            render_theme_setting(
                ui,
                text_ui,
                config,
                available_themes,
                available_theme_labels,
            );
            render_window_transparency_setting(ui, text_ui, config);
            render_ui_opacity_setting(ui, text_ui, config);
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::DragScrollEnabled],
            );
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Privacy",
        "On-stream safety and account security settings.",
        |ui, text_ui| {
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::StreamerModeEnabled],
            );
            render_insecure_storage_setting(ui, text_ui, config);
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Notifications",
        "How notification popups expire and fade away.",
        |ui, text_ui| {
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::NotificationExpiryBarsEmptyLeft],
            );
            render_selected_float_settings(
                ui,
                text_ui,
                config,
                &[FloatSettingId::NotificationFadeOutSeconds],
            );
            render_notification_easing_setting(ui, text_ui, config);
            if text_ui
                .button(
                    ui,
                    "settings_test_notification",
                    "Test notification",
                    &style::neutral_button(ui),
                )
                .clicked()
            {
                crate::notification::emit(
                    crate::notification::Severity::Info,
                    "Settings",
                    "This is a test notification.",
                );
            }
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Text",
        "Font, shaping, and glyph rendering settings that affect launcher text.",
        |ui, text_ui| {
            render_ui_font_settings(
                ui,
                text_ui,
                config,
                available_ui_fonts,
                available_ui_font_labels,
            );
            render_emoji_font_settings(
                ui,
                text_ui,
                config,
                available_emoji_fonts,
                available_emoji_font_labels,
            );
            render_text_rendering_path_setting(ui, text_ui, config);
            render_typography_settings(
                ui,
                text_ui,
                config,
                available_ui_fonts,
                available_ui_font_labels,
            );
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::OpenTypeFeaturesEnabled],
            );
            render_selected_text_settings(
                ui,
                text_ui,
                config,
                &[config::TextSettingId::OpenTypeFeaturesToEnable],
            );
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Discord Rich Presence",
        "Discord integration and launcher-owned activity reporting.",
        |ui, text_ui| {
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::DiscordRichPresenceEnabled],
            );
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Instance Sync",
        "Keep multiplayer servers, command history, and creative hotbars in step across your instances. Sync runs before an instance launches and after it closes; right-click a server on Home to choose which instances it goes to.",
        |ui, text_ui| {
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[
                    config::ToggleSettingId::SyncServersEnabled,
                    config::ToggleSettingId::SyncServersToAllInstancesByDefault,
                    config::ToggleSettingId::SyncCommandHistoryEnabled,
                    config::ToggleSettingId::SyncHotbarsEnabled,
                ],
            );
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Graphics & Performance",
        "GPU, skin preview rendering, frame pacing, and download throughput behavior.",
        |ui, text_ui| {
            render_skin_preview_setting(ui, text_ui, config);
            render_svg_aa_setting(ui, text_ui, config);
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[
                    config::ToggleSettingId::SkinPreviewFreshFormatEnabled,
                    config::ToggleSettingId::SkinPreview3dLayersEnabled,
                ],
            );
            render_skin_preview_motion_blur_settings(ui, text_ui, config);
            render_graphics_adapter_settings(ui, text_ui, config, settings_info);
            render_graphics_api_setting(ui, text_ui, config);
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::FrameLimiterEnabled],
            );
            render_download_settings(ui, text_ui, config);
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Minecraft",
        "Version catalog behavior and content provider access.",
        |ui, text_ui| {
            render_settings_subgroup(
                ui,
                text_ui,
                "Minecraft Version Groups",
                "Controls which non-default Minecraft version categories appear in version pickers.",
            );
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[
                    config::ToggleSettingId::SnapshotsAndBetasEnabled,
                    config::ToggleSettingId::AlphaVersionsEnabled,
                    config::ToggleSettingId::ExperimentalVersionsEnabled,
                ],
            );
            render_curseforge_settings(ui, text_ui, config);
        },
    );

    render_settings_section(
        ui,
        text_ui,
        "Java",
        "Shared Java behavior for launches that use managed runtime selection.",
        |ui, text_ui| {
            render_selected_toggles(
                ui,
                text_ui,
                config,
                &[config::ToggleSettingId::ForceJava21Minimum],
            );
            render_java_runtime_settings(ui, text_ui, config);
        },
    );

    render_instance_defaults_section(ui, text_ui, config);
    render_platform_specific_settings_section(ui, text_ui, config);
    render_info_section(ui, text_ui, settings_info);
}

fn render_info_section(ui: &mut Ui, text_ui: &mut TextUi, settings_info: &SettingsInfo) {
    render_settings_section(
        ui,
        text_ui,
        "Info",
        "Runtime and hardware information for this installation.",
        |ui, text_ui| {
            let mut key_style = style::muted(ui);
            key_style.weight = 700;
            key_style.wrap = false;

            let value_style = style::code(ui);

            egui::Grid::new("settings_info_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    render_info_row(
                        text_ui,
                        ui,
                        "CPU",
                        &settings_info.cpu,
                        &key_style,
                        &value_style,
                    );
                    render_info_row(
                        text_ui,
                        ui,
                        "GPU",
                        &settings_info.gpu,
                        &key_style,
                        &value_style,
                    );
                    render_info_row(
                        text_ui,
                        ui,
                        "Memory",
                        &settings_info.memory,
                        &key_style,
                        &value_style,
                    );
                    render_info_row(
                        text_ui,
                        ui,
                        "Graphics API",
                        &settings_info.graphics_api,
                        &key_style,
                        &value_style,
                    );
                    render_info_row(
                        text_ui,
                        ui,
                        "Graphics Driver + Version",
                        &settings_info.graphics_driver,
                        &key_style,
                        &value_style,
                    );
                    render_info_row(
                        text_ui,
                        ui,
                        "App Version + Commit Hash",
                        &settings_info.app_version,
                        &key_style,
                        &value_style,
                    );
                });
        },
    );
}

fn render_platform_specific_settings_section(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let Some(section) = platform::current_platform_specific_section() else {
        return;
    };

    render_settings_section(
        ui,
        text_ui,
        section.heading,
        section.launcher_description,
        |ui, text_ui| platform::render_launcher_platform_settings(ui, text_ui, config),
    );
}

fn render_info_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    label: &str,
    value: &str,
    key_style: &LabelOptions,
    value_style: &LabelOptions,
) {
    let _ = text_ui.label(ui, ("settings_info_label", label), label, key_style);
    let _ = text_ui.label(ui, ("settings_info_value", label), value, value_style);
    ui.end_row();
}

fn render_settings_section(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    heading: &str,
    description: &str,
    render_body: impl FnOnce(&mut Ui, &mut TextUi),
) {
    let heading_style = style::section_heading(ui);
    let mut body_style = style::muted(ui);
    body_style.wrap = true;

    ui.add_space(style::SPACE_XL);
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_MD))
        .inner_margin(egui::Margin::same(style::SPACE_XL as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = style::SPACE_MD;
            let _ = text_ui.label(ui, ("settings_heading", heading), heading, &heading_style);
            let _ = text_ui.label(
                ui,
                ("settings_description", heading),
                description,
                &body_style,
            );
            ui.add_space(style::SPACE_MD);
            render_body(ui, text_ui);
        });
}

fn render_settings_subgroup(ui: &mut Ui, text_ui: &mut TextUi, heading: &str, description: &str) {
    let mut heading_style = style::body(ui);
    heading_style.weight = 700;
    heading_style.wrap = false;

    let mut description_style = style::muted(ui);
    description_style.wrap = true;

    let _ = text_ui.label(
        ui,
        ("settings_subgroup_heading", heading),
        heading,
        &heading_style,
    );
    let _ = text_ui.label(
        ui,
        ("settings_subgroup_description", heading),
        description,
        &description_style,
    );
    ui.add_space(style::SPACE_MD);
}

fn render_selected_float_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    ids: &[FloatSettingId],
) {
    config.for_each_float_mut(|setting, value| {
        if !ids.contains(&setting.id) {
            return;
        }
        ui.push_id(setting.id, |ui| {
            settings_widgets::float_stepper_row(
                text_ui,
                ui,
                setting.id,
                setting.label,
                setting.info_tooltip,
                value,
                setting.min,
                setting.max,
                setting.step,
            );
        });
        ui.add_space(style::SPACE_MD);
    });
}

fn render_selected_toggles(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    selected_ids: &[config::ToggleSettingId],
) {
    let mut frame_limit_fps = config.frame_limit_fps();
    config.for_each_toggle_mut(|setting, value| {
        if !selected_ids.contains(&setting.id) {
            return;
        }
        ui.push_id(setting.id, |ui| {
            settings_widgets::toggle_row(text_ui, ui, setting.label, setting.info_tooltip, value);
        });
        if setting.id == config::ToggleSettingId::FrameLimiterEnabled && *value {
            let spec = IntSettingId::FrameLimitFps.spec();
            ui.push_id(spec.id, |ui| {
                let _ = settings_widgets::int_stepper_row(
                    text_ui,
                    ui,
                    spec.id,
                    spec.label,
                    spec.info_tooltip,
                    &mut frame_limit_fps,
                    spec.min,
                    spec.max,
                    spec.step,
                );
            });
        }
        ui.add_space(style::SPACE_MD);
    });
    config.set_frame_limit_fps(frame_limit_fps);
}

fn render_selected_int_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    selected_ids: &[IntSettingId],
) {
    config.for_each_int_mut(|setting, value| {
        if !selected_ids.contains(&setting.id) {
            return;
        }
        ui.push_id(setting.id, |ui| {
            settings_widgets::int_stepper_row(
                text_ui,
                ui,
                setting.id,
                setting.label,
                setting.info_tooltip,
                value,
                setting.min,
                setting.max,
                setting.step,
            );
        });
        ui.add_space(style::SPACE_MD);
    });
}

fn render_selected_text_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
    selected_ids: &[config::TextSettingId],
) {
    config.for_each_text_mut(|setting, value| {
        if !selected_ids.contains(&setting.id) {
            return;
        }
        ui.push_id(setting.id, |ui| {
            settings_widgets::text_input_row(
                text_ui,
                ui,
                setting.id,
                setting.label,
                setting.info_tooltip,
                value,
            );
        });
        ui.add_space(style::SPACE_MD);
    });
}

fn render_curseforge_settings(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let _ = settings_widgets::text_input_row(
        text_ui,
        ui,
        "curseforge_api_key",
        "CurseForge API Key",
        Some(
            "Required for CurseForge modpack zip imports. Overrides VERTEX_CURSEFORGE_API_KEY/CURSEFORGE_API_KEY and enables CurseForge browsing/download metadata.",
        ),
        config.curseforge_api_key_mut(),
    );
    ui.add_space(style::SPACE_MD);
}
