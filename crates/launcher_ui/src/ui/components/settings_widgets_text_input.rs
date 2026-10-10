use super::*;

pub fn text_input_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut String,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let input_id = ui.make_persistent_id(id_source).with("text_input");

    ui.horizontal(|ui| {
        let label_response = text_ui.label(ui, ("text_label", label), label, &label_options);

        if info_tooltip.is_some() {
            ui.add_space(6.0);
            info_hint(text_ui, ui, ("text_info", label), info_tooltip);
        }

        let text_response = ui
            .with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(metrics.right_padding);
                let mut input_options = text_input_options(ui, metrics);
                input_options.desired_width = Some(metrics.dropdown_width);
                input_options.min_width = metrics.dropdown_width;
                text_ui.singleline_input(ui, input_id, value, &input_options)
            })
            .inner;

        text_response.union(label_response)
    })
    .inner
}

pub fn full_width_text_input_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut String,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let input_id = ui
        .make_persistent_id(id_source)
        .with("full_width_text_input");

    ui.vertical(|ui| {
        let label_response = ui
            .horizontal(|ui| {
                let label_response =
                    text_ui.label(ui, ("full_width_text_label", label), label, &label_options);
                if info_tooltip.is_some() {
                    ui.add_space(6.0);
                    info_hint(text_ui, ui, ("full_width_text_info", label), info_tooltip);
                }
                label_response
            })
            .inner;

        let mut input_options = text_input_options(ui, metrics);
        let input_width = ui.available_width().max(1.0);
        input_options.desired_width = Some(input_width);
        input_options.min_width = 1.0;
        let input_response = text_ui.singleline_input(ui, input_id, value, &input_options);

        label_response.union(input_response)
    })
    .inner
}

pub fn full_width_multiline_text_input_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    value: &mut String,
    desired_rows: usize,
    placeholder: Option<&str>,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let input_id = ui
        .make_persistent_id(id_source)
        .with("full_width_multiline_text_input");

    ui.vertical(|ui| {
        let label_response = ui
            .horizontal(|ui| {
                let label_response = text_ui.label(
                    ui,
                    ("full_width_multiline_text_label", label),
                    label,
                    &label_options,
                );
                if info_tooltip.is_some() {
                    ui.add_space(6.0);
                    info_hint(
                        text_ui,
                        ui,
                        ("full_width_multiline_text_info", label),
                        info_tooltip,
                    );
                }
                label_response
            })
            .inner;

        let mut input_options = text_input_options(ui, metrics);
        input_options.desired_width = Some(ui.available_width().max(1.0));
        input_options.min_width = 1.0;
        input_options.desired_rows = desired_rows.max(2);
        input_options.monospace = true;
        input_options.padding = egui::vec2(10.0, 8.0);
        input_options.placeholder_text = placeholder.map(str::to_owned);
        let input_response = text_ui.multiline_input(ui, input_id, value, &input_options);

        label_response.union(input_response)
    })
    .inner
}
