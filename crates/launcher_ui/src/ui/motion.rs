use egui::{Context, Id};

pub fn progress(ctx: &Context, id: Id, target_on: bool) -> f32 {
    ctx.animate_bool(id, target_on)
}

pub fn is_animating(value: f32) -> bool {
    value > 0.0 && value < 1.0
}
