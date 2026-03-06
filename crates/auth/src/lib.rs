use base64::Engine;
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD};
use image::{DynamicImage, ImageFormat, RgbaImage, imageops};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::runtime::{Builder, Handle, Runtime};
use url::Url;
use zeroize::Zeroizing;

const OAUTH_BASE_URL: &str = "https://login.microsoftonline.com";
const LIVE_AUTHORIZE_URL: &str = "https://login.live.com/oauth20_authorize.srf";
const LIVE_TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
const LIVE_REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";
const LIVE_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
const XBOX_USER_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_LOGIN_URL: &str = "https://api.minecraftservices.com/launcher/login";
const MINECRAFT_LOGIN_LEGACY_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MINECRAFT_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const ACCOUNT_CACHE_DEFAULT_PATH: &str = "account_cache.json";
const DEVICE_CODE_SCOPE: &str = "XboxLive.signin offline_access";
static AUTH_TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn auth_runtime() -> &'static Runtime {
    AUTH_TOKIO_RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            .thread_name("vertex-auth-tokio")
            .build()
            .expect("failed to build auth tokio runtime")
    })
}

fn auth_runtime_handle() -> &'static Handle {
    auth_runtime().handle()
}

/// Built-in Microsoft OAuth client id used when `VERTEX_MSA_CLIENT_ID` is not set.
/// Leave empty to force env-based configuration.
pub const BUILTIN_MICROSOFT_CLIENT_ID: &str = "00000000402b5328";
/// Built-in OAuth tenant used when `VERTEX_MSA_TENANT` is not set.
pub const BUILTIN_MICROSOFT_TENANT: &str = "consumers";

pub fn builtin_client_id() -> Option<&'static str> {
    let value = BUILTIN_MICROSOFT_CLIENT_ID.trim();
    if value.is_empty() { None } else { Some(value) }
}

pub fn oauth_redirect_uri() -> &'static str {
    LIVE_REDIRECT_URI
}

