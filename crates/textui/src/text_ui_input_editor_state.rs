use super::*;

/// Gives every CJK and emoji run in `lines` its chosen font, leaving other text on the line's own
/// base attributes. Lines that already match are left alone, so calling this is cheap.
pub(crate) fn apply_script_fonts_to_lines(
    lines: &mut [cosmic_text::BufferLine],
    fonts: ScriptFonts<'_>,
) {
    for line in lines {
        let base = AttrsOwned::new(&line.attrs_list().defaults());
        let list = fonts.attrs_list(line.text(), &base);
        line.set_attrs_list(list);
    }
}

impl TextUi {
    /// Owned copies of the CJK and emoji families for `fundamentals`, for code that needs them
    /// while the font system is borrowed mutably.
    pub(crate) fn script_font_names(
        &self,
        fundamentals: &TextFundamentals,
    ) -> (Option<String>, Option<String>) {
        let fonts = self.script_fonts(fundamentals);
        (fonts.cjk.map(str::to_owned), fonts.emoji.map(str::to_owned))
    }

    /// Re-applies script fonts to an editor after its text changed, so text typed or composed
    /// since the last sync gets the right font, then reshapes what changed.
    pub(crate) fn reapply_script_fonts(
        &mut self,
        editor: &mut Editor<'static>,
        fundamentals: &TextFundamentals,
    ) {
        let (cjk_font, emoji_font) = self.script_font_names(fundamentals);
        let fonts = ScriptFonts {
            cjk: cjk_font.as_deref(),
            emoji: emoji_font.as_deref(),
        };
        if fonts.is_empty() {
            return;
        }
        editor.with_buffer_mut(|buffer| apply_script_fonts_to_lines(&mut buffer.lines, fonts));
        editor
            .borrow_with(&mut self.font_system)
            .shape_as_needed(false);
    }

