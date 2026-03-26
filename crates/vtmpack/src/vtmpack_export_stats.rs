#[derive(Debug, Clone, Copy)]
pub struct VtmpackExportStats {
    pub bundled_mod_files: usize,
    pub config_files: usize,
    pub additional_files: usize,
}

#[derive(Debug, Clone)]
pub struct VtmpackExportProgress {
    pub message: String,
    pub completed_steps: usize,
    pub total_steps: usize,
}
