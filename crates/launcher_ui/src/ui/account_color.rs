//! Shows an account's chosen color: highlighted names while streamer mode hides them, and
//! the swatches of the color picker.

use egui::{Color32, CornerRadius, Painter, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};
use textui::TextUi;
use textui_egui::{interaction, prelude::*};

use crate::account_colors;

const BOX_CORNER_RADIUS: u8 = 3;
/// How far the name highlight extends past the text. It is only painted, so it never
/// changes the layout.
const HIGHLIGHT_OVERHANG: egui::Vec2 = egui::vec2(3.0, 1.0);

/// Fills `rect` with `color`, or with a neutral placeholder when no color is chosen.
fn paint_box(painter: &Painter, ui: &Ui, rect: Rect, color: Option<Color32>, stroke: Stroke) {
    let radius = CornerRadius::same(BOX_CORNER_RADIUS);
    let fill = color.unwrap_or(ui.visuals().widgets.inactive.weak_bg_fill);
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
}

/// Marks a selected swatch with a dot in whichever of black or white reads on `background`.
fn paint_selection_dot(painter: &Painter, rect: Rect, background: Color32) {
    painter.circle_filled(
        rect.center(),
        rect.width().min(rect.height()) * 0.2,
        account_colors::contrasting_color(background),
    );
}

/// The color to highlight `account_id`'s name with: its chosen color while streamer mode
/// hides the name, otherwise none.
pub fn name_highlight(streamer_mode: bool, account_id: &str) -> Option<Color32> {
    streamer_mode
        .then(|| account_colors::color_for(account_id))
        .flatten()
}

/// Draws `name` as a label (clickable if `clickable`). With a `highlight`, the color is
/// painted behind the text and the text switches to black or white for contrast. The highlight
/// is painted under the label without taking any space, so the layout is the same with or
/// without it.
pub fn highlighted_name_label(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    id_source: impl std::hash::Hash,
    name: &str,
    options: &LabelOptions,
    highlight: Option<Color32>,
    clickable: bool,
) -> Response {
    // Reserve the slot first so the highlight lands under the text.
    let background_slot = ui.painter().add(Shape::Noop);
    let mut options = options.clone();
    if let Some(color) = highlight {
        options.color = account_colors::contrasting_color(color);
    }
    let response = if clickable {
        text_ui.clickable_label(ui, id_source, name, &options)
    } else {
        text_ui.label(ui, id_source, name, &options)
    };
    if let Some(color) = highlight {
        ui.painter().set(
            background_slot,
            Shape::rect_filled(
                response.rect.expand2(HIGHLIGHT_OVERHANG),
                CornerRadius::same(BOX_CORNER_RADIUS + 1),
                color,
            ),
        );
    }
    response
}

/// Restyles a button so it is filled with `color` (a highlighted account row), keeping the
/// text readable and the hover and press feedback visible. `emphasized` outlines the button.
pub fn highlight_button_style(style: &mut ButtonOptions, color: Color32, emphasized: bool) {
    let text_color = account_colors::contrasting_color(color);
    style.fill = color;
    style.fill_selected = color;
    style.fill_hovered = color.lerp_to_gamma(text_color, 0.18);
    style.fill_active = color.lerp_to_gamma(text_color, 0.3);
    style.text_color = text_color;
    if emphasized {
        style.stroke = Stroke::new(2.0, text_color);
    }
}

/// A clickable color swatch. `selected` draws the dot that shows which color is in use.
/// `color` of `None` draws the neutral "no color" box.
pub fn color_swatch_button(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    color: Option<Color32>,
    size: f32,
    selected: bool,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
    let response = ui.interact(rect, ui.id().with(id_source), Sense::click());
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    let idle = Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color);
    let stroke = interaction::focus_stroke(
        state,
        ui.visuals(),
        interaction::hover_stroke(state, idle, ui.visuals().widgets.hovered.bg_stroke),
    );
    if ui.is_rect_visible(rect) {
        paint_box(ui.painter(), ui, rect, color, stroke);
        if selected {
            let background = color.unwrap_or(ui.visuals().widgets.inactive.weak_bg_fill);
            paint_selection_dot(ui.painter(), rect, background);
        }
        interaction::paint_focus_ring(ui.painter(), ui.visuals(), state, rect, BOX_CORNER_RADIUS);
    }
    response
}

/// A non-interactive color box of `size`; `selected` draws the selection dot on it.
pub fn color_preview_box(ui: &mut Ui, color: Option<Color32>, size: f32, selected: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let stroke = Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
        paint_box(ui.painter(), ui, rect, color, stroke);
        if selected {
            let background = color.unwrap_or(ui.visuals().widgets.inactive.weak_bg_fill);
            paint_selection_dot(ui.painter(), rect, background);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_only_highlighted_in_streamer_mode() {
        // Outside streamer mode the color store is not even consulted.
        assert_eq!(name_highlight(false, "any-account"), None);
    }

    #[test]
    fn highlighted_buttons_stay_readable_and_show_feedback() {
        for color in [
            Color32::from_rgb(0xFD, 0xD8, 0x35),
            Color32::from_rgb(0x5E, 0x35, 0xB1),
        ] {
            let mut style = ButtonOptions::default();
            highlight_button_style(&mut style, color, false);
            assert_eq!(style.fill, color);
            assert_eq!(style.fill_selected, color);
            assert_eq!(style.text_color, account_colors::contrasting_color(color));
            assert_ne!(style.fill_hovered, color, "hover must be visible");
            assert_ne!(
                style.fill_active, style.fill_hovered,
                "press must be visible"
            );
        }
    }

    #[test]
    fn only_the_emphasized_button_gets_an_outline() {
        let color = Color32::from_rgb(0x43, 0xA0, 0x47);
        let mut plain = ButtonOptions::default();
        let untouched_stroke = plain.stroke;
        highlight_button_style(&mut plain, color, false);
        assert_eq!(plain.stroke, untouched_stroke);

        let mut emphasized = ButtonOptions::default();
        highlight_button_style(&mut emphasized, color, true);
        assert_eq!(emphasized.stroke.width, 2.0);
    }
}
