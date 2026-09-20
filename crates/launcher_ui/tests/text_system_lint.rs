//! Guards the text system: UI text must be drawn with textui using the launcher's text
//! roles, not with egui's own text or hardcoded sizes.
//!
//! A line can opt out with a trailing `// text-system: allow (reason)`.

use std::fs;
use std::path::{Path, PathBuf};

const ALLOW_MARKER: &str = "text-system: allow";

/// `(needle, why)`. Matched per line, outside comments.
const FORBIDDEN: &[(&str, &str)] = &[
    (
        "LabelOptions::default()",
        "use a text role (crate::ui::style::body/caption/role) so the label follows font settings",
    ),
    (
        ".on_hover_text(",
        "egui tooltips use egui's fonts; use crate::ui::style::hover_tip",
    ),
    ("RichText", "egui text; draw with textui"),
    ("egui::Label", "egui text; draw with textui"),
    ("egui::Button::new", "egui text; use text_ui.button"),
    ("egui::DragValue", "egui text; use a textui input"),
    (
        ".show_percentage(",
        "egui text; use components::progress_bar",
    ),
    ("layout_job(", "egui text layout; use textui"),
    ("layout_no_wrap(", "egui text layout; measure with textui"),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// True if `needle` appears as a whole token: type-like needles must not be part of a longer
/// identifier (`egui::Label` vs `textui_egui::LabelOptions`, `RichText` vs `RichTextSpan`).
fn contains_token(line: &str, needle: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(needle).any(|(at, _)| {
        if needle.ends_with('(') {
            return true;
        }
        let before_ok = line[..at].chars().next_back().is_none_or(|c| !is_ident(c));
        let after_ok = line[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|c| !is_ident(c));
        before_ok && after_ok
    })
}

/// A numeric literal assigned to a font size, like `font_size: 14.0` or `font_size = 14.0`.
fn has_literal_font_size(line: &str) -> bool {
    ["font_size", "line_height"].iter().any(|field| {
        line.match_indices(field).any(|(at, _)| {
            let rest = line[at + field.len()..].trim_start();
            let value = rest.strip_prefix(':').or_else(|| rest.strip_prefix('='));
            value.is_some_and(|v| v.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        })
    })
}

#[test]
fn ui_text_goes_through_textui_and_text_roles() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&manifest.join("src"), &mut files);
    rust_files(&manifest.join("../vertexlauncher/src"), &mut files);

    let mut violations = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || line.contains(ALLOW_MARKER) {
                continue;
            }
            let location = format!("{}:{}", file.display(), index + 1);
            for (needle, why) in FORBIDDEN {
                if contains_token(line, needle) {
                    violations.push(format!("{location}: `{needle}` - {why}"));
                }
            }
            if has_literal_font_size(line) {
                violations.push(format!(
                    "{location}: hardcoded font size/line height - use a text role in Settings > Text styles"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "text system violations:\n{}",
        violations.join("\n")
    );
}
