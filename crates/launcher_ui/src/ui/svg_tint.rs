//! Recoloring of the launcher's monochrome SVG icons.
//!
//! Icons are authored with `currentColor`; tinting swaps in a concrete color so the same
//! asset follows the theme.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

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
///
/// The tinted bytes and URI are memoized per `(scope, icon_id, color)` so repeated per-frame
/// calls only clone a URI string and bump an `Arc` instead of re-tinting the SVG.
pub fn themed_svg_image(
    scope: &str,
    icon_id: &str,
    svg_bytes: &[u8],
    color: Color32,
    icon_size: f32,
) -> egui::Image<'static> {
    let (uri, bytes) = tinted_icon(scope, icon_id, svg_bytes, color);
    egui::Image::from_bytes(uri, bytes).fit_to_exact_size(egui::vec2(icon_size, icon_size))
}

struct TintedIcon {
    scope: String,
    icon_id: String,
    rgb: [u8; 3],
    source: Vec<u8>,
    uri: String,
    bytes: Arc<[u8]>,
}

fn tinted_icon(
    scope: &str,
    icon_id: &str,
    svg_bytes: &[u8],
    color: Color32,
) -> (String, Arc<[u8]>) {
    static CACHE: OnceLock<Mutex<HashMap<u64, TintedIcon>>> = OnceLock::new();
    let rgb = [color.r(), color.g(), color.b()];
    let mut hasher = DefaultHasher::new();
    (scope, icon_id, rgb, svg_bytes.len()).hash(&mut hasher);
    let key = hasher.finish();

    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(entry) = cache.get(&key)
        && entry.rgb == rgb
        && entry.scope == scope
        && entry.icon_id == icon_id
        && entry.source == svg_bytes
    {
        return (entry.uri.clone(), Arc::clone(&entry.bytes));
    }

    let uri = format!(
        "bytes://vertex-{scope}-icons/{icon_id}-{}.svg",
        &hex(color)[1..]
    );
    let bytes: Arc<[u8]> = tint_svg(svg_bytes, color).into();
    cache.insert(
        key,
        TintedIcon {
            scope: scope.to_owned(),
            icon_id: icon_id.to_owned(),
            rgb,
            source: svg_bytes.to_vec(),
            uri: uri.clone(),
            bytes: Arc::clone(&bytes),
        },
    );
    (uri, bytes)
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
