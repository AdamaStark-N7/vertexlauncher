use super::*;

pub(super) fn render_clickable_entry_row(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    height: f32,
    add_contents: impl FnOnce(&mut Ui),
) -> egui::Response {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let response = ui.interact(
        rect,
        ui.make_persistent_id(&id_source),
        egui::Sense::click(),
    );
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    let visuals = ui.visuals();
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    let fill = interaction::FillPalette::from_visuals(visuals).fill(state, false);
    let stroke = interaction::focus_stroke(
        state,
        visuals,
        interaction::hover_stroke(
            state,
            visuals.widgets.inactive.bg_stroke,
            visuals.widgets.hovered.bg_stroke,
        ),
    );
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), visuals, state, rect, 8);
    let inner = rect.shrink2(egui::vec2(
        ACTIVITY_ENTRY_ROW_HORIZONTAL_PADDING,
        ACTIVITY_ENTRY_ROW_VERTICAL_PADDING,
    ));
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(id_source)
            .max_rect(inner)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            let horizontal_clip = egui::Rect::from_min_max(
                egui::pos2(inner.min.x, rect.min.y - 8.0),
                egui::pos2(inner.max.x, rect.max.y + 13.0),
            );
            ui.set_clip_rect(horizontal_clip);
            add_contents(ui)
        },
    );
    response
}

pub(super) fn render_entry_thumbnail(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    image_png: Option<(String, Arc<[u8]>)>,
    icon_svg: &'static [u8],
    width: f32,
    height: f32,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let visuals = ui.visuals();
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        visuals.selection.bg_fill.gamma_multiply(0.16),
        visuals.widgets.inactive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        if let Some((uri, png)) = image_png {
            let image_size = (height - 4.0).max(1.0).min(width - 4.0).max(1.0);
            let image_rect =
                egui::Rect::from_center_size(rect.center(), egui::vec2(image_size, image_size));
            if let image_textures::ManagedTextureStatus::Ready(texture) =
                image_textures::request_texture(ui.ctx(), uri, png, TextureOptions::LINEAR)
            {
                ui.put(
                    image_rect,
                    texture
                        .image()
                        .fit_to_exact_size(egui::vec2(image_size, image_size)),
                );
                return;
            }
        }

        ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(((height - ENTRY_ICON_SIZE) * 0.5).max(0.0));
            let themed_svg = cached_tinted_svg(icon_svg, ui.visuals().text_color());
            let uri = format!("bytes://home/entry-thumb/{:?}.svg", ui.id().with(id_source));
            ui.add(
                egui::Image::from_bytes(uri, themed_svg)
                    .fit_to_exact_size(egui::vec2(ENTRY_ICON_SIZE, ENTRY_ICON_SIZE)),
            );
        });
    });
}

pub(super) fn render_favorite_star_button(
    ui: &mut Ui,
    id_source: impl std::hash::Hash + Copy,
    active: bool,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(FAVORITE_STAR_BUTTON_SIZE, FAVORITE_STAR_BUTTON_SIZE),
        egui::Sense::hover(),
    );
    let response = ui.interact(rect, ui.make_persistent_id(id_source), egui::Sense::click());
    let star_fill = if active {
        ui.visuals().warn_fg_color
    } else {
        ui.visuals().extreme_bg_color
    };
    let star_outline = ui.visuals().widgets.active.bg_stroke.color;
    let themed_svg = cached_star_svg(star_fill, star_outline);
    let uri = format!(
        "bytes://home/favorite-star/{:?}-{:02x}{:02x}{:02x}-{:02x}{:02x}{:02x}.svg",
        ui.id().with((id_source, active)),
        star_fill.r(),
        star_fill.g(),
        star_fill.b(),
        star_outline.r(),
        star_outline.g(),
        star_outline.b()
    );
    let icon_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(FAVORITE_STAR_ICON_SIZE, FAVORITE_STAR_ICON_SIZE),
    );
    let state = interaction::InteractionState::of(ui.ctx(), &response, true);
    let button_fill = interaction::FillPalette::new(
        Color32::TRANSPARENT,
        ui.visuals().widgets.hovered.bg_fill,
        ui.visuals().widgets.active.bg_fill,
        ui.visuals().selection.bg_fill,
        ui.visuals().text_color(),
    )
    .fill(state, false);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), button_fill);
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(6),
        interaction::focus_stroke(
            state,
            ui.visuals(),
            interaction::hover_stroke(
                state,
                ui.visuals().widgets.inactive.bg_stroke,
                ui.visuals().widgets.hovered.bg_stroke,
            ),
        ),
        egui::StrokeKind::Inside,
    );
    interaction::paint_focus_ring(ui.painter(), ui.visuals(), state, rect, 6);
    let mut icon_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::left_to_right(egui::Align::Center)),
    );
    icon_ui.add(
        egui::Image::from_bytes(uri, themed_svg)
            .fit_to_exact_size(icon_rect.size())
            .sense(egui::Sense::hover()),
    );

    response
}

fn apply_star_fill_and_stroke_svg(svg_bytes: &[u8], fill: Color32, stroke: Color32) -> Vec<u8> {
    let fill_hex = format!("#{:02x}{:02x}{:02x}", fill.r(), fill.g(), fill.b());
    let stroke_hex = format!("#{:02x}{:02x}{:02x}", stroke.r(), stroke.g(), stroke.b());
    let mut svg = String::from_utf8_lossy(svg_bytes).replace("currentColor", fill_hex.as_str());
    svg = svg.replace(
        "<path d=\"M8.243",
        format!(
            "<path fill=\"{}\" stroke=\"{}\" stroke-width=\"2.0\" stroke-linejoin=\"round\" paint-order=\"stroke\" d=\"M8.243",
            fill_hex, stroke_hex
        )
        .as_str(),
    );
    svg.into_bytes()
}

thread_local! {
    static TINTED_SVG_CACHE: std::cell::RefCell<HashMap<(usize, usize, Color32), Arc<[u8]>>> =
        std::cell::RefCell::new(HashMap::new());
    static STAR_SVG_CACHE: std::cell::RefCell<HashMap<(Color32, Color32), Arc<[u8]>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Tinted icon bytes, built once per `(icon, color)` instead of every frame for every row.
fn cached_tinted_svg(icon_svg: &'static [u8], color: Color32) -> Arc<[u8]> {
    let key = (icon_svg.as_ptr() as usize, icon_svg.len(), color);
    TINTED_SVG_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| Arc::from(crate::ui::svg_tint::tint_svg(icon_svg, color)))
            .clone()
    })
}

/// Star icon bytes, built once per `(fill, outline)` instead of every frame for every row.
fn cached_star_svg(fill: Color32, outline: Color32) -> Arc<[u8]> {
    STAR_SVG_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((fill, outline))
            .or_insert_with(|| {
                Arc::from(apply_star_fill_and_stroke_svg(
                    assets::STAR_SVG,
                    fill,
                    outline,
                ))
            })
            .clone()
    })
}
