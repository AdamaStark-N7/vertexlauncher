use super::*;

#[derive(Debug)]
pub(crate) struct BandwidthLimiter {
    bits_per_second: u64,
    state: Mutex<BandwidthState>,
}

#[derive(Debug)]
pub(crate) struct BandwidthState {
    window_start: Instant,
    bits_sent: u128,
}

impl BandwidthLimiter {
    pub(super) fn new(bits_per_second: u64) -> Self {
        Self {
            bits_per_second: bits_per_second.max(1),
            state: Mutex::new(BandwidthState {
                window_start: Instant::now(),
                bits_sent: 0,
            }),
        }
    }

    pub(super) fn consume(&self, bytes: usize) {
        let requested_bits = (bytes as u128).saturating_mul(8);
        loop {
            let wait_duration = {
                let Ok(mut state) = self.state.lock() else {
                    return;
                };
                let elapsed = state.window_start.elapsed();
                if elapsed >= Duration::from_secs(1) {
                    state.window_start = Instant::now();
                    state.bits_sent = 0;
                }
                let max_bits = self.bits_per_second as u128;
                if state.bits_sent.saturating_add(requested_bits) <= max_bits {
                    state.bits_sent = state.bits_sent.saturating_add(requested_bits);
                    None
                } else {
                    Some(Duration::from_secs(1).saturating_sub(elapsed))
                }
            };
            if let Some(wait) = wait_duration {
                thread::sleep(wait.max(Duration::from_millis(1)));
                continue;
            }
            return;
        }
    }
}

#[derive(Debug)]
pub(crate) struct DownloadTelemetry {
    pub(super) started_at: Instant,
    pub(super) total_files: u32,
    pub(super) completed_files: AtomicU32,
    pub(super) downloaded_bytes: AtomicU64,
    pub(super) known_total_bytes: AtomicU64,
    pub(super) last_emit_millis: AtomicU64,
    pub(super) eta_state: Mutex<ProgressEtaState>,
}

impl DownloadTelemetry {
    pub(super) fn new(total_files: u32, known_total_bytes: u64) -> Self {
        Self {
            started_at: Instant::now(),
            total_files,
            completed_files: AtomicU32::new(0),
            downloaded_bytes: AtomicU64::new(0),
            known_total_bytes: AtomicU64::new(known_total_bytes),
            last_emit_millis: AtomicU64::new(0),
            eta_state: Mutex::new(ProgressEtaState::default()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProgressEtaPoint {
    pub(crate) fraction: f64,
    pub(crate) at_millis: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProgressEtaState {
    last_point: Option<ProgressEtaPoint>,
    last_eta_seconds: Option<u64>,
}

impl ProgressEtaState {
    pub(crate) fn observe(&mut self, point: ProgressEtaPoint) -> Option<u64> {
        let fraction = point.fraction.clamp(0.0, 1.0);
        if fraction >= 1.0 {
            self.last_point = Some(point);
            self.last_eta_seconds = Some(0);
            return Some(0);
        }

        let Some(previous) = self.last_point else {
            self.last_point = Some(point);
            self.last_eta_seconds = None;
            return None;
        };

        if fraction < previous.fraction || point.at_millis <= previous.at_millis {
            self.last_point = Some(point);
            self.last_eta_seconds = None;
            return None;
        }

        let delta_fraction = fraction - previous.fraction;
        if delta_fraction <= f64::EPSILON {
            return self.last_eta_seconds;
        }

        let delta_seconds = (point.at_millis - previous.at_millis) as f64 / 1000.0;
        if delta_seconds <= 0.0 {
            return self.last_eta_seconds;
        }

        let fraction_per_second = delta_fraction / delta_seconds;
        let eta_seconds = if fraction_per_second > 0.0 {
            Some(((1.0 - fraction) / fraction_per_second).ceil().max(0.0) as u64)
        } else {
            None
        };

        self.last_point = Some(point);
        self.last_eta_seconds = eta_seconds;
        eta_seconds
    }
}

pub(crate) fn install_progress_fraction(
    downloaded_files: u32,
    total_files: u32,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> f64 {
    if let Some(total_bytes) = total_bytes
        && total_bytes > 0
    {
        return (downloaded_bytes as f64 / total_bytes as f64).clamp(0.0, 1.0);
    }
    if total_files > 0 {
        return (downloaded_files as f64 / total_files as f64).clamp(0.0, 1.0);
    }
    0.0
}

pub(crate) fn emit_download_progress(
    progress: Option<&InstallProgressSink>,
    telemetry: &DownloadTelemetry,
    stage: InstallStage,
    downloaded_files_offset: u32,
) {
    let Some(progress) = progress else {
        return;
    };

    let now_millis = telemetry.started_at.elapsed().as_millis() as u64;
    let last_millis = telemetry.last_emit_millis.load(Ordering::Relaxed);
    if now_millis > 0 && now_millis.saturating_sub(last_millis) < 200 {
        return;
    }
    telemetry
        .last_emit_millis
        .store(now_millis, Ordering::Relaxed);

    let completed_files = telemetry.completed_files.load(Ordering::Relaxed);
    let downloaded_bytes = telemetry.downloaded_bytes.load(Ordering::Relaxed);
    let known_total_bytes = telemetry.known_total_bytes.load(Ordering::Relaxed);
    let elapsed = telemetry.started_at.elapsed().as_secs_f64().max(0.001);
    let bytes_per_second = downloaded_bytes as f64 / elapsed;
    let total_bytes = (known_total_bytes > 0).then_some(known_total_bytes);
    let downloaded_files = downloaded_files_offset.saturating_add(completed_files);
    let total_files = downloaded_files_offset.saturating_add(telemetry.total_files);
    let fraction =
        install_progress_fraction(downloaded_files, total_files, downloaded_bytes, total_bytes);
    let eta_seconds = telemetry.eta_state.lock().ok().and_then(|mut state| {
        state.observe(ProgressEtaPoint {
            fraction,
            at_millis: now_millis,
        })
    });

    progress(InstallProgress {
        stage,
        message: format!(
            "Downloading files ({}/{})...",
            downloaded_files, total_files
        ),
        downloaded_files,
        total_files,
        downloaded_bytes,
        total_bytes,
        bytes_per_second,
        eta_seconds,
    });
}
