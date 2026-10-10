use super::*;

pub(super) fn texture_ids_for_gpu_scene(
    text_ui: &TextUi,
    ctx: &Context,
    scene: &TextGpuScene,
) -> HashMap<usize, TextureId> {
    let sampling = text_ui.graphics_config().atlas_sampling;
    let current_frame = ctx.cumulative_frame_nr();
    let cache = gpu_scene_texture_cache(ctx);
    let mut texture_ids = HashMap::new();
    let mut cache_guard = cache.lock().expect("textui_egui texture cache poisoned");
    if current_frame > cache_guard.last_eviction_frame {
        cache_guard.last_eviction_frame = current_frame;
        cache_guard.retain_textures(|entry| {
            current_frame.saturating_sub(entry.last_used_frame)
                <= GPU_SCENE_TEXTURE_CACHE_STALE_FRAMES
        });
    }

    for page in &scene.atlas_pages {
        let key = hash_gpu_scene_page(page, sampling);
        if !cache_guard.entries.contains_key(&key) {
            let handle = ctx.load_texture(
                format!("textui_egui_gpu_scene_{key:016x}"),
                egui::ColorImage::from_rgba_premultiplied(page.size_px, &page.rgba8),
                texture_options_for_sampling(sampling),
            );
            cache_guard.bytes += handle.byte_size();
            cache_guard.entries.insert(
                key,
                CachedGpuSceneTexture {
                    handle,
                    last_used_frame: current_frame,
                },
            );
            cache_guard.evict_textures_over_budget(current_frame);
        }
        if let Some(entry) = cache_guard.entries.get_mut(&key) {
            entry.last_used_frame = current_frame;
            texture_ids.insert(page.page_index, entry.handle.id());
        }
    }

    texture_ids
}

pub(super) fn add_gpu_quad(
    mesh: &mut egui::epaint::Mesh,
    positions: [egui::Pos2; 4],
    uvs: [egui::Pos2; 4],
    tint: Color32,
) {
    let start = mesh.vertices.len() as u32;
    for (pos, uv) in positions.into_iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: tint,
        });
    }
    mesh.indices
        .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
}

/// Optional affine transform applied to quad positions during painting.
/// When `None`, positions are used as-is (absolute mode).
pub(super) struct PaintTransform {
    pub(super) offset: [f32; 2],
    pub(super) scale: [f32; 2],
}

pub(super) fn paint_gpu_scene_impl(
    text_ui: &TextUi,
    painter: &Painter,
    scene: &TextGpuScene,
    tint: Color32,
    transform: Option<&PaintTransform>,
) {
    let texture_ids = texture_ids_for_gpu_scene(text_ui, painter.ctx(), scene);
    let draw_options = if let Some(t) = transform {
        textui::TextGpuSceneDrawOptions {
            offset: textui::TextPoint::new(t.offset[0], t.offset[1]),
            scale: textui::TextVector::new(t.scale[0], t.scale[1]),
            tint: tint.into(),
        }
    } else {
        textui::TextGpuSceneDrawOptions {
            offset: textui::TextPoint::ZERO,
            scale: textui::TextVector::splat(1.0),
            tint: tint.into(),
        }
    };
    for batch in text_ui
        .prepare_gpu_scene_draw_batches(scene, draw_options)
        .iter()
    {
        let Some(texture_id) = texture_ids.get(&batch.page_index).copied() else {
            continue;
        };
        let mut mesh = egui::epaint::Mesh::with_texture(texture_id);
        for quad in batch.quads.iter() {
            let positions = quad.positions.map(|point| egui::pos2(point[0], point[1]));
            let uvs = quad.uvs.map(|point| egui::pos2(point[0], point[1]));
            let final_tint = Color32::from_rgba_premultiplied(
                quad.tint_rgba[0],
                quad.tint_rgba[1],
                quad.tint_rgba[2],
                quad.tint_rgba[3],
            );
            add_gpu_quad(&mut mesh, positions, uvs, final_tint);
        }
        if !mesh.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

pub(super) fn paint_gpu_scene_absolute(
    text_ui: &TextUi,
    painter: &Painter,
    scene: &TextGpuScene,
    tint: Color32,
) {
    paint_gpu_scene_impl(text_ui, painter, scene, tint, None);
}

pub(super) fn paint_gpu_scene_in_rect(
    text_ui: &TextUi,
    painter: &Painter,
    rect: Rect,
    scene: &TextGpuScene,
    tint: Color32,
) {
    let size = egui::vec2(scene.size_points[0], scene.size_points[1]);
    if size.x.abs() <= f32::EPSILON || size.y.abs() <= f32::EPSILON {
        return;
    }

    let rect = snap_rect_to_pixel_grid(rect, painter.pixels_per_point());
    let transform = PaintTransform {
        offset: [rect.min.x, rect.min.y],
        scale: [rect.width() / size.x, rect.height() / size.y],
    };
    paint_gpu_scene_impl(text_ui, painter, scene, tint, Some(&transform));
}
