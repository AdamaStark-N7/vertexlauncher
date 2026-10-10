use super::*;

pub(crate) fn collect_library_download_tasks(
    instance_root: &Path,
    version_meta: &serde_json::Value,
    tasks: &mut Vec<FileDownloadTask>,
) {
    let Some(libraries) = version_meta
        .get("libraries")
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    let mut seen_destinations: HashSet<PathBuf> =
        tasks.iter().map(|t| t.destination.clone()).collect();
    for library in libraries {
        if !library_rules_allow(library) {
            continue;
        }
        if let Some(downloads) = library.get("downloads") {
            if let Some(artifact) = downloads.get("artifact") {
                push_download_task_from_download_entry(
                    instance_root.join("libraries").as_path(),
                    artifact,
                    tasks,
                    &mut seen_destinations,
                );
            }
            if let Some(classifiers) = downloads
                .get("classifiers")
                .and_then(serde_json::Value::as_object)
            {
                for entry in classifiers.values() {
                    push_download_task_from_download_entry(
                        instance_root.join("libraries").as_path(),
                        entry,
                        tasks,
                        &mut seen_destinations,
                    );
                }
            }
        } else if let Some((url, relative_path)) = resolve_library_maven_download(library) {
            let destination = instance_root.join("libraries").join(relative_path.as_str());
            if !destination.exists() && seen_destinations.insert(destination.clone()) {
                tasks.push(FileDownloadTask {
                    url,
                    destination,
                    expected_size: library
                        .get("size")
                        .and_then(serde_json::Value::as_u64)
                        .filter(|size| *size > 0),
                });
            }
        }
    }
}

pub(crate) fn resolve_library_maven_download(
    library: &serde_json::Value,
) -> Option<(String, String)> {
    let name = library.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    let mut parts = name.split(':');
    let group = parts.next()?.trim();
    let artifact = parts.next()?.trim();
    let version_and_ext = parts.next()?.trim();
    let classifier = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if group.is_empty() || artifact.is_empty() || version_and_ext.is_empty() {
        return None;
    }

    let (version, extension) = if let Some((version, ext)) = version_and_ext.split_once('@') {
        (version.trim(), ext.trim())
    } else {
        (version_and_ext, "jar")
    };
    if version.is_empty() || extension.is_empty() {
        return None;
    }

    let base_url = library
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("https://libraries.minecraft.net/");
    let group_path = group.replace('.', "/");
    let file_name = if let Some(classifier) = classifier {
        format!("{artifact}-{version}-{classifier}.{extension}")
    } else {
        format!("{artifact}-{version}.{extension}")
    };
    let relative_path = format!("{group_path}/{artifact}/{version}/{file_name}");
    let url = format!(
        "{}{relative_path}",
        base_url.trim_end_matches('/').to_owned() + "/"
    );
    Some((url, relative_path))
}

pub(crate) fn collect_asset_index_download_task(
    instance_root: &Path,
    version_meta: &serde_json::Value,
    tasks: &mut Vec<FileDownloadTask>,
) -> Option<PathBuf> {
    let asset_index = version_meta.get("assetIndex")?;
    let url = asset_index.get("url")?.as_str()?.trim();
    if url.is_empty() {
        return None;
    }
    let id = asset_index
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let destination = instance_root
        .join("assets")
        .join("indexes")
        .join(format!("{id}.json"));
    if !destination.exists() {
        tasks.push(FileDownloadTask {
            url: url.to_owned(),
            destination: destination.clone(),
            expected_size: asset_index
                .get("size")
                .and_then(serde_json::Value::as_u64)
                .filter(|size| *size > 0),
        });
    }
    Some(destination)
}

pub(crate) fn collect_asset_object_download_tasks(
    instance_root: &Path,
    asset_index_path: &Path,
    tasks: &mut Vec<FileDownloadTask>,
) -> Result<(), InstallationError> {
    if !asset_index_path.exists() {
        return Ok(());
    }
    let raw = fs_read_to_string(asset_index_path).map_err(|err| {
        tracing::error!(
            target: "vertexlauncher/installation/dependencies",
            instance_root = %instance_root.display(),
            asset_index_path = %asset_index_path.display(),
            error = %err,
            "Failed to read asset index JSON."
        );
        InstallationError::Io(err)
    })?;
    let index: serde_json::Value = serde_json::from_str(&raw)?;
    let Some(objects) = index.get("objects").and_then(serde_json::Value::as_object) else {
        return Ok(());
    };
    let mut seen_destinations = HashSet::new();
    let mut duplicate_count = 0usize;
    for entry in objects.values() {
        let Some(hash) = entry.get("hash").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let hash = hash.trim();
        if hash.len() < 2 {
            continue;
        }
        let prefix = &hash[..2];
        let destination = instance_root
            .join("assets")
            .join("objects")
            .join(prefix)
            .join(hash);
        if destination.exists() {
            continue;
        }
        if !seen_destinations.insert(destination.clone()) {
            duplicate_count += 1;
            continue;
        }
        tasks.push(FileDownloadTask {
            url: format!("https://resources.download.minecraft.net/{prefix}/{hash}"),
            destination,
            expected_size: entry
                .get("size")
                .and_then(serde_json::Value::as_u64)
                .filter(|size| *size > 0),
        });
    }
    if duplicate_count > 0 {
        tracing::info!(
            target: "vertexlauncher/installation/dependencies",
            asset_index_path = %asset_index_path.display(),
            duplicate_count,
            deduped_task_count = tasks.len(),
            "Deduplicated repeated asset object downloads that pointed to the same destination."
        );
    }
    Ok(())
}

pub(crate) fn push_download_task_from_download_entry(
    root: &Path,
    download_entry: &serde_json::Value,
    tasks: &mut Vec<FileDownloadTask>,
    seen_destinations: &mut HashSet<PathBuf>,
) {
    let Some(url) = download_entry
        .get("url")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let Some(path) = download_entry
        .get("path")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let url = url.trim();
    let path = path.trim();
    if url.is_empty() || path.is_empty() {
        return;
    }
    let destination = root.join(path);
    if destination.exists() || !seen_destinations.insert(destination.clone()) {
        return;
    }
    tasks.push(FileDownloadTask {
        url: url.to_owned(),
        destination,
        expected_size: download_entry
            .get("size")
            .and_then(serde_json::Value::as_u64)
            .filter(|size| *size > 0),
    });
}
