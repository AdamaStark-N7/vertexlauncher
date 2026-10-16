use super::*;



/// Key → labels of every binding using it, to flag duplicates.
pub(super) fn keybind_usage(
    live: &LiveOptions,
    version: Option<Ver>,
) -> BTreeMap<String, Vec<String>> {
    let legacy = version.is_some_and(|v| v < Ver(1, 13, 0));
    let mut usage: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in live.file().keys() {
        if !schema::is_keybind_key(key) {
            continue;
        }
        let raw = live.get(key).unwrap_or_default();
        let modern = if legacy {
            raw.parse::<i32>().ok().and_then(keys::legacy_to_modern)
        } else {
            Some(raw.to_owned())
        };
        if let Some(modern) = modern {
            let label = schema::find_def(key, version)
                .map_or_else(|| pretty_key(key), |d| d.label.to_owned());
            usage.entry(modern).or_default().push(label);
        }
    }
    usage
}

pub(super) fn pretty_key(key: &str) -> String {
    key.trim_start_matches("key_key.")
        .trim_start_matches("key_")
        .replace(['.', '_'], " ")
}
