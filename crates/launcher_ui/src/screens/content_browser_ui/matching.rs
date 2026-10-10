use super::*;

pub(in crate::screens::content_browser) fn installed_project_for_entry<'a>(
    manifest: &'a ContentInstallManifest,
    entry: &BrowserProjectEntry,
) -> Option<(&'a str, &'a InstalledContentProject)> {
    manifest
        .projects
        .get_key_value(entry.dedupe_key.as_str())
        .map(|(key, project)| (key.as_str(), project))
        .or_else(|| {
            manifest
                .projects
                .iter()
                .find(|(_, project)| installed_project_matches_entry(project, entry))
                .map(|(key, project)| (key.as_str(), project))
        })
}

fn installed_project_matches_entry(
    project: &InstalledContentProject,
    entry: &BrowserProjectEntry,
) -> bool {
    if let (Some(project_id), Some(entry_project_id)) = (
        project.modrinth_project_id.as_deref(),
        entry.modrinth_project_id.as_deref(),
    ) && project_id == entry_project_id
    {
        return true;
    }

    if let (Some(project_id), Some(entry_project_id)) =
        (project.curseforge_project_id, entry.curseforge_project_id)
        && project_id == entry_project_id
    {
        return true;
    }

    false
}

pub(super) fn version_matches_loader(version: &BrowserVersionEntry, loader: BrowserLoader) -> bool {
    if loader == BrowserLoader::Any || version.loaders.is_empty() {
        return true;
    }
    let Some(expected) = loader.modrinth_slug() else {
        return true;
    };
    version
        .loaders
        .iter()
        .any(|value| normalize_search_key(value).contains(expected))
}

pub(super) fn version_matches_game_version(
    version: &BrowserVersionEntry,
    game_version_filter: &str,
) -> bool {
    let filter = game_version_filter.trim();
    if filter.is_empty() || version.game_versions.is_empty() {
        return true;
    }
    version
        .game_versions
        .iter()
        .any(|value| value.trim() == filter)
}

pub(in crate::screens::content_browser) fn browser_loader_from_modloader(
    modloader: &str,
) -> BrowserLoader {
    match modloader.trim().to_ascii_lowercase().as_str() {
        "fabric" => BrowserLoader::Fabric,
        "forge" => BrowserLoader::Forge,
        "neoforge" => BrowserLoader::NeoForge,
        "quilt" => BrowserLoader::Quilt,
        _ => BrowserLoader::Any,
    }
}

pub(in crate::screens::content_browser) fn normalize_search_key(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();
    normalize_inline_whitespace(normalized.as_str())
}

pub(in crate::screens::content_browser) fn parse_content_type(
    value: &str,
) -> Option<BrowserContentType> {
    let normalized = normalize_search_key(value);
    if normalized.contains("shader") {
        Some(BrowserContentType::Shader)
    } else if normalized.contains("resource pack")
        || normalized.contains("resourcepack")
        || normalized.contains("texture pack")
        || normalized.contains("texturepack")
    {
        Some(BrowserContentType::ResourcePack)
    } else if normalized.contains("data pack") || normalized.contains("datapack") {
        Some(BrowserContentType::DataPack)
    } else if normalized.contains("mod") {
        Some(BrowserContentType::Mod)
    } else {
        None
    }
}
