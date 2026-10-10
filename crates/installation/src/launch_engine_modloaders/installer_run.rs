use super::*;

pub(crate) fn install_fabric_or_quilt_profile(
    instance_root: &Path,
    game_version: &str,
    loader_kind: LoaderKind,
    loader_version: &str,
    policy: &DownloadPolicy,
    downloaded_files_offset: u32,
    progress: Option<&InstallProgressSink>,
) -> Result<u32, InstallationError> {
    let profile_url = match loader_kind {
        LoaderKind::Fabric => format!(
            "{}/{}/{}/profile/json",
            FABRIC_VERSION_MATRIX_URL.trim_end_matches('/'),
            url_encode_component(game_version),
            url_encode_component(loader_version),
        ),
        LoaderKind::Quilt => format!(
            "{}/{}/{}/profile/json",
            QUILT_VERSION_MATRIX_URL.trim_end_matches('/'),
            url_encode_component(game_version),
            url_encode_component(loader_version),
        ),
        _ => return Ok(0),
    };

    let id_prefix = match loader_kind {
        LoaderKind::Fabric => "fabric-loader",
        LoaderKind::Quilt => "quilt-loader",
        _ => "loader",
    };
    let version_id = format!("{id_prefix}-{loader_version}-{game_version}");
    let profile_path = instance_root
        .join("versions")
        .join(version_id.as_str())
        .join(format!("{version_id}.json"));
    let task = FileDownloadTask {
        url: profile_url,
        destination: profile_path,
        expected_size: None,
    };
    download_files_concurrent(
        InstallStage::InstallingModloader,
        vec![task],
        policy,
        downloaded_files_offset,
        progress,
    )
}

pub(crate) fn install_forge_installer(
    instance_root: &Path,
    game_version: &str,
    loader_version: &str,
    java_executable: Option<&str>,
    policy: &DownloadPolicy,
    downloaded_files_offset: u32,
    progress: Option<&InstallProgressSink>,
) -> Result<u32, InstallationError> {
    let artifact_version = format!("{game_version}-{loader_version}");
    let installer_file = format!("forge-{artifact_version}-installer.jar");
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{artifact_version}/{installer_file}"
    );
    let destination = instance_root
        .join("loaders")
        .join("forge")
        .join(game_version)
        .join(loader_version)
        .join(installer_file);
    let mut tasks = Vec::new();
    if !destination.exists() {
        tasks.push(FileDownloadTask {
            url,
            destination,
            expected_size: None,
        });
    }
    let downloaded = download_files_concurrent(
        InstallStage::InstallingModloader,
        tasks,
        policy,
        downloaded_files_offset,
        progress,
    )?;
    run_modloader_installer_and_verify(
        instance_root,
        LoaderKind::Forge,
        game_version,
        loader_version,
        java_executable,
    )?;
    Ok(downloaded)
}

pub(crate) fn install_neoforge_installer(
    instance_root: &Path,
    game_version: &str,
    loader_version: &str,
    java_executable: Option<&str>,
    policy: &DownloadPolicy,
    downloaded_files_offset: u32,
    progress: Option<&InstallProgressSink>,
) -> Result<u32, InstallationError> {
    let installer_file = format!("neoforge-{loader_version}-installer.jar");
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/{installer_file}"
    );
    let destination = instance_root
        .join("loaders")
        .join("neoforge")
        .join(game_version)
        .join(loader_version)
        .join(installer_file);
    let mut tasks = Vec::new();
    if !destination.exists() {
        tasks.push(FileDownloadTask {
            url,
            destination,
            expected_size: None,
        });
    }
    let downloaded = download_files_concurrent(
        InstallStage::InstallingModloader,
        tasks,
        policy,
        downloaded_files_offset,
        progress,
    )?;
    run_modloader_installer_and_verify(
        instance_root,
        LoaderKind::NeoForge,
        game_version,
        loader_version,
        java_executable,
    )?;
    Ok(downloaded)
}

