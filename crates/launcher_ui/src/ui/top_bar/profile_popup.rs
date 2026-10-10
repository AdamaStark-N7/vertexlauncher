use super::*;
use textui_egui::prelude::*;
use ui_foundation::popup_width;

use crate::{privacy, ui::components::settings_widgets};

/// Tracks which popup control should take gamepad focus when the popup opens, plus the
/// hand-off that returns focus to the profile button once the popup closes.
struct PopupFocus {
    popup_id: egui::Id,
    focus_first: bool,
    first_focus_applied: bool,
}

impl PopupFocus {
    fn take(ui: &egui::Ui, popup_id: egui::Id) -> Self {
        let focus_first = ui.ctx().data_mut(|data| {
            let key = egui::Id::new((PROFILE_POPUP_FOCUS_FIRST_KEY, popup_id));
            let value = data.get_temp::<bool>(key).unwrap_or(false);
            if value {
                data.remove::<bool>(key);
            }
            value
        });
        Self {
            popup_id,
            focus_first,
            first_focus_applied: false,
        }
    }

    /// Gives the first control offered the requested focus.
    fn offer(&mut self, response: &egui::Response) {
        if self.focus_first && !self.first_focus_applied {
            response.request_focus();
            self.first_focus_applied = true;
        }
    }

    fn request_owner_focus(&self, ui: &egui::Ui) {
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::new((PROFILE_POPUP_OWNER_FOCUS_KEY, self.popup_id)),
                true,
            );
        });
    }
}

pub(super) fn render_profile_popup(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    profile_ui: ProfileUiModel<'_>,
    output: &mut TopBarOutput,
    popup_id: egui::Id,
    _owner_id: egui::Id,
    compact: bool,
) {
    let mut focus = PopupFocus::take(ui, popup_id);
    ui.spacing_mut().item_spacing = egui::vec2(style::SPACE_MD, style::SPACE_MD);
    let full_action_width = if compact {
        ui.available_width().max(160.0)
    } else {
        ui.available_width().max(220.0)
    };

    let muted_style = style::muted_single_line(ui);

    let button_style = ButtonOptions {
        min_size: egui::vec2(full_action_width, style::CONTROL_HEIGHT),
        corner_radius: style::CORNER_RADIUS_SM,
        padding: egui::vec2(style::SPACE_MD, style::SPACE_XS),
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().widgets.open.bg_fill,
        stroke: egui::Stroke::new(1.4, ui.visuals().widgets.hovered.bg_stroke.color),
        ..ButtonOptions::default()
    };

    render_profile_header(ui, text_ui, profile_ui);

    if let Some(prompt) = profile_ui.device_code_prompt {
        render_device_code_section(
            ui,
            text_ui,
            prompt,
            output,
            full_action_width,
            &button_style,
        );
        return;
    }

    if !profile_ui.accounts.is_empty() {
        ui.add_space(style::SPACE_XS / 2.0);
        let _ = text_ui.label(
            ui,
            "profile_popup_accounts_title",
            "Saved accounts",
            &muted_style,
        );
        render_saved_accounts(ui, text_ui, profile_ui, output, &mut focus, &button_style);
    }

    ui.add_space(style::SPACE_XS / 2.0);
    ui.separator();
    ui.add_space(style::SPACE_XS / 2.0);

    render_sign_in_actions(
        ui,
        text_ui,
        profile_ui,
        output,
        &mut focus,
        &button_style,
        full_action_width,
    );
}

/// Frame at the top of the popup: signed-in account name and, outside device-code mode, the
/// latest status message.
fn render_profile_header(ui: &mut egui::Ui, text_ui: &mut TextUi, profile_ui: ProfileUiModel<'_>) {
    let heading_style = style::stat_label(ui);
    let muted_style = style::muted_single_line(ui);
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(1.0, ui.visuals().window_stroke.color))
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_MD))
        .inner_margin(egui::Margin::same(style::SPACE_LG as i8))
        .show(ui, |ui| {
            if let Some(name) = profile_ui.display_name {
                let redacted_name = privacy::redact_account_label(profile_ui.streamer_mode, name);
                let _ = text_ui.label(
                    ui,
                    "profile_popup_signed_in",
                    &format!("Signed in as {redacted_name}"),
                    &heading_style,
                );
            } else {
                let _ = text_ui.label(
                    ui,
                    "profile_popup_signed_out",
                    "No Microsoft account signed in",
                    &heading_style,
                );
            }

            if profile_ui.device_code_prompt.is_none() {
                if let Some(message) = profile_ui.status_message {
                    let _ = text_ui.label(ui, "profile_popup_status", message, &muted_style);
                }
            }
        });
}

