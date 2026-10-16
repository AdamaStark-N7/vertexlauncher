use super::*;

pub(super) fn validate_move_dest(path_str: &str) -> Result<PathBuf, String> {
    let path_str = path_str.trim();
    if path_str.is_empty() {
        return Err("Please enter or browse to a destination folder.".to_owned());
    }
    let path = PathBuf::from(path_str);
    if path.is_dir() {
        match std::fs::read_dir(&path) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err("The destination folder must be empty.".to_owned());
                }
            }
            Err(err) => {
                return Err(format!("Cannot read destination folder: {err}"));
            }
        }
        return Ok(path);
    }
    if path.exists() {
        return Err("The destination path already exists and is not a folder.".to_owned());
    }
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() || parent.is_dir() => {}
        Some(parent) => {
            return Err(format!(
                "Parent folder does not exist: {}",
                parent.display()
            ));
        }
        None => {
            return Err("The destination path is invalid.".to_owned());
        }
    }
    Ok(path)
}

pub(super) fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}






