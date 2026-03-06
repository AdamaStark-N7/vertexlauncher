mod cache;
mod constants;
mod device_code;
mod error;
mod minecraft;
mod oauth;
mod runtime;
mod types;
mod util;

use std::sync::mpsc;

use tokio::runtime::Handle;

pub use constants::{BUILTIN_MICROSOFT_CLIENT_ID, BUILTIN_MICROSOFT_TENANT};
pub use error::AuthError;
pub use types::{
    CachedAccount, CachedAccountsState, DeviceCodeLoginFlow, DeviceCodePrompt, LoginEvent,
    MinecraftCapeState, MinecraftLoginFlow, MinecraftProfileState, MinecraftSkinState,
};

pub fn builtin_client_id() -> Option<&'static str> {
    let value = BUILTIN_MICROSOFT_CLIENT_ID.trim();
    if value.is_empty() { None } else { Some(value) }
}

pub fn oauth_redirect_uri() -> &'static str {
    constants::LIVE_REDIRECT_URI
}

pub fn start_device_code_login(client_id: impl Into<String>) -> DeviceCodeLoginFlow {
    if let Ok(handle) = Handle::try_current() {
        return start_device_code_login_with_handle(client_id, &handle);
    }

    start_device_code_login_with_handle(client_id, runtime::auth_runtime_handle())
}

pub fn start_device_code_login_with_handle(
    client_id: impl Into<String>,
    handle: &Handle,
) -> DeviceCodeLoginFlow {
    let client_id = client_id.into();
    let (sender, receiver) = mpsc::channel();
    let sender_for_task = sender.clone();

    // Run the blocking device-code polling flow on the runtime's blocking pool
    // so UI threads stay responsive.
    handle.spawn_blocking(move || {
        if let Err(err) = device_code::run_device_code_login(client_id, &sender_for_task) {
            let _ = sender_for_task.send(LoginEvent::Failed(err.to_string()));
        }
    });

    DeviceCodeLoginFlow {
        receiver,
        finished: false,
    }
}

pub fn login_begin(client_id: impl Into<String>) -> Result<MinecraftLoginFlow, AuthError> {
    oauth::login_begin(client_id.into())
}

pub fn login_finish(code: &str, flow: MinecraftLoginFlow) -> Result<CachedAccount, AuthError> {
    let agent = util::build_http_agent();

    // Exchange the browser auth code for a Microsoft OAuth access token first.
    let microsoft_token = oauth::exchange_auth_code_for_microsoft_token(&agent, code, &flow)
        .map_err(|err| error::prefix_auth_error("GetOAuthToken", err))?;

    // Continue through Xbox -> XSTS -> Minecraft service token chain.
    minecraft::complete_minecraft_login(&agent, &microsoft_token.access_token)
}

pub fn login_finish_from_redirect(
    callback_url: &str,
    flow: MinecraftLoginFlow,
) -> Result<CachedAccount, AuthError> {
    let code = oauth::extract_authorization_code(callback_url, &flow.state)?;
    login_finish(&code, flow)
}

pub fn load_cached_accounts() -> Result<CachedAccountsState, AuthError> {
    cache::load_cached_accounts()
}

pub fn save_cached_accounts(state: &CachedAccountsState) -> Result<(), AuthError> {
    cache::save_cached_accounts(state)
}

pub fn clear_cached_accounts() -> Result<(), AuthError> {
    cache::clear_cached_accounts()
}

pub fn load_cached_account() -> Result<Option<CachedAccount>, AuthError> {
    cache::load_cached_account()
}

pub fn save_cached_account(account: &CachedAccount) -> Result<(), AuthError> {
    cache::save_cached_account(account)
}

pub fn clear_cached_account() -> Result<(), AuthError> {
    cache::clear_cached_account()
}
