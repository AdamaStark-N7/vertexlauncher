
#[cfg(target_os = "windows")]
use config::WindowsBackdropType;
#[cfg(target_os = "macos")]
use config::{MacosVisualEffectBlendingMode, MacosVisualEffectMaterial, MacosVisualEffectState};











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


