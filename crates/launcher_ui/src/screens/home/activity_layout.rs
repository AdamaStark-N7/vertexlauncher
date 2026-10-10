use super::*;

pub(super) fn render_activity_text_stack(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    name: &str,
    meta: &str,
    width: f32,
    name_label_options: &LabelOptions,
    meta_label_options: &LabelOptions,
) {
    render_activity_text_stack_sized(
        ui,
        text_ui,
        id_source,
        name,
        meta,
        width,
        activity_entry_text_stack_height(ui, name_label_options, meta_label_options),
        name_label_options,
        meta_label_options,
    );
}

pub(super) fn render_activity_text_stack_sized(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash + Copy,
    name: &str,
    meta: &str,
    width: f32,
    height: f32,
    name_label_options: &LabelOptions,
    meta_label_options: &LabelOptions,
) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        Layout::top_down(egui::Align::LEFT),
        |ui| {
            let text_stack_height =
                activity_entry_text_stack_height(ui, name_label_options, meta_label_options);
            ui.add_space(((height - text_stack_height) * 0.5).max(0.0));
            let clip_rect = ui.max_rect().intersect(ui.clip_rect());
            ui.set_clip_rect(clip_rect);
            let _ = text_ui.label(ui, (id_source, "name"), name, name_label_options);
            let _ = text_ui.label(ui, (id_source, "meta"), meta, meta_label_options);
        },
    );
}

pub(super) fn activity_entry_name_label_options(ui: &Ui) -> LabelOptions {
    style::body_strong(ui)
}

pub(super) fn activity_entry_meta_label_options(ui: &Ui) -> LabelOptions {
    style::muted_single_line(ui)
}

pub(super) fn activity_entry_row_height(
    ui: &Ui,
    metrics: HomeUiMetrics,
    entry: &HomeEntryRef<'_>,
) -> f32 {
    let trailing_content_height = match entry {
        HomeEntryRef::World(_) => 0.0,
        HomeEntryRef::Server(_) => SERVER_PING_ICON_SIZE,
    };
    let name_label_options = activity_entry_name_label_options(ui);
    let meta_label_options = activity_entry_meta_label_options(ui);
    activity_entry_min_row_height(
        ui,
        metrics,
        &name_label_options,
        &meta_label_options,
        trailing_content_height,
    )
}

pub(super) fn activity_entry_min_row_height(
    ui: &Ui,
    metrics: HomeUiMetrics,
    name_label_options: &LabelOptions,
    meta_label_options: &LabelOptions,
    trailing_content_height: f32,
) -> f32 {
    let text_stack_height = activity_entry_text_stack_height_for_spacing(
        ui.spacing().item_spacing.y,
        name_label_options,
        meta_label_options,
    );
    let content_height = text_stack_height
        .max(ACTIVITY_ENTRY_THUMBNAIL_SIZE)
        .max(FAVORITE_STAR_BUTTON_SIZE)
        .max(trailing_content_height);
    metrics
        .activity_row_height
        .max((content_height + (ACTIVITY_ENTRY_ROW_VERTICAL_PADDING * 2.0)).ceil())
}

pub(super) fn activity_entry_text_stack_height(
    ui: &Ui,
    name_label_options: &LabelOptions,
    meta_label_options: &LabelOptions,
) -> f32 {
    activity_entry_text_stack_height_for_spacing(
        ui.spacing().item_spacing.y,
        name_label_options,
        meta_label_options,
    )
}

pub(super) fn activity_entry_text_stack_height_for_spacing(
    item_spacing_y: f32,
    name_label_options: &LabelOptions,
    meta_label_options: &LabelOptions,
) -> f32 {
    activity_entry_label_height(name_label_options)
        + item_spacing_y
        + activity_entry_label_height(meta_label_options)
}

pub(super) fn activity_entry_label_height(label_options: &LabelOptions) -> f32 {
    (label_options.line_height + (label_options.padding.y * 2.0)).ceil()
}
