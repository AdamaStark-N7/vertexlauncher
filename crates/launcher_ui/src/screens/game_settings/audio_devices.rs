//! Lists the operating system's audio output devices, for the Output Device dropdown.
//!
//! Minecraft names devices `OpenAL Soft on <name>`, where `<name>` is the platform's own device
//! name, so we ask the platform for those names. Runs external tools; call from a blocking thread.

use std::process::Command;

/// Prefix Minecraft puts in front of the platform device name.
pub const OPENAL_PREFIX: &str = "OpenAL Soft on ";

/// Output device names, best effort: empty if the platform can't be queried.
pub fn list_output_devices() -> Vec<String> {
    let mut names = platform_devices();
    names.retain(|name| !name.trim().is_empty());
    names.sort();
    names.dedup();
    names
}

#[cfg(target_os = "linux")]
fn platform_devices() -> Vec<String> {
    // PulseAudio/PipeWire: OpenAL Soft names a device after its sink description.
    let Ok(output) = Command::new("pactl")
        .args(["list", "sinks"])
        .env("LC_ALL", "C")
        .output()
    else {
        return Vec::new();
    };
    parse_pactl_sinks(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "windows")]
fn platform_devices() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = "Get-PnpDevice -Class AudioEndpoint -Status OK | \
                  Where-Object { $_.InstanceId -like '*{0.0.0.*' } | \
                  ForEach-Object { $_.FriendlyName }";
    let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_owned())
        .collect()
}

#[cfg(target_os = "macos")]
fn platform_devices() -> Vec<String> {
    let Ok(output) = Command::new("system_profiler")
        .args(["SPAudioDataType", "-json"])
        .output()
    else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for group in json["SPAudioDataType"].as_array().into_iter().flatten() {
        for item in group["_items"].as_array().into_iter().flatten() {
            let is_output = item
                .get("coreaudio_device_output")
                .is_some_and(|v| !v.is_null());
            if let (true, Some(name)) = (is_output, item["_name"].as_str()) {
                names.push(name.to_owned());
            }
        }
    }
    names
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn platform_devices() -> Vec<String> {
    Vec::new()
}

#[cfg(any(target_os = "linux", test))]
fn parse_pactl_sinks(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("Description:"))
        .map(|name| name.trim().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pactl_output_yields_sink_descriptions() {
        let text = "Sink #1\n\tState: RUNNING\n\tName: alsa_output.pci\n\tDescription: Built-in Audio Analog Stereo\n\nSink #2\n\tDescription: PCPanel Bus 1 (7.1)\n";
        assert_eq!(
            parse_pactl_sinks(text),
            ["Built-in Audio Analog Stereo", "PCPanel Bus 1 (7.1)"]
        );
    }
}
