use super::*;

pub(super) async fn find_java_executable_under_async(
    root: PathBuf,
) -> Result<Option<PathBuf>, InstallationError> {
    if !tokio::fs::try_exists(&root).await? {
        return Ok(None);
    }
    let binary = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
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
    tokio::fs::canonicalize(path.as_path())
        .await
        .unwrap_or(path)
}

pub(super) fn map_tokio_join_error(err: tokio::task::JoinError) -> InstallationError {
    InstallationError::Io(std::io::Error::other(format!(
        "background task failed: {err}"
    )))
}

pub(super) async fn get_json_with_user_agent_async<T: DeserializeOwned>(
    url: &str,
    user_agent: &str,
) -> Result<T, InstallationError> {
    let raw = call_get_text_with_retry_async(url, user_agent).await?;
    Ok(serde_json::from_str(&raw)?)
}

pub(super) async fn call_get_text_with_retry_async(
    url: &str,
    user_agent: &str,
) -> Result<String, InstallationError> {
    let response = call_get_response_with_retry_async(url, user_agent).await?;
    response
        .text()
        .await
        .map_err(|err| InstallationError::Transport {
            url: url.to_owned(),
            message: err.to_string(),
        })
}

pub(super) async fn call_get_response_with_retry_async(
    url: &str,
    user_agent: &str,
) -> Result<reqwest::Response, InstallationError> {
    let client = reqwest::Client::builder()
        .connect_timeout(HTTP_TIMEOUT_CONNECT)
        .timeout(HTTP_TIMEOUT_GLOBAL)
        .build()
        .map_err(|err| InstallationError::Transport {
            url: url.to_owned(),
            message: err.to_string(),
        })?;
    let mut last_err = None;
    for attempt in 1..=HTTP_RETRY_ATTEMPTS {
        let response = client
            .get(url)
            .header("User-Agent", user_agent)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(InstallationError::HttpStatus {
                    url: url.to_owned(),
                    status,
                    body,
                });
            }
            Err(err) => {
                last_err = Some(err.to_string());
                if attempt < HTTP_RETRY_ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(
                        HTTP_RETRY_BASE_DELAY_MS.saturating_mul(attempt as u64),
                    ))
                    .await;
                }
            }
        }
    }
    Err(InstallationError::Transport {
        url: url.to_owned(),
        message: last_err.unwrap_or_else(|| "request failed".to_owned()),
    })
}

pub(crate) async fn download_file_simple_async(
    url: &str,
    destination: &Path,
) -> Result<(), InstallationError> {
    if tokio::fs::try_exists(destination).await? {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut response = call_get_response_with_retry_async(url, OPENJDK_USER_AGENT).await?;
    let temp = temporary_download_path(destination);
    let mut file = tokio::fs::File::create(&temp).await?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| InstallationError::Transport {
            url: url.to_owned(),
            message: err.to_string(),
        })?
    {
        tokio::io::AsyncWriteExt::write_all(&mut file, chunk.as_ref()).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    drop(file);
    tokio::fs::rename(temp.as_path(), destination).await?;
    Ok(())
}
