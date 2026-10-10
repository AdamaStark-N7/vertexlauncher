use super::*;

fn emit_auth_flow_event(
    sender: &mpsc::Sender<AuthFlowEvent>,
    target: &'static str,
    event_name: &'static str,
    started_at: std::time::Instant,
    event: AuthFlowEvent,
) -> bool {
    if let Err(err) = sender.send(event) {
        match target {
            "vertexlauncher/auth/signin" => tracing::error!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                event = event_name,
                error = %err,
                "Failed to deliver auth flow event to UI."
            ),
            "vertexlauncher/auth/signin/browser" => tracing::error!(
                target: "vertexlauncher/auth/signin/browser",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                event = event_name,
                error = %err,
                "Failed to deliver auth flow event to UI."
            ),
            _ => tracing::error!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                event = event_name,
                error = %err,
                requested_target = target,
                "Failed to deliver auth flow event to UI."
            ),
        };
        return false;
    }
    true
}

pub(super) fn run_sign_in_flow(client_id: String, sender: mpsc::Sender<AuthFlowEvent>) {
    let started_at = std::time::Instant::now();
    tracing::info!(
        target: "vertexlauncher/auth/signin",
        "Starting Microsoft sign-in flow."
    );

    let flow = match auth::login_begin(client_id) {
        Ok(flow) => flow,
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin",
                error = %webview_sign_in::sanitize_message_for_log(&err.to_string()),
                "Failed to initialize Microsoft sign-in flow."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin",
                "failed:init",
                started_at,
                AuthFlowEvent::Failed(err.to_string()),
            );
            return;
        }
    };

    tracing::info!(
        target: "vertexlauncher/auth/signin",
        auth_url = %webview_sign_in::sanitize_url_for_log(&flow.auth_request_uri),
        redirect_url = %webview_sign_in::sanitize_url_for_log(&flow.redirect_uri),
        expected_state = %webview_sign_in::fingerprint_for_log(flow.expected_state()),
        "Microsoft sign-in flow initialized; opening embedded webview."
    );

    if !emit_auth_flow_event(
        &sender,
        "vertexlauncher/auth/signin",
        "awaiting_browser",
        started_at,
        AuthFlowEvent::AwaitingBrowser,
    ) {
        return;
    }

    let auth_code = match webview_sign_in::open_microsoft_sign_in(
        &flow.auth_request_uri,
        &flow.redirect_uri,
        flow.expected_state(),
    ) {
        Ok(auth_code) => auth_code,
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error = %webview_sign_in::sanitize_message_for_log(&err),
                "Microsoft sign-in webview failed before returning a callback URL."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin",
                "failed:webview",
                started_at,
                AuthFlowEvent::Failed(err),
            );
            return;
        }
    };

    tracing::info!(
        target: "vertexlauncher/auth/signin",
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "Microsoft sign-in webview returned an auth code; exchanging tokens."
    );

    if !emit_auth_flow_event(
        &sender,
        "vertexlauncher/auth/signin",
        "waiting_for_authorization",
        started_at,
        AuthFlowEvent::WaitingForAuthorization,
    ) {
        return;
    }

    match auth::login_finish(&auth_code, flow) {
        Ok(account) => {
            tracing::info!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "Microsoft sign-in flow completed successfully."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin",
                "completed",
                started_at,
                AuthFlowEvent::Completed(account),
            );
        }
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error = %webview_sign_in::sanitize_message_for_log(&err.to_string()),
                "Microsoft sign-in callback exchange failed."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin",
                "failed:exchange",
                started_at,
                AuthFlowEvent::Failed(err.to_string()),
            );
        }
    }
}

pub(super) fn run_system_browser_sign_in_flow(
    sender: mpsc::Sender<AuthFlowEvent>,
    colors: system_browser_sign_in::CallbackPageColors,
) {
    let started_at = std::time::Instant::now();
    tracing::info!(
        target: "vertexlauncher/auth/signin/browser",
        "Starting Microsoft system-browser sign-in flow."
    );

    let callback_listener = match system_browser_sign_in::prepare_loopback_callback_listener() {
        Ok(callback_listener) => callback_listener,
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin/browser",
                error = %webview_sign_in::sanitize_message_for_log(&err),
                "Failed to allocate localhost loopback redirect URI."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin/browser",
                "failed:prepare_listener",
                started_at,
                AuthFlowEvent::Failed(err),
            );
            return;
        }
    };
    let flow = match auth::login_begin_with_device_code_client_redirect_uri(
        callback_listener.redirect_uri().to_owned(),
    ) {
        Ok(flow) => flow,
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin/browser",
                error = %webview_sign_in::sanitize_message_for_log(&err.to_string()),
                "Failed to initialize Microsoft system-browser sign-in flow."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin/browser",
                "failed:init",
                started_at,
                AuthFlowEvent::Failed(err.to_string()),
            );
            return;
        }
    };

    tracing::info!(
        target: "vertexlauncher/auth/signin/browser",
        auth_url = %webview_sign_in::sanitize_url_for_log(&flow.auth_request_uri),
        redirect_url = %webview_sign_in::sanitize_url_for_log(&flow.redirect_uri),
        expected_state = %webview_sign_in::fingerprint_for_log(flow.expected_state()),
        "Microsoft system-browser sign-in flow initialized; opening default browser."
    );

    if !emit_auth_flow_event(
        &sender,
        "vertexlauncher/auth/signin/browser",
        "awaiting_external_browser",
        started_at,
        AuthFlowEvent::AwaitingExternalBrowser,
    ) {
        return;
    }

    let callback_url = match system_browser_sign_in::open_microsoft_sign_in(
        &flow.auth_request_uri,
        callback_listener,
        &colors,
    ) {
        Ok(callback_url) => callback_url,
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin/browser",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error = %webview_sign_in::sanitize_message_for_log(&err),
                "Microsoft system-browser sign-in failed before returning a callback URL."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin/browser",
                "failed:browser",
                started_at,
                AuthFlowEvent::Failed(err),
            );
            return;
        }
    };

    if !emit_auth_flow_event(
        &sender,
        "vertexlauncher/auth/signin/browser",
        "waiting_for_authorization",
        started_at,
        AuthFlowEvent::WaitingForAuthorization,
    ) {
        return;
    }

    finish_browser_sign_in_flow(
        flow,
        callback_url,
        sender,
        started_at,
        "Microsoft system-browser callback received; exchanging tokens.",
    );
}

fn finish_browser_sign_in_flow(
    flow: auth::MinecraftLoginFlow,
    callback_url: String,
    sender: mpsc::Sender<AuthFlowEvent>,
    started_at: std::time::Instant,
    callback_message: &str,
) {
    tracing::info!(
        target: "vertexlauncher/auth/signin",
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        callback_url = %webview_sign_in::sanitize_url_for_log(&callback_url),
        "{}",
        callback_message
    );

    match auth::login_finish_from_redirect(&callback_url, flow) {
        Ok(account) => {
            tracing::info!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "Microsoft sign-in flow completed successfully."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin",
                "completed",
                started_at,
                AuthFlowEvent::Completed(account),
            );
        }
        Err(err) => {
            tracing::error!(
                target: "vertexlauncher/auth/signin",
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error = %webview_sign_in::sanitize_message_for_log(&err.to_string()),
                "Microsoft sign-in callback exchange failed."
            );
            let _ = emit_auth_flow_event(
                &sender,
                "vertexlauncher/auth/signin",
                "failed:exchange",
                started_at,
                AuthFlowEvent::Failed(err.to_string()),
            );
        }
    }
}
