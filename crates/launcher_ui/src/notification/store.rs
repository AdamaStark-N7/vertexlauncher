use super::*;

pub(super) fn drain_notifications(fade_out: Duration) {
    let center = center();
    let Ok(rx) = center.rx.lock() else {
        tracing::error!(
            target: "notification",
            "Notification receiver mutex was poisoned while draining notifications."
        );
        return;
    };
    let Ok(mut store) = center.store.lock() else {
        tracing::error!(
            target: "notification",
            "Notification store mutex was poisoned while draining notifications."
        );
        return;
    };

    while let Ok(notif) = rx.try_recv() {
        if let Some(replace_key) = notif.replace_key.as_deref() {
            if let Some(existing) = store
                .entries
                .iter_mut()
                .find(|entry| entry.replace_key.as_deref() == Some(replace_key))
            {
                existing.severity = notif.severity;
                existing.source = notif.source;
                existing.message = notif.message;
                existing.progress = notif.progress;
                existing.spinner = notif.spinner;
                existing.count = 1;
                existing.replace_key = notif.replace_key;
                existing.last_seen = notif.when;
                continue;
            }
        } else if let Some(existing) = store.entries.iter_mut().find(|entry| {
            notif.actions.is_empty()
                && entry.actions.is_empty()
                && entry.severity == notif.severity
                && entry.source == notif.source
                && entry.message == notif.message
        }) {
            existing.count = existing.count.saturating_add(1);
            existing.last_seen = notif.when;
            continue;
        }

        let entry_id = store.next_id;
        store.entries.push(NotificationEntry {
            id: entry_id,
            severity: notif.severity,
            source: notif.source,
            message: notif.message,
            progress: notif.progress,
            spinner: notif.spinner,
            replace_key: notif.replace_key,
            actions: notif.actions,
            icon: notif.icon,
            count: 1,
            last_seen: notif.when,
        });
        store.next_id = entry_id.saturating_add(1);
    }

    let now = Instant::now();
    store.entries.retain(|entry| {
        now.saturating_duration_since(entry.last_seen) < entry_ttl(entry) + fade_out
    });
    store.entries.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    if store.entries.len() > NOTIFICATION_MAX_STACK {
        store.entries.truncate(NOTIFICATION_MAX_STACK);
    }
}

pub(super) fn entry_ttl(entry: &NotificationEntry) -> Duration {
    if entry.progress.is_some() || entry.spinner {
        PROGRESS_NOTIFICATION_STALE_TTL
    } else if !entry.actions.is_empty() {
        NOTIFICATION_ACTION_TTL
    } else {
        NOTIFICATION_TTL
    }
}

pub(super) fn entry_fade_opacity(
    entry: &NotificationEntry,
    now: Instant,
    settings: &NotificationDisplaySettings,
) -> f32 {
    let past_ttl = now
        .saturating_duration_since(entry.last_seen)
        .saturating_sub(entry_ttl(entry));
    if past_ttl.is_zero() {
        return 1.0;
    }
    let fade_progress = past_ttl.as_secs_f32() / settings.fade_out.as_secs_f32().max(f32::EPSILON);
    1.0 - settings.easing.apply(fade_progress)
}
