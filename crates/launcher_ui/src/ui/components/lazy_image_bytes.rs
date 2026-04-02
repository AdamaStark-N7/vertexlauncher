use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use crate::app::tokio_runtime;

const LAZY_IMAGE_MAX_BYTES: usize = 64 * 1024 * 1024;
const LAZY_IMAGE_STALE_FRAMES: u64 = 900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LazyImageBytesStatus {
    Unrequested,
    Loading,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
enum LazyImageBytesState {
    Loading,
    Ready(Arc<[u8]>),
    Failed,
}

#[derive(Clone, Debug)]
struct LazyImageEntry {
    state: LazyImageBytesState,
    last_touched_frame: u64,
    approx_bytes: usize,
}

#[derive(Clone, Debug, Default)]
pub struct LazyImageBytes {
    states: HashMap<String, LazyImageEntry>,
    frame_index: u64,
    results_tx: Option<mpsc::Sender<(String, Result<Arc<[u8]>, String>)>>,
    results_rx: Option<Arc<Mutex<mpsc::Receiver<(String, Result<Arc<[u8]>, String>)>>>>,
}

impl LazyImageBytes {
    pub fn begin_frame(&mut self) {
        self.frame_index = self.frame_index.saturating_add(1);
        self.trim_stale();
        self.trim_to_budget();
    }

    pub fn poll(&mut self) -> bool {
        let mut updates = Vec::new();
        let mut should_reset = false;
        if let Some(rx) = self.results_rx.as_ref() {
            match rx.lock() {
                Ok(receiver) => loop {
                    match receiver.try_recv() {
                        Ok(update) => updates.push(update),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            tracing::error!(
                                target: "vertexlauncher/lazy_image",
                                "Lazy image worker disconnected unexpectedly."
                            );
                            should_reset = true;
                            break;
                        }
                    }
                },
                Err(_) => {
                    tracing::error!(
                        target: "vertexlauncher/lazy_image",
                        "Lazy image receiver mutex was poisoned."
                    );
                    should_reset = true;
                }
            }
        }

        if should_reset {
            self.results_tx = None;
            self.results_rx = None;
        }

        let mut did_update = false;
        for (key, result) in updates {
            match result {
                Ok(bytes) => {
                    self.states.insert(
                        key,
                        LazyImageEntry {
                            approx_bytes: bytes.len(),
                            last_touched_frame: self.frame_index,
                            state: LazyImageBytesState::Ready(bytes),
                        },
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        target: "vertexlauncher/lazy_image",
                        image_key = %key,
                        error = %err,
                        "Lazy image load failed."
                    );
                    self.states.insert(
                        key,
                        LazyImageEntry {
                            approx_bytes: 0,
                            last_touched_frame: self.frame_index,
                            state: LazyImageBytesState::Failed,
                        },
                    );
                }
            }
            did_update = true;
        }
        if did_update {
            self.trim_to_budget();
        }
        did_update
    }

    pub fn has_in_flight(&self) -> bool {
        self.states
            .values()
            .any(|entry| matches!(entry.state, LazyImageBytesState::Loading))
    }

    pub fn status(&self, key: &str) -> LazyImageBytesStatus {
        match self.states.get(key) {
            Some(entry) => match entry.state {
                LazyImageBytesState::Loading => LazyImageBytesStatus::Loading,
                LazyImageBytesState::Ready(_) => LazyImageBytesStatus::Ready,
                LazyImageBytesState::Failed => LazyImageBytesStatus::Failed,
            },
            None => LazyImageBytesStatus::Unrequested,
        }
    }

    pub fn bytes(&self, key: &str) -> Option<Arc<[u8]>> {
        match self.states.get(key) {
            Some(LazyImageEntry {
                state: LazyImageBytesState::Ready(bytes),
                ..
            }) => Some(Arc::clone(bytes)),
            _ => None,
        }
    }

    pub fn request(&mut self, key: impl Into<String>, path: PathBuf) -> LazyImageBytesStatus {
        let key = key.into();
        if let Some(entry) = self.states.get_mut(key.as_str()) {
            entry.last_touched_frame = self.frame_index;
            return match entry.state {
                LazyImageBytesState::Loading => LazyImageBytesStatus::Loading,
                LazyImageBytesState::Ready(_) => LazyImageBytesStatus::Ready,
                LazyImageBytesState::Failed => LazyImageBytesStatus::Failed,
            };
        }

        self.ensure_channel();
        let Some(tx) = self.results_tx.as_ref().cloned() else {
            self.states.insert(
                key,
                LazyImageEntry {
                    state: LazyImageBytesState::Failed,
                    last_touched_frame: self.frame_index,
                    approx_bytes: 0,
                },
            );
            return LazyImageBytesStatus::Failed;
        };

        self.states.insert(
            key.clone(),
            LazyImageEntry {
                state: LazyImageBytesState::Loading,
                last_touched_frame: self.frame_index,
                approx_bytes: 0,
            },
        );
        let key_for_task = key.clone();
        let path_label = path.display().to_string();
        let _ = tokio_runtime::spawn_detached(async move {
            let result = tokio::fs::read(path.as_path())
                .await
                .map(Arc::<[u8]>::from)
                .map_err(|err| format!("failed to read '{path_label}': {err}"));
            if let Err(err) = tx.send((key_for_task.clone(), result)) {
                tracing::error!(
                    target: "vertexlauncher/lazy_image",
                    key = %key_for_task,
                    path = %path.display(),
                    error = %err,
                    "Failed to deliver lazy-image bytes result."
                );
            }
        });

        LazyImageBytesStatus::Loading
    }

    pub fn retain_loaded(&mut self, keep: &HashSet<String>) {
        self.states.retain(|key, entry| {
            keep.contains(key.as_str()) || matches!(entry.state, LazyImageBytesState::Loading)
        });
    }

    fn ensure_channel(&mut self) {
        if self.results_tx.is_some() && self.results_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel::<(String, Result<Arc<[u8]>, String>)>();
        self.results_tx = Some(tx);
        self.results_rx = Some(Arc::new(Mutex::new(rx)));
    }

    fn trim_stale(&mut self) {
        let stale_before = self.frame_index.saturating_sub(LAZY_IMAGE_STALE_FRAMES);
        self.states.retain(|_, entry| {
            matches!(entry.state, LazyImageBytesState::Loading)
                || entry.last_touched_frame >= stale_before
        });
    }

    fn trim_to_budget(&mut self) {
        let mut total_bytes: usize = self.states.values().map(|entry| entry.approx_bytes).sum();
        if total_bytes <= LAZY_IMAGE_MAX_BYTES {
            return;
        }

        let mut eviction_order = self
            .states
            .iter()
            .filter_map(|(key, entry)| match entry.state {
                LazyImageBytesState::Ready(_) | LazyImageBytesState::Failed => {
                    Some((key.clone(), entry.last_touched_frame, entry.approx_bytes))
                }
                LazyImageBytesState::Loading => None,
            })
            .collect::<Vec<_>>();
        eviction_order.sort_by_key(|(_, last_touched_frame, _)| *last_touched_frame);

        for (key, _, approx_bytes) in eviction_order {
            if total_bytes <= LAZY_IMAGE_MAX_BYTES {
                break;
            }
            if self.states.remove(key.as_str()).is_some() {
                total_bytes = total_bytes.saturating_sub(approx_bytes);
            }
        }
    }
}
