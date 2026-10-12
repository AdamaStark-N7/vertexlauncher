@binding(0) @group(0) var source_tex_0 : texture_2d<f32>;

@binding(0) @group(1) var depth_tex_0 : texture_2d<f32>;

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

fn load_color_0( pixel_0 : vec2<i32>) -> vec4<f32>
{
    var dims_0 : vec2<u32>;
    var _S1 : u32 = dims_0[i32(0)];
    var _S2 : u32 = dims_0[i32(1)];
    {var dim = textureDimensions((source_tex_0));((_S1)) = dim.x;((_S2)) = dim.y;};
    dims_0[i32(0)] = _S1;
    dims_0[i32(1)] = _S2;
    var _S3 : vec3<i32> = vec3<i32>(clamp(pixel_0, vec2<i32>(i32(0), i32(0)), vec2<i32>(dims_0) - vec2<i32>(i32(1))), i32(0));
    return (textureLoad((source_tex_0), ((_S3)).xy, ((_S3)).z));
}

fn load_depth_0( pixel_1 : vec2<i32>) -> f32
{
    var dims_1 : vec2<u32>;
    var _S4 : u32 = dims_1[i32(0)];
    var _S5 : u32 = dims_1[i32(1)];
    {var dim = textureDimensions((depth_tex_0));((_S4)) = dim.x;((_S5)) = dim.y;};
    dims_1[i32(0)] = _S4;
    dims_1[i32(1)] = _S5;
    var _S6 : vec3<i32> = vec3<i32>(clamp(pixel_1, vec2<i32>(i32(0), i32(0)), vec2<i32>(dims_1) - vec2<i32>(i32(1))), i32(0));
    return (textureLoad((depth_tex_0), ((_S6)).xy, ((_S6)).z)).x;
}

struct pixelOutput_0
{
    @location(0) output_1 : vec4<f32>,
};

@fragment
fn fs_main(@builtin(position) pos_1 : vec4<f32>) -> pixelOutput_0
{
    var _S7 : vec2<i32> = vec2<i32>(pos_1.xy);
    var base_0 : vec4<f32> = load_color_0(_S7);
    var _S8 : f32 = load_depth_0(_S7);
    var y_0 : i32 = i32(-1);
    var occlusion_0 : f32 = 0.0f;
    for(;;)
    {
        if(y_0 <= i32(1))
        {
        }
        else
        {
            break;
        }
        var x_0 : i32 = i32(-1);
        var occlusion_1 : f32 = occlusion_0;
        for(;;)
        {
            if(x_0 <= i32(1))
            {
            }
            else
            {
                break;
            }
            var _S9 : bool;
            if(x_0 == i32(0))
            {
                _S9 = y_0 == i32(0);
            }
            else
            {
                _S9 = false;
            }
            if(_S9)
            {
                x_0 = x_0 + i32(1);
                continue;
            }
            occlusion_1 = occlusion_1 + smoothstep(0.00050000002374873f, 0.02500000037252903f, max(0.0f, _S8 - load_depth_0(_S7 + vec2<i32>(x_0, y_0))));
            x_0 = x_0 + i32(1);
        }
        y_0 = y_0 + i32(1);
        occlusion_0 = occlusion_1;
    }
    var _S10 : pixelOutput_0 = pixelOutput_0( vec4<f32>(base_0.xyz * vec3<f32>(clamp(1.0f - occlusion_0 / 8.0f * 0.55000001192092896f, 0.34999999403953552f, 1.0f)), base_0.w) );
    return _S10;
}

