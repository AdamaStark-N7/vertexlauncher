use std::sync::{LazyLock, Mutex};
#[cfg(target_os = "windows")]
use std::{thread, time::Duration};

use keyring::{Entry, Error as KeyringError};

use crate::error::AuthError;

const ACCOUNTS_STATE_SERVICE: &str = "vertexlauncher.accounts_state.v1";
const ACCOUNTS_STATE_ACCOUNT: &str = "cached_accounts";
const REFRESH_TOKEN_SERVICE: &str = "vertexlauncher.microsoft_refresh_token.v2";
const LEGACY_REFRESH_TOKEN_SERVICE: &str = "vertexlauncher.microsoft_refresh_token";
#[cfg(target_os = "windows")]
const WINDOWS_REFRESH_TOKEN_VERIFY_ATTEMPTS: usize = 3;
#[cfg(target_os = "windows")]
const WINDOWS_REFRESH_TOKEN_VERIFY_RETRY_DELAY: Duration = Duration::from_millis(15);

static SECURE_STORE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn accounts_state_entry() -> Result<Entry, AuthError> {
    Entry::new(ACCOUNTS_STATE_SERVICE, ACCOUNTS_STATE_ACCOUNT).map_err(|err| {
        AuthError::SecureStorage(format!(
            "Failed to open secure storage entry for cached accounts state: {err}",
        ))
    })
}

fn refresh_token_entry(service: &str, profile_id: &str) -> Result<Entry, AuthError> {
    Entry::new(service, profile_id).map_err(|err| {
        AuthError::SecureStorage(format!(
            "Failed to open refresh-token secure storage entry for profile '{profile_id}': {err}",
        ))
    })
}

pub(crate) fn load_accounts_state() -> Result<Option<String>, AuthError> {
    with_secure_store_lock(load_accounts_state_unlocked)
}

fn load_accounts_state_unlocked() -> Result<Option<String>, AuthError> {
    let entry = accounts_state_entry()?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                error = %err,
                "secure storage unavailable while loading cached accounts state; using empty cache"
            );
            Ok(None)
        }
        Err(err) if is_corrupt_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                error = %err,
                "ignoring unreadable cached accounts state entry"
            );
            let _ = delete_accounts_state_unlocked();
            Ok(None)
        }
        Err(err) => Err(AuthError::SecureStorage(format!(
            "Failed to load cached accounts state from secure storage: {err}",
        ))),
    }
}

pub(crate) fn delete_accounts_state() -> Result<(), AuthError> {
    with_secure_store_lock(delete_accounts_state_unlocked)
}

fn delete_accounts_state_unlocked() -> Result<(), AuthError> {
    let entry = accounts_state_entry()?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                error = %err,
                "secure storage unavailable while deleting cached accounts state; keeping in-memory state cleared"
            );
            Ok(())
        }
        Err(err) => Err(AuthError::SecureStorage(format!(
            "Failed to delete cached accounts state from secure storage: {err}",
        ))),
    }
}

pub(crate) fn load_refresh_token(profile_id: &str) -> Result<Option<String>, AuthError> {
    with_secure_store_lock(|| load_refresh_token_unlocked(profile_id))
}

fn load_refresh_token_unlocked(profile_id: &str) -> Result<Option<String>, AuthError> {
    let entry = refresh_token_entry(REFRESH_TOKEN_SERVICE, profile_id)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => load_legacy_refresh_token_unlocked(profile_id),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "secure storage unavailable while loading refresh token; continuing without a persisted token"
            );
            Ok(None)
        }
        Err(err) if is_corrupt_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "ignoring unreadable refresh-token entry"
            );
            let _ = delete_refresh_token_for_service_unlocked(REFRESH_TOKEN_SERVICE, profile_id);
            load_legacy_refresh_token_unlocked(profile_id)
        }
        Err(err) => Err(AuthError::SecureStorage(format!(
            "Failed to load refresh token for profile '{profile_id}': {err}",
        ))),
    }
}

pub(crate) fn store_refresh_token(profile_id: &str, refresh_token: &str) -> Result<(), AuthError> {
    with_secure_store_lock(|| store_refresh_token_unlocked(profile_id, refresh_token))
}

fn store_refresh_token_unlocked(profile_id: &str, refresh_token: &str) -> Result<(), AuthError> {
    let entry = refresh_token_entry(REFRESH_TOKEN_SERVICE, profile_id)?;
    entry.set_password(refresh_token).map_err(|err| {
        AuthError::SecureStorage(format!(
            "Failed to store refresh token for profile '{profile_id}': {err}",
        ))
    })?;
    verify_refresh_token_round_trip_unlocked(profile_id, refresh_token)?;
    let _ = delete_refresh_token_for_service_unlocked(LEGACY_REFRESH_TOKEN_SERVICE, profile_id);
    Ok(())
}

