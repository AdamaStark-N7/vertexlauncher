//! Async file helpers: each runs the `std::fs` call on the blocking pool.

use std::io;
use std::path::{Path, PathBuf};

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    match crate::spawn_blocking(work).await {
        Ok(result) => result,
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}

pub async fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
    let path = path.as_ref().to_path_buf();
    blocking(move || path.try_exists()).await
}

pub async fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::metadata(path)).await
}

pub async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::create_dir_all(path)).await
}

pub async fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::remove_file(path)).await
}

pub async fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::remove_dir_all(path)).await
}

pub async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::read(path)).await
}

pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (from.as_ref().to_path_buf(), to.as_ref().to_path_buf());
    blocking(move || std::fs::rename(from, to)).await
}

pub async fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::canonicalize(path)).await
}

/// The entries of a folder, all read up front.
pub async fn read_dir(path: impl AsRef<Path>) -> io::Result<Vec<std::fs::DirEntry>> {
    let path = path.as_ref().to_path_buf();
    blocking(move || std::fs::read_dir(path)?.collect()).await
}
