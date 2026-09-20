use super::*;
pub(crate) use logged_fs::canonicalize as fs_canonicalize;
pub(crate) use logged_fs::create_dir_all as fs_create_dir_all;
pub(crate) use logged_fs::file_create as fs_file_create;
pub(crate) use logged_fs::file_open as fs_file_open;
pub(crate) use logged_fs::read_dir as fs_read_dir;
pub(crate) use logged_fs::read_to_string as fs_read_to_string;
pub(crate) use logged_fs::remove_dir_all as fs_remove_dir_all;
pub(crate) use logged_fs::rename as fs_rename;
pub(crate) use logged_fs::write as fs_write;

pub fn display_user_path(path: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        return normalize_windows_cli_path(path.as_os_str().to_string_lossy().as_ref());
    }

    #[cfg(not(target_os = "windows"))]
    {
        path.as_os_str().to_string_lossy().into_owned()
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn normalize_windows_cli_path(raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{stripped}");
    }
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        return stripped.to_owned();
    }
    raw.to_owned()
}

pub(crate) fn normalize_child_process_path(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return PathBuf::from(normalize_windows_cli_path(
            path.as_os_str().to_string_lossy().as_ref(),
        ));
    }

    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

pub fn normalize_path_key(path: &Path) -> String {
    let normalized = fs_canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    display_user_path(normalized.as_path())
}
