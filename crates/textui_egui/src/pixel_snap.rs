use super::*;

pub(super) const WIDTH_BIN_PX: f32 = 16.0;

pub(super) fn snap_rect_to_pixel_grid(rect: Rect, pixels_per_point: f32) -> Rect {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return rect;
    }
    let snap = |value: f32| (value * pixels_per_point).round() / pixels_per_point;
    Rect::from_min_max(
        egui::pos2(snap(rect.min.x), snap(rect.min.y)),
        egui::pos2(snap(rect.max.x), snap(rect.max.y)),
    )
}

pub(super) fn snap_width_to_bin(width_points: f32, scale: f32) -> f32 {
    let width_px = (width_points * scale).round();
    let snapped_px = (width_px / WIDTH_BIN_PX).floor() * WIDTH_BIN_PX;
    (snapped_px / scale).max(1.0)
}

pub(super) fn normalize_wrapped_width(width_points_opt: Option<f32>, scale: f32) -> Option<f32> {
    width_points_opt.map(|width| snap_width_to_bin(width.max(1.0), scale))
}
