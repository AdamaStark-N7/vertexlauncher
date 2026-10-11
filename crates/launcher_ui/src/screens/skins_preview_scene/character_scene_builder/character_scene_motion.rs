use super::*;

pub(super) struct CharacterSceneMotion {
    pub(super) arm_width: f32,
    pub(super) limb_speed: f32,
    pub(super) age_ticks: f32,
    pub(super) entity_random: f32,
    pub(super) pose: PlayerMotionPose,
}

impl CharacterSceneMotion {
    pub(super) fn new(
        preview_pose: PreviewPose,
        variant: MinecraftSkinVariant,
        skin_hash: Option<u64>,
    ) -> Self {
        let arm_width = if variant == MinecraftSkinVariant::Slim {
            3.0
        } else {
            4.0
        };
        let limb_speed = preview_pose.limb_speed();
        let age_ticks = preview_pose.time_seconds * 20.0;
        let entity_random = skin_hash.map_or(0.5, preview_entity_random);
        let pose = player_motion_pose(PlayerMotionInput {
            age_ticks,
            limb_swing: preview_pose.limb_swing,
            limb_speed,
            entity_random,
        });

        Self {
            arm_width,
            limb_speed,
            age_ticks,
            entity_random,
            pose,
        }
    }
}

/// Stable per-player random value, derived from the skin so each skin keeps
/// its own breathing and blink timing.
fn preview_entity_random(skin_hash: u64) -> f32 {
    let mut value = skin_hash ^ 0x5bd1_e995_9e37_79b9;
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    ((value >> 40) as f32) / ((1u64 << 24) as f32)
}
