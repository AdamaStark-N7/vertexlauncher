use super::*;

pub(crate) fn cache_file_path(filter: VersionCatalogFilter) -> PathBuf {
    if filter == VersionCatalogFilter::default() {
        return cache_root_dir().join(CACHE_VERSION_CATALOG_RELEASES_FILE);
    }

    if filter.include_snapshots_and_betas && filter.include_alpha && !filter.include_experimental {
        return cache_root_dir().join(CACHE_VERSION_CATALOG_ALL_FILE);
    }

    let mut suffix_parts = Vec::new();
    if filter.include_snapshots_and_betas {
        suffix_parts.push("snapshots");
    }
    if filter.include_alpha {
        suffix_parts.push("alpha");
    }
    if filter.include_experimental {
        suffix_parts.push("experimental");
    }
    let suffix = suffix_parts.join("_");
    cache_root_dir().join(format!("version_catalog_{suffix}.json"))
}

pub(crate) fn read_cached_version_catalog(
    filter: VersionCatalogFilter,
) -> Result<CachedVersionCatalog, InstallationError> {
    let raw = fs_read_to_string(cache_file_path(filter))?;
    Ok(serde_json::from_str(&raw)?)
}

pub(crate) fn write_cached_version_catalog(
    filter: VersionCatalogFilter,
    catalog: &VersionCatalog,
) -> Result<(), InstallationError> {
    let path = cache_file_path(filter);
    if let Some(parent) = path.parent() {
        fs_create_dir_all(parent)?;
    }

    let payload = CachedVersionCatalog {
        fetched_at_unix_secs: now_unix_secs(),
        filter,
        catalog: catalog.clone(),
    };
    let file = fs_file_create(path)?;
    serde_json::to_writer_pretty(file, &payload)?;
    Ok(())
}

pub(crate) fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(crate) fn is_cache_expired(fetched_at_unix_secs: u64) -> bool {
    let now = now_unix_secs();
    now.saturating_sub(fetched_at_unix_secs) > VERSION_CATALOG_CACHE_TTL.as_secs()
}

pub(crate) fn catalog_has_loader_version_data(catalog: &VersionCatalog) -> bool {
    let loader_versions = &catalog.loader_versions;
    [
        &loader_versions.fabric,
        &loader_versions.forge,
        &loader_versions.neoforge,
        &loader_versions.quilt,
    ]
    .into_iter()
    .any(|versions_by_game_version| {
        versions_by_game_version
            .values()
            .any(|versions| !versions.is_empty())
    })
}

/// Orders game versions newest first by Mojang's release time.
///
/// Version ids can't be compared as text: `26.3-snapshot-10` and `26.3-pre1` would rank above
/// `26.3` (more tokens), and weekly snapshots like `24w14a` above every `1.x` release. Release
/// time is what the manifest itself is ordered by. The sort is stable, so entries without a
/// time (from older cached catalogs, which were already stored in release-time order) keep
/// their relative position.
pub(crate) fn sort_game_versions_newest_first(
    versions: &mut [crate::installation_core::MinecraftVersionEntry],
) {
    if versions.iter().all(|entry| entry.release_time.is_some()) {
        versions.sort_by(|left, right| right.release_time.cmp(&left.release_time));
    }
}

pub(crate) fn normalize_version_catalog_ordering(catalog: &mut VersionCatalog) {
    sort_game_versions_newest_first(&mut catalog.game_versions);
    catalog.loader_versions.sort_desc();
}
