use super::*;

/// Fetches the latest Minecraft profile and persists it back into the cached account
/// snapshot for the active profile.
///
/// `profile_id` should match the active Minecraft profile identifier. `access_token`
/// must be a valid token accepted by the auth backend.
///
/// Returns a user-facing error string on network or cache failures.
pub(super) fn fetch_and_cache_profile(
    profile_id: String,
    access_token: &str,
    display_name: &str,
) -> Result<LoadedProfile, String> {
    tracing::info!(
        target: "vertexlauncher/skins",
        display_name,
        "Fetching latest skin profile."
    );
    let profile = auth::fetch_minecraft_profile(access_token)
        .map_err(|err| format!("Failed to fetch latest profile: {err}"))?;
    tracing::info!(
        target: "vertexlauncher/skins",
        display_name,
        skins = profile.skins.len(),
        capes = profile.capes.len(),
        "Fetched latest skin profile."
    );
    update_cached_profile(profile_id.as_str(), &profile, display_name)?;
    Ok(LoadedProfile::from_profile(profile))
}

/// Rewrites the cached Minecraft profile snapshot for one cached account if present.
///
/// `profile_id` is matched case-insensitively against cached profile identifiers.
/// Returns `Ok(())` when no matching cache entry exists.
pub(super) fn update_cached_profile(
    profile_id: &str,
    profile: &MinecraftProfileState,
    display_name: &str,
) -> Result<(), String> {
    tracing::info!(
        target: "vertexlauncher/skins",
        display_name,
        "Updating cached account profile snapshot."
    );
    let mut cache =
        auth::load_cached_accounts().map_err(|err| format!("Cache read failed: {err}"))?;
    let mut changed = false;
    let mut new_capes: Vec<String> = Vec::new();

    for account in &mut cache.accounts {
        if account
            .minecraft_profile
            .id
            .eq_ignore_ascii_case(profile_id)
        {
            for cape in &profile.capes {
                let already_known = account
                    .minecraft_profile
                    .capes
                    .iter()
                    .any(|known| known.id == cape.id);
                if !already_known {
                    new_capes.push(
                        cape.alias
                            .clone()
                            .unwrap_or_else(|| "Unknown cape".to_owned()),
                    );
                }
            }
            account.minecraft_profile = profile.clone();
            account.cached_at_unix_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            changed = true;
            break;
        }
    }

    if changed {
        auth::save_cached_accounts(&cache).map_err(|err| format!("Cache write failed: {err}"))?;
        // Only announce after the new list is persisted so a cape is congratulated exactly once.
        // Accounts with no cache entry never reach here: with nothing to compare against, every
        // cape would look new.
        if let Some(message) = new_capes_message(display_name, &new_capes) {
            crate::notification::emit_with_icon(
                crate::notification::Severity::Info,
                "skins",
                message,
                crate::assets::CONFETTI_SVG,
            );
        }
    } else {
        tracing::info!(
            target: "vertexlauncher/skins",
            display_name,
            "No matching cached account found to update."
        );
    }

    Ok(())
}

/// Congratulation text for the capes an account gained since the last check, worded for one,
/// two, or many capes. `None` when there is nothing new.
fn new_capes_message(account_name: &str, cape_names: &[String]) -> Option<String> {
    let body = match cape_names {
        [] => return None,
        [only] => format!("got a new cape: {only}"),
        [first, second] => format!("got two new capes: {first} and {second}"),
        [init @ .., last] => format!(
            "got {} new capes: {}, and {last}",
            count_word(cape_names.len()),
            init.join(", ")
        ),
    };
    Some(format!("Your account {account_name} {body}, Congrats!"))
}

fn count_word(count: usize) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS
        .get(count)
        .map_or_else(|| "several".to_owned(), |word| (*word).to_owned())
}

#[cfg(test)]
mod tests {
    use super::new_capes_message;

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn message_matches_the_number_of_new_capes() {
        assert_eq!(new_capes_message("Steve", &[]), None);
        assert_eq!(
            new_capes_message("Steve", &names(&["Migrator"])).as_deref(),
            Some("Your account Steve got a new cape: Migrator, Congrats!")
        );
        assert_eq!(
            new_capes_message("Steve", &names(&["Migrator", "Vanilla"])).as_deref(),
            Some("Your account Steve got two new capes: Migrator and Vanilla, Congrats!")
        );
        assert_eq!(
            new_capes_message("Steve", &names(&["Migrator", "Vanilla", "Mojang"])).as_deref(),
            Some(
                "Your account Steve got three new capes: Migrator, Vanilla, and Mojang, Congrats!"
            )
        );
    }
}
