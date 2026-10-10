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
        absorb(&mut store, notif);
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

/// Adds `notif` to the store, folding it into an entry that is already showing when it can:
///
/// 1. A progress/spinner update replaces the entry with the same `replace_key`.
/// 2. A notification with a [`StackSpec`] joins the entry with the same stack key: items are
///    added, the message is re-rendered from all of them, and the newest buttons win.
/// 3. Otherwise an identical notification (same severity, source and message, no buttons)
///    bumps the entry's "xN" count.
pub(super) fn absorb(store: &mut NotificationStore, notif: Notification) {
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
            return;
        }
    } else if let Some(stack) = notif.stack.as_ref() {
        if stack.remove {
            retract(store, stack);
            return;
        }
        if let Some(existing) = store.entries.iter_mut().find(|entry| {
            entry
                .stack
                .as_ref()
                .is_some_and(|existing| existing.key == stack.key)
        }) {
            if let Some(existing_stack) = existing.stack.as_mut() {
                existing_stack.items.extend(stack.items.iter().cloned());
                existing.message = emitters::sanitize_field(
                    streamer_mode_enabled(),
                    (existing_stack.render)(&existing_stack.items),
                );
            }
            existing.severity = notif.severity;
            if !notif.actions.is_empty() {
                existing.actions = notif.actions;
            }
            existing.last_seen = notif.when;
            return;
        }
    } else if let Some(existing) = store.entries.iter_mut().find(|entry| {
        notif.actions.is_empty()
            && entry.actions.is_empty()
            && entry.stack.is_none()
            && entry.severity == notif.severity
            && entry.source == notif.source
            && entry.message == notif.message
    }) {
        existing.count = existing.count.saturating_add(1);
        existing.last_seen = notif.when;
        return;
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
        stack: notif.stack,
        count: 1,
        last_seen: notif.when,
    });
    store.next_id = entry_id.saturating_add(1);
}

/// Removes the items of a retraction from the entry with the same stack key.
fn retract(store: &mut NotificationStore, stack: &StackSpec) {
    let Some(position) = store.entries.iter().position(|entry| {
        entry
            .stack
            .as_ref()
            .is_some_and(|existing| existing.key == stack.key)
    }) else {
        return;
    };
    let entry = &mut store.entries[position];
    let Some(existing_stack) = entry.stack.as_mut() else {
        return;
    };
    for item in &stack.items {
        if let Some(index) = existing_stack.items.iter().position(|held| held == item) {
            existing_stack.items.remove(index);
        }
    }
    if existing_stack.items.is_empty() {
        store.entries.remove(position);
    } else {
        entry.message = emitters::sanitize_field(
            streamer_mode_enabled(),
            (existing_stack.render)(&existing_stack.items),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(message: &str) -> Notification {
        Notification {
            severity: Severity::Info,
            source: "test".to_owned(),
            message: message.to_owned(),
            progress: None,
            spinner: false,
            replace_key: None,
            actions: Vec::new(),
            icon: None,
            stack: None,
            when: Instant::now(),
        }
    }

    fn capes(items: &[&str]) -> Notification {
        let items: Vec<String> = items.iter().map(|item| (*item).to_owned()).collect();
        let render = |items: &[String]| format!("capes: {}", join_natural(items));
        let mut notif = plain(&render(&items));
        notif.stack = Some(StackSpec {
            key: "capes".to_owned(),
            items,
            render: Arc::new(render),
            remove: false,
        });
        notif
    }

    #[test]
    fn identical_notifications_collapse_into_a_count() {
        let mut store = NotificationStore::default();
        absorb(&mut store, plain("Saved."));
        absorb(&mut store, plain("Saved."));
        absorb(&mut store, plain("Saved."));
        absorb(&mut store, plain("Something else."));
        assert_eq!(store.entries.len(), 2);
        assert_eq!(store.entries[0].count, 3);
        assert_eq!(store.entries[1].count, 1);
    }

    #[test]
    fn stacked_notifications_merge_into_one_natural_message() {
        let mut store = NotificationStore::default();
        absorb(&mut store, capes(&["A"]));
        absorb(&mut store, capes(&["B"]));
        absorb(&mut store, capes(&["C"]));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].message, "capes: A, B, and C");
        assert_eq!(store.entries[0].count, 1);
    }

    #[test]
    fn different_stack_keys_stay_separate_and_newest_actions_win() {
        let mut store = NotificationStore::default();
        let mut other = capes(&["X"]);
        other.stack.as_mut().unwrap().key = "other".to_owned();
        absorb(&mut store, capes(&["A"]));
        absorb(&mut store, other);
        assert_eq!(store.entries.len(), 2);

        let mut with_button = capes(&["B"]);
        with_button.actions = vec![NotificationAction::new("Open", |_| {})];
        absorb(&mut store, with_button);
        let merged = store
            .entries
            .iter()
            .find(|entry| entry.message.contains('A'))
            .unwrap();
        assert_eq!(merged.actions.len(), 1);
    }

    #[test]
    fn retracting_items_shrinks_then_dismisses_the_stack() {
        let mut store = NotificationStore::default();
        absorb(&mut store, capes(&["A", "B"]));
        let retraction = |item: &str| {
            let mut notif = plain("");
            notif.stack = Some(StackSpec {
                key: "capes".to_owned(),
                items: vec![item.to_owned()],
                render: Arc::new(|_| String::new()),
                remove: true,
            });
            notif
        };
        absorb(&mut store, retraction("A"));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].message, "capes: B");
        absorb(&mut store, retraction("missing"));
        assert_eq!(store.entries[0].message, "capes: B");
        absorb(&mut store, retraction("B"));
        assert!(store.entries.is_empty());
        // Retracting from a stack that is not showing is a no-op.
        absorb(&mut store, retraction("B"));
        assert!(store.entries.is_empty());
    }
}
