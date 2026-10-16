//! Animated eyes, brows, and lashes for expression skins: layered face panels for the
//! detected eye/brow layout, driven by blink, glance, squint, and brow timing.

use super::*;
use std::f32::consts::{PI, TAU};

#[path = "skins_preview_expressions/face_rig.rs"]
mod face_rig;

use self::face_rig::*;

#[derive(Clone, Copy)]
pub(super) struct FaceAnimationInput {
    pub(super) age_ticks: f32,
    pub(super) limb_speed: f32,
    /// Stable per-player random value in `0.0..1.0`.
    pub(super) entity_random: f32,
    /// Camera distance, used to push the panel layers apart at range.
    pub(super) camera_distance_blocks: f32,
}

/// Face animation values for one frame. The preview has no head look
/// input (`head_yaw`/`head_pitch` are zero) and the player is never hurt.
#[derive(Clone, Copy)]
pub(super) struct FaceExpressionPose {
    /// Level-of-detail factor that spreads the panel layers apart in depth.
    lod: f32,
    eye_top: f32,
    right_pupil_x: f32,
    left_pupil_x: f32,
    pupil_y: f32,
    blink: f32,
    ender_eye_top: f32,
    ender_right_pupil_x: f32,
    ender_left_pupil_x: f32,
    ender_pupil_y: f32,
    ender_blink: f32,
    right_brow_ty: f32,
    left_brow_ty: f32,
    villager_brow_ty: f32,
}

pub(super) fn compute_face_pose(input: FaceAnimationInput) -> FaceExpressionPose {
    let random = input.entity_random;
    let id = TAU * random;
    let age = input.age_ticks;
    let limb_speed = input.limb_speed;
    let breath = age / (11.0 - random * 2.0);

    // Occasional idle glance and pupil wander, both suppressed while walking.
    let glance_window = mc_clamp(-5.6 + (-PI / 6.0 + id + age / 80.0).cos() * 6.0, 0.0, 1.0)
        * mc_clamp(2.0 - limb_speed * 4.0, 0.0, 1.0);
    let wander = mc_clamp(
        -39.0 + (-PI / 4.0 + id + breath / 20.0).cos() * 40.0,
        0.0,
        1.0,
    ) * mc_clamp(1.0 - limb_speed * 2.0, 0.0, 1.0);
    let glance = mc_clamp(glance_window * 4.0, 0.0, 1.0);
    let squint = mc_clamp(2.0 * glance_window, 0.0, 0.3);

    let eye_top = mc_clamp(0.0, -0.8, 0.5) + squint;
    let pupils_x = -(PI / 4.0 + id + age / 10.0).sin() * 3.0 * glance;
    let pupil_y = 0.5 + (-(id + breath + (id + breath).sin()).cos() / 4.0) * wander + squint;
    let blink_phase = id + age / 8.0;
    let blink_pulse = (1.5 - (blink_phase.sin() * 6.0).abs())
        * mc_clamp(
            -32.0 + (blink_phase / 1.5).cos() * 40.0 + (blink_phase / 4.0).cos() * 40.0,
            0.0,
            1.0,
        );

    // The enderman-style 3x1 eyes have their own drift.
    let ender_drift_gate = mc_clamp(-40.0 - (id + age / 125.0).cos() * 60.0, 0.0, 1.0);
    let ender_wobble =
        mc_clamp((id + age / 38.0).sin() + (id + age / 13.0).sin(), -0.2, 0.2) * ender_drift_gate;
    let ender_lid = mc_clamp(
        2.0 * mc_clamp(-0.2 - (id + age / 70.0).cos() * 2.0, 0.0, 1.0),
        0.0,
        0.3,
    );
    let ender_eye_top = ender_lid + ender_wobble / 2.0;
    let ender_right_pupil_x = -0.5
        + (-(PI / 4.0 + id + age / 10.0).sin() * 9.0 - 0.3) * glance
        + mc_clamp((id + age / 27.0).sin() + (id + age / 16.0).sin(), -0.2, 0.2)
            * ender_drift_gate
            * mc_clamp(236.0 - (id + age / 187.0).sin() * 240.0, 0.0, 1.0);
    let ender_pupil_y = 0.5
        + (ender_wobble - mc_clamp(-24.0 + (id + age / 100.0).sin() * 40.0, 0.0, 1.0) / 10.0
            + ender_lid);

    let emote = ((-PI / 3.0 + id + breath).cos() / 1.5 - 0.1) * wander;
    let brows_y = 28.0 + mc_clamp(-(id + age / 110.0).sin() * 4.0, -0.3, 0.0) + emote * 0.5
        - 0.2 * glance_window;
    let brow_raise = mc_clamp(-4.0 + (id + age / 97.0).sin() * 8.0, 0.0, 1.0) * random;
    let villager_lift = mc_clamp((id + age / 73.0).sin() * 3.0, 0.0, 1.0);

    FaceExpressionPose {
        lod: mc_clamp(input.camera_distance_blocks, 0.0, 128.0) / 1000.0,
        eye_top,
        right_pupil_x: 0.5 + pupils_x - 0.3 * glance,
        left_pupil_x: -0.5 + pupils_x + 0.3 * glance,
        pupil_y,
        blink: mc_clamp(blink_pulse, eye_top, 1.0),
        ender_eye_top,
        ender_right_pupil_x,
        ender_left_pupil_x: ender_right_pupil_x + 1.0 + 0.6 * glance,
        ender_pupil_y,
        ender_blink: mc_clamp(blink_pulse, ender_eye_top, 1.0),
        right_brow_ty: brows_y + (-1.0 + (-id).sin() / 3.0) / 4.0 * brow_raise,
        left_brow_ty: brows_y + (-1.0 + (-id).cos() / 3.0) / 4.0 * brow_raise,
        villager_brow_ty: -0.5
            + mc_clamp(1.0 - (id + age / 110.0).sin() * 4.0, -0.3, 0.0)
            + mc_clamp(0.0, -0.8, -0.3 * villager_lift) * 1.2
            + emote
            - 0.2 * glance_window,
    }
}

