use auth::CachedAccount;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::webview_sign_in;

pub const REPAINT_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Debug)]
pub enum AuthUiStatus {
    Idle,
    Starting,
    AwaitingBrowser,
    WaitingForAuthorization,
    Success(String),
    Error(String),
}

impl AuthUiStatus {
    fn status_message(&self) -> Option<&str> {
        match self {
            AuthUiStatus::Idle => None,
            AuthUiStatus::Starting => Some("Preparing Microsoft sign-in..."),
            AuthUiStatus::AwaitingBrowser => {
                Some("Complete sign-in in the Microsoft webview window...")
            }
            AuthUiStatus::WaitingForAuthorization => Some("Finalizing sign-in..."),
            AuthUiStatus::Success(message) | AuthUiStatus::Error(message) => Some(message.as_str()),
        }
    }
}

enum AuthFlowEvent {
    AwaitingBrowser,
    WaitingForAuthorization,
    Completed(CachedAccount),
    Failed(String),
}

pub struct AuthState {
    account: Option<CachedAccount>,
    avatar_png: Option<Vec<u8>>,
    flow: Option<Receiver<AuthFlowEvent>>,
    status: AuthUiStatus,
}

impl AuthState {
    pub fn load() -> Self {
        let (account, status) = match auth::load_cached_account() {
            Ok(account) => (account, AuthUiStatus::Idle),
            Err(err) => (
                None,
                AuthUiStatus::Error(format!("Failed to load cached account state: {err}")),
            ),
        };

        Self {
            avatar_png: account.as_ref().and_then(CachedAccount::avatar_png_bytes),
            account,
            flow: None,
            status,
        }
    }

    pub fn poll(&mut self) {
        let mut flow_finished = false;

        if let Some(flow) = self.flow.as_mut() {
            loop {
                match flow.try_recv() {
                    Ok(event) => match event {
                        AuthFlowEvent::AwaitingBrowser => {
                            self.status = AuthUiStatus::AwaitingBrowser;
                        }
                        AuthFlowEvent::WaitingForAuthorization => {
                            self.status = AuthUiStatus::WaitingForAuthorization;
                        }
                        AuthFlowEvent::Completed(account) => {
                            self.avatar_png = account.avatar_png_bytes();
                            self.status = AuthUiStatus::Success(format!(
                                "Signed in as {}",
                                account.minecraft_profile.name
                            ));
                            self.account = Some(account.clone());

                            if let Err(err) = auth::save_cached_account(&account) {
                                self.status = AuthUiStatus::Error(format!(
                                    "Sign-in succeeded, but failed to cache account state: {err}",
                                ));
                            }

                            flow_finished = true;
                        }
                        AuthFlowEvent::Failed(err) => {
                            self.status = AuthUiStatus::Error(err);
                            flow_finished = true;
                        }
                    },
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !flow_finished {
                            self.status = AuthUiStatus::Error(
                                "Sign-in stopped unexpectedly before completion".to_owned(),
                            );
                        }
                        flow_finished = true;
                        break;
                    }
                }
            }
        }

        if flow_finished {
            self.flow = None;
        }
    }

    pub fn start_sign_in(&mut self) {
        if self.flow.is_some() {
            return;
        }

        let client_id = match microsoft_client_id() {
            Ok(client_id) => client_id,
            Err(err) => {
                self.status = AuthUiStatus::Error(err);
                return;
            }
        };

        self.status = AuthUiStatus::Starting;

        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            run_sign_in_flow(client_id, sender);
        });

        self.flow = Some(receiver);
    }

    pub fn sign_out(&mut self) {
        self.flow = None;
        self.account = None;
        self.avatar_png = None;
        self.status = AuthUiStatus::Idle;

        if let Err(err) = auth::clear_cached_account() {
            self.status = AuthUiStatus::Error(format!(
                "Signed out in memory, but failed to clear cached account state: {err}",
            ));
        }
    }

    pub fn should_request_repaint(&self) -> bool {
        self.flow.is_some()
    }

    pub fn sign_in_in_progress(&self) -> bool {
        self.flow.is_some()
    }

    pub fn display_name(&self) -> Option<&str> {
        self.account
            .as_ref()
            .map(|account| account.minecraft_profile.name.as_str())
    }

    pub fn avatar_png(&self) -> Option<&[u8]> {
        self.avatar_png.as_deref()
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status.status_message()
    }
}

fn run_sign_in_flow(client_id: String, sender: mpsc::Sender<AuthFlowEvent>) {
    let flow = match auth::login_begin(client_id) {
        Ok(flow) => flow,
        Err(err) => {
            let _ = sender.send(AuthFlowEvent::Failed(err.to_string()));
            return;
        }
    };

    let _ = sender.send(AuthFlowEvent::AwaitingBrowser);

    let callback_url = match webview_sign_in::open_microsoft_sign_in(
        &flow.auth_request_uri,
        auth::oauth_redirect_uri(),
    ) {
        Ok(callback_url) => callback_url,
        Err(err) => {
            let _ = sender.send(AuthFlowEvent::Failed(err));
            return;
        }
    };

    let _ = sender.send(AuthFlowEvent::WaitingForAuthorization);

    match auth::login_finish_from_redirect(&callback_url, flow) {
        Ok(account) => {
            let _ = sender.send(AuthFlowEvent::Completed(account));
        }
        Err(err) => {
            let _ = sender.send(AuthFlowEvent::Failed(err.to_string()));
        }
    }
}

fn microsoft_client_id() -> Result<String, String> {
    let client_id = std::env::var("VERTEX_MSA_CLIENT_ID")
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
        .or_else(|| auth::builtin_client_id().map(str::to_owned))
        .ok_or_else(|| {
            "Microsoft OAuth client ID is not configured. Set VERTEX_MSA_CLIENT_ID or set \
auth::BUILTIN_MICROSOFT_CLIENT_ID in crates/auth/src/lib.rs."
                .to_owned()
        })?;

    if is_valid_microsoft_client_id(&client_id) {
        Ok(client_id)
    } else {
        Err(format!(
            "Invalid Microsoft client id '{client_id}'. Set VERTEX_MSA_CLIENT_ID to a valid \
16-character hex id or GUID application id.",
        ))
    }
}

fn is_valid_microsoft_client_id(value: &str) -> bool {
    is_hex_client_id(value) || is_guid_client_id(value)
}

fn is_hex_client_id(value: &str) -> bool {
    value.len() == 16 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_guid_client_id(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }

    for (index, ch) in value.chars().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if ch != '-' {
                return false;
            }
            continue;
        }

        if !ch.is_ascii_hexdigit() {
            return false;
        }
    }

    true
}
