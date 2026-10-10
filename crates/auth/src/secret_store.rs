use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use keyring::{Entry, Error as KeyringError};

use crate::constants::{
    ACCOUNTS_STATE_ACCOUNT, ACCOUNTS_STATE_SERVICE, LEGACY_REFRESH_TOKEN_SERVICE,
    REFRESH_TOKEN_SERVICE, REFRESH_TOKEN_STORE_ATTEMPTS, REFRESH_TOKEN_VERIFY_ATTEMPTS,
    SECURE_STORE_RETRY_ATTEMPTS, SECURE_STORE_RETRY_DELAY,
};
use crate::error::AuthError;

static SECURE_STORE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static INSECURE_FILE_STORAGE: AtomicBool = AtomicBool::new(false);

/// Switches refresh-token storage between the OS keyring (default) and a plain-text file next
/// to the account cache. The file is NOT encrypted: anyone who can read it can sign in as the
/// account. It exists for systems where the keyring is unusable, such as a Steam Deck.
///
/// Tokens move between the two stores as they are next read or written, so flipping this does
/// not sign anyone out.
pub fn set_insecure_file_storage(enabled: bool) {
    INSECURE_FILE_STORAGE.store(enabled, Ordering::Relaxed);
}

fn insecure_file_storage_enabled() -> bool {
    INSECURE_FILE_STORAGE.load(Ordering::Relaxed)
}

/// Where refresh tokens are kept while insecure file storage is in use.
pub fn insecure_token_file_path() -> PathBuf {
    crate::cache::account_cache_path().with_file_name("refresh_tokens.json")
}

/// Plain-text refresh-token file, keyed by Minecraft profile id.
mod token_file {
    use super::*;

    fn read_all() -> BTreeMap<String, String> {
        let path = insecure_token_file_path();
        match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|err| {
                tracing::warn!(
                    target: "vertexlauncher/auth/secret_store",
                    path = %path.display(),
                    error = %err,
                    "ignoring unreadable refresh-token file"
                );
                BTreeMap::new()
            }),
            Err(_) => BTreeMap::new(),
        }
    }

    fn write_all(tokens: &BTreeMap<String, String>) -> Result<(), AuthError> {
        let path = insecure_token_file_path();
        if tokens.is_empty() {
            return match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(file_error(&path, "remove", err)),
            };
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|err| file_error(parent, "create", err))?;
        }
        let json = serde_json::to_string(tokens)?;
        // Write beside the target and rename, so a crash never leaves a half-written file.
        let temp = path.with_extension("json.tmp");
        write_private(&temp, json.as_bytes()).map_err(|err| file_error(&temp, "write", err))?;
        fs::rename(&temp, &path).map_err(|err| file_error(&path, "replace", err))
    }

    #[cfg(unix)]
    fn write_private(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents)
    }

    #[cfg(not(unix))]
    fn write_private(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
        fs::write(path, contents)
    }

    fn file_error(path: &std::path::Path, action: &str, err: std::io::Error) -> AuthError {
        AuthError::SecureStorage(format!(
            "Failed to {action} the refresh-token file {}: {err}",
            path.display()
        ))
    }

    pub(super) fn load(profile_id: &str) -> Option<String> {
        read_all()
            .remove(profile_id)
            .filter(|token| !token.is_empty())
    }

    pub(super) fn store(profile_id: &str, token: &str) -> Result<(), AuthError> {
        let mut tokens = read_all();
        tokens.insert(profile_id.to_owned(), token.to_owned());
        write_all(&tokens)
    }

    pub(super) fn remove(profile_id: &str) -> Result<(), AuthError> {
        let mut tokens = read_all();
        if tokens.remove(profile_id).is_some() {
            write_all(&tokens)?;
        }
        Ok(())
    }
}

