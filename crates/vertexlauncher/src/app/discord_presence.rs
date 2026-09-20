use std::collections::{HashMap, HashSet};
#[cfg(target_os = "linux")]
use std::env;
use std::fs;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use config::Config;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use installation::{display_user_path, running_instance_roots};
use instances::{InstanceStore, instance_root_path};
use launcher_ui::screens::{
    AppScreen, HomePresenceSection, InstancePresenceSection, MenuPresenceContext,
};
use vertex_constants::branding::DISCORD_APPLICATION_ID;
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const PRESENCE_RESYNC_INTERVAL: Duration = Duration::from_secs(15);

#[path = "discord_presence/desired_presence.rs"]
mod desired_presence;
#[path = "discord_presence/discord_presence_manager.rs"]
mod discord_presence_manager;

use self::desired_presence::DesiredPresence;
pub use self::discord_presence_manager::DiscordPresenceManager;

impl DiscordPresenceManager {
    pub fn update(
        &mut self,
        config: &Config,
        instances: &InstanceStore,
        installations_root: &Path,
        menu_context: MenuPresenceContext,
        selected_instance_id: Option<&str>,
    ) {
        let running_roots = running_instance_roots();
        let desired = self.desired_presence(
            config,
            instances,
            installations_root,
            &running_roots,
            menu_context,
            selected_instance_id,
        );
        let should_resync = self
            .last_presence_sync_at
            .is_none_or(|last| last.elapsed() >= PRESENCE_RESYNC_INTERVAL);

        if desired == self.active_presence && !should_resync {
            return;
        }

        if desired.is_some()
            && !self.connected
            && !self.can_attempt_connect()
            && desired == self.last_desired_presence
        {
            return;
        }

        match desired.clone() {
            Some(next) => {
                self.last_desired_presence = Some(next.clone());
                if self.set_presence(&next).is_ok() {
                    if self.active_presence.is_none() {
                        tracing::info!(
                            target: "vertexlauncher/discord_presence",
                            "Discord Rich Presence started."
                        );
                    }
                    self.active_presence = Some(next);
                }
            }
            None => {
                if self.active_presence.is_some() {
                    tracing::info!(
                        target: "vertexlauncher/discord_presence",
                        "Discord Rich Presence stopped."
                    );
                }
                self.clear_presence();
                self.active_presence = None;
                self.last_desired_presence = None;
            }
        }
    }

    fn desired_presence(
        &mut self,
        config: &Config,
        instances: &InstanceStore,
        installations_root: &Path,
        running_roots: &[String],
        menu_context: MenuPresenceContext,
        selected_instance_id: Option<&str>,
    ) -> Option<DesiredPresence> {
        let running_set: HashSet<&str> = running_roots.iter().map(String::as_str).collect();
        let mut active_instance_ids = HashSet::new();

        let mut first_eligible = None;
        let mut launcher_presence_blocked_by_mod = false;
        for instance in &instances.instances {
            // Nothing is running, so nothing can match: skip the per-instance canonicalize
            // (a filesystem call) that this per-frame path would otherwise make for every instance.
            if running_set.is_empty() {
                break;
            }
            let instance_root = instance_root_path(installations_root, instance);
            let instance_key = fs::canonicalize(instance_root.as_path())
                .map(|path| display_user_path(path.as_path()))
                .unwrap_or_else(|_| display_user_path(instance_root.as_path()));
            if !running_set.contains(instance_key.as_str()) {
                continue;
            }
            active_instance_ids.insert(instance.id.clone());
            if instance.discord_rich_presence_mod_installed {
                launcher_presence_blocked_by_mod = true;
                continue;
            }
            if first_eligible.is_none() {
                let started_at_unix_secs = *self
                    .session_start_by_instance_id
                    .entry(instance.id.clone())
                    .or_insert_with(current_unix_timestamp_secs);
                first_eligible = Some(DesiredPresence::InGame {
                    instance_id: instance.id.clone(),
                    instance_name: instance.name.clone(),
                    started_at_unix_secs,
                });
            }
        }

        self.session_start_by_instance_id
            .retain(|instance_id, _| active_instance_ids.contains(instance_id));

        if !config.discord_rich_presence_enabled() {
            return None;
        }

        if let Some(presence) = first_eligible {
            return Some(presence);
        }

        if launcher_presence_blocked_by_mod {
            return None;
        }

        Some(self.menu_presence(menu_context, selected_instance_id, instances))
    }

