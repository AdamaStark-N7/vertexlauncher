use super::*;

#[path = "launch_engine_modloaders/installer_run.rs"]
mod installer_run;
#[path = "launch_engine_modloaders/profile_check.rs"]
mod profile_check;

pub(crate) use installer_run::*;
pub(crate) use profile_check::*;

#[path = "launch_engine_modloaders/http_client.rs"]
mod http_client;
#[path = "launch_engine_modloaders/java_archive.rs"]
mod java_archive;
#[path = "launch_engine_modloaders/loader_catalog_fetch.rs"]
mod loader_catalog_fetch;
#[path = "launch_engine_modloaders/loader_catalog_parse.rs"]
mod loader_catalog_parse;
#[path = "launch_engine_modloaders/runtime_architecture.rs"]
mod runtime_architecture;
#[path = "launch_engine_modloaders/version_catalog_cache.rs"]
mod version_catalog_cache;
#[path = "launch_engine_modloaders/version_compare.rs"]
mod version_compare;

pub(crate) use http_client::*;
pub(crate) use java_archive::*;
pub(crate) use loader_catalog_fetch::*;
pub(crate) use loader_catalog_parse::*;
pub(crate) use runtime_architecture::*;
pub(crate) use version_catalog_cache::*;
pub(crate) use version_compare::*;

#[cfg(test)]
#[path = "launch_engine_modloaders_tests.rs"]
mod launch_engine_modloaders_tests;

pub(crate) fn install_selected_modloader(
    instance_root: &Path,
    game_version: &str,
    modloader: &str,
    modloader_version: Option<&str>,
    java_executable: Option<&str>,
    policy: &DownloadPolicy,
    downloaded_files: &mut u32,
    progress: Option<&InstallProgressSink>,
) -> Result<Option<String>, InstallationError> {
    let loader_kind = normalized_loader_label(modloader);
    tracing::info!(
        target: "vertexlauncher/installation/modloader",
        requested_modloader = %modloader,
        requested_game_version = %game_version,
        requested_modloader_version = %modloader_version.unwrap_or(""),
        "Selecting modloader installation strategy."
    );
    match loader_kind {
        LoaderKind::Vanilla | LoaderKind::Custom => Ok(None),
        LoaderKind::Fabric | LoaderKind::Quilt => {
            let loader_label = if loader_kind == LoaderKind::Fabric {
                "Fabric"
            } else {
                "Quilt"
            };
            let resolved =
                resolve_loader_version(loader_kind, loader_label, game_version, modloader_version)?;
            if has_fabric_or_quilt_profile(instance_root, game_version, loader_kind, &resolved)? {
                tracing::info!(
                    target: "vertexlauncher/installation/modloader",
                    loader = %loader_label,
                    game_version = %game_version,
                    resolved = %resolved,
                    "Modloader profile already present; skipping profile install."
                );
                return Ok(Some(resolved));
            }
            emit_installing_modloader_progress(
                loader_label,
                &resolved,
                *downloaded_files,
                progress,
            );
            *downloaded_files += install_fabric_or_quilt_profile(
                instance_root,
                game_version,
                loader_kind,
                &resolved,
                policy,
                *downloaded_files,
                progress,
            )?;
            Ok(Some(resolved))
        }
        LoaderKind::Forge => {
            let resolved =
                resolve_loader_version(loader_kind, "Forge", game_version, modloader_version)?;
            if verify_modloader_profile(instance_root, loader_kind, game_version, &resolved)? {
                tracing::info!(
                    target: "vertexlauncher/installation/modloader",
                    loader = "Forge",
                    game_version = %game_version,
                    resolved = %resolved,
                    "Modloader profile already present; skipping installer execution."
                );
                return Ok(Some(resolved));
            }
            emit_installing_modloader_progress("Forge", &resolved, *downloaded_files, progress);
            *downloaded_files += install_forge_installer(
                instance_root,
                game_version,
                &resolved,
                java_executable,
                policy,
                *downloaded_files,
                progress,
            )?;
            Ok(Some(resolved))
        }
        LoaderKind::NeoForge => {
            let resolved =
                resolve_loader_version(loader_kind, "NeoForge", game_version, modloader_version)?;
            if verify_modloader_profile(instance_root, loader_kind, game_version, &resolved)? {
                tracing::info!(
                    target: "vertexlauncher/installation/modloader",
                    loader = "NeoForge",
                    game_version = %game_version,
                    resolved = %resolved,
                    "Modloader profile already present; skipping installer execution."
                );
                return Ok(Some(resolved));
            }
            emit_installing_modloader_progress("NeoForge", &resolved, *downloaded_files, progress);
            *downloaded_files += install_neoforge_installer(
                instance_root,
                game_version,
                &resolved,
                java_executable,
                policy,
                *downloaded_files,
                progress,
            )?;
            Ok(Some(resolved))
        }
    }
}

