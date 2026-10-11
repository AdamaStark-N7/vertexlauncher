use super::*;

/// Affine transform from a model part's local space into preview world space.
///
/// Mirrors Minecraft's `ModelPart` pose stack: translate to the pivot, rotate Z·Y·X,
/// then scale. Children are composed with [`ModelTransform::then`].
#[derive(Clone, Copy)]
pub(crate) struct ModelTransform {
    pub(crate) origin: Vec3,
    pub(crate) x_axis: Vec3,
    pub(crate) y_axis: Vec3,
    pub(crate) z_axis: Vec3,
}

impl ModelTransform {
    pub(crate) fn translation(origin: Vec3) -> Self {
        Self {
            origin,
            x_axis: Vec3::new(1.0, 0.0, 0.0),
            y_axis: Vec3::new(0.0, 1.0, 0.0),
            z_axis: Vec3::new(0.0, 0.0, 1.0),
        }
    }

    /// Maps Minecraft entity-model axes (y down, model facing -z) onto preview axes (y up,
    /// model facing +z); a 180° rotation about X.
    pub(crate) fn minecraft_model_axes() -> Self {
        Self {
            origin: Vec3::new(0.0, 0.0, 0.0),
            x_axis: Vec3::new(1.0, 0.0, 0.0),
            y_axis: Vec3::new(0.0, -1.0, 0.0),
            z_axis: Vec3::new(0.0, 0.0, -1.0),
        }
    }

    /// Builds a pivot transform with rotations applied in Minecraft's X, then Y, then Z order.
    pub(crate) fn from_pivot_rotation(
        pivot: Vec3,
        rotate_x: f32,
        rotate_y: f32,
        rotate_z: f32,
    ) -> Self {
        let rotate =
            |v: Vec3| rotate_z_vec(rotate_y_vec(rotate_x_vec(v, rotate_x), rotate_y), rotate_z);
        Self {
            origin: pivot,
            x_axis: rotate(Vec3::new(1.0, 0.0, 0.0)),
            y_axis: rotate(Vec3::new(0.0, 1.0, 0.0)),
            z_axis: rotate(Vec3::new(0.0, 0.0, 1.0)),
        }
    }

    /// Scales the local axes, so children and geometry are scaled about this pivot.
    pub(crate) fn scaled(self, scale: Vec3) -> Self {
        Self {
            origin: self.origin,
            x_axis: self.x_axis * scale.x,
            y_axis: self.y_axis * scale.y,
            z_axis: self.z_axis * scale.z,
        }
    }

    /// Composes `child` (expressed in this transform's local space) onto `self`.
    pub(crate) fn then(self, child: Self) -> Self {
        Self {
            origin: self.transform_point(child.origin),
            x_axis: self.transform_vector(child.x_axis),
            y_axis: self.transform_vector(child.y_axis),
            z_axis: self.transform_vector(child.z_axis),
        }
    }

    pub(crate) fn transform_point(self, point: Vec3) -> Vec3 {
        self.origin + self.transform_vector(point)
    }

    pub(crate) fn transform_vector(self, vector: Vec3) -> Vec3 {
        self.x_axis * vector.x + self.y_axis * vector.y + self.z_axis * vector.z
    }
}

pub(super) fn rotate_x_vec(point: Vec3, radians: f32) -> Vec3 {
    let (sin, cos) = radians.sin_cos();
    Vec3::new(
        point.x,
        point.y * cos - point.z * sin,
        point.y * sin + point.z * cos,
    )
}

pub(super) fn rotate_y_vec(point: Vec3, radians: f32) -> Vec3 {
    let (sin, cos) = radians.sin_cos();
    Vec3::new(
        point.x * cos + point.z * sin,
        point.y,
        -point.x * sin + point.z * cos,
    )
}

pub(super) fn rotate_z_vec(point: Vec3, radians: f32) -> Vec3 {
    let (sin, cos) = radians.sin_cos();
    Vec3::new(
        point.x * cos - point.y * sin,
        point.x * sin + point.y * cos,
        point.z,
    )
}
