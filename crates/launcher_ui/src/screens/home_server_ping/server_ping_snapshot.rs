use super::*;

#[derive(Debug, Clone)]
pub(crate) struct ServerPingSnapshot {
    pub(crate) status: ServerPingStatus,
    pub(crate) motd: Option<String>,
    pub(crate) players_online: Option<u32>,
    pub(crate) players_max: Option<u32>,
    /// Names from the status response's player sample; servers may cap, hide, or fake this list.
    pub(crate) players_sample: Vec<String>,
    pub(crate) checked_at: Instant,
}
