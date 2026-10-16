//! Guards the SE5 front end's widget layer: every piece of text and every control is drawn through `se5/kit.rs`, which
//! applies the launcher's text roles, streamer-mode name scrubbing, theme colours and keyboard/controller focus.
//!
//! A line can opt out with a trailing `// kit: allow (reason)`.

use std::fs;
use std::path::{Path, PathBuf};

const ALLOW_MARKER: &str = "kit: allow";

/// Engine calls that must only appear in the kit.
const FORBIDDEN: &[&str] = &[
    "ui.text(",
    "ui.button(",
    "ui.switch(",
    "ui.slider(",
    "ui.dropdown(",
    "ui.text_field(",
    "ui.text_area(",
    "ui.checkbox(",
    "ui.radio_button(",
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

#[test]
fn widgets_and_text_go_through_the_kit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/se5");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let mut problems = Vec::new();
    for file in files {
        if file.file_name().is_some_and(|name| name == "kit.rs") {
            continue;
        }
        let text = fs::read_to_string(&file).expect("source file is readable");
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if line.contains(ALLOW_MARKER) {
                continue;
            }
            for needle in FORBIDDEN {
                if code.contains(needle) {
                    problems.push(format!(
                        "{}:{}: `{needle}` belongs in se5/kit.rs",
                        file.display(),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
