use super::*;

impl TextUi {
    pub(crate) fn input_span_attrs_owned(
        &self,
        style: &RichTextStyle,
        options: &InputOptions,
        scale: f32,
    ) -> AttrsOwned {
        let mut attrs = Attrs::new()
            .color(to_cosmic_text_color(style.color))
            // Spans at the default weight take the input's own weight; styled spans keep theirs.
            .weight(Weight(self.effective_weight(if style.weight == 400 {
                options.weight
            } else {
                style.weight
            })))
            .metrics(Metrics::new(
                (self.effective_font_size(options.font_size) * scale).max(1.0),
                (self.effective_line_height(options.line_height) * scale).max(1.0),
            ));

        if style.monospace {
            attrs = attrs.family(Family::Monospace);
        } else if let Some(family) = options
            .fundamentals
            .font_family
            .as_deref()
            .or(self.ui_font_family.as_deref())
        {
            attrs = attrs.family(Family::Name(family));
        }
        if style.italic {
            attrs = attrs.style(FontStyle::Italic);
        }
        if let Some(features) =
            compose_font_features(&self.open_type_feature_tags, &options.fundamentals)
        {
            attrs = attrs.font_features(features);
        }

        AttrsOwned::new(&attrs)
    }

    pub(crate) fn rich_viewer_attrs_fingerprint(
        &self,
        spans: &[RichTextSpan],
        options: &InputOptions,
        scale: f32,
        wrap: bool,
    ) -> u64 {
        let mut hasher = new_fingerprint_hasher();
        "rich_viewer_attrs".hash(&mut hasher);
        options.font_size.to_bits().hash(&mut hasher);
        options.line_height.to_bits().hash(&mut hasher);
        options.weight.hash(&mut hasher);
        scale.to_bits().hash(&mut hasher);
        wrap.hash(&mut hasher);
        hash_text_fundamentals(&options.fundamentals, &mut hasher);
        self.ui_font_family.hash(&mut hasher);
        self.ui_cjk_font_family.hash(&mut hasher);
        self.ui_emoji_font_family.hash(&mut hasher);
        self.ui_font_size_scale.to_bits().hash(&mut hasher);
        self.ui_font_weight.hash(&mut hasher);
        self.open_type_features_enabled.hash(&mut hasher);
        self.open_type_features_to_enable.hash(&mut hasher);
        for span in spans {
            span.text.hash(&mut hasher);
            span.style.color.hash(&mut hasher);
            span.style.monospace.hash(&mut hasher);
            span.style.italic.hash(&mut hasher);
            span.style.weight.hash(&mut hasher);
        }
        hasher.finish()
    }

    pub(crate) fn input_attrs_owned(&self, options: &InputOptions, scale: f32) -> AttrsOwned {
        let mut attrs = Attrs::new()
            .color(to_cosmic_color(options.text_color))
            .metrics(Metrics::new(
                (self.effective_font_size(options.font_size) * scale).max(1.0),
                (self.effective_line_height(options.line_height) * scale).max(1.0),
            ))
            .weight(Weight(self.effective_weight(options.weight)));

        if options.monospace {
            attrs = attrs.family(Family::Monospace);
        } else if let Some(family) = options
            .fundamentals
            .font_family
            .as_deref()
            .or(self.ui_font_family.as_deref())
        {
            attrs = attrs.family(Family::Name(family));
        }
        if let Some(features) =
            compose_font_features(&self.open_type_feature_tags, &options.fundamentals)
        {
            attrs = attrs.font_features(features);
        }

        AttrsOwned::new(&attrs)
    }

    pub(crate) fn input_attrs_fingerprint(&self, options: &InputOptions, scale: f32) -> u64 {
        let mut hasher = new_fingerprint_hasher();
        "input_attrs".hash(&mut hasher);
        options.font_size.to_bits().hash(&mut hasher);
        options.line_height.to_bits().hash(&mut hasher);
        options.text_color.hash(&mut hasher);
        options.monospace.hash(&mut hasher);
        options.weight.hash(&mut hasher);
        hash_text_fundamentals(&options.fundamentals, &mut hasher);
        scale.to_bits().hash(&mut hasher);
        self.ui_font_family.hash(&mut hasher);
        self.ui_cjk_font_family.hash(&mut hasher);
        self.ui_emoji_font_family.hash(&mut hasher);
        self.ui_font_size_scale.to_bits().hash(&mut hasher);
        self.ui_font_weight.hash(&mut hasher);
        self.open_type_features_enabled.hash(&mut hasher);
        self.open_type_features_to_enable.hash(&mut hasher);
        hasher.finish()
    }
}
