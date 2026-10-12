@binding(0) @group(0) var source_tex_0 : texture_2d<f32>;

struct FullscreenOut_0
{
    @builtin(position) pos_0 : vec4<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vertex_index_0 : u32) -> FullscreenOut_0
{
    var output_0 : FullscreenOut_0;
    output_0.pos_0 = vec4<f32>(f32((((vertex_index_0 << (u32(1)))) & (u32(2)))) * 2.0f - 1.0f, 1.0f - f32((vertex_index_0 & (u32(2)))) * 2.0f, 0.0f, 1.0f);
    return output_0;
}

fn lin_slope_0() -> f32
{
    return 0.90495049953460693f;
}

fn hermite_seg_0( t_0 : f32,  p0_0 : f32,  m0_0 : f32,  p1_0 : f32,  m1_0 : f32) -> f32
{
    var t2_0 : f32 = t_0 * t_0;
    var t3_0 : f32 = t2_0 * t_0;
    var _S1 : f32 = 3.0f * t2_0;
    return p0_0 * (2.0f * t3_0 - _S1 + 1.0f) + m0_0 * (t3_0 - 2.0f * t2_0 + t_0) + p1_0 * (-2.0f * t3_0 + _S1) + m1_0 * (t3_0 - t2_0);
}

fn tonemap_channel_0( x_0 : f32) -> f32
{
    var _S2 : f32 = max(x_0, 0.0f);
    var m_0 : f32 = lin_slope_0();
    if(_S2 < 0.03999999910593033f)
    {
        return hermite_seg_0(_S2 / 0.03999999910593033f, 0.0f, 0.0f, 0.03599999845027924f, m_0 * 0.03999999910593033f);
    }
    else
    {
        if(_S2 < 1.04999995231628418f)
        {
            return 0.03599999845027924f + (_S2 - 0.03999999910593033f) * m_0;
        }
        else
        {
            return clamp(hermite_seg_0(min((_S2 - 1.04999995231628418f) / 0.95000004768371582f, 1.0f), 0.94999998807907104f, m_0 * 0.95000004768371582f, 1.0f, 0.0f), 0.0f, 1.0f);
        }
    }
}

fn tonemap_0( rgb_0 : vec3<f32>) -> vec3<f32>
{
    return vec3<f32>(tonemap_channel_0(rgb_0.x), tonemap_channel_0(rgb_0.y), tonemap_channel_0(rgb_0.z));
}

fn linear_to_srgb_channel_0( c_0 : f32) -> f32
{
    if(c_0 <= 0.00313080009073019f)
    {
        return c_0 * 12.92000007629394531f;
    }
    return 1.0549999475479126f * pow(c_0, 0.4166666567325592f) - 0.05499999970197678f;
}

fn linear_to_srgb_0( rgb_1 : vec3<f32>) -> vec3<f32>
{
    return vec3<f32>(linear_to_srgb_channel_0(rgb_1.x), linear_to_srgb_channel_0(rgb_1.y), linear_to_srgb_channel_0(rgb_1.z));
}

struct pixelOutput_0
{
    @location(0) output_1 : vec4<f32>,
};

@fragment
fn fs_main(@builtin(position) pos_1 : vec4<f32>) -> pixelOutput_0
{
    var dims_0 : vec2<u32>;
    var _S3 : u32 = dims_0[i32(0)];
    var _S4 : u32 = dims_0[i32(1)];
    {var dim = textureDimensions((source_tex_0));((_S3)) = dim.x;((_S4)) = dim.y;};
    dims_0[i32(0)] = _S3;
    dims_0[i32(1)] = _S4;
    var _S5 : vec3<i32> = vec3<i32>(clamp(vec2<i32>(pos_1.xy), vec2<i32>(i32(0), i32(0)), vec2<i32>(dims_0) - vec2<i32>(i32(1))), i32(0));
    var src_0 : vec4<f32> = (textureLoad((source_tex_0), ((_S5)).xy, ((_S5)).z));
    var _S6 : pixelOutput_0 = pixelOutput_0( vec4<f32>(linear_to_srgb_0(tonemap_0(src_0.xyz)), src_0.w) );
    return _S6;
}

