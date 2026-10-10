//! Search-tag chips and tag list helpers shared by the content browser and discover screens.
//!
//! This file is mounted as a module by both screens (`#[path]`), so it only relies on the
//! prelude items both parents import.

use super::*;

/// Per-screen identity of the tag chips. These strings feed egui image URIs and text ids, so
/// each screen keeps its historical values.
#[derive(Clone, Copy)]
pub(super) struct SearchTagChipIds {
    /// URI prefix (without the trailing `/{index}-{rrggbb}.svg`) of the remove-icon bytes.
    pub remove_icon_uri_prefix: &'static str,
    /// Id source of the tag label text.
    pub label_id: &'static str,
}

pub(super) fn add_search_tag(search_tags: &mut Vec<String>, candidate: &str) -> bool {
    let Some(normalized) = normalize_search_tag(candidate) else {
        return false;
    };
    if search_tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(&normalized))
    {
        return false;
    }
    search_tags.push(normalized);
    true
}

fn normalize_search_tag(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Renders the removable tag chips; returns `true` when a tag was removed.
pub(super) fn render_search_tag_chips(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    search_tags: &mut Vec<String>,
    ids: SearchTagChipIds,
) -> bool {
    let mut removed_index: Option<usize> = None;
    let tag_style = LabelOptions {
        color: ui.visuals().text_color(),
        ..style::caption(ui)
    };
    // The tint depends only on the text color, so build it once (lazily) per frame, not per tag.
    let mut themed_svg: Option<std::sync::Arc<[u8]>> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(style::SPACE_SM, style::SPACE_SM);
        for (index, tag) in search_tags.iter().enumerate() {
            let fill = ui.visuals().selection.bg_fill.gamma_multiply(0.28);
            let stroke = egui::Stroke::new(1.0, ui.visuals().selection.bg_fill.gamma_multiply(0.7));
            let text_color = ui.visuals().text_color();
            let svg_bytes = themed_svg
                .get_or_insert_with(|| {
                    crate::ui::svg_tint::tint_svg(assets::X_SVG, text_color).into()
                })
                .clone();
            let uri = format!(
                "{}/{index}-{:02x}{:02x}{:02x}.svg",
                ids.remove_icon_uri_prefix,
                text_color.r(),
                text_color.g(),
                text_color.b()
            );
            let max_chip_width = ui.available_width().max(1.0);
            let max_text_width =
                (max_chip_width - 22.0 - ui.spacing().item_spacing.x - 16.0).max(1.0);
            let display_tag = truncate_single_line_text_with_ellipsis(
                text_ui,
                ui,
                tag.as_str(),
                max_text_width,
                &tag_style,
            );
            egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_height(24.0);
                        let icon_button = egui::Button::image(
                            egui::Image::from_bytes(uri, svg_bytes)
                                .fit_to_exact_size(egui::vec2(16.0, 16.0)),
                        )
                        .frame(false)
                        .min_size(egui::vec2(22.0, 22.0));
                        let icon_response = ui.add(icon_button);
                        let icon_state =
                            interaction::InteractionState::of(ui.ctx(), &icon_response, true);
                        interaction::paint_card_highlight(
                            ui.painter(),
                            ui.visuals(),
                            icon_state,
                            icon_response.rect,
                            6,
                            true,
                        );
                        if crate::ui::style::hover_tip(
                            ui,
                            icon_response,
                            format!("Remove tag: {tag}"),
                        )
                        .clicked()
                        {
                            removed_index = Some(index);
                        }
                        let _ = text_ui.label(
                            ui,
                            (ids.label_id, tag.as_str()),
                            display_tag.as_str(),
                            &tag_style,
                        );
                    });
                });
        }
    });
    if let Some(index) = removed_index {
        search_tags.remove(index);
        true
    } else {
        false
    }
}
