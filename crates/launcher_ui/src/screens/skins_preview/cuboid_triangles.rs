use super::*;

/// Adds a lit cuboid whose top-center sits at `local_top_center` in `transform`'s space.
///
/// This function does not panic.
pub(in super::super) fn add_transformed_cuboid_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    transform: ModelTransform,
    size: Vec3,
    local_top_center: Vec3,
    uv: FaceUvs,
    cull_backfaces: bool,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    light_dir: Vec3,
) {
    let w = size.x;
    let h = size.y;
    let d = size.z;
    let x0 = -w * 0.5;
    let x1 = w * 0.5;
    let y0 = 0.0;
    let y1 = -h;
    let z0 = -d * 0.5;
    let z1 = d * 0.5;

    let faces = [
        (
            [
                Vec3::new(x0, y0, z1),
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x0, y1, z1),
            ],
            uv.front,
            Vec3::new(0.0, 0.0, 1.0),
        ),
        (
            [
                Vec3::new(x1, y0, z0),
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y1, z0),
                Vec3::new(x1, y1, z0),
            ],
            uv.back,
            Vec3::new(0.0, 0.0, -1.0),
        ),
        (
            [
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y0, z1),
                Vec3::new(x0, y1, z1),
                Vec3::new(x0, y1, z0),
            ],
            uv.left,
            Vec3::new(-1.0, 0.0, 0.0),
        ),
        (
            [
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y1, z0),
                Vec3::new(x1, y1, z1),
            ],
            uv.right,
            Vec3::new(1.0, 0.0, 0.0),
        ),
        (
            [
                Vec3::new(x0, y0, z0),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y0, z1),
                Vec3::new(x0, y0, z1),
            ],
            uv.top,
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            [
                Vec3::new(x0, y1, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x1, y1, z0),
                Vec3::new(x0, y1, z0),
            ],
            uv.bottom,
            Vec3::new(0.0, -1.0, 0.0),
        ),
    ];

    for (quad, uv_rect, normal) in faces {
        add_transformed_quad_triangles(
            out,
            texture,
            transform,
            quad.map(|vertex| vertex + local_top_center),
            normal,
            uv_rect,
            cull_backfaces,
            camera,
            projection,
            rect,
            light_dir,
        );
    }
}

/// A box built like Minecraft's `ModelPart` cubes: `texOffs(u, v).addBox(origin, size)` with
/// a `CubeDeformation` and optional `mirror()`, in Minecraft model space (y down, front -z).
#[derive(Clone, Copy)]
pub(crate) struct MinecraftBox {
    pub(crate) origin: Vec3,
    pub(crate) size: Vec3,
    pub(crate) inflate: f32,
    pub(crate) tex_offset: [f32; 2],
    /// The model's texture size (UV space), independent of the image resolution.
    pub(crate) texture_size: [f32; 2],
    pub(crate) mirror: bool,
}

/// Adds a [`MinecraftBox`] using the same per-face vertex/UV assignment as
/// `ModelPart.Cube`. `transform` maps Minecraft model space into preview space.
///
/// This function does not panic.
pub(in super::super) fn add_minecraft_box_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    transform: ModelTransform,
    cube: MinecraftBox,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    light_dir: Vec3,
) {
    let (w, h, d) = (cube.size.x, cube.size.y, cube.size.z);
    let [u, v] = cube.tex_offset;
    let (u0, u1, u2, u3, u4, u5) = (
        u,
        u + d,
        u + d + w,
        u + d + w + w,
        u + d + w + d,
        u + d + w + d + w,
    );
    let (v0, v1, v2) = (v, v + d, v + d + h);

    let mut min_x = cube.origin.x - cube.inflate;
    let mut max_x = cube.origin.x + w + cube.inflate;
    if cube.mirror {
        std::mem::swap(&mut min_x, &mut max_x);
    }
    let min_y = cube.origin.y - cube.inflate;
    let max_y = cube.origin.y + h + cube.inflate;
    let min_z = cube.origin.z - cube.inflate;
    let max_z = cube.origin.z + d + cube.inflate;
    let c = [
        Vec3::new(min_x, min_y, min_z),
        Vec3::new(max_x, min_y, min_z),
        Vec3::new(max_x, max_y, min_z),
        Vec3::new(min_x, max_y, min_z),
        Vec3::new(min_x, min_y, max_z),
        Vec3::new(max_x, min_y, max_z),
        Vec3::new(max_x, max_y, max_z),
        Vec3::new(min_x, max_y, max_z),
    ];
    let mirror_x = if cube.mirror { -1.0 } else { 1.0 };
    // (vertices, u_a, v_a, u_b, v_b, normal) in `ModelPart.Polygon` order: vertex 0 gets
    // (u_b, v_a), 1 gets (u_a, v_a), 2 gets (u_a, v_b), 3 gets (u_b, v_b).
    let faces = [
        (
            [c[5], c[4], c[0], c[1]],
            u1,
            v0,
            u2,
            v1,
            Vec3::new(0.0, -1.0, 0.0),
        ),
        (
            [c[2], c[3], c[7], c[6]],
            u2,
            v1,
            u3,
            v0,
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            [c[0], c[4], c[7], c[3]],
            u0,
            v1,
            u1,
            v2,
            Vec3::new(-mirror_x, 0.0, 0.0),
        ),
        (
            [c[1], c[0], c[3], c[2]],
            u1,
            v1,
            u2,
            v2,
            Vec3::new(0.0, 0.0, -1.0),
        ),
        (
            [c[5], c[1], c[2], c[6]],
            u2,
            v1,
            u4,
            v2,
            Vec3::new(mirror_x, 0.0, 0.0),
        ),
        (
            [c[4], c[5], c[6], c[7]],
            u4,
            v1,
            u5,
            v2,
            Vec3::new(0.0, 0.0, 1.0),
        ),
    ];
    let [tex_w, tex_h] = cube.texture_size;
    for (vertices, u_a, v_a, u_b, v_b, normal) in faces {
        add_transformed_quad_triangles(
            out,
            texture,
            transform,
            [vertices[1], vertices[0], vertices[3], vertices[2]],
            normal,
            Rect::from_min_max(
                egui::pos2(u_a / tex_w, v_a / tex_h),
                egui::pos2(u_b / tex_w, v_b / tex_h),
            ),
            false,
            camera,
            projection,
            rect,
            light_dir,
        );
    }
}

