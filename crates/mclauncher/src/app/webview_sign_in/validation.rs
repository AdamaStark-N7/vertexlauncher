pub(super) fn validate_sign_in_urls(
    auth_request_uri: &str,
    redirect_uri: &str,
) -> Result<(), String> {
    if !auth_request_uri.starts_with("https://login.live.com/oauth20_authorize.srf") {
        return Err("Auth URL is not an expected Microsoft OAuth authorize endpoint".to_owned());
    }

    if !redirect_uri.starts_with("https://login.live.com/oauth20_desktop.srf") {
        return Err("Redirect URL is not an expected Microsoft desktop OAuth endpoint".to_owned());
    }

    Ok(())
}
