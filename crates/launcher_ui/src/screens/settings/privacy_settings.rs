use super::*;
use ui_foundation::{DialogPreset, danger_button, dialog_options, secondary_button, show_dialog};

const CONFIRM_ID: &str = "settings_confirm_insecure_account_storage";
const CONFIRM_BUTTON_WIDTH: f32 = 140.0;

const INSECURE_STORAGE_WARNING: &str = "Warning: this keeps your sign-in tokens in a plain-text file instead of your system's secure keyring. Doing so can improve compatibility with the Steam Deck, but it also means you need to be extra careful with your security, because it can make it really easy for your login details to be compromised: anyone or anything that can read that file can sign in as you.";

/// The warning, then the toggle. Turning the toggle on only opens the confirmation popup (see
/// [`render_insecure_storage_confirm`]); turning it off applies immediately.
pub(super) fn render_insecure_storage_setting(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let _ = text_ui.label(
        ui,
        "settings_insecure_storage_warning",
        INSECURE_STORAGE_WARNING,
        &style::warning_text(ui),
    );
    let tooltip = format!(
        "Stores sign-in tokens in {} instead of the system keyring. The file is not encrypted. Turn this off to move them back into the keyring. Default: Off.",
        auth::insecure_token_file_path().display()
    );
    let current = config.insecure_file_account_storage();
    let mut requested = current;
    settings_widgets::toggle_row(
        text_ui,
        ui,
        "Use Unsecured File Account Storage",
        Some(tooltip.as_str()),
        &mut requested,
    );
    if requested && !current {
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new(CONFIRM_ID), true));
    } else if !requested && current {
        config.set_insecure_file_account_storage(false);
    }
    ui.add_space(style::SPACE_MD);
}

/// Confirmation popup shown before unsecured account storage is switched on. Call once per
/// frame from the settings screen.
pub(super) fn render_insecure_storage_confirm(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    let id = egui::Id::new(CONFIRM_ID);
    if !ctx.data(|data| data.get_temp::<bool>(id)).unwrap_or(false) {
        return;
    }

    let danger = ctx.global_style().visuals.error_fg_color;
    let request_cancel_focus = super::super::home::modal_default_focus_requested(ctx, CONFIRM_ID);
    let mut confirmed = false;
    let mut cancelled = false;
    let response = show_dialog(
        ctx,
        dialog_options(CONFIRM_ID, DialogPreset::Confirm),
        |ui| {
            let heading_style = style::heading_color(ui, 28.0, 32.0, danger);
            let _ = text_ui.label(
                ui,
                "settings_insecure_storage_confirm_heading",
                "Use unsecured account storage?",
                &heading_style,
            );
            let _ = text_ui.label(
                ui,
                "settings_insecure_storage_confirm_warning",
                INSECURE_STORAGE_WARNING,
                &style::warning_text(ui),
            );
            let _ = text_ui.label(
                ui,
                "settings_insecure_storage_confirm_path",
                &format!(
                    "Tokens will be written, unencrypted, to {}. Keep that file private, never share it, and leave it out of backups and screenshots.",
                    auth::insecure_token_file_path().display()
                ),
                &style::body(ui),
            );

            ui.add_space(style::SPACE_XL);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let size = egui::vec2(CONFIRM_BUTTON_WIDTH, style::CONTROL_HEIGHT);
                if text_ui
                    .button(
                        ui,
                        "settings_insecure_storage_confirm",
                        "Enable anyway",
                        &danger_button(ui, size),
                    )
                    .clicked()
                {
                    confirmed = true;
                }
                // Cancel is the default focus, so a stray Enter never enables this.
                let cancel_response = text_ui.button(
                    ui,
                    "settings_insecure_storage_cancel",
                    "Cancel",
                    &secondary_button(ui, size),
                );
                if request_cancel_focus {
                    cancel_response.request_focus();
                }
                if cancel_response.clicked() {
                    cancelled = true;
                }
            });
        },
    );
    cancelled |= response.close_requested;

    if confirmed {
        config.set_insecure_file_account_storage(true);
    }
    if confirmed || cancelled {
        ctx.data_mut(|data| data.remove::<bool>(id));
    }
}
