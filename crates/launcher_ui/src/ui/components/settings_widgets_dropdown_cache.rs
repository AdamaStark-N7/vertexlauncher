use super::*;

/// Cached result of measuring all dropdown option widths.
/// Stored in egui's temp data, keyed by the dropdown's open_id.
#[derive(Clone)]
struct DropdownWidthCache {
    fingerprint: u64,
    width: f32,
}

/// Cached result of a single selected-text truncation.
#[derive(Clone)]
struct TruncatedTextCache {
    /// Hash of (raw_text, max_width bits, font_size bits).
    fingerprint: u64,
    display: String,
}

/// Return the truncated display string for a dropdown's selected text, caching
/// the result in egui's temp data so text shaping is skipped on stable inputs.
pub(super) fn cached_truncate_selected_text(
    text_ui: &mut TextUi,
    ui: &Ui,
    text: &str,
    max_width: f32,
    label_style: &LabelOptions,
    cache_id: egui::Id,
) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    max_width.to_bits().hash(&mut hasher);
    label_style.font_size.to_bits().hash(&mut hasher);
    let fingerprint = hasher.finish();

    if let Some(cached) = ui
        .ctx()
        .data_mut(|d| d.get_temp::<TruncatedTextCache>(cache_id))
    {
        if cached.fingerprint == fingerprint {
            return cached.display;
        }
    }

    let display =
        truncate_single_line_text_with_ellipsis(text_ui, ui, text, max_width, label_style);
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            cache_id,
            TruncatedTextCache {
                fingerprint,
                display: display.clone(),
            },
        );
    });
    display
}

/// Hash the options list to a stable u64 fingerprint for cache invalidation.
fn options_fingerprint(options: &[&str]) -> u64 {
    let mut hasher = DefaultHasher::new();
    options.len().hash(&mut hasher);
    for option in options {
        option.hash(&mut hasher);
    }
    hasher.finish()
}

/// Return the maximum option text width, using egui temp data as a cache so
/// text shaping only runs once per unique options list instead of every frame.
///
/// When `popup_is_open` is false and no cached value exists the measurement is
/// deferred and `0.0` is returned — the value is only needed when the popup is
/// actually visible, so doing the work while the popup is closed is wasteful.
pub(super) fn cached_option_text_width(
    text_ui: &mut TextUi,
    ui: &Ui,
    options: &[&str],
    label_style: &LabelOptions,
    cache_id: egui::Id,
    popup_is_open: bool,
) -> f32 {
    let fingerprint = options_fingerprint(options);
    if let Some(cached) = ui
        .ctx()
        .data_mut(|d| d.get_temp::<DropdownWidthCache>(cache_id))
    {
        if cached.fingerprint == fingerprint {
            return cached.width;
        }
    }
    // Only measure when the popup is actually open — the width is unused otherwise.
    if !popup_is_open {
        return 0.0;
    }
    let width = options.iter().fold(0.0_f32, |max_w, opt| {
        max_w.max(text_ui.measure_text_size(ui, opt, label_style).x)
    });
    ui.ctx().data_mut(|d| {
        d.insert_temp(cache_id, DropdownWidthCache { fingerprint, width });
    });
    width
}
