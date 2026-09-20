use config::Config;
use egui::Ui;
use textui::TextUi;
use textui_egui::prelude::*;

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
use crate::ui::{components::settings_widgets, style};
#[cfg(target_os = "linux")]
use config::LinuxBlurProtocol;
#[cfg(target_os = "windows")]
use config::WindowsBackdropType;
#[cfg(target_os = "macos")]
use config::{MacosVisualEffectBlendingMode, MacosVisualEffectMaterial, MacosVisualEffectState};

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
            launcher_description: "Linux-specific settings that apply across the launcher.",
            instance_description: "Linux-specific launch compatibility settings for this instance.",
        });
    }

    #[cfg(target_os = "windows")]
    {
        return Some(PlatformSpecificSection {
            id: "windows",
            heading: "Windows",
            launcher_description: "Windows-specific window composition and compatibility settings.",
            instance_description: "Reserved for Windows-specific instance settings.",
        });
    }

    #[cfg(target_os = "macos")]
    {
        return Some(PlatformSpecificSection {
            id: "macos",
            heading: "macOS",
            launcher_description: "macOS-specific settings.",
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
    #[cfg(target_os = "windows")]
    {
        render_windows_launcher_settings(ui, text_ui, config);
        return;
    }

    #[cfg(target_os = "linux")]
    {
        render_linux_launcher_settings(ui, text_ui, config);
        return;
    }

    #[cfg(target_os = "macos")]
    {
        render_macos_launcher_settings(ui, text_ui, config);
        return;
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = (ui, text_ui, config);
    }
}

#[cfg(target_os = "windows")]
fn render_windows_launcher_settings(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let mut selected_backdrop = WindowsBackdropType::ALL
        .iter()
        .position(|value| *value == config.windows_backdrop_type())
        .unwrap_or(0);
    let backdrop_labels: Vec<&str> = WindowsBackdropType::ALL
        .iter()
        .map(|value| value.label())
        .collect();
    let backdrop_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "windows_backdrop_type",
        "Window Backdrop Type",
        Some(
            "Windows DWM system backdrop material used when Window Transparency is Blurred. Auto tries modern backdrops before legacy blur; None requests DWMSBT_NONE.",
        ),
        &mut selected_backdrop,
        &backdrop_labels,
    );
    if backdrop_response.changed()
        && let Some(next) = WindowsBackdropType::ALL.get(selected_backdrop).copied()
    {
        config.set_windows_backdrop_type(next);
    }
    let _ = text_ui.label(
        ui,
        "windows_backdrop_note",
        "Applied only when Window Transparency is Blurred. Windows does not expose supported per-window radius, strength, saturation, or noise controls through DWM.",
        &style::muted(ui),
    );
    ui.add_space(style::SPACE_MD);
}

#[cfg(target_os = "macos")]
fn render_macos_launcher_settings(ui: &mut Ui, text_ui: &mut TextUi, config: &mut Config) {
    let mut selected_material = MacosVisualEffectMaterial::ALL
        .iter()
        .position(|value| *value == config.macos_visual_effect_material())
        .unwrap_or(0);
    let material_labels: Vec<&str> = MacosVisualEffectMaterial::ALL
        .iter()
        .map(|value| value.label())
        .collect();
    let material_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "macos_visual_effect_material",
        "Visual Effect Material",
        Some("NSVisualEffectView material used when macOS native blur is active."),
        &mut selected_material,
        &material_labels,
    );
    if material_response.changed()
        && let Some(next) = MacosVisualEffectMaterial::ALL
            .get(selected_material)
            .copied()
    {
        config.set_macos_visual_effect_material(next);
    }
    ui.add_space(style::SPACE_MD);

    let mut selected_blending_mode = MacosVisualEffectBlendingMode::ALL
        .iter()
        .position(|value| *value == config.macos_visual_effect_blending_mode())
        .unwrap_or(0);
    let blending_mode_labels: Vec<&str> = MacosVisualEffectBlendingMode::ALL
        .iter()
        .map(|value| value.label())
        .collect();
    let blending_mode_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "macos_visual_effect_blending_mode",
        "Visual Effect Blending Mode",
        Some("NSVisualEffectView blending mode used when macOS native blur is active."),
        &mut selected_blending_mode,
        &blending_mode_labels,
    );
    if blending_mode_response.changed()
        && let Some(next) = MacosVisualEffectBlendingMode::ALL
            .get(selected_blending_mode)
            .copied()
    {
        config.set_macos_visual_effect_blending_mode(next);
    }
    ui.add_space(style::SPACE_MD);

    let mut selected_state = MacosVisualEffectState::ALL
        .iter()
        .position(|value| *value == config.macos_visual_effect_state())
        .unwrap_or(0);
    let state_labels: Vec<&str> = MacosVisualEffectState::ALL
        .iter()
        .map(|value| value.label())
        .collect();
    let state_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "macos_visual_effect_state",
        "Visual Effect State",
        Some("NSVisualEffectView state used when macOS native blur is active."),
        &mut selected_state,
        &state_labels,
    );
    if state_response.changed()
        && let Some(next) = MacosVisualEffectState::ALL.get(selected_state).copied()
    {
        config.set_macos_visual_effect_state(next);
    }
    ui.add_space(style::SPACE_MD);

    let mut emphasized = config.macos_visual_effect_emphasized();
    let emphasized_response = settings_widgets::toggle_row(
        text_ui,
        ui,
        "Visual Effect Emphasized",
        Some("Controls NSVisualEffectView.isEmphasized when macOS native blur is active."),
        &mut emphasized,
    );
    if emphasized_response.changed() {
        config.set_macos_visual_effect_emphasized(emphasized);
    }
    ui.add_space(style::SPACE_MD);

    let note = if crate::window_effects::platform_supports_blur() {
        "Applied only when Window Transparency is Blurred. Mask image is not exposed because Vertex uses a full-window app-wide blur region."
    } else {
        "macOS native blur is currently downgraded to transparent mode at startup; these NSVisualEffectView settings are saved for when that path is re-enabled."
    };
    let _ = text_ui.label(ui, "macos_visual_effect_note", note, &style::muted(ui));
    ui.add_space(style::SPACE_MD);
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
    let mut selected_protocol = LinuxBlurProtocol::ALL
        .iter()
        .position(|value| *value == config.linux_blur_protocol())
        .unwrap_or(0);
    let protocol_labels: Vec<&str> = LinuxBlurProtocol::ALL
        .iter()
        .map(|value| value.label())
        .collect();
    let protocol_response = settings_widgets::dropdown_row(
        text_ui,
        ui,
        "linux_blur_protocol",
        "Blur Protocol",
        Some(
            "Linux compositor blur protocol used when Window Transparency is Blurred. Auto prefers ext_background_effect_v1 and falls back to KDE blur.",
        ),
        &mut selected_protocol,
        &protocol_labels,
    );
    if protocol_response.changed()
        && let Some(next) = LinuxBlurProtocol::ALL.get(selected_protocol).copied()
    {
        config.set_linux_blur_protocol(next);
    }
    let _ = text_ui.label(
        ui,
        "linux_blur_protocol_note",
        "Linux blur protocols expose only blur regions to clients; Vertex requests a full-window region. Blur strength, radius, saturation, and noise are controlled by the compositor.",
        &style::muted(ui),
    );
    ui.add_space(style::SPACE_MD);

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
    ui.add_space(style::SPACE_MD);

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
    ui.add_space(style::SPACE_MD);
}
