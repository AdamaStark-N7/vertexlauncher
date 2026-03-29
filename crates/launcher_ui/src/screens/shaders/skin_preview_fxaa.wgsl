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
    let nw = load_rgba(p + vec2<i32>(-1, -1));
    let ne = load_rgba(p + vec2<i32>(1, -1));
    let sw = load_rgba(p + vec2<i32>(-1, 1));
    let se = load_rgba(p + vec2<i32>(1, 1));
    let m = load_rgba(p);

    let luma_nw = rgb_luma(nw.rgb);
    let luma_ne = rgb_luma(ne.rgb);
    let luma_sw = rgb_luma(sw.rgb);
    let luma_se = rgb_luma(se.rgb);
    let luma_m = rgb_luma(m.rgb);

    let luma_min = min(luma_m, min(min(luma_nw, luma_ne), min(luma_sw, luma_se)));
    let luma_max = max(luma_m, max(max(luma_nw, luma_ne), max(luma_sw, luma_se)));
    let luma_range = luma_max - luma_min;
    let threshold = max(1.0 / 16.0, luma_max * (1.0 / 8.0));
    if luma_range < threshold {
        return m;
    }

    var dir = vec2<f32>(
        -((luma_nw + luma_ne) - (luma_sw + luma_se)),
        (luma_nw + luma_sw) - (luma_ne + luma_se),
    );
    let dir_reduce = max(
        ((luma_nw + luma_ne + luma_sw + luma_se) * 0.25) * (1.0 / 8.0),
        1.0 / 128.0,
    );
    let rcp_dir_min = 1.0 / (min(abs(dir.x), abs(dir.y)) + dir_reduce);
    dir = clamp(dir * rcp_dir_min, vec2<f32>(-8.0), vec2<f32>(8.0));

    let fp = pos.xy;
    let rgb_a = 0.5 * (
        sample_linear(fp + dir * (1.0 / 3.0 - 0.5)).rgb +
        sample_linear(fp + dir * (2.0 / 3.0 - 0.5)).rgb
    );
    let rgb_b = rgb_a * 0.5 + 0.25 * (
        sample_linear(fp + dir * -0.5).rgb +
        sample_linear(fp + dir * 0.5).rgb
    );

    let luma_b = rgb_luma(rgb_b);
    var final_rgb = rgb_b;
    if luma_b < luma_min || luma_b > luma_max {
        final_rgb = rgb_a;
    }
    return vec4<f32>(final_rgb, m.a);
}
