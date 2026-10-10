use super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use textui_egui::interaction;
use textui_egui::prelude::*;

use crate::{
    assets,
    ui::components::{icon_button, image_textures},
};

pub(super) fn render_profile_button(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    profile_ui: ProfileUiModel<'_>,
    button_size: f32,
) -> egui::Response {
    if let Some(avatar_png) = profile_ui.avatar_png {
        let mut hasher = DefaultHasher::new();
        avatar_png.hash(&mut hasher);
        let avatar_hash = hasher.finish();
        let key = format!("bytes://vertex-profile/avatar-rounded-{avatar_hash:016x}.png");
        let rounded =
            rounded_profile_avatar_png(avatar_hash, avatar_png, PROFILE_BUTTON_CORNER_RADIUS);

        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(button_size, button_size), egui::Sense::click());
        let state = interaction::InteractionState::of(ui.ctx(), &response, true);
        let radius = egui::CornerRadius::same(PROFILE_BUTTON_CORNER_RADIUS);
        let fill = if profile_ui.auth_busy {
            ui.visuals().widgets.active.weak_bg_fill
        } else {
            interaction::FillPalette::from_visuals(ui.visuals()).fill(state, false)
        };
        ui.painter().rect_filled(rect, radius, fill);
        if let image_textures::ManagedTextureStatus::Ready(texture) =
            image_textures::request_texture(ui.ctx(), key, rounded, egui::TextureOptions::LINEAR)
        {
            // Clip the avatar to the button's own rounded shape at its drawn size.
            texture
                .image()
                .fit_to_exact_size(rect.size())
                .corner_radius(radius)
                .paint_at(ui, rect);
        }
        // The avatar covers the whole button, so hover feedback is painted on top of it.
        ui.painter().rect_filled(
            rect,
            radius,
            egui::Color32::from_white_alpha((36.0 * state.hover) as u8),
        );
        ui.painter().rect_stroke(
            rect,
            radius,
            interaction::focus_stroke(
                state,
                ui.visuals(),
                interaction::hover_stroke(
                    state,
                    egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
                    ui.visuals().widgets.hovered.bg_stroke,
                ),
            ),
            egui::StrokeKind::Inside,
        );
        interaction::paint_focus_ring(
            ui.painter(),
            ui.visuals(),
            state,
            rect,
            PROFILE_BUTTON_CORNER_RADIUS,
        );
        response
    } else if profile_ui.display_name.is_none() && profile_ui.auth_busy {
        render_profile_pending_button(ui, button_size)
    } else if profile_ui.display_name.is_none() {
        let sign_in_style = ButtonOptions {
            min_size: egui::vec2((button_size * 3.2).clamp(68.0, 110.0), button_size),
            text_color: ui.visuals().text_color(),
            fill: ui.visuals().widgets.inactive.weak_bg_fill,
            fill_hovered: ui.visuals().widgets.hovered.weak_bg_fill,
            fill_active: ui.visuals().widgets.active.weak_bg_fill,
            fill_selected: ui.visuals().widgets.open.weak_bg_fill,
            stroke: egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
            ..ButtonOptions::default()
        };
        text_ui.button(ui, "profile_button_sign_in", "Sign in", &sign_in_style)
    } else {
        ui.allocate_ui_with_layout(
            egui::vec2(button_size, button_size),
            Layout::left_to_right(Align::Center),
            |ui| {
                icon_button::svg(
                    ui,
                    "profile_selector_default",
                    assets::USER_SVG,
                    "Profile selector",
                    profile_ui.auth_busy,
                    button_size,
                )
            },
        )
        .inner
    }
}

pub(super) fn render_profile_pending_button(ui: &mut egui::Ui, button_size: f32) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(button_size, button_size), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(PROFILE_BUTTON_CORNER_RADIUS),
        ui.visuals().widgets.active.weak_bg_fill,
    );
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(PROFILE_BUTTON_CORNER_RADIUS),
        egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space((button_size * 0.2).max(2.0));
            ui.spinner();
        });
    });
    response
}

pub(super) fn render_active_user_terminal_button(
    ui: &mut egui::Ui,
    text_ui: &mut TextUi,
    button_height: f32,
    button_width: f32,
    compact: bool,
) -> egui::Response {
    let text_color = ui.visuals().text_color();
    let icon_size = (button_height - 14.0).clamp(12.0, 18.0);
    let icon = crate::ui::svg_tint::themed_svg_image(
        "topbar",
        "user-active-terminal",
        assets::TERMINAL_2_SVG,
        text_color,
        icon_size,
    );
    // The label follows the user's font settings, so size the button to it instead of assuming
    // a fixed width: the button grows to fit the label (the short one when compact).
    let label_style = LabelOptions {
        color: text_color,
        wrap: false,
        ..style::stat_label(ui)
    };
    let chrome_width = 8.0 * 2.0 + icon_size + 6.0;
    let full_label = "player active";
    let short_label = "active";
    let label = if compact { short_label } else { full_label };
    let label_width = text_ui.measure_text_size(ui, label, &label_style).x;
    let button_width = button_width.max((chrome_width + label_width).ceil());
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(button_width, button_height),
        egui::Sense::click(),
    );
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    let fill = interaction::FillPalette::from_visuals(ui.visuals()).fill(state, false);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(8),
        interaction::focus_stroke(
            state,
            ui.visuals(),
            interaction::hover_stroke(
                state,
                egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
                ui.visuals().widgets.hovered.bg_stroke,
            ),
        ),
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), ui.visuals(), state, rect, 8);

    let inner_rect = rect.shrink2(egui::vec2(8.0, 4.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let _ = ui.add_sized(egui::vec2(icon_size, icon_size), icon);
            ui.add_space(6.0);

            let text_lane_width = ui.available_width().max(1.0);
            let text_lane_height = inner_rect.height().max(1.0);
            ui.allocate_ui_with_layout(
                egui::vec2(text_lane_width, text_lane_height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_clip_rect(ui.max_rect());
                    let _ = text_ui.label(ui, "topbar_player_active_label", label, &label_style);
                },
            );
        });
    });

    style::hover_tip(ui, response, "Open the console of the game you are playing")
}
