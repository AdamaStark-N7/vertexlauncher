use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct LibraryState {
    pub(super) runtime: RuntimeWorkflowState,
    pub(super) thumbnail_cache_frame_index: u64,
    pub(super) thumbnail_results: launcher_runtime::WorkerChannel<(String, Option<Arc<[u8]>>)>,
    pub(super) thumbnail_cache: HashMap<String, ThumbnailCacheEntry>,
    pub(super) thumbnail_in_flight: HashSet<String>,
}
