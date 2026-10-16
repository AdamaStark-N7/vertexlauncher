//! Software rasteriser for the skin preview. The preview scene is a list of already-projected triangles
//! ([`RenderTriangle`]); this draws them with a depth buffer into an RGBA image, which the SE5 front end uploads
//! as an engine texture each frame. It follows the GPU path it replaces: nearest sampling clamped to each face's
//! texel region, optional texel-border anti-aliasing, supersampling for MSAA, an FXAA-style post pass, and
//! weighted accumulation for motion blur.

use super::*;

/// How a frame is drawn.
#[derive(Clone, Copy, Debug)]
pub struct RasterSettings {
    pub width: u32,
    pub height: u32,
    /// Samples per pixel along each axis (1 = none).
    pub supersample: u32,
    /// Smooth the hard edges between skin texels.
    pub texel_border_aa: bool,
    /// Edge-smoothing post pass.
    pub post_aa: bool,
}

impl RasterSettings {
    /// Settings for the configured anti-aliasing mode.
    pub fn for_modes(
        width: u32,
        height: u32,
        aa: SkinPreviewAaMode,
        msaa_samples: u32,
        texel: SkinPreviewTexelAaMode,
    ) -> Self {
        let (supersample, post_aa) = match aa {
            SkinPreviewAaMode::Off => (1, false),
            SkinPreviewAaMode::Msaa => ((msaa_samples.max(1) as f32).sqrt().ceil() as u32, false),
            SkinPreviewAaMode::Fxaa | SkinPreviewAaMode::Smaa | SkinPreviewAaMode::FxaaTaa => (1, true),
            SkinPreviewAaMode::Taa => (2, false),
        };
        Self {
            width: width.max(1),
            height: height.max(1),
            supersample: supersample.clamp(1, 4),
            texel_border_aa: texel == SkinPreviewTexelAaMode::TexelBoundary,
            post_aa,
        }
    }
}

/// Premultiplied linear-in-sRGB-space accumulation buffer.
struct Accumulator {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Accumulator {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![[0.0; 4]; width * height] }
    }
}

/// Draws weighted scenes (one scene with weight 1 when there is no motion blur) and returns a straight-alpha
/// RGBA image of `settings.width` x `settings.height`.
pub(crate) fn render_scenes(
    scenes: &[(f32, &[RenderTriangle])],
    skin: &RgbaImage,
    cape: Option<&RgbaImage>,
    settings: RasterSettings,
) -> RgbaImage {
    let ss = settings.supersample.max(1) as usize;
    let (out_w, out_h) = (settings.width as usize, settings.height as usize);
    let (w, h) = (out_w * ss, out_h * ss);
    let mut total = Accumulator::new(out_w, out_h);
    for (weight, triangles) in scenes {
        let hi = rasterise(triangles, skin, cape, w, h, ss as f32, settings.texel_border_aa);
        // Resolve the supersamples (premultiplied average) and add this scene's share, a band of rows per thread.
        let inv = 1.0 / (ss * ss) as f32;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 16);
        let rows = out_h.div_ceil(threads).max(1);
        let hi = &hi;
        std::thread::scope(|scope| {
            for (band, chunk) in total.pixels.chunks_mut(rows * out_w).enumerate() {
                scope.spawn(move || {
                    for (offset, dst) in chunk.iter_mut().enumerate() {
                        let (x, y) = (offset % out_w, band * rows + offset / out_w);
                        let mut sum = [0.0_f32; 4];
                        for sy in 0..ss {
                            for sx in 0..ss {
                                let px = hi.pixels[(y * ss + sy) * w + x * ss + sx];
                                for c in 0..4 {
                                    sum[c] += px[c];
                                }
                            }
                        }
                        for c in 0..4 {
                            dst[c] += sum[c] * inv * weight;
                        }
                    }
                });
            }
        });
    }
    let mut image = to_straight(&total);
    if settings.post_aa {
        image = fxaa(&image);
    }
    image
}

