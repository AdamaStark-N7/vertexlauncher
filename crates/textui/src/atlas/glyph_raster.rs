use super::*;

pub(super) fn glyph_atlas_worker_loop(
    rx: mpsc::Receiver<GlyphAtlasWorkerMessage>,
    tx: mpsc::Sender<GlyphAtlasWorkerResponse>,
) {
    let mut font_system = FontSystem::new();
    configure_text_font_defaults(&mut font_system);
    let mut scale_context = ScaleContext::new();

    while let Ok(message) = rx.recv() {
        match message {
            GlyphAtlasWorkerMessage::RegisterFont(bytes) => {
                font_system.db_mut().load_font_data(bytes);
            }
            GlyphAtlasWorkerMessage::Rasterize {
                generation,
                cache_key,
                rasterization,
                padding_px,
            } => {
                let glyph = rasterize_atlas_glyph(
                    &mut font_system,
                    &mut scale_context,
                    &cache_key,
                    rasterization,
                    padding_px,
                );
                let _ = tx.send(GlyphAtlasWorkerResponse {
                    generation,
                    cache_key,
                    glyph,
                });
            }
        }
    }
}

pub(crate) fn rasterize_atlas_glyph(
    font_system: &mut FontSystem,
    scale_context: &mut ScaleContext,
    cache_key: &GlyphRasterKey,
    rasterization: TextRasterizationConfig,
    padding_px: usize,
) -> Option<PreparedAtlasGlyph> {
    if cache_key.content_mode() != GlyphContentMode::AlphaMask
        && !cache_key
            .cache_key
            .flags
            .contains(cosmic_text::CacheKeyFlags::PIXEL_FONT)
    {
        if let Some(glyph) = rasterize_field_glyph(
            font_system,
            scale_context,
            cache_key,
            rasterization,
            padding_px,
        ) {
            return Some(glyph);
        }
    }

    let image = render_swash_image(font_system, scale_context, cache_key, rasterization)?;
    let glyph_width = image.placement.width as usize;
    let glyph_height = image.placement.height as usize;
    if glyph_width == 0 || glyph_height == 0 {
        return None;
    }

    let glyph_image = swash_image_to_color_image(&image)?;
    let upload_image = build_atlas_upload_image(&glyph_image, padding_px);
    Some(PreparedAtlasGlyph {
        approx_bytes: color_image_byte_size(&upload_image),
        upload_image,
        size_px: [glyph_width, glyph_height],
        placement_left_px: image.placement.left,
        placement_top_px: image.placement.top,
        is_color: matches!(image.content, SwashContent::Color),
        content_mode: GlyphContentMode::AlphaMask,
    })
}

pub(super) fn render_swash_image(
    font_system: &mut FontSystem,
    scale_context: &mut ScaleContext,
    raster_key: &GlyphRasterKey,
    rasterization: TextRasterizationConfig,
) -> Option<SwashImage> {
    let cache_key = raster_key.cache_key;
    let display_scale = raster_key.display_scale().max(1.0);
    let ppem = f32::from_bits(cache_key.font_size_bits);
    let logical_font_size = (ppem / display_scale).max(1.0);

    let font = font_system.get_font(cache_key.font_id, cache_key.font_weight)?;
    let swash_font = font.as_swash();

    let mut settings_by_tag = std::collections::BTreeMap::<[u8; 4], f32>::new();
    if let Some(variation) = swash_font
        .variations()
        .find_by_tag(SwashTag::from_be_bytes(*b"wght"))
    {
        settings_by_tag.insert(
            *b"wght",
            f32::from(cache_key.font_weight.0).clamp(variation.min_value(), variation.max_value()),
        );
    }
    if let Some(variation) = swash_font
        .variations()
        .find_by_tag(SwashTag::from_be_bytes(*b"opsz"))
    {
        if rasterization.optical_sizing != TextOpticalSizingMode::Disabled {
            settings_by_tag.insert(
                *b"opsz",
                opsz_for_font_size(logical_font_size)
                    .clamp(variation.min_value(), variation.max_value()),
            );
        }
    }
    for variation in raster_key.variation_settings.iter().copied() {
        let tag = SwashTag::from_be_bytes(variation.tag);
        if let Some(axis) = swash_font.variations().find_by_tag(tag) {
            settings_by_tag.insert(
                variation.tag,
                variation.value().clamp(axis.min_value(), axis.max_value()),
            );
        }
    }
    let settings = settings_by_tag
        .into_iter()
        .map(|(tag, value)| SwashSetting {
            tag: SwashTag::from_be_bytes(tag),
            value,
        })
        .collect::<Vec<_>>();

    let mut scaler = scale_context
        .builder(swash_font)
        .size(ppem)
        .hint(resolved_hinting_enabled(display_scale, rasterization));
    if !settings.is_empty() {
        scaler = scaler.variations(settings.into_iter());
    }
    let mut scaler = scaler.build();

    let offset = if cache_key
        .flags
        .contains(cosmic_text::CacheKeyFlags::PIXEL_FONT)
    {
        SwashVector::new(
            cache_key.x_bin.as_float().round() + 1.0,
            cache_key.y_bin.as_float().round(),
        )
    } else {
        SwashVector::new(cache_key.x_bin.as_float(), cache_key.y_bin.as_float())
    };

    let mut render = Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ]);
    render
        .format(SwashFormat::Alpha)
        .offset(offset)
        .embolden(resolved_stem_darkening_strength(
            ppem,
            raster_key.stem_darkening(),
            rasterization,
        ))
        .transform(
            if cache_key
                .flags
                .contains(cosmic_text::CacheKeyFlags::FAKE_ITALIC)
            {
                Some(SwashTransform::skew(
                    SwashAngle::from_degrees(14.0),
                    SwashAngle::from_degrees(0.0),
                ))
            } else {
                None
            },
        );

    render.render(&mut scaler, cache_key.glyph_id)
}

