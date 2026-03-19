use config::Config;
use egui::Ui;
use textui::TextUi;

#[cfg(target_os = "linux")]
use crate::ui::components::settings_widgets;

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

pub(crate) fn render_launcher_platform_settings(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    config: &mut Config,
) {
    #[cfg(target_os = "linux")]
    {
        render_linux_launcher_settings(ui, text_ui, config);
        return;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (ui, text_ui, config);
    }
}

pub(crate) fn detect_total_memory_mib() -> Option<u128> {
    #[cfg(target_os = "linux")]
    {
        tracing::debug!(
            target: "vertexlauncher/io",
            op = "read_to_string",
            path = "/proc/meminfo",
            context = "detect total memory"
        );
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = meminfo.lines().find(|line| line.starts_with("MemTotal:"))?;
        let kib = line.split_whitespace().nth(1)?.parse::<u128>().ok()?;
        return Some(kib / 1024);
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..unsafe { std::mem::zeroed() }
        };

        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        if ok == 0 {
            return None;
        }

        return Some((status.ullTotalPhys as u128) / (1024 * 1024));
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let bytes = String::from_utf8(output.stdout).ok()?;
        let bytes = bytes.trim().parse::<u128>().ok()?;
        return Some(bytes / (1024 * 1024));
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "linux")]
fn render_linux_launcher_settings(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let mut set_linux_opengl_driver = config.linux_set_opengl_driver();
    let response = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Set Linux OpenGL Driver",
        Some(
            "Linux-only. Vertex will explicitly manage OpenGL driver environment variables for launched games. This affects all launched versions by default; versions using Vulkan directly should ignore it.",
        ),
        &mut set_linux_opengl_driver,
    );
    if response.changed() {
        config.set_linux_set_opengl_driver(set_linux_opengl_driver);
    }
    ui.add_space(crate::ui::style::SPACE_MD);

    let mut use_zink_driver = config.linux_use_zink_driver();
    let zink_response = ui.add_enabled_ui(config.linux_set_opengl_driver(), |ui| {
        settings_widgets::toggle_row(
            text_ui,
            ui,
            "Use Zink Driver (Experimental)",
            Some(
                "Linux-only. Experimental. When the setting above is enabled, forces Mesa Zink so OpenGL runs over Vulkan. Disable it to keep Mesa's default OpenGL driver selection. Versions using Vulkan directly should ignore it.",
            ),
            &mut use_zink_driver,
        )
    });
    if zink_response.inner.changed() {
        config.set_linux_use_zink_driver(use_zink_driver);
    }
    ui.add_space(crate::ui::style::SPACE_MD);
}