/// Adds one lit, textured quad. `quad` is wound top-left, top-right, bottom-right,
/// bottom-left as seen from the side `normal` points to, matching `uv_rect`'s corners.
///
/// This function does not panic.
pub(in super::super) fn add_transformed_quad_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    transform: ModelTransform,
    quad: [Vec3; 4],
    normal: Vec3,
    uv_rect: Rect,
    cull_backfaces: bool,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    light_dir: Vec3,
) {
    let world_normal = transform.transform_vector(normal).normalized();
    let brightness = 0.58 + world_normal.dot(light_dir).max(0.0) * 0.42;
    let tint = color_with_brightness(Color32::WHITE, brightness);

    let transformed = quad.map(|vertex| transform.transform_point(vertex));
    let camera_vertices = transformed.map(|v| camera.world_to_camera(v));
    if camera_vertices.iter().any(|v| v.z <= projection.near) {
        return;
    }
    if cull_backfaces {
        let normal_camera = Vec3::new(
            world_normal.dot(camera.right),
            world_normal.dot(camera.up),
            world_normal.dot(camera.forward),
        );
        let center_camera =
            (camera_vertices[0] + camera_vertices[1] + camera_vertices[2] + camera_vertices[3])
                * 0.25;
        let to_camera = (Vec3::new(0.0, 0.0, 0.0) - center_camera).normalized();
        if normal_camera.dot(to_camera) <= 0.0 {
            return;
        }
    }
    let projected = camera_vertices.map(|v| project_point(v, projection, rect));
    if projected.iter().any(Option::is_none) {
        return;
    }
    let projected = projected.map(Option::unwrap);

    let uv0 = uv_rect.left_top();
    let uv1 = uv_rect.right_top();
    let uv2 = uv_rect.right_bottom();
    let uv3 = uv_rect.left_bottom();
    // `uv_rect` may be mirrored (min > max), so normalize it for the sampling bounds.
    let uv_bounds = Rect::from_two_pos(uv_rect.min, uv_rect.max);
    out.push(RenderTriangle {
        texture,
        pos: [projected[0], projected[1], projected[2]],
        uv: [uv0, uv1, uv2],
        uv_bounds,
        depth: [
            camera_vertices[0].z,
            camera_vertices[1].z,
            camera_vertices[2].z,
        ],
        color: tint,
    });
    out.push(RenderTriangle {
        texture,
        pos: [projected[0], projected[2], projected[3]],
        uv: [uv0, uv2, uv3],
        uv_bounds,
        depth: [
            camera_vertices[0].z,
            camera_vertices[2].z,
            camera_vertices[3].z,
        ],
        color: tint,
    });
}

fn project_point(camera_space: Vec3, projection: Projection, rect: Rect) -> Option<Pos2> {
    if camera_space.z <= projection.near {
        return None;
    }

    let aspect = (rect.width() / rect.height().max(1.0)).max(0.01);
    let tan_half_fov = (projection.fov_y_radians * 0.5).tan().max(0.01);
    let x_ndc = camera_space.x / (camera_space.z * tan_half_fov * aspect);
    let y_ndc = camera_space.y / (camera_space.z * tan_half_fov);
    let x = rect.center().x + x_ndc * (rect.width() * 0.5);
    let y = rect.center().y - y_ndc * (rect.height() * 0.5);
    Some(Pos2::new(x, y))
}

fn color_with_brightness(base: Color32, brightness: f32) -> Color32 {
    let b = brightness.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        ((base.r() as f32) * b).round() as u8,
        ((base.g() as f32) * b).round() as u8,
        ((base.b() as f32) * b).round() as u8,
        base.a(),
    )
}
