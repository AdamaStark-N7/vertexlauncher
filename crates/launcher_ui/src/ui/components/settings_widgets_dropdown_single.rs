// interaction: allow (hover painting lives in settings_widgets_dropdown_button.rs)
use super::*;

pub(super) fn dropdown(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    selected_index: &mut usize,
    options: &[&str],
    metrics: ControlMetrics,
) -> Response {
    let open_id = ui.id().with("settings_dropdown_open");
    let was_open = egui::Popup::is_id_open(ui.ctx(), open_id);
    if !was_open && response_will_open(ui) && gamepad_input_history(ui.ctx()) {
        set_popup_focus_pending(ui.ctx(), open_id, true);
    }

    let mut label_style = row_label_options(ui);
    label_style.wrap = false;

    let selected_text_raw = options.get(*selected_index).copied().unwrap_or("Select...");
    let text_budget = dropdown_text_budget(metrics);
    let selected_text = cached_truncate_selected_text(
        text_ui,
        ui,
        selected_text_raw,
        text_budget,
        &label_style,
        open_id.with("selected_text_cache"),
    );
    let option_text_width = cached_option_text_width(
        text_ui,
        ui,
        options,
        &label_style,
        open_id.with("option_width_cache"),
        was_open,
    );
    let option_horizontal_padding = 16.0;
    let popup_button_width = (option_text_width + option_horizontal_padding)
        .ceil()
        .max(metrics.control_height * 2.0);
    let popup_width = popup_button_width + 4.0;

    let (button_rect, mut response) = ui.allocate_exact_size(
        egui::vec2(metrics.dropdown_width, metrics.control_height),
        Sense::click(),
    );

    let popup_response = egui::Popup::menu(&response)
        .id(open_id)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[
            egui::RectAlign::TOP_START,
            egui::RectAlign::BOTTOM_END,
            egui::RectAlign::TOP_END,
        ])
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            let mut clip_rect = ui.clip_rect();
            clip_rect.min.y = clip_rect.min.y.max(ui.ctx().content_rect().top());
            ui.set_clip_rect(clip_rect);
            ui.set_width(popup_width);

            let mut popup_changed = false;
            let mut focus_first_option = take_popup_focus_pending(ui.ctx(), open_id);
            let mut popup_had_focus = false;
            let button_options = ButtonOptions {
                min_size: egui::vec2(popup_button_width, metrics.control_height),
                corner_radius: 4,
                padding: egui::vec2(8.0, 4.0),
                text_color: ui.visuals().text_color(),
                fill: ui.visuals().widgets.inactive.bg_fill,
                fill_hovered: ui.visuals().widgets.hovered.bg_fill,
                fill_active: ui.visuals().widgets.active.bg_fill,
                fill_selected: ui.visuals().widgets.active.bg_fill,
                stroke: ui.visuals().widgets.inactive.bg_stroke,
                ..ButtonOptions::default()
            };

            let max_popup_height = (ui.ctx().content_rect().height() * 0.58)
                .clamp(metrics.control_height * 4.0, metrics.control_height * 14.0);
            let row_height = metrics.control_height + ui.spacing().item_spacing.y;
            let popup_height =
                dropdown_popup_height_limit(max_popup_height, row_height, options.len());
            let scroll_output = egui::ScrollArea::vertical()
                .scroll_source(modal_host::drag_scroll_source())
                .id_salt(("settings_dropdown_scroll", open_id))
                .max_height(popup_height)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show_rows(ui, row_height, options.len(), |ui, row_range| {
                    for index in row_range {
                        let option = options[index];
                        let option_response = text_ui.selectable_button(
                            ui,
                            ("dropdown_option", open_id, index),
                            option,
                            *selected_index == index,
                            &button_options,
                        );
                        popup_had_focus |= option_response.has_focus();
                        if focus_first_option && index == 0 {
                            option_response.request_focus();
                            popup_had_focus = true;
                            focus_first_option = false;
                        }
                        if option_response.clicked() {
                            *selected_index = index;
                            popup_changed = true;
                            set_owner_focus_pending(ui.ctx(), open_id, true);
                            egui::Popup::close_id(ui.ctx(), open_id);
                        }
                    }
                });
            make_gamepad_scrollable(ui.ctx(), &scroll_output);
            let gamepad_scroll_delta = gamepad_scroll_delta(ui.ctx());
            if gamepad_scroll_delta != egui::Vec2::ZERO
                && apply_gamepad_scroll_to_registered_id(
                    ui.ctx(),
                    scroll_output.id,
                    gamepad_scroll_delta,
                )
            {
                ui.ctx().request_repaint();
            }

            if popup_changed {
                ui.ctx().request_repaint();
            }
            if popup_had_focus {
                set_popup_had_focus(ui.ctx(), open_id, true);
            }

            popup_changed
        });

    let is_open = egui::Popup::is_id_open(ui.ctx(), open_id);
    paint_dropdown_button(
        text_ui,
        ui,
        DropdownButtonPaint {
            response: &response,
            button_rect,
            is_open,
            metrics,
            chevron_icon_id: "settings-dropdown-chevron",
            selected_text_id: "dropdown_selected_text",
        },
        &mut label_style,
        &selected_text,
    );
    if popup_response
        .as_ref()
        .map(|inner| inner.inner)
        .unwrap_or(false)
    {
        response.mark_changed();
    }
    let restore_owner_focus = if !is_open {
        take_owner_focus_pending(ui.ctx(), open_id)
            || (was_open && take_popup_had_focus(ui.ctx(), open_id))
    } else {
        false
    };
    if restore_owner_focus {
        response.request_focus();
    }

    response
}
