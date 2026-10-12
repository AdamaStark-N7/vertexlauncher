//! Guards the account name system: names reach the user as mentions (`account_text::mention`)
//! drawn by `ui::account_label`, so streamer mode hides and highlights them everywhere,
//! including text created before streamer mode was switched on.
//!
//! `privacy::redact_account_label` bakes the decision into a string at the moment it is called, so
//! new code should not use it. A line can opt out with `// account-text: allow (reason)` on the
//! line or one of the two lines above.

use std::fs;
use std::path::{Path, PathBuf};

const ALLOW_MARKER: &str = "account-text: allow";
const FORBIDDEN: &str = "redact_account_label(";

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
fn names_are_mentioned_not_redacted_into_strings() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&manifest.join("src"), &mut files);
    rust_files(&manifest.join("../vertexlauncher/src"), &mut files);

    let mut violations = Vec::new();
    for file in files {
        // The helper itself lives here.
        if file.ends_with("src/privacy.rs") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("//") || !line.contains(FORBIDDEN) {
                continue;
            }
            // rustfmt may split `let x =` from the call, so the marker may be two lines up.
            let allowed = line.contains(ALLOW_MARKER)
                || (1..=2).any(|back| {
                    index
                        .checked_sub(back)
                        .is_some_and(|previous| lines[previous].contains(ALLOW_MARKER))
                });
            if !allowed {
                violations.push(format!(
                    "{}:{}: write the account with `account_text::mention` and draw it with \
                     `ui::account_label` instead of `redact_account_label`",
                    file.display(),
                    index + 1
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "account name violations:\n{}",
        violations.join("\n")
    );
}