#[derive(Debug, Clone)]
pub struct MinecraftLoginFlow {
    pub verifier: String,
    pub challenge: String,
    pub session_id: String,
    pub auth_request_uri: String,
    state: String,
    client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftSkinState {
    pub id: String,
    pub state: String,
    pub url: String,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub alias: Option<String>,
    /// Base64-encoded PNG bytes for this skin texture.
    #[serde(default)]
    pub texture_png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftCapeState {
    pub id: String,
    pub state: String,
    pub url: String,
    #[serde(default)]
    pub alias: Option<String>,
    /// Base64-encoded PNG bytes for this cape texture.
    #[serde(default)]
    pub texture_png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfileState {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<MinecraftSkinState>,
    #[serde(default)]
    pub capes: Vec<MinecraftCapeState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAccount {
    pub minecraft_profile: MinecraftProfileState,
    /// Base64-encoded PNG bytes for the generated profile avatar.
    #[serde(default)]
    pub avatar_png_base64: Option<String>,
    pub cached_at_unix_secs: u64,
}

impl CachedAccount {
    pub fn avatar_png_bytes(&self) -> Option<Vec<u8>> {
        self.avatar_png_base64
            .as_deref()
            .and_then(|raw| decode_base64(raw).ok())
    }
}

#[derive(Debug, Clone)]
pub struct DeviceCodePrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in_secs: u64,
    pub poll_interval_secs: u64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum LoginEvent {
    DeviceCode(DeviceCodePrompt),
    WaitingForAuthorization,
    Completed(CachedAccount),
    Failed(String),
}

#[derive(Debug)]
pub struct DeviceCodeLoginFlow {
    receiver: Receiver<LoginEvent>,
    finished: bool,
}

impl DeviceCodeLoginFlow {
    pub fn poll_events(&mut self) -> Vec<LoginEvent> {
        let mut out = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    if matches!(event, LoginEvent::Completed(_) | LoginEvent::Failed(_)) {
                        self.finished = true;
                    }
                    out.push(event);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.finished = true;
                    break;
                }
            }
        }
        out
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Failed to decode image data: {0}")]
    Image(#[from] image::ImageError),
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Device-code authorization timed out")]
    DeviceCodeExpired,
    #[error("Microsoft authorization was declined")]
    AuthorizationDeclined,
    #[error("Minecraft Java profile is unavailable for this account")]
    MinecraftProfileUnavailable,
    #[error("OAuth error: {0}")]
    OAuth(String),
}

pub fn start_device_code_login(client_id: impl Into<String>) -> DeviceCodeLoginFlow {
    if let Ok(handle) = Handle::try_current() {
        return start_device_code_login_with_handle(client_id, &handle);
    }

    start_device_code_login_with_handle(client_id, auth_runtime_handle())
}

pub fn start_device_code_login_with_handle(
    client_id: impl Into<String>,
    handle: &Handle,
) -> DeviceCodeLoginFlow {
    let client_id = client_id.into();
    let (sender, receiver) = mpsc::channel();
    let sender_for_task = sender.clone();

    handle.spawn_blocking(move || {
        if let Err(err) = run_device_code_login(client_id, &sender_for_task) {
            let _ = sender_for_task.send(LoginEvent::Failed(err.to_string()));
        }
    });

    DeviceCodeLoginFlow {
        receiver,
        finished: false,
    }
}

pub fn login_begin(client_id: impl Into<String>) -> Result<MinecraftLoginFlow, AuthError> {
    let client_id = client_id.into();
    let verifier = generate_pkce_verifier();
    let challenge = pkce_challenge(&verifier);
    let state = generate_random_token(24);
    let session_id = generate_random_token(16);

    let mut auth_url = Url::parse(LIVE_AUTHORIZE_URL)
        .map_err(|err| AuthError::OAuth(format!("Failed to build authorize URL: {err}")))?;
    {
        let mut query = auth_url.query_pairs_mut();
        query.append_pair("client_id", &client_id);
        query.append_pair("response_type", "code");
        query.append_pair("redirect_uri", LIVE_REDIRECT_URI);
        query.append_pair("scope", LIVE_SCOPE);
        query.append_pair("code_challenge", &challenge);
        query.append_pair("code_challenge_method", "S256");
        query.append_pair("state", &state);
        query.append_pair("prompt", "select_account");
    }

    Ok(MinecraftLoginFlow {
        verifier,
        challenge,
        session_id,
        auth_request_uri: auth_url.to_string(),
        state,
        client_id,
    })
}

pub fn login_finish(code: &str, flow: MinecraftLoginFlow) -> Result<CachedAccount, AuthError> {
    let agent = build_http_agent();

    let microsoft_token = exchange_auth_code_for_microsoft_token(&agent, code, &flow)?;
    let microsoft_access_token = Zeroizing::new(microsoft_token.access_token);
    let xbox_user = authenticate_with_xbox_live(&agent, &microsoft_access_token)?;
    let xsts_token = Zeroizing::new(authorize_xsts(&agent, &xbox_user.token)?);

    let minecraft_token = Zeroizing::new(authenticate_with_minecraft(
        &agent,
        &xbox_user.user_hash,
        &xsts_token,
    )?);
    let _ = fetch_minecraft_entitlements(&agent, &minecraft_token);
    let profile = fetch_minecraft_profile(&agent, &minecraft_token)?;

    Ok(build_cached_account(&agent, profile))
}

pub fn login_finish_from_redirect(
    callback_url: &str,
    flow: MinecraftLoginFlow,
) -> Result<CachedAccount, AuthError> {
    let code = extract_authorization_code(callback_url, &flow.state)?;
    login_finish(&code, flow)
}

pub fn load_cached_account() -> Result<Option<CachedAccount>, AuthError> {
    let path = account_cache_path();
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(path)?;
    let parsed: CachedAccount = serde_json::from_str(&contents)?;
    Ok(Some(parsed))
}

pub fn save_cached_account(account: &CachedAccount) -> Result<(), AuthError> {
    let path = account_cache_path();
    let json = serde_json::to_string_pretty(account)?;
    write_secure_file_atomic(&path, json.as_bytes())
}

pub fn clear_cached_account() -> Result<(), AuthError> {
    let path = account_cache_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn run_device_code_login(client_id: String, sender: &Sender<LoginEvent>) -> Result<(), AuthError> {
    let agent = build_http_agent();
    let tenant = oauth_tenant();

    let device_code = request_device_code(&agent, &client_id, &tenant)?;
    let prompt = DeviceCodePrompt {
        user_code: device_code.user_code.clone(),
        verification_uri: device_code.verification_uri.clone(),
        verification_uri_complete: device_code.verification_uri_complete.clone(),
        expires_in_secs: device_code.expires_in,
        poll_interval_secs: device_code.interval.max(1),
        message: device_code.message.clone(),
    };
    let _ = sender.send(LoginEvent::DeviceCode(prompt));

    let microsoft_token =
        poll_for_microsoft_token(&agent, &client_id, &tenant, &device_code, sender)?;

    let microsoft_access_token = Zeroizing::new(microsoft_token.access_token);
    let xbox_user = authenticate_with_xbox_live(&agent, &microsoft_access_token)?;
    let xbox_token = Zeroizing::new(xbox_user.token);
    let xsts_token = Zeroizing::new(authorize_xsts(&agent, &xbox_token)?);

    let minecraft_token = Zeroizing::new(authenticate_with_minecraft(
        &agent,
        &xbox_user.user_hash,
        &xsts_token,
    )?);
    let _ = fetch_minecraft_entitlements(&agent, &minecraft_token);
    let profile = fetch_minecraft_profile(&agent, &minecraft_token)?;
    let account = build_cached_account(&agent, profile);

    let _ = sender.send(LoginEvent::Completed(account));
    Ok(())
}

fn build_http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .timeout_write(Duration::from_secs(20))
        .build()
}

fn exchange_auth_code_for_microsoft_token(
    agent: &ureq::Agent,
    code: &str,
    flow: &MinecraftLoginFlow,
) -> Result<MicrosoftTokenResponse, AuthError> {
    let response = agent
        .post(LIVE_TOKEN_URL)
        .set("Accept", "application/json")
        .send_form(&[
            ("client_id", flow.client_id.as_str()),
            ("code", code),
            ("code_verifier", flow.verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", LIVE_REDIRECT_URI),
            ("scope", LIVE_SCOPE),
        ]);

    match response {
        Ok(ok) => Ok(ok.into_json::<MicrosoftTokenResponse>()?),
        Err(ureq::Error::Status(_, err_response)) => {
            if let Ok(oauth_error) = err_response.into_json::<OAuthErrorResponse>() {
                let description = oauth_error
                    .error_description
                    .unwrap_or_else(|| "No details provided".to_owned());
                return Err(AuthError::OAuth(format!(
                    "{}: {}",
                    oauth_error.error, description
                )));
            }

            Err(AuthError::OAuth(
                "Authorization-code exchange failed with an unknown response".to_owned(),
            ))
        }
        Err(err) => Err(map_http_error(err)),
    }
}

fn extract_authorization_code(
    callback_url: &str,
    expected_state: &str,
) -> Result<String, AuthError> {
    let parsed = Url::parse(callback_url)
        .map_err(|err| AuthError::OAuth(format!("Failed to parse callback URL: {err}")))?;

    if parsed.scheme() != "https"
        || parsed.host_str() != Some("login.live.com")
        || parsed.path() != "/oauth20_desktop.srf"
    {
        return Err(AuthError::OAuth(
            "Microsoft callback URL did not match the expected redirect URI".to_owned(),
        ));
    }

    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut error_description = None;

    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "error_description" => error_description = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(error) = error {
        let description = error_description.unwrap_or_else(|| "No details provided".to_owned());
        return Err(AuthError::OAuth(format!(
            "Microsoft sign-in failed: {error}: {description}"
        )));
    }

    let returned_state = state.ok_or_else(|| {
        AuthError::OAuth("Microsoft callback was missing the OAuth state".to_owned())
    })?;
    if returned_state != expected_state {
        return Err(AuthError::OAuth(
            "Microsoft callback state did not match the login session".to_owned(),
        ));
    }

    code.ok_or_else(|| {
        AuthError::OAuth("Microsoft callback did not include an auth code".to_owned())
    })
}

fn build_cached_account(agent: &ureq::Agent, profile: MinecraftProfileResponse) -> CachedAccount {
    let mut minecraft_profile = MinecraftProfileState {
        id: profile.id,
        name: profile.name,
        skins: Vec::new(),
        capes: Vec::new(),
    };

    for raw_skin in profile.skins {
        let texture_png_base64 = fetch_texture_base64(agent, &raw_skin.url);
        minecraft_profile.skins.push(MinecraftSkinState {
            id: raw_skin.id,
            state: raw_skin.state,
            url: raw_skin.url,
            variant: raw_skin.variant,
            alias: raw_skin.alias,
            texture_png_base64,
        });
    }

    for raw_cape in profile.capes {
        let texture_png_base64 = fetch_texture_base64(agent, &raw_cape.url);
        minecraft_profile.capes.push(MinecraftCapeState {
            id: raw_cape.id,
            state: raw_cape.state,
            url: raw_cape.url,
            alias: raw_cape.alias,
            texture_png_base64,
        });
    }

    let avatar_png_base64 = generate_avatar_from_profile(&minecraft_profile);

    CachedAccount {
        minecraft_profile,
        avatar_png_base64,
        cached_at_unix_secs: unix_now_secs(),
    }
}

fn request_device_code(
    agent: &ureq::Agent,
    client_id: &str,
    tenant: &str,
) -> Result<DeviceCodeResponse, AuthError> {
    let url = device_code_url(tenant);
    let response = agent
        .post(&url)
        .set("Accept", "application/json")
        .send_form(&[("client_id", client_id), ("scope", DEVICE_CODE_SCOPE)]);

    match response {
        Ok(ok) => Ok(ok.into_json::<DeviceCodeResponse>()?),
        Err(ureq::Error::Status(_, err_response)) => {
            if let Ok(oauth_error) = err_response.into_json::<OAuthErrorResponse>() {
                if oauth_error.error == "unauthorized_client" {
                    return Err(AuthError::OAuth(format!(
                        "unauthorized_client for client id '{client_id}' on tenant '{tenant}'. \
Set VERTEX_MSA_CLIENT_ID to your app id and ensure the app supports personal Microsoft accounts \
plus public client flows. If your app is multi-tenant/AAD, try VERTEX_MSA_TENANT=common or \
set auth::BUILTIN_MICROSOFT_TENANT in crates/auth/src/lib.rs.",
                    )));
                }

                let description = oauth_error
                    .error_description
                    .unwrap_or_else(|| "No details provided".to_owned());
                return Err(oauth_error_with_guidance(
                    &oauth_error.error,
                    &description,
                    tenant,
                ));
            }

            Err(AuthError::Http(
                "HTTP status error while requesting device code".to_owned(),
            ))
        }
        Err(err) => Err(map_http_error(err)),
    }
}

fn poll_for_microsoft_token(
    agent: &ureq::Agent,
    client_id: &str,
    tenant: &str,
    device_code: &DeviceCodeResponse,
    sender: &Sender<LoginEvent>,
) -> Result<MicrosoftTokenResponse, AuthError> {
    let expires_after = Duration::from_secs(device_code.expires_in);
    let started_at = Instant::now();
    let mut poll_interval_secs = device_code.interval.max(1);
    let mut sent_waiting = false;

    loop {
        if started_at.elapsed() >= expires_after {
            return Err(AuthError::DeviceCodeExpired);
        }

        let response = agent
            .post(&token_url(tenant))
            .set("Accept", "application/json")
            .send_form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", device_code.device_code.as_str()),
            ]);

        match response {
            Ok(ok) => {
                let parsed = ok.into_json::<MicrosoftTokenResponse>()?;
                return Ok(parsed);
            }
            Err(ureq::Error::Status(_, err_response)) => {
                let oauth_error = err_response.into_json::<OAuthErrorResponse>().ok();
                let Some(oauth_error) = oauth_error else {
                    return Err(AuthError::OAuth(
                        "Token polling failed with unknown response".to_owned(),
                    ));
                };

                match oauth_error.error.as_str() {
                    "authorization_pending" => {
                        if !sent_waiting {
                            let _ = sender.send(LoginEvent::WaitingForAuthorization);
                            sent_waiting = true;
                        }
                    }
                    "slow_down" => {
                        poll_interval_secs = (poll_interval_secs + 5).min(30);
                        if !sent_waiting {
                            let _ = sender.send(LoginEvent::WaitingForAuthorization);
                            sent_waiting = true;
                        }
                    }
                    "authorization_declined" => return Err(AuthError::AuthorizationDeclined),
                    "expired_token" | "bad_verification_code" => {
                        return Err(AuthError::DeviceCodeExpired);
                    }
                    other => {
                        let description = oauth_error
                            .error_description
                            .unwrap_or_else(|| "No details provided".to_owned());
                        return Err(oauth_error_with_guidance(other, &description, tenant));
                    }
                }
            }
            Err(other) => return Err(map_http_error(other)),
        }

        thread::sleep(Duration::from_secs(poll_interval_secs));
    }
}

fn authenticate_with_xbox_live(
    agent: &ureq::Agent,
    microsoft_access_token: &str,
) -> Result<XboxUserAuthResult, AuthError> {
    match authenticate_with_xbox_live_rps(agent, microsoft_access_token, "d=") {
        Ok(result) => Ok(result),
        Err(first_err) => {
            let first_is_401 = matches!(&first_err, AuthError::Http(message) if message.starts_with("HTTP status 401"));

            if !first_is_401 {
                return Err(first_err);
            }

            match authenticate_with_xbox_live_rps(agent, microsoft_access_token, "t=") {
                Ok(result) => Ok(result),
                Err(second_err) => Err(AuthError::Http(format!(
                    "Xbox user auth failed with both RPS ticket formats (d= then t=). First error: {first_err}; second error: {second_err}",
                ))),
            }
        }
    }
}

fn authenticate_with_xbox_live_rps(
    agent: &ureq::Agent,
    microsoft_access_token: &str,
    ticket_prefix: &str,
) -> Result<XboxUserAuthResult, AuthError> {
    let response = agent
        .post(XBOX_USER_AUTH_URL)
        .set("Accept", "application/json")
        .send_json(json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("{ticket_prefix}{microsoft_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }));

    match response {
        Ok(ok) => {
            let parsed = ok.into_json::<XboxUserAuthResponse>()?;
            let user_hash = parsed
                .display_claims
                .xui
                .first()
                .map(|entry| entry.user_hash.clone())
                .ok_or_else(|| {
                    AuthError::OAuth("Xbox response did not include user hash".to_owned())
                })?;

            Ok(XboxUserAuthResult {
                token: parsed.token,
                user_hash,
            })
        }
        Err(err) => Err(map_http_error(err)),
    }
}

fn authorize_xsts(agent: &ureq::Agent, xbox_token: &str) -> Result<String, AuthError> {
    let response = agent
        .post(XSTS_AUTH_URL)
        .set("Accept", "application/json")
        .send_json(json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox_token],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }));

    match response {
        Ok(ok) => {
            let parsed = ok.into_json::<XstsAuthResponse>()?;
            Ok(parsed.token)
        }
        Err(err) => Err(map_http_error(err)),
    }
}

fn authenticate_with_minecraft(
    agent: &ureq::Agent,
    user_hash: &str,
    xsts_token: &str,
) -> Result<String, AuthError> {
    let xtoken = format!("XBL3.0 x={user_hash};{xsts_token}");
    let launcher_response = agent
        .post(MINECRAFT_LOGIN_URL)
        .set("Accept", "application/json")
        .send_json(json!({
            "platform": "PC_LAUNCHER",
            "xtoken": xtoken,
        }));

    match launcher_response {
        Ok(ok) => {
            let parsed = ok.into_json::<MinecraftLoginResponse>()?;
            Ok(parsed.access_token)
        }
        Err(ureq::Error::Status(code, response)) if matches!(code, 400 | 401 | 403 | 404) => {
            let launcher_error = map_http_error(ureq::Error::Status(code, response)).to_string();

            let legacy_response = agent
                .post(MINECRAFT_LOGIN_LEGACY_URL)
                .set("Accept", "application/json")
                .send_json(json!({
                    "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}"),
                }));

            match legacy_response {
                Ok(ok) => {
                    let parsed = ok.into_json::<MinecraftLoginResponse>()?;
                    Ok(parsed.access_token)
                }
                Err(err) => Err(AuthError::Http(format!(
                    "Minecraft token exchange failed on both endpoints. launcher/login error: {launcher_error}; legacy login_with_xbox error: {}",
                    map_http_error(err)
                ))),
            }
        }
        Err(err) => Err(map_http_error(err)),
    }
}

