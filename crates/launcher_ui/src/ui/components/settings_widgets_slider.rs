use super::*;

#[derive(Clone, Debug)]
struct U128InputState {
    text: String,
    last_valid: u128,
}

pub fn u128_slider_with_input_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut u128,
    min: u128,
    max: u128,
    step: u128,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let id = ui.make_persistent_id(id_source);
    let input_id = id.with("u128_slider_input");
    let full_width = ui.available_width().max(1.0);

    *value = (*value).clamp(min, max);

    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<U128InputState>(id))
        .unwrap_or(U128InputState {
            text: value.to_string(),
            last_valid: *value,
        });

    let row_response = ui
        .vertical(|ui| {
            ui.set_min_width(full_width);
            let label_response = ui
                .horizontal(|ui| {
                    let label_response =
                        text_ui.label(ui, ("int_slider_label", label), label, &label_options);
                    if info_tooltip.is_some() {
                        ui.add_space(6.0);
                        info_hint(text_ui, ui, ("int_slider_info", label), info_tooltip);
                    }
                    label_response
                })
                .inner;

            let controls_response = ui
                .vertical(|ui| {
                    ui.set_min_width(full_width);
                    let slider_min = min.min(u64::MAX as u128) as u64;
                    let slider_max = max.min(u64::MAX as u128) as u64;
                    let mut slider_value = (*value).clamp(min, max).min(u64::MAX as u128) as u64;

                    let slider_outer_height = metrics.control_height + 12.0;
                    let (slider_outer_rect, _) = ui.allocate_exact_size(
                        egui::vec2(full_width, slider_outer_height),
                        Sense::hover(),
                    );

                    let slider_inner_rect = slider_outer_rect.shrink2(egui::vec2(8.0, 6.0));
                    let slider_id = id.with("u128_slider_drag");
                    let mut slider_response =
                        ui.interact(slider_inner_rect, slider_id, Sense::click_and_drag());
                    register_gamepad_custom_activate_id(ui.ctx(), slider_id);
                    maybe_request_default_focus(ui.ctx(), &slider_response);
                    let activate_target = gamepad_activate_target(ui.ctx());
                    let slider_editing = gamepad_active_slider(ui.ctx()) == Some(slider_id);
                    if slider_editing && !slider_response.has_focus() {
                        set_gamepad_active_slider(ui.ctx(), None);
                    }
                    let mut slider_changed = false;

                    if activate_target == Some(slider_id) {
                        slider_response.request_focus();
                        set_gamepad_active_slider(ui.ctx(), Some(slider_id));
                    }

                    if (slider_response.dragged() || slider_response.clicked())
                        && let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.interact_pos())
                    {
                        let slider_width = slider_inner_rect.width().max(1.0);
                        let t = ((pointer_pos.x - slider_inner_rect.left()) / slider_width)
                            .clamp(0.0, 1.0);
                        let range = slider_max.saturating_sub(slider_min);
                        let raw = slider_min as f64 + (range as f64 * t as f64);
                        let mut next = raw.round() as u64;

                        let step_u64 = step.min(u64::MAX as u128) as u64;
                        if step_u64 > 1 {
                            let from_min = next.saturating_sub(slider_min);
                            let quantized =
                                ((from_min + (step_u64 / 2)) / step_u64).saturating_mul(step_u64);
                            next = slider_min.saturating_add(quantized).min(slider_max);
                        }

                        if next != slider_value {
                            slider_value = next;
                            slider_changed = true;
                        }
                    }

                    let gamepad_step_delta = if slider_editing && slider_response.has_focus() {
                        gamepad_slider_step_delta(ui.ctx())
                    } else {
                        0
                    };
                    if gamepad_step_delta != 0 {
                        let step_u64 = step.min(u64::MAX as u128) as u64;
                        let step_u64 = step_u64.max(1);
                        let delta = if gamepad_step_delta > 0 {
                            step_u64
                        } else {
                            step_u64.saturating_mul((-gamepad_step_delta) as u64)
                        };
                        let next = if gamepad_step_delta > 0 {
                            slider_value.saturating_add(delta).min(slider_max)
                        } else {
                            slider_value.saturating_sub(delta).max(slider_min)
                        };
                        if next != slider_value {
                            slider_value = next;
                            slider_changed = true;
                        }
                    }
                    apply_gamepad_scroll_if_focused(ui, &slider_response);

                    let progress = if slider_max > slider_min {
                        (slider_value - slider_min) as f32 / (slider_max - slider_min) as f32
                    } else {
                        0.0
                    }
                    .clamp(0.0, 1.0);

                    paint_slider_rail(
                        ui,
                        slider_inner_rect,
                        progress,
                        &slider_response,
                        slider_editing,
                    );

                    if slider_changed {
                        *value = slider_value as u128;
                        state.last_valid = *value;
                        state.text = value.to_string();
                        slider_response.mark_changed();
                    }

                    let mut input_options = number_input_options(ui, metrics);
                    let input_width = (metrics.number_input_width + 32.0).clamp(120.0, 220.0);
                    input_options.desired_width = Some(input_width);
                    input_options.min_width = input_width;

                    let text_response = ui
                        .horizontal(|ui| {
                            text_ui.singleline_input(ui, input_id, &mut state.text, &input_options)
                        })
                        .inner;

                    sanitize_u128_text(&mut state.text);

                    if let Some(parsed) = parse_u128_text(&state.text) {
                        if parsed >= min && parsed <= max {
                            *value = parsed;
                            state.last_valid = parsed;
                        }
                    }

                    if text_response.lost_focus() {
                        if let Some(parsed) = parse_u128_text(&state.text) {
                            if parsed >= min && parsed <= max {
                                *value = parsed;
                                state.last_valid = parsed;
                                state.text = parsed.to_string();
                            } else {
                                state.text = state.last_valid.to_string();
                            }
                        } else {
                            state.text = state.last_valid.to_string();
                        }
                    }

                    if !text_response.has_focus() {
                        state.last_valid = *value;
                        state.text = value.to_string();
                    }

                    slider_response.union(text_response)
                })
                .inner;

            label_response.union(controls_response)
        })
        .inner;

    ui.ctx().data_mut(|d| d.insert_temp(id, state));
    row_response
}

