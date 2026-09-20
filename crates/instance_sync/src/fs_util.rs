use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

pub(crate) fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

/// Writes via a temp file and rename so a crash never leaves a half-written game file.
pub(crate) fn write_atomic(
    path: &Path,
    bytes: &[u8],
    modified: Option<SystemTime>,
) -> io::Result<()> {
    write_atomic_bytes(path, bytes, modified)
}

pub(crate) fn write_atomic_bytes(
    path: &Path,
    bytes: &[u8],
    modified: Option<SystemTime>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temp_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    temp_name.push(".vertex-sync-tmp");
    let temp = path.with_file_name(temp_name);
    fs::write(&temp, bytes)?;
    if let Some(modified) = modified {
        // Best effort: keeps the copy from looking newer than its source.
        if let Ok(file) = fs::File::options().write(true).open(&temp) {
            let _ = file.set_modified(modified);
        }
    }
    fs::rename(&temp, path)
}

/// Recursively copies a directory tree (following symlinked entries as their targets).
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if fs::metadata(&from)?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
            // Keep modification times so a copy never looks newer than its source; the
            // mirror logic compares them to decide which side to keep.
            if let Ok(source_modified) = fs::metadata(&from).and_then(|m| m.modified())
                && let Ok(file) = fs::File::options().write(true).open(&to)
            {
                let _ = file.set_modified(source_modified);
            }
        }
    }
    Ok(())
}

/// Moves a directory, falling back to copy-then-delete across filesystems. The source is
/// only removed after the copy finished, so a failure never loses the original.
pub(crate) fn move_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    if let Err(err) = copy_dir_recursive(src, dst) {
        let _ = fs::remove_dir_all(dst);
        return Err(err);
    }
    fs::remove_dir_all(src)
}

/// True for symlinks and Windows junctions.
pub(crate) fn is_link(path: &Path) -> bool {
    fs::read_link(path).is_ok()
}

/// Creates a directory link at `link` pointing to `target`.
pub(crate) fn create_dir_link(target: &Path, link: &Path) -> io::Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        // Symlinks need developer mode or elevation; junctions don't.
        std::os::windows::fs::symlink_dir(target, link).or_else(|_| {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::other("mklink /J failed"))
            }
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(io::Error::other("directory links are not supported here"))
    }
}

/// Removes a link without touching what it points to.
pub(crate) fn remove_link(link: &Path) -> io::Result<()> {
    fs::remove_file(link).or_else(|_| fs::remove_dir(link))
}

/// Last time a world was saved: `level.dat` modification, else the folder's.
pub(crate) fn world_freshness(world_dir: &Path) -> Option<SystemTime> {
    modified(&world_dir.join("level.dat")).or_else(|| modified(world_dir))
}
