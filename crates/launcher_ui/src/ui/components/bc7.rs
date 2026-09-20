//! CPU BC7 encoder (mode 6) with mip generation, for GPU-compressed image textures.
//!
//! BC7 stores 4x4 pixel blocks in 16 bytes (1 byte/pixel, a 4x saving over RGBA8) and is far more
//! accurate than BC1 (the older 0.5 byte/pixel format): mode 6 gives every block two 8-bit-precision endpoints and 16 interpolation
//! steps between them, so smooth gradients and dark scenes don't band. Only opaque colour is
//! encoded (alpha is written as 255), which suits screenshots and photo-like thumbnails.

/// Blocks are 4x4 pixels.
const BLOCK: usize = 4;
/// Bytes per compressed block.
const BLOCK_BYTES: usize = 16;
/// Stop generating mips once the smaller side would drop below one block.
const MIN_MIP_SIDE: usize = BLOCK;

#[derive(Debug)]
pub struct Bc7Mip {
    /// Mip size in pixels (before rounding up to whole blocks).
    pub width: u32,
    pub height: u32,
    pub blocks_x: u32,
    pub blocks_y: u32,
    /// `blocks_x * blocks_y` 16-byte blocks, row-major.
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Bc7Image {
    /// Original image size.
    pub width: u32,
    pub height: u32,
    /// Base level size rounded up to a multiple of 4 (what the GPU texture is created with).
    pub padded_width: u32,
    pub padded_height: u32,
    pub mips: Vec<Bc7Mip>,
}

impl Bc7Image {
    pub fn byte_size(&self) -> usize {
        self.mips.iter().map(|mip| mip.data.len()).sum()
    }

