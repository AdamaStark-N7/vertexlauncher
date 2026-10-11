use super::*;

/// Texture rectangles for each cuboid face. `left`/`right` are the -x/+x faces; preview
/// space keeps Minecraft's handedness, so on body parts -x is the player's right side.
#[derive(Clone, Copy)]
pub(crate) struct FaceUvs {
    pub(crate) top: Rect,
    pub(crate) bottom: Rect,
    pub(crate) left: Rect,
    pub(crate) right: Rect,
    pub(crate) front: Rect,
    pub(crate) back: Rect,
}
