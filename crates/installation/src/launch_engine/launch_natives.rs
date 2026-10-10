use super::*;

pub(crate) fn prepare_natives_dir(
    instance_root: &Path,
    profile_id: &str,
    chain: &[serde_json::Value],
) -> Result<PathBuf, InstallationError> {
    let natives_root = instance_root.join("natives").join(profile_id);
    if natives_root.exists() {
        fs_remove_dir_all(&natives_root)?;
    }
    fs_create_dir_all(&natives_root)?;

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
            let Some(natives) = lib.get("natives").and_then(serde_json::Value::as_object) else {
                continue;
            };
            let os_key = current_os_natives_key();
            let Some(classifier_template) = natives.get(os_key).and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let classifier = classifier_template.replace("${arch}", current_arch_natives_value());
            let Some(path) = lib
                .get("downloads")
                .and_then(|v| v.get("classifiers"))
                .and_then(|v| v.get(classifier.as_str()))
                .and_then(|v| v.get("path"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let archive = instance_root.join("libraries").join(path);
            if !archive.exists() {
                continue;
            }
            let excludes = lib
                .get("extract")
                .and_then(|v| v.get("exclude"))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            extract_natives_archive(archive.as_path(), natives_root.as_path(), &excludes)?;
        }
    }
    Ok(natives_root)
}

pub(crate) fn extract_natives_archive(
    archive_path: &Path,
    destination: &Path,
    excludes: &[String],
) -> Result<(), InstallationError> {
    let file = fs_file_open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if name.starts_with("META-INF/") || excludes.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        let out = destination.join(name.as_str());
        if let Some(parent) = out.parent() {
            fs_create_dir_all(parent)?;
        }
        let mut writer = fs_file_create(out)?;
        std::io::copy(&mut entry, &mut writer)?;
    }
    Ok(())
}

pub(crate) fn current_os_natives_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub(crate) fn current_arch_natives_value() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    }
}

pub(crate) fn arch_matches_current_target(expected: &str) -> bool {
    let normalized = expected.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "x86" | "i386" | "i686" | "32" => cfg!(target_arch = "x86"),
        "x86_64" | "amd64" | "64" => cfg!(target_arch = "x86_64"),
        "arm64" | "aarch64" => cfg!(target_arch = "aarch64"),
        "arm" => cfg!(target_arch = "arm"),
        other => std::env::consts::ARCH.eq_ignore_ascii_case(other),
    }
}
