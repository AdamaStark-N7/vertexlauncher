struct VertexIn {
    @location(0) pos_points: vec2<f32>,
    @location(1) camera_z: f32,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct Globals {
    screen_size_points: vec2<f32>,
    _pad: vec2<f32>,
};

struct Scalars {
    value: vec4<f32>,
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0)
var preview_tex: texture_2d<f32>;
@group(0) @binding(1)
var preview_sampler: sampler;
@group(1) @binding(0)
var<uniform> globals: Globals;
@group(2) @binding(0)
var<uniform> scalars: Scalars;

fn sample_preview_pixel_art(uv: vec2<f32>) -> vec4<f32> {
    let dims_i = textureDimensions(preview_tex);
    let dims = vec2<f32>(dims_i);
    let texel = 0.5 / dims;
    let clamped_uv = clamp(uv, texel, vec2<f32>(1.0) - texel);
    let uv_grad_x = dpdx(clamped_uv);
    let uv_grad_y = dpdy(clamped_uv);
    let texel_grad_x = uv_grad_x * dims;
    let texel_grad_y = uv_grad_y * dims;
    let footprint = max(
        max(abs(texel_grad_x.x), abs(texel_grad_x.y)),
        max(abs(texel_grad_y.x), abs(texel_grad_y.y)),
    );
    let pixel = clamp(
        vec2<i32>(clamped_uv * dims),
        vec2<i32>(0),
        vec2<i32>(dims_i) - vec2<i32>(1),
    );
    let nearest = textureLoad(preview_tex, pixel, 0);
    let filtered = textureSampleGrad(
        preview_tex,
        preview_sampler,
        clamped_uv,
        uv_grad_x,
        uv_grad_y,
    );
    let filtered_mix = smoothstep(0.85, 1.35, footprint);
    return mix(nearest, filtered, filtered_mix);
}

fn load_preview_texel(texel: vec2<i32>, dims_i: vec2<u32>) -> vec4<f32> {
    let max_texel = vec2<i32>(dims_i) - vec2<i32>(1);
    return textureLoad(preview_tex, clamp(texel, vec2<i32>(0), max_texel), 0);
}

fn sample_preview_texel_border_aa(uv: vec2<f32>) -> vec4<f32> {
    let dims_i = textureDimensions(preview_tex);
    let dims = vec2<f32>(dims_i);
    let texel = 0.5 / dims;
    let clamped_uv = clamp(uv, texel, vec2<f32>(1.0) - texel);
    let p = clamped_uv * dims - vec2<f32>(0.5);
    let base = vec2<i32>(floor(p));
    let f = fract(p);
    let edge_width = clamp(max(fwidth(p), vec2<f32>(0.0001)) * 0.75, vec2<f32>(0.0), vec2<f32>(0.5));
    let t = smoothstep(vec2<f32>(0.5) - edge_width, vec2<f32>(0.5) + edge_width, f);
    let c00 = load_preview_texel(base, dims_i);
    let c10 = load_preview_texel(base + vec2<i32>(1, 0), dims_i);
    let c01 = load_preview_texel(base + vec2<i32>(0, 1), dims_i);
    let c11 = load_preview_texel(base + vec2<i32>(1, 1), dims_i);
    let cx0 = mix(c00, c10, t.x);
    let cx1 = mix(c01, c11, t.x);
    return mix(cx0, cx1, t.y);
}

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let x_ndc = (input.pos_points.x / globals.screen_size_points.x) * 2.0 - 1.0;
    let y_ndc = 1.0 - (input.pos_points.y / globals.screen_size_points.y) * 2.0;
    let z_cam = max(input.camera_z, 1.5 + 0.0001);
    let clip_w = z_cam;
    let clip_z = z_cam - 1.5;
    out.pos = vec4<f32>(x_ndc * clip_w, y_ndc * clip_w, clip_z, clip_w);
    out.uv = input.uv;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let sampled = select(
        sample_preview_pixel_art(input.uv),
        sample_preview_texel_border_aa(input.uv),
        scalars.value.x > 0.5,
    ) * input.color;
    if sampled.a <= 0.001 {
        discard;
    }
    return sampled;
}
