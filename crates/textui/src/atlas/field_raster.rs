use super::*;

/// Returns true when two or more contours wind in the same direction and their bounding boxes
/// overlap. This indicates overlapping filled regions that our even-odd `point_inside_outline`
/// will mis-classify as "outside", making SDF/MSDF distances incorrect.
///
/// Counter-wound nested contours (holes, like the inside of "O") wind opposite to their parent,
/// so they are correctly excluded by the same-winding filter.
pub(super) fn outline_has_same_winding_overlap(contours: &[Vec<[f32; 2]>]) -> bool {
    if contours.len() < 2 {
        return false;
    }

    // Compute signed area (shoelace) and bounding box per contour.
    // Positive area → CCW in the swash coordinate space; negative → CW.
    let mut winding = Vec::with_capacity(contours.len());
    let mut bboxes: Vec<([f32; 2], [f32; 2])> = Vec::with_capacity(contours.len());
    for contour in contours {
        let mut min = [f32::INFINITY, f32::INFINITY];
        let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
        let mut area = 0.0f32;
        let n = contour.len();
        for i in 0..n {
            let a = contour[i];
            let b = contour[(i + 1) % n];
            area += (b[0] - a[0]) * (b[1] + a[1]);
            min[0] = min[0].min(a[0]);
            min[1] = min[1].min(a[1]);
            max[0] = max[0].max(a[0]);
            max[1] = max[1].max(a[1]);
        }
        winding.push(area.signum());
        bboxes.push((min, max));
    }

    for i in 0..contours.len() {
        for j in (i + 1)..contours.len() {
            if winding[i] != winding[j] {
                continue;
            }
            let (a_min, a_max) = bboxes[i];
            let (b_min, b_max) = bboxes[j];
            if a_min[0] < b_max[0]
                && a_max[0] > b_min[0]
                && a_min[1] < b_max[1]
                && a_max[1] > b_min[1]
            {
                return true;
            }
        }
    }
    false
}

pub(super) fn rasterize_field_glyph(
    font_system: &mut FontSystem,
    scale_context: &mut ScaleContext,
    cache_key: &GlyphRasterKey,
    rasterization: TextRasterizationConfig,
    padding_px: usize,
) -> Option<PreparedAtlasGlyph> {
    let commands =
        render_swash_outline_commands(font_system, scale_context, cache_key, rasterization)?;
    let outline = flatten_outline_commands_for_field(&commands, cache_key.content_mode());
    if outline.segments.is_empty()
        || !outline.min[0].is_finite()
        || !outline.min[1].is_finite()
        || !outline.max[0].is_finite()
        || !outline.max[1].is_finite()
    {
        return None;
    }

    // Glyphs with overlapping same-winding contours (common in CJK, e.g. "经") fail SDF/MSDF
    // rendering because point_inside_outline uses the even-odd rule. Under even-odd, regions
    // covered by two same-winding contours flip back to "outside", producing wrong signed
    // distances and a glyph that renders too dark. field_glyph_matches_reference would catch
    // this, but only after the expensive O(width * height * segments) pixel loop and a full
    // reference rasterization. Detect the failure condition up-front from the contour geometry.
    if outline_has_same_winding_overlap(&outline.contours) {
        return None;
    }

    let field_range_px = cache_key.field_range_px().max(1.0);
    let left = (outline.min[0] - field_range_px).floor() as i32;
    let bottom = (outline.min[1] - field_range_px).floor() as i32;
    let right = (outline.max[0] + field_range_px).ceil() as i32;
    let top = (outline.max[1] + field_range_px).ceil() as i32;
    let glyph_width = (right - left).max(1) as usize;
    let glyph_height = (top - bottom).max(1) as usize;

    let mut glyph_image = ColorImage::filled([glyph_width, glyph_height], Color32::TRANSPARENT);
    for y in 0..glyph_height {
        for x in 0..glyph_width {
            let sample = [left as f32 + x as f32 + 0.5, top as f32 - y as f32 - 0.5];
            let inside = point_inside_outline(sample, &outline.contours);
            let rgba = match cache_key.content_mode() {
                GlyphContentMode::Sdf => {
                    encode_sdf_sample(sample, inside, &outline.segments, field_range_px)
                }
                GlyphContentMode::Msdf => {
                    encode_msdf_sample(sample, inside, &outline.segments, field_range_px)
                }
                GlyphContentMode::AlphaMask => return None,
            };
            glyph_image.pixels[y * glyph_width + x] =
                Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
        }
    }

    if !field_glyph_matches_reference(
        font_system,
        scale_context,
        cache_key,
        rasterization,
        &glyph_image,
        left,
        top,
        field_range_px,
    ) {
        return None;
    }

    let upload_image = build_atlas_upload_image(&glyph_image, padding_px);
    Some(PreparedAtlasGlyph {
        approx_bytes: color_image_byte_size(&upload_image),
        upload_image,
        size_px: [glyph_width, glyph_height],
        placement_left_px: left,
        placement_top_px: top,
        is_color: false,
        content_mode: cache_key.content_mode(),
    })
}

