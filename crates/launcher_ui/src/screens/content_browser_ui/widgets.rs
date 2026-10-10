use super::*;

pub(super) fn render_browser_thumbnail(
    ui: &mut Ui,
    id_source: impl std::hash::Hash + Copy,
    entry: &BrowserProjectEntry,
    size: f32,
) {
    let thumbnail_size = egui::vec2(size, size);
    if let Some(icon_url) = entry.icon_url.as_deref() {
        remote_tiled_image::show(
            ui,
            icon_url,
            thumbnail_size,
            (id_source, "remote-icon"),
            assets::LIBRARY_SVG,
        );
    } else {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id_source.hash(&mut hasher);
        ui.add(
            egui::Image::from_bytes(
                format!("bytes://content-browser/default/{}", hasher.finish()),
                assets::LIBRARY_SVG,
            )
            .fit_to_exact_size(thumbnail_size),
        );
    }
}

pub(super) fn render_rounded_icon_button(
    ui: &mut Ui,
    icon_id: &str,
    svg_bytes: &'static [u8],
    tooltip: &str,
    fill: egui::Color32,
    width: f32,
    height: f32,
    enabled: bool,
) -> IconButtonOutcome {
    let text_color = ui.visuals().text_color();
    let themed_svg = crate::ui::svg_tint::tint_svg(svg_bytes, text_color);
    let uri = format!(
        "bytes://content-browser-rounded/{icon_id}-{:02x}{:02x}{:02x}.svg",
        text_color.r(),
        text_color.g(),
        text_color.b()
    );
    let button_size = egui::vec2(width, height);
    let icon_size = (height - 10.0).max(12.0);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(button_size, sense);
    let state = interaction::InteractionState::of_covered(ui.ctx(), &response, rect, enabled);
    let button_fill = if enabled {
        interaction::FillPalette::new(
            fill,
            fill.lerp_to_gamma(ui.visuals().text_color(), 0.14),
            fill.gamma_multiply(0.9),
            fill,
            ui.visuals().text_color(),
        )
        .fill(state, false)
    } else {
        ui.visuals().widgets.inactive.weak_bg_fill
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), button_fill);
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(8),
        interaction::focus_stroke(
            state,
            ui.visuals(),
            interaction::hover_stroke(
                state,
                ui.visuals().widgets.noninteractive.bg_stroke,
                ui.visuals().widgets.hovered.bg_stroke,
            ),
        ),
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), ui.visuals(), state, rect, 8);

    let image = egui::Image::from_bytes(uri, themed_svg)
        .fit_to_exact_size(egui::vec2(icon_size, icon_size));
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
    let _ = ui.put(icon_rect, image);

    IconButtonOutcome {
        clicked: crate::ui::style::hover_tip(ui, response, tooltip).clicked(),
        rect,
    }
}
