use super::*;

#[derive(Default)]
pub struct ImportInstanceState {
    pub source_mode_index: usize,
    pub package_path: PathBuf,
    pub launcher_path: PathBuf,
    pub launcher_kind_index: usize,
    pub instance_name: String,
    pub error: Option<String>,
    pub(crate) preview_in_flight: bool,
    pub(crate) preview_status_message: Option<String>,
    pub(crate) preview_request_serial: u64,
    pub(crate) preview_results:
        launcher_runtime::WorkerChannel<(u64, Result<ImportPreview, String>)>,
    pub(crate) preview_progress: launcher_runtime::WorkerChannel<(u64, String)>,
    pub import_in_flight: bool,
    pub import_latest_progress: Option<ImportProgress>,
    pub import_progress: launcher_runtime::WorkerChannel<ImportProgress>,
    pub import_results: launcher_runtime::WorkerChannel<ImportTaskResult>,
    pub(crate) preview: Option<ImportPreview>,
}

impl ImportInstanceState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
