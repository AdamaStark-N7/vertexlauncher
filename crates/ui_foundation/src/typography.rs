//! The launcher's text role system: per-role size, weight and font, published once per frame
//! from the config and looked up by every widget that draws text.

use config::{ResolvedTypography, TextRole, TypographySettings};
use egui::{Color32, Ui};
use textui::TextFundamentals;
use textui_egui::prelude::*;

fn typography_id() -> egui::Id {
    egui::Id::new("vertex_typography_settings")
}

/// Publishes the configured per-role typography so every style helper below follows it.
/// Call once per frame before drawing.
pub fn publish_typography(ctx: &egui::Context, settings: &TypographySettings) {
    let current = ctx.data(|data| data.get_temp::<TypographySettings>(typography_id()));
    if current.as_ref() != Some(settings) {
        ctx.data_mut(|data| data.insert_temp(typography_id(), settings.clone()));
        textui_egui::set_button_typography(ctx, &button_typography(settings));
        ctx.request_repaint();
    }
}

fn button_typography(settings: &TypographySettings) -> textui_egui::ButtonTypography {
    let resolved = settings.resolve(TextRole::Button);
    textui_egui::ButtonTypography {
        font_size: resolved.size,
        line_height: resolved.line_height,
        weight: resolved.weight,
        font_family: resolved.font_family,
    }
}

/// Resolved size, weight and font for `role` from the published typography settings.
pub fn resolved_typography(ui: &Ui, role: TextRole) -> ResolvedTypography {
    resolved_typography_ctx(ui.ctx(), role)
}

pub fn resolved_typography_ctx(ctx: &egui::Context, role: TextRole) -> ResolvedTypography {
    let settings = ctx
        .data(|data| data.get_temp::<TypographySettings>(typography_id()))
        .unwrap_or_default();
    settings.resolve(role)
}

/// Label options for a text role, in the given color.
pub fn role_color(ui: &Ui, role: TextRole, color: Color32, wrap: bool) -> LabelOptions {
    role_color_ctx(ui.ctx(), role, color, wrap)
}

/// [`role_color`] for code that only has an [`egui::Context`].
pub fn role_color_ctx(
    ctx: &egui::Context,
    role: TextRole,
    color: Color32,
    wrap: bool,
) -> LabelOptions {
    let resolved = resolved_typography_ctx(ctx, role);
    LabelOptions {
        font_size: resolved.size,
        line_height: resolved.line_height,
        weight: resolved.weight,
        monospace: resolved.monospace,
        color,
        wrap,
        fundamentals: TextFundamentals {
            letter_spacing_points: resolved.letter_spacing,
            case_sensitive_forms: role == TextRole::Badge,
            font_family: resolved.font_family,
            ..TextFundamentals::default()
        },
        ..LabelOptions::default()
    }
}

/// Label options for a text role in the normal text color.
pub fn role(ui: &Ui, role: TextRole, wrap: bool) -> LabelOptions {
    role_color(ui, role, ui.visuals().text_color(), wrap)
}
