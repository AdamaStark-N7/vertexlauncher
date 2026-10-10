use super::*;

pub(super) fn render_qr_code(ui: &mut egui::Ui, text: &str, size: f32) {
    use qrcode::QrCode;
    use qrcode::types::Color as QrColor;
    use resvg::tiny_skia::{self, Paint, Transform};

    // Cache by text content — only rasterise once per unique URL
    let cache_key = ui.make_persistent_id(("qr_fancy_v1", text));
    let cached: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(cache_key));

    let texture = if let Some(t) = cached {
        t
    } else {
        let Ok(code) = QrCode::new(text.as_bytes()) else {
            return;
        };
        let module_count = code.width();
        let colors = code.to_colors();

        let dark = |row: i32, col: i32| -> bool {
            if row < 0 || col < 0 || row >= module_count as i32 || col >= module_count as i32 {
                return false;
            }
            colors[row as usize * module_count + col as usize] == QrColor::Dark
        };

        let quiet = 2usize; // quiet zone in modules
        let total_modules = module_count + quiet * 2;
        // Supersample 4× then downsample with Lanczos3 for clean anti-aliasing
        let ss: u32 = 4;
        let module_px: u32 = 10; // target pixels per module in the final texture
        let img_px = total_modules as u32 * module_px;
        let ss_module_px = module_px * ss;
        let ss_img_px = img_px * ss;
        let corner_r = ss_module_px as f32 * 0.40;

        let Some(mut pixmap) = tiny_skia::Pixmap::new(ss_img_px, ss_img_px) else {
            return;
        };
        pixmap.fill(tiny_skia::Color::WHITE);

        let mut paint = Paint::default();
        paint.set_color_rgba8(26, 26, 46, 255); // near-black with slight blue tint
        paint.anti_alias = true;

        for row in 0..module_count {
            for col in 0..module_count {
                if !dark(row as i32, col as i32) {
                    continue;
                }

                let x = (col + quiet) as f32 * ss_module_px as f32;
                let y = (row + quiet) as f32 * ss_module_px as f32;
                let s = ss_module_px as f32;

                let row = row as i32;
                let col = col as i32;
                let top = dark(row - 1, col);
                let bot = dark(row + 1, col);
                let left = dark(row, col - 1);
                let right = dark(row, col + 1);

                // A corner is only rounded when NEITHER neighbour along that corner is dark,
                // i.e. it is a true outer convex corner of the merged shape.
                let r_nw = if top || left { 0.0 } else { corner_r };
                let r_ne = if top || right { 0.0 } else { corner_r };
                let r_sw = if bot || left { 0.0 } else { corner_r };
                let r_se = if bot || right { 0.0 } else { corner_r };

                if let Some(path) = qr_rounded_rect(x, y, s, s, r_nw, r_ne, r_sw, r_se) {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }

                // Concave (inner) corner rounding — carve a white arc into the dark shape
                // where two edge-neighbours are dark but their shared diagonal is light.
                // The white arc points INWARD (opposite quadrant to the notch direction).
                let cx = x;
                let cy = y;
                let cr = corner_r;
                if top && left && !dark(row - 1, col - 1) {
                    if let Some(p) = qr_concave_arc(cx, cy, cr, 0) {
                        pixmap.fill_path(
                            &p,
                            &paint,
                            tiny_skia::FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
                if top && right && !dark(row - 1, col + 1) {
                    if let Some(p) = qr_concave_arc(cx + s, cy, cr, 1) {
                        pixmap.fill_path(
                            &p,
                            &paint,
                            tiny_skia::FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
                if bot && left && !dark(row + 1, col - 1) {
                    if let Some(p) = qr_concave_arc(cx, cy + s, cr, 2) {
                        pixmap.fill_path(
                            &p,
                            &paint,
                            tiny_skia::FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
                if bot && right && !dark(row + 1, col + 1) {
                    if let Some(p) = qr_concave_arc(cx + s, cy + s, cr, 3) {
                        pixmap.fill_path(
                            &p,
                            &paint,
                            tiny_skia::FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }

        // Downsample the supersampled pixmap to the target resolution
        let downscaled = image::imageops::resize(
            &image::RgbaImage::from_raw(ss_img_px, ss_img_px, pixmap.data().to_vec())
                .unwrap_or_default(),
            img_px,
            img_px,
            image::imageops::FilterType::Lanczos3,
        );
        let color_image = egui::ColorImage::from_rgba_premultiplied(
            [img_px as usize, img_px as usize],
            downscaled.as_raw(),
        );
        let t = ui.ctx().load_texture(
            format!("qr_fancy_{}", text.len()),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        ui.ctx().data_mut(|d| d.insert_temp(cache_key, t.clone()));
        t
    };

    let resp = ui.add(
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(&texture))
            .fit_to_exact_size(egui::vec2(size, size))
            .corner_radius(egui::CornerRadius::same(8)),
    );
    // Border around the QR
    ui.painter().rect_stroke(
        resp.rect,
        egui::CornerRadius::same(8),
        egui::Stroke::new(1.5, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Outside,
    );
}

/// Builds a tiny_skia path for a rectangle with independent per-corner radii.
pub(super) fn qr_rounded_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r_nw: f32,
    r_ne: f32,
    r_sw: f32,
    r_se: f32,
) -> Option<resvg::tiny_skia::Path> {
    use resvg::tiny_skia::PathBuilder;
    // Cubic bezier approximation constant for a quarter-circle
    const K: f32 = 0.5523;
    let mut pb = PathBuilder::new();

    pb.move_to(x + r_nw, y);

    // top edge → NE corner
    pb.line_to(x + w - r_ne, y);
    if r_ne > 0.0 {
        pb.cubic_to(
            x + w - r_ne * (1.0 - K),
            y,
            x + w,
            y + r_ne * (1.0 - K),
            x + w,
            y + r_ne,
        );
    } else {
        pb.line_to(x + w, y);
    }

    // right edge → SE corner
    pb.line_to(x + w, y + h - r_se);
    if r_se > 0.0 {
        pb.cubic_to(
            x + w,
            y + h - r_se * (1.0 - K),
            x + w - r_se * (1.0 - K),
            y + h,
            x + w - r_se,
            y + h,
        );
    } else {
        pb.line_to(x + w, y + h);
    }

    // bottom edge → SW corner
    pb.line_to(x + r_sw, y + h);
    if r_sw > 0.0 {
        pb.cubic_to(
            x + r_sw * (1.0 - K),
            y + h,
            x,
            y + h - r_sw * (1.0 - K),
            x,
            y + h - r_sw,
        );
    } else {
        pb.line_to(x, y + h);
    }

    // left edge → NW corner
    pb.line_to(x, y + r_nw);
    if r_nw > 0.0 {
        pb.cubic_to(
            x,
            y + r_nw * (1.0 - K),
            x + r_nw * (1.0 - K),
            y,
            x + r_nw,
            y,
        );
    }

    pb.close();
    pb.finish()
}

/// Expands the dark shape into the white notch at an inner corner.
/// Each path walks out along the two dark boundary lines then arcs back toward
/// the corner — ctrl points bow the arc *inward* so the new boundary is concave
/// from outside (smooth, not bumpy).
/// `quadrant`: 0=NW, 1=NE, 2=SW, 3=SE — which direction the notch faces.
pub(super) fn qr_concave_arc(
    cx: f32,
    cy: f32,
    r: f32,
    quadrant: u8,
) -> Option<resvg::tiny_skia::Path> {
    use resvg::tiny_skia::PathBuilder;
    const K: f32 = 0.5523;
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy);
    match quadrant {
        0 => {
            // NW: up along boundary, arc left — ctrl points pull arc toward corner
            pb.line_to(cx, cy - r);
            pb.cubic_to(cx, cy - r + K * r, cx - r + K * r, cy, cx - r, cy);
        }
        1 => {
            // NE: up along boundary, arc right
            pb.line_to(cx, cy - r);
            pb.cubic_to(cx, cy - r + K * r, cx + r - K * r, cy, cx + r, cy);
        }
        2 => {
            // SW: down along boundary, arc left
            pb.line_to(cx, cy + r);
            pb.cubic_to(cx, cy + r - K * r, cx - r + K * r, cy, cx - r, cy);
        }
        _ => {
            // SE: down along boundary, arc right
            pb.line_to(cx, cy + r);
            pb.cubic_to(cx, cy + r - K * r, cx + r - K * r, cy, cx + r, cy);
        }
    }
    pb.close();
    pb.finish()
}
