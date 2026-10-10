use super::*;

impl TextUi {
    /// Produces syntax-highlighted rich spans for code rendering.
    pub fn highlight_code_spans(
        &self,
        code: &str,
        language: Option<&str>,
        fallback_color: TextColor,
    ) -> Vec<RichTextSpan> {
        self.highlight_code_spans_impl(code, language, fallback_color.into())
    }

    pub fn parse_markdown_blocks_cached(
        &mut self,
        id_source: impl Hash,
        markdown: &str,
        fingerprint: u64,
    ) -> Arc<[TextMarkdownBlock]> {
        let cache_id = Id::new(id_source).with("textui_markdown_blocks");
        if let Some((fp, last_used, cached)) = self.markdown_cache.get_mut(&cache_id) {
            *last_used = self.current_frame;
            if *fp == fingerprint {
                return Arc::clone(cached);
            }
            let blocks = Arc::<[TextMarkdownBlock]>::from(parse_markdown_blocks(markdown));
            *fp = fingerprint;
            *cached = Arc::clone(&blocks);
            return blocks;
        }

        let blocks = Arc::<[TextMarkdownBlock]>::from(parse_markdown_blocks(markdown));
        self.markdown_cache.insert(
            cache_id,
            (fingerprint, self.current_frame, Arc::clone(&blocks)),
        );
        blocks
    }

    /// Renders a syntax-highlighted code block synchronously.
    #[allow(dead_code)]
    pub(super) fn code_block(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        code: &str,
        options: &CodeBlockOptions,
    ) -> Response {
        let scale = ui.ctx().pixels_per_point();
        let width_points_opt = if options.wrap {
            Some(snap_width_to_bin(
                (ui.available_width() - options.padding.x * 2.0).max(1.0),
                scale,
            ))
        } else {
            None
        };

        let mut hasher = new_fingerprint_hasher();
        "code".hash(&mut hasher);
        code.hash(&mut hasher);
        options.font_size.to_bits().hash(&mut hasher);
        options.line_height.to_bits().hash(&mut hasher);
        options.wrap.hash(&mut hasher);
        options.text_color.hash(&mut hasher);
        options.background_color.hash(&mut hasher);
        options.language.hash(&mut hasher);
        scale.to_bits().hash(&mut hasher);
        width_points_opt
            .map(f32::to_bits)
            .unwrap_or(0)
            .hash(&mut hasher);
        self.hash_typography(&mut hasher);
        let _fingerprint = hasher.finish();
        let scene_id = ui.make_persistent_id(id_source).with("textui_code");

        let spans =
            self.highlight_code_spans_impl(code, options.language.as_deref(), options.text_color);
        let scene = self.prepare_rich_text_scene(
            ui.ctx(),
            scene_id,
            &spans,
            &LabelOptions {
                font_size: options.font_size,
                line_height: options.line_height,
                color: options.text_color,
                wrap: options.wrap,
                monospace: true,
                weight: 400,
                italic: false,
                padding: egui::Vec2::ZERO,
                fundamentals: options.fundamentals.clone(),
                ..LabelOptions::default()
            },
            width_points_opt,
        );

        let scene_size = egui_vec_from_text(scene.size_points);
        let desired_size = scene_size + options.padding * 2.0;
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

        let bg_shape = egui::Shape::rect_filled(
            rect,
            CornerRadius::same(options.corner_radius),
            options.background_color,
        );
        ui.painter().add(bg_shape);
        if options.stroke.width > 0.0 {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(options.corner_radius),
                options.stroke,
                egui::StrokeKind::Inside,
            );
        }

        let image_rect = Rect::from_min_size(rect.min + options.padding, scene_size);
        let painter = ui.painter().with_clip_rect(ui.clip_rect());
        self.paint_scene_in_rect(&painter, image_rect, &scene);

