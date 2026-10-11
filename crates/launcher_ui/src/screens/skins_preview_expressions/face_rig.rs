//! Face rig geometry for expression skins.
//!
//! All positions are Minecraft entity-model coordinates (pixels, y down, face toward -z). Each node's position is its
//! pivot relative to the parent node. Boxes are flat panels on the face; only the
//! front (north) face is drawn. `uv` lists texel coordinates as
//! `[u at viewer's left, v top, u at viewer's right, v bottom]`, so `u0 > u1` means the
//! texture is mirrored.

pub(super) type RigVec = [f32; 3];

#[derive(Clone, Copy)]
pub(super) struct RigBox {
    pub(super) min: RigVec,
    pub(super) max: RigVec,
    pub(super) uv: [u32; 4],
}

/// `pupil` (moves with gaze) → `inner`/`outer` (clip horizontally) → `down`/`up` (clip
/// vertically) → `quad`.
#[derive(Clone, Copy)]
pub(super) struct PupilRig {
    pub(super) pupil: RigVec,
    pub(super) inner: RigVec,
    pub(super) outer: RigVec,
    pub(super) down: RigVec,
    pub(super) up: RigVec,
    pub(super) quad: RigBox,
}

/// Eye white, scaled vertically about its bottom edge.
#[derive(Clone, Copy)]
pub(super) struct WhiteRig {
    pub(super) pivot: RigVec,
    pub(super) quad: RigBox,
}

/// `eyelid` stretches with the white; `blink` scales the lid down from the top edge.
#[derive(Clone, Copy)]
pub(super) struct LidRig {
    pub(super) eyelid: RigVec,
    pub(super) blink: RigVec,
    pub(super) quad: RigBox,
}

#[derive(Clone, Copy)]
pub(super) struct EyeSideRig {
    pub(super) eye: RigVec,
    pub(super) pupil: Option<PupilRig>,
    pub(super) white: Option<WhiteRig>,
    pub(super) lid: Option<LidRig>,
}

#[derive(Clone, Copy)]
pub(super) struct EyeRig {
    /// Matches `EyeExpressionSpec::id`.
    pub(super) id: &'static str,
    pub(super) right: EyeSideRig,
    pub(super) left: EyeSideRig,
}

/// Pivot of the eye, brow, and lash groups relative to the head pivot.
pub(super) const FACE_FRONT_PIVOT: RigVec = [0.0, -4.0, -4.0];

/// Pivot of each brow relative to the brow group (the animated height replaces the 28).
pub(super) const BROWS_PIVOT: RigVec = [0.0, 28.0, 4.0];
/// Pivot of the villager unibrow relative to the brow group (the animated height replaces
/// the -0.5).
pub(super) const VILLAGER_BROW_PIVOT: RigVec = [0.0, -0.5, 0.0];
/// Pivot of the lashes relative to the lash group.
pub(super) const EYELASHES_PIVOT: RigVec = [0.0, 3.0, 0.0];

/// Brow panels relative to each brow's pivot (hat brows use the same pivot), with the mask
/// that hides the brow painted on the base face (relative to the brow group pivot).
#[derive(Clone, Copy)]
pub(super) struct BrowRig {
    pub(super) id: &'static str,
    pub(super) right: RigBox,
    pub(super) left: Option<RigBox>,
    pub(super) right_mask: Option<RigBox>,
    pub(super) left_mask: Option<RigBox>,
}

const fn rig_box(min: RigVec, max: RigVec, uv: [u32; 4]) -> RigBox {
    RigBox { min, max, uv }
}

