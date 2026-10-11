use super::*;

/// Adds the cape cuboid. `cape_transform` is the cape's hinge (top-center of its inner
/// face) in preview space, already swung by the current pose.
pub(in super::super::super) fn add_cape_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    cape_transform: ModelTransform,
    cape_uv: FaceUvs,
    light_dir: Vec3,
) {
    add_transformed_cuboid_triangles(
        out,
        texture,
        cape_transform,
        Vec3::new(10.0, 16.0, 1.0),
        Vec3::new(0.0, 0.0, -0.55),
        cape_uv,
        true,
        camera,
        projection,
        rect,
        light_dir,
    );
}
