//! Native file/folder pickers that never block the UI thread.
//!
//! `rfd`'s blocking dialogs freeze rendering (and on some platforms the whole window) while
//! they are open. These run the dialog as an async task and hand the result back through the
//! egui context: call [`open`] when the user clicks, then poll [`take`] every frame from the
//! same place.
//!
//! ```ignore
//! if button.clicked() {
//!     file_dialog::open(ctx, "import_pick", Pick::File, Dialog::new().title("Choose pack"));
//! }
//! if let Some(paths) = file_dialog::take(ctx, "import_pick") {
//!     if let Some(path) = paths.first() { /* picked */ } // empty = cancelled
//! }
//! ```

use std::hash::Hash;
use std::path::{Path, PathBuf};

use egui::{Context, Id};
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

    fn build(&self) -> rfd::AsyncFileDialog {
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

#[derive(Clone, Default)]
struct Slot {
    results: WorkerChannel<Vec<PathBuf>>,
    open: bool,
}

fn slot_id(slot: impl Hash) -> Id {
    Id::new(("vertex_file_dialog", slot))
}

/// Opens a dialog for `slot`. Does nothing (returns false) if that slot's dialog is already
/// open, so a double click can't stack two windows.
pub fn open(ctx: &Context, slot: impl Hash, pick: Pick, dialog: Dialog) -> bool {
    let id = slot_id(slot);
    let mut state = ctx
        .data(|data| data.get_temp::<Slot>(id))
        .unwrap_or_default();
    if state.open {
        return false;
    }
    let tx = state.results.sender();
    state.open = true;
    ctx.data_mut(|data| data.insert_temp(id, state));

    let ctx = ctx.clone();
    let _ = tokio_runtime::spawn_detached(async move {
        let builder = dialog.build();
        let paths: Vec<PathBuf> = match pick {
            Pick::File => builder
                .pick_file()
                .await
                .map(|file| vec![file.path().to_path_buf()])
                .unwrap_or_default(),
            Pick::Files => builder
                .pick_files()
                .await
                .map(|files| files.iter().map(|f| f.path().to_path_buf()).collect())
                .unwrap_or_default(),
            Pick::Folder => builder
                .pick_folder()
                .await
                .map(|folder| vec![folder.path().to_path_buf()])
                .unwrap_or_default(),
            Pick::SaveFile => builder
                .save_file()
                .await
                .map(|file| vec![file.path().to_path_buf()])
                .unwrap_or_default(),
        };
        let _ = tx.send(paths);
        ctx.request_repaint();
    });
    true
}

/// Whether `slot`'s dialog is currently showing.
pub fn is_open(ctx: &Context, slot: impl Hash) -> bool {
    ctx.data(|data| data.get_temp::<Slot>(slot_id(slot)))
        .is_some_and(|state| state.open)
}

/// The finished dialog's result: `Some(paths)` once, then `None` until it is opened again.
/// `Some(empty)` means the user cancelled.
pub fn take(ctx: &Context, slot: impl Hash) -> Option<Vec<PathBuf>> {
    let id = slot_id(slot);
    let mut state = ctx.data(|data| data.get_temp::<Slot>(id))?;
    if !state.open {
        return None;
    }
    let drained = state.results.drain();
    if drained.disconnected {
        // The task died without answering; treat it as cancelled.
        state.open = false;
        ctx.data_mut(|data| data.insert_temp(id, state));
        return Some(Vec::new());
    }
    let result = drained.items.into_iter().next();
    if result.is_some() {
        state.open = false;
    }
    ctx.data_mut(|data| data.insert_temp(id, state));
    result
}