    pub(crate) fn new_input_state(
        font_system: &mut FontSystem,
        text: &str,
        multiline: bool,
    ) -> InputState {
        let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 22.0));
        {
            let mut borrowed = buffer.borrow_with(font_system);
            borrowed.set_wrap(if multiline {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            });
            borrowed.set_text(text, &Attrs::new(), Shaping::Advanced, None);
            borrowed.shape_until_scroll(true);
        }

        InputState {
            editor: Editor::new(buffer),
            last_text: text.to_owned(),
            attrs_fingerprint: 0,
            multiline,
            preferred_cursor_x_px: None,
            scroll_metrics: EditorScrollMetrics::default(),
            last_used_frame: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_undo_op: UndoOpKind::None,
            preedit: None,
        }
    }

    pub(crate) fn replace_editor_text(
        &mut self,
        editor: &mut Editor<'static>,
        text: &str,
        options: &InputOptions,
        multiline: bool,
        width_px: f32,
        height_px: f32,
        scale: f32,
    ) -> EditorScrollMetrics {
        let attrs_owned = self.input_attrs_owned(options, scale);
        let effective_font_size = self.effective_font_size(options.font_size) * scale;
        let effective_line_height = self.effective_line_height(options.line_height) * scale;
        let previous_cursor = editor.cursor();
        let previous_selection = editor.selection();
        let previous_scroll = editor.with_buffer(|buffer| buffer.scroll());
        let (cjk_font, emoji_font) = self.script_font_names(&options.fundamentals);
        let mut scroll_metrics = EditorScrollMetrics::default();
        editor.with_buffer_mut(|buffer| {
            let mut borrowed = buffer.borrow_with(&mut self.font_system);
            borrowed.set_metrics_and_size(
                Metrics::new(effective_font_size, effective_line_height),
                Some(width_px),
                Some(height_px),
            );
            borrowed.set_wrap(if multiline {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            });
            let attrs = attrs_owned.as_attrs();
            borrowed.set_text(text, &attrs, Shaping::Advanced, None);
            apply_script_fonts_to_lines(
                &mut borrowed.lines,
                ScriptFonts {
                    cjk: cjk_font.as_deref(),
                    emoji: emoji_font.as_deref(),
                },
            );
            borrowed.set_scroll(previous_scroll);
            borrowed.shape_until_scroll(true);
            scroll_metrics =
                clamp_borrowed_buffer_scroll(&mut borrowed, &options.fundamentals, scale);
        });
        editor.set_cursor(clamp_cursor_to_editor(editor, previous_cursor));
        editor.set_selection(clamp_selection_to_editor(editor, previous_selection));
        scroll_metrics
    }

    pub(crate) fn configure_editor(
        &mut self,
        editor: &mut Editor<'static>,
        options: &InputOptions,
        multiline: bool,
        width_px: f32,
        height_px: f32,
        scale: f32,
    ) -> EditorScrollMetrics {
        let effective_font_size = self.effective_font_size(options.font_size) * scale;
        let effective_line_height = self.effective_line_height(options.line_height) * scale;
        let mut scroll_metrics = EditorScrollMetrics::default();
        editor.with_buffer_mut(|buffer| {
            let mut borrowed = buffer.borrow_with(&mut self.font_system);
            borrowed.set_metrics_and_size(
                Metrics::new(effective_font_size, effective_line_height),
                Some(width_px),
                Some(height_px),
            );
            borrowed.set_wrap(if multiline {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            });
            borrowed.shape_until_scroll(true);
            scroll_metrics =
                clamp_borrowed_buffer_scroll(&mut borrowed, &options.fundamentals, scale);
        });
        scroll_metrics
    }

    pub(crate) fn replace_editor_rich_text(
        &mut self,
        editor: &mut Editor<'static>,
        spans: &[RichTextSpan],
        options: &InputOptions,
        width_px: f32,
        height_px: f32,
        scale: f32,
        wrap: bool,
    ) -> EditorScrollMetrics {
        let effective_font_size = self.effective_font_size(options.font_size) * scale;
        let effective_line_height = self.effective_line_height(options.line_height) * scale;
        let previous_cursor = editor.cursor();
        let previous_selection = editor.selection();
        let previous_scroll = editor.with_buffer(|buffer| buffer.scroll());
        let default_attrs = self.input_attrs_owned(options, scale);
        let span_attrs_owned = spans
            .iter()
            .map(|span| self.input_span_attrs_owned(&span.style, options, scale))
            .collect::<Vec<_>>();
        let owned_spans: Vec<(&str, AttrsOwned)> = spans
            .iter()
            .zip(span_attrs_owned.iter())
            .map(|(span, attrs)| (span.text.as_str(), attrs.clone()))
            .collect();
        let (script_spans, _) = self
            .script_fonts(&options.fundamentals)
            .split_spans(&owned_spans);
        let mut scroll_metrics = EditorScrollMetrics::default();

        editor.with_buffer_mut(|buffer| {
            let mut borrowed = buffer.borrow_with(&mut self.font_system);
            borrowed.set_metrics_and_size(
                Metrics::new(effective_font_size, effective_line_height),
                Some(width_px),
                Some(height_px),
            );
            borrowed.set_wrap(if wrap { Wrap::WordOrGlyph } else { Wrap::None });
            let rich_text = script_spans
                .iter()
                .map(|(text, attrs)| (*text, attrs.as_attrs()))
                .collect::<Vec<_>>();
            borrowed.set_rich_text(
                rich_text,
                &default_attrs.as_attrs(),
                Shaping::Advanced,
                None,
            );
            borrowed.set_scroll(previous_scroll);
            borrowed.shape_until_scroll(true);
            scroll_metrics =
                clamp_borrowed_buffer_scroll(&mut borrowed, &options.fundamentals, scale);
        });
        editor.set_cursor(clamp_cursor_to_editor(editor, previous_cursor));
        editor.set_selection(clamp_selection_to_editor(editor, previous_selection));
        scroll_metrics
    }

    pub(crate) fn configure_viewer(
        &mut self,
        editor: &mut Editor<'static>,
        options: &InputOptions,
        width_px: f32,
        height_px: f32,
        scale: f32,
        wrap: bool,
    ) -> EditorScrollMetrics {
        let effective_font_size = self.effective_font_size(options.font_size) * scale;
        let effective_line_height = self.effective_line_height(options.line_height) * scale;
        let mut scroll_metrics = EditorScrollMetrics::default();
        editor.with_buffer_mut(|buffer| {
            let mut borrowed = buffer.borrow_with(&mut self.font_system);
            borrowed.set_metrics_and_size(
                Metrics::new(effective_font_size, effective_line_height),
                Some(width_px),
                Some(height_px),
            );
            borrowed.set_wrap(if wrap { Wrap::WordOrGlyph } else { Wrap::None });
            borrowed.shape_until_scroll(true);
            scroll_metrics =
                clamp_borrowed_buffer_scroll(&mut borrowed, &options.fundamentals, scale);
        });
        scroll_metrics
    }
}
