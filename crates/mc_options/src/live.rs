//! A live view of one `options.txt`: it follows changes made by the game or by sync, applies
//! the user's edits without clobbering anything else, and always shows the merged truth.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::file::OptionsFile;

/// Cheap fingerprint of the file on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    modified: SystemTime,
    len: u64,
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        modified: meta.modified().ok()?,
        len: meta.len(),
    })
}

/// What [`LiveOptions::poll`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    Unchanged,
    /// The file changed on disk and the view was refreshed.
    ReloadedFromDisk,
    /// Pending edits were written.
    Saved,
    /// Writing failed; the edits stay pending and will be retried.
    SaveFailed,
}

pub struct LiveOptions {
    path: PathBuf,
    /// What the UI shows: disk contents with pending edits on top.
    file: OptionsFile,
    stamp: Option<Stamp>,
    /// Pending edits: `Some(raw)` sets, `None` removes.
    pending: BTreeMap<String, Option<String>>,
    pending_since: Option<Instant>,
    last_error: Option<String>,
}

impl LiveOptions {
    pub fn open(path: PathBuf) -> Self {
        let mut live = Self {
            path,
            file: OptionsFile::default(),
            stamp: None,
            pending: BTreeMap::new(),
            pending_since: None,
            last_error: None,
        };
        live.reload();
        live
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The merged view: disk plus pending edits.
    pub fn file(&self) -> &OptionsFile {
        &self.file
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.file.get(key)
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn reload(&mut self) {
        self.stamp = stamp_of(&self.path);
        let mut file = OptionsFile::read(&self.path).unwrap_or_default();
        for (key, edit) in &self.pending {
            match edit {
                Some(raw) => {
                    file.set(key, raw);
                }
                None => {
                    file.remove(key);
                }
            }
        }
        self.file = file;
    }

    /// Records a user edit; it shows immediately and is written after the debounce.
    pub fn edit(&mut self, key: &str, raw: Option<&str>) {
        match raw {
            Some(raw) => {
                self.file.set(key, raw);
            }
            None => {
                self.file.remove(key);
            }
        }
        self.pending.insert(key.to_owned(), raw.map(str::to_owned));
        self.pending_since = Some(Instant::now());
    }

    /// Follows the file and writes pending edits once they've been idle for `delay`.
    /// Call every frame; it only stats the file unless something changed.
    pub fn poll(&mut self, delay: Duration) -> Poll {
        let mut result = Poll::Unchanged;
        if stamp_of(&self.path) != self.stamp {
            self.reload();
            result = Poll::ReloadedFromDisk;
        }
        if self
            .pending_since
            .is_some_and(|since| since.elapsed() >= delay)
        {
            result = match self.flush() {
                Ok(()) => Poll::Saved,
                Err(_) => Poll::SaveFailed,
            };
        }
        result
    }

    /// Time until the debounce fires, for scheduling a repaint.
    pub fn time_until_save(&self, delay: Duration) -> Option<Duration> {
        self.pending_since
            .map(|since| delay.saturating_sub(since.elapsed()))
    }

    /// Writes pending edits now, on top of whatever is on disk *right now*, so changes the
    /// game made to other keys are kept.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            self.pending_since = None;
            return Ok(());
        }
        let mut disk = OptionsFile::read(&self.path).unwrap_or_default();
        let mut changed = false;
        for (key, edit) in &self.pending {
            changed |= match edit {
                Some(raw) => disk.set(key, raw),
                None => disk.remove(key),
            };
        }
        if changed {
            if let Err(err) = disk.write_atomic(&self.path) {
                self.last_error = Some(err.to_string());
                // Retry after another debounce interval rather than every frame.
                self.pending_since = Some(Instant::now());
                return Err(err);
            }
        }
        self.last_error = None;
        self.pending.clear();
        self.pending_since = None;
        self.file = disk;
        self.stamp = stamp_of(&self.path);
        Ok(())
    }
}

impl Drop for LiveOptions {
    fn drop(&mut self) {
        // Closing the editor must never lose an edit made in the last few milliseconds.
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_file(name: &str, contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vertex-live-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("options.txt");
        fs::write(&path, contents).unwrap();
        path
    }

    fn bump(path: &Path) {
        // Ensure a distinct modification time even on coarse filesystems.
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn edits_show_immediately_and_save_without_clobbering_the_game() {
        let path = temp_file("edit", "fov:0.0\nchatScale:1.0\n");
        let mut live = LiveOptions::open(path.clone());
        live.edit("fov", Some("0.5"));
        assert_eq!(live.get("fov"), Some("0.5"));

        // The game rewrites a different key before our debounce fires.
        fs::write(&path, "fov:0.0\nchatScale:0.25\n").unwrap();
        bump(&path);
        assert_eq!(live.poll(Duration::from_secs(60)), Poll::ReloadedFromDisk);
        // The user's pending edit stays on top of the game's change.
        assert_eq!(live.get("fov"), Some("0.5"));
        assert_eq!(live.get("chatScale"), Some("0.25"));

        assert_eq!(live.poll(Duration::ZERO), Poll::Saved);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "fov:0.5\nchatScale:0.25\n"
        );
        assert!(!live.has_pending());
        // Our own write is not mistaken for an outside change.
        assert_eq!(live.poll(Duration::from_secs(60)), Poll::Unchanged);
    }

    #[test]
    fn external_changes_flow_into_the_view() {
        let path = temp_file("external", "fov:0.0\n");
        let mut live = LiveOptions::open(path.clone());
        assert_eq!(live.get("fov"), Some("0.0"));
        fs::write(&path, "fov:0.75\nnew:1\n").unwrap();
        bump(&path);
        assert_eq!(live.poll(Duration::from_secs(60)), Poll::ReloadedFromDisk);
        assert_eq!(live.get("fov"), Some("0.75"));
        assert_eq!(live.get("new"), Some("1"));
        // Removal from outside shows up too.
        fs::write(&path, "new:1\n").unwrap();
        bump(&path);
        live.poll(Duration::from_secs(60));
        assert_eq!(live.get("fov"), None);
    }

    #[test]
    fn edits_can_remove_keys_and_survive_a_missing_file() {
        let path = temp_file("remove", "a:1\nb:2\n");
        let mut live = LiveOptions::open(path.clone());
        live.edit("a", None);
        assert_eq!(live.get("a"), None);
        live.flush().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "b:2\n");

        fs::remove_file(&path).unwrap();
        live.edit("c", Some("3"));
        live.flush().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "c:3\n");
    }

    #[test]
    fn dropping_flushes_pending_edits() {
        let path = temp_file("drop", "a:1\n");
        {
            let mut live = LiveOptions::open(path.clone());
            live.edit("a", Some("9"));
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), "a:9\n");
    }
}
