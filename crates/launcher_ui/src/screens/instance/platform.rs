use config::Config;
use instances::InstanceRecord;

use super::InstanceScreenState;

pub(super) fn effective_linux_graphics_settings_for_state(
    state: &InstanceScreenState,
    config: &Config,
) -> (bool, bool) {
    #[cfg(target_os = "linux")]
    {
        return effective_linux_graphics_settings_for_flags(
            state.linux_set_opengl_driver,
            state.linux_use_zink_driver,
            config.linux_set_opengl_driver(),
            config.linux_use_zink_driver(),
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = config;
        return (state.linux_set_opengl_driver, state.linux_use_zink_driver);
    }
}

pub(super) fn linux_instance_driver_settings_for_save(
    state: &InstanceScreenState,
    existing: Option<&InstanceRecord>,
) -> (Option<bool>, Option<bool>) {
    #[cfg(target_os = "linux")]
    {
        let _ = existing;
        return linux_instance_driver_override_for_save(
            state.linux_set_opengl_driver,
            state.linux_use_zink_driver,
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = state;
        return existing
            .map(|instance| {
                (
                    instance.linux_set_opengl_driver,
                    instance.linux_use_zink_driver,
                )
            })
            .unwrap_or((None, None));
    }
}



#[cfg(target_os = "linux")]
fn effective_linux_graphics_settings_for_flags(
    override_enabled: bool,
    use_zink_driver: bool,
    global_set_opengl_driver: bool,
    global_use_zink_driver: bool,
) -> (bool, bool) {
    if override_enabled {
        (true, use_zink_driver)
    } else {
        (global_set_opengl_driver, global_use_zink_driver)
    }
}

#[cfg(target_os = "linux")]
fn linux_instance_driver_override_for_save(
    override_enabled: bool,
    use_zink_driver: bool,
) -> (Option<bool>, Option<bool>) {
    if override_enabled {
        (Some(true), Some(use_zink_driver))
    } else {
        (None, None)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{
        effective_linux_graphics_settings_for_flags, linux_instance_driver_override_for_save,
    };

    #[test]
    fn linux_driver_override_is_cleared_when_instance_override_is_disabled() {
        assert_eq!(
            linux_instance_driver_override_for_save(false, false),
            (None, None)
        );
        assert_eq!(
            linux_instance_driver_override_for_save(false, true),
            (None, None)
        );
    }

    #[test]
    fn linux_driver_override_is_saved_when_instance_override_is_enabled() {
        assert_eq!(
            linux_instance_driver_override_for_save(true, false),
            (Some(true), Some(false))
        );
        assert_eq!(
            linux_instance_driver_override_for_save(true, true),
            (Some(true), Some(true))
        );
    }

    #[test]
    fn effective_linux_graphics_settings_use_global_when_instance_override_is_disabled() {
        assert_eq!(
            effective_linux_graphics_settings_for_flags(false, false, true, true),
            (true, true)
        );
    }

    #[test]
    fn effective_linux_graphics_settings_use_instance_zink_when_override_is_enabled() {
        assert_eq!(
            effective_linux_graphics_settings_for_flags(true, true, false, false),
            (true, true)
        );
    }
}
