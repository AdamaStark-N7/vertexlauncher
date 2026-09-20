//! In-game settings sync: translates `options.txt` between instances of different versions.

use std::collections::HashSet;
use std::time::SystemTime;

use instances::{GameSettingsSync, InstanceStore};
use mc_options::schema::Group;
use mc_options::translate::plan_sync;
use mc_options::{OptionsFile, parse_game_version};

use crate::fs_util::{modified, write_atomic_bytes};
use crate::{SyncInstance, SyncReport};

const OPTIONS_FILE: &str = "options.txt";

fn allowed_group(sync: &GameSettingsSync) -> impl Fn(Group) -> bool + '_ {
    move |group| sync.categories.contains(group.slug())
}

/// Brings opted-in instances in line with the most recently saved one.
///
/// Files this pass writes keep the source's modification time, so a translated copy is never
/// mistaken for a newer edit and echoed back (which would round-trip lossy conversions such
/// as old 4-step render distance).
pub(crate) fn sync(
    instances: &[SyncInstance],
    store: &InstanceStore,
    skip: &HashSet<String>,
    report: &mut SyncReport,
) {
    let sync = &store.game_settings_sync;
    let participants: Vec<&SyncInstance> = instances
        .iter()
        .filter(|instance| sync.instances.contains(&instance.id))
        .collect();
    if participants.len() < 2 {
        return;
    }

    struct Loaded<'a> {
        instance: &'a SyncInstance,
        file: OptionsFile,
        modified: SystemTime,
    }
    let mut loaded: Vec<Loaded<'_>> = Vec::new();
    for instance in participants {
        let path = instance.root.join(OPTIONS_FILE);
        let (Ok(file), Some(modified)) = (OptionsFile::read(&path), modified(&path)) else {
            continue;
        };
        loaded.push(Loaded {
            instance,
            file,
            modified,
        });
    }
    let Some(master) = loaded.iter().max_by_key(|entry| entry.modified) else {
        return;
    };
    let master_file = master.file.clone();
    let master_version = parse_game_version(&master.instance.game_version);
    let master_modified = master.modified;
    let master_id = master.instance.id.clone();
    let allowed = allowed_group(sync);

    for entry in &mut loaded {
        // Equal timestamps mean "already in step" (see the note on `sync`).
        if entry.instance.id == master_id
            || entry.modified >= master_modified
            || skip.contains(&entry.instance.id)
        {
            continue;
        }
        let writes = plan_sync(
            &master_file,
            master_version,
            &entry.file,
            parse_game_version(&entry.instance.game_version),
            &allowed,
        );
        let path = entry.instance.root.join(OPTIONS_FILE);
        if writes.is_empty() {
            // Nothing to change; still mark it as in step so it stays quiet.
            continue;
        }
        for (key, raw) in &writes {
            entry.file.set(key, raw);
        }
        match write_atomic_bytes(
            &path,
            entry.file.serialize().as_bytes(),
            Some(master_modified),
        ) {
            Ok(()) => report.settings_files += 1,
            Err(err) => report.errors.push(format!(
                "{}: failed to write {OPTIONS_FILE}: {err}",
                entry.instance.id
            )),
        }
    }
}
