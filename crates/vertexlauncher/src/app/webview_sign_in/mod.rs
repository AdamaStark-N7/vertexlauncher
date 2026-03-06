use std::io::Write;
use std::process::{Command, Stdio};

mod desktop;
mod ipc;
mod validation;

const HELPER_FLAG: &str = "--vertex-webview-signin";

pub fn maybe_run_helper_from_args() -> Result<bool, String> {
    let mut args = std::env::args();
    let _ = args.next();

    let Some(flag) = args.next() else {
        return Ok(false);
    };
    if flag != HELPER_FLAG {
        return Ok(false);
    }

    if args.next().is_some() {
        return Err("Unexpected extra arguments for webview helper".to_owned());
    }

    let (auth_request_uri, redirect_uri) = ipc::read_helper_request_from_stdin()?;
    validation::validate_sign_in_urls(&auth_request_uri, &redirect_uri)?;

    let callback_url = desktop::run_webview_window(&auth_request_uri, &redirect_uri)?;
    println!("{callback_url}");
    std::io::stdout()
        .flush()
        .map_err(|err| format!("Failed to flush webview helper output: {err}"))?;

    Ok(true)
}

pub fn open_microsoft_sign_in(
    auth_request_uri: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    validation::validate_sign_in_urls(auth_request_uri, redirect_uri)?;

    let current_exe = std::env::current_exe()
        .map_err(|err| format!("Failed to resolve launcher executable path: {err}"))?;

    let mut child = Command::new(current_exe)
        .arg(HELPER_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to start webview helper process: {err}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        ipc::write_helper_request_to_stdin(&mut stdin, auth_request_uri, redirect_uri)?;
    } else {
        return Err("Webview sign-in helper stdin was unavailable".to_owned());
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("Failed waiting for webview helper process: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if stderr.is_empty() {
            return Err("Webview sign-in helper failed without an error message".to_owned());
        }

        return Err(format!("Webview sign-in helper failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let callback_url = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .next_back()
        .ok_or_else(|| "Webview sign-in helper returned no callback URL".to_owned())?;

    Ok(callback_url.to_owned())
}
