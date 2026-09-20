//! Collapses worlds and servers that exist in several instances into one Home entry, with a
//! row of per-instance launch buttons.

use std::collections::BTreeMap;

use super::*;

const BUTTON_GAP: f32 = style::SPACE_SM;
/// Room around the widest instance name inside a button, on top of the button's own padding.
const BUTTON_LABEL_PADDING: f32 = 12.0;
const BUTTONS_TOP_SPACE: f32 = style::SPACE_XS;

/// One instance an entry can be launched in, and what to launch there (the world folder
/// name or server address as that instance knows it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryInstance {
    pub(crate) instance_id: String,
    pub(crate) instance_name: String,
    pub(crate) target: String,
    pub(crate) favorite: bool,
}

/// Merges entries sharing a key into one, keeping the most recently used as the primary.
fn collapse<T>(
    entries: Vec<T>,
    key_of: impl Fn(&T) -> String,
    instances_of: impl Fn(&mut T) -> &mut Vec<EntryInstance>,
    last_used_of: impl Fn(&T) -> Option<u64>,
    absorb: impl Fn(&mut T, T),
) -> Vec<T> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: BTreeMap<String, T> = BTreeMap::new();
    for mut entry in entries {
        let key = key_of(&entry);
        match groups.remove(&key) {
            None => {
                order.push(key.clone());
                groups.insert(key, entry);
            }
            Some(mut existing) => {
                let entry_is_newer = last_used_of(&entry) > last_used_of(&existing);
                let mut merged = std::mem::take(instances_of(&mut existing));
                merged.append(instances_of(&mut entry));
                let mut winner = if entry_is_newer {
                    absorb(&mut entry, existing);
                    entry
                } else {
                    absorb(&mut existing, entry);
                    existing
                };
                merged.sort_by(|a, b| {
                    a.instance_name
                        .to_lowercase()
                        .cmp(&b.instance_name.to_lowercase())
                        .then_with(|| a.instance_id.cmp(&b.instance_id))
                });
                // The same server or world can be listed twice by one instance.
                merged.dedup_by(|a, b| a.instance_id == b.instance_id);
                *instances_of(&mut winner) = merged;
                groups.insert(key, winner);
            }
        }
    }
    order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .collect()
}

pub(super) fn collapse_worlds(worlds: Vec<WorldEntry>) -> Vec<WorldEntry> {
    collapse(
        worlds,
        |world| world.group_key.clone(),
        |world| &mut world.instances,
        |world| world.last_used_at_ms,
        |winner, loser| {
            winner.favorite |= loser.favorite;
            winner.last_used_at_ms = winner.last_used_at_ms.max(loser.last_used_at_ms);
            if winner.thumbnail_png.is_none() {
                winner.thumbnail_png = loser.thumbnail_png;
            }
        },
    )
}

pub(super) fn collapse_servers(servers: Vec<ServerEntry>) -> Vec<ServerEntry> {
    collapse(
        servers,
        |server| server.favorite_id.clone(),
        |server| &mut server.instances,
        |server| server.last_used_at_ms,
        |winner, loser| {
            winner.favorite |= loser.favorite;
            winner.last_used_at_ms = winner.last_used_at_ms.max(loser.last_used_at_ms);
            if winner.icon_png.is_none() {
                winner.icon_png = loser.icon_png;
            }
        },
    )
}

/// Instances of a grouped entry; empty for single-instance entries.
pub(super) fn group_instances<'a>(entry: &'a HomeEntryRef<'_>) -> &'a [EntryInstance] {
    let instances = match entry {
        HomeEntryRef::World(world) => &world.instances,
        HomeEntryRef::Server(server) => &server.instances,
    };
    if instances.len() > 1 { instances } else { &[] }
}

/// How the launch buttons are arranged for a given width.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct InstanceButtonsLayout {
    pub(super) columns: usize,
    pub(super) button_width: f32,
    pub(super) button_height: f32,
    pub(super) rows: usize,
    /// Total height including the space above the buttons; zero when there are none.
    pub(super) total_height: f32,
}

