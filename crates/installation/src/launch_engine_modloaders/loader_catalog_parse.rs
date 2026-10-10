use super::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct LoaderVersionCatalog {
    pub(crate) supported_game_versions: HashSet<String>,
    pub(crate) versions_by_game_version: BTreeMap<String, Vec<String>>,
}

impl LoaderVersionCatalog {
    fn finalize(mut self) -> Self {
        self.supported_game_versions = self.versions_by_game_version.keys().cloned().collect();
        sort_loader_version_map_desc(&mut self.versions_by_game_version);
        self
    }
}

pub(crate) fn parse_minecraft_versions_from_maven_metadata(
    metadata_xml: &str,
    read_prefix_before_dash: bool,
) -> HashSet<String> {
    parse_xml_versions(metadata_xml)
        .into_iter()
        .filter_map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }

            let candidate = if read_prefix_before_dash {
                trimmed.split('-').next().unwrap_or(trimmed)
            } else {
                trimmed
            };

            if is_probable_minecraft_version(candidate) {
                Some(candidate.to_owned())
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn parse_loader_version_matrix(matrix: &serde_json::Value) -> LoaderVersionCatalog {
    let mut catalog = LoaderVersionCatalog::default();

    match matrix {
        serde_json::Value::Array(entries) => {
            collect_loader_versions_from_entries(entries, &mut catalog);
        }
        serde_json::Value::Object(object) => {
            // Support alternate wrappers some APIs use.
            for key in ["loader", "versions", "data"] {
                if let Some(entries) = object.get(key).and_then(serde_json::Value::as_array) {
                    collect_loader_versions_from_entries(entries, &mut catalog);
                }
            }
        }
        _ => {}
    }

    catalog.finalize()
}

pub(crate) fn collect_loader_versions_from_entries(
    entries: &[serde_json::Value],
    catalog: &mut LoaderVersionCatalog,
) {
    for entry in entries {
        let Some(entry) = entry.as_object() else {
            continue;
        };

        let Some(game_version) = extract_game_version_from_loader_entry(entry) else {
            continue;
        };
        let Some(loader_version) = extract_loader_version_from_loader_entry(entry) else {
            continue;
        };

        push_unique_loader_version(
            &mut catalog.versions_by_game_version,
            game_version.as_str(),
            loader_version,
        );
    }
}

pub(crate) fn parse_global_loader_versions(matrix: &serde_json::Value) -> Vec<String> {
    let mut versions = Vec::new();
    let mut seen = HashSet::new();
    let mut push_unique = |candidate: String| {
        if seen.insert(candidate.clone()) {
            versions.push(candidate);
        }
    };

    match matrix {
        serde_json::Value::Array(entries) => {
            collect_global_loader_versions_from_entries(entries, &mut push_unique);
        }
        serde_json::Value::Object(object) => {
            let mut found_wrapped_entries = false;
            for key in ["loader", "versions", "data"] {
                if let Some(entries) = object.get(key).and_then(serde_json::Value::as_array) {
                    found_wrapped_entries = true;
                    collect_global_loader_versions_from_entries(entries, &mut push_unique);
                }
            }
            if !found_wrapped_entries {
                if let Some(version) = extract_loader_version_from_loader_entry(object) {
                    push_unique(version);
                } else if let Some(version) = object
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                {
                    push_unique(version);
                }
            }
        }
        _ => {}
    }

    sort_loader_versions_desc(versions)
}

pub(crate) fn collect_global_loader_versions_from_entries<F>(
    entries: &[serde_json::Value],
    push_unique: &mut F,
) where
    F: FnMut(String),
{
    for entry in entries {
        let Some(object) = entry.as_object() else {
            continue;
        };
        if let Some(version) = extract_loader_version_from_loader_entry(object) {
            push_unique(version);
            continue;
        }
        if let Some(version) = object
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
        {
            push_unique(version);
        }
    }
}

pub(crate) fn extract_game_version_from_loader_entry(
    entry: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    // Fabric/Quilt loader endpoints commonly encode Minecraft version in "intermediary.version".
    for key in [
        "game",
        "minecraft",
        "minecraft_version",
        "mcversion",
        "intermediary",
    ] {
        if let Some(version) = entry.get(key).and_then(extract_version_from_json_value)
            && is_probable_minecraft_version(version.as_str())
        {
            return Some(version);
        }
    }

    // Fallback: check all object fields for a probable MC version string.
    entry
        .values()
        .find_map(extract_version_from_json_value)
        .filter(|version| is_probable_minecraft_version(version.as_str()))
}

pub(crate) fn extract_loader_version_from_loader_entry(
    entry: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    for key in ["loader", "loader_version", "version"] {
        if let Some(version) = entry.get(key).and_then(extract_version_from_json_value) {
            return Some(version);
        }
    }
    None
}

pub(crate) fn parse_forge_loader_catalog_from_metadata(metadata_xml: &str) -> LoaderVersionCatalog {
    let mut catalog = LoaderVersionCatalog::default();
    for raw in parse_xml_versions(metadata_xml) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((game_version, loader_version)) = trimmed.split_once('-') else {
            continue;
        };
        let game_version = game_version.trim();
        let loader_version = loader_version.trim();
        if game_version.is_empty()
            || loader_version.is_empty()
            || !is_probable_minecraft_version(game_version)
        {
            continue;
        }
        push_unique_loader_version(
            &mut catalog.versions_by_game_version,
            game_version,
            loader_version.to_owned(),
        );
    }
    catalog.finalize()
}

