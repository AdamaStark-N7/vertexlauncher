use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

use crate::error::AuthError;

pub(crate) fn build_http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .timeout_write(Duration::from_secs(20))
        .build()
}

pub(crate) fn generate_pkce_verifier() -> String {
    generate_random_token(64)
}

pub(crate) fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub(crate) fn generate_random_token(length: usize) -> String {
    let mut out = Vec::with_capacity(length);
    while out.len() < length {
        let chunk: [u8; 48] = rand::random();
        let encoded = URL_SAFE_NO_PAD.encode(chunk);
        out.extend_from_slice(encoded.as_bytes());
    }
    String::from_utf8_lossy(&out[..length]).to_string()
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
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