fn fetch_minecraft_entitlements(
    agent: &ureq::Agent,
    minecraft_access_token: &str,
) -> Result<(), AuthError> {
    let response = agent
        .get(MINECRAFT_ENTITLEMENTS_URL)
        .set("Accept", "application/json")
        .set("Authorization", &format!("Bearer {minecraft_access_token}"))
        .call();

    match response {
        Ok(ok) => {
            let _ = ok.into_json::<serde_json::Value>()?;
            Ok(())
        }
        Err(err) => Err(map_http_error(err)),
    }
}

fn fetch_minecraft_profile(
    agent: &ureq::Agent,
    minecraft_access_token: &str,
) -> Result<MinecraftProfileResponse, AuthError> {
    let response = agent
        .get(MINECRAFT_PROFILE_URL)
        .set("Accept", "application/json")
        .set("Authorization", &format!("Bearer {minecraft_access_token}"))
        .call();

    match response {
        Ok(ok) => Ok(ok.into_json::<MinecraftProfileResponse>()?),
        Err(ureq::Error::Status(404, _)) => Err(AuthError::MinecraftProfileUnavailable),
        Err(err) => Err(map_http_error(err)),
    }
}

fn fetch_texture_base64(agent: &ureq::Agent, url: &str) -> Option<String> {
    let response = agent
        .get(url)
        .set("Accept", "image/png,image/*")
        .call()
        .ok()?;

    let mut bytes = Vec::new();
    let mut reader = response.into_reader();
    if reader.read_to_end(&mut bytes).is_err() || bytes.is_empty() {
        return None;
    }

    Some(encode_base64(&bytes))
}

