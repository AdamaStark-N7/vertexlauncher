//! Procedural player idle/walk poses for the preview, written against the 1.21.2+ player
//! model rig (body, head, limbs, cape, and elytra).
//!
//! Only the states the preview can show are kept: standing or walking forward on the
//! ground, right-handed, empty hands, no head look input. Every term that depends on
//! sneaking, swimming, flying, jumping, items, or camera movement evaluates to zero there
//! and has been dropped.

use super::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// One model part's pose in Minecraft entity-model space: pixels, y pointing down, the
/// model facing -z, origin at the neck (24 px above the feet).
#[derive(Clone, Copy, Default)]
pub(super) struct McPartPose {
    pub(super) tx: f32,
    pub(super) ty: f32,
    pub(super) tz: f32,
    pub(super) rx: f32,
    pub(super) ry: f32,
    pub(super) rz: f32,
}

impl McPartPose {
    /// Converts to a preview-space transform. Preview space is model space rotated 180°
    /// about X (y up, model facing +z) with the feet at y = 0, so X rotations carry over
    /// and Y/Z rotations flip sign.
    pub(super) fn transform(self, model_offset: Vec3) -> ModelTransform {
        ModelTransform::from_pivot_rotation(
            Vec3::new(self.tx, 24.0 - self.ty, -self.tz) + model_offset,
            self.rx,
            -self.ry,
            -self.rz,
        )
    }
}

#[derive(Clone, Copy)]
pub(super) struct PlayerMotionInput {
    pub(super) age_ticks: f32,
    pub(super) limb_swing: f32,
    pub(super) limb_speed: f32,
    /// A stable per-player random value in `0.0..1.0`.
    pub(super) entity_random: f32,
}

#[derive(Clone, Copy)]
pub(super) struct PlayerMotionPose {
    pub(super) body: McPartPose,
    pub(super) head: McPartPose,
    pub(super) right_arm: McPartPose,
    pub(super) left_arm: McPartPose,
    pub(super) right_leg: McPartPose,
    pub(super) left_leg: McPartPose,
    /// The cape hinge follows the body pitch and, when standing, the body yaw.
    pub(super) cape_hinge_rx: f32,
    pub(super) cape_hinge_ry: f32,
    /// Cape swing (positive lifts the cape away from the back) and sway.
    pub(super) cape_rx: f32,
    pub(super) cape_rz: f32,
    /// Elytra `wings` lift plus per-wing yaw/roll sway, added to the vanilla wing pose.
    pub(super) wings_rx: f32,
    pub(super) wing_ry: f32,
    pub(super) right_wing_rz: f32,
    pub(super) left_wing_rz: f32,
}

fn torad(degrees: f32) -> f32 {
    degrees.to_radians()
}

