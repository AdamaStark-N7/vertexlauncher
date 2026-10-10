//! Painting and viewer math shared by the home screenshot gallery and the per-instance one.
//!
//! Both galleries render the same masonry tiles and the same zoom/pan viewer; only their egui
//! id salts, constants, and state types differ, so those are passed in by the callers.

use std::ffi::OsStr;
use std::hash::Hash;
use std::path::Path;
use std::sync::Arc;

use egui::{Color32, TextureOptions, Ui};
use textui::TextUi;
use textui_egui::interaction;
use textui_egui::prelude::*;

use crate::ui::{
    components::{image_textures, lazy_image_bytes::LazyImageBytesStatus},
    style,
};

const TILE_TEXTURE_MAX_EDGE: u32 = 1024;
const TILE_CORNER_RADIUS: u8 = 14;
const VIEWER_CORNER_RADIUS: u8 = 12;

pub(in crate::screens) fn is_supported_screenshot_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .map(|extension| extension.to_ascii_lowercase()),
        Some(extension)
            if matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
            )
    )
}

pub(in crate::screens) fn ui_pointer_over_rect(ui: &Ui, rect: egui::Rect) -> bool {
    ui.input(|input| {
        input
            .pointer
            .interact_pos()
            .or_else(|| input.pointer.hover_pos())
            .is_some_and(|pointer_pos| rect.contains(pointer_pos))
    })
}

pub(in crate::screens) fn fit_rect_to_aspect(rect: egui::Rect, aspect_ratio: f32) -> egui::Rect {
    let safe_aspect = aspect_ratio.max(0.01);
    let rect_aspect = rect.width() / rect.height().max(1.0);
    if rect_aspect > safe_aspect {
        let width = rect.height() * safe_aspect;
        let x = rect.center().x - width * 0.5;
        egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(width, rect.height()))
    } else {
        let height = rect.width() / safe_aspect;
        let y = rect.center().y - height * 0.5;
        egui::Rect::from_min_size(egui::pos2(rect.min.x, y), egui::vec2(rect.width(), height))
    }
}

pub(in crate::screens) fn decode_color_image(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let rgba = image::load_from_memory(bytes)
        .map_err(|err| err.to_string())?
        .to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

/// Paints the tile picture (or its loading/failed placeholder) into `rect`.
pub(in crate::screens) fn paint_tile_image(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    rect: egui::Rect,
    image_key: &str,
    image_bytes: Option<&Arc<[u8]>>,
    image_status: LazyImageBytesStatus,
    placeholder_salt: &'static str,
) {
    let Some(bytes) = image_bytes else {
        paint_tile_placeholder(ui, text_ui, rect, image_status, placeholder_salt);
        return;
    };
    match image_textures::request_texture_with_max_edge(
        ui.ctx(),
        image_key.to_owned(),
        Arc::clone(bytes),
        TextureOptions::LINEAR,
        TILE_TEXTURE_MAX_EDGE,
    ) {
        image_textures::ManagedTextureStatus::Ready(texture) => {
            texture
                .image()
                .fit_to_exact_size(rect.size())
                .corner_radius(egui::CornerRadius::same(TILE_CORNER_RADIUS))
                .paint_at(ui, rect);
        }
        image_textures::ManagedTextureStatus::Loading => {
            paint_tile_placeholder(
                ui,
                text_ui,
                rect,
                LazyImageBytesStatus::Loading,
                placeholder_salt,
            );
        }
        image_textures::ManagedTextureStatus::Failed => {
            paint_tile_placeholder(
                ui,
                text_ui,
                rect,
                LazyImageBytesStatus::Failed,
                placeholder_salt,
            );
        }
    }
}

fn paint_tile_placeholder(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    rect: egui::Rect,
    image_status: LazyImageBytesStatus,
    salt: &'static str,
) {
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(TILE_CORNER_RADIUS),
        ui.visuals().widgets.inactive.bg_fill,
    );
    let label = match image_status {
        LazyImageBytesStatus::Loading => "Loading...",
        LazyImageBytesStatus::Failed => "Failed to load",
        LazyImageBytesStatus::Ready | LazyImageBytesStatus::Unrequested => "Waiting...",
    };
    paint_centered_label(ui, text_ui, rect, (salt, label), label);
}

fn paint_centered_label(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    rect: egui::Rect,
    id_source: impl Hash,
    label: &str,
) {
    let label_style = LabelOptions {
        color: ui.visuals().weak_text_color(),
        ..style::body_strong(ui)
    };
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                let _ = text_ui.label(ui, id_source, label, &label_style);
            },
        );
    });
}