/// Label for a saved account row, with the active/failed suffix.
fn saved_account_label(profile_ui: ProfileUiModel<'_>, account: &ProfileAccountOption) -> String {
    let redacted =
        privacy::redact_account_label(profile_ui.streamer_mode, account.display_name.as_str());
    if account.is_active && account.is_failed {
        format!("{redacted} (Failed)")
    } else if account.is_active {
        format!("{redacted} (Active)")
    } else {
        redacted.into_owned()
    }
}

/// The framed list of saved accounts with select / refresh / remove controls.
fn render_saved_accounts(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    profile_ui: ProfileUiModel<'_>,
    output: &mut TopBarOutput,
    focus: &mut PopupFocus,
    button_style: &ButtonOptions,
) {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(1.0, ui.visuals().window_stroke.color))
        .corner_radius(egui::CornerRadius::same(style::CORNER_RADIUS_MD))
        .inner_margin(egui::Margin::same(style::SPACE_MD as i8))
        .show(ui, |ui| {
            let mut list_button_style = button_style.clone();
            list_button_style.min_size = egui::vec2(150.0, style::CONTROL_HEIGHT);

            let mut remove_button_style = button_style.clone();
            remove_button_style.min_size = egui::vec2(72.0, style::CONTROL_HEIGHT);

            let mut refresh_button_style = button_style.clone();
            refresh_button_style.min_size = egui::vec2(78.0, style::CONTROL_HEIGHT);

            for account in profile_ui.accounts {
                let label = saved_account_label(profile_ui, account);

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), style::CONTROL_HEIGHT),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if account.is_failed {
                            let refresh_response = ui
                                .add_enabled_ui(!profile_ui.auth_busy, |ui| {
                                    text_ui.button(
                                        ui,
                                        ("profile_popup_account_refresh", &account.profile_id),
                                        "Refresh",
                                        &refresh_button_style,
                                    )
                                })
                                .inner;
                            focus.offer(&refresh_response);
                            if refresh_response.clicked() {
                                output.refresh_account_id = Some(account.profile_id.clone());
                                focus.request_owner_focus(ui);
                            }
                        }

                        let remove_response = text_ui.button(
                            ui,
                            ("profile_popup_account_remove", &account.profile_id),
                            "Remove",
                            &remove_button_style,
                        );
                        focus.offer(&remove_response);
                        if remove_response.clicked() {
                            output.remove_account_id = Some(account.profile_id.clone());
                            focus.request_owner_focus(ui);
                        }

                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let mut fill_style = list_button_style.clone();
                            fill_style.min_size.x = ui.available_width().max(1.0);
                            let select_response = text_ui.selectable_button(
                                ui,
                                ("profile_popup_account_select", &account.profile_id),
                                &label,
                                account.is_active,
                                &fill_style,
                            );
                            focus.offer(&select_response);
                            if select_response.clicked() {
                                output.select_account_id = Some(account.profile_id.clone());
                                focus.request_owner_focus(ui);
                            }
                        });
                    },
                );
            }
        });
}

/// Bottom sign-in controls: a pending indicator while auth is busy, otherwise the webview and
/// device-code sign-in buttons.
fn render_sign_in_actions(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    profile_ui: ProfileUiModel<'_>,
    output: &mut TopBarOutput,
    focus: &mut PopupFocus,
    button_style: &ButtonOptions,
    full_action_width: f32,
) {
    let popup_id = focus.popup_id;
    let mut primary_button_style = button_style.clone();
    primary_button_style.min_size = egui::vec2(full_action_width, style::CONTROL_HEIGHT);
    primary_button_style.text_color = ui.visuals().text_color();
    primary_button_style.fill = ui.visuals().widgets.hovered.bg_fill;
    primary_button_style.fill_hovered = style::hover_fill(ui, primary_button_style.fill);
    primary_button_style.fill_active = ui.visuals().widgets.active.bg_fill;
    primary_button_style.fill_selected = ui.visuals().widgets.open.bg_fill;
    primary_button_style.stroke = egui::Stroke::new(1.8, ui.visuals().widgets.open.bg_stroke.color);

    if profile_ui.auth_busy {
        let mut pending_button_style = button_style.clone();
        pending_button_style.min_size = egui::vec2(full_action_width, style::CONTROL_HEIGHT);
        pending_button_style.stroke =
            egui::Stroke::new(1.4, ui.visuals().widgets.inactive.bg_stroke.color);
        ui.add_enabled_ui(false, |ui| {
            let _ = render_pending_text_button(
                ui,
                text_ui,
                "profile_popup_signing_in",
                pending_auth_label(profile_ui),
                &pending_button_style,
                true,
            );
        });
    } else {
        let half_width = (full_action_width - ui.spacing().item_spacing.x) / 2.0;
        let mut half_button_style = primary_button_style.clone();
        half_button_style.min_size = egui::vec2(half_width.max(1.0), style::CONTROL_HEIGHT);

        ui.allocate_ui_with_layout(
            egui::vec2(full_action_width, style::CONTROL_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let webview_response = text_ui.button(
                    ui,
                    "profile_popup_signin_webview",
                    "Webview Sign-in",
                    &half_button_style,
                );
                focus.offer(&webview_response);
                if webview_response.clicked() {
                    output.start_webview_sign_in = true;
                    egui::Popup::open_id(ui.ctx(), popup_id);
                }

                let device_code_response = text_ui.button(
                    ui,
                    "profile_popup_signin_device_code",
                    "Device Code Sign-in",
                    &half_button_style,
                );
                focus.offer(&device_code_response);
                if device_code_response.clicked() {
                    output.start_device_code_sign_in = true;
                    egui::Popup::open_id(ui.ctx(), popup_id);
                }
            },
        );
    }
}

