use super::*;

pub(super) fn matches_search(search: &str, label: &str, key: &str) -> bool {
    matches_needle(&search.trim().to_lowercase(), label, key)
}

/// [`matches_search`] with the trimmed, lowercased needle already computed.
fn matches_needle(needle: &str, label: &str, key: &str) -> bool {
    needle.is_empty()
        || label.to_lowercase().contains(needle)
        || key.to_lowercase().contains(needle)
}

/// Like [`matches_search`], but also looks in the option's description, so people can search for
/// what a setting does ("shadows", "gamma") and not only for its name.
pub(super) fn def_matches_search(search: &str, def: &OptionDef) -> bool {
    let needle = search.trim().to_lowercase();
    needle.is_empty()
        || matches_needle(&needle, def.label, def.key)
        || mc_options::descriptions::description(def.key)
            .is_some_and(|text| text.to_lowercase().contains(&needle))
}


