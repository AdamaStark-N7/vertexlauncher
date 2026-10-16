//! Native file/folder pickers that never block the UI thread, for any front end.
//!
//! `rfd`'s blocking dialogs freeze rendering while they are open, so the dialog runs as an async task and the result
//! comes back through a slot: call [`FileDialogs::open`] when the user clicks, then poll [`FileDialogs::take`] every
//! frame from the same place.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use launcher_runtime::WorkerChannel;

use crate::app::tokio_runtime;

/// What the dialog picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    File,
    Files,
    Folder,
    /// A path to write; the file need not exist.
    SaveFile,
}

/// Dialog appearance and starting point.
#[derive(Clone, Debug, Default)]
pub struct Dialog {
    title: Option<String>,
    filters: Vec<(String, Vec<String>)>,
    directory: Option<PathBuf>,
    file_name: Option<String>,
}

impl Dialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters.push((
            name.into(),
            extensions.iter().map(|ext| (*ext).to_owned()).collect(),
        ));
        self
    }

    /// Starts in `directory`, or, if a file path is given, in its parent folder.
    /// Paths that don't exist are ignored.
    pub fn start_in(mut self, path: &Path) -> Self {
        // Cheap metadata checks are fine here; they run once per click, not per frame.
        if path.is_dir() {
            self.directory = Some(path.to_path_buf());
        } else if path.is_file() {
            self.directory = path.parent().map(Path::to_path_buf);
        }
        self
    }

    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    pub(crate) fn build(&self) -> rfd::AsyncFileDialog {
        let mut dialog = rfd::AsyncFileDialog::new();
        if let Some(title) = &self.title {
            dialog = dialog.set_title(title);
        }
        for (name, extensions) in &self.filters {
            let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(name, &extensions);
        }
        if let Some(directory) = &self.directory {
            dialog = dialog.set_directory(directory);
        }
        if let Some(name) = &self.file_name {
            dialog = dialog.set_file_name(name);
        }
        dialog
    }
}


/// Runs `dialog` on a worker and sends the chosen paths (empty = cancelled) to `tx`, then wakes the UI.
pub(crate) fn spawn(pick: Pick, dialog: Dialog, tx: std::sync::mpsc::Sender<Vec<PathBuf>>) {
    let _ = tokio_runtime::spawn_detached(async move {
        let builder = dialog.build();
        let paths: Vec<PathBuf> = match pick {
            Pick::File => builder.pick_file().await.map(|file| vec![file.path().to_path_buf()]).unwrap_or_default(),
            Pick::Files => builder
                .pick_files()
                .await
                .map(|files| files.iter().map(|f| f.path().to_path_buf()).collect())
                .unwrap_or_default(),
            Pick::Folder => builder.pick_folder().await.map(|folder| vec![folder.path().to_path_buf()]).unwrap_or_default(),
            Pick::SaveFile => builder.save_file().await.map(|file| vec![file.path().to_path_buf()]).unwrap_or_default(),
        };
        let _ = tx.send(paths);
        crate::ui_waker::wake();
    });
}

#[derive(Default)]
struct SlotState {
    results: WorkerChannel<Vec<PathBuf>>,
    open: bool,
}

/// Named dialog slots. Keep one per UI.
#[derive(Default)]
pub struct FileDialogs {
    slots: HashMap<String, SlotState>,
}

impl FileDialogs {
    /// Opens a dialog for `slot`; does nothing (returns false) while that slot's dialog is already open.
    pub fn open(&mut self, slot: &str, pick: Pick, dialog: Dialog) -> bool {
        let state = self.slots.entry(slot.to_owned()).or_default();
        if state.open {
            return false;
        }
        state.open = true;
        spawn(pick, dialog, state.results.sender());
        true
    }

    pub fn is_open(&self, slot: &str) -> bool {
        self.slots.get(slot).is_some_and(|state| state.open)
    }

    /// The finished dialog's result: `Some(paths)` once, then `None` until opened again. `Some(empty)` is a cancel.
    pub fn take(&mut self, slot: &str) -> Option<Vec<PathBuf>> {
        let state = self.slots.get_mut(slot)?;
        if !state.open {
            return None;
        }
        let drained = state.results.drain();
        if drained.disconnected {
            state.open = false;
            return Some(Vec::new());
        }
        let result = drained.items.into_iter().next();
        if result.is_some() {
            state.open = false;
        }
        result
    }
}
