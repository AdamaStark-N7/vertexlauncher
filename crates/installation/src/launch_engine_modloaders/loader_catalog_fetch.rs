use super::*;

pub(crate) fn fetch_fabric_versions() -> Result<HashSet<String>, InstallationError> {
    let versions: Vec<FabricGameVersion> = get_json(FABRIC_GAME_VERSIONS_URL)?;
    Ok(versions
        .into_iter()
        .map(|version| version.version.trim().to_owned())
        .filter(|version| !version.is_empty())
        .collect())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LoaderVersionFetchResult {
    pub(crate) selected_versions: Vec<String>,
    pub(crate) versions_by_game_version: BTreeMap<String, Vec<String>>,
}

pub(crate) fn fetch_fabric_loader_catalog() -> Result<LoaderVersionCatalog, InstallationError> {
    let matrix: serde_json::Value = get_json(FABRIC_VERSION_MATRIX_URL)?;
    Ok(parse_loader_version_matrix(&matrix))
}

pub(crate) fn fetch_quilt_versions() -> Result<HashSet<String>, InstallationError> {
    let versions: Vec<QuiltGameVersion> = get_json(QUILT_GAME_VERSIONS_URL)?;
    Ok(versions
        .into_iter()
        .filter_map(|version| {
            let id = version.version.or(version.id)?;
            let trimmed = id.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
        .collect())
}

pub(crate) fn fetch_quilt_loader_catalog() -> Result<LoaderVersionCatalog, InstallationError> {
    let matrix: serde_json::Value = get_json(QUILT_VERSION_MATRIX_URL)?;
    Ok(parse_loader_version_matrix(&matrix))
}

pub(crate) fn fetch_forge_versions() -> Result<HashSet<String>, InstallationError> {
    let metadata = get_text(FORGE_MAVEN_METADATA_URL)?;
    Ok(parse_minecraft_versions_from_maven_metadata(
        &metadata, true,
    ))
}

pub(crate) fn fetch_forge_loader_catalog() -> Result<LoaderVersionCatalog, InstallationError> {
    let metadata = get_text(FORGE_MAVEN_METADATA_URL)?;
    Ok(parse_forge_loader_catalog_from_metadata(&metadata))
}

pub(crate) fn fetch_neoforge_versions() -> Result<HashSet<String>, InstallationError> {
    let primary = get_text(NEOFORGE_MAVEN_METADATA_URL)?;
    let mut versions = parse_neoforge_versions_from_metadata(&primary);

    if let Ok(legacy) = get_text(NEOFORGE_LEGACY_FORGE_METADATA_URL) {
        versions.extend(parse_minecraft_versions_from_maven_metadata(&legacy, true));
    }

    Ok(versions)
}

pub(crate) fn fetch_neoforge_loader_catalog() -> Result<LoaderVersionCatalog, InstallationError> {
    let primary = get_text(NEOFORGE_MAVEN_METADATA_URL)?;
    let mut catalog = parse_neoforge_loader_catalog_from_metadata(&primary);

    if let Ok(legacy) = get_text(NEOFORGE_LEGACY_FORGE_METADATA_URL) {
        let legacy_neoforge = parse_neoforge_loader_catalog_from_metadata(&legacy);
        merge_loader_catalog(&mut catalog, legacy_neoforge);
        let legacy_forge_style = parse_forge_loader_catalog_from_metadata(&legacy);
        merge_loader_catalog(&mut catalog, legacy_forge_style);
    }

    Ok(catalog)
}

pub(crate) fn fetch_fabric_loader_catalog_with_fallback() -> LoaderVersionCatalog {
    match fetch_fabric_loader_catalog() {
        Ok(catalog) if !catalog.supported_game_versions.is_empty() => catalog,
        _ => LoaderVersionCatalog {
            supported_game_versions: fetch_fabric_versions().unwrap_or_default(),
            ..LoaderVersionCatalog::default()
        },
    }
}

pub(crate) fn fetch_quilt_loader_catalog_with_fallback() -> LoaderVersionCatalog {
    match fetch_quilt_loader_catalog() {
        Ok(catalog) if !catalog.supported_game_versions.is_empty() => catalog,
        _ => LoaderVersionCatalog {
            supported_game_versions: fetch_quilt_versions().unwrap_or_default(),
            ..LoaderVersionCatalog::default()
        },
    }
}

pub(crate) fn fetch_forge_loader_catalog_with_fallback() -> LoaderVersionCatalog {
    match fetch_forge_loader_catalog() {
        Ok(catalog) if !catalog.supported_game_versions.is_empty() => catalog,
        _ => LoaderVersionCatalog {
            supported_game_versions: fetch_forge_versions().unwrap_or_default(),
            ..LoaderVersionCatalog::default()
        },
    }
}

pub(crate) fn fetch_neoforge_loader_catalog_with_fallback() -> LoaderVersionCatalog {
    match fetch_neoforge_loader_catalog() {
        Ok(catalog) if !catalog.supported_game_versions.is_empty() => catalog,
        _ => LoaderVersionCatalog {
            supported_game_versions: fetch_neoforge_versions().unwrap_or_default(),
            ..LoaderVersionCatalog::default()
        },
    }
}

pub(crate) fn url_encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if is_unreserved {
            out.push(byte as char);
        } else {
            use std::fmt::Write as _;

            out.push('%');
            let _ = write!(out, "{byte:02X}");
        }
    }
    out
}

pub(crate) fn fetch_loader_versions_for_game_uncached(
    loader_kind: LoaderKind,
    game_version: &str,
) -> Result<LoaderVersionFetchResult, InstallationError> {
    match loader_kind {
        LoaderKind::Fabric => {
            let url = format!(
                "{}/{}",
                FABRIC_VERSION_MATRIX_URL.trim_end_matches('/'),
                url_encode_component(game_version)
            );
            let payload: serde_json::Value = get_json(&url)?;
            let selected_versions = parse_global_loader_versions(&payload);
            let mut versions_by_game_version = BTreeMap::new();
            versions_by_game_version.insert(game_version.to_owned(), selected_versions.clone());
            Ok(LoaderVersionFetchResult {
                selected_versions,
                versions_by_game_version,
            })
        }
        LoaderKind::Quilt => {
            let url = format!(
                "{}/{}",
                QUILT_VERSION_MATRIX_URL.trim_end_matches('/'),
                url_encode_component(game_version)
            );
            let payload: serde_json::Value = get_json(&url)?;
            let selected_versions = parse_global_loader_versions(&payload);
            let mut versions_by_game_version = BTreeMap::new();
            versions_by_game_version.insert(game_version.to_owned(), selected_versions.clone());
            Ok(LoaderVersionFetchResult {
                selected_versions,
                versions_by_game_version,
            })
        }
        LoaderKind::Forge => {
            let metadata = get_text(FORGE_MAVEN_METADATA_URL)?;
            let catalog = parse_forge_loader_catalog_from_metadata(&metadata);
            let selected_versions = catalog
                .versions_by_game_version
                .get(game_version)
                .cloned()
                .unwrap_or_default();
            Ok(LoaderVersionFetchResult {
                selected_versions,
                versions_by_game_version: catalog.versions_by_game_version,
            })
        }
        LoaderKind::NeoForge => {
            let catalog = fetch_neoforge_loader_catalog()?;
            let selected_versions = catalog
                .versions_by_game_version
                .get(game_version)
                .cloned()
                .unwrap_or_default();
            Ok(LoaderVersionFetchResult {
                selected_versions,
                versions_by_game_version: catalog.versions_by_game_version,
            })
        }
        LoaderKind::Vanilla | LoaderKind::Custom => Ok(LoaderVersionFetchResult::default()),
    }
}

pub(crate) fn loader_versions_cache_file_path(loader_kind: LoaderKind) -> Option<PathBuf> {
    let file_name = match loader_kind {
        LoaderKind::Fabric => "fabric_loader_versions.json",
        LoaderKind::Forge => "forge_loader_versions.json",
        LoaderKind::NeoForge => "neoforge_loader_versions.json",
        LoaderKind::Quilt => "quilt_loader_versions.json",
        LoaderKind::Vanilla | LoaderKind::Custom => return None,
    };
    Some(
        cache_root_dir()
            .join(CACHE_LOADER_VERSIONS_DIR_NAME)
            .join(file_name),
    )
}

pub(crate) fn read_cached_loader_versions(
    loader_kind: LoaderKind,
) -> Result<CachedLoaderVersions, InstallationError> {
    let Some(path) = loader_versions_cache_file_path(loader_kind) else {
        return Ok(CachedLoaderVersions::default());
    };
    let raw = fs_read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub(crate) fn write_cached_loader_versions(
    loader_kind: LoaderKind,
    cached: &CachedLoaderVersions,
) -> Result<(), InstallationError> {
    let Some(path) = loader_versions_cache_file_path(loader_kind) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs_create_dir_all(parent)?;
    }
    let file = fs_file_create(path)?;
    serde_json::to_writer_pretty(file, cached)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FabricGameVersion {
    version: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuiltGameVersion {
    version: Option<String>,
    id: Option<String>,
}
