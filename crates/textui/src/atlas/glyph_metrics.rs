use super::*;

pub(crate) fn hash_text_fundamentals<H: Hasher>(fundamentals: &TextFundamentals, state: &mut H) {
    fundamentals.kerning.hash(state);
    fundamentals.stem_darkening.hash(state);
    fundamentals.standard_ligatures.hash(state);
    fundamentals.contextual_alternates.hash(state);
    fundamentals.discretionary_ligatures.hash(state);
    fundamentals.historical_ligatures.hash(state);
    fundamentals.case_sensitive_forms.hash(state);
    fundamentals.slashed_zero.hash(state);
    fundamentals.tabular_numbers.hash(state);
    fundamentals.smart_quotes.hash(state);
    fundamentals.letter_spacing_points.to_bits().hash(state);
    fundamentals.word_spacing_points.to_bits().hash(state);
    fundamentals.letter_spacing_floor.to_bits().hash(state);
    fundamentals.feature_settings.len().hash(state);
    for feature in &fundamentals.feature_settings {
        feature.hash(state);
    }
    fundamentals.font_family.hash(state);
    fundamentals.cjk_font_family.hash(state);
    fundamentals.emoji_font_family.hash(state);
    fundamentals.variation_settings.len().hash(state);
    for variation in &fundamentals.variation_settings {
        variation.hash(state);
    }
}

pub(crate) fn shared_variation_settings(
    fundamentals: &TextFundamentals,
) -> Arc<[TextVariationSetting]> {
    Arc::from(fundamentals.variation_settings.clone().into_boxed_slice())
}

pub(super) fn glyph_cluster_text<'a>(run_text: &'a str, glyph: &LayoutGlyph) -> &'a str {
    run_text.get(glyph.start..glyph.end).unwrap_or_default()
}

pub(super) fn spacing_after_glyph_points(
    run_text: &str,
    glyph: &LayoutGlyph,
    glyph_index: usize,
    glyph_count: usize,
    fundamentals: &TextFundamentals,
) -> f32 {
    if glyph_index + 1 >= glyph_count {
        return 0.0;
    }
    let mut spacing = fundamentals
        .letter_spacing_points
        .max(fundamentals.letter_spacing_floor);
    let cluster = glyph_cluster_text(run_text, glyph);
    if !cluster.is_empty() && cluster.chars().all(char::is_whitespace) {
        spacing += fundamentals.word_spacing_points.max(0.0);
    }
    spacing
}

pub(super) fn spacing_after_glyph_pixels(
    run_text: &str,
    glyph: &LayoutGlyph,
    glyph_index: usize,
    glyph_count: usize,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> f32 {
    spacing_after_glyph_points(run_text, glyph, glyph_index, glyph_count, fundamentals) * scale
}

pub(crate) fn collect_glyph_spacing_prefixes_px(
    run_text: &str,
    glyphs: &[LayoutGlyph],
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Vec<f32> {
    let mut prefixes = Vec::with_capacity(glyphs.len());
    let mut extra_px = 0.0;
    for (glyph_index, glyph) in glyphs.iter().enumerate() {
        prefixes.push(extra_px);
        extra_px += spacing_after_glyph_pixels(
            run_text,
            glyph,
            glyph_index,
            glyphs.len(),
            fundamentals,
            scale,
        );
    }
    prefixes
}

pub(crate) fn adjusted_glyph_x_px(glyph: &LayoutGlyph, prefix_px: f32) -> f32 {
    glyph.x + prefix_px
}

pub(crate) fn adjusted_glyph_right_px(glyph: &LayoutGlyph, prefix_px: f32) -> f32 {
    adjusted_glyph_x_px(glyph, prefix_px) + glyph.w
}

pub(super) fn run_cursor_from_glyph_right(run: &LayoutRun<'_>, glyph: &LayoutGlyph) -> Cursor {
    if run.rtl {
        Cursor::new_with_affinity(run.line_i, glyph.start, Affinity::After)
    } else {
        Cursor::new_with_affinity(run.line_i, glyph.end, Affinity::Before)
    }
}

pub(super) fn run_cursor_stops(
    run: &LayoutRun<'_>,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Vec<(Cursor, f32)> {
    cursor_stops_for_glyphs(run.line_i, run.text, run.glyphs, fundamentals, scale)
}

pub(crate) fn cursor_stops_for_glyphs(
    line_i: usize,
    text: &str,
    glyphs: &[LayoutGlyph],
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Vec<(Cursor, f32)> {
    let mut stops = Vec::new();
    if glyphs.is_empty() {
        stops.push((Cursor::new(line_i, 0), 0.0));
        return stops;
    }

    let prefixes = collect_glyph_spacing_prefixes_px(text, glyphs, fundamentals, scale);
    for (glyph_index, glyph) in glyphs.iter().enumerate() {
        let cluster = glyph_cluster_text(text, glyph);
        let graphemes = cluster.grapheme_indices(true).collect::<Vec<_>>();
        let total = graphemes.len().max(1);
        let glyph_x = adjusted_glyph_x_px(glyph, prefixes[glyph_index]);

        for step in 0..=total {
            let cursor_index = graphemes
                .get(step)
                .map_or(glyph.end, |(offset, _)| glyph.start + *offset);
            let offset = glyph.w * step as f32 / total as f32;
            let x = if glyph.level.is_rtl() {
                glyph_x + glyph.w - offset
            } else {
                glyph_x + offset
            };
            stops.push((Cursor::new(line_i, cursor_index), x));
        }
    }

    stops.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.index.cmp(&b.0.index))
    });
    stops.dedup_by(|a, b| a.0 == b.0 && (a.1 - b.1).abs() <= 0.25);
    stops
}

