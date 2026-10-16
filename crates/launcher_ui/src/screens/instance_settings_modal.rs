use super::*;
use std::sync::{Mutex, OnceLock, mpsc};

#[path = "instance_settings_modal/memory_slider_max_state.rs"]
mod memory_slider_max_state;

#[path = "instance_settings_modal/instance_settings_api.rs"]
pub mod api;
#[path = "instance_settings_modal/actions_section.rs"]
mod actions_section;
#[path = "instance_settings_modal/detected_versions.rs"]
mod detected_versions;
#[path = "instance_settings_modal/memory_slider_max.rs"]
mod memory_slider_max;
#[path = "instance_settings_modal/runtime_overrides_autosave.rs"]
mod runtime_overrides_autosave;
#[path = "instance_settings_modal/save_versions.rs"]
mod save_versions;

use self::actions_section::*;
use self::detected_versions::*;
use self::memory_slider_max::*;
use self::memory_slider_max_state::MemorySliderMaxState;
use self::runtime_overrides_autosave::*;
use self::save_versions::*;


