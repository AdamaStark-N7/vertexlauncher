use super::*;

pub(super) fn collect_pack_rows(
    state: &ModalState,
    instances: &InstanceStore,
    config: &Config,
) -> Vec<PackRow> {
    let root_of = |instance: &InstanceRecord| {
        instance_root_path(config.minecraft_installations_root_path(), instance)
    };
    let mut rows: BTreeMap<String, PackRow> = BTreeMap::new();
    let Some(this) = instances.find(&state.instance_id) else {
        return Vec::new();
    };
    for pack in scan_packs(&root_of(this)) {
        rows.insert(
            pack.name.clone(),
            PackRow {
                name: pack.name,
                support: pack.support,
                present_here: true,
                elsewhere: Vec::new(),
            },
        );
    }
    // Packs other syncing instances have, so they can be vouched for before they arrive.
    for other in &instances.instances {
        if other.id == state.instance_id
            || !instances.game_settings_sync.instances.contains(&other.id)
        {
            continue;
        }
        for pack in scan_packs(&root_of(other)) {
            let row = rows.entry(pack.name.clone()).or_insert_with(|| PackRow {
                name: pack.name.clone(),
                support: pack.support,
                present_here: false,
                elsewhere: Vec::new(),
            });
            row.elsewhere.push(other.name.clone());
        }
    }
    let mut rows: Vec<PackRow> = rows.into_values().collect();
    rows.sort_by_key(|row| row.name.to_lowercase());
    rows
}

pub(super) fn status_text(
    status: PackStatus,
    support: Option<PackSupport>,
    version: Option<Ver>,
) -> String {
    let format = version.and_then(mc_options::packs::resource_format_for);
    let format_note = format.map(|f| {
        format!(
            "Minecraft {} uses format {}.{}",
            version.map_or(String::new(), |v| format!("{}.{}", v.0, v.1)),
            f.major,
            f.minor
        )
    });
    match status {
        PackStatus::Compatible => "Compatible with this version".to_owned(),
        PackStatus::Incompatible => format!(
            "Not declared for this version{}",
            format_note.map(|n| format!(" ({n})")).unwrap_or_default()
        ),
        PackStatus::Unknown => {
            if support.is_none() {
                "No readable pack.mcmeta; compatibility unknown".to_owned()
            } else {
                "Compatibility unknown for this version".to_owned()
            }
        }
        PackStatus::ForcedAllow => "Marked as working here".to_owned(),
        PackStatus::ForcedDeny => "Marked as not working here".to_owned(),
    }
}

pub(super) fn render_packs_tab(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    state: &ModalState,
    instances: &mut InstanceStore,
    config: &Config,
) {
    let _ = text_ui.label(
        ui,
        "packs_intro",
        "Each pack declares which Minecraft versions it supports. You know your setup better than the metadata: mark a pack as working (or not) for this instance and sync will follow that instead. Overrides only affect sync; they don't change the pack or what the game allows.",
        &style::muted(ui),
    );
    if !instances.game_settings_sync.resource_packs {
        let _ = text_ui.label(
            ui,
            "packs_sync_off",
            "Resource pack sync is off. Turn it on in the Sync tab; overrides are remembered either way.",
            &style::warning_text(ui),
        );
    }
    ui.add_space(style::SPACE_LG);

    let rows = {
        let mut cache = state.pack_rows.lock().expect("pack row cache poisoned");
        let stale = cache
            .as_ref()
            .is_none_or(|(when, _)| when.elapsed() > Duration::from_secs(1));
        if stale {
            *cache = Some((
                Instant::now(),
                Arc::from(collect_pack_rows(state, instances, config)),
            ));
        }
        cache.as_ref().map(|(_, rows)| Arc::clone(rows))
    };
    let rows = match rows {
        Some(rows) => rows,
        None => Arc::from(Vec::new()),
    };
    if rows.is_empty() {
        let _ = text_ui.label(
            ui,
            "packs_empty",
            "No resource packs found.",
            &style::muted(ui),
        );
        return;
    }

    let mut changed = false;
    for row in rows.iter() {
        let status = pack_status(
            instances,
            &state.instance_id,
            &state.game_version,
            &row.name,
            row.support,
        );
        let verdict = instances.pack_override(&row.name, &state.instance_id);
        let _ = text_ui.label(
            ui,
            ("pack_name", row.name.as_str()),
            row.name.as_str(),
            &style::body_strong(ui),
        );
        let declared = row
            .support
            .map_or_else(|| "none".to_owned(), |s| s.serialize());
        let location = if row.present_here {
            "In this instance".to_owned()
        } else {
            format!("Only in {}", row.elsewhere.join(", "))
        };
        let detail = format!(
            "{location} · declared formats: {declared} · {}",
            status_text(status, row.support, state.version)
        );
        let _ = text_ui.label(
            ui,
            ("pack_detail", row.name.as_str()),
            detail.as_str(),
            &style::muted(ui),
        );
        ui.horizontal(|ui| {
            for (label, target) in [
                ("Works here", Some(PackOverride::Allow)),
                ("Doesn't work here", Some(PackOverride::Deny)),
                ("Use pack's metadata", None),
            ] {
                let selected = verdict == target;
                if settings_widgets::selectable_chip_button(
                    text_ui,
                    ui,
                    ("pack_override", row.name.as_str(), label),
                    label,
                    selected,
                    170.0,
                    true,
                )
                .clicked()
                    && !selected
                {
                    instances.set_pack_override(&row.name, &state.instance_id, target);
                    changed = true;
                }
            }
        });
        ui.add_space(style::SPACE_MD);
    }
    if changed {
        *state.pack_rows.lock().expect("pack row cache poisoned") = None;
        if instances.game_settings_sync.resource_packs {
            sync_runner::spawn(instances.clone(), SyncRunConfig::from_config(config));
        }
    }
}