/// Animated values for one pupil chain. `None` keeps the rig's rest position.
#[derive(Clone, Copy)]
struct PupilPose {
    tx: Option<f32>,
    ty: Option<f32>,
    inner_sx: f32,
    outer_sx: f32,
    down_sy: f32,
    up_sy: f32,
}

#[derive(Clone, Copy)]
struct EyePose {
    /// Depth scale of the eye group, keeping the layers apart at range.
    group_sz: f32,
    white_sy: f32,
    right_pupil: PupilPose,
    left_pupil: PupilPose,
    eyelid_sy: f32,
    blink_sy: f32,
}

/// Evaluates the animation values for `rig`; the eye family it belongs to decides the
/// clamps.
fn eye_pose(rig_index: usize, pose: FaceExpressionPose) -> EyePose {
    let eye_top = pose.eye_top;
    let pupil_y = pose.pupil_y;
    let standard_up = mc_clamp(
        1.0 - (pupil_y - 0.5),
        0.5,
        mc_clamp(1.0 - eye_top, 0.5, 1.0),
    );
    let standard_down = mc_clamp(1.0 + (pupil_y - 0.5) - eye_top, 0.5, 1.0);
    let right_inner = mc_clamp(1.0 - (pose.right_pupil_x - 0.5), 0.75, 1.0);
    let left_outer = mc_clamp(1.0 + (pose.left_pupil_x + 0.5), 0.75, 1.0);
    let right_tx = mc_clamp(pose.right_pupil_x, -0.5, 0.5);
    let left_tx = mc_clamp(pose.left_pupil_x, -0.5, 0.5);
    let standard_white = mc_clamp(1.0 - eye_top, 0.5, 3.0);
    let pupils = |ty: f32, up_sy: f32, down_sy: f32| {
        (
            PupilPose {
                tx: Some(right_tx),
                ty: Some(ty),
                inner_sx: right_inner,
                outer_sx: 1.0,
                down_sy,
                up_sy,
            },
            PupilPose {
                tx: Some(left_tx),
                ty: Some(ty),
                inner_sx: 1.0,
                outer_sx: left_outer,
                down_sy,
                up_sy,
            },
        )
    };
    let group_sz = |factor: f32| 1.0 + pose.lod * factor;

    match rig_index + 1 {
        // 2x2
        5..=8 => {
            let white_sy = mc_clamp(1.0 - eye_top, 0.75, 1.5);
            let up = mc_clamp(
                1.0 - (pupil_y - 0.5),
                0.75,
                mc_clamp(1.0 - eye_top, 0.5, 1.0),
            );
            let (right_pupil, left_pupil) =
                pupils(mc_clamp(pupil_y, eye_top, 0.75), up, standard_down);
            EyePose {
                group_sz: group_sz(500.0),
                white_sy,
                right_pupil,
                left_pupil,
                eyelid_sy: mc_clamp(white_sy, 1.0, 1.5),
                blink_sy: pose.blink,
            }
        }
        // 2x3
        9 => {
            let white_sy = mc_clamp(1.0 - eye_top, 0.75, 1.1);
            let (right_pupil, left_pupil) = pupils(mc_clamp(pupil_y, eye_top, 0.75), 1.0, 1.0);
            EyePose {
                group_sz: group_sz(500.0),
                white_sy,
                right_pupil,
                left_pupil,
                eyelid_sy: mc_clamp(white_sy, 1.0, 1.1),
                blink_sy: pose.blink,
            }
        }
        // 3x1 "enderman"
        10 | 11 => {
            let eye_top = pose.ender_eye_top;
            let pupil_y = pose.ender_pupil_y;
            let white_sy = mc_clamp(1.0 - eye_top, 0.5, 3.0);
            let ty = mc_clamp(pupil_y, eye_top, 1.0);
            let up_sy = mc_clamp(
                1.0 - (pupil_y - 0.5),
                0.5,
                mc_clamp(1.0 - eye_top, 0.5, 1.0),
            );
            let down_sy = mc_clamp(1.0 + (pupil_y - 0.5) - eye_top, 0.5, 1.0);
            EyePose {
                group_sz: group_sz(500.0),
                white_sy,
                right_pupil: PupilPose {
                    tx: Some(mc_clamp(pose.ender_right_pupil_x, -1.5, 0.5)),
                    ty: Some(ty),
                    inner_sx: 1.0,
                    outer_sx: 1.0,
                    down_sy,
                    up_sy,
                },
                left_pupil: PupilPose {
                    tx: Some(mc_clamp(pose.ender_left_pupil_x, -0.5, 1.5)),
                    ty: Some(ty),
                    inner_sx: 1.0,
                    outer_sx: 1.0,
                    down_sy,
                    up_sy,
                },
                eyelid_sy: mc_clamp(white_sy, 1.0, 4.0),
                blink_sy: pose.ender_blink,
            }
        }
        // 1x1 pupils only: each side borrows the opposite 2x1 pupil's motion.
        12..=15 => {
            let ty = mc_clamp(pupil_y, eye_top, 1.0);
            EyePose {
                group_sz: group_sz(250.0),
                white_sy: 1.0,
                right_pupil: PupilPose {
                    tx: Some(left_tx),
                    ty: Some(ty),
                    inner_sx: 1.0,
                    outer_sx: left_outer,
                    down_sy: standard_down,
                    up_sy: standard_up,
                },
                left_pupil: PupilPose {
                    tx: Some(right_tx),
                    ty: Some(ty),
                    inner_sx: right_inner,
                    outer_sx: 1.0,
                    down_sy: standard_down,
                    up_sy: standard_up,
                },
                eyelid_sy: 1.0,
                blink_sy: 0.0,
            }
        }
        // Lid-only eyes.
        17..=24 => {
            let (right_pupil, left_pupil) = pupils(0.0, 1.0, 1.0);
            EyePose {
                group_sz: group_sz(150.0),
                white_sy: 1.0,
                right_pupil,
                left_pupil,
                eyelid_sy: mc_clamp(standard_white, 1.0, 4.0),
                blink_sy: pose.blink,
            }
        }
        // 2x1, including the far-set variant.
        _ => {
            let (right_pupil, left_pupil) =
                pupils(mc_clamp(pupil_y, eye_top, 1.0), standard_up, standard_down);
            EyePose {
                group_sz: group_sz(500.0),
                white_sy: standard_white,
                right_pupil,
                left_pupil,
                eyelid_sy: mc_clamp(standard_white, 1.0, 4.0),
                blink_sy: pose.blink,
            }
        }
    }
}

