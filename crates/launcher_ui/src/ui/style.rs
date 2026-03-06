use egui::Ui;
use textui::LabelOptions;

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 6.0;
pub const SPACE_MD: f32 = 8.0;
pub const SPACE_LG: f32 = 10.0;
pub const SPACE_XL: f32 = 12.0;

pub const CONTROL_HEIGHT: f32 = 30.0;
pub const CONTROL_HEIGHT_LG: f32 = 34.0;
pub const CORNER_RADIUS_SM: u8 = 8;
pub const CORNER_RADIUS_MD: u8 = 10;

pub fn page_heading(ui: &Ui) -> LabelOptions {
    LabelOptions {
        font_size: 30.0,
        line_height: 34.0,
        weight: 700,
        color: ui.visuals().text_color(),
        wrap: false,
        ..LabelOptions::default()
    }
}

pub fn section_heading(ui: &Ui) -> LabelOptions {
    LabelOptions {
        font_size: 20.0,
        line_height: 24.0,
        weight: 700,
        color: ui.visuals().text_color(),
        wrap: false,
        ..LabelOptions::default()
    }
}

pub fn body(ui: &Ui) -> LabelOptions {
    LabelOptions {
        color: ui.visuals().text_color(),
        wrap: true,
        ..LabelOptions::default()
    }
}

pub fn muted(ui: &Ui) -> LabelOptions {
    LabelOptions {
        color: ui.visuals().weak_text_color(),
        wrap: true,
        ..LabelOptions::default()
    }
}
