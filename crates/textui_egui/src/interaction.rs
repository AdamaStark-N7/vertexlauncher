//! Launcher-wide rules for how interactive widgets react to the pointer and keyboard.
//!
//! Every clickable widget, built-in or hand-painted, follows these rules so the whole UI
//! reads the same way:
//!
//! 1. **Cursor.** An enabled clickable widget shows the pointing-hand cursor while hovered.
//! 2. **Hover fade.** Fill and stroke fade from idle to hovered over the theme's animation
//!    time (`Style::animation_time`, `0` under reduced motion). Hover must change *both* the
//!    fill and the stroke, so it stays visible on transparent or low-contrast surfaces.
//! 3. **Press.** While the primary button is held the widget snaps to the pressed fill with
//!    no fade, so a click always feels instant.
//! 4. **Selected.** A selected widget rests on the selection fill and still reacts to hover
//!    by fading to a slightly lighter selected fill, so hover is visible on selected items too.
//! 5. **Focus.** Keyboard focus draws the selection-colored ring around the widget, on top
//!    of whichever hover state applies.
//! 6. **Disabled.** A disabled widget has no cursor change, no hover fade and no press
//!    state, and is painted at [`DISABLED_OPACITY`].
//! 7. **Cards, rows and tiles.** Large clickable surfaces use the same fill and stroke rules.
//!    Media tiles that must not be tinted highlight with the stroke only.
//! 8. **Icon-only controls** always carry a tooltip that names the action.

use egui::{Color32, Context, CornerRadius, Painter, Rect, Response, Stroke, StrokeKind, Visuals};

/// Opacity applied to the content of a disabled widget.
pub const DISABLED_OPACITY: f32 = 0.5;

/// How much of the text color is mixed into a selected widget's fill while it is hovered.
const SELECTED_HOVER_MIX: f32 = 0.14;

/// Per-frame interaction state of one widget.
#[derive(Clone, Copy, Debug)]
pub struct InteractionState {
    /// Animated hover amount, `0.0` (idle) to `1.0` (fully hovered).
    pub hover: f32,
    /// `true` while the primary pointer button is held on the widget.
    pub pressed: bool,
    /// `true` while the widget has keyboard focus.
    pub focused: bool,
    /// `false` for a disabled widget; all other fields are then inert.
    pub enabled: bool,
}

impl InteractionState {
    /// Like [`InteractionState::of`], for a control drawn *under* a larger interactive
    /// surface that captures the pointer (for example a button inside a clickable tile), where
    /// egui reports the control as not hovered. Hover and press come from the pointer position
    /// over `rect` instead.
    pub fn of_covered(ctx: &Context, response: &Response, rect: Rect, enabled: bool) -> Self {
        if !enabled {
            return Self::of(ctx, response, false);
        }
        let over = ctx
            .pointer_hover_pos()
            .is_some_and(|pos| rect.contains(pos));
        if over {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        Self {
            hover: ctx.animate_bool(response.id.with("interaction_hover"), over),
            pressed: over && ctx.input(|input| input.pointer.primary_down()),
            focused: response.has_focus(),
            enabled,
        }
    }

    /// Reads `response`, advances the hover fade and applies the pointing-hand cursor.
    pub fn of(ctx: &Context, response: &Response, enabled: bool) -> Self {
        if !enabled {
            return Self {
                hover: 0.0,
                pressed: false,
                focused: false,
                enabled,
            };
        }
        let hovered = response.hovered() || response.has_focus();
        if response.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        Self {
            hover: ctx.animate_bool(response.id.with("interaction_hover"), hovered),
            pressed: response.is_pointer_button_down_on(),
            focused: response.has_focus(),
            enabled,
        }
    }
}

/// The fills a widget moves between.
#[derive(Clone, Copy, Debug)]
pub struct FillPalette {
    pub idle: Color32,
    pub hovered: Color32,
    pub pressed: Color32,
    pub selected: Color32,
    pub selected_hovered: Color32,
}

impl FillPalette {
    /// Palette for a widget that uses the current theme's standard widget colors.
    pub fn from_visuals(visuals: &Visuals) -> Self {
        Self::new(
            visuals.widgets.inactive.bg_fill,
            visuals.widgets.hovered.bg_fill,
            visuals.widgets.active.bg_fill,
            visuals.selection.bg_fill,
            visuals.text_color(),
        )
    }