pub(super) fn field_glyph_matches_reference(
    font_system: &mut FontSystem,
    scale_context: &mut ScaleContext,
    cache_key: &GlyphRasterKey,
    rasterization: TextRasterizationConfig,
    field_image: &ColorImage,
    field_left: i32,
    field_top: i32,
    field_range_px: f32,
) -> bool {
    let Some(reference) = render_swash_image(font_system, scale_context, cache_key, rasterization)
    else {
        return false;
    };
    let Some(reference_image) = swash_image_to_color_image(&reference) else {
        return false;
    };

    let reference_left = reference.placement.left;
    let reference_top = reference.placement.top;
    let field_bottom = field_top - field_image.size[1] as i32;
    let reference_bottom = reference_top - reference_image.size[1] as i32;
    let compare_left = field_left.min(reference_left);
    let compare_right = (field_left + field_image.size[0] as i32)
        .max(reference_left + reference_image.size[0] as i32);
    let compare_bottom = field_bottom.min(reference_bottom);
    let compare_top = field_top.max(reference_top);
    let total_pixels =
        ((compare_right - compare_left).max(0) * (compare_top - compare_bottom).max(0)) as usize;
    if total_pixels == 0 {
        return false;
    }

    let mut total_error = 0.0;
    let mut max_error = 0.0_f32;
    let mut large_error_pixels = 0usize;
    for y in compare_bottom..compare_top {
        for x in compare_left..compare_right {
            let reference_alpha =
                sample_color_image_alpha(reference_left, reference_top, &reference_image, x, y);
            let field_alpha = sample_field_image_alpha(
                cache_key.content_mode(),
                field_left,
                field_top,
                field_image,
                x,
                y,
                field_range_px,
            );
            let error = (field_alpha - reference_alpha).abs();
            total_error += error;
            max_error = max_error.max(error);
            if error > FIELD_GLYPH_MAX_ALPHA_ERROR_LIMIT {
                large_error_pixels += 1;
            }
        }
    }

    let mean_error = total_error / total_pixels as f32;
    let large_error_ratio = large_error_pixels as f32 / total_pixels as f32;
    mean_error <= FIELD_GLYPH_MEAN_ALPHA_ERROR_LIMIT
        && max_error <= FIELD_GLYPH_MAX_ALPHA_ERROR_LIMIT
        && large_error_ratio <= FIELD_GLYPH_LARGE_ERROR_PIXEL_RATIO_LIMIT
}

