use super::*;

use crate::privacy;

/// Redacts (in streamer mode) and sanitizes one notification text field for logs and display.
fn sanitize_field(streamer_mode: bool, value: String) -> String {
    privacy::sanitize_text_for_log(
        &privacy::redact_sensitive_text(streamer_mode, &value).into_owned(),
    )
}

pub fn emit(severity: Severity, source: impl Into<String>, message: impl Into<String>) {
    let streamer_mode = streamer_mode_enabled();
    let source = sanitize_field(streamer_mode, source.into());
    let message = sanitize_field(streamer_mode, message.into());
    match severity {
        Severity::Log => {
            tracing::debug!(target: "notification", source = %source, message = %message)
        }
        Severity::Info => {
            tracing::info!(target: "notification", source = %source, message = %message)
        }
        Severity::Warn => {
            tracing::warn!(target: "notification", source = %source, message = %message)
        }
        Severity::Error => {
            tracing::error!(target: "notification", source = %source, message = %message)
        }
    }
    if let Err(err) = center().tx.send(Notification {
        severity,
        source,
        message,
        progress: None,
        spinner: false,
        replace_key: None,
        actions: Vec::new(),
        icon: None,
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue notification."
        );
    }
}

/// Like [`emit`], but with buttons attached to the popup.
pub fn emit_with_actions(
    severity: Severity,
    source: impl Into<String>,
    message: impl Into<String>,
    actions: Vec<NotificationAction>,
) {
    let streamer_mode = streamer_mode_enabled();
    let source = sanitize_field(streamer_mode, source.into());
    let message = sanitize_field(streamer_mode, message.into());
    tracing::info!(target: "notification", source = %source, message = %message);
    if let Err(err) = center().tx.send(Notification {
        severity,
        source,
        message,
        progress: None,
        spinner: false,
        replace_key: None,
        actions,
        icon: None,
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue notification."
        );
    }
}

/// Like [`emit`], but shows `icon_svg` in place of the severity icon.
pub fn emit_with_icon(
    severity: Severity,
    source: impl Into<String>,
    message: impl Into<String>,
    icon_svg: &'static [u8],
) {
    let streamer_mode = streamer_mode_enabled();
    let source = sanitize_field(streamer_mode, source.into());
    let message = sanitize_field(streamer_mode, message.into());
    tracing::info!(target: "notification", source = %source, message = %message);
    if let Err(err) = center().tx.send(Notification {
        severity,
        source,
        message,
        progress: None,
        spinner: false,
        replace_key: None,
        actions: Vec::new(),
        icon: Some(icon_svg),
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue notification."
        );
    }
}

pub fn emit_progress(
    severity: Severity,
    source: impl Into<String>,
    message: impl Into<String>,
    progress: f32,
) {
    let streamer_mode = streamer_mode_enabled();
    let source = sanitize_field(streamer_mode, source.into());
    let message = sanitize_field(streamer_mode, message.into());
    let progress = progress.clamp(0.0, 1.0);
    tracing::info!(
        target: "notification",
        source = %source,
        message = %message,
        progress,
        "notification progress update"
    );
    if let Err(err) = center().tx.send(Notification {
        severity,
        source: source.clone(),
        message,
        progress: Some(progress),
        spinner: false,
        replace_key: Some(source.clone()),
        actions: Vec::new(),
        icon: None,
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            source = %source,
            progress,
            error = %err,
            "Failed to enqueue progress notification."
        );
    }
}

pub fn emit_spinner(
    severity: Severity,
    source: impl Into<String>,
    message: impl Into<String>,
    replace_key: impl Into<String>,
) {
    let streamer_mode = streamer_mode_enabled();
    let source = sanitize_field(streamer_mode, source.into());
    let message = sanitize_field(streamer_mode, message.into());
    let replace_key = sanitize_field(streamer_mode, replace_key.into());
    tracing::info!(
        target: "notification",
        source = %source,
        message = %message,
        replace_key = %replace_key,
        spinner = true,
        "notification spinner update"
    );
    if let Err(err) = center().tx.send(Notification {
        severity,
        source,
        message,
        progress: None,
        spinner: true,
        replace_key: Some(replace_key),
        actions: Vec::new(),
        icon: None,
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue spinner notification."
        );
    }
}

pub fn emit_replace(
    severity: Severity,
    source: impl Into<String>,
    message: impl Into<String>,
    replace_key: impl Into<String>,
) {
    let streamer_mode = streamer_mode_enabled();
    let source = sanitize_field(streamer_mode, source.into());
    let message = sanitize_field(streamer_mode, message.into());
    let replace_key = sanitize_field(streamer_mode, replace_key.into());
    tracing::info!(
        target: "notification",
        source = %source,
        message = %message,
        replace_key = %replace_key,
        "notification replacement update"
    );
    if let Err(err) = center().tx.send(Notification {
        severity,
        source,
        message,
        progress: None,
        spinner: false,
        replace_key: Some(replace_key),
        actions: Vec::new(),
        icon: None,
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue replacement notification."
        );
    }
}