pub(super) fn pending_auth_label(profile_ui: ProfileUiModel<'_>) -> &'static str {
    if profile_ui.token_refresh_in_progress {
        "Refreshing session..."
    } else if profile_ui.sign_in_in_progress {
        "Signing in with Microsoft..."
    } else {
        "Authenticating..."
    }
}

pub(super) fn render_pending_text_button(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash,
    label: &str,
    options: &ButtonOptions,
    show_spinner: bool,
) -> egui::Response {
    if !show_spinner {
        return text_ui.button(ui, id_source, label, options);
    }

    let desired_size = options.min_size;
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let label_id = ui.id().with("pending_auth_label").with(&id_source);
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(options.corner_radius),
        options.fill,
        options.stroke,
        egui::StrokeKind::Inside,
    );

    let inner_rect = rect.shrink2(options.padding);
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.spinner();
            ui.add_space(crate::ui::style::SPACE_XS);
            let label_style = LabelOptions {
                color: options.text_color,
                ..crate::ui::style::caption(ui)
            };
            let _ = text_ui.label(ui, label_id, label, &label_style);
        });
    });

    response
}

/// Hosts the account popup anchored to the profile button: picks the normal or device-code
/// popup id, carries open state across mode changes, and restores focus to the owner.
pub(super) fn show_profile_popup(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    profile_ui: ProfileUiModel<'_>,
    output: &mut TopBarOutput,
    profile_response: &egui::Response,
    compact: bool,
    viewport_width: f32,
) {
    // Use a separate popup ID for device-code mode so the normal popup's
    // cached area size is never inflated by the wider device-code layout.
    let in_dc = profile_ui.device_code_prompt.is_some();
    let normal_popup_id = ui.id().with("profile_selector_popup");
    let dc_popup_id = ui.id().with("profile_selector_popup_dc");
    let profile_popup_id = if in_dc { dc_popup_id } else { normal_popup_id };

    // When device-code state changes, transfer open state to the new ID.
    let prev_in_dc_key = ui.id().with("prev_in_dc");
    let prev_in_dc = ui
        .ctx()
        .data_mut(|d| d.get_temp::<bool>(prev_in_dc_key))
        .unwrap_or(in_dc);
    ui.ctx().data_mut(|d| d.insert_temp(prev_in_dc_key, in_dc));
    if prev_in_dc != in_dc {
        let old_id = if prev_in_dc {
            dc_popup_id
        } else {
            normal_popup_id
        };
        if egui::Popup::is_id_open(ui.ctx(), old_id) {
            egui::Popup::open_id(ui.ctx(), profile_popup_id);
        }
    }
    let popup_was_open = egui::Popup::is_id_open(ui.ctx(), profile_popup_id);
    if !popup_was_open {
        let _ = style::hover_tip(ui, profile_response.clone(), "Accounts and sign-in");
    }

    let _ = egui::Popup::menu(profile_response)
        .id(profile_popup_id)
        .width(popup_width(
            viewport_width,
            if compact {
                PROFILE_POPUP_MIN_WIDTH_COMPACT
            } else {
                PROFILE_POPUP_MIN_WIDTH
            },
            PROFILE_POPUP_MIN_WIDTH,
            8.0,
        ))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            if !popup_was_open && settings_widgets::gamepad_input_history(ui.ctx()) {
                ui.ctx().data_mut(|data| {
                    data.insert_temp(
                        egui::Id::new((PROFILE_POPUP_FOCUS_FIRST_KEY, profile_popup_id)),
                        true,
                    );
                });
            }
            render_profile_popup(
                ui,
                text_ui,
                profile_ui,
                output,
                profile_popup_id,
                profile_response.id,
                compact,
            );
        });
    let popup_is_open = egui::Popup::is_id_open(ui.ctx(), profile_popup_id);
    let should_restore_owner_focus = ui.ctx().data_mut(|data| {
        let key = egui::Id::new((PROFILE_POPUP_OWNER_FOCUS_KEY, profile_popup_id));
        let pending = data.get_temp::<bool>(key).unwrap_or(false);
        if pending && !popup_is_open {
            data.remove::<bool>(key);
            true
        } else {
            false
        }
    });
    if should_restore_owner_focus {
        profile_response.request_focus();
    }
}
