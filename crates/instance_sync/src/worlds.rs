//! Worlds shared between instances.
//!
//! A synced world's data lives once in the shared worlds folder. Each member instance's
//! `saves/<name>` is a symlink to it (a junction on Windows). Where linking isn't possible
//! the member gets a real copy that [`mirror_pass`] keeps in step (newest save wins).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use instances::{InstanceStore, SyncedWorld, WorldLinkMode, instance_root_path};

use crate::SyncReport;
use crate::fs_util::{
    copy_dir_recursive, create_dir_link, is_link, move_dir, remove_link, world_freshness,
};

/// Outcome of a world sync operation. `warnings` are per-instance problems that didn't stop
/// the rest of the operation (conflicts, fallbacks to mirroring).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldSyncOutcome {
    pub warnings: Vec<String>,
}

/// Where worlds are and how to reach them, shared by all operations.
pub struct WorldPaths<'a> {
    pub installations_root: &'a Path,
    pub shared_root: &'a Path,
}

impl WorldPaths<'_> {
    fn saves_path(
        &self,
        store: &InstanceStore,
        instance_id: &str,
        folder: &str,
    ) -> Option<PathBuf> {
        let instance = store.find(instance_id)?;
        Some(
            instance_root_path(self.installations_root, instance)
                .join("saves")
                .join(folder),
        )
    }

    fn shared_path(&self, world_id: &str) -> PathBuf {
        self.shared_root.join(world_id)
    }
}

fn instance_label(store: &InstanceStore, id: &str) -> String {
    store
        .find(id)
        .map_or_else(|| id.to_owned(), |instance| instance.name.clone())
}

fn ensure_stopped(
    store: &InstanceStore,
    ids: impl IntoIterator<Item = impl AsRef<str>>,
    running: &HashSet<String>,
) -> Result<(), String> {
    for id in ids {
        if running.contains(id.as_ref()) {
            return Err(format!(
                "Close {} first; a running game can't have its worlds moved.",
                instance_label(store, id.as_ref())
            ));
        }
    }
    Ok(())
}

fn new_world_id(folder: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let slug: String = folder
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("{}-{millis}", slug.trim_matches('-'))
}

/// Puts the shared world at `saves_path`: a link when possible, else a mirrored copy.
fn place_member(shared: &Path, saves_path: &Path) -> Result<WorldLinkMode, String> {
    match create_dir_link(shared, saves_path) {
        Ok(()) => Ok(WorldLinkMode::Symlink),
        Err(link_err) => {
            copy_dir_recursive(shared, saves_path).map_err(|copy_err| {
                let _ = fs::remove_dir_all(saves_path);
                format!("could not link ({link_err}) or copy ({copy_err})")
            })?;
            Ok(WorldLinkMode::Mirror)
        }
    }
}

fn describe_mode(mode: WorldLinkMode) -> Option<&'static str> {
    (mode == WorldLinkMode::Mirror).then_some(
        "links aren't available here, so it holds a copy that is refreshed at launch and exit",
    )
}

/// Starts syncing `saves/<folder>` of `source_id`: moves it to the shared folder, links it
/// back, and links or copies it into each of `other_members`.
pub fn enable_world_sync(
    store: &mut InstanceStore,
    paths: &WorldPaths<'_>,
    source_id: &str,
    folder: &str,
    other_members: &BTreeSet<String>,
    running: &HashSet<String>,
) -> Result<WorldSyncOutcome, String> {
    let mut all_ids: BTreeSet<&str> = other_members.iter().map(String::as_str).collect();
    all_ids.insert(source_id);
    ensure_stopped(store, all_ids, running)?;
    if store.synced_world_for(source_id, folder).is_some() {
        return Err("This world is already synced.".to_owned());
    }
    let source_path = paths
        .saves_path(store, source_id, folder)
        .ok_or_else(|| "Instance not found.".to_owned())?;
    if is_link(&source_path) || !source_path.is_dir() {
        return Err(format!("{folder} is not a regular world folder."));
    }

    let id = new_world_id(folder);
    let shared = paths.shared_path(&id);
    move_dir(&source_path, &shared).map_err(|err| format!("Could not move the world: {err}"))?;

    let mut outcome = WorldSyncOutcome::default();
    let mut members = BTreeMap::new();
    match place_member(&shared, &source_path) {
        Ok(mode) => {
            members.insert(source_id.to_owned(), mode);
        }
        Err(err) => {
            // Put the world back so nothing is lost.
            let _ = move_dir(&shared, &source_path);
            return Err(format!(
                "Could not link the world back into its instance: {err}"
            ));
        }
    }
    for member in other_members
        .iter()
        .filter(|member| member.as_str() != source_id)
    {
        let label = instance_label(store, member);
        let Some(path) = paths.saves_path(store, member, folder) else {
            continue;
        };
        if path.exists() || is_link(&path) {
            outcome.warnings.push(format!(
                "{label} already has a world named {folder}; rename it to sync them."
            ));
            continue;
        }
        match place_member(&shared, &path) {
            Ok(mode) => {
                if let Some(note) = describe_mode(mode) {
                    outcome.warnings.push(format!("{label}: {note}."));
                }
                members.insert(member.clone(), mode);
            }
            Err(err) => outcome.warnings.push(format!("{label}: {err}.")),
        }
    }
    store.synced_worlds.push(SyncedWorld {
        id,
        folder_name: folder.to_owned(),
        members,
    });
    Ok(outcome)
}