fn rig_vec(v: RigVec) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

fn node(pivot: Vec3) -> ModelTransform {
    ModelTransform::translation(pivot)
}

fn rig_uv(uv: [u32; 4]) -> Rect {
    let inset = FACE_TEXEL_EXACT_INSET_TEXELS;
    let inset_u = if uv[0] <= uv[2] { inset } else { -inset };
    let inset_v = if uv[1] <= uv[3] { inset } else { -inset };
    Rect::from_min_max(
        pos2(
            (uv[0] as f32 + inset_u) / 64.0,
            (uv[1] as f32 + inset_v) / 64.0,
        ),
        pos2(
            (uv[2] as f32 - inset_u) / 64.0,
            (uv[3] as f32 - inset_v) / 64.0,
        ),
    )
}

struct FaceRenderer<'a> {
    out: &'a mut Vec<RenderTriangle>,
    camera: &'a Camera,
    projection: Projection,
    rect: Rect,
    light_dir: Vec3,
}

impl FaceRenderer<'_> {
    /// Draws the front face of `quad`, given in `transform`'s Minecraft-space coordinates.
    fn panel(&mut self, transform: ModelTransform, quad: RigBox) {
        let [x0, y0, z] = quad.min;
        let [x1, y1, _] = quad.max;
        add_transformed_quad_triangles(
            self.out,
            TriangleTexture::Skin,
            transform,
            [
                Vec3::new(x0, y0, z),
                Vec3::new(x1, y0, z),
                Vec3::new(x1, y1, z),
                Vec3::new(x0, y1, z),
            ],
            Vec3::new(0.0, 0.0, -1.0),
            rig_uv(quad.uv),
            false,
            self.camera,
            self.projection,
            self.rect,
            self.light_dir,
        );
    }

    fn eye_side(
        &mut self,
        group: ModelTransform,
        rig: EyeSideRig,
        pose: EyePose,
        pupil: PupilPose,
    ) {
        let eye = group.then(node(rig_vec(rig.eye)));
        if let Some(white) = rig.white {
            let white_node =
                eye.then(node(rig_vec(white.pivot)))
                    .scaled(Vec3::new(1.0, pose.white_sy, 1.0));
            self.panel(white_node, white.quad);
        }
        if let Some(chain) = rig.pupil {
            let rest = rig_vec(chain.pupil);
            let pupil_node = eye
                .then(node(Vec3::new(
                    pupil.tx.unwrap_or(rest.x),
                    pupil.ty.unwrap_or(rest.y),
                    rest.z,
                )))
                .then(node(rig_vec(chain.inner)))
                .scaled(Vec3::new(pupil.inner_sx, 1.0, 1.0))
                .then(node(rig_vec(chain.outer)))
                .scaled(Vec3::new(pupil.outer_sx, 1.0, 1.0))
                .then(node(rig_vec(chain.down)))
                .scaled(Vec3::new(1.0, pupil.down_sy, 1.0))
                .then(node(rig_vec(chain.up)))
                .scaled(Vec3::new(1.0, pupil.up_sy, 1.0));
            self.panel(pupil_node, chain.quad);
        }
        if let Some(lid) = rig.lid
            && pose.eyelid_sy > 0.0
            && pose.blink_sy > 0.0
        {
            let blink_node = eye
                .then(node(rig_vec(lid.eyelid)))
                .scaled(Vec3::new(1.0, pose.eyelid_sy, 1.0))
                .then(node(rig_vec(lid.blink)))
                .scaled(Vec3::new(1.0, pose.blink_sy, 1.0));
            self.panel(blink_node, lid.quad);
        }
    }
}

