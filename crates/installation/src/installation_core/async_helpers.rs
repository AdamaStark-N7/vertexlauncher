use super::*;

pub(super) async fn find_java_executable_under_async(
    root: PathBuf,
) -> Result<Option<PathBuf>, InstallationError> {
    if !launcher_runtime::fs::try_exists(&root).await? {
        return Ok(None);
    }
    let binary = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in launcher_runtime::fs::read_dir(&dir).await? {
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.eq_ignore_ascii_case(binary)
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|n| n.to_str())
                    .is_some_and(|part| part.eq_ignore_ascii_case("bin"))
            {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

pub(super) async fn canonicalize_existing_path_async(path: PathBuf) -> PathBuf {
    launcher_runtime::fs::canonicalize(path.as_path())
        .await
        .unwrap_or(path)
}

pub(super) fn map_join_error(err: launcher_runtime::TaskError) -> InstallationError {
    InstallationError::Io(std::io::Error::other(format!(
        "background task failed: {err}"
    )))
}

pub(super) async fn get_json_with_user_agent_async<T: DeserializeOwned + Send + 'static>(
    url: &str,
    user_agent: &str,
) -> Result<T, InstallationError> {
    let (url, user_agent) = (url.to_owned(), user_agent.to_owned());
    launcher_runtime::spawn_blocking(move || get_json_with_user_agent(url.as_str(), user_agent.as_str()))
        .await
        .map_err(map_join_error)?
}

pub(crate) async fn download_file_simple_async(
    url: &str,
    destination: &Path,
) -> Result<(), InstallationError> {
    let (url, destination) = (url.to_owned(), destination.to_path_buf());
    launcher_runtime::spawn_blocking(move || download_file_simple(url.as_str(), destination.as_path()))
        .await
        .map_err(map_join_error)?
}
