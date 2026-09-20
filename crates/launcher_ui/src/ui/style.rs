use config::TextRole;
use egui::{Color32, Ui, Vec2};
use textui_egui::prelude::*;

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 6.0;
pub const SPACE_MD: f32 = 8.0;
pub const SPACE_LG: f32 = 10.0;
pub const SPACE_XL: f32 = 12.0;

pub const CONTROL_HEIGHT: f32 = 30.0;
pub const CONTROL_HEIGHT_LG: f32 = 34.0;
pub const CORNER_RADIUS_SM: u8 = 8;
pub const CORNER_RADIUS_MD: u8 = 10;

pub use ui_foundation::typography::{
    publish_typography, resolved_typography, resolved_typography_ctx, role, role_color,
    role_color_ctx,
};

pub fn page_heading(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::PageHeading, false)
}

pub fn section_heading(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::SectionHeading, false)
}

/// Heading with an explicit size, for one-off layouts. Weight and font still follow
/// the section-heading role.
pub fn heading(ui: &Ui, font_size: f32, line_height: f32) -> LabelOptions {
    heading_color(ui, font_size, line_height, ui.visuals().text_color())
}

pub fn heading_color(ui: &Ui, font_size: f32, line_height: f32, color: Color32) -> LabelOptions {
    LabelOptions {
        font_size,
        line_height,
        ..role_color(ui, TextRole::Subtitle, color, false)
    }
}

pub fn body(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::Body, true)
}

pub fn caption(ui: &Ui) -> LabelOptions {
    role_color(ui, TextRole::Caption, ui.visuals().weak_text_color(), false)
}

pub fn badge_label(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::Badge, false)
}

/// Subtitle — used for modal/dialog titles and mid-level headings.
pub fn subtitle(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::Subtitle, false)
}

/// Modal/dialog title.
pub fn modal_title(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::ModalTitle, false)
}

/// [`stat_label`] for code that only has an [`egui::Context`].
pub fn stat_label_ctx(ctx: &egui::Context) -> LabelOptions {
    role_color_ctx(
        ctx,
        TextRole::StatLabel,
        ctx.global_style().visuals.text_color(),
        false,
    )
}

/// Small heading / stat label.
pub fn stat_label(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::StatLabel, false)
}

/// Applies the Input role's size and font to text input options.
pub fn apply_input_typography(ui: &Ui, options: &mut InputOptions) {
    let resolved = resolved_typography(ui, TextRole::Input);
    options.font_size = resolved.size;
    options.line_height = resolved.line_height;
    options.weight = resolved.weight;
    options.monospace |= resolved.monospace;
    options.fundamentals.font_family = resolved.font_family;
}

/// Default text input options that follow the Input role.
pub fn input_options(ui: &Ui) -> InputOptions {
    let mut options = InputOptions::default();
    apply_input_typography(ui, &mut options);
    options
}

/// Console and other monospace text.
pub fn code(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::Code, false)
}

#[derive(Clone)]
struct PendingTooltip {
    id: egui::Id,
    pointer: egui::Pos2,
    text: String,
}

fn pending_tooltip_id() -> egui::Id {
    egui::Id::new("vertex_pending_tooltip")
}

/// Queues a tooltip for `response`, drawn with textui by [`flush_tooltips`] at the end of
/// the frame. Use instead of `Response::on_hover_text`, which draws with egui's own fonts.
/// Returns the response for chaining. The last hovered widget in draw order wins.
pub fn hover_tip(ui: &Ui, response: egui::Response, text: impl Into<String>) -> egui::Response {
    if response.hovered() || response.contains_pointer() {
        let pointer = response
            .hover_pos()
            .or_else(|| ui.ctx().pointer_latest_pos())
            .unwrap_or(response.rect.right_bottom());
        let pending = PendingTooltip {
            id: response.id,
            pointer,
            text: text.into(),
        };
        ui.ctx()
            .data_mut(|data| data.insert_temp(pending_tooltip_id(), pending));
    }
    response
}

/// Draws the tooltip queued by [`hover_tip`] this frame, if any. Call once per frame after
/// all UI has been laid out.
pub fn flush_tooltips(text_ui: &mut textui::TextUi, ctx: &egui::Context) {
    let Some(pending) = ctx.data_mut(|data| {
        let pending = data.get_temp::<PendingTooltip>(pending_tooltip_id());
        data.remove::<PendingTooltip>(pending_tooltip_id());
        pending
    }) else {
        return;
    };
    let options = TooltipOptions {
        text: role_color_ctx(
            ctx,
            TextRole::Caption,
            ctx.global_style().visuals.text_color(),
            true,
        ),
        ..TooltipOptions::default()
    };
    text_ui.tooltip_at(
        ctx,
        pending.id,
        pending.pointer,
        pending.text.as_str(),
        &options,
    );
}

/// Error text — body-sized, error foreground color.
pub fn error_text(ui: &Ui) -> LabelOptions {
    role_color(ui, TextRole::Body, ui.visuals().error_fg_color, true)
}

/// Warning text — body-sized, warn foreground color.
pub fn warning_text(ui: &Ui) -> LabelOptions {
    role_color(ui, TextRole::Body, ui.visuals().warn_fg_color, true)
}

/// Emphasized body text.
pub fn body_strong(ui: &Ui) -> LabelOptions {
    role(ui, TextRole::BodyStrong, true)
}

pub fn muted(ui: &Ui) -> LabelOptions {
    role_color(ui, TextRole::Body, ui.visuals().weak_text_color(), true)
}

pub fn muted_single_line(ui: &Ui) -> LabelOptions {
    let mut style = muted(ui);
    style.wrap = false;
    style
}

pub fn neutral_button(ui: &Ui) -> ButtonOptions {
    ButtonOptions {
        text_color: ui.visuals().text_color(),
        fill: ui.visuals().widgets.inactive.bg_fill,
        fill_hovered: ui.visuals().widgets.hovered.bg_fill,
        fill_active: ui.visuals().widgets.active.bg_fill,
        fill_selected: ui.visuals().selection.bg_fill,
        stroke: ui.visuals().widgets.inactive.bg_stroke,
        ..ButtonOptions::default()
    }
}

pub fn neutral_button_with_min_size(ui: &Ui, min_size: Vec2) -> ButtonOptions {
    ButtonOptions {
        min_size,
        ..neutral_button(ui)
    }
}
