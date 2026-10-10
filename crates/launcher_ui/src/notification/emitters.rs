use super::*;

use crate::privacy;

/// Redacts (in streamer mode) and sanitizes one notification text field for logs and display.
pub(super) fn sanitize_field(streamer_mode: bool, value: String) -> String {
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
        stack: None,
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
    NotificationBuilder::new(severity, source, message)
        .actions(actions)
        .emit();
}

/// Like [`emit`], but shows `icon_svg` in place of the severity icon.
pub fn emit_with_icon(
    severity: Severity,
    source: impl Into<String>,
    message: impl Into<String>,
    icon_svg: &'static [u8],
) {
    NotificationBuilder::new(severity, source, message)
        .icon(icon_svg)
        .emit();
}

/// Builds a notification with optional buttons, icon and stacking.
///
/// ```ignore
/// NotificationBuilder::new(Severity::Info, "skins", "")
///     .stack("new_capes:Steve", vec!["Migrator".into()], |capes| capes_message(capes))
///     .emit();
/// ```
pub struct NotificationBuilder {
    severity: Severity,
    source: String,
    message: String,
    actions: Vec<NotificationAction>,
    icon: Option<&'static [u8]>,
    stack: Option<StackSpec>,
    spinner: bool,
}

impl NotificationBuilder {
    pub fn new(severity: Severity, source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity,
            source: source.into(),
            message: message.into(),
            actions: Vec::new(),
            icon: None,
            stack: None,
            spinner: false,
        }
    }

    pub fn actions(mut self, actions: Vec<NotificationAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn icon(mut self, icon_svg: &'static [u8]) -> Self {
        self.icon = Some(icon_svg);
        self
    }

    /// Merges this notification into any showing one with the same `key`. `render` turns the
    /// combined `items` into the message, and the message given to [`Self::new`] is replaced by
    /// `render(&items)`. Buttons on a merged popup belong to the most recent arrival.
    pub fn stack(
        mut self,
        key: impl Into<String>,
        items: Vec<String>,
        render: impl Fn(&[String]) -> String + Send + Sync + 'static,
    ) -> Self {
        self.message = render(&items);
        self.stack = Some(StackSpec {
            key: key.into(),
            items,
            render: Arc::new(render),
            remove: false,
        });
        self
    }

    /// Shows a spinner beside the message, for work that is still in progress.
    pub fn spinner(mut self) -> Self {
        self.spinner = true;
        self
    }

    pub fn emit(self) {
        let streamer_mode = streamer_mode_enabled();
        let source = sanitize_field(streamer_mode, self.source);
        let message = sanitize_field(streamer_mode, self.message);
        tracing::info!(target: "notification", source = %source, message = %message);
        if let Err(err) = center().tx.send(Notification {
            severity: self.severity,
            source,
            message,
            progress: None,
            spinner: self.spinner,
            replace_key: None,
            actions: self.actions,
            icon: self.icon,
            stack: self.stack,
            when: Instant::now(),
        }) {
            tracing::error!(
                target: "notification",
                error = %err,
                "Failed to enqueue notification."
            );
        }
    }
}

/// Takes `items` back out of the showing notification stacked under `key`, re-rendering its
/// message, and dismissing it once no items are left. Does nothing if no such notification is
/// showing. Use it when part of what a stacked notification describes stops being true, such as
/// one of several "renewing..." spinners finishing.
pub fn retract_from_stack(key: impl Into<String>, items: Vec<String>) {
    if let Err(err) = center().tx.send(Notification {
        severity: Severity::Log,
        source: String::new(),
        message: String::new(),
        progress: None,
        spinner: false,
        replace_key: None,
        actions: Vec::new(),
        icon: None,
        stack: Some(StackSpec {
            key: key.into(),
            items,
            render: Arc::new(|_| String::new()),
            remove: true,
        }),
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue stack retraction."
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
        stack: None,
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
        stack: None,
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
        stack: None,
        when: Instant::now(),
    }) {
        tracing::error!(
            target: "notification",
            error = %err,
            "Failed to enqueue replacement notification."
        );
    }
}
