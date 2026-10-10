use super::*;

// ── Apply helpers ─────────────────────────────────────────────────────────────

pub(super) fn download_downloadable_entry(
    entry: &VtmpackDownloadableEntry,
    instance_root: &Path,
) -> Result<(), String> {
    if entry
        .selected_source
        .as_deref()
        .is_some_and(|source| source.eq_ignore_ascii_case("CurseForge"))
        || entry.curseforge_project_id.is_some() && entry.modrinth_project_id.is_none()
    {
        return download_curseforge_entry(entry, instance_root);
    }
    download_modrinth_entry(entry, instance_root)
}

/// Download a single Modrinth-tracked file and write it to the instance.
pub(super) fn download_modrinth_entry(
    entry: &VtmpackDownloadableEntry,
    instance_root: &Path,
) -> Result<(), String> {
    let client = modrinth::Client::default();
    let version_id = entry
        .selected_version_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "downloadable entry {} has no version_id",
                entry.file_path.display()
            )
        })?;

    let version = client
        .get_version(version_id)
        .map_err(|err| format!("failed to fetch Modrinth version {version_id}: {err}"))?;

    // Prefer the file whose sha512 matches the manifest; fall back to primary.
    let file = version
        .files
        .iter()
        .find(|f| {
            entry
                .selected_file_sha512
                .as_deref()
                .is_some_and(|expected| {
                    f.hashes
                        .get("sha512")
                        .is_some_and(|h| h.eq_ignore_ascii_case(expected))
                })
        })
        .or_else(|| version.files.iter().find(|f| f.primary))
        .or_else(|| version.files.first())
        .ok_or_else(|| format!("no downloadable files for Modrinth version {version_id}"))?;

    let mut response = ureq::get(&file.url)
        .call()
        .map_err(|err| format!("HTTP request to {} failed: {err}", file.url))?;
    let mut reader = response.body_mut().as_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read response body from {}: {err}", file.url))?;

    let destination = instance_root.join(entry.file_path.as_path());
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&destination, &bytes)
        .map_err(|err| format!("failed to write {}: {err}", destination.display()))?;

    // Verify the hash if we have it.
    if let Some(expected_sha512) = &entry.selected_file_sha512 {
        let actual = modrinth::hash_file_sha512_hex(destination.as_path()).map_err(|err| {
            format!(
                "failed to hash downloaded file {}: {err}",
                destination.display()
            )
        })?;
        if !actual.eq_ignore_ascii_case(expected_sha512) {
            let _ = fs::remove_file(destination.as_path());
            return Err(format!(
                "SHA-512 mismatch for {} (expected {expected_sha512}, got {actual})",
                file.filename
            ));
        }
    }

    Ok(())
}

pub(super) fn download_curseforge_entry(
    entry: &VtmpackDownloadableEntry,
    instance_root: &Path,
) -> Result<(), String> {
    let project_id = entry.curseforge_project_id.ok_or_else(|| {
        format!(
            "downloadable entry {} has no CurseForge project id",
            entry.file_path.display()
        )
    })?;
    let file_id = entry
        .selected_version_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "downloadable entry {} has no CurseForge file id",
                entry.file_path.display()
            )
        })?
        .parse::<u64>()
        .map_err(|err| format!("invalid CurseForge file id for {}: {err}", entry.name))?;
    let client = curseforge::Client::from_env().ok_or_else(|| {
        "CurseForge API key missing. Add one in Settings or set VERTEX_CURSEFORGE_API_KEY/CURSEFORGE_API_KEY to apply this patch."
            .to_owned()
    })?;
    let download_url = client
        .get_mod_file_download_url(project_id, file_id)
        .map_err(|err| {
            format!("failed to resolve CurseForge download URL for {project_id}/{file_id}: {err}")
        })?
        .ok_or_else(|| {
            format!("CurseForge file {file_id} for project {project_id} has no download URL")
        })?;
    let mut response = ureq::get(download_url.as_str())
        .call()
        .map_err(|err| format!("HTTP request to {download_url} failed: {err}"))?;
    let mut reader = response.body_mut().as_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read response body from {download_url}: {err}"))?;
    let destination = instance_root.join(entry.file_path.as_path());
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&destination, &bytes)
        .map_err(|err| format!("failed to write {}: {err}", destination.display()))
}
