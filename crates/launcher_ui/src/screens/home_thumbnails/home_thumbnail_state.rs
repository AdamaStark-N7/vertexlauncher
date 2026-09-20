use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct HomeThumbnailState {
    pub(crate) cache_frame_index: u64,
    pub(crate) results: launcher_runtime::WorkerChannel<(String, Option<Arc<[u8]>>)>,
    pub(crate) cache: HashMap<String, ThumbnailCacheEntry>,
    pub(crate) in_flight: HashSet<String>,
}