pub(crate) fn resolve_loader_version(
    _loader_kind: LoaderKind,
    loader_label: &str,
    game_version: &str,
    requested: Option<&str>,
) -> Result<String, InstallationError> {
    let versions = fetch_loader_versions_for_game(loader_label, game_version, false)?;
    if let Some(value) = requested.map(str::trim).filter(|value| !value.is_empty())
        && !is_latest_loader_version_alias(value)
    {
        let supported = versions.iter().any(|candidate| candidate == value);
        if !supported {
            tracing::warn!(
                target: "vertexlauncher/installation/modloader",
                loader = %loader_label,
                game_version = %game_version,
                requested = %value,
                supported_versions = ?versions,
                "Requested modloader version is not compatible with selected Minecraft version."
            );
            return Err(InstallationError::MissingModloaderVersion {
                loader: loader_label.to_owned(),
                game_version: game_version.to_owned(),
            });
        }
        tracing::info!(
            target: "vertexlauncher/installation/modloader",
            loader = %loader_label,
            game_version = %game_version,
            requested = %value,
            "Using explicitly requested compatible modloader version."
        );
        return Ok(value.to_owned());
    }
    let resolved =
        versions
            .first()
            .cloned()
            .ok_or_else(|| InstallationError::MissingModloaderVersion {
                loader: loader_label.to_owned(),
                game_version: game_version.to_owned(),
            })?;
    tracing::info!(
        target: "vertexlauncher/installation/modloader",
        loader = %loader_label,
        game_version = %game_version,
        resolved = %resolved,
        "Resolved latest compatible modloader version."
    );
    Ok(resolved)
}

pub(crate) fn is_latest_loader_version_alias(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "latest" | "latest available" | "use latest version" | "auto" | "default"
    )
}

pub(crate) fn emit_installing_modloader_progress(
    loader_label: &str,
    loader_version: &str,
    downloaded_files: u32,
    progress: Option<&InstallProgressSink>,
) {
    report_install_progress(
        progress,
        InstallProgress {
            stage: InstallStage::InstallingModloader,
            message: format!("Installing {loader_label} {loader_version} artifacts..."),
            downloaded_files,
            total_files: downloaded_files.max(1),
            downloaded_bytes: 0,
            total_bytes: None,
            bytes_per_second: 0.0,
            eta_seconds: None,
        },
    );
}

pub(crate) fn cache_root_dir() -> PathBuf {
    app_paths::cache_root()
}

pub(crate) fn canonicalize_existing_path(path: PathBuf) -> PathBuf {
    fs_canonicalize(path.as_path()).unwrap_or(path)
}

pub(crate) fn map_version_type(raw: &str) -> MinecraftVersionType {
    match raw {
        "release" => MinecraftVersionType::Release,
        "snapshot" => MinecraftVersionType::Snapshot,
        "old_beta" => MinecraftVersionType::OldBeta,
        "old_alpha" => MinecraftVersionType::OldAlpha,
        _ => MinecraftVersionType::Unknown,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoaderKind {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    Custom,
}

pub(crate) fn normalized_loader_label(loader_label: &str) -> LoaderKind {
    match loader_label.trim().to_ascii_lowercase().as_str() {
        "vanilla" => LoaderKind::Vanilla,
        "fabric" => LoaderKind::Fabric,
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        "quilt" => LoaderKind::Quilt,
        _ => LoaderKind::Custom,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MojangVersionManifest {
    pub(crate) versions: Vec<MojangVersionEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MojangVersionEntry {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) version_type: String,
    pub(crate) release_time: String,
    pub(crate) url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MojangVersionMeta {
    pub(crate) downloads: Option<MojangDownloads>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MojangDownloads {
    pub(crate) client: Option<MojangDownloadArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MojangDownloadArtifact {
    pub(crate) url: String,
    pub(crate) size: Option<u64>,
}
