use egui::{Context, Id, vec2};
use modal_host::{AxisSizing, DismissBehavior, ModalLayer, ModalOptions, ModalShowResponse};

#[derive(Clone, Copy, Debug)]
pub enum DialogPreset {
    Compact,
    Confirm,
    Form,
    Viewer,
}

pub type DialogResponse<R> = ModalShowResponse<R>;

pub fn dialog_options(id: impl std::hash::Hash, preset: DialogPreset) -> ModalOptions {
    let layout = match preset {
        DialogPreset::Compact => modal_host::ModalLayout::centered(
            AxisSizing::new(0.42, 360.0, 560.0),
            AxisSizing::new(0.36, 220.0, 420.0),
        ),
        DialogPreset::Confirm => modal_host::ModalLayout::centered(
            AxisSizing::new(0.46, 380.0, 560.0),
            AxisSizing::new(0.34, 220.0, 320.0),
        ),
        DialogPreset::Form => modal_host::ModalLayout::centered(
            AxisSizing::new(0.9, 420.0, 1080.0),
            AxisSizing::new(0.9, 320.0, 900.0),
        ),
        DialogPreset::Viewer => modal_host::ModalLayout::centered(
            AxisSizing::new(0.92, 320.0, 1600.0),
            AxisSizing::new(0.9, 280.0, 1200.0),
        ),
    }
    .with_viewport_margin(vec2(12.0, 12.0))
    .with_viewport_margin_fraction(vec2(0.02, 0.02));

    ModalOptions::new(Id::new(id), layout)
        .with_layer(ModalLayer::Elevated)
        .with_dismiss_behavior(DismissBehavior::EscapeAndScrim)
}

pub fn show_dialog<R>(
    ctx: &Context,
    options: ModalOptions,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> DialogResponse<R> {
    modal_host::show_window(ctx, "", options, add_contents)
}
