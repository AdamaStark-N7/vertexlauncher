use super::*;

pub(super) fn hash_text_fundamentals(
    hasher: &mut DefaultHasher,
    fundamentals: &textui::TextFundamentals,
) {
    fundamentals.kerning.hash(hasher);
    fundamentals.stem_darkening.hash(hasher);
    fundamentals.standard_ligatures.hash(hasher);
    fundamentals.contextual_alternates.hash(hasher);
    fundamentals.discretionary_ligatures.hash(hasher);
    fundamentals.historical_ligatures.hash(hasher);
    fundamentals.case_sensitive_forms.hash(hasher);
    fundamentals.slashed_zero.hash(hasher);
    fundamentals.tabular_numbers.hash(hasher);
    fundamentals.smart_quotes.hash(hasher);
    fundamentals.letter_spacing_points.to_bits().hash(hasher);
    fundamentals.word_spacing_points.to_bits().hash(hasher);
    fundamentals.letter_spacing_floor.to_bits().hash(hasher);
    fundamentals.feature_settings.hash(hasher);
    fundamentals.variation_settings.hash(hasher);
    fundamentals.font_family.hash(hasher);
}

pub(super) fn hash_label_options(hasher: &mut DefaultHasher, options: &LabelOptions) {
    options.font_size.to_bits().hash(hasher);
    options.line_height.to_bits().hash(hasher);
    options.color.hash(hasher);
    options.wrap.hash(hasher);
    options.monospace.hash(hasher);
    options.weight.hash(hasher);
    options.italic.hash(hasher);
    options.padding.x.to_bits().hash(hasher);
    options.padding.y.to_bits().hash(hasher);
    hash_text_fundamentals(hasher, &options.fundamentals);
    options.ellipsis.hash(hasher);
}

pub(super) fn hash_label_scene_request(
    text: &str,
    options: &LabelOptions,
    width_points_opt: Option<f32>,
    scale: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    "label_scene".hash(&mut hasher);
    text.hash(&mut hasher);
    hash_label_options(&mut hasher, options);
    width_points_opt.map(f32::to_bits).hash(&mut hasher);
    scale.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub(super) fn hash_code_block_scene_request(
    code: &str,
    options: &CodeBlockOptions,
    width_points_opt: Option<f32>,
    scale: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    "code_block_scene".hash(&mut hasher);
    code.hash(&mut hasher);
    options.font_size.to_bits().hash(&mut hasher);
    options.line_height.to_bits().hash(&mut hasher);
    options.text_color.hash(&mut hasher);
    options.wrap.hash(&mut hasher);
    options.language.hash(&mut hasher);
    options.padding.x.to_bits().hash(&mut hasher);
    options.padding.y.to_bits().hash(&mut hasher);
    hash_text_fundamentals(&mut hasher, &options.fundamentals);
    width_points_opt.map(f32::to_bits).hash(&mut hasher);
    scale.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub(super) fn hash_rich_text_scene_request(
    spans: &[RichTextSpan],
    options: &LabelOptions,
    width_points_opt: Option<f32>,
    scale: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    "rich_text_scene".hash(&mut hasher);
    for span in spans {
        span.text.hash(&mut hasher);
        span.style.color.hash(&mut hasher);
        span.style.monospace.hash(&mut hasher);
        span.style.italic.hash(&mut hasher);
        span.style.weight.hash(&mut hasher);
    }
    hash_label_options(&mut hasher, options);
    width_points_opt.map(f32::to_bits).hash(&mut hasher);
    scale.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub(super) fn hash_text_render_scene(scene: &TextRenderScene) -> u64 {
    let mut hasher = DefaultHasher::new();
    "text_render_scene".hash(&mut hasher);
    scene.bounds.min.x.to_bits().hash(&mut hasher);
    scene.bounds.min.y.to_bits().hash(&mut hasher);
    scene.bounds.max.x.to_bits().hash(&mut hasher);
    scene.bounds.max.y.to_bits().hash(&mut hasher);
    scene.size_points.x.to_bits().hash(&mut hasher);
    scene.size_points.y.to_bits().hash(&mut hasher);
    for quad in &scene.quads {
        quad.atlas_page_index.hash(&mut hasher);
        for point in quad.positions {
            point.x.to_bits().hash(&mut hasher);
            point.y.to_bits().hash(&mut hasher);
        }
        for point in quad.uvs {
            point.x.to_bits().hash(&mut hasher);
            point.y.to_bits().hash(&mut hasher);
        }
        quad.tint.hash(&mut hasher);
        quad.is_color.hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn retained_gpu_scene_for_render_scene(
    text_ui: &TextUi,
    ctx: &Context,
    scene: &TextRenderScene,
) -> Arc<TextGpuScene> {
    let fingerprint = hash_text_render_scene(scene);
    retained_gpu_scene(
        ctx,
        Id::new(("textui_render_scene", fingerprint)),
        fingerprint,
        || {
            let mut gpu_scene = Arc::new(text_ui.gpu_scene_for_scene(scene));
            if let Some(scene) = Arc::get_mut(&mut gpu_scene) {
                scene.fingerprint = fingerprint;
            }
            Some(gpu_scene)
        },
    )
    .expect("text render scene conversion should always produce a gpu scene")
}

pub(super) fn hash_gpu_scene_page(page: &TextAtlasPageData, sampling: TextAtlasSampling) -> u64 {
    let mut hasher = DefaultHasher::new();
    page.content_hash.hash(&mut hasher);
    match sampling {
        TextAtlasSampling::Linear => 0_u8,
        TextAtlasSampling::Nearest => 1_u8,
    }
    .hash(&mut hasher);
    hasher.finish()
}
