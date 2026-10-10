use super::*;

const GAMEPAD_SLIDER_EDIT_ID: &str = "settings_widgets_gamepad_slider_edit";
const GAMEPAD_SLIDER_STEP_DELTA_ID: &str = "settings_widgets_gamepad_slider_step_delta";
const GAMEPAD_ACTIVATE_TARGET_ID: &str = "settings_widgets_gamepad_activate_target";
const GAMEPAD_CUSTOM_ACTIVATE_IDS: &str = "settings_widgets_gamepad_custom_activate_ids";
const GAMEPAD_INPUT_HISTORY_ID: &str = "settings_widgets_gamepad_input_history";
const SETTINGS_DEFAULT_FOCUS_REQUEST_ID: &str = "settings_widgets_default_focus_request";

pub fn set_gamepad_slider_step_delta(ctx: &egui::Context, delta: i32) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(GAMEPAD_SLIDER_STEP_DELTA_ID), delta));
}

pub fn gamepad_active_slider(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data_mut(|data| data.get_temp::<egui::Id>(egui::Id::new(GAMEPAD_SLIDER_EDIT_ID)))
}

pub fn set_gamepad_active_slider(ctx: &egui::Context, slider_id: Option<egui::Id>) {
    ctx.data_mut(|data| {
        let key = egui::Id::new(GAMEPAD_SLIDER_EDIT_ID);
        if let Some(slider_id) = slider_id {
            data.insert_temp(key, slider_id);
        } else {
            data.remove::<egui::Id>(key);
        }
    });
}

pub fn set_gamepad_activate_target(ctx: &egui::Context, target: Option<egui::Id>) {
    ctx.data_mut(|data| {
        let key = egui::Id::new(GAMEPAD_ACTIVATE_TARGET_ID);
        if let Some(target) = target {
            data.insert_temp(key, target);
        } else {
            data.remove::<egui::Id>(key);
        }
    });
}

pub fn set_gamepad_input_history(ctx: &egui::Context, seen: bool) {
    if !seen {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(GAMEPAD_INPUT_HISTORY_ID), true));
}

pub fn gamepad_input_history(ctx: &egui::Context) -> bool {
    ctx.data_mut(|data| {
        data.get_temp::<bool>(egui::Id::new(GAMEPAD_INPUT_HISTORY_ID))
            .unwrap_or(false)
    })
}

pub fn clear_gamepad_custom_activate_ids(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<Vec<egui::Id>>(egui::Id::new(GAMEPAD_CUSTOM_ACTIVATE_IDS)));
}

pub fn register_gamepad_custom_activate_id(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| {
        let key = egui::Id::new(GAMEPAD_CUSTOM_ACTIVATE_IDS);
        let mut ids = data.get_temp::<Vec<egui::Id>>(key).unwrap_or_default();
        if !ids.contains(&id) {
            ids.push(id);
        }
        data.insert_temp(key, ids);
    });
}

pub fn is_gamepad_custom_activate_id(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data_mut(|data| {
        data.get_temp::<Vec<egui::Id>>(egui::Id::new(GAMEPAD_CUSTOM_ACTIVATE_IDS))
            .is_some_and(|ids| ids.contains(&id))
    })
}

pub fn request_default_focus(ctx: &egui::Context, requested: bool) {
    ctx.data_mut(|data| {
        let key = egui::Id::new(SETTINGS_DEFAULT_FOCUS_REQUEST_ID);
        if requested {
            data.insert_temp(key, true);
        } else {
            data.remove::<bool>(key);
        }
    });
}

pub(super) fn gamepad_slider_step_delta(ctx: &egui::Context) -> i32 {
    ctx.data_mut(|data| {
        data.get_temp::<i32>(egui::Id::new(GAMEPAD_SLIDER_STEP_DELTA_ID))
            .unwrap_or(0)
    })
}

pub(super) fn gamepad_activate_target(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data_mut(|data| data.get_temp::<egui::Id>(egui::Id::new(GAMEPAD_ACTIVATE_TARGET_ID)))
}

pub(super) fn maybe_request_default_focus(ctx: &egui::Context, response: &egui::Response) {
    let requested = ctx.data_mut(|data| {
        data.get_temp::<bool>(egui::Id::new(SETTINGS_DEFAULT_FOCUS_REQUEST_ID))
            .unwrap_or(false)
    });
    if requested {
        response.request_focus();
        request_default_focus(ctx, false);
    }
}