/// Adds the expression panels on top of the head. `head_transform` is the head part's
/// preview-space transform (pivot at the neck).
pub(super) fn add_expression_triangles(
    out: &mut Vec<RenderTriangle>,
    camera: &Camera,
    projection: Projection,
    rect: Rect,
    head_transform: ModelTransform,
    light_dir: Vec3,
    layout: DetectedExpressionsLayout,
    pose: FaceExpressionPose,
) {
    // Minecraft model space is preview space rotated 180° about X.
    let head = head_transform.then(ModelTransform::minecraft_model_axes());
    let face_front = head.then(node(rig_vec(FACE_FRONT_PIVOT)));
    let mut renderer = FaceRenderer {
        out,
        camera,
        projection,
        rect,
        light_dir,
    };

    if let Some(rig_index) = EYE_RIGS.iter().position(|rig| rig.id == layout.eye.id) {
        let rig = EYE_RIGS[rig_index];
        let eye_pose = eye_pose(rig_index, pose);
        let group = face_front.scaled(Vec3::new(1.0, 1.0, eye_pose.group_sz));
        renderer.eye_side(group, rig.right, eye_pose, eye_pose.right_pupil);
        renderer.eye_side(group, rig.left, eye_pose, eye_pose.left_pupil);
    }

    if let Some(brow) = layout.brow {
        let masks = face_front.scaled(Vec3::new(1.0, 1.0, 1.0 + pose.lod * 500.0));
        if brow.kind == BrowKind::Villager {
            renderer.panel(masks, VILLAGER_BROW_MASK);
            let villager = face_front.then(node(Vec3::new(
                VILLAGER_BROW_PIVOT[0],
                pose.villager_brow_ty,
                -pose.lod * 3.0,
            )));
            renderer.panel(villager, VILLAGER_BROW);
        } else if let Some(rig) = BROW_RIGS.iter().find(|rig| rig.id == brow.id) {
            // Hat brows sit on the hat layer, nudged back by the level-of-detail offset.
            let overlay_tz = if brow.kind == BrowKind::Hat {
                pose.lod
            } else {
                0.0
            };
            let brows_tz = BROWS_PIVOT[2] - pose.lod * 2.0 + overlay_tz;
            let right = face_front.then(node(Vec3::new(0.0, pose.right_brow_ty, brows_tz)));
            let left = face_front.then(node(Vec3::new(0.0, pose.left_brow_ty, brows_tz)));
            if let Some(mask) = rig.right_mask {
                renderer.panel(masks, mask);
            }
            if let Some(mask) = rig.left_mask {
                renderer.panel(masks, mask);
            }
            renderer.panel(right, rig.right);
            if let Some(left_brow) = rig.left {
                renderer.panel(left, left_brow);
            }
        }
    }

    let lashes = face_front
        .then(node(Vec3::new(
            EYELASHES_PIVOT[0],
            EYELASHES_PIVOT[1],
            -pose.lod,
        )))
        .scaled(Vec3::new(1.0, mc_clamp(1.0 - pose.eye_top, 1.0, 1.5), 1.0));
    for lash in EYELASHES {
        renderer.panel(lashes, lash);
    }
}

