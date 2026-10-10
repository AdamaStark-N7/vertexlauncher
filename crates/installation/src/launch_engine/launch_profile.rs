use super::*;

pub(crate) fn prepare_launch_log_file(
    instance_root: &Path,
) -> Result<(std::fs::File, PathBuf), InstallationError> {
    let logs_dir = instance_root.join("logs");
    fs_create_dir_all(&logs_dir)?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let log_path = logs_dir.join(format!("launch_{timestamp_ms}.log"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    Ok((file, log_path))
}

pub(crate) fn resolve_launch_profile_path(
    instance_root: &Path,
    game_version: &str,
    modloader: &str,
    modloader_version: Option<&str>,
) -> Result<(String, PathBuf), InstallationError> {
    let versions_dir = instance_root.join("versions");
    let requested_loader = normalized_loader_label(modloader);
    let allow_vanilla_fallback =
        matches!(requested_loader, LoaderKind::Vanilla | LoaderKind::Custom);
    tracing::info!(
        target: "vertexlauncher/installation/launch_profile",
        requested_modloader = %modloader,
        requested_game_version = %game_version,
        requested_modloader_version = %modloader_version.unwrap_or(""),
        allow_vanilla_fallback,
        "Resolving launch profile."
    );
    let mut candidates = Vec::<(String, PathBuf)>::new();

    if allow_vanilla_fallback {
        let game_path = versions_dir
            .join(game_version)
            .join(format!("{game_version}.json"));
        if game_path.exists() {
            candidates.push((game_version.to_owned(), game_path));
        }
    }

    if matches!(requested_loader, LoaderKind::Fabric | LoaderKind::Quilt)
        && let Some(loader_version) = modloader_version.map(str::trim).filter(|v| !v.is_empty())
    {
        let prefix = if requested_loader == LoaderKind::Fabric {
            "fabric-loader"
        } else {
            "quilt-loader"
        };
        let id = format!("{prefix}-{loader_version}-{game_version}");
        let path = versions_dir.join(id.as_str()).join(format!("{id}.json"));
        if path.exists() {
            candidates.insert(0, (id, path));
        }
    }

    let loader_hint = match requested_loader {
        LoaderKind::Forge => Some("forge"),
        LoaderKind::NeoForge => Some("neoforge"),
        LoaderKind::Fabric => Some("fabric-loader"),
        LoaderKind::Quilt => Some("quilt-loader"),
        LoaderKind::Vanilla | LoaderKind::Custom => None,
    };
    if let Some(loader_hint) = loader_hint
        && versions_dir.exists()
    {
        for entry in fs_read_dir(&versions_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let lower = dir_name.to_ascii_lowercase();
            if !lower.contains(loader_hint) {
                continue;
            }
            let game_lower = game_version.to_ascii_lowercase();
            if !lower.contains(game_lower.as_str()) {
                let profile_path = entry.path().join(format!("{dir_name}.json"));
                if !profile_path.exists() {
                    continue;
                }
                let raw = fs_read_to_string(&profile_path)?;
                let parsed: serde_json::Value = serde_json::from_str(&raw)?;
                let inherits = parsed
                    .get("inheritsFrom")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if !inherits.starts_with(game_lower.as_str()) && inherits != game_lower {
                    continue;
                }
            }
            if let Some(loader_version) = modloader_version.map(str::trim).filter(|v| !v.is_empty())
            {
                let lv = loader_version.to_ascii_lowercase();
                if !lower.contains(lv.as_str()) {
                    continue;
                }
            }
            let profile_path = entry.path().join(format!("{dir_name}.json"));
            if profile_path.exists() {
                candidates.insert(0, (dir_name, profile_path));
            }
        }
    }

    let resolved = candidates
        .into_iter()
        .find(|(_, path)| path.exists())
        .ok_or_else(|| InstallationError::LaunchProfileMissing {
            modloader: modloader.to_owned(),
            game_version: game_version.to_owned(),
        })?;
    tracing::info!(
        target: "vertexlauncher/installation/launch_profile",
        profile_id = %resolved.0,
        profile_path = %resolved.1.display(),
        "Resolved launch profile."
    );
    Ok(resolved)
}

pub(crate) fn load_profile_chain(
    instance_root: &Path,
    profile_path: &Path,
) -> Result<Vec<serde_json::Value>, InstallationError> {
    let mut chain = Vec::new();
    let mut cursor = profile_path.to_path_buf();
    let mut guard = 0usize;
    while guard < 16 {
        let raw = fs_read_to_string(cursor.as_path())?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)?;
        let inherits = parsed
            .get("inheritsFrom")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned);
        chain.push(parsed);
        let Some(parent) = inherits else {
            break;
        };
        cursor = instance_root
            .join("versions")
            .join(parent.as_str())
            .join(format!("{parent}.json"));
        guard = guard.saturating_add(1);
    }
    chain.reverse();
    Ok(chain)
}

pub(crate) fn resolve_main_class(chain: &[serde_json::Value]) -> Option<String> {
    for profile in chain.iter().rev() {
        if let Some(main_class) = profile.get("mainClass").and_then(serde_json::Value::as_str) {
            let trimmed = main_class.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}