pub(super) fn player_motion_pose(input: PlayerMotionInput) -> PlayerMotionPose {
    let age = input.age_ticks;
    let random = input.entity_random;
    let limb_swing = input.limb_swing;
    let limb_speed = input.limb_speed.clamp(0.0, 1.0);
    let speed_sqrt = limb_speed.sqrt();

    // State variables. `run` would normally integrate over time once limb speed passes 0.63; the
    // preview only ramps between standing and walking, so derive it from the speed.
    let walk = (3.0 * limb_speed).clamp(0.0, 1.0);
    let run = smoothstep(((limb_speed - 0.5) / 0.3).clamp(0.0, 1.0));
    let idle = 1.0 - (3.0 * limb_speed).min(1.0);
    let move_forward = 1.0;
    let forward_drag = (limb_speed / PREVIEW_WALK_LIMB_SPEED).clamp(0.0, 1.0);
    let breath = random * PI * 4.0 + age / (11.5 - 2.0 * random);
    let ls = limb_swing * 0.58 + FRAC_PI_2;

    // Body: pivots about the hips (body.t* keeps the hip point fixed while it leans).
    let idle_body_rx = torad(breath.sin()) * idle;
    let move_body_rx =
        torad(2.0 * (ls * 2.0).cos() + 7.0 * forward_drag) * speed_sqrt * (walk / 2.0 + run / 2.0);
    let body_rx = idle_body_rx + move_body_rx;
    let idle_body_ry = torad(2.0 * (breath / 2.0).sin()) * idle;
    let move_body_ry = torad(13.0 * ls.cos()) * mc_clamp(speed_sqrt * walk, 0.5 * idle, 1.0);
    let body_ry = idle_body_ry + move_body_ry;
    let body_rz = torad(-3.0 * ls.sin() * (1.0 - 0.5 * run)) * speed_sqrt * walk;
    let idle_body_tx = 0.25 * (breath / 2.0).sin() * idle - idle_body_rx * idle_body_ry * 16.0;
    let move_body_tx = -ls.cos() / 2.6 * move_forward * speed_sqrt * run;
    let idle_body_ty = 0.05 + 0.1 * (PI / 12.0 + breath).sin();
    let move_body_ty = ((PI / 4.0 + ls * 2.0 - (PI / 4.0 + ls * 2.0).cos() / 6.0).sin() + 0.8)
        * 0.8
        * walk
        * (1.0 - run)
        + -(PI / 4.0 + ls * 2.0 - (ls * 2.0).cos() / 4.0).cos() * speed_sqrt * run;
    let body = McPartPose {
        tx: body_rz.sin() * 12.0 + idle_body_tx + move_body_tx,
        ty: -body_rx.cos() * 12.0 + 12.0 - body_rz.cos() * 12.0
            + 12.0
            + idle_body_ty
            + move_body_ty,
        tz: -body_rx.sin() * 12.0 + 0.1 * breath.sin(),
        rx: body_rx,
        ry: body_ry,
        rz: body_rz,
    };

    // Head rides on the neck and keeps looking ahead.
    let head = McPartPose {
        tx: body.tx,
        ty: body.ty + 0.03 * (1.0 - breath.cos()),
        tz: body.tz,
        ..McPartPose::default()
    };

    // Arms.
    let arm_swing = 60.0 * (PI / 7.0 + ls).cos() * (1.0 - 0.2 * walk);
    let arm_motion = speed_sqrt * walk;
    let arm_yaw_motion = speed_sqrt * walk + idle;
    let right_arm_ry = torad(breath.cos() + 0.5 + 3.0) * idle
        + torad(
            (18.0 * ls.cos() - 10.0 * (0.8 + (-1.3 + ls - (-1.3 + ls).sin() / 1.5).cos())) / 2.0,
        ) * arm_yaw_motion;
    let left_arm_ry = torad(-breath.cos() - 0.5) * idle
        + torad(
            (18.0 * ls.cos() + 10.0 * (0.8 - (-1.3 + ls + (-1.3 + ls).sin() / 1.5).cos())) / 2.0,
        ) * arm_yaw_motion;
    let idle_arm_roll = (-PI / 12.0 + breath).cos() / 1.6 + 2.0;
    let arm_ty = body.ty + body_rx.cos() * 2.0 + (-PI / 15.0 + breath).sin() / 6.0 + 0.3;
    let arm_tz = body.tz + body_rx.sin() * 2.0;
    let right_arm = McPartPose {
        tx: body.tx + (5.0 - body_ry.cos() * 5.0) * idle - 5.0,
        ty: arm_ty,
        tz: arm_tz
            + body_ry.sin() * 5.0 * idle
            + 2.0 * (ls - ls.sin() / 3.0 * 0.3).cos() * arm_motion,
        rx: torad(arm_swing + 10.0 * move_forward) * arm_motion,
        ry: right_arm_ry,
        rz: torad(idle_arm_roll) * idle + torad(2.0 + 4.0 * (PI / 5.0 + ls).cos()) * arm_motion,
    };
    let left_arm = McPartPose {
        tx: body.tx + (body_ry.cos() * 5.0 - 5.0) * idle + 5.0,
        ty: arm_ty,
        tz: arm_tz
            - body_ry.sin() * 5.0 * idle
            - 2.0 * (ls + ls.sin() / 3.0 * 0.3).cos() * arm_motion,
        rx: torad(-arm_swing + 10.0 * move_forward) * arm_motion,
        ry: left_arm_ry,
        rz: torad(-idle_arm_roll) * idle + torad(-2.0 + 4.0 * (PI / 5.0 + ls).cos()) * arm_motion,
    };

    // Legs. The "foot" angles rotate a leg about its sole, so the pivot is shifted to keep
    // the foot planted.
    let foot_rx = torad((1.0 + (PI / 4.0 + breath).sin()) / 2.0) * idle;
    let foot_rz = torad((breath / 2.0).sin()) * idle;
    let right_foot_ry = torad(10.0 + 10.0 * ls.cos()) * idle;
    let left_foot_ry = torad(-10.0 + 10.0 * ls.cos()) * idle;
    let stride_weight = mc_clamp(walk, 0.1 * idle, 1.0) * (1.0 - run).max(0.0);
    let right_leg_rx = foot_rx
        + torad(20.0 * (0.67 + 0.33 * move_forward) - 60.0 * (ls + ls.cos() / 4.0).cos())
            * speed_sqrt
            * run
        + torad(13.0 * (1.0 - idle) - 40.0 * (ls + ls.cos() / 2.5).cos()) * stride_weight;
    let left_leg_rx = foot_rx
        + torad(20.0 * (0.67 + 0.33 * move_forward) + 60.0 * (ls - ls.cos() / 4.0).cos())
            * speed_sqrt
            * run
        + torad(13.0 * (1.0 - idle) + 40.0 * (ls - ls.cos() / 2.5).cos()) * stride_weight;
    let foot_lift = (12.0 - foot_rx.cos() * 12.0) + (12.0 - foot_rz.cos() * 12.0);
    let right_leg_ty = 12.0
        + foot_lift
        + (-0.4 - 2.0 * (0.3 + ls + (0.2 + ls).cos() * 1.3).cos()) * speed_sqrt * run
        - (-0.3
            + mc_clamp(
                -1.3 * (ls + (-PI / 4.0 + ls).cos() / 2.5).sin(),
                -0.45 + 0.7 * (PI / 6.0 + ls).sin(),
                3.0,
            ))
            * walk
            * (1.0 - run);
    let left_leg_ty = 12.0
        + foot_lift
        + (-0.4 + 2.0 * (0.3 + ls - (0.2 + ls).cos() * 1.3).cos()) * speed_sqrt * run
        - (-0.3
            + mc_clamp(
                1.3 * (ls - (-PI / 4.0 + ls).cos() / 2.5).sin(),
                -0.45 - 0.7 * (PI / 6.0 + ls).sin(),
                3.0,
            ))
            * walk
            * (1.0 - run);
    let right_leg = McPartPose {
        tx: -2.0 - foot_rx.sin() * 12.0 * right_foot_ry + foot_rz.sin() * 12.0
            - 0.1 * idle
            - 0.05 * speed_sqrt * run,
        ty: right_leg_ty,
        tz: -foot_rx.sin() * 12.0
            + 0.4 * ls.cos() * idle
            + (-2.5 + 2.5 * (0.4 + ls + (1.0 + ls).cos() / 1.4).sin()) * speed_sqrt * run
            + (-1.5 + 1.6 * (ls + (PI / 4.0 + ls).cos() / 2.5).sin()) * walk * (1.0 - run),
        rx: right_leg_rx,
        ry: right_foot_ry,
        rz: foot_rz
            + right_leg_rx * right_foot_ry
            + torad(2.0) * idle
            + torad(1.0 - 6.0 * (ls - ls.cos() / 1.3).cos()) * speed_sqrt * 0.5 * run,
    };
    let left_leg = McPartPose {
        tx: 2.0 - foot_rx.sin() * 12.0 * left_foot_ry
            + foot_rz.sin() * 12.0
            + 0.1 * idle
            + 0.05 * speed_sqrt * run,
        ty: left_leg_ty,
        tz: -foot_rx.sin() * 12.0 - 0.4 * ls.cos() * idle
            + (-2.5 - 2.5 * (0.4 + ls - (1.0 + ls).cos() / 1.4).sin()) * speed_sqrt * run
            + (-1.5 - 1.6 * (ls - (PI / 4.0 + ls).cos() / 2.5).sin()) * walk * (1.0 - run),
        rx: left_leg_rx,
        ry: left_foot_ry,
        rz: foot_rz + left_leg_rx * left_foot_ry - torad(2.0) * idle
            + torad(-1.0 - 6.0 * (ls + ls.cos() / 1.3).cos()) * speed_sqrt * 0.5 * run,
    };

    // Cape.
    let cape_swing = torad(
        (60.0 + limb_swing.sin() * 2.0) * limb_speed * (0.3 + 0.7 * forward_drag)
            + 7.0 * (ls * 2.0).sin() * speed_sqrt * run,
    ) + torad(
        5.0 + ((age / 16.0).sin() * 1.5 + (age / 21.0).cos()) * (1.0 - limb_speed),
    ) - idle_body_rx;
    let cape_rz =
        torad(-10.0 * ls.sin() * run) * limb_speed + torad(-7.0 * ls.cos()) * (walk - run);

    // Elytra.
    let wings_rx = mc_clamp(
        torad(50.0 * limb_speed * move_forward + 10.0 * (ls * 2.0).sin() * speed_sqrt * run),
        0.0,
        torad(170.0),
    ) / 4.0;
    let wing_sway = (-PI / 4.0 + ls).sin() * (1.0 + 0.3 * (1.0 - run));
    let wing_ry = torad(7.0 * wing_sway) / 6.0 * speed_sqrt * walk;
    let wing_roll = torad(-6.0 * wing_sway) * speed_sqrt * walk;

    PlayerMotionPose {
        body,
        head,
        right_arm,
        left_arm,
        right_leg,
        left_leg,
        cape_hinge_rx: body_rx,
        cape_hinge_ry: body_ry * (1.0 - walk.min(1.0)),
        cape_rx: mc_clamp(cape_swing, 0.0, 2.8),
        cape_rz,
        wings_rx,
        wing_ry,
        right_wing_rz: mc_clamp(wing_roll, -torad(10.0), torad(90.0)),
        left_wing_rz: mc_clamp(wing_roll, -torad(90.0), torad(10.0)),
    }
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
