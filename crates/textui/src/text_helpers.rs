use egui::Ui;

use crate::{LabelOptions, TextUi};

/// Collapses any repeated whitespace into single ASCII spaces for single-line UI labels.
pub fn normalize_inline_whitespace(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(word);
    }
    normalized
}

/// Truncates a single-line label after collapsing repeated whitespace.
pub fn truncate_single_line_text_with_ellipsis(
    text_ui: &mut TextUi,
    ui: &Ui,
    text: &str,
    max_width: f32,
    label_options: &LabelOptions,
) -> String {
    let normalized = normalize_inline_whitespace(text);
    truncate_prepared_single_line_text_with_ellipsis(
        text_ui,
        ui,
        normalized.as_str(),
        max_width,
        label_options,
    )
}

/// Truncates a single-line label while preserving internal whitespace.
pub fn truncate_single_line_text_with_ellipsis_preserving_whitespace(
    text_ui: &mut TextUi,
    ui: &Ui,
    text: &str,
    max_width: f32,
    label_options: &LabelOptions,
) -> String {
    truncate_prepared_single_line_text_with_ellipsis(
        text_ui,
        ui,
        text.trim(),
        max_width,
        label_options,
    )
}

fn truncate_prepared_single_line_text_with_ellipsis(
    text_ui: &mut TextUi,
    ui: &Ui,
    text: &str,
    max_width: f32,
    label_options: &LabelOptions,
) -> String {
    if text.is_empty() {
        return String::new();
    }

    if max_width <= 0.0 {
        return "...".to_owned();
    }

    if text_ui.measure_text_size(ui, text, label_options).x <= max_width {
        return text.to_owned();
    }

    let ellipsis = "...";
    if text_ui.measure_text_size(ui, ellipsis, label_options).x > max_width {
        return String::new();
    }

    let chars = text.chars().collect::<Vec<_>>();
    let mut low = 0usize;
    let mut high = chars.len();
    let mut best = 0usize;

    while low <= high {
        let mid = low + (high - low) / 2;
        let mut candidate = String::with_capacity(mid + ellipsis.len());
        for ch in chars.iter().take(mid) {
            candidate.push(*ch);
        }
        candidate.push_str(ellipsis);

        if text_ui
            .measure_text_size(ui, candidate.as_str(), label_options)
            .x
            <= max_width
        {
            best = mid;
            low = mid.saturating_add(1);
        } else if mid == 0 {
            break;
        } else {
            high = mid - 1;
        }
    }

    if best == 0 {
        return ellipsis.to_owned();
    }

    let mut truncated = String::with_capacity(best + ellipsis.len());
    for ch in chars.iter().take(best) {
        truncated.push(*ch);
    }
    truncated.push_str(ellipsis);
    truncated
}