    fn set_presence(&mut self, desired: &DesiredPresence) -> Result<(), String> {
        let activity = build_activity(desired);

        let initial_attempt = self
            .ensure_client_connected()?
            .set_activity(activity.clone())
            .map_err(|err| format!("failed to set Discord activity: {err}"));

        match initial_attempt {
            Ok(()) => {
                self.last_presence_sync_at = Some(Instant::now());
                Ok(())
            }
            Err(initial_err) => {
                self.reset_client();
                self.ensure_client_connected()?
                    .set_activity(activity)
                    .map_err(|err| {
                        format!(
                            "{initial_err}; retry after reconnect also failed to set Discord activity: {err}"
                        )
                    })?;
                self.last_presence_sync_at = Some(Instant::now());
                Ok(())
            }
        }
    }

    fn menu_presence(
        &self,
        menu_context: MenuPresenceContext,
        selected_instance_id: Option<&str>,
        instances: &InstanceStore,
    ) -> DesiredPresence {
        let selected_instance_name = match menu_context {
            MenuPresenceContext::Instance(_) | MenuPresenceContext::Screen(AppScreen::Instance) => {
                selected_instance_id.and_then(|instance_id| {
                    instances
                        .find(instance_id)
                        .map(|instance| instance.name.clone())
                })
            }
            _ => None,
        };
        DesiredPresence::Menu {
            context: menu_context,
            selected_instance_name,
        }
    }

    fn clear_presence(&mut self) {
        let clear_failed = if let Some(client) = self.client.as_mut() {
            if client.clear_activity().is_err() {
                true
            } else {
                false
            }
        } else {
            false
        };

        if clear_failed {
            self.reset_client();
        }
        self.last_presence_sync_at = None;
    }

    fn ensure_client_connected(&mut self) -> Result<&mut DiscordIpcClient, String> {
        if self.client.is_none() {
            prepare_discord_ipc_environment();
            self.client = Some(DiscordIpcClient::new(DISCORD_APPLICATION_ID));
        }

        if self.connected {
            return self
                .client
                .as_mut()
                .ok_or_else(|| "Discord client missing".to_owned());
        }

        let should_retry = self.can_attempt_connect();

        if should_retry {
            self.last_connect_attempt_at = Some(Instant::now());

            let connect_result = {
                let client = self
                    .client
                    .as_mut()
                    .ok_or_else(|| "Discord client missing".to_owned())?;
                client.connect()
            };

            connect_result.map_err(|err| {
                let message = format!("failed to connect to Discord IPC: {err}");
                self.last_connect_error = Some(message.clone());
                message
            })?;
            self.connected = true;
            self.last_connect_error = None;
        }

        if !self.connected {
            return Err(match self.last_connect_error.as_deref() {
                Some(previous) => format!(
                    "Discord IPC reconnect is rate-limited; waiting before retrying after previous failure: {previous}"
                ),
                None => "Discord IPC reconnect is rate-limited; waiting before retrying".to_owned(),
            });
        }

        self.client
            .as_mut()
            .ok_or_else(|| "Discord client missing".to_owned())
    }

    fn reset_client(&mut self) {
        if let Some(client) = self.client.as_mut() {
            let _ = client.close();
        }
        self.client = None;
        self.connected = false;
        self.last_connect_attempt_at = None;
        self.last_presence_sync_at = None;
    }

    fn can_attempt_connect(&self) -> bool {
        self.last_connect_attempt_at
            .is_none_or(|last| last.elapsed() >= CONNECT_RETRY_INTERVAL)
    }
}

#[cfg(target_os = "linux")]
fn prepare_discord_ipc_environment() {
    let candidate_dirs = discord_ipc_candidate_dirs();

    let Some(socket_dir) = find_discord_ipc_socket_dir(&candidate_dirs) else {
        return;
    };

    // SAFETY: This is called on the UI thread before each Discord IPC client creation.
    // We only update process env vars used by the `discord-rich-presence` crate's socket lookup.
    unsafe {
        env::set_var("TMPDIR", &socket_dir);
    }
}

#[cfg(not(target_os = "linux"))]
fn prepare_discord_ipc_environment() {}