fn to_straight(acc: &Accumulator) -> RgbaImage {
    let mut image = RgbaImage::new(acc.width as u32, acc.height as u32);
    for (index, px) in acc.pixels.iter().enumerate() {
        let a = px[3].clamp(0.0, 1.0);
        let (r, g, b) = if a > 0.000_1 { (px[0] / a, px[1] / a, px[2] / a) } else { (0.0, 0.0, 0.0) };
        let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        image.put_pixel((index % acc.width) as u32, (index / acc.width) as u32, image::Rgba([to8(r), to8(g), to8(b), to8(a)]));
    }
    image
}

fn rasterise(
    triangles: &[RenderTriangle],
    skin: &RgbaImage,
    cape: Option<&RgbaImage>,
    w: usize,
    h: usize,
    scale: f32,
    texel_border_aa: bool,
) -> Accumulator {
    let mut color = Accumulator::new(w, h);
    let mut depth = vec![f32::INFINITY; w * h];
    // Horizontal bands, one thread each: every band draws every triangle clipped to its own rows.
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 16);
    let rows = h.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (band, (pixels, depths)) in color.pixels.chunks_mut(rows * w).zip(depth.chunks_mut(rows * w)).enumerate() {
            scope.spawn(move || {
                let band = Band { width: w, height: h, y0: band * rows, y1: (band * rows + pixels.len() / w).min(h) };
                for tri in triangles {
                    let texture = match tri.texture {
                        TriangleTexture::Skin => skin,
                        TriangleTexture::Cape => match cape {
                            Some(cape) => cape,
                            None => continue,
                        },
                    };
                    draw_triangle(&band, pixels, depths, tri, texture, scale, texel_border_aa);
                }
            });
        }
    });
    color
}

/// The rows `y0..y1` of a `width` x `height` target that one thread owns.
struct Band {
    width: usize,
    height: usize,
    y0: usize,
    y1: usize,
}