pub(in super::super) fn eye_lid_rects(spec: EyeExpressionSpec) -> (TextureRectU32, TextureRectU32) {
    if let Some(right_lid) = spec.blink {
        let left_dx = spec.left_eye.x as i32 - spec.right_eye.x as i32;
        let left_x = (right_lid.x as i32 + left_dx).max(0) as u32;
        (
            right_lid,
            TextureRectU32 {
                x: left_x,
                y: right_lid.y,
                w: right_lid.w,
                h: right_lid.h,
            },
        )
    } else {
        (spec.right_eye, spec.left_eye)
    }
}

pub(in super::super) fn compatibility_score(
    eye: EyeExpressionSpec,
    brow: BrowExpressionSpec,
) -> i32 {
    let mut score = 0;
    if eye.offset == brow.offset {
        score += 10;
    }
    match (eye.family, brow.kind) {
        (EyeFamily::Spread, BrowKind::Spread) => score += 6,
        (EyeFamily::ThreeByOne, BrowKind::Villager) => score += 3,
        (_, BrowKind::Standard) => score += 2,
        _ => {}
    }
    score - ((eye.center_y - brow.center_y).abs() * 4.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_rigs_are_indexed_by_their_number() {
        // `eye_pose` picks the animation group from the rig's position.
        for (index, rig) in EYE_RIGS.iter().enumerate() {
            assert_eq!(rig.id, format!("eye_{}", index + 1));
        }
    }

    #[test]
    fn mc_clamp_matches_minecraft() {
        assert_eq!(mc_clamp(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(mc_clamp(2.0, 0.0, 1.0), 1.0);
        // Inverted bounds: below `min` returns `min`, otherwise `max`.
        assert_eq!(mc_clamp(0.0, 0.75, 0.5), 0.75);
        assert_eq!(mc_clamp(0.9, 0.75, 0.5), 0.5);
    }
}