pub(crate) fn delete_refresh_token(profile_id: &str) -> Result<(), AuthError> {
    with_secure_store_lock(|| {
        delete_refresh_token_for_service_unlocked(REFRESH_TOKEN_SERVICE, profile_id)?;
        delete_refresh_token_for_service_unlocked(LEGACY_REFRESH_TOKEN_SERVICE, profile_id)?;
        Ok(())
    })
}

fn load_legacy_refresh_token_unlocked(profile_id: &str) -> Result<Option<String>, AuthError> {
    let legacy_entry = refresh_token_entry(LEGACY_REFRESH_TOKEN_SERVICE, profile_id)?;
    match legacy_entry.get_password() {
        Ok(value) => {
            store_refresh_token_unlocked(profile_id, &value)?;
            Ok(Some(value))
        }
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "secure storage unavailable while loading legacy refresh token; continuing without a persisted token"
            );
            Ok(None)
        }
        Err(err) if is_corrupt_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "ignoring unreadable legacy refresh-token entry"
            );
            let _ =
                delete_refresh_token_for_service_unlocked(LEGACY_REFRESH_TOKEN_SERVICE, profile_id);
            Ok(None)
        }
        Err(err) => Err(AuthError::SecureStorage(format!(
            "Failed to load refresh token for profile '{profile_id}' from legacy secure storage: {err}",
        ))),
    }
}

fn delete_refresh_token_for_service_unlocked(
    service: &str,
    profile_id: &str,
) -> Result<(), AuthError> {
    let entry = refresh_token_entry(service, profile_id)?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "secure storage unavailable while deleting refresh token; keeping in-memory state cleared"
            );
            Ok(())
        }
        Err(err) => Err(AuthError::SecureStorage(format!(
            "Failed to delete refresh token for profile '{profile_id}': {err}",
        ))),
    }
}

fn verify_refresh_token_round_trip_unlocked(
    profile_id: &str,
    refresh_token: &str,
) -> Result<(), AuthError> {
    for attempt in 0..refresh_token_verify_attempts() {
        let entry = refresh_token_entry(REFRESH_TOKEN_SERVICE, profile_id)?;
        match entry.get_password() {
            Ok(stored) if stored == refresh_token => return Ok(()),
            Ok(_) | Err(KeyringError::NoEntry) if attempt + 1 < refresh_token_verify_attempts() => {
                sleep_before_refresh_token_retry();
            }
            Err(err)
                if attempt + 1 < refresh_token_verify_attempts()
                    && should_retry_refresh_token_verification(&err) =>
            {
                sleep_before_refresh_token_retry();
            }
            Err(err) => {
                return Err(AuthError::SecureStorage(format!(
                    "Failed to verify refresh token for profile '{profile_id}' after writing it to secure storage: {err}",
                )));
            }
            Ok(_) => {
                return Err(AuthError::SecureStorage(format!(
                    "Refresh token for profile '{profile_id}' did not round-trip correctly through secure storage."
                )));
            }
        }
    }

    Err(AuthError::SecureStorage(format!(
        "Refresh token for profile '{profile_id}' did not round-trip correctly through secure storage."
    )))
}

fn with_secure_store_lock<T>(
    operation: impl FnOnce() -> Result<T, AuthError>,
) -> Result<T, AuthError> {
    let _guard = SECURE_STORE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    operation()
}

fn is_corrupt_secure_storage_error(err: &KeyringError) -> bool {
    let error_text = err.to_string();
    matches!(err, KeyringError::BadEncoding(_))
        || error_text.contains("Crypto error")
        || error_text.contains("Unpad Error")
}

#[cfg(target_os = "windows")]
fn is_unavailable_secure_storage_error(err: &KeyringError) -> bool {
    matches!(err, KeyringError::NoStorageAccess(_))
}

#[cfg(not(target_os = "windows"))]
fn is_unavailable_secure_storage_error(_err: &KeyringError) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn refresh_token_verify_attempts() -> usize {
    WINDOWS_REFRESH_TOKEN_VERIFY_ATTEMPTS
}

#[cfg(not(target_os = "windows"))]
fn refresh_token_verify_attempts() -> usize {
    1
}

#[cfg(target_os = "windows")]
fn should_retry_refresh_token_verification(err: &KeyringError) -> bool {
    matches!(err, KeyringError::PlatformFailure(_))
}

#[cfg(not(target_os = "windows"))]
fn should_retry_refresh_token_verification(_err: &KeyringError) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn sleep_before_refresh_token_retry() {
    thread::sleep(WINDOWS_REFRESH_TOKEN_VERIFY_RETRY_DELAY);
}

#[cfg(not(target_os = "windows"))]
fn sleep_before_refresh_token_retry() {}
