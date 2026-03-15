#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlatformSpecificSection {
    pub id: &'static str,
    pub heading: &'static str,
    pub launcher_description: &'static str,
    pub instance_description: &'static str,
}

pub(crate) fn current_platform_specific_section() -> Option<PlatformSpecificSection> {
    #[cfg(target_os = "linux")]
    {
        return Some(PlatformSpecificSection {
            id: "linux",
            heading: "Linux",
            launcher_description: "Linux-specific launch compatibility settings that apply across the launcher.",
            instance_description: "Linux-specific launch compatibility settings for this instance.",
        });
    }

    #[cfg(target_os = "windows")]
    {
        return Some(PlatformSpecificSection {
            id: "windows",
            heading: "Windows",
            launcher_description: "Reserved for Windows-specific launcher settings.",
            instance_description: "Reserved for Windows-specific instance settings.",
        });
    }

    #[cfg(target_os = "macos")]
    {
        return Some(PlatformSpecificSection {
            id: "macos",
            heading: "macOS",
            launcher_description: "Reserved for macOS-specific launcher settings.",
            instance_description: "Reserved for macOS-specific instance settings.",
        });
    }

    #[allow(unreachable_code)]
    None
}
