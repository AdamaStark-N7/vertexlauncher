use super::*;

#[path = "launch_engine_downloads/download_progress.rs"]
mod download_progress;
#[path = "launch_engine_downloads/download_tasks.rs"]
mod download_tasks;

pub(crate) use download_progress::*;
pub(crate) use download_tasks::*;

pub(crate) fn download_version_dependencies(
    instance_root: &Path,
    version_json_path: &Path,
    policy: &DownloadPolicy,
    downloaded_files_offset: u32,
    progress: Option<&InstallProgressSink>,
) -> Result<u32, InstallationError> {
    if !version_json_path.exists() {
        return Ok(0);
    }
    tracing::info!(
        target: "vertexlauncher/installation/dependencies",
        instance_root = %instance_root.display(),
        version_json_path = %version_json_path.display(),
        "Starting version dependency resolution."
    );
    let raw = fs_read_to_string(version_json_path).map_err(|err| {
        tracing::error!(
            target: "vertexlauncher/installation/dependencies",
            instance_root = %instance_root.display(),
            version_json_path = %version_json_path.display(),
            error = %err,
            "Failed to read version metadata JSON."
        );
        InstallationError::Io(err)
    })?;
    let version_meta: serde_json::Value = serde_json::from_str(&raw)?;
    let mut downloaded = 0u32;

    let mut library_tasks = Vec::new();
    collect_library_download_tasks(instance_root, &version_meta, &mut library_tasks);
    tracing::info!(
        target: "vertexlauncher/installation/dependencies",
        library_task_count = library_tasks.len(),
        "Collected library download tasks."
    );
    downloaded += download_files_concurrent(
        InstallStage::DownloadingCore,
        library_tasks,
        policy,
        downloaded_files_offset.saturating_add(downloaded),
        progress,
    )
    .map_err(|err| {
        tracing::error!(
            target: "vertexlauncher/installation/dependencies",
            instance_root = %instance_root.display(),
            version_json_path = %version_json_path.display(),
            error = %err,
            "Library dependency download batch failed."
        );
        err
    })?;

    let mut asset_index_task = Vec::new();
    let asset_index_path =
        collect_asset_index_download_task(instance_root, &version_meta, &mut asset_index_task);
    tracing::info!(
        target: "vertexlauncher/installation/dependencies",
        asset_index_task_count = asset_index_task.len(),
        asset_index_path = %asset_index_path.as_ref().map(|path| path.display().to_string()).unwrap_or_default(),
        "Collected asset index download task."
    );
    downloaded += download_files_concurrent(
        InstallStage::DownloadingCore,
        asset_index_task,
        policy,
        downloaded_files_offset.saturating_add(downloaded),
        progress,
    )
    .map_err(|err| {
        tracing::error!(
            target: "vertexlauncher/installation/dependencies",
            instance_root = %instance_root.display(),
            version_json_path = %version_json_path.display(),
            asset_index_path = %asset_index_path.as_ref().map(|path| path.display().to_string()).unwrap_or_default(),
            error = %err,
            "Asset index download batch failed."
        );
        err
    })?;

    if let Some(asset_index_path) = asset_index_path {
        let mut object_tasks = Vec::new();
        collect_asset_object_download_tasks(
            instance_root,
            asset_index_path.as_path(),
            &mut object_tasks,
        )
        .map_err(|err| {
            tracing::error!(
                target: "vertexlauncher/installation/dependencies",
                instance_root = %instance_root.display(),
                asset_index_path = %asset_index_path.display(),
                error = %err,
                "Failed while collecting asset object download tasks from asset index."
            );
            err
        })?;
        tracing::info!(
            target: "vertexlauncher/installation/dependencies",
            asset_index_path = %asset_index_path.display(),
            asset_object_task_count = object_tasks.len(),
            "Collected asset object download tasks."
        );
        downloaded += download_files_concurrent(
            InstallStage::DownloadingCore,
            object_tasks,
            policy,
            downloaded_files_offset.saturating_add(downloaded),
            progress,
        )
        .map_err(|err| {
            tracing::error!(
                target: "vertexlauncher/installation/dependencies",
                instance_root = %instance_root.display(),
                asset_index_path = %asset_index_path.display(),
                error = %err,
                "Asset object download batch failed."
            );
            err
        })?;
    }

    tracing::info!(
        target: "vertexlauncher/installation/dependencies",
        instance_root = %instance_root.display(),
        version_json_path = %version_json_path.display(),
        downloaded,
        "Completed version dependency resolution."
    );
    Ok(downloaded)
}

#[derive(Clone, Debug)]
pub(crate) struct FileDownloadTask {
    pub(crate) url: String,
    pub(crate) destination: PathBuf,
    pub(crate) expected_size: Option<u64>,
}