pub(super) fn flatten_outline_commands_for_field(
    commands: &[swash::zeno::Command],
    content_mode: GlyphContentMode,
) -> FlattenedOutline {
    let mut outline = FlattenedOutline::new();
    let mut current = [0.0, 0.0];
    let mut contour_start = [0.0, 0.0];
    let mut contour_points = Vec::<[f32; 2]>::new();
    let mut contour_segments = Vec::<([f32; 2], [f32; 2])>::new();

    let flush_contour = |outline: &mut FlattenedOutline,
                         contour_points: &mut Vec<[f32; 2]>,
                         contour_segments: &mut Vec<([f32; 2], [f32; 2])>| {
        if contour_segments.is_empty() {
            contour_points.clear();
            return;
        }
        if contour_points.len() >= 3 {
            outline.contours.push(contour_points.clone());
        }
        let colors = [1_u8, 2_u8, 4_u8];
        for (index, (a, b)) in contour_segments.iter().copied().enumerate() {
            outline.include_point(a);
            outline.include_point(b);
            let color_mask = match content_mode {
                GlyphContentMode::AlphaMask | GlyphContentMode::Sdf => 0b111,
                GlyphContentMode::Msdf => colors[index % colors.len()],
            };
            outline.segments.push(FieldLineSegment { a, b, color_mask });
        }
        contour_points.clear();
        contour_segments.clear();
    };

    for command in commands {
        match *command {
            swash::zeno::Command::MoveTo(point) => {
                flush_contour(&mut outline, &mut contour_points, &mut contour_segments);
                current = [point.x, point.y];
                contour_start = current;
                contour_points.push(current);
            }
            swash::zeno::Command::LineTo(point) => {
                let next = [point.x, point.y];
                contour_segments.push((current, next));
                contour_points.push(next);
                current = next;
            }
            swash::zeno::Command::QuadTo(control, point) => {
                let next = [point.x, point.y];
                let control = [control.x, control.y];
                let steps = curve_steps(current, control, control, next);
                let mut prev = current;
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let p = eval_quad(current, control, next, t);
                    contour_segments.push((prev, p));
                    contour_points.push(p);
                    prev = p;
                }
                current = next;
            }
            swash::zeno::Command::CurveTo(control_a, control_b, point) => {
                let next = [point.x, point.y];
                let control_a = [control_a.x, control_a.y];
                let control_b = [control_b.x, control_b.y];
                let steps = curve_steps(current, control_a, control_b, next);
                let mut prev = current;
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let p = eval_cubic(current, control_a, control_b, next, t);
                    contour_segments.push((prev, p));
                    contour_points.push(p);
                    prev = p;
                }
                current = next;
            }
            swash::zeno::Command::Close => {
                if current != contour_start {
                    contour_segments.push((current, contour_start));
                    contour_points.push(contour_start);
                    current = contour_start;
                }
                flush_contour(&mut outline, &mut contour_points, &mut contour_segments);
            }
        }
    }
    flush_contour(&mut outline, &mut contour_points, &mut contour_segments);
    outline
}

pub(super) fn curve_steps(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> usize {
    let len = point_distance(p0, p1) + point_distance(p1, p2) + point_distance(p2, p3);
    ((len / 6.0).ceil() as usize).clamp(4, 24)
}

pub(super) fn eval_quad(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], t: f32) -> [f32; 2] {
    let mt = 1.0 - t;
    [
        mt * mt * p0[0] + 2.0 * mt * t * p1[0] + t * t * p2[0],
        mt * mt * p0[1] + 2.0 * mt * t * p1[1] + t * t * p2[1],
    ]
}

pub(super) fn eval_cubic(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    t: f32,
) -> [f32; 2] {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;
    [
        mt2 * mt * p0[0] + 3.0 * mt2 * t * p1[0] + 3.0 * mt * t2 * p2[0] + t2 * t * p3[0],
        mt2 * mt * p0[1] + 3.0 * mt2 * t * p1[1] + 3.0 * mt * t2 * p2[1] + t2 * t * p3[1],
    ]
}

pub(super) fn point_distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