pub(crate) fn parse_neoforge_loader_catalog_from_metadata(
    metadata_xml: &str,
) -> LoaderVersionCatalog {
    let mut catalog = LoaderVersionCatalog::default();
    for raw in parse_xml_versions(metadata_xml) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(game_version) = infer_neoforge_game_version(trimmed) else {
            continue;
        };
        push_unique_loader_version(
            &mut catalog.versions_by_game_version,
            game_version.as_str(),
            trimmed.to_owned(),
        );
    }
    catalog.finalize()
}

pub(crate) fn parse_neoforge_versions_from_metadata(metadata_xml: &str) -> HashSet<String> {
    parse_xml_versions(metadata_xml)
        .into_iter()
        .filter_map(|version| {
            let prefix = version.split('-').next().unwrap_or(version.as_str());
            let mut segments = prefix.split('.');
            let major = segments.next()?.parse::<u32>().ok()?;
            let minor = segments.next()?.parse::<u32>().ok()?;
            Some(format!("1.{major}.{minor}"))
        })
        .collect()
}

pub(crate) fn infer_neoforge_game_version(raw: &str) -> Option<String> {
    let prefix = raw.split('-').next().unwrap_or(raw).trim();
    if prefix.is_empty() {
        return None;
    }
    if is_probable_minecraft_version(prefix) {
        return Some(prefix.to_owned());
    }

    let mut segments = prefix.split('.');
    let major = segments.next()?.parse::<u32>().ok()?;
    let minor = segments.next()?.parse::<u32>().ok()?;
    Some(format!("1.{major}.{minor}"))
}

pub(crate) fn extract_version_from_json_value(value: &serde_json::Value) -> Option<String> {
    if let Some(raw) = value.as_str() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(trimmed.to_owned());
    }

    let object = value.as_object()?;
    for key in ["version", "id", "name"] {
        let Some(raw) = object.get(key).and_then(serde_json::Value::as_str) else {
            continue;
        };
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    None
}

pub(crate) fn push_unique_loader_version(
    versions_by_game_version: &mut BTreeMap<String, Vec<String>>,
    game_version: &str,
    loader_version: String,
) {
    let versions = versions_by_game_version
        .entry(game_version.to_owned())
        .or_default();
    if !versions.iter().any(|existing| existing == &loader_version) {
        versions.push(loader_version);
    }
}

pub(crate) fn sort_loader_version_map_desc(
    versions_by_game_version: &mut BTreeMap<String, Vec<String>>,
) {
    for versions in versions_by_game_version.values_mut() {
        sort_loader_versions_desc_in_place(versions);
    }
}

pub(crate) fn merge_loader_catalog(
    target: &mut LoaderVersionCatalog,
    source: LoaderVersionCatalog,
) {
    for game_version in source.supported_game_versions {
        target.supported_game_versions.insert(game_version);
    }
    for (game_version, versions) in source.versions_by_game_version {
        for version in versions {
            push_unique_loader_version(
                &mut target.versions_by_game_version,
                &game_version,
                version,
            );
        }
    }
}

pub(crate) fn parse_xml_versions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    const START: &str = "<version>";
    const END: &str = "</version>";

    while let Some(start_offset) = xml[cursor..].find(START) {
        let start_index = cursor + start_offset + START.len();
        let Some(end_offset) = xml[start_index..].find(END) else {
            break;
        };
        let end_index = start_index + end_offset;
        out.push(xml[start_index..end_index].to_owned());
        cursor = end_index + END.len();
    }

    out
}

pub(crate) fn is_probable_minecraft_version(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };
    if !first.chars().all(|ch| ch.is_ascii_digit()) || !second.chars().all(|ch| ch.is_ascii_digit())
    {
        return false;
    }

    if !first.starts_with('1') {
        return false;
    }

    segments.all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
}