pub(crate) fn prefetch_batch_total_bytes(
    tasks: &mut [FileDownloadTask],
    probe_workers: usize,
    max_probe_count: usize,
) -> Result<u64, InstallationError> {
    let mut total_known_bytes = 0u64;
    let mut unknown = std::collections::VecDeque::new();
    let mut skipped_probe_count = 0usize;
    for (index, task) in tasks.iter().enumerate() {
        if let Some(size) = task.expected_size {
            total_known_bytes = total_known_bytes.saturating_add(size);
        } else {
            if unknown.len() < max_probe_count {
                unknown.push_back((index, task.url.clone()));
            } else {
                skipped_probe_count += 1;
            }
        }
    }
    if unknown.is_empty() {
        return Ok(total_known_bytes);
    }
    if skipped_probe_count > 0 {
        tracing::debug!(
            target: "vertexlauncher/installation/downloads",
            probed = unknown.len(),
            skipped = skipped_probe_count,
            "Skipping some HEAD size probes to reduce batch startup latency"
        );
    }

    let queue = Arc::new(Mutex::new(unknown));
    let discovered = Arc::new(Mutex::new(Vec::<(usize, u64)>::new()));
    thread::scope(|scope| -> Result<(), InstallationError> {
        let mut workers = Vec::new();
        for _ in 0..probe_workers.max(1) {
            let queue = Arc::clone(&queue);
            let discovered = Arc::clone(&discovered);
            workers.push(scope.spawn(move || {
                loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some((index, url)) = next else {
                        break;
                    };
                    if let Some(size) = probe_content_length(url.as_str())
                        && let Ok(mut guard) = discovered.lock()
                    {
                        guard.push((index, size));
                    }
                }
            }));
        }
        for worker in workers {
            worker.join().map_err(|_| {
                InstallationError::Io(std::io::Error::other(
                    "content-length probe worker panicked",
                ))
            })?;
        }
        Ok(())
    })?;

    if let Ok(discovered) = discovered.lock() {
        for (index, size) in discovered.iter().copied() {
            if let Some(task) = tasks.get_mut(index)
                && task.expected_size.is_none()
            {
                task.expected_size = Some(size);
                total_known_bytes = total_known_bytes.saturating_add(size);
            }
        }
    }

    Ok(total_known_bytes)
}

pub(crate) fn probe_content_length(url: &str) -> Option<u64> {
    let response = match http_agent()
        .head(url)
        .header("User-Agent", DEFAULT_USER_AGENT)
        .config()
        .http_status_as_error(false)
        .build()
        .call()
    {
        Ok(response) => response,
        Err(err) => {
            tracing::debug!(
                target: "vertexlauncher/installation/downloads",
                "Size prefetch HEAD transport error for {}: {}",
                url,
                err
            );
            return None;
        }
    };
    if response.status().as_u16() >= 400 {
        tracing::debug!(
            target: "vertexlauncher/installation/downloads",
            "Size prefetch HEAD failed for {} with status {}",
            url,
            response.status().as_u16()
        );
        return None;
    }
    response
        .headers()
        .get("Content-Length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|size| *size > 0)
}

pub(crate) fn download_files_concurrent(
    stage: InstallStage,
    tasks: Vec<FileDownloadTask>,
    policy: &DownloadPolicy,
    downloaded_files_offset: u32,
    progress: Option<&InstallProgressSink>,
) -> Result<u32, InstallationError> {
    if tasks.is_empty() {
        return Ok(0);
    }

    let total_files = tasks.len() as u32;
    let mut tasks = tasks;
    let batch_started_at = Instant::now();
    let worker_count = policy.max_concurrent_downloads.clamp(1, 64) as usize;
    let size_probe_workers = worker_count.min(8).max(1);
    let max_size_probes = MAX_CONTENT_LENGTH_PROBES_PER_BATCH.max(size_probe_workers);
    let prefetched_total_bytes =
        prefetch_batch_total_bytes(&mut tasks, size_probe_workers, max_size_probes)?;
    // Prioritize larger files so long-running transfers start earlier.
    tasks.sort_by_key(|task| std::cmp::Reverse(task.expected_size.unwrap_or(0)));
    tracing::info!(
        target: "vertexlauncher/installation/downloads",
        "Starting {:?} batch: {} file(s), prefetched_total_bytes={}, max_concurrent_downloads={}, speed_limit_bps={:?}.",
        stage,
        total_files,
        prefetched_total_bytes,
        policy.max_concurrent_downloads,
        policy.max_download_bps
    );
    let queue = Arc::new(Mutex::new(std::collections::VecDeque::from(tasks)));
    let bandwidth_limiter = policy
        .max_download_bps
        .map(BandwidthLimiter::new)
        .map(Arc::new);
    let telemetry = Arc::new(DownloadTelemetry::new(total_files, prefetched_total_bytes));

    emit_download_progress(progress, &telemetry, stage, downloaded_files_offset);

    let downloaded_files = thread::scope(|scope| -> Result<u32, InstallationError> {
        let mut workers = Vec::new();
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let bandwidth_limiter = bandwidth_limiter.as_ref().map(Arc::clone);
            let telemetry = Arc::clone(&telemetry);
            workers.push(scope.spawn(move || -> Result<u32, InstallationError> {
                let mut completed = 0u32;
                loop {
                    let next_task = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some(task) = next_task else {
                        break;
                    };
                    download_to_file(
                        task,
                        bandwidth_limiter.as_deref(),
                        &telemetry,
                        downloaded_files_offset,
                        stage,
                        progress,
                    )?;
                    completed += 1;
                }
                Ok(completed)
            }));
        }

        let mut total = 0u32;
        for worker in workers {
            match worker.join() {
                Ok(Ok(count)) => total += count,
                Ok(Err(err)) => return Err(err),
                Err(_) => {
                    return Err(InstallationError::Io(std::io::Error::other(
                        "download worker panicked",
                    )));
                }
            }
        }
        Ok(total)
    })?;

    emit_download_progress(progress, &telemetry, stage, downloaded_files_offset);
    tracing::info!(
        target: "vertexlauncher/installation/downloads",
        "Completed {:?} batch: {} file(s) in {:.2}s.",
        stage,
        downloaded_files,
        batch_started_at.elapsed().as_secs_f64()
    );
    Ok(downloaded_files)
}

