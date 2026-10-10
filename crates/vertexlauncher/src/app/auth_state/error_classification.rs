pub(super) fn is_nonfatal_account_cache_error(error_text: &str) -> bool {
    let lowered = error_text.to_ascii_lowercase();
    let mentions_secure_storage = lowered.contains("secure storage")
        || lowered.contains("platform secure storage")
        || lowered.contains("couldn't access platform secure storage")
        || lowered.contains("secret service")
        || lowered.contains("org/freedesktop/secrets");
    let likely_session_bus_issue = lowered.contains("can't find session")
        || lowered.contains("dbus error")
        || lowered.contains("no such object path")
        || lowered.contains("no such interface")
        || lowered.contains("windows error_no_such_logon_session");
    mentions_secure_storage && likely_session_bus_issue
}

pub(super) fn is_http_auth_error(error_text: &str) -> bool {
    let lowered = error_text.to_ascii_lowercase();
    lowered.contains("http status")
        || lowered.contains("http request failed")
        || lowered.contains("transport")
        || lowered.contains("connection")
}