fn generate_avatar_from_profile(profile: &MinecraftProfileState) -> Option<String> {
    let active_skin = profile
        .skins
        .iter()
        .find(|skin| skin.state.eq_ignore_ascii_case("active"))
        .or_else(|| profile.skins.first())?;

    let skin_base64 = active_skin.texture_png_base64.as_deref()?;
    let skin_bytes = decode_base64(skin_base64).ok()?;
    let avatar_png = generate_avatar_png_from_skin(&skin_bytes).ok()?;
    Some(encode_base64(&avatar_png))
}

fn generate_avatar_png_from_skin(skin_png_bytes: &[u8]) -> Result<Vec<u8>, AuthError> {
    let skin = image::load_from_memory(skin_png_bytes)?.to_rgba8();
    let (width, height) = skin.dimensions();

    if width < 64 || height < 16 {
        return Err(AuthError::Image(image::ImageError::IoError(
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Skin texture is smaller than expected",
            ),
        )));
    }

    let mut head = RgbaImage::new(8, 8);

    for y in 0..8 {
        for x in 0..8 {
            let pixel = skin.get_pixel(8 + x, 8 + y);
            head.put_pixel(x, y, *pixel);
        }
    }

    if width >= 48 && height >= 16 {
        for y in 0..8 {
            for x in 0..8 {
                let overlay = skin.get_pixel(40 + x, 8 + y);
                if overlay[3] > 0 {
                    head.put_pixel(x, y, *overlay);
                }
            }
        }
    }

    let upscaled = imageops::resize(&head, 64, 64, imageops::FilterType::Nearest);
    let mut png_out = Vec::new();
    DynamicImage::ImageRgba8(upscaled)
        .write_to(&mut Cursor::new(&mut png_out), ImageFormat::Png)?;
    Ok(png_out)
}

