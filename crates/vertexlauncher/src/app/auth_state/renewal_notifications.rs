use super::*;

pub(super) fn emit_cached_account_renewal_notification(
    event: CachedAccountRenewalEvent,
    streamer_mode: bool,
) {
    match event {
        CachedAccountRenewalEvent::Started {
            profile_id,
            display_name,
        } => {
            let display_name = privacy::redact_account_label(streamer_mode, &display_name);
            notification::emit_spinner(
                notification::Severity::Info,
                "Login Renewal",
                format!("Renewing Login Token for {display_name}"),
                renewal_replace_key(&profile_id),
            );
        }
        CachedAccountRenewalEvent::Succeeded {
            profile_id,
            display_name,
        } => {
            let display_name = privacy::redact_account_label(streamer_mode, &display_name);
            notification::emit_replace(
                notification::Severity::Info,
                "Login Renewal",
                format!("login token for {display_name} successful, you are now ready to play!"),
                renewal_replace_key(&profile_id),
            );
        }
        CachedAccountRenewalEvent::Failed {
            profile_id,
            display_name,
            error,
        } => {
            let display_name_for_notification =
                privacy::redact_account_label(streamer_mode, &display_name);
            tracing::warn!(
                target: "vertexlauncher/auth/renew",
                profile_fingerprint = %webview_sign_in::fingerprint_for_log(&profile_id),
                error = %webview_sign_in::sanitize_message_for_log(&error),
                "Auto-renew login notification reported an error."
            );
            notification::emit_replace(
                notification::Severity::Error,
                "Login Renewal",
                format!(
                    "Error in attepting to renew login for {display_name_for_notification}: {error}"
                ),
                renewal_replace_key(&profile_id),
            );
        }
    }
}

fn renewal_replace_key(profile_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    profile_id.hash(&mut hasher);
    format!("login-renewal:{:016x}", hasher.finish())
}
