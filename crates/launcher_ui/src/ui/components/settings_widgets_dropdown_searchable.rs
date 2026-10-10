// interaction: allow (hover painting lives in settings_widgets_dropdown_button.rs)
use super::*;

pub(super) fn searchable_dropdown(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    selected_index: &mut usize,
    options: &[&str],
    metrics: ControlMetrics,
) -> Response {
    let open_id = ui.id().with("settings_searchable_dropdown_open");
    let state_id = ui.id().with("settings_searchable_dropdown_state");
    let input_id = ui.id().with("settings_searchable_dropdown_input");
    let was_open = egui::Popup::is_id_open(ui.ctx(), open_id);
    if !was_open && response_will_open(ui) && gamepad_input_history(ui.ctx()) {
        set_popup_focus_pending(ui.ctx(), open_id, true);
    }

    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<SearchableDropdownState>(state_id))
        .unwrap_or_default();
    if !was_open {
        state.query.clear();
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
    let popup_button_width = (option_text_width + 16.0)
        .ceil()
        .max(metrics.dropdown_width)
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
            let focus_popup_entry = take_popup_focus_pending(ui.ctx(), open_id);
            let mut popup_had_focus = false;
            let search_label_style = crate::ui::style::caption(ui);
            let _ = text_ui.label(
                ui,
                ("searchable_dropdown_hint", open_id),
                "Type to search",
                &search_label_style,
            );
            ui.add_space(4.0);

            let mut search_input_options = text_input_options(ui, metrics);
            search_input_options.desired_width = Some(popup_button_width);
            search_input_options.min_width = popup_button_width;
            let search_response =
                text_ui.singleline_input(ui, input_id, &mut state.query, &search_input_options);
            popup_had_focus |= search_response.has_focus();
            if focus_popup_entry {
                search_response.request_focus();
                popup_had_focus = true;
            }
            ui.add_space(6.0);

            let filtered_indices = searchable_dropdown_matches(options, &state.query);
            if search_response.has_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter))
                && let Some(&match_index) = filtered_indices.first()
            {
                *selected_index = match_index;
                popup_changed = true;
                set_owner_focus_pending(ui.ctx(), open_id, true);
                egui::Popup::close_id(ui.ctx(), open_id);
            }

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

            if filtered_indices.is_empty() {
                let empty_style = style::caption(ui);
                let _ = text_ui.label(
                    ui,
                    ("searchable_dropdown_empty", open_id),
                    "No matches found.",
                    &empty_style,
                );
            } else {
                let scroll_output = egui::ScrollArea::vertical()
                    .scroll_source(modal_host::drag_scroll_source())
                    .id_salt(("settings_searchable_dropdown_scroll", open_id))
                    .max_height(dropdown_popup_height_limit(
                        max_popup_height,
                        row_height,
                        filtered_indices.len(),
                    ))
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show_rows(ui, row_height, filtered_indices.len(), |ui, row_range| {
                        for filtered_index in row_range {
                            let option_index = filtered_indices[filtered_index];
                            let option = options[option_index];
                            let option_response = text_ui.selectable_button(
                                ui,
                                ("searchable_dropdown_option", open_id, option_index),
                                option,
                                *selected_index == option_index,
                                &button_options,
                            );
                            popup_had_focus |= option_response.has_focus();
                            if option_response.clicked() {
                                *selected_index = option_index;
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
            }

            if popup_changed {
                state.query.clear();
                ui.ctx().request_repaint();
            }
            if popup_had_focus {
                set_popup_had_focus(ui.ctx(), open_id, true);
            }

            popup_changed
        });

    let is_open = egui::Popup::is_id_open(ui.ctx(), open_id);
    if !is_open && !response.has_focus() {
        state.query.clear();
    }
    paint_dropdown_button(
        text_ui,
        ui,
        DropdownButtonPaint {
            response: &response,
            button_rect,
            is_open,
            metrics,
            chevron_icon_id: "settings-searchable-dropdown-chevron",
            selected_text_id: "searchable_dropdown_selected_text",
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

    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
    response
}
