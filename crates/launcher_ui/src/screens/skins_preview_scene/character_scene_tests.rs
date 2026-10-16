use super::*;

fn triangles_within_uv<'a>(
    scene: &'a BuiltCharacterScene,
    min: [u32; 2],
    max: [u32; 2],
) -> impl Iterator<Item = &'a RenderTriangle> {
    let lo = pos2(min[0] as f32 / 64.0, min[1] as f32 / 64.0);
    let hi = pos2(max[0] as f32 / 64.0, max[1] as f32 / 64.0);
    scene.triangles.iter().filter(move |tri| {
        tri.uv
            .iter()
            .all(|uv| uv.x >= lo.x && uv.x <= hi.x && uv.y >= lo.y && uv.y <= hi.y)
    })
}

fn mean_screen_x_for_uv(scene: &BuiltCharacterScene, min: [u32; 2], max: [u32; 2]) -> f32 {
    let xs: Vec<f32> = triangles_within_uv(scene, min, max)
        .flat_map(|tri| tri.pos.map(|pos| pos.x))
        .collect();
    assert!(!xs.is_empty(), "no triangles sample {min:?}..{max:?}");
    xs.iter().sum::<f32>() / xs.len() as f32
}

#[test]
fn right_limbs_render_on_the_same_side_as_the_right_half_of_the_face() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0));
    let skin = Arc::new(RgbaImage::from_pixel(
        64,
        64,
        image::Rgba([200, 150, 120, 255]),
    ));
    // Camera in front of the player.
    let scene = build_character_scene(
        rect,
        default_cape_uv_layout(),
        std::f32::consts::FRAC_PI_2,
        PreviewPose {
            time_seconds: 0.0,
            locomotion_blend: 0.0,
            limb_swing: 0.0,
        },
        MinecraftSkinVariant::Classic,
        false,
        false,
        false,
        None,
        Some(skin),
        None,
        None,
    );
    let center = rect.center().x;

    // The texture's left edge of the face is the player's right side, which faces the
    // viewer's left.
    let face_edge_x = |want_left: bool| {
        let xs: Vec<f32> = triangles_within_uv(&scene, [8, 8], [16, 16])
            .flat_map(|tri| tri.uv.into_iter().zip(tri.pos))
            .filter(|(uv, _)| {
                if want_left {
                    uv.x < 9.0 / 64.0
                } else {
                    uv.x > 15.0 / 64.0
                }
            })
            .map(|(_, pos)| pos.x)
            .collect();
        xs.iter().sum::<f32>() / xs.len().max(1) as f32
    };
    let right_arm_front = mean_screen_x_for_uv(&scene, [44, 20], [48, 32]);
    let right_leg_front = mean_screen_x_for_uv(&scene, [4, 20], [8, 32]);
    let left_arm_front = mean_screen_x_for_uv(&scene, [36, 52], [40, 64]);
    assert!(face_edge_x(true) < center && face_edge_x(false) > center);
    assert!(right_arm_front < center);
    assert!(right_leg_front < center);
    assert!(left_arm_front > center);
}

#[test]
fn walk_cycle_produces_finite_geometry() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0));
    let skin = Arc::new(RgbaImage::from_pixel(
        64,
        64,
        image::Rgba([200, 150, 120, 255]),
    ));
    for step in 0..32 {
        let pose = PreviewPose {
            time_seconds: step as f32 * 0.37,
            locomotion_blend: (step % 5) as f32 / 4.0,
            limb_swing: step as f32 * 1.9,
        };
        let scene = build_character_scene(
            rect,
            default_cape_uv_layout(),
            step as f32 * 0.4,
            pose,
            MinecraftSkinVariant::Slim,
            true,
            step % 2 == 0,
            true,
            None,
            Some(skin.clone()),
            None,
            None,
        );
        assert!(!scene.triangles.is_empty());
        assert!(scene.triangles.iter().all(|tri| {
            tri.pos
                .iter()
                .all(|pos| pos.x.is_finite() && pos.y.is_finite())
                && tri.depth.iter().all(|depth| depth.is_finite())
        }));
    }
}

/// Mirrors the scene shader's `texel_region` snapping.
fn snapped_texel_region(bounds: Rect, dims: [f32; 2]) -> ([i32; 2], [i32; 2]) {
    let min = [
        (bounds.min.x * dims[0] + 0.01).floor() as i32,
        (bounds.min.y * dims[1] + 0.01).floor() as i32,
    ];
    let max = [
        ((bounds.max.x * dims[0] - 0.01).ceil() as i32 - 1).max(min[0]),
        ((bounds.max.y * dims[1] - 0.01).ceil() as i32 - 1).max(min[1]),
    ];
    (min, max)
}

#[test]
fn every_face_samples_only_its_own_texel_region() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0));
    let skin = Arc::new(RgbaImage::from_pixel(
        64,
        64,
        image::Rgba([200, 150, 120, 255]),
    ));
    let cape = Arc::new(RgbaImage::from_pixel(
        64,
        32,
        image::Rgba([90, 90, 200, 255]),
    ));
    // Every eye layout's texels are opaque, so the classifier finds one.
    let layout = detect_expression_layout(&skin);
    for step in 0..12 {
        for show_elytra in [false, true] {
            let scene = build_character_scene(
                rect,
                default_cape_uv_layout(),
                step as f32 * 0.55,
                PreviewPose {
                    time_seconds: step as f32 * 0.4,
                    locomotion_blend: 1.0,
                    limb_swing: step as f32 * 2.3,
                },
                MinecraftSkinVariant::Classic,
                step % 2 == 0,
                show_elytra,
                true,
                layout,
                Some(skin.clone()),
                Some(cape.clone()),
                None,
            );
            for tri in &scene.triangles {
                let dims = match tri.texture {
                    TriangleTexture::Skin => [64.0, 64.0],
                    TriangleTexture::Cape => [64.0, 32.0],
                };
                let (min, max) = snapped_texel_region(tri.uv_bounds, dims);
                assert!(min[0] <= max[0] && min[1] <= max[1]);
                // The texel under the triangle's centroid must lie inside the region.
                let centroid =
                    (tri.uv[0].to_vec2() + tri.uv[1].to_vec2() + tri.uv[2].to_vec2()) / 3.0;
                let texel = [
                    (centroid.x * dims[0]).floor() as i32,
                    (centroid.y * dims[1]).floor() as i32,
                ];
                assert!(
                    (min[0]..=max[0]).contains(&texel[0]) && (min[1]..=max[1]).contains(&texel[1]),
                    "texel {texel:?} outside region {min:?}..={max:?}"
                );
            }
        }
    }
}
