use std::collections::HashSet;
use std::fs;

use crate::fs_util::{modified, write_atomic};
use crate::{SyncInstance, SyncReport};

const HISTORY_FILE: &str = "command_history.txt";
/// Minecraft keeps at most this many entries.
const MAX_ENTRIES: usize = 50;

/// Merges every instance's command history into one list (oldest file first, duplicates
/// collapse to their latest use) and writes it back wherever it differs.
pub(crate) fn sync(instances: &[SyncInstance], skip: &HashSet<String>, report: &mut SyncReport) {
    let mut sources: Vec<(Option<std::time::SystemTime>, Vec<String>)> = instances
        .iter()
        .filter_map(|instance| {
            let path = instance.root.join(HISTORY_FILE);
            let text = fs::read_to_string(&path).ok()?;
            let lines = text
                .lines()
                .map(str::trim_end)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect();
            Some((modified(&path), lines))
        })
        .collect();
    if sources.is_empty() {
        return;
    }
    sources.sort_by_key(|(modified, _)| *modified);

    let mut merged: Vec<String> = Vec::new();
    for line in sources.into_iter().flat_map(|(_, lines)| lines) {
        merged.retain(|existing| *existing != line);
        merged.push(line);
    }
    let start = merged.len().saturating_sub(MAX_ENTRIES);
    let mut text = merged[start..].join("\n");
    text.push('\n');

    for instance in instances {
        if skip.contains(&instance.id) {
            continue;
        }
        let path = instance.root.join(HISTORY_FILE);
        if fs::read_to_string(&path).is_ok_and(|current| current == text) {
            continue;
        }
        match write_atomic(&path, text.as_bytes(), None) {
            Ok(()) => report.command_history_files += 1,
            Err(err) => report.errors.push(format!(
                "{}: failed to write {HISTORY_FILE}: {err}",
                instance.id
            )),
        }
    }
}