fn draw_triangle(
    band: &Band,
    pixels: &mut [[f32; 4]],
    depth: &mut [f32],
    tri: &RenderTriangle,
    texture: &RgbaImage,
    scale: f32,
    texel_border_aa: bool,
) {
    let p: [(f32, f32); 3] = std::array::from_fn(|i| (tri.pos[i].x * scale, tri.pos[i].y * scale));
    let z: [f32; 3] = std::array::from_fn(|i| tri.depth[i].max(SKIN_PREVIEW_NEAR + 0.000_1));
    let area = edge(p[0], p[1], p[2]);
    if area.abs() < 1e-8 {
        return;
    }
    let min_x = p.iter().map(|v| v.0).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
    let max_x = (p.iter().map(|v| v.0).fold(f32::NEG_INFINITY, f32::max).ceil().min(band.width as f32 - 1.0)).max(0.0) as usize;
    let min_y = (p.iter().map(|v| v.1).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize).max(band.y0);
    let max_y = ((p.iter().map(|v| v.1).fold(f32::NEG_INFINITY, f32::max).ceil().min(band.height as f32 - 1.0)).max(0.0) as usize).min(band.y1.saturating_sub(1));
    if band.y1 <= band.y0 || min_x > max_x || min_y > max_y {
        return;
    }
    let inv_z = [1.0 / z[0], 1.0 / z[1], 1.0 / z[2]];
    let dims = (texture.width() as f32, texture.height() as f32);
    let region = texel_region(tri.uv_bounds, dims);
    let tint = tri.color.to_normalized_gamma_f32();

    // Perspective-correct (u, v) and depth at a sample position, or None outside the triangle.
    let interpolate = |x: f32, y: f32, require_inside: bool| -> Option<([f32; 2], f32)> {
        let w0 = edge(p[1], p[2], (x, y)) / area;
        let w1 = edge(p[2], p[0], (x, y)) / area;
        let w2 = 1.0 - w0 - w1;
        const EPS: f32 = -1e-5;
        if require_inside && (w0 < EPS || w1 < EPS || w2 < EPS) {
            return None;
        }
        let q = [w0 * inv_z[0], w1 * inv_z[1], w2 * inv_z[2]];
        let sum = q[0] + q[1] + q[2];
        if sum <= 0.0 {
            return None;
        }
        let u = (q[0] * tri.uv[0].x + q[1] * tri.uv[1].x + q[2] * tri.uv[2].x) / sum;
        let v = (q[0] * tri.uv[0].y + q[1] * tri.uv[1].y + q[2] * tri.uv[2].y) / sum;
        Some(([u, v], 1.0 / sum))
    };

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let Some((uv, z_here)) = interpolate(cx, cy, true) else { continue };
            let index = (y - band.y0) * band.width + x;
            if z_here >= depth[index] {
                continue;
            }
            // Screen-space derivatives of the texel coordinate, for the anti-aliased sampling.
            let (dx, dy) = match (interpolate(cx + 1.0, cy, false), interpolate(cx, cy + 1.0, false)) {
                (Some((a, _)), Some((b, _))) => (
                    [(a[0] - uv[0]) * dims.0, (a[1] - uv[1]) * dims.1],
                    [(b[0] - uv[0]) * dims.0, (b[1] - uv[1]) * dims.1],
                ),
                _ => ([0.0; 2], [0.0; 2]),
            };
            let sampled = if texel_border_aa {
                sample_border_aa(texture, uv, dims, &region, dx, dy)
            } else {
                sample_pixel_art(texture, uv, dims, &region, dx, dy)
            };
            let a = sampled[3] * tint[3];
            if a <= 0.001 {
                continue;
            }
            let src = [sampled[0] * tint[0], sampled[1] * tint[1], sampled[2] * tint[2]];
            let dst = &mut pixels[index];
            if a >= 0.999 {
                *dst = [src[0], src[1], src[2], 1.0];
            } else {
                // Over, premultiplied.
                let keep = 1.0 - a;
                *dst = [src[0] * a + dst[0] * keep, src[1] * a + dst[1] * keep, src[2] * a + dst[2] * keep, a + dst[3] * keep];
            }
            depth[index] = z_here;
        }
    }
}

fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)
}

/// The texel rectangle `[min, max]` (inclusive) a face owns, snapped from its inset UV bounds.
struct TexelRegion {
    min: (i32, i32),
    max: (i32, i32),
}

fn texel_region(bounds: Rect, dims: (f32, f32)) -> TexelRegion {
    let min = ((bounds.min.x * dims.0 + 0.01).floor() as i32, (bounds.min.y * dims.1 + 0.01).floor() as i32);
    let max = (
        ((bounds.max.x * dims.0 - 0.01).ceil() as i32 - 1).max(min.0),
        ((bounds.max.y * dims.1 - 0.01).ceil() as i32 - 1).max(min.1),
    );
    TexelRegion { min, max }
}

fn load(texture: &RgbaImage, texel: (i32, i32), region: &TexelRegion) -> [f32; 4] {
    let x = texel.0.clamp(region.min.0, region.max.0).clamp(0, texture.width() as i32 - 1);
    let y = texel.1.clamp(region.min.1, region.max.1).clamp(0, texture.height() as i32 - 1);
    let px = texture.get_pixel(x as u32, y as u32).0;
    [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0, px[3] as f32 / 255.0]
}

