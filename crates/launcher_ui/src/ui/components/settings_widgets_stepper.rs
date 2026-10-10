use super::*;

#[derive(Clone, Debug)]
struct FloatInputState {
    text: String,
    last_valid: f32,
}

#[derive(Clone, Debug)]
struct IntInputState {
    text: String,
    last_valid: i32,
}

pub fn float_stepper_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
) -> Response {
    let metrics = control_metrics(ui);
    let id = ui.make_persistent_id(id_source);
    let input_id = id.with("float_input");
    let label_options = row_label_options(ui);

    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<FloatInputState>(id))
        .unwrap_or(FloatInputState {
            text: format_float(*value),
            last_valid: *value,
        });

    let row_response = ui
        .horizontal(|ui| {
            let label_response = text_ui.label(ui, ("float_label", label), label, &label_options);

            if info_tooltip.is_some() {
                ui.add_space(6.0);
                info_hint(text_ui, ui, ("float_info", label), info_tooltip);
            }

            let (controls_response, text_response, plus_clicked, minus_clicked) = ui
                .with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(metrics.right_padding);

                    let plus_response =
                        step_button(ui, "float_plus", assets::PLUS_SVG, "Increase", metrics);
                    ui.add_space(metrics.control_gap);

                    let mut input_options = number_input_options(ui, metrics);
                    input_options.desired_width = Some(metrics.number_input_width);
                    let text_response =
                        text_ui.singleline_input(ui, input_id, &mut state.text, &input_options);
                    ui.add_space(metrics.control_gap);

                    let minus_response =
                        step_button(ui, "float_minus", assets::MINUS_SVG, "Decrease", metrics);

                    let merged = plus_response
                        .clone()
                        .union(text_response.clone())
                        .union(minus_response.clone());

                    (
                        merged,
                        text_response,
                        plus_response.clicked(),
                        minus_response.clicked(),
                    )
                })
                .inner;

            sanitize_float_text(&mut state.text, min < 0.0);

            if let Some(parsed) = parse_float_text(&state.text) {
                if parsed >= min && parsed <= max {
                    *value = parsed;
                    state.last_valid = parsed;
                }
            }

            if plus_clicked {
                *value = (*value + step).clamp(min, max);
                state.last_valid = *value;
                state.text = format_float(*value);
            } else if minus_clicked {
                *value = (*value - step).clamp(min, max);
                state.last_valid = *value;
                state.text = format_float(*value);
            }

            if text_response.lost_focus() {
                if let Some(parsed) = parse_float_text(&state.text) {
                    if parsed >= min && parsed <= max {
                        *value = parsed;
                        state.last_valid = parsed;
                        state.text = format_float(parsed);
                    } else {
                        state.text = format_float(state.last_valid);
                    }
                } else {
                    state.text = format_float(state.last_valid);
                }
            }

            if !text_response.has_focus() {
                state.last_valid = *value;
                state.text = format_float(*value);
            }

            controls_response.union(label_response)
        })
        .inner;

    ui.ctx().data_mut(|d| d.insert_temp(id, state));
    row_response
}

pub fn int_stepper_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut i32,
    min: i32,
    max: i32,
    step: i32,
) -> Response {
    let metrics = control_metrics(ui);
    let id = ui.make_persistent_id(id_source);
    let input_id = id.with("int_input");
    let label_options = row_label_options(ui);

    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<IntInputState>(id))
        .unwrap_or(IntInputState {
            text: value.to_string(),
            last_valid: *value,
        });

    let row_response = ui
        .horizontal(|ui| {
            let label_response = text_ui.label(ui, ("int_label", label), label, &label_options);

            if info_tooltip.is_some() {
                ui.add_space(6.0);
                info_hint(text_ui, ui, ("int_info", label), info_tooltip);
            }

            let (controls_response, text_response, plus_clicked, minus_clicked) = ui
                .with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(metrics.right_padding);

                    let plus_response =
                        step_button(ui, "int_plus", assets::PLUS_SVG, "Increase", metrics);
                    ui.add_space(metrics.control_gap);

                    let mut input_options = number_input_options(ui, metrics);
                    input_options.desired_width = Some(metrics.number_input_width);
                    let text_response =
                        text_ui.singleline_input(ui, input_id, &mut state.text, &input_options);
                    ui.add_space(metrics.control_gap);

                    let minus_response =
                        step_button(ui, "int_minus", assets::MINUS_SVG, "Decrease", metrics);

                    let merged = plus_response
                        .clone()
                        .union(text_response.clone())
                        .union(minus_response.clone());

                    (
                        merged,
                        text_response,
                        plus_response.clicked(),
                        minus_response.clicked(),
                    )
                })
                .inner;

            sanitize_int_text(&mut state.text, min < 0);

            if let Some(parsed) = parse_int_text(&state.text) {
                if parsed >= min && parsed <= max {
                    *value = parsed;
                    state.last_valid = parsed;
                }
            }

            if plus_clicked {
                *value = (*value + step).clamp(min, max);
                state.last_valid = *value;
                state.text = value.to_string();
            } else if minus_clicked {
                *value = (*value - step).clamp(min, max);
                state.last_valid = *value;
                state.text = value.to_string();
            }

            if text_response.lost_focus() {
                if let Some(parsed) = parse_int_text(&state.text) {
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

            controls_response.union(label_response)
        })
        .inner;

    ui.ctx().data_mut(|d| d.insert_temp(id, state));
    row_response
}

fn step_button(
    ui: &mut Ui,
    icon_id: &str,
    icon_bytes: &'static [u8],
    tooltip: &str,
    metrics: ControlMetrics,
) -> Response {
    icon_button::svg(
        ui,
        icon_id,
        icon_bytes,
        tooltip,
        false,
        metrics.control_height,
    )
}