fn oauth_tenant() -> String {
    std::env::var("VERTEX_MSA_TENANT")
        .ok()
        .map(|tenant| tenant.trim().to_owned())
        .filter(|tenant| !tenant.is_empty())
        .unwrap_or_else(|| {
            let builtin = BUILTIN_MICROSOFT_TENANT.trim();
            if builtin.is_empty() {
                "common".to_owned()
            } else {
                builtin.to_owned()
            }
        })
}

fn device_code_url(tenant: &str) -> String {
    format!("{OAUTH_BASE_URL}/{tenant}/oauth2/v2.0/devicecode")
}

fn token_url(tenant: &str) -> String {
    format!("{OAUTH_BASE_URL}/{tenant}/oauth2/v2.0/token")
}

fn account_cache_path() -> PathBuf {
    std::env::var("VERTEX_ACCOUNT_CACHE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(ACCOUNT_CACHE_DEFAULT_PATH))
}

fn write_secure_file_atomic(path: &Path, bytes: &[u8]) -> Result<(), AuthError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let temp_path = path.with_extension("tmp");

    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }

        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }

    #[cfg(windows)]
    {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    fs::rename(temp_path, path)?;
    Ok(())
}

fn map_http_error(error: ureq::Error) -> AuthError {
    match error {
        ureq::Error::Status(code, response) => {
            let mut snippet = String::new();
            let _ = response
                .into_reader()
                .take(1024)
                .read_to_string(&mut snippet);
            if snippet.trim().is_empty() {
                AuthError::Http(format!("HTTP status {code}"))
            } else {
                AuthError::Http(format!("HTTP status {code}: {}", snippet.trim()))
            }
        }
        ureq::Error::Transport(transport) => AuthError::Http(transport.to_string()),
    }
}