pub fn float_slider_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut f32,
    min: f32,
    max: f32,
    show_percentage: bool,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let full_width = ui.available_width().max(1.0);
    let id = ui.make_persistent_id(id_source).with("float_slider");

    *value = value.clamp(min, max);

    let row_response = ui
        .vertical(|ui| {
            ui.set_min_width(full_width);
            let label_response = ui
                .horizontal(|ui| {
                    let label_response =
                        text_ui.label(ui, ("float_slider_label", label), label, &label_options);
                    if info_tooltip.is_some() {
                        ui.add_space(6.0);
                        info_hint(text_ui, ui, ("float_slider_info", label), info_tooltip);
                    }
                    label_response
                })
                .inner;

            let controls_response = ui
                .vertical(|ui| {
                    ui.set_min_width(full_width);

                    let slider_outer_height = metrics.control_height + 12.0;
                    let (slider_outer_rect, _) = ui.allocate_exact_size(
                        egui::vec2(full_width, slider_outer_height),
                        Sense::hover(),
                    );

                    let slider_inner_rect = slider_outer_rect.shrink2(egui::vec2(8.0, 6.0));
                    let mut slider_response =
                        ui.interact(slider_inner_rect, id, Sense::click_and_drag());
                    register_gamepad_custom_activate_id(ui.ctx(), id);
                    maybe_request_default_focus(ui.ctx(), &slider_response);
                    let activate_target = gamepad_activate_target(ui.ctx());
                    let slider_editing = gamepad_active_slider(ui.ctx()) == Some(id);
                    if slider_editing && !slider_response.has_focus() {
                        set_gamepad_active_slider(ui.ctx(), None);
                    }

                    if activate_target == Some(id) {
                        slider_response.request_focus();
                        set_gamepad_active_slider(ui.ctx(), Some(id));
                    }

                    if (slider_response.dragged() || slider_response.clicked())
                        && let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.interact_pos())
                    {
                        let slider_width = slider_inner_rect.width().max(1.0);
                        let t = ((pointer_pos.x - slider_inner_rect.left()) / slider_width)
                            .clamp(0.0, 1.0);
                        let next = egui::lerp(min..=max, t).clamp(min, max);
                        if (next - *value).abs() > f32::EPSILON {
                            *value = next;
                            slider_response.mark_changed();
                        }
                    }

                    let gamepad_step_delta = if slider_editing && slider_response.has_focus() {
                        gamepad_slider_step_delta(ui.ctx())
                    } else {
                        0
                    };
                    if gamepad_step_delta != 0 {
                        let step = ((max - min) / 50.0).abs().max(0.001);
                        let next = (*value + step * gamepad_step_delta as f32).clamp(min, max);
                        if (next - *value).abs() > f32::EPSILON {
                            *value = next;
                            slider_response.mark_changed();
                        }
                    }
                    apply_gamepad_scroll_if_focused(ui, &slider_response);

                    let progress = if max > min {
                        (*value - min) / (max - min)
                    } else {
                        0.0
                    }
                    .clamp(0.0, 1.0);

                    paint_slider_rail(
                        ui,
                        slider_inner_rect,
                        progress,
                        &slider_response,
                        slider_editing,
                    );

                    let value_text = if show_percentage {
                        format!("{:.0}%", *value * 100.0)
                    } else {
                        format_float(*value)
                    };
                    let mut value_style = row_label_options(ui);
                    value_style.wrap = false;
                    value_style.color = ui.visuals().weak_text_color();
                    let value_response = ui.horizontal(|ui| {
                        text_ui.label(ui, ("float_slider_value", label), &value_text, &value_style)
                    });

                    slider_response.union(value_response.inner)
                })
                .inner;

            label_response.union(controls_response)
        })
        .inner;

    row_response
}