fn premultiply(c: [f32; 4]) -> [f32; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Nearest texel, blending toward a filtered sample once a pixel covers more than about one texel.
fn sample_pixel_art(
    texture: &RgbaImage,
    uv: [f32; 2],
    dims: (f32, f32),
    region: &TexelRegion,
    dx: [f32; 2],
    dy: [f32; 2],
) -> [f32; 4] {
    let footprint = dx[0].abs().max(dx[1].abs()).max(dy[0].abs()).max(dy[1].abs());
    let nearest = load(texture, ((uv[0] * dims.0).floor() as i32, (uv[1] * dims.1).floor() as i32), region);
    let mix = smoothstep(0.85, 1.35, footprint);
    if mix <= 0.0 {
        return nearest;
    }
    // Bilinear at the clamped centre, standing in for the GPU's mip-filtered fetch.
    let lo = (region.min.0 as f32 + 0.5, region.min.1 as f32 + 0.5);
    let hi = (region.max.0 as f32 + 0.5, region.max.1 as f32 + 0.5);
    let p = (
        (uv[0] * dims.0).clamp(lo.0, hi.0) - 0.5,
        (uv[1] * dims.1).clamp(lo.1, hi.1) - 0.5,
    );
    let filtered = bilinear(texture, p, region);
    std::array::from_fn(|i| nearest[i] + (filtered[i] - nearest[i]) * mix)
}

fn bilinear(texture: &RgbaImage, p: (f32, f32), region: &TexelRegion) -> [f32; 4] {
    let base = (p.0.floor() as i32, p.1.floor() as i32);
    let f = (p.0 - base.0 as f32, p.1 - base.1 as f32);
    let c00 = premultiply(load(texture, base, region));
    let c10 = premultiply(load(texture, (base.0 + 1, base.1), region));
    let c01 = premultiply(load(texture, (base.0, base.1 + 1), region));
    let c11 = premultiply(load(texture, (base.0 + 1, base.1 + 1), region));
    let blended: [f32; 4] = std::array::from_fn(|i| {
        let top = c00[i] + (c10[i] - c00[i]) * f.0;
        let bottom = c01[i] + (c11[i] - c01[i]) * f.0;
        top + (bottom - top) * f.1
    });
    unpremultiply(blended)
}

fn unpremultiply(c: [f32; 4]) -> [f32; 4] {
    if c[3] <= 0.0001 { [0.0; 4] } else { [c[0] / c[3], c[1] / c[3], c[2] / c[3], c[3]] }
}

/// Smooths the seams between texels: a bilinear fetch whose blend zone is as wide as a screen pixel, done in
/// premultiplied space so transparent texels (stored black) don't darken silhouettes.
fn sample_border_aa(
    texture: &RgbaImage,
    uv: [f32; 2],
    dims: (f32, f32),
    region: &TexelRegion,
    dx: [f32; 2],
    dy: [f32; 2],
) -> [f32; 4] {
    let p = (uv[0] * dims.0 - 0.5, uv[1] * dims.1 - 0.5);
    let base = (p.0.floor() as i32, p.1.floor() as i32);
    let f = (p.0 - base.0 as f32, p.1 - base.1 as f32);
    let width = |a: f32, b: f32| ((a.abs() + b.abs()).max(0.0001) * 0.75).clamp(0.0, 0.5);
    let (ew, eh) = (width(dx[0], dy[0]), width(dx[1], dy[1]));
    let t = (smoothstep(0.5 - ew, 0.5 + ew, f.0), smoothstep(0.5 - eh, 0.5 + eh, f.1));
    let c00 = premultiply(load(texture, base, region));
    let c10 = premultiply(load(texture, (base.0 + 1, base.1), region));
    let c01 = premultiply(load(texture, (base.0, base.1 + 1), region));
    let c11 = premultiply(load(texture, (base.0 + 1, base.1 + 1), region));
    let blended: [f32; 4] = std::array::from_fn(|i| {
        let top = c00[i] + (c10[i] - c00[i]) * t.0;
        let bottom = c01[i] + (c11[i] - c01[i]) * t.0;
        top + (bottom - top) * t.1
    });
    unpremultiply(blended)
}

/// A compact FXAA: where luma contrast crosses a threshold, blend toward the neighbours across the edge.
fn fxaa(image: &RgbaImage) -> RgbaImage {
    let (w, h) = (image.width() as i32, image.height() as i32);
    let mut out = image.clone();
    let px = |x: i32, y: i32| -> [f32; 4] {
        let p = image.get_pixel(x.clamp(0, w - 1) as u32, y.clamp(0, h - 1) as u32).0;
        let a = p[3] as f32 / 255.0;
        [p[0] as f32 / 255.0 * a, p[1] as f32 / 255.0 * a, p[2] as f32 / 255.0 * a, a]
    };
    let luma = |c: [f32; 4]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2] + c[3];
    for y in 0..h {
        for x in 0..w {
            let m = px(x, y);
            let (n, s, e, west) = (px(x, y - 1), px(x, y + 1), px(x + 1, y), px(x - 1, y));
            let ls = [luma(n), luma(s), luma(e), luma(west)];
            let lm = luma(m);
            let hi = ls.iter().cloned().fold(lm, f32::max);
            let lo = ls.iter().cloned().fold(lm, f32::min);
            let range = hi - lo;
            if range < (0.0312_f32).max(hi * 0.125) {
                continue;
            }
            let avg: [f32; 4] = std::array::from_fn(|i| (n[i] + s[i] + e[i] + west[i]) * 0.25);
            let blend = ((range / hi.max(0.0001)) * 0.75).clamp(0.0, 0.75);
            let mixed: [f32; 4] = std::array::from_fn(|i| m[i] + (avg[i] - m[i]) * blend);
            let [r, g, b, a] = unpremultiply(mixed);
            let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out.put_pixel(x as u32, y as u32, image::Rgba([to8(r), to8(g), to8(b), to8(a)]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_skin(color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(64, 64, image::Rgba(color))
    }

    fn quad(depth: f32, color: Color32) -> Vec<RenderTriangle> {
        let uv_bounds = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let make = |a: Pos2, b: Pos2, c: Pos2| RenderTriangle {
            texture: TriangleTexture::Skin,
            pos: [a, b, c],
            uv: [pos2(0.25, 0.25); 3],
            uv_bounds,
            depth: [depth; 3],
            color,
        };
        vec![
            make(pos2(2.0, 2.0), pos2(14.0, 2.0), pos2(14.0, 14.0)),
            make(pos2(2.0, 2.0), pos2(14.0, 14.0), pos2(2.0, 14.0)),
        ]
    }

    #[test]
    fn nearer_triangles_win_regardless_of_draw_order() {
        let skin = solid_skin([255, 255, 255, 255]);
        let mut scene = quad(6.0, Color32::from_rgb(255, 0, 0));
        scene.extend(quad(3.0, Color32::from_rgb(0, 255, 0)));
        let settings = RasterSettings { width: 16, height: 16, supersample: 1, texel_border_aa: false, post_aa: false };
        let image = render_scenes(&[(1.0, &scene)], &skin, None, settings);
        assert_eq!(image.get_pixel(8, 8).0, [0, 255, 0, 255]);
        assert_eq!(image.get_pixel(0, 0).0[3], 0, "outside the quad stays transparent");
    }

    #[test]
    fn transparent_texels_are_not_drawn() {
        let skin = solid_skin([10, 20, 30, 0]);
        let scene = quad(5.0, Color32::WHITE);
        let settings = RasterSettings { width: 16, height: 16, supersample: 2, texel_border_aa: true, post_aa: true };
        let image = render_scenes(&[(1.0, &scene)], &skin, None, settings);
        assert!(image.pixels().all(|p| p.0[3] == 0));
    }

    #[test]
    fn motion_blur_weights_blend_scenes() {
        let skin = solid_skin([255, 255, 255, 255]);
        let red = quad(5.0, Color32::from_rgb(255, 0, 0));
        let blue = quad(5.0, Color32::from_rgb(0, 0, 255));
        let settings = RasterSettings { width: 16, height: 16, supersample: 1, texel_border_aa: false, post_aa: false };
        let image = render_scenes(&[(0.5, &red), (0.5, &blue)], &skin, None, settings);
        let p = image.get_pixel(8, 8).0;
        assert_eq!(p[3], 255);
        assert!((120..=135).contains(&p[0]) && (120..=135).contains(&p[2]) && p[1] == 0, "{p:?}");
    }
}
