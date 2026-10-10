use super::*;

pub fn dropdown_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    selected_index: &mut usize,
    options: &[&str],
) -> Response {
    labeled_dropdown_row(
        text_ui,
        ui,
        id_source,
        label,
        info_tooltip,
        selected_index,
        options,
        DropdownRowIds {
            label: "dropdown_label",
            info: "dropdown_info",
        },
        dropdown,
    )
}

pub fn searchable_dropdown_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    selected_index: &mut usize,
    options: &[&str],
) -> Response {
    labeled_dropdown_row(
        text_ui,
        ui,
        id_source,
        label,
        info_tooltip,
        selected_index,
        options,
        DropdownRowIds {
            label: "searchable_dropdown_label",
            info: "searchable_dropdown_info",
        },
        searchable_dropdown,
    )
}

/// Widget id prefixes for the label and info hint of a dropdown row.
struct DropdownRowIds {
    label: &'static str,
    info: &'static str,
}

type DropdownControl = fn(&mut TextUi, &mut Ui, &mut usize, &[&str], ControlMetrics) -> Response;

/// Shared row layout (compact vertical or wide horizontal) for dropdown rows.
#[allow(clippy::too_many_arguments)]
fn labeled_dropdown_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    selected_index: &mut usize,
    options: &[&str],
    ids: DropdownRowIds,
    control: DropdownControl,
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);
    let compact_layout = ui.available_width() < 460.0;

    if compact_layout {
        let response = ui
            .vertical(|ui| {
                let label_response = text_ui.label(ui, (ids.label, label), label, &label_options);

                if info_tooltip.is_some() {
                    ui.add_space(4.0);
                    info_hint(text_ui, ui, (ids.info, label), info_tooltip);
                }

                let mut compact_metrics = metrics;
                compact_metrics.dropdown_width = ui.available_width().max(1.0);
                let dropdown_response = ui.push_id(id_source, |ui| {
                    control(text_ui, ui, selected_index, options, compact_metrics)
                });
                maybe_request_default_focus(ui.ctx(), &dropdown_response.inner);

                label_response.union(dropdown_response.inner)
            })
            .inner;
        if response.has_focus() {
            paint_focus_outline(ui, response.rect);
        }
        return response;
    }

    let response = ui
        .horizontal(|ui| {
            let label_response = text_ui.label(ui, (ids.label, label), label, &label_options);

            if info_tooltip.is_some() {
                ui.add_space(6.0);
                info_hint(text_ui, ui, (ids.info, label), info_tooltip);
            }

            let dropdown_response = ui
                .with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(metrics.right_padding);
                    ui.push_id(id_source, |ui| {
                        control(text_ui, ui, selected_index, options, metrics)
                    })
                    .inner
                })
                .inner;
            maybe_request_default_focus(ui.ctx(), &dropdown_response);

            dropdown_response.union(label_response)
        })
        .inner;
    if response.has_focus() {
        paint_focus_outline(ui, response.rect);
    }
    response
}

pub fn dropdown_picker(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    selected_index: &mut usize,
    options: &[&str],
    width: Option<f32>,
) -> Response {
    let mut metrics = control_metrics(ui);
    metrics.dropdown_width = width.unwrap_or_else(|| ui.available_width()).max(1.0);
    let response = ui.push_id(id_source, |ui| {
        dropdown(text_ui, ui, selected_index, options, metrics)
    });
    maybe_request_default_focus(ui.ctx(), &response.inner);
    response.inner
}

pub fn full_width_dropdown_row(
    text_ui: &mut TextUi,
    ui: &mut Ui,
    id_source: impl Hash,
    label: &str,
    info_tooltip: Option<&str>,
    selected_index: &mut usize,
    options: &[&str],
) -> Response {
    let metrics = control_metrics(ui);
    let label_options = row_label_options(ui);

    ui.vertical(|ui| {
        let label_response = ui
            .horizontal_wrapped(|ui| {
                let label_response = text_ui.label(
                    ui,
                    ("full_width_dropdown_label", label),
                    label,
                    &label_options,
                );
                if info_tooltip.is_some() {
                    ui.add_space(6.0);
                    info_hint(
                        text_ui,
                        ui,
                        ("full_width_dropdown_info", label),
                        info_tooltip,
                    );
                }
                label_response
            })
            .inner;

        let mut compact_metrics = metrics;
        compact_metrics.dropdown_width = ui.available_width().max(1.0);
        let dropdown_response = ui.push_id(id_source, |ui| {
            dropdown(text_ui, ui, selected_index, options, compact_metrics)
        });

        label_response.union(dropdown_response.inner)
    })
    .inner
}