/// Buttons are all the same width, at least as wide as the longest instance name plus
/// padding, and flow onto more lines when that doesn't fit.
pub(super) fn instance_buttons_layout(
    ui: &Ui,
    text_ui: &mut TextUi,
    instances: &[EntryInstance],
    width: f32,
) -> InstanceButtonsLayout {
    if instances.is_empty() {
        return InstanceButtonsLayout::default();
    }
    let label_style = style::role(ui, config::TextRole::Button, false);
    let widest_label = instances
        .iter()
        .map(|instance| {
            text_ui
                .measure_text_size(ui, instance.instance_name.as_str(), &label_style)
                .x
        })
        .fold(0.0, f32::max);
    let padding = ButtonOptions::default().padding;
    let min_width = widest_label + padding.x * 2.0 + BUTTON_LABEL_PADDING;
    let width = width.max(min_width);
    let columns = (((width + BUTTON_GAP) / (min_width + BUTTON_GAP)).floor() as usize)
        .clamp(1, instances.len());
    let button_width = (width - BUTTON_GAP * (columns - 1) as f32) / columns as f32;
    let button_height = (label_style.line_height + padding.y * 2.0).max(style::CONTROL_HEIGHT);
    let rows = instances.len().div_ceil(columns);
    InstanceButtonsLayout {
        columns,
        button_width,
        button_height,
        rows,
        total_height: BUTTONS_TOP_SPACE
            + rows as f32 * button_height
            + rows.saturating_sub(1) as f32 * BUTTON_GAP,
    }
}

/// Draws the button rows into a rect of exactly `layout.total_height`, returning the
/// instance whose button was clicked.
pub(super) fn render_instance_buttons(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    instances: &[EntryInstance],
    layout: &InstanceButtonsLayout,
    enabled: bool,
) -> Option<EntryInstance> {
    if layout.rows == 0 {
        return None;
    }
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), layout.total_height),
        egui::Sense::hover(),
    );
    let mut clicked = None;
    let mut top = rect.top() + BUTTONS_TOP_SPACE;
    for row in instances.chunks(layout.columns) {
        for (column, instance) in row.iter().enumerate() {
            let left = rect.left() + column as f32 * (layout.button_width + BUTTON_GAP);
            let button_rect = egui::Rect::from_min_size(
                egui::pos2(left, top),
                egui::vec2(layout.button_width, layout.button_height),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(button_rect), |ui| {
                let options = style::neutral_button_with_min_size(ui, button_rect.size());
                let response = ui
                    .add_enabled_ui(enabled, |ui| {
                        text_ui.button(
                            ui,
                            (id_source, "instance_button", instance.instance_id.as_str()),
                            instance.instance_name.as_str(),
                            &options,
                        )
                    })
                    .inner;
                if response.clicked() {
                    clicked = Some(instance.clone());
                }
            });
        }
        top += layout.button_height + BUTTON_GAP;
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(instance: &str, name: &str, last_used: u64, favorite: bool) -> ServerEntry {
        ServerEntry {
            instance_id: instance.to_owned(),
            server_name: "Server".to_owned(),
            address: "play.example.net".to_owned(),
            favorite_id: "play.example.net".to_owned(),
            icon_png: None,
            last_used_at_ms: Some(last_used),
            favorite,
            instances: vec![EntryInstance {
                instance_id: instance.to_owned(),
                instance_name: name.to_owned(),
                target: "play.example.net".to_owned(),
                favorite,
            }],
        }
    }

    #[test]
    fn servers_in_several_instances_collapse_to_the_most_recent() {
        let merged = collapse_servers(vec![
            server("b", "Bravo", 10, false),
            server("a", "alpha", 30, true),
            server("c", "Charlie", 20, false),
        ]);
        assert_eq!(merged.len(), 1);
        let entry = &merged[0];
        assert_eq!(entry.instance_id, "a");
        assert!(entry.favorite);
        assert_eq!(entry.last_used_at_ms, Some(30));
        let names: Vec<&str> = entry
            .instances
            .iter()
            .map(|i| i.instance_name.as_str())
            .collect();
        assert_eq!(names, ["alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn distinct_servers_stay_separate() {
        let mut other = server("a", "Alpha", 5, false);
        other.favorite_id = "other.example.net".to_owned();
        assert_eq!(
            collapse_servers(vec![server("a", "Alpha", 1, false), other]).len(),
            2
        );
    }
}