        response
    }

    /// Renders simple markdown (headings, paragraphs, fenced code).
    #[allow(dead_code)]
    pub(super) fn markdown(
        &mut self,
        ui: &mut Ui,
        id_source: impl Hash,
        markdown: &str,
        options: &MarkdownOptions,
    ) {
        // ── Markdown block cache ──────────────────────────────────────────────
        // parse_markdown_blocks is a full pulldown-cmark parse.  Cache the
        // result by (content + options) fingerprint to avoid re-parsing every
        // frame when nothing changed.
        let cache_id = ui.make_persistent_id(&id_source).with("md_cache");
        let md_fingerprint = {
            let mut hasher = new_fingerprint_hasher();
            "markdown_blocks".hash(&mut hasher);
            markdown.hash(&mut hasher);
            options.heading_scale.to_bits().hash(&mut hasher);
            options.paragraph_spacing.to_bits().hash(&mut hasher);
            options.body.font_size.to_bits().hash(&mut hasher);
            options.body.line_height.to_bits().hash(&mut hasher);
            options.body.color.hash(&mut hasher);
            options.code.font_size.to_bits().hash(&mut hasher);
            hasher.finish()
        };

        let blocks = if let Some((fp, last_used, cached)) = self.markdown_cache.get_mut(&cache_id) {
            *last_used = self.current_frame;
            if *fp == md_fingerprint {
                Arc::clone(cached)
            } else {
                let b = Arc::<[TextMarkdownBlock]>::from(parse_markdown_blocks(markdown));
                *fp = md_fingerprint;
                *cached = Arc::clone(&b);
                b
            }
        } else {
            let b = Arc::<[TextMarkdownBlock]>::from(parse_markdown_blocks(markdown));
            self.markdown_cache.insert(
                cache_id,
                (md_fingerprint, self.current_frame, Arc::clone(&b)),
            );
            b
        };

        ui.push_id(id_source, |ui| {
            for (index, block) in blocks.iter().enumerate() {
                match block {
                    TextMarkdownBlock::Heading { level, text } => {
                        let factor = match level {
                            TextMarkdownHeadingLevel::H1 => options.heading_scale + 0.26,
                            TextMarkdownHeadingLevel::H2 => options.heading_scale + 0.12,
                            TextMarkdownHeadingLevel::H3 => options.heading_scale,
                            TextMarkdownHeadingLevel::H4 => options.heading_scale - 0.08,
                            TextMarkdownHeadingLevel::H5 => options.heading_scale - 0.12,
                            TextMarkdownHeadingLevel::H6 => options.heading_scale - 0.16,
                        }
                        .max(1.0);
                        let heading_style = LabelOptions {
                            font_size: options.body.font_size * factor,
                            line_height: options.body.line_height * factor,
                            color: options.body.color,
                            wrap: true,
                            monospace: false,
                            weight: 700,
                            italic: false,
                            padding: egui::Vec2::ZERO,
                            fundamentals: options.body.fundamentals.clone(),
                            ..LabelOptions::default()
                        };
                        let _ = self.label(ui, ("md_h", index), text.as_str(), &heading_style);
                    }
                    TextMarkdownBlock::Paragraph(text) => {
                        let _ = self.label(ui, ("md_p", index), text.as_str(), &options.body);
                    }
                    TextMarkdownBlock::Code { language, text } => {
                        let mut code_options = options.code.clone();
                        code_options.language = language.clone();
                        let _ =
                            self.code_block(ui, ("md_code", index), text.as_str(), &code_options);
                    }
                }

                if index + 1 < blocks.len() {
                    ui.add_space(options.paragraph_spacing);
                }
            }
        });
    }

    pub(super) fn highlight_code_spans_impl(
        &self,
        code: &str,
        language: Option<&str>,
        fallback_color: Color32,
    ) -> Vec<RichSpan> {
        if language
            .map(|lang| {
                let normalized = lang.trim();
                normalized.eq_ignore_ascii_case("text")
                    || normalized.eq_ignore_ascii_case("txt")
                    || normalized.eq_ignore_ascii_case("plain")
                    || normalized.eq_ignore_ascii_case("plaintext")
            })
            .unwrap_or(false)
        {
            return vec![RichSpan {
                text: code.to_owned(),
                style: SpanStyle {
                    color: fallback_color.into(),
                    monospace: true,
                    italic: false,
                    weight: 400,
                },
            }];
        }

        let syntax = language
            .and_then(|lang| self.syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.code_theme);
        let mut spans = Vec::new();

        for line in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    for (style, segment) in ranges {
                        spans.push(RichSpan {
                            text: segment.to_owned(),
                            style: SpanStyle {
                                color: Color32::from_rgba_premultiplied(
                                    style.foreground.r,
                                    style.foreground.g,
                                    style.foreground.b,
                                    style.foreground.a,
                                )
                                .into(),
                                monospace: true,
                                italic: style.font_style.contains(SyntectFontStyle::ITALIC),
                                weight: if style.font_style.contains(SyntectFontStyle::BOLD) {
                                    700
                                } else {
                                    400
                                },
                            },
                        });
                    }
                }
                Err(_) => {
                    spans.push(RichSpan {
                        text: line.to_owned(),
                        style: SpanStyle {
                            color: fallback_color.into(),
                            monospace: true,
                            italic: false,
                            weight: 400,
                        },
                    });
                }
            }
        }

        spans
    }
}