pub(super) const BROW_RIGS: [BrowRig; 9] = [
    BrowRig {
        id: "brow_bottom",
        right: rig_box([-3.0, -27.0, -4.05], [-1.0, -26.0, -4.05], [60, 7, 62, 8]),
        left: Some(rig_box(
            [1.0, -27.0, -4.05],
            [3.0, -26.0, -4.05],
            [62, 7, 64, 8],
        )),
        right_mask: Some(rig_box(
            [-3.0, 1.0, -0.001],
            [-1.0, 2.0, 0.0],
            [60, 6, 62, 7],
        )),
        left_mask: Some(rig_box([1.0, 1.0, -0.001], [3.0, 2.0, 0.0], [62, 6, 64, 7])),
    },
    BrowRig {
        id: "brow_lower_mid",
        right: rig_box([-3.0, -28.0, -4.05], [-1.0, -27.0, -4.05], [60, 5, 62, 6]),
        left: Some(rig_box(
            [1.0, -28.0, -4.05],
            [3.0, -27.0, -4.05],
            [62, 5, 64, 6],
        )),
        right_mask: Some(rig_box(
            [-3.0, 0.0, -0.001],
            [-1.0, 1.0, 0.0],
            [60, 4, 62, 5],
        )),
        left_mask: Some(rig_box([1.0, 0.0, -0.001], [3.0, 1.0, 0.0], [62, 4, 64, 5])),
    },
    BrowRig {
        id: "brow_upper_mid",
        right: rig_box([-3.0, -29.0, -4.05], [-1.0, -28.0, -4.05], [60, 3, 62, 4]),
        left: Some(rig_box(
            [1.0, -29.0, -4.05],
            [3.0, -28.0, -4.05],
            [62, 3, 64, 4],
        )),
        right_mask: Some(rig_box(
            [-3.0, -1.0, -0.001],
            [-1.0, 0.0, 0.0],
            [60, 2, 62, 3],
        )),
        left_mask: Some(rig_box(
            [1.0, -1.0, -0.001],
            [3.0, 0.0, 0.0],
            [62, 2, 64, 3],
        )),
    },
    BrowRig {
        id: "brow_top",
        right: rig_box([-3.0, -30.0, -4.05], [-1.0, -29.0, -4.05], [60, 1, 62, 2]),
        left: Some(rig_box(
            [1.0, -30.0, -4.05],
            [3.0, -29.0, -4.05],
            [62, 1, 64, 2],
        )),
        right_mask: Some(rig_box(
            [-3.0, -2.0, -0.001],
            [-1.0, -1.0, 0.0],
            [60, 0, 62, 1],
        )),
        left_mask: Some(rig_box(
            [1.0, -2.0, -0.001],
            [3.0, -1.0, 0.0],
            [62, 0, 64, 1],
        )),
    },
    BrowRig {
        id: "brow_spread",
        right: rig_box([-4.0, -29.0, -4.05], [-2.0, -28.0, -4.05], [36, 5, 38, 6]),
        left: Some(rig_box(
            [2.0, -29.0, -4.05],
            [4.0, -28.0, -4.05],
            [38, 5, 40, 6],
        )),
        right_mask: Some(rig_box(
            [-4.0, -1.0, -0.001],
            [-2.0, 0.0, 0.0],
            [36, 4, 38, 5],
        )),
        left_mask: Some(rig_box(
            [2.0, -1.0, -0.001],
            [4.0, 0.0, 0.0],
            [38, 4, 40, 5],
        )),
    },
    BrowRig {
        id: "brow_hat_bottom",
        right: rig_box([-3.0, -27.0, -4.501], [-1.0, -26.0, -4.501], [56, 7, 58, 8]),
        left: Some(rig_box(
            [1.0, -27.0, -4.501],
            [3.0, -26.0, -4.501],
            [58, 7, 60, 8],
        )),
        right_mask: None,
        left_mask: None,
    },
    BrowRig {
        id: "brow_hat_lower_mid",
        right: rig_box([-3.0, -28.0, -4.501], [-1.0, -27.0, -4.501], [56, 6, 58, 7]),
        left: Some(rig_box(
            [1.0, -28.0, -4.501],
            [3.0, -27.0, -4.501],
            [58, 6, 60, 7],
        )),
        right_mask: None,
        left_mask: None,
    },
    BrowRig {
        id: "brow_hat_upper_mid",
        right: rig_box([-3.0, -29.0, -4.501], [-1.0, -28.0, -4.501], [56, 5, 58, 6]),
        left: Some(rig_box(
            [1.0, -29.0, -4.501],
            [3.0, -28.0, -4.501],
            [58, 5, 60, 6],
        )),
        right_mask: None,
        left_mask: None,
    },
    BrowRig {
        id: "brow_hat_top",
        right: rig_box([-3.0, -30.0, -4.501], [-1.0, -29.0, -4.501], [56, 4, 58, 5]),
        left: Some(rig_box(
            [1.0, -30.0, -4.501],
            [3.0, -29.0, -4.501],
            [58, 4, 60, 5],
        )),
        right_mask: None,
        left_mask: None,
    },
];

