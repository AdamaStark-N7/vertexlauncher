use super::*;

/// Background and line decorations for a character range of a console line.
/// textui spans only carry color and italics, so these are painted separately.
pub(super) struct SpanDecoration {
    pub(super) start_char: usize,
    pub(super) end_char: usize,
    pub(super) background: Color32,
    pub(super) underline: Option<Color32>,
    pub(super) strikethrough: Option<Color32>,
}

/// One laid-out console line, drawn with textui.
///
/// Character positions are found by measuring text prefixes (memoized), which keeps
/// selection and hit-testing correct for proportional and monospace fonts alike.
pub(super) struct ConsoleLineLayout {
    pub(super) display_text: String,
    pub(super) rich_spans: Vec<RichTextSpan>,
    pub(super) decorations: Vec<SpanDecoration>,
    pub(super) options: LabelOptions,
    pub(super) size: egui::Vec2,
    pub(super) char_count: usize,
    prefix_widths: Mutex<HashMap<usize, f32>>,
}

impl ConsoleLineLayout {
    pub(super) fn build(
        text_ui: &mut TextUi,
        ctx: &egui::Context,
        line: &str,
        options: &LabelOptions,
    ) -> Self {
        let mut options = options.clone();
        // Character indices must match the text exactly, so no typographic substitutions.
        options.fundamentals.smart_quotes = false;
        options.wrap = false;

        let (display_text, ansi_spans) = if line.contains('\x1b') {
            let parsed = parse_ansi_annotated(line);
            (parsed.text.to_string(), parsed.spans)
        } else {
            (line.to_owned(), Vec::new())
        };

        let default_format = SpanFormat::plain(options.color, options.italic);
        let mut segments: Vec<(usize, usize, SpanFormat)> = Vec::new();
        let mut cursor = 0usize;
        for span in &ansi_spans {
            let start = span.start.min(display_text.len());
            let end = span.end.min(display_text.len());
            if start > cursor {
                segments.push((cursor, start, default_format));
            }
            if end > start {
                segments.push((
                    start,
                    end,
                    ansi_span_format(&span.codes, options.color, options.italic),
                ));
            }
            cursor = cursor.max(end);
        }
        if cursor < display_text.len() {
            segments.push((cursor, display_text.len(), default_format));
        }

        let char_at = |byte: usize| display_text[..byte].chars().count();
        let mut rich_spans = Vec::with_capacity(segments.len());
        let mut decorations = Vec::new();
        for (start, end, format) in segments {
            rich_spans.push(RichTextSpan {
                text: display_text[start..end].to_owned(),
                style: RichTextStyle {
                    color: format.color.into(),
                    monospace: options.monospace,
                    italic: format.italic,
                    weight: options.weight,
                },
            });
            if format.has_decoration() {
                decorations.push(SpanDecoration {
                    start_char: char_at(start),
                    end_char: char_at(end),
                    background: format.background,
                    underline: format.underline,
                    strikethrough: format.strikethrough,
                });
            }
        }

        let char_count = display_text.chars().count();
        let width = if display_text.is_empty() {
            0.0
        } else {
            text_ui
                .measure_text_size_ctx(ctx, display_text.as_str(), &options)
                .x
        };
        Self {
            display_text,
            rich_spans,
            decorations,
            size: egui::vec2(width, options.line_height),
            options,
            char_count,
            prefix_widths: Mutex::new(HashMap::new()),
        }
    }

    /// Horizontal offset of the boundary before `char_index`.
    pub(super) fn x_at_char(
        &self,
        text_ui: &mut TextUi,
        ctx: &egui::Context,
        char_index: usize,
    ) -> f32 {
        let char_index = char_index.min(self.char_count);
        if char_index == 0 {
            return 0.0;
        }
        if char_index == self.char_count {
            return self.size.x;
        }
        if let Some(width) = self
            .prefix_widths
            .lock()
            .expect("console prefix width cache poisoned")
            .get(&char_index)
        {
            return *width;
        }
        let byte = self
            .display_text
            .char_indices()
            .nth(char_index)
            .map_or(self.display_text.len(), |(byte, _)| byte);
        let width = text_ui
            .measure_text_size_ctx(ctx, &self.display_text[..byte], &self.options)
            .x;
        self.prefix_widths
            .lock()
            .expect("console prefix width cache poisoned")
            .insert(char_index, width);
        width
    }

    /// Character boundary closest to horizontal offset `x`.
    pub(super) fn char_at_x(&self, text_ui: &mut TextUi, ctx: &egui::Context, x: f32) -> usize {
        if x <= 0.0 || self.char_count == 0 {
            return 0;
        }
        if x >= self.size.x {
            return self.char_count;
        }
        // Smallest boundary at or past `x`, then pick whichever neighbor is closer.
        let (mut low, mut high) = (0usize, self.char_count);
        while low < high {
            let mid = (low + high) / 2;
            if self.x_at_char(text_ui, ctx, mid) < x {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        if low == 0 {
            return 0;
        }
        let before = self.x_at_char(text_ui, ctx, low - 1);
        let after = self.x_at_char(text_ui, ctx, low);
        if x - before <= after - x {
            low - 1
        } else {
            low
        }
    }
}

/// Resolved formatting of one ANSI span.
#[derive(Clone, Copy)]
pub(super) struct SpanFormat {
    pub(super) color: Color32,
    pub(super) background: Color32,
    pub(super) italic: bool,
    pub(super) underline: Option<Color32>,
    pub(super) strikethrough: Option<Color32>,
}

impl SpanFormat {
    fn plain(color: Color32, italic: bool) -> Self {
        Self {
            color,
            background: Color32::TRANSPARENT,
            italic,
            underline: None,
            strikethrough: None,
        }
    }

    fn has_decoration(&self) -> bool {
        self.background != Color32::TRANSPARENT
            || self.underline.is_some()
            || self.strikethrough.is_some()
    }
}

/// Derive the effective format for an ANSI span, falling back to the line-level
/// `default_color` for any attribute not set by the span's codes.
fn ansi_span_format(
    codes: &[SgrAttribute],
    default_color: Color32,
    default_italic: bool,
) -> SpanFormat {
    let mut format = SpanFormat::plain(default_color, default_italic);
    for code in codes {
        match code {
            SgrAttribute::Foreground(c) => format.color = ansi_color_to_egui(c),
            SgrAttribute::Background(c) => format.background = ansi_color_to_egui(c),
            SgrAttribute::Italic => format.italic = true,
            SgrAttribute::Underline => format.underline = Some(format.color),
            SgrAttribute::CrossedOut => format.strikethrough = Some(format.color),
            SgrAttribute::Reset => format = SpanFormat::plain(default_color, default_italic),
            _ => {}
        }
    }
    format
}