pub(crate) fn run_modloader_installer_and_verify(
    instance_root: &Path,
    loader_kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
    java_executable: Option<&str>,
) -> Result<(), InstallationError> {
    let loader_label = match loader_kind {
        LoaderKind::Forge => "Forge",
        LoaderKind::NeoForge => "NeoForge",
        _ => return Ok(()),
    };
    let configured_java = java_executable
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| InstallationError::MissingJavaRuntime {
            loader: loader_label.to_owned(),
        })?;
    let java_resolution = resolve_java_executable(Some(configured_java.as_str()));
    let java = java_resolution.executable.as_str();
    log_java_executable_resolution(
        "vertexlauncher/installation/modloader",
        "modloader installer",
        &java_resolution,
    );
    let installer_path =
        find_installer_jar(instance_root, loader_kind, game_version, loader_version)?.ok_or_else(
            || InstallationError::ModloaderInstallOutputMissing {
                loader: loader_label.to_owned(),
                game_version: game_version.to_owned(),
                loader_version: loader_version.to_owned(),
                versions_dir: instance_root.join("versions"),
            },
        )?;
    let installer_path = match fs_canonicalize(installer_path.as_path()) {
        Ok(path) => path,
        Err(_) => installer_path,
    };
    ensure_launcher_profiles(instance_root)?;
    let installer_target =
        fs_canonicalize(instance_root).unwrap_or_else(|_| instance_root.to_path_buf());
    let installer_path = normalize_child_process_path(installer_path.as_path());
    let installer_target = normalize_child_process_path(installer_target.as_path());
    let installer_path_arg = display_user_path(installer_path.as_path());
    let installer_target_arg = display_user_path(installer_target.as_path());

    // Try both flag variants used by Forge/NeoForge installers.
    let mut last_failure = None;
    for flag in ["--installClient", "--install-client"] {
        let mut cmd = Command::new(java);
        cmd.arg("-jar")
            .arg(installer_path.as_os_str())
            .arg(flag)
            .arg(installer_target.as_os_str())
            .current_dir(installer_target.as_path());
        let command_line = format!(
            "{} -jar {} {} {}",
            java, installer_path_arg, flag, installer_target_arg
        );
        let output = run_command_output(&mut cmd, &java_resolution)?;
        if output.status.success() {
            if verify_modloader_profile(instance_root, loader_kind, game_version, loader_version)? {
                return Ok(());
            }
            return Err(InstallationError::ModloaderInstallOutputMissing {
                loader: loader_label.to_owned(),
                game_version: game_version.to_owned(),
                loader_version: loader_version.to_owned(),
                versions_dir: instance_root.join("versions"),
            });
        }
        last_failure = Some((command_line, output.status.code(), output.stderr));
    }

    let (command, status_code, stderr_bytes) = last_failure.unwrap_or_default();
    Err(InstallationError::ModloaderInstallerFailed {
        loader: loader_label.to_owned(),
        game_version: game_version.to_owned(),
        loader_version: loader_version.to_owned(),
        command,
        status: status_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_owned()),
        stderr: String::from_utf8_lossy(&stderr_bytes).trim().to_owned(),
    })
}

pub(crate) fn ensure_launcher_profiles(instance_root: &Path) -> Result<(), InstallationError> {
    let profile_path = instance_root.join("launcher_profiles.json");
    if profile_path.exists() {
        return Ok(());
    }
    let profile = serde_json::json!({
        "profiles": {},
        "selectedProfile": null,
        "clientToken": "vertexlauncher",
        "authenticationDatabase": {},
        "launcherVersion": {
            "name": "Vertex Launcher",
            "format": 21
        },
        "settings": {}
    });
    fs_write(profile_path, serde_json::to_string_pretty(&profile)?)?;
    Ok(())
}

pub(crate) fn find_installer_jar(
    instance_root: &Path,
    loader_kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
) -> Result<Option<PathBuf>, InstallationError> {
    let file_name = match loader_kind {
        LoaderKind::Forge => format!("forge-{game_version}-{loader_version}-installer.jar"),
        LoaderKind::NeoForge => format!("neoforge-{loader_version}-installer.jar"),
        _ => return Ok(None),
    };
    let loader_dir = match loader_kind {
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        _ => "",
    };
    let path = instance_root
        .join("loaders")
        .join(loader_dir)
        .join(game_version)
        .join(loader_version)
        .join(file_name);
    Ok(path.exists().then_some(path))
}
