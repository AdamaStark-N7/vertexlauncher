//! Notifies the user when a new screenshot shows up in any instance while the launcher is running.
//!
//! Minecraft writes screenshots to `<instance root>/screenshots`, the same folders the Home and
//! Instance galleries scan. Those galleries only scan while visible, so this polls independently.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use config::Config;
use egui::Context;
use instances::{InstanceStore, instance_root_path};
use launcher_runtime as tokio_runtime;

use crate::notification::{self, NotificationAction, Severity};

const POLL_INTERVAL: Duration = Duration::from_secs(2);

struct DirListing {
    dir: PathBuf,
    files: Vec<PathBuf>,
}

struct WatchState {
    tx: mpsc::Sender<Vec<DirListing>>,
    rx: mpsc::Receiver<Vec<DirListing>>,
    in_flight: bool,
    last_poll: Option<Instant>,
    /// Directories already listed once. A directory's first listing is treated as a baseline so
    /// existing screenshots (or a freshly imported instance) don't raise notifications.
    seeded_dirs: HashSet<PathBuf>,
    known_files: HashSet<PathBuf>,
}

fn state() -> &'static Mutex<WatchState> {
    static STATE: OnceLock<Mutex<WatchState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        Mutex::new(WatchState {
            tx,
            rx,
            in_flight: false,
            last_poll: None,
            seeded_dirs: HashSet::new(),
            known_files: HashSet::new(),
        })
    })
}

/// Call once per frame. Cheap: the directory listing runs on a blocking worker every few seconds.
///
/// `player_name` should be the account that is playing right now, or `None` when no game is
/// running (e.g. the file was copied in by hand), in which case the notification stays generic.
pub fn poll(ctx: &Context, instances: &InstanceStore, config: &Config, player_name: Option<&str>) {
    let Ok(mut state) = state().lock() else {
        return;
    };

    while let Ok(listings) = state.rx.try_recv() {
        state.in_flight = false;
        for listing in listings {
            let baseline = state.seeded_dirs.insert(listing.dir.clone());
            for file in listing.files {
                if state.known_files.insert(file.clone()) && !baseline {
                    let message = match player_name {
                        Some(name) => format!("{name} took a screenshot!"),
                        None => "We detected a new screenshot!".to_owned(),
                    };
                    notification::emit_with_actions(
                        Severity::Info,
                        "screenshots",
                        message,
                        screenshot_actions(file),
                    );
                }
            }
        }
    }

    let due = state
        .last_poll
        .is_none_or(|last| last.elapsed() >= POLL_INTERVAL);
    if due && !state.in_flight {
        state.in_flight = true;
        state.last_poll = Some(Instant::now());
        let installations_root = config.minecraft_installations_root_path().to_path_buf();
        let targets: Vec<PathBuf> = instances
            .instances
            .iter()
            .map(|instance| {
                instance_root_path(installations_root.as_path(), instance).join("screenshots")
            })
            .collect();
        let tx = state.tx.clone();
        tokio_runtime::spawn_detached(async move {
            let listings = tokio_runtime::spawn_blocking(move || {
                targets
                    .into_iter()
                    .map(|dir| DirListing {
                        files: list_screenshots(dir.as_path()),
                        dir,
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default();
            let _ = tx.send(listings);
        });
    }
    ctx.request_repaint_after(POLL_INTERVAL);
}

fn list_screenshots(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().and_then(OsStr::to_str).is_some_and(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
                    )
                })
        })
        .collect()
}

fn screenshot_actions(path: PathBuf) -> Vec<NotificationAction> {
    let view_path = path.clone();
    let copy_path = path.clone();
    let remove_path = path;
    vec![
        NotificationAction::new("View", move |ctx| {
            crate::screens::open_screenshot_in_viewer(ctx, view_path.clone());
        }),
        NotificationAction::new("Copy", move |ctx| {
            match copy_to_clipboard(ctx, &copy_path) {
                Ok(()) => notification::info!("screenshots", "Copied screenshot to clipboard."),
                Err(err) => {
                    notification::error!("screenshots", "Failed to copy screenshot: {}", err)
                }
            }
        }),
        NotificationAction::new("Remove", move |_ctx| {
            let path = remove_path.clone();
            tokio_runtime::spawn_detached(async move {
                match tokio::fs::remove_file(path.as_path()).await {
                    Ok(()) => notification::info!("screenshots", "Deleted screenshot from disk."),
                    Err(err) => {
                        tracing::warn!(
                            target: "vertexlauncher/io",
                            op = "remove_file",
                            path = %path.display(),
                            error = %err,
                            context = "delete screenshot from notification"
                        );
                        notification::error!("screenshots", "Failed to delete screenshot: {}", err);
                    }
                }
            });
        })
        .danger(),
    ]
}

fn copy_to_clipboard(ctx: &Context, path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let rgba = image::load_from_memory(&bytes)
        .map_err(|err| err.to_string())?
        .to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ));
    Ok(())
}