pub(super) fn point_inside_outline(point: [f32; 2], contours: &[Vec<[f32; 2]>]) -> bool {
    let mut inside = false;
    for contour in contours {
        if contour.len() < 3 {
            continue;
        }
        let mut j = contour.len() - 1;
        for i in 0..contour.len() {
            let a = contour[i];
            let b = contour[j];
            let intersects = ((a[1] > point[1]) != (b[1] > point[1]))
                && (point[0] < (b[0] - a[0]) * (point[1] - a[1]) / ((b[1] - a[1]) + 1e-6) + a[0]);
            if intersects {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

pub(super) fn encode_sdf_sample(
    point: [f32; 2],
    inside: bool,
    segments: &[FieldLineSegment],
    field_range_px: f32,
) -> [u8; 4] {
    let signed = signed_distance_to_segments(point, inside, segments.iter().copied());
    let encoded = encode_signed_distance(signed, field_range_px);
    [encoded, encoded, encoded, 255]
}

pub(super) fn encode_msdf_sample(
    point: [f32; 2],
    inside: bool,
    segments: &[FieldLineSegment],
    field_range_px: f32,
) -> [u8; 4] {
    let sign = if inside { 1.0 } else { -1.0 };
    let mut channel_distances = [field_range_px; 3];
    for segment in segments {
        let distance = distance_to_segment(point, segment.a, segment.b);
        if segment.color_mask & 1 != 0 {
            channel_distances[0] = channel_distances[0].min(distance);
        }
        if segment.color_mask & 2 != 0 {
            channel_distances[1] = channel_distances[1].min(distance);
        }
        if segment.color_mask & 4 != 0 {
            channel_distances[2] = channel_distances[2].min(distance);
        }
    }
    [
        encode_signed_distance(sign * channel_distances[0], field_range_px),
        encode_signed_distance(sign * channel_distances[1], field_range_px),
        encode_signed_distance(sign * channel_distances[2], field_range_px),
        255,
    ]
}

pub(super) fn signed_distance_to_segments(
    point: [f32; 2],
    inside: bool,
    segments: impl Iterator<Item = FieldLineSegment>,
) -> f32 {
    let mut min_distance = f32::INFINITY;
    for segment in segments {
        min_distance = min_distance.min(distance_to_segment(point, segment.a, segment.b));
    }
    if inside { min_distance } else { -min_distance }
}

pub(super) fn encode_signed_distance(distance: f32, field_range_px: f32) -> u8 {
    let normalized = (0.5 + 0.5 * (distance / field_range_px).clamp(-1.0, 1.0)).clamp(0.0, 1.0);
    (normalized * 255.0).round() as u8
}

pub(super) fn decode_signed_distance(encoded: u8, field_range_px: f32) -> f32 {
    (((encoded as f32) / 255.0) - 0.5) * 2.0 * field_range_px
}

pub(super) fn decode_field_alpha_from_pixel(
    content_mode: GlyphContentMode,
    pixel: Color32,
    field_range_px: f32,
) -> f32 {
    let signed_distance = match content_mode {
        GlyphContentMode::AlphaMask => pixel.a() as f32 / 255.0,
        GlyphContentMode::Sdf => decode_signed_distance(pixel.r(), field_range_px),
        GlyphContentMode::Msdf => {
            let red = decode_signed_distance(pixel.r(), field_range_px);
            let green = decode_signed_distance(pixel.g(), field_range_px);
            let blue = decode_signed_distance(pixel.b(), field_range_px);
            median3(red, green, blue)
        }
    };
    smoothstep(-0.5, 0.5, signed_distance)
}

pub(super) fn sample_color_image_alpha(
    left: i32,
    top: i32,
    image: &ColorImage,
    x: i32,
    y: i32,
) -> f32 {
    let Some(pixel) = color_image_pixel_at_world_position(left, top, image, x, y) else {
        return 0.0;
    };
    pixel.a() as f32 / 255.0
}

pub(super) fn sample_field_image_alpha(
    content_mode: GlyphContentMode,
    left: i32,
    top: i32,
    image: &ColorImage,
    x: i32,
    y: i32,
    field_range_px: f32,
) -> f32 {
    let Some(pixel) = color_image_pixel_at_world_position(left, top, image, x, y) else {
        return 0.0;
    };
    decode_field_alpha_from_pixel(content_mode, pixel, field_range_px)
}

pub(super) fn color_image_pixel_at_world_position(
    left: i32,
    top: i32,
    image: &ColorImage,
    x: i32,
    y: i32,
) -> Option<Color32> {
    let width = image.size[0] as i32;
    let height = image.size[1] as i32;
    let ix = x - left;
    let iy = top - 1 - y;
    if ix < 0 || iy < 0 || ix >= width || iy >= height {
        return None;
    }
    image
        .pixels
        .get(iy as usize * image.size[0] + ix as usize)
        .copied()
}

pub(super) fn median3(a: f32, b: f32, c: f32) -> f32 {
    a.max(b).min(a.min(b).max(c))
}

pub(super) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn distance_to_segment(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [point[0] - a[0], point[1] - a[1]];
    let denom = ab[0] * ab[0] + ab[1] * ab[1];
    if denom <= 1e-6 {
        return point_distance(point, a);
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / denom).clamp(0.0, 1.0);
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    point_distance(point, closest)
}
