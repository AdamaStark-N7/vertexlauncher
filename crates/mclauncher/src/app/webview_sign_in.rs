use std::io::Write;
use std::process::{Command, Stdio};

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

    let Some(auth_request_uri) = args.next() else {
        return Err("Missing auth request URL for webview helper".to_owned());
    };
    let Some(redirect_uri) = args.next() else {
        return Err("Missing redirect URI for webview helper".to_owned());
    };

    if args.next().is_some() {
        return Err("Unexpected extra arguments for webview helper".to_owned());
    }

    let callback_url = run_webview_window(&auth_request_uri, &redirect_uri)?;
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
    let current_exe = std::env::current_exe()
        .map_err(|err| format!("Failed to resolve launcher executable path: {err}"))?;

    let output = Command::new(current_exe)
        .arg(HELPER_FLAG)
        .arg(auth_request_uri)
        .arg(redirect_uri)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("Failed to start webview helper process: {err}"))?;

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

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn run_webview_window(auth_request_uri: &str, redirect_uri: &str) -> Result<String, String> {
    use std::sync::{Arc, Mutex};
    use tao::dpi::LogicalSize;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::run_return::EventLoopExtRunReturn;
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    #[cfg(target_os = "linux")]
    use tao::platform::unix::WindowExtUnix;
    #[cfg(target_os = "linux")]
    use wry::WebViewBuilderExtUnix;

    #[derive(Clone, Copy)]
    enum UserEvent {
        Finish,
    }

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let result = Arc::new(Mutex::new(None::<Result<String, String>>));
    let result_for_nav = Arc::clone(&result);
    let redirect_prefix = redirect_uri.to_owned();

    let window = WindowBuilder::new()
        .with_title("Microsoft Sign-In")
        .with_inner_size(LogicalSize::new(980.0, 760.0))
        .build(&event_loop)
        .map_err(|err| format!("Failed to create sign-in window: {err}"))?;

    let webview_builder = WebViewBuilder::new()
        .with_url(auth_request_uri)
        .with_navigation_handler(move |uri: String| {
            let current_uri = uri;
            if current_uri.starts_with(&redirect_prefix) {
                if let Ok(mut slot) = result_for_nav.lock() {
                    *slot = Some(Ok(current_uri));
                }
                let _ = proxy.send_event(UserEvent::Finish);
                return false;
            }
            true
        });

    #[cfg(target_os = "linux")]
    let _webview =
        webview_builder
            .build_gtk(window.default_vbox().ok_or_else(|| {
                "Failed to access Tao default GTK container for webview".to_owned()
            })?)
            .map_err(|err| format!("Failed to build webview: {err}"))?;

    #[cfg(not(target_os = "linux"))]
    let _webview = webview_builder
        .build(&window)
        .map_err(|err| format!("Failed to build webview: {err}"))?;

    let result_for_loop = Arc::clone(&result);
    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::Finish) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Ok(mut slot) = result_for_loop.lock() {
                    if slot.is_none() {
                        *slot = Some(Err("Microsoft sign-in was canceled".to_owned()));
                    }
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });

    match result.lock() {
        Ok(mut slot) => slot
            .take()
            .unwrap_or_else(|| Err("Microsoft sign-in ended without a callback URL".to_owned())),
        Err(_) => Err("Sign-in state was poisoned unexpectedly".to_owned()),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn run_webview_window(_auth_request_uri: &str, _redirect_uri: &str) -> Result<String, String> {
    Err("Webview sign-in is not supported on this platform".to_owned())
}
