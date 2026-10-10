use super::*;

pub(crate) fn has_fabric_or_quilt_profile(
    instance_root: &Path,
    game_version: &str,
    loader_kind: LoaderKind,
    loader_version: &str,
) -> Result<bool, InstallationError> {
    let id_prefix = match loader_kind {
        LoaderKind::Fabric => "fabric-loader",
        LoaderKind::Quilt => "quilt-loader",
        _ => return Ok(false),
    };
    let version_id = format!("{id_prefix}-{loader_version}-{game_version}");
    let profile_path = instance_root
        .join("versions")
        .join(version_id.as_str())
        .join(format!("{version_id}.json"));
    if !profile_path.exists() {
        return Ok(false);
    }
    let raw = match fs_read_to_string(profile_path.as_path()) {
        Ok(contents) => contents,
        Err(_) => return Ok(false),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let id = parsed
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if id.eq_ignore_ascii_case(version_id.as_str()) {
        return Ok(true);
    }
    let inherits = parsed
        .get("inheritsFrom")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let game_version_lower = game_version.to_ascii_lowercase();
    let loader_version_lower = loader_version.to_ascii_lowercase();
    let id_lower = id.to_ascii_lowercase();
    Ok(id_lower.contains(loader_version_lower.as_str())
        && id_lower.contains(id_prefix)
        && (inherits == game_version_lower || inherits.starts_with(game_version_lower.as_str())))
}

pub(crate) fn verify_modloader_profile(
    instance_root: &Path,
    loader_kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
) -> Result<bool, InstallationError> {
    let versions_dir = instance_root.join("versions");
    if !versions_dir.exists() {
        return Ok(false);
    }
    let loader_hint = match loader_kind {
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        _ => return Ok(true),
    };
    for entry in fs_read_dir(&versions_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        let profile_path = entry.path().join(format!("{dir_name}.json"));
        if !profile_path.exists() {
            continue;
        }
        let raw = fs_read_to_string(&profile_path)?;
        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let id = parsed
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let inherits = parsed
            .get("inheritsFrom")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let game_version_lower = game_version.to_ascii_lowercase();
        let loader_version_lower = loader_version.to_ascii_lowercase();
        let matches_loader = id.contains(loader_hint)
            || (loader_kind == LoaderKind::NeoForge && id.contains("forge"));
        let matches_version = id.contains(loader_version_lower.as_str());
        let matches_game = id.contains(game_version_lower.as_str())
            || inherits == game_version_lower
            || inherits.starts_with(game_version_lower.as_str());
        if matches_loader && matches_version && matches_game {
            return Ok(true);
        }
    }
    Ok(false)
}
