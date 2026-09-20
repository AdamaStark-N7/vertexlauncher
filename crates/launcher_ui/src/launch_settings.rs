//! The one place that decides which settings an instance launches with.
//!
//! Launching from the library, the instance screen, a Home world/server entry or the CLI all
//! use this, so they can't disagree about JVM arguments, memory or graphics overrides.

use config::Config;
use instances::{InstanceRecord, effective_linux_graphics_settings, normalize_optional};

/// Launch settings after applying per-instance overrides over the launcher defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceLaunchSettings {
    pub max_memory_mib: u128,
    pub extra_jvm_args: Option<String>,
    pub extra_env_vars: Option<String>,
    pub linux_set_opengl_driver: bool,
    pub linux_use_zink_driver: bool,
}

impl InstanceLaunchSettings {
    /// Per-instance values win; blank ones fall back to the launcher-wide defaults.
    pub fn resolve(config: &Config, instance: &InstanceRecord) -> Self {
        let (linux_set_opengl_driver, linux_use_zink_driver) = effective_linux_graphics_settings(
            instance,
            config.linux_set_opengl_driver(),
            config.linux_use_zink_driver(),
        );
        Self {
            max_memory_mib: instance
                .max_memory_mib
                .unwrap_or_else(|| config.default_instance_max_memory_mib()),
            extra_jvm_args: instance
                .cli_args
                .as_deref()
                .and_then(normalize_optional)
                .or_else(|| normalize_optional(config.default_instance_cli_args())),
            extra_env_vars: instance.env_vars.as_deref().and_then(normalize_optional),
            linux_set_opengl_driver,
            linux_use_zink_driver,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_overrides_beat_defaults_and_blanks_fall_back() {
        let mut config = Config::default();
        *config.default_instance_cli_args_mut() = "-XX:+UseG1GC".to_owned();

        let plain = InstanceLaunchSettings::resolve(&config, &InstanceRecord::default());
        assert_eq!(plain.extra_jvm_args.as_deref(), Some("-XX:+UseG1GC"));
        assert_eq!(
            plain.max_memory_mib,
            config.default_instance_max_memory_mib()
        );
        assert_eq!(plain.extra_env_vars, None);

        let custom = InstanceRecord {
            cli_args: Some(" -XX:+UseZGC ".to_owned()),
            max_memory_mib: Some(8192),
            env_vars: Some("A=1".to_owned()),
            ..InstanceRecord::default()
        };
        let resolved = InstanceLaunchSettings::resolve(&config, &custom);
        assert_eq!(resolved.extra_jvm_args.as_deref(), Some("-XX:+UseZGC"));
        assert_eq!(resolved.max_memory_mib, 8192);
        assert_eq!(resolved.extra_env_vars.as_deref(), Some("A=1"));

        let blank = InstanceRecord {
            cli_args: Some("   ".to_owned()),
            ..InstanceRecord::default()
        };
        assert_eq!(
            InstanceLaunchSettings::resolve(&config, &blank)
                .extra_jvm_args
                .as_deref(),
            Some("-XX:+UseG1GC")
        );
    }
}
