struct Scalar {
    value: vec4<f32>,
};

struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
};

@group(0) @binding(0)
var source_tex: texture_2d<f32>;
@group(1) @binding(0)
var<uniform> scalar: Scalar;

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
    let dims = textureDimensions(source_tex);
    let pixel = clamp(vec2<i32>(pos.xy), vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    return textureLoad(source_tex, pixel, 0) * scalar.value.x;
}