pub(crate) fn download_to_file(
    task: FileDownloadTask,
    bandwidth_limiter: Option<&BandwidthLimiter>,
    telemetry: &DownloadTelemetry,
    downloaded_files_offset: u32,
    stage: InstallStage,
    progress: Option<&InstallProgressSink>,
) -> Result<(), InstallationError> {
    if let Some(parent) = task.destination.parent() {
        fs_create_dir_all(parent)?;
    }
    let started_at = Instant::now();
    tracing::debug!(
        target: "vertexlauncher/installation/downloads",
        "Download start: {} -> {}",
        task.url,
        task.destination.display()
    );

    let mut response = call_get_response_with_retry(task.url.as_str(), DEFAULT_USER_AGENT)?;
    if task.expected_size.is_none()
        && let Some(content_length) = response
            .headers()
            .get("Content-Length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|size| *size > 0)
    {
        telemetry
            .known_total_bytes
            .fetch_add(content_length, Ordering::Relaxed);
    }

    let temp_path = temporary_download_path(task.destination.as_path());
    let mut reader = response.body_mut().as_reader();
    let mut file = fs_file_create(&temp_path)?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|err| {
            tracing::error!(
                target: "vertexlauncher/installation/downloads",
                url = %task.url,
                temp_path = %temp_path.display(),
                destination = %task.destination.display(),
                error = %err,
                "Failed while reading HTTP response body for download."
            );
            InstallationError::Io(err)
        })?;
        if read == 0 {
            break;
        }
        if let Some(limiter) = bandwidth_limiter {
            limiter.consume(read);
        }
        telemetry
            .downloaded_bytes
            .fetch_add(read as u64, Ordering::Relaxed);
        emit_download_progress(progress, telemetry, stage, downloaded_files_offset);
        file.write_all(&buffer[..read]).map_err(|err| {
            tracing::error!(
                target: "vertexlauncher/installation/downloads",
                url = %task.url,
                temp_path = %temp_path.display(),
                destination = %task.destination.display(),
                error = %err,
                "Failed while writing download chunk to temporary file."
            );
            InstallationError::Io(err)
        })?;
    }
    file.flush().map_err(|err| {
        tracing::error!(
            target: "vertexlauncher/installation/downloads",
            url = %task.url,
            temp_path = %temp_path.display(),
            destination = %task.destination.display(),
            error = %err,
            "Failed while flushing temporary download file."
        );
        InstallationError::Io(err)
    })?;
    fs_rename(temp_path.as_path(), task.destination.as_path()).map_err(|err| {
        tracing::error!(
            target: "vertexlauncher/installation/downloads",
            url = %task.url,
            temp_path = %temp_path.display(),
            destination = %task.destination.display(),
            error = %err,
            "Failed while promoting temporary download file into place."
        );
        err
    })?;
    telemetry.completed_files.fetch_add(1, Ordering::Relaxed);
    emit_download_progress(progress, telemetry, stage, downloaded_files_offset);
    tracing::debug!(
        target: "vertexlauncher/installation/downloads",
        "Download complete: {} ({:.2}s)",
        task.destination.display(),
        started_at.elapsed().as_secs_f64()
    );
    Ok(())
}
