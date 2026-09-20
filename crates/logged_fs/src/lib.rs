//! `std::fs` operations that log what they touch: a debug line per call and a warning with the
//! error when one fails, under the `vertexlauncher/io` tracing target. Use these instead of
//! bare `std::fs` calls anywhere a failure should be diagnosable from the launcher log.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Defines a wrapper around a one-path `std::fs` function.
macro_rules! logged_path_op {
    ($(#[$doc:meta])* $name:ident, $op:literal, |$path:ident $(, $arg:ident : $arg_ty:ty)*| -> $ret:ty, $call:expr) => {
        $(#[$doc])*
        #[track_caller]
        pub fn $name($path: impl AsRef<Path> $(, $arg: $arg_ty)*) -> io::Result<$ret> {
            let $path = $path.as_ref();
            tracing::debug!(target: "vertexlauncher/io", op = $op, path = %$path.display());
            let result = $call;
            if let Err(err) = &result {
                tracing::warn!(target: "vertexlauncher/io", op = $op, path = %$path.display(), error = %err);
            }
            result
        }
    };
}

logged_path_op!(create_dir_all, "create_dir_all", |path| -> (), fs::create_dir_all(path));
logged_path_op!(remove_dir_all, "remove_dir_all", |path| -> (), fs::remove_dir_all(path));
logged_path_op!(remove_file, "remove_file", |path| -> (), fs::remove_file(path));
logged_path_op!(read_to_string, "read_to_string", |path| -> String, fs::read_to_string(path));
logged_path_op!(read, "read", |path| -> Vec<u8>, fs::read(path));
logged_path_op!(read_dir, "read_dir", |path| -> fs::ReadDir, fs::read_dir(path));
logged_path_op!(canonicalize, "canonicalize", |path| -> PathBuf, fs::canonicalize(path));
logged_path_op!(file_create, "file_create", |path| -> fs::File, fs::File::create(path));
logged_path_op!(file_open, "file_open", |path| -> fs::File, fs::File::open(path));
logged_path_op!(
    write,
    "write",
    |path, contents: impl AsRef<[u8]>| -> (),
    fs::write(path, contents)
);

/// `fs::rename` with logging.
#[track_caller]
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    tracing::debug!(target: "vertexlauncher/io", op = "rename", from = %from.display(), to = %to.display());
    let result = fs::rename(from, to);
    if let Err(err) = &result {
        tracing::warn!(target: "vertexlauncher/io", op = "rename", from = %from.display(), to = %to.display(), error = %err);
    }
    result
}

/// `fs::copy` with logging.
#[track_caller]
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let (from, to) = (from.as_ref(), to.as_ref());
    tracing::debug!(target: "vertexlauncher/io", op = "copy", from = %from.display(), to = %to.display());
    let result = fs::copy(from, to);
    if let Err(err) = &result {
        tracing::warn!(target: "vertexlauncher/io", op = "copy", from = %from.display(), to = %to.display(), error = %err);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_behave_like_std_fs_and_return_the_same_errors() {
        let dir = std::env::temp_dir().join(format!("vertex-logged-fs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        create_dir_all(dir.join("a/b")).unwrap();
        write(dir.join("a/file.txt"), "hello").unwrap();
        assert_eq!(read_to_string(dir.join("a/file.txt")).unwrap(), "hello");
        assert_eq!(read(dir.join("a/file.txt")).unwrap(), b"hello");
        copy(dir.join("a/file.txt"), dir.join("copy.txt")).unwrap();
        rename(dir.join("copy.txt"), dir.join("moved.txt")).unwrap();
        assert!(canonicalize(dir.join("moved.txt")).unwrap().is_absolute());
        assert_eq!(read_dir(dir.join("a")).unwrap().count(), 2);
        remove_file(dir.join("moved.txt")).unwrap();
        assert_eq!(
            read_to_string(dir.join("moved.txt")).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        remove_dir_all(&dir).unwrap();
    }
}
