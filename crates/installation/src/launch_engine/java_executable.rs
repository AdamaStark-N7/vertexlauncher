use super::*;

#[derive(Clone, Debug)]
pub(crate) struct JavaExecutableResolution {
    pub executable: String,
    pub source: String,
    pub requested: Option<String>,
    pub fallback_reason: Option<String>,
}

impl JavaExecutableResolution {
    pub fn requested_display(&self) -> String {
        self.requested.as_deref().unwrap_or("<none>").to_owned()
    }

    pub fn fallback_display(&self) -> &str {
        self.fallback_reason.as_deref().unwrap_or("<none>")
    }
}

pub(crate) fn resolve_java_executable(configured: Option<&str>) -> JavaExecutableResolution {
    let requested = configured
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let mut java = requested.as_deref().unwrap_or("java").to_owned();
    let mut source = if requested.is_some() {
        "configured Java executable".to_owned()
    } else {
        "java from PATH".to_owned()
    };
    let mut fallback_reason = None;
    let requested_for_error = requested.clone();
    let path_like = java.contains('/') || java.contains('\\');
    if path_like {
        let original_java = java.clone();
        let java_path = Path::new(java.as_str());
        if !java_path.exists() {
            java = "java".to_owned();
            source = "java from PATH fallback".to_owned();
            fallback_reason = Some(format!("configured path does not exist: {original_java}"));
        } else if java_path.is_relative() {
            if let Ok(canonical) = fs_canonicalize(java_path) {
                java = display_user_path(canonical.as_path());
                source = "configured relative Java path (canonicalized)".to_owned();
            } else if let Ok(cwd) = std::env::current_dir() {
                java = display_user_path(cwd.join(java_path).as_path());
                source = "configured relative Java path (current-dir resolved)".to_owned();
            }
        } else {
            java = display_user_path(java_path);
            source = "configured absolute Java path".to_owned();
        }
    }

    JavaExecutableResolution {
        executable: java,
        source,
        requested: requested_for_error,
        fallback_reason,
    }
}

pub(crate) fn log_java_executable_resolution(
    target: &'static str,
    context: &str,
    resolution: &JavaExecutableResolution,
) {
    if let Some(reason) = resolution.fallback_reason.as_deref() {
        tracing::warn!(
            target,
            context = %context,
            executable = %resolution.executable,
            source = %resolution.source,
            requested = %resolution.requested_display(),
            fallback_reason = %reason,
            "Resolved Java executable with fallback."
        );
    } else {
        tracing::info!(
            target,
            context = %context,
            executable = %resolution.executable,
            source = %resolution.source,
            requested = %resolution.requested_display(),
            "Resolved Java executable."
        );
    }
}

pub(crate) fn format_java_executable_resolution(resolution: &JavaExecutableResolution) -> String {
    format!(
        "executable={} | source={} | requested={} | fallback_reason={}",
        resolution.executable,
        resolution.source,
        resolution.requested_display(),
        resolution.fallback_display()
    )
}

pub(crate) fn run_command_output(
    cmd: &mut Command,
    resolution: &JavaExecutableResolution,
) -> Result<Output, InstallationError> {
    cmd.output().map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            InstallationError::JavaExecutableNotFound {
                executable: resolution.executable.clone(),
                java_source: resolution.source.clone(),
                requested: resolution.requested_display(),
                message: err.to_string(),
            }
        } else {
            InstallationError::Io(err)
        }
    })
}

pub(crate) fn spawn_command_child(
    cmd: &mut Command,
    resolution: &JavaExecutableResolution,
) -> Result<Child, InstallationError> {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            InstallationError::JavaExecutableNotFound {
                executable: resolution.executable.clone(),
                java_source: resolution.source.clone(),
                requested: resolution.requested_display(),
                message: err.to_string(),
            }
        } else {
            InstallationError::Io(err)
        }
    })
}
