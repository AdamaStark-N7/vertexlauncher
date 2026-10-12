use super::super::*;

/// Renders the active-account status banner.
///
/// Returns `true` when rendering should continue and `false` when the screen should stop
/// after showing the signed-out message.
pub(super) fn render_account_status_section(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    state: &SkinManagerState,
    streamer_mode: bool,
) -> bool {
    let body = style::body(ui);
    let muted = style::muted(ui);

    if let Some(name) = state.active_player_name.as_deref() {
        let _ = crate::ui::account_label::account_label(
            ui,
            text_ui,
            "skins_active_user",
            &format!(
                "Active account: {}",
                crate::account_text::mention(
                    state.active_profile_id.as_deref().unwrap_or(""),
                    name
                )
            ),
            &body,
            streamer_mode,
            false,
        );
        true
    } else {
        let _ = text_ui.label(
            ui,
            "skins_no_active_user",
            "Sign in with a Minecraft account to manage skins and capes.",
            &muted,
        );
        false
    }
}
