use super::*;

/// Procedural elytra motion layered on top of the vanilla standing pose, as Minecraft
/// model-space rotations (y down, front -z).
#[derive(Clone, Copy)]
pub(in super::super::super) struct ElytraMotion {
    pub(in super::super::super) wings_rx: f32,
    pub(in super::super::super) wing_ry: f32,
    pub(in super::super::super) right_wing_rz: f32,
    pub(in super::super::super) left_wing_rz: f32,
}

/// Adds both elytra wings attached to `body_transform` (the body part's pivot at the neck),
/// built like vanilla `ElytraModel`: `texOffs(22, 0)` 10×20×2 boxes with a 1.0 cube
/// inflation on a 64×32 texture, the right wing mirrored.
pub(in super::super::super) fn add_elytra_triangles(
    out: &mut Vec<RenderTriangle>,
    texture: TriangleTexture,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    body_transform: ModelTransform,
    motion: ElytraMotion,
    light_dir: Vec3,
) {
    let neutral_pose = VANILLA_ELYTRA_POSE_STANDING;
    let _ = VANILLA_ELYTRA_POSE_SNEAKING;
    let _ = VANILLA_ELYTRA_POSE_GLIDE_OPEN;
    // The wings layer sits 2px behind the body pivot.
    let wings = body_transform
        .then(ModelTransform::from_pivot_rotation(
            Vec3::new(0.0, 0.0, -2.0),
            motion.wings_rx,
            0.0,
            0.0,
        ))
        .then(ModelTransform::minecraft_model_axes());

    let wing_parts = [
        (neutral_pose.left, 5.0, motion.left_wing_rz, -10.0, false),
        (neutral_pose.right, -5.0, motion.right_wing_rz, 0.0, true),
    ];
    for (wing_pose, pivot_x, roll, box_x, mirror) in wing_parts {
        let wing = wings.then(ModelTransform::from_pivot_rotation(
            Vec3::new(pivot_x, wing_pose.pivot_y_offset, 0.0),
            wing_pose.rotate_x,
            wing_pose.rotate_y + motion.wing_ry,
            wing_pose.rotate_z + roll,
        ));
        add_minecraft_box_triangles(
            out,
            texture,
            wing,
            MinecraftBox {
                origin: Vec3::new(box_x, 0.0, 0.0),
                size: Vec3::new(10.0, 20.0, 2.0),
                inflate: 1.0,
                tex_offset: [22.0, 0.0],
                texture_size: [64.0, 32.0],
                mirror,
            },
            camera,
            projection,
            rect,
            light_dir,
        );
    }
}
