use std::hash::{DefaultHasher, Hash, Hasher};

#[path = "settings_widgets_button.rs"]
mod settings_widgets_button;
#[path = "settings_widgets_dropdown.rs"]
mod settings_widgets_dropdown;
#[path = "settings_widgets_dropdown_button.rs"]
mod settings_widgets_dropdown_button;
#[path = "settings_widgets_dropdown_cache.rs"]
mod settings_widgets_dropdown_cache;
#[path = "settings_widgets_dropdown_searchable.rs"]
mod settings_widgets_dropdown_searchable;
#[path = "settings_widgets_dropdown_single.rs"]
mod settings_widgets_dropdown_single;
#[path = "settings_widgets_gamepad.rs"]
mod settings_widgets_gamepad;
#[path = "settings_widgets_number_text.rs"]
mod settings_widgets_number_text;
#[path = "settings_widgets_search.rs"]
mod settings_widgets_search;
#[path = "settings_widgets_slider.rs"]
mod settings_widgets_slider;
#[path = "settings_widgets_stepper.rs"]
mod settings_widgets_stepper;
#[path = "settings_widgets_text_input.rs"]
mod settings_widgets_text_input;
#[path = "settings_widgets_toggle.rs"]
mod settings_widgets_toggle;

use egui::{self, Align, Layout, Response, Sense, Ui};
use textui::TextUi;
use textui_egui::interaction;
use textui_egui::{
    apply_gamepad_scroll_if_focused, apply_gamepad_scroll_to_registered_id, gamepad_scroll_delta,
    make_gamepad_scrollable, prelude::*,
};

use crate::{
    assets,
    ui::{
        color::lerp_color32_oklab, components::icon_button, style, text_input_theme,
        toggle_anim::ToggleAnimState,
    },
};
pub use settings_widgets_button::{full_width_button, selectable_chip_button};
pub use settings_widgets_dropdown::{
    dropdown_picker, dropdown_row, full_width_dropdown_row, searchable_dropdown_row,
};
use settings_widgets_dropdown_button::{
    DropdownButtonPaint, dropdown_popup_height_limit, dropdown_text_budget, paint_dropdown_button,
    response_will_open,
};
use settings_widgets_dropdown_cache::{cached_option_text_width, cached_truncate_selected_text};
use settings_widgets_dropdown_searchable::searchable_dropdown;
use settings_widgets_dropdown_single::dropdown;
pub use settings_widgets_gamepad::{
    clear_gamepad_custom_activate_ids, gamepad_active_slider, gamepad_input_history,
    is_gamepad_custom_activate_id, register_gamepad_custom_activate_id, request_default_focus,
    set_gamepad_activate_target, set_gamepad_active_slider, set_gamepad_input_history,
    set_gamepad_slider_step_delta,
};
use settings_widgets_gamepad::{
    gamepad_activate_target, gamepad_slider_step_delta, maybe_request_default_focus,
};
use settings_widgets_number_text::{
    format_float, parse_float_text, parse_int_text, parse_u128_text, sanitize_float_text,
    sanitize_int_text, sanitize_u128_text,
};
use settings_widgets_search::{
    SearchableDropdownState, searchable_dropdown_matches, set_owner_focus_pending,
    set_popup_focus_pending, set_popup_had_focus, take_owner_focus_pending,
    take_popup_focus_pending, take_popup_had_focus,
};
pub use settings_widgets_slider::{float_slider_row, u128_slider_with_input_row};
pub use settings_widgets_stepper::{float_stepper_row, int_stepper_row};
pub use settings_widgets_text_input::{
    full_width_multiline_text_input_row, full_width_text_input_row, text_input_row,
};
pub use settings_widgets_toggle::toggle_row;

#[derive(Clone, Copy, Debug)]
struct ControlMetrics {
    right_padding: f32,
    control_height: f32,
    switch_width: f32,
    dropdown_width: f32,
    number_input_width: f32,
    icon_size: f32,
    control_gap: f32,
}

fn paint_focus_outline(ui: &Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect.expand2(egui::vec2(2.0, 2.0)),
        egui::CornerRadius::same(8),
        ui.visuals().selection.stroke,
        egui::StrokeKind::Outside,
    );
}

pub fn info_hint(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    tooltip: Option<&str>,
) -> Response {
    let metrics = control_metrics(ui);
    let icon = themed_svg_image(
        "settings-info-circle",
        assets::INFO_CIRCLE_SVG,
        metrics.icon_size,
        ui.visuals().weak_text_color(),
    )
    .sense(Sense::hover())
    .fit_to_exact_size(egui::vec2(metrics.icon_size, metrics.icon_size));

    let response = ui.add(icon);
    if let Some(text) = tooltip {
        let mut tooltip_options = TooltipOptions::default();
        tooltip_options.text.color = ui.visuals().text_color();
        tooltip_options.background = ui.visuals().widgets.noninteractive.bg_fill;
        tooltip_options.stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        text_ui.tooltip_for_response(ui, id_source, &response, text, &tooltip_options);
    }
    response
}

/// Settings-row icon: tinted, cached under the `settings` scope.
fn themed_svg_image(
    icon_id: &str,
    svg_bytes: &[u8],
    icon_size: f32,
    color: egui::Color32,
) -> egui::Image<'static> {
    crate::ui::svg_tint::themed_svg_image("settings", icon_id, svg_bytes, color, icon_size)
}

fn row_label_options(ui: &Ui) -> LabelOptions {
    LabelOptions {
        color: ui.visuals().text_color(),
        wrap: false,
        ..style::body(ui)
    }
}

fn number_input_options(ui: &Ui, metrics: ControlMetrics) -> InputOptions {
    let mut options = text_input_theme::themed_text_input_options(ui, true);
    options.desired_rows = 1;
    options.desired_width = Some(metrics.number_input_width);
    options.min_width = metrics.number_input_width;
    options.padding = egui::vec2(6.0, 4.0);
    options
}

fn text_input_options(ui: &Ui, metrics: ControlMetrics) -> InputOptions {
    let mut options = text_input_theme::themed_text_input_options(ui, false);
    options.desired_rows = 1;
    options.desired_width = Some(metrics.dropdown_width);
    options.min_width = metrics.dropdown_width;
    options.padding = egui::vec2(6.0, 4.0);
    options
}

fn control_metrics(ui: &Ui) -> ControlMetrics {
    let viewport_width = ui.ctx().input(|i| i.content_rect().width()).max(320.0);
    let local_width = ui.available_width().clamp(220.0, viewport_width);
    let text_height = crate::ui::style::resolved_typography(ui, config::TextRole::Body)
        .line_height
        .max(14.0);
    let control_height = (local_width * 0.024).clamp(22.0, 34.0);
    let control_gap = (control_height * 0.20).clamp(4.0, 8.0);
    let number_input_width = (local_width * 0.10).clamp(84.0, 150.0);
    let step_button_width = control_height;
    let number_selector_width =
        number_input_width + (step_button_width * 2.0) + (control_gap * 2.0);

    ControlMetrics {
        right_padding: (local_width * 0.01).clamp(6.0, 16.0),
        control_height,
        switch_width: (control_height * 1.95).clamp(42.0, 72.0),
        dropdown_width: number_selector_width,
        number_input_width,
        icon_size: text_height.clamp(14.0, 20.0),
        control_gap,
    }
}