/// Turns one member back into an independent real copy of the world.
fn detach_member(
    paths: &WorldPaths<'_>,
    store: &InstanceStore,
    world: &SyncedWorld,
    member: &str,
) -> Result<(), String> {
    let Some(path) = paths.saves_path(store, member, &world.folder_name) else {
        return Ok(());
    };
    if is_link(&path) {
        remove_link(&path).map_err(|err| format!("could not remove the link: {err}"))?;
        copy_dir_recursive(&paths.shared_path(&world.id), &path)
            .map_err(|err| format!("could not restore a copy: {err}"))?;
    }
    // Mirrored members already hold a real (possibly stale) copy; refresh it first.
    Ok(())
}

/// Stops syncing a world: every member gets an independent copy and the shared data is
/// deleted only after all copies succeed.
pub fn disable_world_sync(
    store: &mut InstanceStore,
    paths: &WorldPaths<'_>,
    world_id: &str,
    running: &HashSet<String>,
) -> Result<WorldSyncOutcome, String> {
    let world = store
        .synced_worlds
        .iter()
        .find(|world| world.id == world_id)
        .cloned()
        .ok_or_else(|| "World is not synced.".to_owned())?;
    ensure_stopped(store, world.members.keys(), running)?;
    refresh_mirrors_of(store, paths, &world, running);
    for member in world.members.keys() {
        detach_member(paths, store, &world, member)
            .map_err(|err| format!("{}: {err}", instance_label(store, member)))?;
    }
    let _ = fs::remove_dir_all(paths.shared_path(&world.id));
    store.synced_worlds.retain(|entry| entry.id != world.id);
    Ok(WorldSyncOutcome::default())
}

/// Changes which instances share a world: new members get a link or copy, removed members
/// keep an independent copy.
pub fn set_world_members(
    store: &mut InstanceStore,
    paths: &WorldPaths<'_>,
    world_id: &str,
    wanted: &BTreeSet<String>,
    running: &HashSet<String>,
) -> Result<WorldSyncOutcome, String> {
    let world = store
        .synced_worlds
        .iter()
        .find(|world| world.id == world_id)
        .cloned()
        .ok_or_else(|| "World is not synced.".to_owned())?;
    let current: BTreeSet<&String> = world.members.keys().collect();
    let changing: Vec<&String> = wanted
        .iter()
        .filter(|id| !current.contains(id))
        .chain(current.iter().copied().filter(|id| !wanted.contains(*id)))
        .collect();
    ensure_stopped(store, changing.iter().map(|id| id.as_str()), running)?;
    if wanted.is_empty() {
        return disable_world_sync(store, paths, world_id, running);
    }

    let mut outcome = WorldSyncOutcome::default();
    let mut members = world.members.clone();
    refresh_mirrors_of(store, paths, &world, running);
    for removed in world.members.keys().filter(|id| !wanted.contains(*id)) {
        detach_member(paths, store, &world, removed)
            .map_err(|err| format!("{}: {err}", instance_label(store, removed)))?;
        members.remove(removed);
    }
    let shared = paths.shared_path(&world.id);
    for added in wanted.iter().filter(|id| !world.members.contains_key(*id)) {
        let label = instance_label(store, added);
        let Some(path) = paths.saves_path(store, added, &world.folder_name) else {
            continue;
        };
        if path.exists() || is_link(&path) {
            outcome.warnings.push(format!(
                "{label} already has a world named {}; rename it to sync them.",
                world.folder_name
            ));
            continue;
        }
        match place_member(&shared, &path) {
            Ok(mode) => {
                if let Some(note) = describe_mode(mode) {
                    outcome.warnings.push(format!("{label}: {note}."));
                }
                members.insert(added.clone(), mode);
            }
            Err(err) => outcome.warnings.push(format!("{label}: {err}.")),
        }
    }
    if let Some(entry) = store
        .synced_worlds
        .iter_mut()
        .find(|entry| entry.id == world.id)
    {
        entry.members = members;
    }
    Ok(outcome)
}

