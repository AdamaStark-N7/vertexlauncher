//! Guards the interaction rules documented in `textui_egui::interaction`: hover, press, focus
//! and cursor feedback must come from that module, not from egui's built-in widget visuals or
//! hand-rolled color choices.
//!
//! A line can opt out with a trailing `// interaction: allow (reason)`. A file whose clickable
//! areas have no visuals of their own can opt out with the same marker anywhere in it.

use std::fs;
use std::path::{Path, PathBuf};

const ALLOW_MARKER: &str = "interaction: allow";

/// `(needle, why)`. Matched per line, outside comments.
const FORBIDDEN: &[(&str, &str)] = &[
    (
        "ui.style().interact(",
        "egui's built-in hover visuals skip the fade and cursor rules; use textui_egui::interaction",
    ),
    (
        ".interact_selectable(",
        "egui's built-in hover visuals skip the fade and cursor rules; use textui_egui::interaction",
    ),
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

fn source_files() -> Vec<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&manifest.join("src"), &mut files);
    rust_files(&manifest.join("../vertexlauncher/src"), &mut files);
    files
}

#[test]
fn hover_visuals_come_from_the_interaction_module() {
    let mut violations = Vec::new();
    for file in source_files() {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") || line.contains(ALLOW_MARKER) {
                continue;
            }
            for (needle, why) in FORBIDDEN {
                if line.contains(needle) {
                    violations.push(format!(
                        "{}:{}: `{needle}` - {why}",
                        file.display(),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "interaction rule violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn files_that_draw_clickable_areas_use_the_interaction_module() {
    let mut violations = Vec::new();
    for file in source_files() {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let makes_clickable = text.lines().any(|line| {
            !line.trim_start().starts_with("//")
                && (line.contains("Sense::click()") || line.contains("Sense::click_and_drag()"))
        });
        if makes_clickable && !text.contains("interaction::") && !text.contains(ALLOW_MARKER) {
            violations.push(format!(
                "{}: creates clickable areas but never uses textui_egui::interaction",
                file.display()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "interaction rule violations:\n{}",
        violations.join("\n")
    );
}
