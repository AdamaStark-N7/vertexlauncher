use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Read};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::constants::{AUTH_REQUEST_MIN_INTERVAL, MAX_LOG_MESSAGE_CHARS};

/// Sanitizes an error message for logging (URLs, bearer tokens and secrets removed).
pub(crate) fn sanitize_message_for_log(value: &str) -> String {
    log_redaction::sanitize_message_for_log(value, MAX_LOG_MESSAGE_CHARS)
}
use crate::error::AuthError;

fn auth_request_gate() -> &'static Mutex<Option<Instant>> {
    static GATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn build_http_agent() -> ureq::Agent {
    ureq::Agent::new_with_defaults()
}

pub(crate) fn wait_for_auth_request_slot(operation: &str) {
    let mut gate = match auth_request_gate().lock() {
        Ok(gate) => gate,
        Err(poisoned) => {
            tracing::warn!(
                target: "vertexlauncher/auth/rate_limit",
                operation,
                "auth request gate mutex was poisoned; recovering lock state"
            );
            poisoned.into_inner()
        }
    };
    let now = Instant::now();

    if let Some(next_allowed_at) = *gate {
        if now < next_allowed_at {
            let delay = next_allowed_at.saturating_duration_since(now);
            tracing::debug!(
                target: "vertexlauncher/auth/rate_limit",
                operation,
                delay_ms = delay.as_millis(),
                "delaying auth request to avoid upstream rate limits"
            );
            thread::sleep(delay);
        }
    }

    *gate = Some(Instant::now() + AUTH_REQUEST_MIN_INTERVAL);
}

pub(crate) fn generate_pkce_verifier() -> String {
    generate_random_token(64)
}

pub(crate) fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub(crate) fn generate_random_token(length: usize) -> String {
    let mut out = String::with_capacity(length + 64);
    let mut rng = rand::thread_rng();
    while out.len() < length {
        let mut chunk = [0_u8; 48];
        rng.fill_bytes(&mut chunk);
        out.push_str(&URL_SAFE_NO_PAD.encode(chunk));
    }
    out.truncate(length);
    out
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

pub(crate) fn fingerprint_for_log(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn decode_base64(raw: &str) -> Result<Vec<u8>, AuthError> {
    BASE64_STANDARD
        .decode(raw)
        .map_err(|err| AuthError::OAuth(format!("Base64 decode failed: {err}")))
}

pub(crate) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) trait UreqResponseExt {
    fn into_json<T: DeserializeOwned>(self) -> Result<T, AuthError>;
    fn into_string(self) -> Result<String, io::Error>;
}

impl UreqResponseExt for ureq::http::Response<ureq::Body> {
    fn into_json<T: DeserializeOwned>(mut self) -> Result<T, AuthError> {
        self.body_mut()
            .read_json::<T>()
            .map_err(|err| AuthError::Http(err.to_string()))
    }

    fn into_string(mut self) -> Result<String, io::Error> {
        let mut raw = String::new();
        self.body_mut().as_reader().read_to_string(&mut raw)?;
        Ok(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::{AUTH_REQUEST_MIN_INTERVAL, sanitize_message_for_log};

    #[test]
    fn auth_request_min_interval_is_nonzero() {
        assert!(AUTH_REQUEST_MIN_INTERVAL > std::time::Duration::ZERO);
    }

    #[test]
    fn sanitize_message_for_log_redacts_urls_and_tokens() {
        let actual = sanitize_message_for_log(
            r#"failed callback https://login.live.com/oauth20_desktop.srf?code=abc&state=secret refresh_token=refresh Bearer access"#,
        );
        assert_eq!(
            actual,
            "failed callback https://login.live.com/oauth20_desktop.srf?params=code,state refresh_token=[redacted] Bearer [redacted]"
        );
    }
}