/// Paints the slider rail, filled portion and knob shared by the slider rows.
fn paint_slider_rail(
    ui: &mut Ui,
    slider_inner_rect: egui::Rect,
    progress: f32,
    slider_response: &Response,
    slider_editing: bool,
) {
    let rail_height = (slider_inner_rect.height() * 0.22).clamp(3.0, 8.0);
    let rail_rect = egui::Rect::from_center_size(
        slider_inner_rect.center(),
        egui::vec2(slider_inner_rect.width(), rail_height),
    );
    let active_width = rail_rect.width() * progress;
    let active_rect =
        egui::Rect::from_min_size(rail_rect.min, egui::vec2(active_width, rail_rect.height()));
    let knob_x = rail_rect.left() + active_width;
    let knob_center = egui::pos2(knob_x, rail_rect.center().y);
    let knob_radius = (slider_inner_rect.height() * 0.34).clamp(6.0, 11.0);

    let slider_state =
        interaction::InteractionState::of(ui.ctx(), slider_response, ui.is_enabled());
    let rail_stroke = if slider_response.has_focus() || slider_editing {
        ui.visuals().selection.stroke
    } else {
        interaction::hover_stroke(
            slider_state,
            ui.visuals().widgets.inactive.bg_stroke,
            ui.visuals().widgets.hovered.bg_stroke,
        )
    };
    ui.painter().rect(
        rail_rect,
        egui::CornerRadius::same((rail_height * 0.5).round() as u8),
        ui.visuals().widgets.inactive.bg_fill,
        rail_stroke,
        egui::StrokeKind::Inside,
    );
    ui.painter().rect(
        active_rect,
        egui::CornerRadius::same((rail_height * 0.5).round() as u8),
        ui.visuals().selection.bg_fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    // The knob grows slightly on hover and while dragged.
    let knob_grow = if slider_state.pressed {
        1.0
    } else {
        slider_state.hover
    };
    ui.painter().circle(
        knob_center,
        knob_radius * (1.0 + 0.2 * knob_grow),
        ui.visuals().widgets.noninteractive.fg_stroke.color,
        egui::Stroke::new(1.0, rail_stroke.color),
    );
}