/// The villager unibrow, relative to its pivot, and its mask relative to the brow group.
pub(super) const VILLAGER_BROW: RigBox =
    rig_box([-3.01, -0.5, -0.05], [3.01, 0.5, -0.05], [24, 1, 29, 2]);
pub(super) const VILLAGER_BROW_MASK: RigBox =
    rig_box([-3.0, -1.0, -0.001], [3.0, 0.0, 0.0], [24, 0, 29, 1]);

/// Eyelash panels relative to the lash pivot.
pub(super) const EYELASHES: [RigBox; 4] = [
    rig_box([-4.0, -2.0, -0.002], [-3.0, 0.0, -0.002], [57, 1, 56, 3]),
    rig_box([3.0, -2.0, -0.002], [4.0, 0.0, -0.002], [60, 1, 59, 3]),
    rig_box([-3.0, -3.0, -0.002], [-1.0, -2.0, -0.002], [56, 0, 58, 1]),
    rig_box([1.0, -3.0, -0.002], [3.0, -2.0, -0.002], [58, 0, 60, 1]),
];

pub(super) const EYE_RIGS: [EyeRig; 24] = [
    EyeRig {
        id: "eye_1",
        right: EyeSideRig {
            eye: [-2.0, 2.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [6, 7, 5, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -1.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [5, 7, 4, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [4, 6, 6, 7],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 2.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [7, 7, 6, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -1.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [8, 7, 7, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [6, 6, 8, 7],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_2",
        right: EyeSideRig {
            eye: [-2.0, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [6, 5, 5, 6],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -1.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [5, 5, 4, 6],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [4, 4, 6, 5],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [7, 5, 6, 6],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -1.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [8, 5, 7, 6],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [6, 4, 8, 5],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_3",
        right: EyeSideRig {
            eye: [-2.0, 0.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [6, 3, 5, 4],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -1.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [5, 3, 4, 4],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [4, 2, 6, 3],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 0.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [7, 3, 6, 4],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -1.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [8, 3, 7, 4],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [6, 2, 8, 3],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_4",
        right: EyeSideRig {
            eye: [-2.0, -1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [6, 1, 5, 2],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -1.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [5, 1, 4, 2],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [4, 0, 6, 1],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, -1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [7, 1, 6, 2],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -1.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [8, 1, 7, 2],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 1.005, -1.995],
                    uv: [6, 0, 8, 1],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_5",
        right: EyeSideRig {
            eye: [-2.0, 2.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [26, 6, 25, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -2.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [25, 6, 24, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [24, 5, 26, 6],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 2.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [27, 6, 26, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -2.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [28, 6, 27, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [26, 5, 28, 6],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_6",
        right: EyeSideRig {
            eye: [-2.0, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [26, 3, 25, 5],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -2.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [25, 3, 24, 5],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [24, 2, 26, 3],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [27, 3, 26, 5],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -2.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [28, 3, 27, 5],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [26, 2, 28, 3],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_7",
        right: EyeSideRig {
            eye: [-2.0, 0.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [30, 6, 29, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -2.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [29, 6, 28, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [28, 5, 30, 6],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 0.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [31, 6, 30, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -2.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [32, 6, 31, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [30, 5, 32, 6],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_8",
        right: EyeSideRig {
            eye: [-2.0, -1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [30, 3, 29, 5],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -2.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [29, 3, 28, 5],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [28, 2, 30, 3],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, -1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [31, 3, 30, 5],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-2.0, -2.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [32, 3, 31, 5],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 2.005, -1.995],
                    uv: [30, 2, 32, 3],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_9",
        right: EyeSideRig {
            eye: [-2.0, 1.5, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.0, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [34, 5, 33, 7],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.5, 0.0],
                quad: RigBox {
                    min: [0.0, -3.0, -1.002],
                    max: [2.0, 0.0, -1.0],
                    uv: [33, 5, 32, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.5, 1.0],
                blink: [0.0, -3.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 3.005, -1.995],
                    uv: [32, 4, 34, 5],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 1.5, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.0, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -1.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [35, 5, 34, 7],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.5, 0.0],
                quad: RigBox {
                    min: [-2.0, -3.0, -1.002],
                    max: [0.0, 0.0, -1.0],
                    uv: [36, 5, 35, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.5, 1.0],
                blink: [0.0, -3.0, 0.0],
                quad: RigBox {
                    min: [-1.005, -0.005, -2.005],
                    max: [1.005, 3.005, -1.995],
                    uv: [34, 4, 36, 5],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_10",
        right: EyeSideRig {
            eye: [-2.0, 1.0, 0.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -0.004],
                    max: [0.504, 1.004, 0.004],
                    uv: [36, 3, 35, 4],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-3.0, -1.0, -0.002],
                    max: [0.0, 0.0, 0.0],
                    uv: [32, 3, 35, 4],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.015, 0.0],
                quad: RigBox {
                    min: [-2.005, 0.01, -1.005],
                    max: [1.005, 1.02, -0.995],
                    uv: [32, 2, 35, 3],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 1.0, 0.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -0.004],
                    max: [0.504, 1.004, 0.004],
                    uv: [37, 3, 36, 4],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -1.0, -0.002],
                    max: [3.0, 0.0, 0.0],
                    uv: [37, 3, 40, 4],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.015, 0.0],
                quad: RigBox {
                    min: [-1.005, 0.01, -1.005],
                    max: [2.005, 1.02, -0.995],
                    uv: [37, 2, 40, 3],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_11",
        right: EyeSideRig {
            eye: [-2.0, 0.0, 0.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -0.004],
                    max: [0.504, 1.004, 0.004],
                    uv: [36, 1, 35, 2],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-3.0, -1.0, -0.002],
                    max: [0.0, 0.0, 0.0],
                    uv: [32, 1, 35, 2],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.015, 0.0],
                quad: RigBox {
                    min: [-2.005, 0.01, -1.005],
                    max: [1.005, 1.02, -0.995],
                    uv: [32, 0, 35, 1],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.0, 0.0, 0.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -0.004],
                    max: [0.504, 1.004, 0.004],
                    uv: [37, 1, 36, 2],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [0.0, -1.0, -0.002],
                    max: [3.0, 0.0, 0.0],
                    uv: [37, 1, 40, 2],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.015, 0.0],
                quad: RigBox {
                    min: [-1.005, 0.01, -1.005],
                    max: [2.005, 1.02, -0.995],
                    uv: [37, 0, 40, 1],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_12",
        right: EyeSideRig {
            eye: [-2.0, 2.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [3, 7, 2, 8],
                },
            }),
            white: None,
            lid: None,
        },
        left: EyeSideRig {
            eye: [2.0, 2.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [4, 7, 3, 8],
                },
            }),
            white: None,
            lid: None,
        },
    },
    EyeRig {
        id: "eye_13",
        right: EyeSideRig {
            eye: [-2.0, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [3, 5, 2, 6],
                },
            }),
            white: None,
            lid: None,
        },
        left: EyeSideRig {
            eye: [2.0, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [4, 5, 3, 6],
                },
            }),
            white: None,
            lid: None,
        },
    },
    EyeRig {
        id: "eye_14",
        right: EyeSideRig {
            eye: [-2.0, 0.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [3, 3, 2, 4],
                },
            }),
            white: None,
            lid: None,
        },
        left: EyeSideRig {
            eye: [2.0, 0.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [4, 3, 3, 4],
                },
            }),
            white: None,
            lid: None,
        },
    },
    EyeRig {
        id: "eye_15",
        right: EyeSideRig {
            eye: [-2.0, -1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [3, 1, 2, 2],
                },
            }),
            white: None,
            lid: None,
        },
        left: EyeSideRig {
            eye: [2.0, -1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.504, -0.004, -1.004],
                    max: [0.504, 1.004, -0.996],
                    uv: [4, 1, 3, 2],
                },
            }),
            white: None,
            lid: None,
        },
    },
    EyeRig {
        id: "eye_16",
        right: EyeSideRig {
            eye: [-2.975, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.529, -0.004, -1.004],
                    max: [0.479, 1.004, -0.996],
                    uv: [38, 7, 37, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [-1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-0.025, -1.0, -1.002],
                    max: [1.975, 0.0, -1.0],
                    uv: [37, 7, 36, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 0.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-1.03, -0.005, -1.005],
                    max: [0.98, 1.005, -0.995],
                    uv: [36, 6, 38, 7],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.975, 1.0, 1.0],
            pupil: Some(PupilRig {
                pupil: [-0.5, 0.5, 0.0],
                inner: [0.5, 0.0, 0.0],
                outer: [-1.0, 0.0, 0.0],
                down: [0.5, 0.5, 0.0],
                up: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.479, -0.004, -1.004],
                    max: [0.529, 1.004, -0.996],
                    uv: [39, 7, 38, 8],
                },
            }),
            white: Some(WhiteRig {
                pivot: [1.0, 1.0, 0.0],
                quad: RigBox {
                    min: [-1.975, -1.0, -1.002],
                    max: [0.025, 0.0, -1.0],
                    uv: [40, 7, 39, 8],
                },
            }),
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 0.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.98, -0.005, -1.005],
                    max: [1.03, 1.005, -0.995],
                    uv: [38, 6, 40, 7],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_17",
        right: EyeSideRig {
            eye: [-2.475, 0.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.48, 1.005, -0.997],
                    uv: [0, 7, 1, 8],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.475, 0.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.48, -0.005, -1.007],
                    max: [0.53, 1.005, -0.997],
                    uv: [1, 7, 2, 8],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_18",
        right: EyeSideRig {
            eye: [-2.475, -1.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.48, 1.005, -0.997],
                    uv: [0, 6, 1, 7],
                },
            }),
        },
        left: EyeSideRig {
            eye: [2.475, -1.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.48, -0.005, -1.007],
                    max: [0.53, 1.005, -0.997],
                    uv: [1, 6, 2, 7],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_19",
        right: EyeSideRig {
            eye: [-1.475, 0.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.48, 1.005, -0.997],
                    uv: [0, 5, 1, 6],
                },
            }),
        },
        left: EyeSideRig {
            eye: [1.475, 0.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.48, -0.005, -1.007],
                    max: [0.53, 1.005, -0.997],
                    uv: [1, 5, 2, 6],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_20",
        right: EyeSideRig {
            eye: [-1.475, -1.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.48, 1.005, -0.997],
                    uv: [0, 4, 1, 5],
                },
            }),
        },
        left: EyeSideRig {
            eye: [1.475, -1.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 1.0, 1.0],
                blink: [0.0, -1.0, 0.0],
                quad: RigBox {
                    min: [-0.48, -0.005, -1.007],
                    max: [0.53, 1.005, -0.997],
                    uv: [1, 4, 2, 5],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_21",
        right: EyeSideRig {
            eye: [-1.475, 0.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 2.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.505, 2.005, -0.997],
                    uv: [0, 3, 1, 4],
                },
            }),
        },
        left: EyeSideRig {
            eye: [1.475, 0.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 2.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-0.505, -0.005, -1.007],
                    max: [0.53, 2.005, -0.997],
                    uv: [1, 3, 2, 4],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_22",
        right: EyeSideRig {
            eye: [-1.475, -1.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 2.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.505, 2.005, -0.997],
                    uv: [0, 2, 1, 3],
                },
            }),
        },
        left: EyeSideRig {
            eye: [1.475, -1.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 2.0, 1.0],
                blink: [0.0, -2.0, 0.0],
                quad: RigBox {
                    min: [-0.505, -0.005, -1.007],
                    max: [0.53, 2.005, -0.997],
                    uv: [1, 2, 2, 3],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_23",
        right: EyeSideRig {
            eye: [-1.475, -1.0, 1.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 3.0, 1.0],
                blink: [0.0, -3.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -2.007],
                    max: [0.48, 3.005, -1.997],
                    uv: [0, 1, 1, 2],
                },
            }),
        },
        left: EyeSideRig {
            eye: [1.475, -1.0, 1.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 3.0, 1.0],
                blink: [0.0, -3.0, 0.0],
                quad: RigBox {
                    min: [-0.48, -0.005, -2.007],
                    max: [0.53, 3.005, -1.997],
                    uv: [1, 1, 2, 2],
                },
            }),
        },
    },
    EyeRig {
        id: "eye_24",
        right: EyeSideRig {
            eye: [-1.475, -2.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 3.0, 1.0],
                blink: [0.0, -3.0, 0.0],
                quad: RigBox {
                    min: [-0.53, -0.005, -1.007],
                    max: [0.48, 3.005, -0.997],
                    uv: [0, 0, 1, 1],
                },
            }),
        },
        left: EyeSideRig {
            eye: [1.475, -2.0, 0.0],
            pupil: None,
            white: None,
            lid: Some(LidRig {
                eyelid: [0.0, 3.0, 1.0],
                blink: [0.0, -3.0, 0.0],
                quad: RigBox {
                    min: [-0.48, -0.005, -1.007],
                    max: [0.53, 3.005, -0.997],
                    uv: [1, 0, 2, 1],
                },
            }),
        },
    },
];