fn refresh_mirrors_of(
    store: &InstanceStore,
    paths: &WorldPaths<'_>,
    world: &SyncedWorld,
    running: &HashSet<String>,
) {
    let mut report = SyncReport::default();
    mirror_world(store, paths, world, running, &mut report);
}

/// Replaces `dst` with a copy of `src`, keeping the previous `dst` as a one-deep backup.
fn replace_with_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    let name = dst
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let staging = dst.with_file_name(format!("{name}.vertex-sync-new"));
    let backup = dst.with_file_name(format!("{name}.vertex-sync-backup"));
    let _ = fs::remove_dir_all(&staging);
    copy_dir_recursive(src, &staging)?;
    let _ = fs::remove_dir_all(&backup);
    if dst.exists() {
        fs::rename(dst, &backup)?;
    }
    fs::rename(&staging, dst)
}

/// Brings mirrored members and the shared copy in line, newest save winning.
fn mirror_world(
    store: &InstanceStore,
    paths: &WorldPaths<'_>,
    world: &SyncedWorld,
    running: &HashSet<String>,
    report: &mut SyncReport,
) {
    let shared = paths.shared_path(&world.id);
    let mirrors: Vec<(&String, PathBuf)> = world
        .members
        .iter()
        .filter(|(_, mode)| **mode == WorldLinkMode::Mirror)
        .filter_map(|(id, _)| Some((id, paths.saves_path(store, id, &world.folder_name)?)))
        .filter(|(id, path)| !running.contains(*id) && path.is_dir() && !is_link(path))
        .collect();

    // 1. The freshest idle mirror updates the shared copy.
    let freshest = mirrors
        .iter()
        .filter_map(|(id, path)| Some((id, path, world_freshness(path)?)))
        .max_by_key(|(_, _, fresh)| *fresh);
    if let Some((id, path, fresh)) = freshest
        && world_freshness(&shared).is_none_or(|shared_fresh| fresh > shared_fresh)
    {
        match replace_with_copy(path, &shared) {
            Ok(()) => report.worlds_updated += 1,
            Err(err) => report.errors.push(format!(
                "{}: could not update shared world {}: {err}",
                instance_label(store, id),
                world.folder_name
            )),
        }
    }
    // 2. The shared copy refreshes any older mirror.
    let shared_fresh = world_freshness(&shared);
    for (id, path) in &mirrors {
        let stale = match (world_freshness(path), shared_fresh) {
            (Some(mine), Some(shared)) => shared > mine,
            (None, Some(_)) => true,
            _ => false,
        };
        if stale {
            match replace_with_copy(&shared, path) {
                Ok(()) => report.worlds_updated += 1,
                Err(err) => report.errors.push(format!(
                    "{}: could not refresh world {}: {err}",
                    instance_label(store, id),
                    world.folder_name
                )),
            }
        }
    }
}

/// Keeps every world with mirrored members in step. Linked members need no work.
pub(crate) fn mirror_pass(
    store: &InstanceStore,
    installations_root: &Path,
    shared_root: &Path,
    running: &HashSet<String>,
    report: &mut SyncReport,
) {
    let paths = WorldPaths {
        installations_root,
        shared_root,
    };
    for world in &store.synced_worlds {
        if world
            .members
            .values()
            .any(|mode| *mode == WorldLinkMode::Mirror)
        {
            mirror_world(store, &paths, world, running, report);
        }
    }
}
