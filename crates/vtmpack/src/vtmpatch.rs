use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::export::{
    append_bytes_to_archive, non_empty, normalize_pack_path, progress_update,
    xz_preset_for_compression_mode,
};
use managed_content::{
    CONTENT_MANIFEST_FILE_NAME, ContentInstallManifest, ManagedContentSource, content_manifest_path,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as Sha2Digest, Sha512};

use crate::{
    VTMPACK_MANIFEST_VERSION, VtmpackDownloadableEntry, VtmpackExportOptions,
    VtmpackExportProgress, VtmpackExportStats, VtmpackManifest, VtmpackProviderMode,
};

#[path = "vtmpatch/patch_apply.rs"]
mod patch_apply;
#[path = "vtmpatch/patch_export.rs"]
mod patch_export;
#[path = "vtmpatch/patch_paths.rs"]
mod patch_paths;
#[path = "vtmpatch/provider_download.rs"]
mod provider_download;
#[path = "vtmpatch/snapshot.rs"]
mod snapshot;

pub use self::patch_apply::apply_vtmpatch_to_instance;
pub use self::patch_export::export_instance_as_vtmpatch_with_progress;
use self::patch_paths::*;
use self::provider_download::*;
use self::snapshot::*;

const PATCH_MANIFEST_PATH: &str = "patch.toml";
const LOG: &str = "vertexlauncher/vtmpatch";
/// Archive path used for the managed-content manifest inside both `.vtmpack`
/// and `.vtmpatch` archives.  Must match the hard-coded string in export.rs.
/// Note: the on-disk filename is `.vertex-content-manifest.toml` (leading dot),
/// but the archive path intentionally omits the dot so it isn't hidden on
/// archive inspection tools.
const CONTENT_MANIFEST_ARCHIVE_PATH: &str = "metadata/vertex-content-manifest.toml";

/// Patch manifest stored inside a `.vtmpatch` archive.
///
/// Files in the patch fall into three categories:
///
/// * **`downloadable_entries`** — mods (and other content) that were resolved
///   to a Modrinth version.  Only their metadata is stored here; the apply
///   step downloads the actual bytes from the CDN.  This is the main reason
///   patches are small: a 20 MB JAR becomes ~300 bytes of TOML.
///
/// * **`bundled_paths`** — files that could *not* be resolved on Modrinth
///   (custom mods, datapacks, local configs, etc.) and are therefore embedded
///   verbatim in the archive, compressed with XZ.
///
/// * **`removed_paths`** — paths that existed in the base pack but are absent
///   from the target; they are deleted during apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VtmpatchManifest {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generated_at_ms: u64,
    #[serde(default)]
    pub base_instance_name: String,
    #[serde(default)]
    pub target_instance_name: String,
    /// Content resolved to Modrinth versions — downloaded, not embedded.
    #[serde(default)]
    pub downloadable_entries: Vec<VtmpackDownloadableEntry>,
    /// Files embedded in the archive (Modrinth lookup failed or not a mod).
    /// Legacy field name `added_or_changed_paths` is accepted for old patches.
    #[serde(default, alias = "added_or_changed_paths")]
    pub bundled_paths: Vec<PathBuf>,
    #[serde(default)]
    pub removed_paths: Vec<PathBuf>,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn default_vtmpatch_file_name(instance_name: &str) -> String {
    let base = crate::default_vtmpack_file_name(instance_name);
    base.strip_suffix(".vtmpack")
        .map(|name| format!("{name}.vtmpatch"))
        .unwrap_or_else(|| "instance.vtmpatch".to_owned())
}

#[must_use]
pub fn enforce_vtmpatch_extension(mut path: PathBuf) -> PathBuf {
    let has_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(crate::VTMPATCH_EXTENSION));
    if !has_extension {
        path.set_extension(crate::VTMPATCH_EXTENSION);
    }
    path
}