/// Paints the tile border, highlighted by `state` (media tiles never tint the picture).
pub(in crate::screens) fn paint_tile_border(
    ui: &Ui,
    rect: egui::Rect,
    state: interaction::InteractionState,
) {
    let hovered_stroke = egui::Stroke::new(2.0, ui.visuals().widgets.hovered.bg_stroke.color);
    let stroke = interaction::focus_stroke(
        state,
        ui.visuals(),
        interaction::hover_stroke(
            state,
            ui.visuals().widgets.inactive.bg_stroke,
            hovered_stroke,
        ),
    );
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(TILE_CORNER_RADIUS),
        stroke,
        egui::StrokeKind::Inside,
    );
}

/// Paints the dark caption pill along the bottom of a tile.
pub(in crate::screens) fn paint_tile_caption(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    tile_rect: egui::Rect,
    id_source: impl Hash,
    caption: &str,
) {
    let label_bg_rect = egui::Rect::from_min_max(
        egui::pos2(tile_rect.min.x + 10.0, tile_rect.max.y - 34.0),
        egui::pos2(tile_rect.max.x - 10.0, tile_rect.max.y - 10.0),
    );
    ui.painter().rect_filled(
        label_bg_rect,
        egui::CornerRadius::same(8),
        Color32::from_rgba_premultiplied(6, 9, 14, 185),
    );
    let label_style = LabelOptions {
        color: Color32::WHITE,
        wrap: false,
        ..style::body(ui)
    };
    let label_text_rect = label_bg_rect.shrink2(egui::vec2(8.0, 0.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(label_text_rect), |ui| {
        ui.set_clip_rect(label_text_rect.intersect(ui.clip_rect()));
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let _ = text_ui.label(ui, id_source, caption, &label_style);
        });
    });
}

/// Zoom limits of one screenshot viewer, plus the zoom/pan math derived from them.
#[derive(Clone, Copy)]
pub(in crate::screens) struct ViewerZoomLimits {
    pub(in crate::screens) min: f32,
    pub(in crate::screens) max: f32,
    pub(in crate::screens) step: f32,
    pub(in crate::screens) scroll_sensitivity: f32,
}

impl ViewerZoomLimits {
    pub(in crate::screens) fn adjust(&self, current_zoom: f32, direction: f32) -> f32 {
        (current_zoom + direction * self.step).clamp(self.min, self.max)
    }

    fn adjust_with_scroll(&self, current_zoom: f32, scroll_delta: f32) -> f32 {
        let scale = (1.0 + scroll_delta * self.scroll_sensitivity).clamp(0.7, 1.3);
        (current_zoom * scale).clamp(self.min, self.max)
    }

    pub(in crate::screens) fn clamp_pan(&self, zoom: f32, pan_uv: &mut egui::Vec2) {
        let visible_fraction = 1.0 / zoom.max(self.min);
        let max_offset = (1.0 - visible_fraction) * 0.5;
        pan_uv.x = pan_uv.x.clamp(-max_offset, max_offset);
        pan_uv.y = pan_uv.y.clamp(-max_offset, max_offset);
    }

    fn uv_rect(&self, zoom: f32, pan_uv: egui::Vec2) -> egui::Rect {
        let visible_fraction = 1.0 / zoom.max(self.min);
        let half = visible_fraction * 0.5;
        let center = egui::pos2(0.5 + pan_uv.x, 0.5 + pan_uv.y);
        egui::Rect::from_min_max(
            egui::pos2(center.x - half, center.y - half),
            egui::pos2(center.x + half, center.y + half),
        )
    }
}

/// Gamepad stick/trigger input for a viewer plus the clamped frame time, read once per frame.
pub(in crate::screens) struct ViewerGamepadInput {
    pan: egui::Vec2,
    zoom: f32,
    frame_dt: f32,
}

impl ViewerGamepadInput {
    pub(in crate::screens) fn read(
        ctx: &egui::Context,
        pan_key: &'static str,
        zoom_key: &'static str,
    ) -> Self {
        let pan = ctx
            .data(|data| data.get_temp::<egui::Vec2>(egui::Id::new(pan_key)))
            .unwrap_or(egui::Vec2::ZERO);
        let zoom = ctx
            .data(|data| data.get_temp::<f32>(egui::Id::new(zoom_key)))
            .unwrap_or(0.0);
        let frame_dt = ctx.input(|input| input.stable_dt).clamp(1.0 / 240.0, 0.05);
        Self {
            pan,
            zoom,
            frame_dt,
        }
    }
}