pub(crate) enum RefreshTokenLoadResult {
    Present(String),
    Missing,
    Unavailable,
}

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
    match retry_keyring_operation(|| entry.get_password()) {
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
    match retry_keyring_operation(|| entry.delete_credential()) {
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

pub(crate) fn load_refresh_token(profile_id: &str) -> Result<RefreshTokenLoadResult, AuthError> {
    with_secure_store_lock(|| {
        // The file wins over the keyring: it is only ever written while insecure storage is on,
        // so it holds the newest token, and the keyring copy may predate a rotation.
        if let Some(token) = token_file::load(profile_id) {
            if !insecure_file_storage_enabled()
                && store_refresh_token_unlocked(profile_id, &token).is_ok()
            {
                // Insecure storage was switched off and the keyring took the token back.
                let _ = token_file::remove(profile_id);
            }
            return Ok(RefreshTokenLoadResult::Present(token));
        }

        let loaded = load_refresh_token_unlocked(profile_id)?;
        if insecure_file_storage_enabled()
            && let RefreshTokenLoadResult::Present(token) = &loaded
        {
            // Insecure storage was just switched on: carry the keyring token over.
            let _ = token_file::store(profile_id, token);
        }
        Ok(loaded)
    })
}

fn load_refresh_token_unlocked(profile_id: &str) -> Result<RefreshTokenLoadResult, AuthError> {
    let entry = refresh_token_entry(REFRESH_TOKEN_SERVICE, profile_id)?;
    match retry_keyring_operation(|| entry.get_password()) {
        Ok(value) => Ok(RefreshTokenLoadResult::Present(value)),
        Err(KeyringError::NoEntry) => load_legacy_refresh_token_unlocked(profile_id),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "secure storage unavailable while loading refresh token; continuing without a persisted token"
            );
            Ok(RefreshTokenLoadResult::Unavailable)
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
    with_secure_store_lock(|| {
        if insecure_file_storage_enabled() {
            return token_file::store(profile_id, refresh_token);
        }
        store_refresh_token_unlocked(profile_id, refresh_token)?;
        // Anything left in the file from insecure mode is now stale.
        let _ = token_file::remove(profile_id);
        Ok(())
    })
}

fn store_refresh_token_unlocked(profile_id: &str, refresh_token: &str) -> Result<(), AuthError> {
    for attempt in 0..REFRESH_TOKEN_STORE_ATTEMPTS {
        let _ = delete_refresh_token_for_service_unlocked(REFRESH_TOKEN_SERVICE, profile_id);
        let entry = refresh_token_entry(REFRESH_TOKEN_SERVICE, profile_id)?;
        retry_keyring_operation(|| entry.set_password(refresh_token)).map_err(|err| {
            AuthError::SecureStorage(format!(
                "Failed to store refresh token for profile '{profile_id}': {err}",
            ))
        })?;

        match verify_refresh_token_round_trip_unlocked(profile_id, refresh_token) {
            Ok(()) => {
                let _ = delete_refresh_token_for_service_unlocked(
                    LEGACY_REFRESH_TOKEN_SERVICE,
                    profile_id,
                );
                return Ok(());
            }
            Err(AuthError::SecureStorage(message))
                if attempt + 1 < REFRESH_TOKEN_STORE_ATTEMPTS
                    && is_corrupt_secure_storage_message(&message) =>
            {
                tracing::warn!(
                    target: "vertexlauncher/auth/secret_store",
                    profile_id,
                    attempt = attempt + 1,
                    "refresh-token verification hit corrupt secure storage data after write; deleting entry and retrying"
                );
                let _ =
                    delete_refresh_token_for_service_unlocked(REFRESH_TOKEN_SERVICE, profile_id);
                thread::sleep(SECURE_STORE_RETRY_DELAY);
            }
            Err(err) => return Err(err),
        }
    }

    Err(AuthError::SecureStorage(format!(
        "Failed to store refresh token for profile '{profile_id}' after repeated secure storage rewrite attempts."
    )))
}

pub(crate) fn delete_refresh_token(profile_id: &str) -> Result<(), AuthError> {
    with_secure_store_lock(|| {
        token_file::remove(profile_id)?;
        delete_refresh_token_for_service_unlocked(REFRESH_TOKEN_SERVICE, profile_id)?;
        delete_refresh_token_for_service_unlocked(LEGACY_REFRESH_TOKEN_SERVICE, profile_id)?;
        Ok(())
    })
}

fn load_legacy_refresh_token_unlocked(
    profile_id: &str,
) -> Result<RefreshTokenLoadResult, AuthError> {
    let legacy_entry = refresh_token_entry(LEGACY_REFRESH_TOKEN_SERVICE, profile_id)?;
    match retry_keyring_operation(|| legacy_entry.get_password()) {
        Ok(value) => {
            store_refresh_token_unlocked(profile_id, &value)?;
            Ok(RefreshTokenLoadResult::Present(value))
        }
        Err(KeyringError::NoEntry) => Ok(RefreshTokenLoadResult::Missing),
        Err(err) if is_unavailable_secure_storage_error(&err) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                profile_id,
                error = %err,
                "secure storage unavailable while loading legacy refresh token; continuing without a persisted token"
            );
            Ok(RefreshTokenLoadResult::Unavailable)
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
            Ok(RefreshTokenLoadResult::Missing)
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
    match retry_keyring_operation(|| entry.delete_credential()) {
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
        match retry_keyring_operation(|| entry.get_password()) {
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
    let _guard = match SECURE_STORE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!(
                target: "vertexlauncher/auth/secret_store",
                "secure storage lock was poisoned; recovering lock state"
            );
            poisoned.into_inner()
        }
    };
    operation()
}

fn retry_keyring_operation<T>(
    mut operation: impl FnMut() -> Result<T, KeyringError>,
) -> Result<T, KeyringError> {
    let retry_attempts = SECURE_STORE_RETRY_ATTEMPTS.max(1);
    let mut last_err = None;
    for attempt in 0..retry_attempts {
        match operation() {
            Ok(value) => return Ok(value),
            Err(err)
                if attempt + 1 < retry_attempts && should_retry_secure_storage_operation(&err) =>
            {
                last_err = Some(err);
                thread::sleep(SECURE_STORE_RETRY_DELAY);
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_err.unwrap_or(KeyringError::NoEntry))
}

fn is_corrupt_secure_storage_error(err: &KeyringError) -> bool {
    let error_text = err.to_string();
    matches!(err, KeyringError::BadEncoding(_)) || is_corrupt_secure_storage_message(&error_text)
}

fn is_corrupt_secure_storage_message(error_text: &str) -> bool {
    error_text.contains("Crypto error") || error_text.contains("Unpad Error")
}

fn is_unavailable_secure_storage_error(err: &KeyringError) -> bool {
    matches!(err, KeyringError::NoStorageAccess(_)) || is_retryable_platform_storage_error(err)
}

fn is_retryable_platform_storage_error(err: &KeyringError) -> bool {
    let KeyringError::PlatformFailure(inner) = err else {
        return false;
    };

    let lowered = inner.to_string().to_ascii_lowercase();
    lowered.contains("dbus error")
        || lowered.contains("can't find session")
        || lowered.contains("secret service")
        || lowered.contains("org.freedesktop.secrets")
        || lowered.contains("no such object path")
        || lowered.contains("no such interface")
        || lowered.contains("keychain")
        || lowered.contains("errsecnotavailable")
        || lowered.contains("errsecreadonly")
        || lowered.contains("errsecnosuchkeychain")
        || lowered.contains("errsecinvalidkeychain")
        || lowered.contains("windows error_no_such_logon_session")
}

fn refresh_token_verify_attempts() -> usize {
    REFRESH_TOKEN_VERIFY_ATTEMPTS
}

fn should_retry_secure_storage_operation(err: &KeyringError) -> bool {
    is_unavailable_secure_storage_error(err)
}

fn should_retry_refresh_token_verification(err: &KeyringError) -> bool {
    should_retry_secure_storage_operation(err)
}

fn sleep_before_refresh_token_retry() {
    thread::sleep(SECURE_STORE_RETRY_DELAY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insecure_file_storage_round_trips_and_deletes_tokens() {
        let dir =
            std::env::temp_dir().join(format!("vertex-insecure-token-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // SAFETY: no other test in this crate reads or writes this variable.
        unsafe { std::env::set_var("VERTEX_ACCOUNT_CACHE_PATH", dir.join("accounts.json")) };
        set_insecure_file_storage(true);

        store_refresh_token("profile-a", "token-a").unwrap();
        store_refresh_token("profile-b", "token-b").unwrap();
        assert!(insecure_token_file_path().starts_with(&dir));
        assert!(matches!(
            load_refresh_token("profile-a").unwrap(),
            RefreshTokenLoadResult::Present(token) if token == "token-a"
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(insecure_token_file_path())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(
                mode & 0o077,
                0,
                "token file must not be group/world accessible"
            );
        }

        token_file::remove("profile-a").unwrap();
        assert!(token_file::load("profile-a").is_none());
        assert_eq!(token_file::load("profile-b").as_deref(), Some("token-b"));
        token_file::remove("profile-b").unwrap();
        assert!(!insecure_token_file_path().exists());

        set_insecure_file_storage(false);
        let _ = fs::remove_dir_all(&dir);
    }
}
