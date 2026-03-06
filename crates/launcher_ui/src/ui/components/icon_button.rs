use egui::{Button, Color32, Image, Response, Stroke, Ui, vec2};

use crate::ui::motion;

pub fn svg(
    ui: &mut Ui,
    icon_id: &str,
    svg_bytes: &'static [u8],
    _tooltip: &str,
    selected: bool,
    max_button_width: f32,
) -> Response {
    let text_color = ui.visuals().text_color();
    let themed_svg = apply_text_color(svg_bytes, text_color);
    let uri = format!(
        "bytes://vertex-icons/{icon_id}-{:02x}{:02x}{:02x}.svg",
        text_color.r(),
        text_color.g(),
        text_color.b()
    );
    let button_size = ui.available_width().min(max_button_width).max(1.0);
    let icon_size = (button_size - 8.0).clamp(10.0, button_size);
    let icon = Image::from_bytes(uri, themed_svg).fit_to_exact_size(vec2(icon_size, icon_size));

    const CORNER_RADIUS_DEFAULT: f32 = 10.0;
    const CORNER_RADIUS_SELECTED: f32 = 5.0;
    let hover_progress_id = ui.make_persistent_id(icon_id).with("hover_progress");
    let hover_progress = ui
        .ctx()
        .data_mut(|d| d.get_temp::<f32>(hover_progress_id))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let hover_target_radius =
        CORNER_RADIUS_DEFAULT - ((CORNER_RADIUS_DEFAULT - CORNER_RADIUS_SELECTED) * 0.5);
    let corner_radius = if selected {
        CORNER_RADIUS_SELECTED
    } else {
        CORNER_RADIUS_DEFAULT + (hover_target_radius - CORNER_RADIUS_DEFAULT) * hover_progress
    };
    let button = Button::image(icon)
        .frame(true)
        .corner_radius(egui::CornerRadius::same(corner_radius.round() as u8))
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.inactive.bg_stroke.color,
        ))
        .fill(if selected {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().widgets.inactive.weak_bg_fill
        });

    let response = ui.add_sized([button_size, button_size], button);
    let emphasis = response.hovered() || response.has_focus();
    let progress = motion::progress(ui.ctx(), response.id.with("hover_anim"), emphasis);
    ui.ctx()
        .data_mut(|d| d.insert_temp(hover_progress_id, progress));
    if progress > 0.0 {
        let stroke_color = ui
            .visuals()
            .widgets
            .hovered
            .bg_stroke
            .color
            .gamma_multiply((0.35 + (0.65 * progress)).clamp(0.0, 1.0));
        let stroke = Stroke::new(1.0 + (0.8 * progress), stroke_color);
        let radius = if selected {
            corner_radius.round() as u8
        } else {
            ((button_size * 0.2).round() as u8).max(4)
        };
        ui.painter().rect_stroke(
            response.rect,
            egui::CornerRadius::same(radius),
            stroke,
            egui::StrokeKind::Inside,
        );
        if motion::is_animating(progress) {
            ui.ctx().request_repaint();
        }
    }
    response
}

fn apply_text_color(svg_bytes: &[u8], color: Color32) -> Vec<u8> {
    let color_hex = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
    let svg = String::from_utf8_lossy(svg_bytes).replace("currentColor", &color_hex);
    svg.into_bytes()
}
