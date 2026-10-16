//! Search-tag chips and tag list helpers shared by the content browser and discover screens.
//!
//! This file is mounted as a module by both screens (`#[path]`), so it only relies on the
//! prelude items both parents import.




pub(in crate::screens) fn add_search_tag(search_tags: &mut Vec<String>, candidate: &str) -> bool {
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


