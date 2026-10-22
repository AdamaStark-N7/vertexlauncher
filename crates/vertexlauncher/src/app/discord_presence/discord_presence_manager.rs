use super::*;

/// Decides what Discord should show and hands it to a background task that owns the socket. Nothing here touches the
/// Discord IPC socket: connecting to it blocks until Discord answers, and a Discord that accepts the connection but
/// never answers (still starting, or stuck) would otherwise freeze the launcher's window with it.
pub struct DiscordPresenceManager {
    pub(super) session_start_by_instance_id: HashMap<String, i64>,
    /// What Discord is known to be showing (confirmed by the task).
    pub(super) active_presence: Option<DesiredPresence>,
    /// The presence most recently asked for.
    pub(super) last_desired_presence: Option<DesiredPresence>,
    pub(super) last_presence_sync_at: Option<Instant>,
    /// A request is with the task and its answer has not come back yet.
    pub(super) in_flight: bool,
    /// When the last request went out and whether it failed, so a failing one is not repeated every frame.
    pub(super) last_request_at: Option<Instant>,
    pub(super) last_request_failed: bool,
    pub(super) worker: Option<PresenceWorker>,
}

impl Default for DiscordPresenceManager {
    fn default() -> Self {
        Self {
            session_start_by_instance_id: HashMap::new(),
            active_presence: None,
            last_desired_presence: None,
            last_presence_sync_at: None,
            in_flight: false,
            last_request_at: None,
            last_request_failed: false,
            worker: None,
        }
    }
}
