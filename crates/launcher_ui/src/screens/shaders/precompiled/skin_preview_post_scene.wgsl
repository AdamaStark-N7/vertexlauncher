struct Globals_std140_0
{
    @align(16) screen_size_points_0 : vec2<f32>,
    @align(8) _pad_0 : vec2<f32>,
};

@binding(0) @group(1) var<uniform> globals_0 : Globals_std140_0;
struct Scalars_std140_0
{
    @align(16) value_0 : vec4<f32>,
};

@binding(0) @group(2) var<uniform> scalars_0 : Scalars_std140_0;
@binding(0) @group(0) var preview_tex_0 : texture_2d<f32>;

@binding(1) @group(0) var preview_sampler_0 : sampler;

struct VertexOut_0
{
    @builtin(position) pos_0 : vec4<f32>,
    @location(0) uv_0 : vec2<f32>,
    @location(2) color_0 : vec4<f32>,
    @interpolate(flat) @location(1) uv_bounds_0 : vec4<f32>,
};

struct vertexInput_0
{
    @location(0) pos_points_0 : vec2<f32>,
    @location(1) camera_z_0 : f32,
    @location(2) uv_1 : vec2<f32>,
    @location(3) color_1 : vec4<f32>,
    @location(4) uv_bounds_1 : vec4<f32>,
};

@vertex
fn vs_main( _S1 : vertexInput_0) -> VertexOut_0
{
    var _S2 : f32 = max(_S1.camera_z_0, 1.50010001659393311f);
    var output_0 : VertexOut_0;
    output_0.pos_0 = vec4<f32>((_S1.pos_points_0.x / globals_0.screen_size_points_0.x * 2.0f - 1.0f) * _S2, (1.0f - _S1.pos_points_0.y / globals_0.screen_size_points_0.y * 2.0f) * _S2, _S2 - 1.5f, _S2);
    output_0.uv_0 = _S1.uv_1;
    output_0.color_0 = _S1.color_1;
    output_0.uv_bounds_0 = _S1.uv_bounds_1;
    return output_0;
}

struct TexelRegion_0
{
     min_texel_0 : vec2<i32>,
     max_texel_0 : vec2<i32>,
};

fn texel_region_0( uv_bounds_2 : vec4<f32>,  dims_0 : vec2<f32>) -> TexelRegion_0
{
    var region_0 : TexelRegion_0;
    var _S3 : vec2<f32> = vec2<f32>(0.00999999977648258f);
    var _S4 : vec2<i32> = vec2<i32>(floor(uv_bounds_2.xy * dims_0 + _S3));
    region_0.min_texel_0 = _S4;
    region_0.max_texel_0 = max(vec2<i32>(ceil(uv_bounds_2.zw * dims_0 - _S3)) - vec2<i32>(i32(1)), _S4);
    return region_0;
}

fn load_region_texel_0( texel_0 : vec2<i32>,  region_1 : TexelRegion_0) -> vec4<f32>
{
    var _S5 : vec3<i32> = vec3<i32>(clamp(texel_0, region_1.min_texel_0, region_1.max_texel_0), i32(0));
    return (textureLoad((preview_tex_0), ((_S5)).xy, ((_S5)).z));
}

fn premultiply_0( color_2 : vec4<f32>) -> vec4<f32>
{
    var _S6 : f32 = color_2.w;
    return vec4<f32>(color_2.xyz * vec3<f32>(_S6), _S6);
}

