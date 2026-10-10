use super::*;

pub(crate) fn platform_for_adoptium()
-> Result<(&'static str, &'static str, &'static str), InstallationError> {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        return Err(InstallationError::UnsupportedPlatform(
            std::env::consts::OS.to_owned(),
        ));
    };
    let arch = current_runtime_architecture().ok_or_else(|| {
        InstallationError::UnsupportedPlatform(
            detected_runtime_architecture().unwrap_or_else(|| std::env::consts::ARCH.to_owned()),
        )
    })?;
    Ok((os, arch.adoptium_value(), arch.cache_key()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeArchitecture {
    X86,
    X64,
    Arm,
    Aarch64,
}

impl RuntimeArchitecture {
    fn adoptium_value(self) -> &'static str {
        match self {
            RuntimeArchitecture::X86 => "x32",
            RuntimeArchitecture::X64 => "x64",
            RuntimeArchitecture::Arm => "arm",
            RuntimeArchitecture::Aarch64 => "aarch64",
        }
    }

    fn cache_key(self) -> &'static str {
        match self {
            RuntimeArchitecture::X86 => "x86",
            RuntimeArchitecture::X64 => "x64",
            RuntimeArchitecture::Arm => "arm",
            RuntimeArchitecture::Aarch64 => "aarch64",
        }
    }
}

pub(crate) fn current_runtime_architecture() -> Option<RuntimeArchitecture> {
    normalize_runtime_architecture(
        detected_runtime_architecture()
            .unwrap_or_else(|| std::env::consts::ARCH.to_owned())
            .as_str(),
    )
}

pub(crate) fn normalize_runtime_architecture(raw: &str) -> Option<RuntimeArchitecture> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "x86" | "i386" | "i486" | "i586" | "i686" => Some(RuntimeArchitecture::X86),
        "x86_64" | "amd64" => Some(RuntimeArchitecture::X64),
        "arm" | "armv7" | "armv7l" => Some(RuntimeArchitecture::Arm),
        "arm64" | "aarch64" => Some(RuntimeArchitecture::Aarch64),
        _ => None,
    }
}

pub(crate) fn detected_runtime_architecture() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var("PROCESSOR_ARCHITEW6432")
            .ok()
            .or_else(|| std::env::var("PROCESSOR_ARCHITECTURE").ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
    }

    #[cfg(target_os = "macos")]
    {
        let uname = command_stdout_trimmed("uname", ["-m"])?;
        if normalize_runtime_architecture(uname.as_str()) == Some(RuntimeArchitecture::X64)
            && command_stdout_trimmed("sysctl", ["-in", "hw.optional.arm64"]).as_deref()
                == Some("1")
        {
            return Some("arm64".to_owned());
        }
        return Some(uname);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return command_stdout_trimmed("uname", ["-m"]);
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn command_stdout_trimmed<const N: usize>(
    program: &str,
    args: [&str; N],
) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod runtime_architecture_tests {
    use super::{RuntimeArchitecture, normalize_runtime_architecture};

    #[test]
    fn normalizes_common_x64_aliases() {
        assert_eq!(
            normalize_runtime_architecture("x86_64"),
            Some(RuntimeArchitecture::X64)
        );
        assert_eq!(
            normalize_runtime_architecture("AMD64"),
            Some(RuntimeArchitecture::X64)
        );
    }

    #[test]
    fn normalizes_common_arm64_aliases() {
        assert_eq!(
            normalize_runtime_architecture("arm64"),
            Some(RuntimeArchitecture::Aarch64)
        );
        assert_eq!(
            normalize_runtime_architecture("aarch64"),
            Some(RuntimeArchitecture::Aarch64)
        );
    }

    #[test]
    fn normalizes_x86_aliases() {
        assert_eq!(
            normalize_runtime_architecture("i686"),
            Some(RuntimeArchitecture::X86)
        );
    }

    #[test]
    fn rejects_unknown_architecture_strings() {
        assert_eq!(normalize_runtime_architecture("sparc64"), None);
    }
}
