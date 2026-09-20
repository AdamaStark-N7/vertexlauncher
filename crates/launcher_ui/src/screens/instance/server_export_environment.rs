//! Leaves client-only mods out of server exports, using Modrinth's `environment` field.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use modrinth::Environment;

/// Projects per `/projects` request, keeping the query string a sensible length.
const PROJECT_BATCH: usize = 50;

/// Mods that a dedicated server shouldn't load.
#[derive(Default)]
pub(super) struct ClientOnlyMods {
    /// Files to leave out of the export.
    pub(super) excluded: HashSet<PathBuf>,
    /// One human-readable line per excluded mod, for the build report.
    pub(super) descriptions: Vec<String>,
    /// Set when Modrinth couldn't be reached; nothing is excluded in that case.
    pub(super) lookup_error: Option<String>,
}

fn is_mod_jar(instance_root: &Path, file: &Path) -> bool {
    file.strip_prefix(instance_root.join("mods")).is_ok()
        && file
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
}

/// Finds mods among `files` whose Modrinth environment says they don't run on a dedicated
/// server. Mods Modrinth doesn't know, or marks unknown, are always kept: shipping a client
/// mod is a nuisance, dropping a needed one breaks the server.
pub(super) fn find_client_only_mods(
    instance_root: &Path,
    manifest: &managed_content::ContentInstallManifest,
    files: &[PathBuf],
) -> ClientOnlyMods {
    let jars: Vec<&PathBuf> = files
        .iter()
        .filter(|file| is_mod_jar(instance_root, file))
        .collect();
    if jars.is_empty() {
        return ClientOnlyMods::default();
    }

    // Managed mods already know their project; hash the rest.
    let managed: HashMap<PathBuf, String> = manifest
        .projects
        .values()
        .filter_map(|project| {
            Some((
                instance_root.join(project.file_path.as_path()),
                project.modrinth_project_id.clone()?,
            ))
        })
        .collect();
    let mut project_of: HashMap<&PathBuf, String> = HashMap::new();
    let mut version_env: HashMap<&PathBuf, Environment> = HashMap::new();
    let mut to_hash: Vec<(&PathBuf, String)> = Vec::new();
    for jar in &jars {
        match managed.get(*jar) {
            Some(project_id) => {
                project_of.insert(*jar, project_id.clone());
            }
            None => {
                if let Ok(hash) = modrinth::hash_file_sha1_hex(jar.as_path()) {
                    to_hash.push((*jar, hash));
                }
            }
        }
    }

    let client = modrinth::Client::default();
    if !to_hash.is_empty() {
        let hashes: Vec<String> = to_hash.iter().map(|(_, hash)| hash.clone()).collect();
        match client.get_versions_from_hashes(&hashes, "sha1") {
            Ok(found) => {
                for (jar, hash) in &to_hash {
                    if let Some(version) = found.get(hash) {
                        project_of.insert(*jar, version.project_id.clone());
                        version_env.insert(*jar, version.environment);
                    }
                }
            }
            Err(err) => {
                return ClientOnlyMods {
                    lookup_error: Some(err.to_string()),
                    ..ClientOnlyMods::default()
                };
            }
        }
    }

    let mut project_ids: Vec<String> = project_of.values().cloned().collect();
    project_ids.sort();
    project_ids.dedup();
    let mut project_env: HashMap<String, Environment> = HashMap::new();
    for chunk in project_ids.chunks(PROJECT_BATCH) {
        match client.get_projects(chunk) {
            Ok(projects) => {
                for project in projects {
                    project_env.insert(project.project_id, project.environment);
                }
            }
            Err(err) => {
                return ClientOnlyMods {
                    lookup_error: Some(err.to_string()),
                    ..ClientOnlyMods::default()
                };
            }
        }
    }

    let mut result = ClientOnlyMods::default();
    for jar in jars {
        let Some(project_id) = project_of.get(jar) else {
            continue;
        };
        // The version's own environment is the most specific; fall back to the project's.
        let environment = match version_env.get(jar).copied() {
            Some(env) if env != Environment::Unknown => env,
            _ => project_env.get(project_id).copied().unwrap_or_default(),
        };
        if !environment.runs_on_dedicated_server() {
            result.excluded.insert(jar.clone());
            let name = jar
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            result
                .descriptions
                .push(format!("{name} ({environment:?})"));
        }
    }
    result.descriptions.sort();
    result
}
