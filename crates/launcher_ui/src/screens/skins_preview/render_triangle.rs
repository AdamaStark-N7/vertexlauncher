use super::*;

pub(crate) struct RenderTriangle {
    pub(crate) texture: TriangleTexture,
    pub(crate) pos: [Pos2; 3],
    pub(crate) uv: [Pos2; 3],
    /// Normalized texture area this triangle's face owns. The shader snaps it to whole
    /// texels and never samples outside it, so neighbouring regions can't bleed in.
    pub(crate) uv_bounds: Rect,
    pub(crate) depth: [f32; 3],
    pub(crate) color: Color32,
}