    /// Palette from explicit colors; the selected+hovered fill is derived from `selected`
    /// mixed toward `text`.
    pub fn new(
        idle: Color32,
        hovered: Color32,
        pressed: Color32,
        selected: Color32,
        text: Color32,
    ) -> Self {
        Self {
            idle,
            hovered,
            pressed,
            selected,
            selected_hovered: selected.lerp_to_gamma(text, SELECTED_HOVER_MIX),
        }
    }

    /// Resolves the fill for the current interaction state.
    pub fn fill(&self, state: InteractionState, selected: bool) -> Color32 {
        let (rest, hover) = if selected {
            (self.selected, self.selected_hovered)
        } else {
            (self.idle, self.hovered)
        };
        if !state.enabled {
            return rest;
        }
        if state.pressed {
            return self.pressed;
        }
        rest.lerp_to_gamma(hover, state.hover)
    }
}

/// Fades a stroke from `idle` to `hovered` (color and width) by the hover amount.
pub fn hover_stroke(state: InteractionState, idle: Stroke, hovered: Stroke) -> Stroke {
    if !state.enabled {
        return idle;
    }
    Stroke::new(
        idle.width + (hovered.width - idle.width) * state.hover,
        idle.color.lerp_to_gamma(hovered.color, state.hover),
    )
}

/// The hovered version of a custom-colored `idle` stroke: slightly wider and mixed toward `toward`
/// (normally the text color). Use it where the theme's hovered stroke would clash with the
/// widget's own colors, for example danger buttons.
pub fn emphasized_stroke(idle: Stroke, toward: Color32) -> Stroke {
    Stroke::new(idle.width + 0.5, idle.color.lerp_to_gamma(toward, 0.35))
}

/// Stroke for a keyboard-focused widget, or `idle` when it is not focused.
pub fn focus_stroke(state: InteractionState, visuals: &Visuals, idle: Stroke) -> Stroke {
    if state.focused {
        visuals.selection.stroke
    } else {
        idle
    }
}

/// Draws the keyboard-focus ring just outside `rect`. Does nothing when not focused.
pub fn paint_focus_ring(
    painter: &Painter,
    visuals: &Visuals,
    state: InteractionState,
    rect: Rect,
    corner_radius: u8,
) {
    if !state.focused {
        return;
    }
    painter.rect_stroke(
        rect.expand(2.0),
        CornerRadius::same(corner_radius.saturating_add(2)),
        Stroke::new(
            (visuals.selection.stroke.width + 1.0).max(2.0),
            visuals.selection.stroke.color,
        ),
        StrokeKind::Outside,
    );
}

/// Highlights a large clickable surface (card, tile, row) that has its own content: a faint
/// wash of the text color when `tint` is set, plus a stroke that fades in on top. Media tiles
/// should pass `tint = false` so the picture is not washed out.
pub fn paint_card_highlight(
    painter: &Painter,
    visuals: &Visuals,
    state: InteractionState,
    rect: Rect,
    corner_radius: u8,
    tint: bool,
) {
    if !state.enabled || state.hover <= 0.0 {
        return;
    }
    let radius = CornerRadius::same(corner_radius);
    if tint {
        painter.rect_filled(
            rect,
            radius,
            visuals.text_color().gamma_multiply(0.06 * state.hover),
        );
    }
    let stroke = visuals.widgets.hovered.bg_stroke;
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(stroke.width + 0.5, stroke.color.gamma_multiply(state.hover)),
        StrokeKind::Inside,
    );
}

/// Multiplies `color`'s opacity by [`DISABLED_OPACITY`] when the widget is disabled.
pub fn dim_if_disabled(state: InteractionState, color: Color32) -> Color32 {
    if state.enabled {
        color
    } else {
        color.gamma_multiply(DISABLED_OPACITY)
    }
}
