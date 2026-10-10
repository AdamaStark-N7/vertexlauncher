use super::*;

pub(crate) fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, InstallationError> {
    let raw = get_text(url)?;
    Ok(serde_json::from_str(&raw)?)
}

pub(crate) fn get_json_with_user_agent<T: DeserializeOwned>(
    url: &str,
    user_agent: &str,
) -> Result<T, InstallationError> {
    let raw = call_get_with_retry(url, user_agent)?;
    Ok(serde_json::from_str(&raw)?)
}

pub(crate) fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(HTTP_TIMEOUT_GLOBAL))
            .timeout_connect(Some(HTTP_TIMEOUT_CONNECT))
            .timeout_recv_response(Some(HTTP_TIMEOUT_RECV_RESPONSE))
            .timeout_recv_body(Some(HTTP_TIMEOUT_RECV_BODY))
            .build();
        ureq::Agent::new_with_config(config)
    })
}

pub(crate) fn get_text(url: &str) -> Result<String, InstallationError> {
    call_get_with_retry(url, DEFAULT_USER_AGENT)
}

pub(crate) fn call_get_response_with_retry(
    url: &str,
    user_agent: &str,
) -> Result<ureq::http::Response<ureq::Body>, InstallationError> {
    let mut last_err = None;
    for attempt in 1..=HTTP_RETRY_ATTEMPTS {
        tracing::trace!(
            target: "vertexlauncher/installation/http",
            url,
            user_agent,
            attempt,
            max_attempts = HTTP_RETRY_ATTEMPTS,
            "Sending HTTP GET request."
        );
        match http_agent()
            .get(url)
            .header("User-Agent", user_agent)
            .config()
            .http_status_as_error(false)
            .build()
            .call()
        {
            Ok(mut response) => {
                let status = response.status().as_u16();
                tracing::trace!(
                    target: "vertexlauncher/installation/http",
                    url,
                    attempt,
                    status,
                    "HTTP GET request completed."
                );
                if status < 400 {
                    return Ok(response);
                }
                let mut body = String::new();
                let _ = response.body_mut().as_reader().read_to_string(&mut body);
                let err = InstallationError::HttpStatus {
                    url: url.to_owned(),
                    status,
                    body,
                };
                let retryable = should_retry_http_status(status);
                if !retryable || attempt >= HTTP_RETRY_ATTEMPTS {
                    return Err(err);
                }
                last_err = Some(err);
            }
            Err(err) => {
                tracing::warn!(
                    target: "vertexlauncher/installation/http",
                    url,
                    attempt,
                    error = %err,
                    "HTTP GET request failed before a valid response was received."
                );
                let mapped = InstallationError::Transport {
                    url: url.to_owned(),
                    message: err.to_string(),
                };
                if attempt >= HTTP_RETRY_ATTEMPTS {
                    return Err(mapped);
                }
                last_err = Some(mapped);
            }
        }

        let delay = retry_delay_for_attempt(attempt);
        tracing::warn!(
            target: "vertexlauncher/installation/downloads",
            "Request retry {}/{} for {} after {:?}: {}",
            attempt,
            HTTP_RETRY_ATTEMPTS,
            url,
            delay,
            last_err
                .as_ref()
                .map_or_else(|| "request failed".to_owned(), ToString::to_string)
        );
        thread::sleep(delay);
    }

    Err(last_err.unwrap_or_else(|| InstallationError::Transport {
        url: url.to_owned(),
        message: "request failed without detailed error".to_owned(),
    }))
}

pub(crate) fn call_get_with_retry(
    url: &str,
    user_agent: &str,
) -> Result<String, InstallationError> {
    let mut response = call_get_response_with_retry(url, user_agent)?;
    let mut raw = String::new();
    response
        .body_mut()
        .as_reader()
        .read_to_string(&mut raw)
        .map_err(InstallationError::Io)?;
    Ok(raw)
}

pub(crate) fn should_retry_http_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429) || (500..=599).contains(&status)
}

pub(crate) fn retry_delay_for_attempt(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(5);
    let multiplier = 1u64 << exponent;
    let millis = HTTP_RETRY_BASE_DELAY_MS
        .saturating_mul(multiplier)
        .min(5_000);
    Duration::from_millis(millis)
}

pub(crate) fn map_ureq_error(error: ureq::Error) -> InstallationError {
    match error {
        ureq::Error::StatusCode(status) => InstallationError::HttpStatus {
            url: "<unknown>".to_owned(),
            status,
            body: String::new(),
        },
        other => InstallationError::Transport {
            url: "<transport>".to_owned(),
            message: other.to_string(),
        },
    }
}