#[cfg(target_os = "linux")]
fn discord_ipc_candidate_dirs() -> Vec<PathBuf> {
    let mut candidate_dirs = Vec::new();

    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        let runtime_dir = PathBuf::from(runtime_dir);
        candidate_dirs.push(runtime_dir.clone());
        candidate_dirs.push(runtime_dir.join("app/com.discordapp.Discord"));
        candidate_dirs.push(runtime_dir.join("app/com.discordapp.DiscordCanary"));
        candidate_dirs.push(runtime_dir.join("app/com.discordapp.DiscordPTB"));
        candidate_dirs.push(runtime_dir.join("app/dev.vencord.Vesktop"));
    }

    if let Ok(home) = env::var("HOME") {
        let home = PathBuf::from(home);
        candidate_dirs.push(home.join(".flatpak/com.discordapp.Discord/xdg-run"));
        candidate_dirs.push(home.join(".flatpak/com.discordapp.DiscordCanary/xdg-run"));
        candidate_dirs.push(home.join(".flatpak/com.discordapp.DiscordPTB/xdg-run"));
        candidate_dirs.push(home.join(".flatpak/dev.vencord.Vesktop/xdg-run"));
    }

    candidate_dirs
}

#[cfg(target_os = "linux")]
fn find_discord_ipc_socket_dir(candidate_dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in candidate_dirs {
        for index in 0..10 {
            if dir.join(format!("discord-ipc-{index}")).exists() {
                return Some(dir.clone());
            }
        }
    }

    None
}

fn build_activity(desired: &DesiredPresence) -> activity::Activity<'static> {
    match desired {
        DesiredPresence::InGame {
            instance_name,
            started_at_unix_secs,
            ..
        } => activity::Activity::new()
            .activity_type(activity::ActivityType::Playing)
            .details(format!("Playing {instance_name}"))
            .state("Launched via Vertex")
            .timestamps(activity::Timestamps::new().start(*started_at_unix_secs)),
        DesiredPresence::Menu {
            context,
            selected_instance_name,
        } => {
            let (details, state) = menu_status(*context, selected_instance_name.as_deref());
            activity::Activity::new()
                .activity_type(activity::ActivityType::Playing)
                .details(details)
                .state(state)
        }
    }
}

fn menu_status(
    context: MenuPresenceContext,
    selected_instance_name: Option<&str>,
) -> (String, &'static str) {
    match context {
        MenuPresenceContext::Home(HomePresenceSection::Activity) => {
            ("Browsing featured packs".to_owned(), "On the home screen")
        }
        MenuPresenceContext::Home(HomePresenceSection::Screenshots) => (
            "Reviewing screenshots".to_owned(),
            "In the screenshot manager",
        ),
        MenuPresenceContext::Instance(InstancePresenceSection::Content)
        | MenuPresenceContext::Screen(AppScreen::Instance) => (
            selected_instance_name
                .map(|name| format!("Managing {name}"))
                .unwrap_or_else(|| "Managing an instance".to_owned()),
            "In instance settings",
        ),
        MenuPresenceContext::Instance(InstancePresenceSection::Screenshots) => (
            selected_instance_name
                .map(|name| format!("Reviewing {name} screenshots"))
                .unwrap_or_else(|| "Reviewing instance screenshots".to_owned()),
            "In the screenshot gallery",
        ),
        MenuPresenceContext::Instance(InstancePresenceSection::Logs) => (
            selected_instance_name
                .map(|name| format!("Reading {name} logs"))
                .unwrap_or_else(|| "Reading instance logs".to_owned()),
            "In the logs viewer",
        ),
        MenuPresenceContext::Screen(AppScreen::Home) => {
            ("Browsing featured packs".to_owned(), "On the home screen")
        }
        MenuPresenceContext::Screen(AppScreen::Library) => {
            ("Managing instances".to_owned(), "In the library")
        }
        MenuPresenceContext::Screen(AppScreen::Discover) => {
            ("Browsing modpacks".to_owned(), "In Discover")
        }
        MenuPresenceContext::Screen(AppScreen::DiscoverDetail) => {
            ("Reviewing a modpack".to_owned(), "In Discover")
        }
        MenuPresenceContext::Screen(AppScreen::ContentBrowser) => {
            ("Adding content".to_owned(), "Managing mods and resources")
        }
        MenuPresenceContext::Screen(AppScreen::Skins) => {
            ("Customizing a skin".to_owned(), "In Skin Manager")
        }
        MenuPresenceContext::Screen(AppScreen::Settings) => {
            ("Adjusting settings".to_owned(), "Configuring Vertex")
        }
        MenuPresenceContext::Screen(AppScreen::Legal) => {
            ("Reviewing licenses".to_owned(), "In legal information")
        }
        MenuPresenceContext::Screen(AppScreen::Console) => {
            ("Checking logs".to_owned(), "In the console")
        }
    }
}

fn current_unix_timestamp_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