pub(crate) fn hit_buffer_with_fundamentals(
    buffer: &Buffer,
    x: f32,
    y: f32,
    fundamentals: &TextFundamentals,
    scale: f32,
) -> Option<Cursor> {
    let mut new_cursor_opt = None;

    let mut runs = buffer.layout_runs().peekable();
    let mut first_run = true;
    while let Some(run) = runs.next() {
        let line_top = run.line_top;
        let line_height = run.line_height;

        if first_run && y < line_top {
            first_run = false;
            new_cursor_opt = Some(Cursor::new(run.line_i, 0));
        } else if y >= line_top && y < line_top + line_height {
            let stops = run_cursor_stops(&run, fundamentals, scale);
            if let Some((first_cursor, first_x)) = stops.first().copied() {
                if x <= first_x {
                    return Some(first_cursor);
                }
            }

            for window in stops.windows(2) {
                let (left_cursor, left_x) = window[0];
                let (right_cursor, right_x) = window[1];
                let mid_x = (left_x + right_x) * 0.5;
                if x <= mid_x {
                    return Some(left_cursor);
                }
                if x <= right_x {
                    return Some(right_cursor);
                }
            }

            if let Some((last_cursor, _)) = stops.last().copied() {
                return Some(last_cursor);
            }

            return Some(Cursor::new(run.line_i, 0));
        } else if runs.peek().is_none() && y > run.line_y {
            if let Some(glyph) = run.glyphs.last() {
                new_cursor_opt = Some(run_cursor_from_glyph_right(&run, glyph));
            } else {
                new_cursor_opt = Some(Cursor::new(run.line_i, 0));
            }
        }
    }

    new_cursor_opt
}

pub(crate) fn collect_prepared_glyphs_from_buffer(
    buffer: &Buffer,
    scale: f32,
    default_color: Color32,
    fundamentals: &TextFundamentals,
) -> (Vec<PreparedGlyph>, f32) {
    let mut glyphs = Vec::new();
    let mut max_line_extra_points: f32 = 0.0;
    let variation_settings = shared_variation_settings(fundamentals);

    for run in buffer.layout_runs() {
        let baseline_y_px = run.line_y as i32;
        let mut line_extra_points = 0.0;

        for (glyph_index, glyph) in run.glyphs.iter().enumerate() {
            let physical = glyph.physical((0.0, 0.0), 1.0);
            glyphs.push(PreparedGlyph {
                cache_key: GlyphRasterKey::new(
                    physical.cache_key,
                    scale,
                    fundamentals.stem_darkening,
                    GlyphContentMode::AlphaMask,
                    0.0,
                    Arc::clone(&variation_settings),
                ),
                offset_points: egui::vec2(
                    physical.x as f32 / scale + line_extra_points,
                    (baseline_y_px + physical.y) as f32 / scale,
                ),
                color: glyph.color_opt.map_or(default_color, cosmic_to_egui_color),
            });
            line_extra_points += spacing_after_glyph_points(
                run.text,
                glyph,
                glyph_index,
                run.glyphs.len(),
                fundamentals,
            );
        }

        max_line_extra_points = max_line_extra_points.max(line_extra_points);
    }

    (glyphs, max_line_extra_points)
}
