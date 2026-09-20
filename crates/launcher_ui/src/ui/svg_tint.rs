//! Recoloring of the launcher's monochrome SVG icons.
//!
//! Icons are authored with `currentColor`; tinting swaps in a concrete color so the same
//! asset follows the theme.

use egui::Color32;

/// `#rrggbb` for `color` (alpha is ignored; SVG opacity is applied by the image widget).
pub fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

/// Replaces every `currentColor` in `svg_bytes` with `color`.
pub fn tint_svg(svg_bytes: &[u8], color: Color32) -> Vec<u8> {
    String::from_utf8_lossy(svg_bytes)
        .replace("currentColor", &hex(color))
        .into_bytes()
}

/// A tinted icon as an `egui::Image`, cached by `(scope, icon_id, color)`.
/// `scope` keeps unrelated icon sets from colliding in egui's byte-image cache.
pub fn themed_svg_image(
    scope: &str,
    icon_id: &str,
    svg_bytes: &[u8],
    color: Color32,
    icon_size: f32,
) -> egui::Image<'static> {
    let uri = format!(
        "bytes://vertex-{scope}-icons/{icon_id}-{}.svg",
        &hex(color)[1..]
    );
    egui::Image::from_bytes(uri, tint_svg(svg_bytes, color))
        .fit_to_exact_size(egui::vec2(icon_size, icon_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_current_color_everywhere() {
        let svg = br#"<svg stroke="currentColor"><path fill="currentColor"/></svg>"#;
        let tinted = String::from_utf8(tint_svg(svg, Color32::from_rgb(255, 128, 0))).unwrap();
        assert_eq!(
            tinted,
            r##"<svg stroke="#ff8000"><path fill="#ff8000"/></svg>"##
        );
    }
}