fn sample_preview_texel_border_aa_0( uv_2 : vec2<f32>,  uv_bounds_3 : vec4<f32>) -> vec4<f32>
{
    var dims_i_0 : vec2<u32>;
    var _S7 : u32 = dims_i_0[i32(0)];
    var _S8 : u32 = dims_i_0[i32(1)];
    {var dim = textureDimensions((preview_tex_0));((_S7)) = dim.x;((_S8)) = dim.y;};
    dims_i_0[i32(0)] = _S7;
    dims_i_0[i32(1)] = _S8;
    var _S9 : vec2<f32> = vec2<f32>(dims_i_0);
    var region_2 : TexelRegion_0 = texel_region_0(uv_bounds_3, _S9);
    var _S10 : vec2<f32> = vec2<f32>(0.5f);
    var p_0 : vec2<f32> = uv_2 * _S9 - _S10;
    var _S11 : vec2<i32> = vec2<i32>(floor(p_0));
    var edge_width_0 : vec2<f32> = clamp(max((fwidth((p_0))), vec2<f32>(0.00009999999747379f)) * vec2<f32>(0.75f), vec2<f32>(0.0f), _S10);
    var t_0 : vec2<f32> = smoothstep(_S10 - edge_width_0, _S10 + edge_width_0, fract(p_0));
    var _S12 : vec4<f32> = vec4<f32>(t_0.x);
    var blended_0 : vec4<f32> = mix(mix(premultiply_0(load_region_texel_0(_S11, region_2)), premultiply_0(load_region_texel_0(_S11 + vec2<i32>(i32(1), i32(0)), region_2)), _S12), mix(premultiply_0(load_region_texel_0(_S11 + vec2<i32>(i32(0), i32(1)), region_2)), premultiply_0(load_region_texel_0(_S11 + vec2<i32>(i32(1), i32(1)), region_2)), _S12), vec4<f32>(t_0.y));
    var _S13 : f32 = blended_0.w;
    if(_S13 <= 0.00009999999747379f)
    {
        return vec4<f32>(0.0f, 0.0f, 0.0f, 0.0f);
    }
    return vec4<f32>(blended_0.xyz / vec3<f32>(_S13), _S13);
}

fn sample_preview_pixel_art_0( uv_3 : vec2<f32>,  uv_bounds_4 : vec4<f32>) -> vec4<f32>
{
    var dims_i_1 : vec2<u32>;
    var _S14 : u32 = dims_i_1[i32(0)];
    var _S15 : u32 = dims_i_1[i32(1)];
    {var dim = textureDimensions((preview_tex_0));((_S14)) = dim.x;((_S15)) = dim.y;};
    dims_i_1[i32(0)] = _S14;
    dims_i_1[i32(1)] = _S15;
    var _S16 : vec2<f32> = vec2<f32>(dims_i_1);
    var region_3 : TexelRegion_0 = texel_region_0(uv_bounds_4, _S16);
    var uv_grad_x_0 : vec2<f32> = dpdx(uv_3);
    var uv_grad_y_0 : vec2<f32> = dpdy(uv_3);
    var texel_grad_x_0 : vec2<f32> = uv_grad_x_0 * _S16;
    var texel_grad_y_0 : vec2<f32> = uv_grad_y_0 * _S16;
    var _S17 : vec2<f32> = vec2<f32>(0.5f);
    return mix(load_region_texel_0(vec2<i32>(floor(uv_3 * _S16)), region_3), (textureSampleGrad((preview_tex_0), (preview_sampler_0), (clamp(uv_3, (vec2<f32>(region_3.min_texel_0) + _S17) / _S16, (vec2<f32>(region_3.max_texel_0) + _S17) / _S16)), (uv_grad_x_0), (uv_grad_y_0))), vec4<f32>(smoothstep(0.85000002384185791f, 1.35000002384185791f, max(max(abs(texel_grad_x_0.x), abs(texel_grad_x_0.y)), max(abs(texel_grad_y_0.x), abs(texel_grad_y_0.y))))));
}

struct FragOut_0
{
    @location(0) color_3 : vec4<f32>,
    @location(1) depth_linear_0 : vec4<f32>,
};

struct pixelInput_0
{
    @location(0) uv_4 : vec2<f32>,
    @location(2) color_4 : vec4<f32>,
    @interpolate(flat) @location(1) uv_bounds_5 : vec4<f32>,
};

@fragment
fn fs_main( _S18 : pixelInput_0, @builtin(position) pos_1 : vec4<f32>) -> FragOut_0
{
    var _S19 : vec4<f32>;
    if((scalars_0.value_0.x) > 0.5f)
    {
        _S19 = sample_preview_texel_border_aa_0(_S18.uv_4, _S18.uv_bounds_5);
    }
    else
    {
        _S19 = sample_preview_pixel_art_0(_S18.uv_4, _S18.uv_bounds_5);
    }
    var sampled_0 : vec4<f32> = _S19 * _S18.color_4;
    if((sampled_0.w) <= 0.00100000004749745f)
    {
        discard;
    }
    var output_1 : FragOut_0;
    output_1.color_3 = sampled_0;
    output_1.depth_linear_0 = vec4<f32>(pos_1.z, 0.0f, 0.0f, 1.0f);
    return output_1;
}

