use super::*;

pub(crate) fn build_classpath_entries(
    instance_root: &Path,
    profile_id: &str,
    game_version: &str,
    main_class: &str,
    chain: &[serde_json::Value],
) -> Result<Vec<PathBuf>, InstallationError> {
    let mut classpath = Vec::<PathBuf>::new();
    let mut library_indices = HashMap::<String, usize>::new();

    for profile in chain {
        let Some(libraries) = profile
            .get("libraries")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for lib in libraries {
            if !library_rules_allow(lib) {
                continue;
            }
            let artifact_path = lib
                .get("downloads")
                .and_then(|v| v.get("artifact"))
                .and_then(|v| v.get("path"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .or_else(|| resolve_library_maven_download(lib).map(|(_, path)| path));
            let Some(artifact_path) = artifact_path else {
                continue;
            };
            let full = instance_root.join("libraries").join(artifact_path.as_str());
            if full.exists() {
                let dedupe_key = library_classpath_dedupe_key(lib, artifact_path.as_str());
                if let Some(existing_index) = library_indices.get(dedupe_key.as_str()).copied() {
                    classpath[existing_index] = full;
                } else {
                    library_indices.insert(dedupe_key, classpath.len());
                    classpath.push(full);
                }
            }
        }
    }

    let launch_jar = instance_root
        .join("versions")
        .join(profile_id)
        .join(format!("{profile_id}.jar"));
    if launch_jar.exists() {
        classpath.push(launch_jar);
    } else {
        // Forge 1.17+ (BootstrapLauncher) and NeoForge use the JPMS module system.
        // They load game classes via JarJar/FML — adding the vanilla jar to the
        // classpath would create a duplicate module (_1._20._1 vs minecraft) and
        // crash at startup. Skip the game jar entirely for these loaders.
        let uses_bootstrap_launcher = main_class.contains("BootstrapLauncher");
        if !uses_bootstrap_launcher {
            let fallback_jar = instance_root
                .join("versions")
                .join(game_version)
                .join(format!("{game_version}.jar"));
            if fallback_jar.exists() {
                classpath.push(fallback_jar);
            } else {
                return Err(InstallationError::LaunchFileMissing {
                    profile_id: profile_id.to_owned(),
                    path: launch_jar,
                });
            }
        }
    }
    Ok(classpath)
}

pub(crate) fn join_classpath(entries: &[PathBuf]) -> String {
    entries
        .iter()
        .map(|entry| display_user_path(entry.as_path()))
        .collect::<Vec<_>>()
        .join(classpath_separator())
}

pub(crate) struct LaunchClasspath {
    pub(crate) resolved: String,
    pub(crate) argfile: Option<PathBuf>,
}

pub(crate) fn prepare_launch_classpath(
    instance_root: &Path,
    profile_id: &str,
    entries: &[PathBuf],
) -> Result<LaunchClasspath, InstallationError> {
    let joined = join_classpath(entries);
    if joined.len() <= 7000 {
        return Ok(LaunchClasspath {
            resolved: joined,
            argfile: None,
        });
    }

    let argfile = write_classpath_argfile(instance_root, profile_id, joined.as_str())?;
    Ok(LaunchClasspath {
        resolved: joined,
        argfile: Some(argfile),
    })
}

pub(crate) fn write_classpath_argfile(
    instance_root: &Path,
    profile_id: &str,
    classpath: &str,
) -> Result<PathBuf, InstallationError> {
    let cache_dir = instance_root.join(".vertexlauncher");
    fs_create_dir_all(&cache_dir)?;
    let argfile_path = cache_dir.join(format!("classpath-{profile_id}.args"));
    let argfile_contents = format!("-cp\n{}\n", quote_java_argfile_value(classpath));
    fs_write(argfile_path.as_path(), argfile_contents.as_bytes())?;
    Ok(argfile_path)
}

pub(crate) fn quote_command_arg(arg: &str) -> String {
    if arg.is_empty()
        || arg
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\''))
    {
        format!("{arg:?}")
    } else {
        arg.to_owned()
    }
}

pub(crate) fn quote_java_argfile_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub(crate) fn strip_explicit_classpath_args(args: &mut Vec<String>) {
    let mut filtered = Vec::with_capacity(args.len());
    let mut index = 0usize;
    while index < args.len() {
        let current = args[index].as_str();
        if current == "-cp" || current == "-classpath" {
            index += 1;
            if index < args.len() {
                index += 1;
            }
            continue;
        }
        filtered.push(args[index].clone());
        index += 1;
    }
    *args = filtered;
}

pub(crate) fn has_explicit_classpath_args(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "-cp" | "-classpath"))
}

pub(crate) fn library_classpath_dedupe_key(lib: &serde_json::Value, artifact_path: &str) -> String {
    if let Some(name) = lib.get("name").and_then(serde_json::Value::as_str) {
        let mut parts = name.split(':');
        if let (Some(group), Some(artifact)) = (parts.next(), parts.next()) {
            let group = group.trim();
            let artifact = artifact.trim();
            if !group.is_empty() && !artifact.is_empty() {
                let classifier = parts
                    .nth(1)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("");
                return format!("{group}:{artifact}:{classifier}");
            }
        }
    }
    artifact_path.to_owned()
}

pub(crate) fn classpath_separator() -> &'static str {
    if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    }
}

pub(crate) fn should_use_environment_classpath() -> bool {
    false
}