fn oauth_error_with_guidance(error: &str, description: &str, tenant: &str) -> AuthError {
    if description.contains("AADSTS9002346")
        || description.contains("configured for use by Microsoft Accounts users only")
    {
        return AuthError::OAuth(format!(
            "{error}: {description}. This app is Microsoft-accounts-only, so use the \
`consumers` endpoint. Set VERTEX_MSA_TENANT=consumers or set \
auth::BUILTIN_MICROSOFT_TENANT=\"consumers\" in crates/auth/src/lib.rs (current tenant: '{tenant}')."
        ));
    }

    if description.contains("AADSTS70002")
        || description.contains("must be marked as 'mobile'")
        || description.contains("not supported for this feature")
    {
        return AuthError::OAuth(format!(
            "{error}: {description}. Device-code flow requires a public/native client. In Azure \
Portal, open your app registration -> Authentication, set 'Allow public client flows' to Yes, \
and add a 'Mobile and desktop applications' platform (native client)."
        ));
    }

    AuthError::OAuth(format!("{error}: {description}"))
}

fn generate_pkce_verifier() -> String {
    generate_random_token(64)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn generate_random_token(length: usize) -> String {
    let mut out = Vec::with_capacity(length);
    while out.len() < length {
        let chunk: [u8; 48] = rand::random();
        let encoded = URL_SAFE_NO_PAD.encode(chunk);
        out.extend_from_slice(encoded.as_bytes());
    }
    String::from_utf8_lossy(&out[..length]).to_string()
}

fn encode_base64(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

fn decode_base64(raw: &str) -> Result<Vec<u8>, AuthError> {
    BASE64_STANDARD
        .decode(raw)
        .map_err(|err| AuthError::OAuth(format!("Base64 decode failed: {err}")))
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default = "default_poll_interval")]
    interval: u64,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct MicrosoftTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxUserAuthResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XboxDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XboxUserHashEntry>,
}

#[derive(Debug, Deserialize)]
struct XboxUserHashEntry {
    #[serde(rename = "uhs")]
    user_hash: String,
}

#[derive(Debug)]
struct XboxUserAuthResult {
    token: String,
    user_hash: String,
}

#[derive(Debug, Deserialize)]
struct XstsAuthResponse {
    #[serde(rename = "Token")]
    token: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftLoginResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<MinecraftSkinResponse>,
    #[serde(default)]
    capes: Vec<MinecraftCapeResponse>,
}

#[derive(Debug, Deserialize)]
struct MinecraftSkinResponse {
    id: String,
    state: String,
    url: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    alias: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MinecraftCapeResponse {
    id: String,
    state: String,
    url: String,
    #[serde(default)]
    alias: Option<String>,
}

const fn default_poll_interval() -> u64 {
    5
}
