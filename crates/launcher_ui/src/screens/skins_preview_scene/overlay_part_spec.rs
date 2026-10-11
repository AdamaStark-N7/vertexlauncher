use super::*;

#[derive(Clone, Copy)]
pub(super) struct OverlayPartSpec {
    pub(super) size: Vec3,
    pub(super) transform: ModelTransform,
    /// Top-center of the base cuboid in `transform`'s local space.
    pub(super) local_top_center: Vec3,
}
