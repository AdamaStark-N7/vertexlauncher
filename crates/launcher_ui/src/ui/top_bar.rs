// interaction: allow (the clickable area here is the window drag region; buttons live in top_bar/)
#[path = "top_bar/account_color_editor.rs"]
mod account_color_editor;
#[path = "top_bar/avatar.rs"]
mod avatar;
#[path = "top_bar/device_code.rs"]
mod device_code;
#[path = "top_bar/profile_button.rs"]
mod profile_button;
#[path = "top_bar/profile_popup.rs"]
mod profile_popup;
#[path = "top_bar/qr_code.rs"]
mod qr_code;
#[path = "top_bar/window_controls.rs"]
mod window_controls;

use egui::{self, Align, Context, Layout, Panel, Sense, ViewportCommand};
use textui::TextUi;
use textui_egui::prelude::*;
use ui_foundation::is_compact_width;

use crate::ui::style;

use avatar::rounded_profile_avatar_png;
use device_code::render_device_code_section;
use profile_button::{render_active_user_terminal_button, render_profile_button};
use qr_code::render_qr_code;
pub use window_controls::handle_window_resize;
use window_controls::render_controls;

const TOP_BAR_HEIGHT: f32 = 38.0;
const CONTROL_SLOT_WIDTH: f32 = 20.0;
const CONTROL_ICON_MAX_WIDTH: f32 = 20.0;
const CONTROL_GAP: f32 = 7.0;
const CONTROL_GROUP_PADDING: f32 = 12.0;
const PROFILE_BUTTON_VERTICAL_PADDING: f32 = 5.0;
const PROFILE_TO_CONTROLS_GAP: f32 = style::SPACE_MD;
const ACTIVE_USER_TO_PROFILE_GAP: f32 = style::SPACE_SM;
const ACTIVE_USER_BUTTON_MIN_WIDTH: f32 = 148.0;
const PROFILE_POPUP_MIN_WIDTH: f32 = 310.0;
const PROFILE_POPUP_MIN_WIDTH_COMPACT: f32 = 220.0;
const RESIZE_GRAB_THICKNESS: f32 = 6.0;
const PROFILE_BUTTON_CORNER_RADIUS: u8 = 10;
const TOP_BAR_COMPACT_THRESHOLD: f32 = 720.0;
const ROUNDED_AVATAR_CACHE_MAX_ENTRIES: usize = 32;
const PROFILE_POPUP_FOCUS_FIRST_KEY: &str = "top_bar_profile_popup_focus_first";
const PROFILE_POPUP_OWNER_FOCUS_KEY: &str = "top_bar_profile_popup_owner_focus";
const PROFILE_POPUP_COLOR_EDITOR_KEY: &str = "top_bar_profile_popup_color_editor";

#[derive(Debug, Clone, Default)]
pub struct TopBarOutput {
    pub start_webview_sign_in: bool,
    pub start_device_code_sign_in: bool,
    pub open_device_code_browser: bool,
    pub cancel_device_code_sign_in: bool,
    pub select_account_id: Option<String>,
    pub remove_account_id: Option<String>,
    pub refresh_account_id: Option<String>,
    pub open_active_user_terminal: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileAccountOption {
    pub profile_id: String,
    pub display_name: String,
    pub is_active: bool,
    pub is_failed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileUiModel<'a> {
    pub display_name: Option<&'a str>,
    pub avatar_png: Option<&'a [u8]>,
    pub sign_in_in_progress: bool,
    pub auth_busy: bool,
    pub token_refresh_in_progress: bool,
    pub streamer_mode: bool,
    pub status_message: Option<&'a str>,
    pub accounts: &'a [ProfileAccountOption],
    pub user_instance_active: bool,
    pub device_code_prompt: Option<&'a auth::DeviceCodePrompt>,
}

#[allow(deprecated)]
pub fn render(
    ctx: &Context,
    section_label: &str,
    text_ui: &mut TextUi,
    profile_ui: ProfileUiModel<'_>,
) -> TopBarOutput {
    let mut output = TopBarOutput::default();

    Panel::top("window_top_bar")
        .exact_size(TOP_BAR_HEIGHT)
        .resizable(false)
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
            ui.spacing_mut().item_spacing.x = 0.0;
            let full_rect = ui.max_rect();
            let viewport_width = full_rect.width();
            let compact = is_compact_width(viewport_width, TOP_BAR_COMPACT_THRESHOLD);
            let profile_button_size =
                (TOP_BAR_HEIGHT - (PROFILE_BUTTON_VERTICAL_PADDING * 2.0)).max(1.0);
            let active_user_button_width = if compact {
                (viewport_width * 0.2).clamp(96.0, ACTIVE_USER_BUTTON_MIN_WIDTH)
            } else {
                ACTIVE_USER_BUTTON_MIN_WIDTH
            };
            let active_user_visible = profile_ui.user_instance_active;
            let control_group_width =
                (CONTROL_SLOT_WIDTH * 3.0) + (CONTROL_GAP * 2.0) + (CONTROL_GROUP_PADDING * 2.0);
            let right_side_width = control_group_width
                + PROFILE_TO_CONTROLS_GAP
                + profile_button_size
                + if active_user_visible {
                    ACTIVE_USER_TO_PROFILE_GAP + active_user_button_width
                } else {
                    0.0
                };
            let controls_min_x = (full_rect.max.x - right_side_width).max(full_rect.min.x);
            let drag_rect = egui::Rect::from_min_max(
                full_rect.min,
                egui::pos2(controls_min_x, full_rect.max.y),
            );
            let controls_rect = egui::Rect::from_min_max(
                egui::pos2(controls_min_x, full_rect.min.y),
                full_rect.max,
            );

            let drag_response = ui.interact(
                drag_rect,
                ui.id().with("top_bar_drag_region"),
                Sense::click_and_drag(),
            );
            if drag_response.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }

            ui.scope_builder(egui::UiBuilder::new().max_rect(drag_rect), |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(style::SPACE_LG);
                    let section_style = style::muted_single_line(ui);
                    let _ = text_ui.label(
                        ui,
                        ("topbar_screen", section_label),
                        section_label,
                        &section_style,
                    );
                });
            });

            ui.scope_builder(egui::UiBuilder::new().max_rect(controls_rect), |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(CONTROL_GROUP_PADDING);
                    render_controls(ui, ctx);
                    ui.add_space(PROFILE_TO_CONTROLS_GAP);

                    let profile_response =
                        render_profile_button(ui, text_ui, profile_ui, profile_button_size);
                    profile_popup::show_profile_popup(
                        ui,
                        text_ui,
                        profile_ui,
                        &mut output,
                        &profile_response,
                        compact,
                        viewport_width,
                    );

                    if active_user_visible {
                        ui.add_space(ACTIVE_USER_TO_PROFILE_GAP);
                        if render_active_user_terminal_button(
                            ui,
                            text_ui,
                            profile_button_size,
                            active_user_button_width,
                            compact,
                        )
                        .clicked()
                        {
                            output.open_active_user_terminal = true;
                        }
                    }

                    ui.add_space(CONTROL_GROUP_PADDING);
                });
            });
        });

    output
}