    /// Fraction of the padded texture that holds real image data, per axis. Multiply UVs by this.
    pub fn uv_scale(&self) -> [f32; 2] {
        [
            self.width as f32 / self.padded_width as f32,
            self.height as f32 / self.padded_height as f32,
        ]
    }
}

/// One rectangle of a larger image, compressed independently.
#[derive(Debug)]
pub struct Bc7Tile {
    /// Top-left of the tile in the source image, in pixels.
    pub x: u32,
    pub y: u32,
    pub image: Bc7Image,
}

/// Splits an image into tiles of at most `tile_max` pixels per side (rounded down to a multiple of
/// 4) and compresses each. An image that already fits yields a single tile.
pub fn compress_rgba_tiled(
    rgba: &[u8],
    width: usize,
    height: usize,
    tile_max: usize,
) -> Vec<Bc7Tile> {
    let tile = (tile_max / BLOCK * BLOCK).max(BLOCK);
    if width <= tile && height <= tile {
        return vec![Bc7Tile {
            x: 0,
            y: 0,
            image: compress_rgba(rgba, width, height),
        }];
    }
    let mut tiles = Vec::new();
    for y in (0..height).step_by(tile) {
        let th = tile.min(height - y);
        for x in (0..width).step_by(tile) {
            let tw = tile.min(width - x);
            let mut sub = Vec::with_capacity(tw * th * 4);
            for row in y..y + th {
                let start = (row * width + x) * 4;
                sub.extend_from_slice(&rgba[start..start + tw * 4]);
            }
            tiles.push(Bc7Tile {
                x: x as u32,
                y: y as u32,
                image: compress_rgba(&sub, tw, th),
            });
        }
    }
    tiles
}

/// Compresses tightly packed RGBA8 (`width * height * 4` bytes) into a BC7 mip chain.
pub fn compress_rgba(rgba: &[u8], width: usize, height: usize) -> Bc7Image {
    assert!(width > 0 && height > 0 && rgba.len() >= width * height * 4);
    let padded_width = width.next_multiple_of(BLOCK);
    let padded_height = height.next_multiple_of(BLOCK);

    // Level 0, edge-replicated out to whole blocks so partial blocks don't bleed in black.
    let mut level = pad_rgb(rgba, width, height, padded_width, padded_height);
    let (mut level_w, mut level_h) = (padded_width, padded_height);

    let mut mips = Vec::new();
    loop {
        mips.push(encode_level(&level, level_w, level_h));
        let (next_w, next_h) = ((level_w / 2).max(1), (level_h / 2).max(1));
        if next_w.min(next_h) < MIN_MIP_SIDE {
            break;
        }
        level = downsample_rgb(&level, level_w, level_h, next_w, next_h);
        level_w = next_w;
        level_h = next_h;
    }

    Bc7Image {
        width: width as u32,
        height: height as u32,
        padded_width: padded_width as u32,
        padded_height: padded_height as u32,
        mips,
    }
}

/// RGBA -> tightly packed RGB, replicating the last row/column into the padding.
fn pad_rgb(rgba: &[u8], width: usize, height: usize, padded_w: usize, padded_h: usize) -> Vec<u8> {
    let mut out = vec![0u8; padded_w * padded_h * 3];
    for y in 0..padded_h {
        let src_y = y.min(height - 1);
        for x in 0..padded_w {
            let src_x = x.min(width - 1);
            let src = (src_y * width + src_x) * 4;
            let dst = (y * padded_w + x) * 3;
            out[dst..dst + 3].copy_from_slice(&rgba[src..src + 3]);
        }
    }
    out
}

/// 2x2 box filter to `(next_w, next_h)`, clamping at odd edges.
fn downsample_rgb(src: &[u8], w: usize, h: usize, next_w: usize, next_h: usize) -> Vec<u8> {
    let mut out = vec![0u8; next_w * next_h * 3];
    for y in 0..next_h {
        let y0 = (y * 2).min(h - 1);
        let y1 = (y * 2 + 1).min(h - 1);
        for x in 0..next_w {
            let x0 = (x * 2).min(w - 1);
            let x1 = (x * 2 + 1).min(w - 1);
            for c in 0..3 {
                let sum = src[(y0 * w + x0) * 3 + c] as u32
                    + src[(y0 * w + x1) * 3 + c] as u32
                    + src[(y1 * w + x0) * 3 + c] as u32
                    + src[(y1 * w + x1) * 3 + c] as u32;
                out[(y * next_w + x) * 3 + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    out
}

fn encode_level(rgb: &[u8], w: usize, h: usize) -> Bc7Mip {
    let blocks_x = w.div_ceil(BLOCK);
    let blocks_y = h.div_ceil(BLOCK);
    let mut data = Vec::with_capacity(blocks_x * blocks_y * BLOCK_BYTES);
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let mut block = [[0u8; 3]; 16];
            for py in 0..BLOCK {
                let y = (by * BLOCK + py).min(h - 1);
                for px in 0..BLOCK {
                    let x = (bx * BLOCK + px).min(w - 1);
                    let src = (y * w + x) * 3;
                    block[py * BLOCK + px] = [rgb[src], rgb[src + 1], rgb[src + 2]];
                }
            }
            data.extend_from_slice(&encode_block(&block));
        }
    }
    Bc7Mip {
        width: w as u32,
        height: h as u32,
        blocks_x: blocks_x as u32,
        blocks_y: blocks_y as u32,
        data,
    }
}

/// BC7's 4-bit index interpolation weights (out of 64).
const WEIGHTS: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

/// A 7-bit endpoint channel as decoded: 7 bits plus the shared p-bit, which we fix to 1 so that
/// alpha (which shares it) decodes to exactly 255.
fn expand(q: u8) -> i32 {
    i32::from(q) * 2 + 1
}

fn quantize_channel(v: f32) -> u8 {
    (((v - 1.0) / 2.0).round().clamp(0.0, 127.0)) as u8
}

fn quantize(color: [f32; 3]) -> [u8; 3] {
    [
        quantize_channel(color[0]),
        quantize_channel(color[1]),
        quantize_channel(color[2]),
    ]
}

fn palette(e0: [u8; 3], e1: [u8; 3]) -> [[i32; 3]; 16] {
    let mut out = [[0; 3]; 16];
    for (color, w) in out.iter_mut().zip(WEIGHTS) {
        for c in 0..3 {
            color[c] = (expand(e0[c]) * (64 - w) + expand(e1[c]) * w + 32) >> 6;
        }
    }
    out
}

fn dist(px: &[u8; 3], color: &[i32; 3]) -> i64 {
    let mut total = 0i64;
    for c in 0..3 {
        let d = i64::from(px[c]) - i64::from(color[c]);
        total += d * d;
    }
    total
}

/// Best index per pixel for the endpoints, and the total squared error.
fn assign(block: &[[u8; 3]; 16], e0: [u8; 3], e1: [u8; 3]) -> ([u8; 16], i64) {
    let pal = palette(e0, e1);
    let a = [
        expand(e0[0]) as f32,
        expand(e0[1]) as f32,
        expand(e0[2]) as f32,
    ];
    let d = [
        expand(e1[0]) as f32 - a[0],
        expand(e1[1]) as f32 - a[1],
        expand(e1[2]) as f32 - a[2],
    ];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let mut indices = [0u8; 16];
    let mut total = 0i64;
    for (i, px) in block.iter().enumerate() {
        // Project onto the endpoint line for a starting guess, then check the neighbours (the
        // weights aren't perfectly linear).
        let guess = if len2 > 0.0 {
            let t = ((f32::from(px[0]) - a[0]) * d[0]
                + (f32::from(px[1]) - a[1]) * d[1]
                + (f32::from(px[2]) - a[2]) * d[2])
                / len2;
            (t * 15.0).round().clamp(0.0, 15.0) as usize
        } else {
            0
        };
        let (mut best, mut best_dist) = (guess, dist(px, &pal[guess]));
        for cand in [guess.saturating_sub(1), (guess + 1).min(15)] {
            let dd = dist(px, &pal[cand]);
            if dd < best_dist {
                best = cand;
                best_dist = dd;
            }
        }
        indices[i] = best as u8;
        total += best_dist;
    }
    (indices, total)
}

/// Endpoints along the block's principal colour axis (its direction of greatest variation).
fn principal_axis_endpoints(block: &[[u8; 3]; 16]) -> ([f32; 3], [f32; 3]) {
    let mut mean = [0f32; 3];
    for px in block {
        for c in 0..3 {
            mean[c] += f32::from(px[c]) / 16.0;
        }
    }
    let mut cov = [[0f32; 3]; 3];
    for px in block {
        let d = [
            f32::from(px[0]) - mean[0],
            f32::from(px[1]) - mean[1],
            f32::from(px[2]) - mean[2],
        ];
        for a in 0..3 {
            for b in 0..3 {
                cov[a][b] += d[a] * d[b];
            }
        }
    }
    let mut axis = [0.577_35f32; 3];
    for _ in 0..6 {
        let next = [
            cov[0][0] * axis[0] + cov[0][1] * axis[1] + cov[0][2] * axis[2],
            cov[1][0] * axis[0] + cov[1][1] * axis[1] + cov[1][2] * axis[2],
            cov[2][0] * axis[0] + cov[2][1] * axis[1] + cov[2][2] * axis[2],
        ];
        let len = (next[0] * next[0] + next[1] * next[1] + next[2] * next[2]).sqrt();
        if len < 1e-6 {
            break;
        }
        axis = [next[0] / len, next[1] / len, next[2] / len];
    }
    let (mut t_min, mut t_max) = (f32::MAX, f32::MIN);
    for px in block {
        let t = (f32::from(px[0]) - mean[0]) * axis[0]
            + (f32::from(px[1]) - mean[1]) * axis[1]
            + (f32::from(px[2]) - mean[2]) * axis[2];
        t_min = t_min.min(t);
        t_max = t_max.max(t);
    }
    let at = |t: f32| {
        [
            mean[0] + axis[0] * t,
            mean[1] + axis[1] * t,
            mean[2] + axis[2] * t,
        ]
    };
    (at(t_min), at(t_max))
}

/// Endpoints minimising squared error for fixed index assignments (least squares per channel).
fn refine_endpoints(block: &[[u8; 3]; 16], indices: &[u8; 16]) -> Option<([f32; 3], [f32; 3])> {
    let (mut aa, mut bb, mut ab) = (0f32, 0f32, 0f32);
    let mut ax = [0f32; 3];
    let mut bx = [0f32; 3];
    for (px, &idx) in block.iter().zip(indices) {
        let t = WEIGHTS[idx as usize] as f32 / 64.0; // weight of endpoint 1
        let s = 1.0 - t; // weight of endpoint 0
        aa += s * s;
        bb += t * t;
        ab += s * t;
        for c in 0..3 {
            ax[c] += s * f32::from(px[c]);
            bx[c] += t * f32::from(px[c]);
        }
    }
    let det = aa * bb - ab * ab;
    if det.abs() < 1e-4 {
        return None;
    }
    let mut e0 = [0f32; 3];
    let mut e1 = [0f32; 3];
    for c in 0..3 {
        e0[c] = (ax[c] * bb - bx[c] * ab) / det;
        e1[c] = (bx[c] * aa - ax[c] * ab) / det;
    }
    Some((e0, e1))
}

fn encode_block(block: &[[u8; 3]; 16]) -> [u8; BLOCK_BYTES] {
    let (mut lo, mut hi) = principal_axis_endpoints(block);

    let mut best: Option<(i64, [u8; 3], [u8; 3], [u8; 16])> = None;
    for _ in 0..4 {
        let (q0, q1) = (quantize(lo), quantize(hi));
        let (indices, error) = assign(block, q0, q1);
        if best.as_ref().is_some_and(|(e, ..)| error >= *e) {
            break;
        }
        best = Some((error, q0, q1, indices));
        if error == 0 {
            break;
        }
        match refine_endpoints(block, &indices) {
            Some((e0, e1)) => {
                lo = e0;
                hi = e1;
            }
            None => break,
        }
    }
    let (_, mut q0, mut q1, mut indices) = best.unwrap_or((0, [0; 3], [0; 3], [0; 16]));

    // The first pixel's index has an implicit top bit of 0, so it must be below 8: swap the
    // endpoints (and mirror every index) when it isn't.
    if indices[0] >= 8 {
        std::mem::swap(&mut q0, &mut q1);
        for idx in &mut indices {
            *idx = 15 - *idx;
        }
    }

    // Mode 6, bit 0 first: mode (0b1000000), R0 R1 G0 G1 B0 B1 A0 A1 (7 bits each), P0, P1,
    // then 16 indices (4 bits, the first only 3).
    let mut bits: u128 = 0;
    let mut pos = 0u32;
    let mut put = |value: u32, count: u32| {
        bits |= u128::from(value) << pos;
        pos += count;
    };
    put(1 << 6, 7);
    for c in 0..3 {
        put(u32::from(q0[c]), 7);
        put(u32::from(q1[c]), 7);
    }
    put(127, 7); // A0 (255 with the p-bit)
    put(127, 7); // A1
    put(1, 1); // P0
    put(1, 1); // P1
    for (i, idx) in indices.iter().enumerate() {
        put(u32::from(*idx), if i == 0 { 3 } else { 4 });
    }
    debug_assert_eq!(pos, 128);
    bits.to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes a mode 6 block to RGB (test-only reference decoder).
    fn decode_block(block: &[u8]) -> [[i32; 3]; 16] {
        let bits = u128::from_le_bytes(block.try_into().unwrap());
        let mut pos = 0u32;
        let mut take = |count: u32| -> i32 {
            let v = ((bits >> pos) & ((1u128 << count) - 1)) as i32;
            pos += count;
            v
        };
        assert_eq!(take(7), 1 << 6, "not a mode 6 block");
        let mut e0 = [0i32; 3];
        let mut e1 = [0i32; 3];
        for c in 0..3 {
            e0[c] = take(7);
            e1[c] = take(7);
        }
        let (a0, a1) = (take(7), take(7));
        let (p0, p1) = (take(1), take(1));
        assert_eq!(
            (a0 << 1 | p0, a1 << 1 | p1),
            (255, 255),
            "alpha must be opaque"
        );
        for c in 0..3 {
            e0[c] = e0[c] << 1 | p0;
            e1[c] = e1[c] << 1 | p1;
        }
        let mut out = [[0; 3]; 16];
        for (i, px) in out.iter_mut().enumerate() {
            let idx = take(if i == 0 { 3 } else { 4 }) as usize;
            let w = WEIGHTS[idx];
            for c in 0..3 {
                px[c] = (e0[c] * (64 - w) + e1[c] * w + 32) >> 6;
            }
        }
        out
    }

    #[test]
    fn flat_color_round_trips_almost_exactly() {
        let rgba: Vec<u8> = (0..8 * 8).flat_map(|_| [200, 100, 50, 255]).collect();
        let image = compress_rgba(&rgba, 8, 8);
        for px in decode_block(&image.mips[0].data[0..BLOCK_BYTES]) {
            assert!(
                (px[0] - 200).abs() <= 1 && (px[1] - 100).abs() <= 1 && (px[2] - 50).abs() <= 1,
                "{px:?}"
            );
        }
    }

    #[test]
    fn gradient_block_error_is_tiny() {
        let mut rgba = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                let v = (x * 16 + y * 4) as u8;
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let image = compress_rgba(&rgba, 4, 4);
        let decoded = decode_block(&image.mips[0].data[0..BLOCK_BYTES]);
        for (i, px) in decoded.iter().enumerate() {
            let expected = ((i % 4) * 16 + (i / 4) * 4) as i32;
            assert!(
                (px[0] - expected).abs() <= 3,
                "pixel {i}: {px:?} vs {expected}"
            );
        }
    }

    #[test]
    fn dark_gradients_do_not_band() {
        // A slow ramp through very dark colours, like a cave screenshot.
        let (w, h) = (64usize, 4usize);
        let mut rgba = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                let v = (10 + x / 4) as u8;
                rgba.extend_from_slice(&[v, v / 2, v / 3, 255]);
            }
        }
        let image = compress_rgba(&rgba, w, h);
        let mip = &image.mips[0];
        for bx in 0..mip.blocks_x as usize {
            let offset = bx * BLOCK_BYTES;
            let decoded = decode_block(&mip.data[offset..offset + BLOCK_BYTES]);
            for (i, px) in decoded.iter().enumerate() {
                let src = ((i / 4) * w + bx * 4 + i % 4) * 4;
                for c in 0..3 {
                    assert!(
                        (px[c] - i32::from(rgba[src + c])).abs() <= 2,
                        "block {bx} px {i}"
                    );
                }
            }
        }
    }

    /// PSNR of a compressed `w`x`h` test image against its source.
    fn round_trip_psnr(w: usize, h: usize, pixel: impl Fn(usize, usize) -> [i32; 3]) -> f64 {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = pixel(x, y);
                rgba.extend_from_slice(&[
                    r.clamp(0, 255) as u8,
                    g.clamp(0, 255) as u8,
                    b.clamp(0, 255) as u8,
                    255,
                ]);
            }
        }
        let image = compress_rgba(&rgba, w, h);
        let mip = &image.mips[0];
        let mut squared_error = 0f64;
        for by in 0..mip.blocks_y as usize {
            for bx in 0..mip.blocks_x as usize {
                let offset = (by * mip.blocks_x as usize + bx) * BLOCK_BYTES;
                let decoded = decode_block(&mip.data[offset..offset + BLOCK_BYTES]);
                for (i, px) in decoded.iter().enumerate() {
                    let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                    let src = (y * w + x) * 4;
                    for c in 0..3 {
                        let d = f64::from(px[c] - i32::from(rgba[src + c]));
                        squared_error += d * d;
                    }
                }
            }
        }
        let mse = squared_error / (w * h * 3) as f64;
        10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
    }

    #[test]
    fn smooth_shading_is_nearly_lossless() {
        // Lighting on a tinted surface: brightness varies in 2D but colour stays on one line.
        let psnr = round_trip_psnr(64, 64, |x, y| {
            let v = (x * 2 + y * 2) as f32;
            [v as i32, (v * 0.7) as i32, (v * 0.4) as i32]
        });
        assert!(psnr > 50.0, "PSNR {psnr:.1} dB is too low");
    }

    #[test]
    fn noisy_detail_stays_above_the_single_line_floor() {
        // Independent per-channel noise can't lie on one colour line, which is the limit of a
        // single-subset mode; this checks we stay close to it.
        let psnr = round_trip_psnr(64, 64, |x, y| {
            let detail = ((x * 7 + y * 13) % 9) as i32 - 4;
            [
                (x * 4) as i32 + detail,
                (y * 4) as i32 - detail,
                ((x + y) * 2) as i32,
            ]
        });
        assert!(psnr > 37.0, "PSNR {psnr:.1} dB is too low");
    }

    #[test]
    fn tiling_covers_the_image_exactly_once() {
        let (w, h) = (70usize, 40usize);
        let rgba = vec![90u8; w * h * 4];
        let tiles = compress_rgba_tiled(&rgba, w, h, 32);
        // 3 columns (32, 32, 6) x 2 rows (32, 8).
        assert_eq!(tiles.len(), 6);
        let area: u32 = tiles.iter().map(|t| t.image.width * t.image.height).sum();
        assert_eq!(area as usize, w * h);
        assert_eq!(compress_rgba_tiled(&rgba, w, h, 4096).len(), 1);
    }

    #[test]
    fn sizes_padding_and_mip_chain_are_consistent() {
        let (w, h) = (30usize, 17usize);
        let rgba = vec![128u8; w * h * 4];
        let image = compress_rgba(&rgba, w, h);
        assert_eq!((image.padded_width, image.padded_height), (32, 20));
        assert_eq!(image.uv_scale(), [30.0 / 32.0, 17.0 / 20.0]);
        assert_eq!((image.mips[0].blocks_x, image.mips[0].blocks_y), (8, 5));
        for mip in &image.mips {
            assert_eq!(
                mip.data.len(),
                mip.blocks_x as usize * mip.blocks_y as usize * BLOCK_BYTES
            );
        }
        // 32x20 -> 16x10 -> 8x5 -> stops before a side drops under 4.
        assert_eq!(image.mips.len(), 3);
        assert!(image.byte_size() < w * h * 4 / 2);
    }
}
