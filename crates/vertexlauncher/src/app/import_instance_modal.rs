use logged_fs::copy as fs_copy_logged;
use logged_fs::create_dir_all as fs_create_dir_all_logged;
use logged_fs::file_open as fs_file_open_logged;
use logged_fs::read_dir as fs_read_dir_logged;
use logged_fs::read_to_string as fs_read_to_string_logged;
use logged_fs::remove_dir_all as fs_remove_dir_all_logged;
use logged_fs::rename as fs_rename_logged;
use logged_fs::write as fs_write_logged;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use content_resolver::{InstalledContentKind, detect_installed_content_kind};
use curseforge::Client as CurseForgeClient;
use instances::{
    InstanceRecord, InstanceStore, NewInstanceSpec, create_instance, delete_instance,
    instance_root_path,
};
use launcher_runtime as tokio_runtime;
use managed_content::{
    CONTENT_MANIFEST_FILE_NAME, ContentInstallManifest, InstalledContentProject,
    ManagedContentSource, ModpackInstallState, load_content_manifest, load_modpack_install_state,
    remove_modpack_install_state, save_content_manifest, save_modpack_install_state,
};
use modrinth::Client as ModrinthClient;
use serde::Deserialize;
use serde_json::Value;
use vtmpack::{
    VtmpackDownloadableEntry, VtmpackManifest, open_vtmpack_tar_archive,
    read_vtmpack_manifest_with_progress,
};

const MODAL_GAP_SM: f32 = 6.0;
const MODAL_GAP_MD: f32 = 8.0;
const MODAL_GAP_LG: f32 = 10.0;
const ACTION_BUTTON_MAX_WIDTH: f32 = 260.0;
const MODRINTH_DOWNLOAD_MIN_SPACING: Duration = Duration::from_millis(250);
const CURSEFORGE_DOWNLOAD_MIN_SPACING: Duration = Duration::from_millis(500);

mod inspection;
mod package_import;
mod render_impl;
mod state;

/// "Fabric 0.16.0" style label for an import preview.
pub(crate) fn loader_label_for_display(modloader: &str, version: &str) -> String {
    format_loader_label(modloader, version)
}
pub use package_import::{
    CurseForgeManualDownloadRequirement, attach_curseforge_modpack_install_state,
    format_curseforge_download_url_error, prepare_curseforge_manual_download_for_file,
    prepare_curseforge_manual_downloads,
};
pub use render_impl::import_package_with_progress;
pub(crate) use render_impl::load_preview_from_state;
pub use state::{
    ImportInstanceState, ImportPackageError, ImportProgress, ImportRequest, ImportSource,
    ImportTaskResult,
};
pub(crate) use state::{
    ImportMode, ImportPackageKind, ImportPreviewKind, LAUNCHER_KIND_OPTIONS, path_input_string,
    poll_preview_results, selected_import_mode, selected_launcher_hint, update_path_from_input,
};

use self::{inspection::*, package_import::*, state::*};
