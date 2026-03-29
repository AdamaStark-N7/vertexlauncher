struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
};

@group(0) @binding(0)
var source_tex: texture_2d<f32>;

fn rgb_luma(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}

fn load_rgba(pixel: vec2<i32>) -> vec4<f32> {
    let dims = textureDimensions(source_tex);
    let clamped = clamp(pixel, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    return textureLoad(source_tex, clamped, 0);
}

fn sample_linear(pixel: vec2<f32>) -> vec4<f32> {
    let dims = textureDimensions(source_tex);
    let max_pixel = vec2<f32>(vec2<i32>(dims) - vec2<i32>(1));
    let p = clamp(pixel, vec2<f32>(0.0), max_pixel);
    let p0 = vec2<i32>(floor(p));
    let p1 = min(p0 + vec2<i32>(1), vec2<i32>(dims) - vec2<i32>(1));
    let f = fract(p);
    let c00 = load_rgba(p0);
    let c10 = load_rgba(vec2<i32>(p1.x, p0.y));
    let c01 = load_rgba(vec2<i32>(p0.x, p1.y));
    let c11 = load_rgba(p1);
    let top = mix(c00, c10, f.x);
    let bottom = mix(c01, c11, f.x);
    return mix(top, bottom, f.y);
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vertex_index: u32) -> FullscreenOut {
    var out: FullscreenOut;
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let c = load_rgba(p);
    let l = load_rgba(p + vec2<i32>(-1, 0));
    let r = load_rgba(p + vec2<i32>(1, 0));
    let t = load_rgba(p + vec2<i32>(0, -1));
    let b = load_rgba(p + vec2<i32>(0, 1));

    let luma_c = rgb_luma(c.rgb);
    let luma_l = rgb_luma(l.rgb);
    let luma_r = rgb_luma(r.rgb);
    let luma_t = rgb_luma(t.rgb);
    let luma_b = rgb_luma(b.rgb);

    let edge_h = max(abs(luma_l - luma_c), abs(luma_r - luma_c));
    let edge_v = max(abs(luma_t - luma_c), abs(luma_b - luma_c));
    let threshold = max(0.04, luma_c * 0.12);

    if max(edge_h, edge_v) < threshold {
        return c;
    }

    let center = pos.xy;
    if edge_h >= edge_v {
        let a = sample_linear(center + vec2<f32>(-0.75, 0.0));
        let b = sample_linear(center + vec2<f32>(0.75, 0.0));
        let long_a = sample_linear(center + vec2<f32>(-1.5, 0.0));
        let long_b = sample_linear(center + vec2<f32>(1.5, 0.0));
        let blend = clamp((edge_h - threshold) * 6.0, 0.0, 1.0);
        let neighbor = mix(0.5 * (a + b), 0.5 * (long_a + long_b), 0.35);
        return mix(c, vec4<f32>(neighbor.rgb, c.a), blend * 0.75);
    }

    let sample_a = sample_linear(center + vec2<f32>(0.0, -0.75));
    let sample_b = sample_linear(center + vec2<f32>(0.0, 0.75));
    let long_sample_a = sample_linear(center + vec2<f32>(0.0, -1.5));
    let long_sample_b = sample_linear(center + vec2<f32>(0.0, 1.5));
    let blend = clamp((edge_v - threshold) * 6.0, 0.0, 1.0);
    let neighbor = mix(
        0.5 * (sample_a + sample_b),
        0.5 * (long_sample_a + long_sample_b),
        0.35,
    );
    return mix(c, vec4<f32>(neighbor.rgb, c.a), blend * 0.75);
}
