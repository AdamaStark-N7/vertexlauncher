//! Painting and viewer math shared by the home screenshot gallery and the per-instance one.
//!
//! Both galleries render the same masonry tiles and the same zoom/pan viewer; only their egui
//! id salts, constants, and state types differ, so those are passed in by the callers.

use std::ffi::OsStr;
use std::path::Path;







pub(in crate::screens) fn is_supported_screenshot_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .map(|extension| extension.to_ascii_lowercase()),
        Some(extension)
            if matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
            )
    )
}






















