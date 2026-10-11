#[derive(Clone, Copy)]
pub(super) struct PreviewPose {
    pub(super) time_seconds: f32,
    pub(super) locomotion_blend: f32,
    /// Accumulated walk-animation position (Minecraft's `limb_swing`), in ticks × speed.
    pub(super) limb_swing: f32,
}

impl PreviewPose {
    pub(super) fn limb_speed(self) -> f32 {
        super::PREVIEW_WALK_LIMB_SPEED * self.locomotion_blend.clamp(0.0, 1.0)
    }

    /// Returns this pose advanced by `seconds`, extrapolating the walk cycle at the
    /// current limb speed.
    pub(super) fn offset_by(self, seconds: f32) -> Self {
        Self {
            time_seconds: self.time_seconds + seconds,
            locomotion_blend: self.locomotion_blend,
            limb_swing: self.limb_swing + seconds * 20.0 * self.limb_speed(),
        }
    }
}
