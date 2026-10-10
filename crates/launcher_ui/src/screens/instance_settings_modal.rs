use super::*;
use std::sync::{Mutex, OnceLock, mpsc};

#[path = "instance_settings_modal/memory_slider_max_state.rs"]
mod memory_slider_max_state;

#[path = "instance_settings_modal/actions_section.rs"]
mod actions_section;
#[path = "instance_settings_modal/detected_versions.rs"]
mod detected_versions;
#[path = "instance_settings_modal/memory_slider_max.rs"]
mod memory_slider_max;
#[path = "instance_settings_modal/metadata_section.rs"]
mod metadata_section;
#[path = "instance_settings_modal/modal_styles.rs"]
mod modal_styles;
#[path = "instance_settings_modal/modloader_section.rs"]
mod modloader_section;
#[path = "instance_settings_modal/runtime_overrides_autosave.rs"]
mod runtime_overrides_autosave;
#[path = "instance_settings_modal/runtime_overrides_section.rs"]
mod runtime_overrides_section;
#[path = "instance_settings_modal/save_versions.rs"]
mod save_versions;

use self::actions_section::*;
use self::detected_versions::*;
use self::memory_slider_max::*;
use self::memory_slider_max_state::MemorySliderMaxState;
use self::metadata_section::*;
use self::modal_styles::*;
use self::modloader_section::*;
use self::runtime_overrides_autosave::*;
use self::runtime_overrides_section::*;
use self::save_versions::*;

pub(super) fn render_instance_settings_modal(
    ctx: &egui::Context,
    text_ui: &mut TextUi,
    instance_id: &str,
    state: &mut InstanceScreenState,
    instances: &mut InstanceStore,
    config: &mut Config,
) -> bool {
    if !state.show_settings_modal {
        return flush_runtime_overrides_autosave(state, instance_id, instances, config);
    }

    let installations_root = config.minecraft_installations_root_path().to_path_buf();
    let instance_root_for_detection = instances
        .find(instance_id)
        .map(|instance| instances::instance_root_path(&installations_root, instance));

    let mut instances_changed = false;
    let mut close_requested = false;
    let response = modal::show_window(
        ctx,
        "Instance Settings",
        modal::ModalOptions::new(
            egui::Id::new(("instance_settings_modal", instance_id)),
            modal::ModalLayout::centered(
                modal::AxisSizing::new(0.92, 1.0, f32::INFINITY),
                modal::AxisSizing::new(0.92, 1.0, f32::INFINITY),
            ),
        )
        .with_layer(modal::ModalLayer::Base)
        .with_dismiss_behavior(modal::DismissBehavior::EscapeAndScrim),
        |ui| {
            let styles = ModalStyles::new(ui);
            egui::ScrollArea::vertical()
                .scroll_source(modal_host::drag_scroll_source())
                .id_salt(("instance_settings_modal_scroll", instance_id))
                .scroll_source(ScrollSource {
                    scroll_bar: true,
                    drag: false,
                    mouse_wheel: true,
                })
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    render_metadata_section(ui, text_ui, state, instance_id, &styles);
                    render_version_actions_row(
                        ui,
                        text_ui,
                        state,
                        instances,
                        config,
                        instance_id,
                        &instance_root_for_detection,
                        &styles,
                        &mut instances_changed,
                    );
                    render_version_catalog_controls(ui, text_ui, state, instance_id, &styles);
                    render_modloader_section(ui, text_ui, state, instance_id, &styles);
                    render_save_versions_row(
                        ui,
                        text_ui,
                        state,
                        instances,
                        config,
                        instance_id,
                        &styles,
                        &mut instances_changed,
                    );
                    render_runtime_overrides_section(
                        ui,
                        text_ui,
                        state,
                        instances,
                        config,
                        instance_id,
                        &styles,
                        &mut instances_changed,
                    );
                    close_requested = render_actions_section(
                        ui,
                        text_ui,
                        state,
                        instances,
                        config,
                        instance_id,
                        &styles,
                    );
                });
        },
    );

    if response.close_requested || close_requested {
        state.show_settings_modal = false;
    }
    instances_changed
}
