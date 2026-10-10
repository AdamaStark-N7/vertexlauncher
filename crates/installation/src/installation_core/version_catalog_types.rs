use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MinecraftVersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    Unknown,
}

impl MinecraftVersionType {
    pub fn label(self) -> &'static str {
        match self {
            MinecraftVersionType::Release => "Release",
            MinecraftVersionType::Snapshot => "Snapshot",
            MinecraftVersionType::OldBeta => "Old Beta",
            MinecraftVersionType::OldAlpha => "Old Alpha",
            MinecraftVersionType::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCatalogFilter {
    pub include_snapshots_and_betas: bool,
    pub include_alpha: bool,
    pub include_experimental: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinecraftVersionEntry {
    pub id: String,
    pub version_type: MinecraftVersionType,
    /// ISO-8601 release time from Mojang's manifest; the authoritative "newer than" order.
    /// Absent in catalogs cached by older launcher versions.
    #[serde(default)]
    pub release_time: Option<String>,
}

impl MinecraftVersionEntry {
    pub fn display_label(&self) -> String {
        format!("{} ({})", self.id, self.version_type.label())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderSupportIndex {
    pub fabric: HashSet<String>,
    pub forge: HashSet<String>,
    pub neoforge: HashSet<String>,
    pub quilt: HashSet<String>,
}

impl LoaderSupportIndex {
    pub fn supports_loader(&self, loader_label: &str, game_version: &str) -> bool {
        match normalized_loader_label(loader_label) {
            LoaderKind::Vanilla => true,
            LoaderKind::Fabric => self.fabric.contains(game_version),
            LoaderKind::Forge => self.forge.contains(game_version),
            LoaderKind::NeoForge => self.neoforge.contains(game_version),
            LoaderKind::Quilt => self.quilt.contains(game_version),
            LoaderKind::Custom => true,
        }
    }

    pub fn unavailable_reason(&self, loader_label: &str, game_version: &str) -> Option<String> {
        if self.supports_loader(loader_label, game_version) {
            None
        } else {
            Some(format!(
                "{loader_label} is not available for Minecraft {game_version}"
            ))
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderVersionIndex {
    pub fabric: BTreeMap<String, Vec<String>>,
    pub forge: BTreeMap<String, Vec<String>>,
    pub neoforge: BTreeMap<String, Vec<String>>,
    pub quilt: BTreeMap<String, Vec<String>>,
}

impl LoaderVersionIndex {
    pub fn versions_for_loader(&self, loader_label: &str, game_version: &str) -> Option<&[String]> {
        match normalized_loader_label(loader_label) {
            LoaderKind::Fabric => self.fabric.get(game_version).map(Vec::as_slice),
            LoaderKind::Forge => self.forge.get(game_version).map(Vec::as_slice),
            LoaderKind::NeoForge => self.neoforge.get(game_version).map(Vec::as_slice),
            LoaderKind::Quilt => self.quilt.get(game_version).map(Vec::as_slice),
            LoaderKind::Vanilla | LoaderKind::Custom => None,
        }
    }

    pub(crate) fn sort_desc(&mut self) {
        sort_loader_version_map_desc(&mut self.fabric);
        sort_loader_version_map_desc(&mut self.forge);
        sort_loader_version_map_desc(&mut self.neoforge);
        sort_loader_version_map_desc(&mut self.quilt);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCatalog {
    pub game_versions: Vec<MinecraftVersionEntry>,
    pub loader_support: LoaderSupportIndex,
    #[serde(default)]
    pub loader_versions: LoaderVersionIndex,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CachedVersionCatalog {
    pub(crate) fetched_at_unix_secs: u64,
    #[serde(default)]
    pub(crate) filter: VersionCatalogFilter,
    pub(crate) catalog: VersionCatalog,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CachedLoaderVersions {
    pub(crate) fetched_at_unix_secs: u64,
    pub(crate) loader_label: String,
    pub(crate) versions_by_game_version: BTreeMap<String, Vec<String>>,
}