pub(crate) fn render_swash_outline_commands(
    font_system: &mut FontSystem,
    scale_context: &mut ScaleContext,
    raster_key: &GlyphRasterKey,
    rasterization: TextRasterizationConfig,
) -> Option<Box<[swash::zeno::Command]>> {
    use swash::zeno::PathData as _;

    let cache_key = raster_key.cache_key;
    let display_scale = raster_key.display_scale().max(1.0);
    let ppem = f32::from_bits(cache_key.font_size_bits);
    let logical_font_size = (ppem / display_scale).max(1.0);

    let font = font_system.get_font(cache_key.font_id, cache_key.font_weight)?;
    let swash_font = font.as_swash();

    let mut settings_by_tag = std::collections::BTreeMap::<[u8; 4], f32>::new();
    if let Some(variation) = swash_font
        .variations()
        .find_by_tag(SwashTag::from_be_bytes(*b"wght"))
    {
        settings_by_tag.insert(
            *b"wght",
            f32::from(cache_key.font_weight.0).clamp(variation.min_value(), variation.max_value()),
        );
    }
    if let Some(variation) = swash_font
        .variations()
        .find_by_tag(SwashTag::from_be_bytes(*b"opsz"))
    {
        if rasterization.optical_sizing != TextOpticalSizingMode::Disabled {
            settings_by_tag.insert(
                *b"opsz",
                opsz_for_font_size(logical_font_size)
                    .clamp(variation.min_value(), variation.max_value()),
            );
        }
    }
    for variation in raster_key.variation_settings.iter().copied() {
        let tag = SwashTag::from_be_bytes(variation.tag);
        if let Some(axis) = swash_font.variations().find_by_tag(tag) {
            settings_by_tag.insert(
                variation.tag,
                variation.value().clamp(axis.min_value(), axis.max_value()),
            );
        }
    }
    let settings = settings_by_tag
        .into_iter()
        .map(|(tag, value)| SwashSetting {
            tag: SwashTag::from_be_bytes(tag),
            value,
        })
        .collect::<Vec<_>>();

    let mut scaler = scale_context
        .builder(swash_font)
        .size(ppem)
        .hint(resolved_hinting_enabled(display_scale, rasterization));
    if !settings.is_empty() {
        scaler = scaler.variations(settings.into_iter());
    }
    let mut scaler = scaler.build();
    let mut outline = scaler
        .scale_outline(cache_key.glyph_id)
        .or_else(|| scaler.scale_color_outline(cache_key.glyph_id))?;
    if cache_key
        .flags
        .contains(cosmic_text::CacheKeyFlags::FAKE_ITALIC)
    {
        outline.transform(&SwashTransform::skew(
            SwashAngle::from_degrees(14.0),
            SwashAngle::from_degrees(0.0),
        ));
    }
    Some(outline.path().commands().collect())
}
