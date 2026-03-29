struct Scalar {
    value: vec4<f32>,
};

struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
};

@group(0) @binding(0)
var current_tex: texture_2d<f32>;
@group(1) @binding(0)
var history_tex: texture_2d<f32>;
@group(2) @binding(0)
var<uniform> scalar: Scalar;

fn load_current(pixel: vec2<i32>) -> vec4<f32> {
    let dims = textureDimensions(current_tex);
    let clamped = clamp(pixel, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    return textureLoad(current_tex, clamped, 0);
}

fn load_history(pixel: vec2<i32>) -> vec4<f32> {
    let dims = textureDimensions(history_tex);
    let clamped = clamp(pixel, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    return textureLoad(history_tex, clamped, 0);
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
    let pixel = vec2<i32>(pos.xy);
    let current = load_current(pixel);
    var lo = current;
    var hi = current;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let sample = load_current(pixel + vec2<i32>(x, y));
            lo = min(lo, sample);
            hi = max(hi, sample);
        }
    }
    let history = clamp(load_history(pixel), lo, hi);
    let current_weight = clamp(scalar.value.x, 0.05, 1.0);
    return mix(history, current, current_weight);
}