/// What the viewer canvas shows and how it should label failures.
pub(in crate::screens) struct ViewerCanvas<'a> {
    pub(in crate::screens) limits: ViewerZoomLimits,
    pub(in crate::screens) gamepad: ViewerGamepadInput,
    pub(in crate::screens) aspect_ratio: f32,
    pub(in crate::screens) image_key: &'a str,
    pub(in crate::screens) image_bytes: Option<&'a Arc<[u8]>>,
    pub(in crate::screens) image_status: LazyImageBytesStatus,
    pub(in crate::screens) status_salt: &'static str,
}

/// Draws the zoomable/pannable picture canvas and applies mouse and gamepad input to
/// `zoom`/`pan_uv`.
pub(in crate::screens) fn render_viewer_canvas(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    canvas: &ViewerCanvas<'_>,
    zoom: &mut f32,
    pan_uv: &mut egui::Vec2,
) {
    let limits = canvas.limits;
    let canvas_size = ui.available_size().max(egui::vec2(1.0, 1.0));
    let (canvas_rect, response) = ui.allocate_exact_size(canvas_size, egui::Sense::drag());
    ui.painter().rect_filled(
        canvas_rect,
        egui::CornerRadius::same(VIEWER_CORNER_RADIUS),
        ui.visuals().faint_bg_color,
    );

    let image_rect = fit_rect_to_aspect(canvas_rect.shrink(8.0), canvas.aspect_ratio);
    apply_viewer_input(ui, canvas, &response, image_rect, zoom, pan_uv);

    let corner = egui::CornerRadius::same(VIEWER_CORNER_RADIUS);
    let status_label = match canvas.image_bytes {
        Some(bytes) => match image_textures::request_viewer_texture(
            ui.ctx(),
            canvas.image_key.to_owned(),
            Arc::clone(bytes),
        ) {
            image_textures::ViewerTextureStatus::Ready(texture) => {
                texture.paint(ui, image_rect, limits.uv_rect(*zoom, *pan_uv), corner);
                None
            }
            image_textures::ViewerTextureStatus::Loading => Some("Loading screenshot..."),
            image_textures::ViewerTextureStatus::Failed => Some("Failed to load screenshot"),
        },
        None => Some(if canvas.image_status == LazyImageBytesStatus::Failed {
            "Failed to load screenshot"
        } else {
            "Loading screenshot..."
        }),
    };
    if let Some(label) = status_label {
        ui.painter()
            .rect_filled(image_rect, corner, ui.visuals().widgets.inactive.bg_fill);
        paint_centered_label(ui, text_ui, image_rect, (canvas.status_salt, label), label);
    }
    ui.painter().rect_stroke(
        image_rect,
        corner,
        ui.visuals().widgets.inactive.bg_stroke,
        egui::StrokeKind::Inside,
    );
}

fn apply_viewer_input(
    ui: &Ui,
    canvas: &ViewerCanvas<'_>,
    response: &egui::Response,
    image_rect: egui::Rect,
    zoom: &mut f32,
    pan_uv: &mut egui::Vec2,
) {
    let limits = canvas.limits;
    let gamepad = &canvas.gamepad;
    if response.hovered() {
        let scroll_delta = ui.ctx().input(|input| input.smooth_scroll_delta.y);
        if scroll_delta.abs() > 0.0 {
            *zoom = limits.adjust_with_scroll(*zoom, scroll_delta);
            limits.clamp_pan(*zoom, pan_uv);
            ui.ctx().request_repaint();
        }
    }
    if response.dragged() && *zoom > limits.min {
        let visible_fraction = 1.0 / zoom.max(limits.min);
        let delta = ui.ctx().input(|input| input.pointer.delta());
        pan_uv.x -= delta.x / image_rect.width().max(1.0) * visible_fraction;
        pan_uv.y -= delta.y / image_rect.height().max(1.0) * visible_fraction;
        limits.clamp_pan(*zoom, pan_uv);
        ui.ctx().request_repaint();
    }
    if gamepad.zoom.abs() > 0.05 {
        let zoom_scale = (1.0 + gamepad.zoom * 1.8 * gamepad.frame_dt).clamp(0.7, 1.3);
        *zoom = (*zoom * zoom_scale).clamp(limits.min, limits.max);
        limits.clamp_pan(*zoom, pan_uv);
        ui.ctx().request_repaint();
    }
    if *zoom > limits.min && (gamepad.pan.x.abs() > 0.05 || gamepad.pan.y.abs() > 0.05) {
        let visible_fraction = 1.0 / zoom.max(limits.min);
        let pan_speed = 1.35 * 0.2 * gamepad.frame_dt * visible_fraction;
        pan_uv.x += gamepad.pan.x * pan_speed;
        pan_uv.y += gamepad.pan.y * pan_speed;
        limits.clamp_pan(*zoom, pan_uv);
        ui.ctx().request_repaint();
    }
}
