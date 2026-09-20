use egui::{Color32, Stroke, Vec2};

/// Font size buttons are authored against; role typography scales relative to it.
pub const DEFAULT_BUTTON_FONT_SIZE: f32 = 18.0;

/// Launcher-wide button label typography, published once per frame with
/// [`set_button_typography`] and applied when a button is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonTypography {
    pub font_size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub font_family: Option<String>,
}

fn button_typography_id() -> egui::Id {
    egui::Id::new("textui_egui_button_typography")
}

pub fn set_button_typography(ctx: &egui::Context, typography: &ButtonTypography) {
    let current = ctx.data(|data| data.get_temp::<ButtonTypography>(button_typography_id()));
    if current.as_ref() != Some(typography) {
        ctx.data_mut(|data| data.insert_temp(button_typography_id(), typography.clone()));
        ctx.request_repaint();
    }
}

pub(crate) fn button_typography(ctx: &egui::Context) -> Option<ButtonTypography> {
    ctx.data(|data| data.get_temp::<ButtonTypography>(button_typography_id()))
}

#[derive(Clone, Debug)]
pub struct ButtonOptions {
    pub font_size: f32,
    pub line_height: f32,
    pub text_color: Color32,
    pub fill: Color32,
    pub fill_hovered: Color32,
    pub fill_active: Color32,
    pub fill_selected: Color32,
    pub stroke: Stroke,
    pub corner_radius: u8,
    pub padding: Vec2,
    pub min_size: Vec2,
}

impl Default for ButtonOptions {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            line_height: 24.0,
            text_color: Color32::WHITE,
            fill: Color32::from_rgb(24, 28, 34),
            fill_hovered: Color32::from_rgb(30, 35, 42),
            fill_active: Color32::from_rgb(36, 43, 52),
            fill_selected: Color32::from_rgb(40, 56, 74),
            stroke: Stroke::new(1.0, Color32::from_rgb(52, 58, 68)),
            corner_radius: 8,
            padding: egui::vec2(10.0, 6.0),
            min_size: egui::vec2(88.0, 30.0),
        }
    }
}
