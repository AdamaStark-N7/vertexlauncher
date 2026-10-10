use super::*;

pub(crate) struct LaunchContext {
    pub(super) substitutions: HashMap<String, String>,
    pub(super) features: HashMap<String, bool>,
}

pub(crate) fn build_launch_context(
    instance_root: &Path,
    _game_version: &str,
    profile_id: &str,
    assets_index_name: &str,
    classpath: &str,
    natives_dir: &Path,
    player_name: Option<&str>,
    player_uuid: Option<&str>,
    auth_access_token: Option<&str>,
    auth_xuid: Option<&str>,
    auth_user_type: Option<&str>,
    quick_play_singleplayer: Option<&str>,
    quick_play_multiplayer: Option<&str>,
) -> LaunchContext {
    let username = player_name
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("Player");
    let uuid = player_uuid
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("00000000000000000000000000000000");
    let access_token = auth_access_token
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("0");
    let xuid = auth_xuid
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("0");
    let user_type = auth_user_type
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("legacy");
    let mut substitutions = HashMap::new();
    substitutions.insert("auth_player_name".to_owned(), username.to_owned());
    substitutions.insert("version_name".to_owned(), profile_id.to_owned());
    substitutions.insert(
        "game_directory".to_owned(),
        display_user_path(instance_root),
    );
    substitutions.insert(
        "assets_root".to_owned(),
        display_user_path(instance_root.join("assets").as_path()),
    );
    substitutions.insert("assets_index_name".to_owned(), assets_index_name.to_owned());
    // Legacy token used by pre-1.13 minecraftArguments as --assetsDir value.
    substitutions.insert(
        "game_assets".to_owned(),
        display_user_path(
            instance_root
                .join("assets")
                .join("virtual")
                .join(assets_index_name)
                .as_path(),
        ),
    );
    substitutions.insert("auth_uuid".to_owned(), uuid.to_owned());
    substitutions.insert("auth_access_token".to_owned(), access_token.to_owned());
    substitutions.insert("clientid".to_owned(), "0".to_owned());
    substitutions.insert("auth_xuid".to_owned(), xuid.to_owned());
    substitutions.insert("user_type".to_owned(), user_type.to_owned());
    substitutions.insert("version_type".to_owned(), "release".to_owned());
    substitutions.insert("user_properties".to_owned(), "{}".to_owned());
    substitutions.insert("classpath".to_owned(), classpath.to_owned());
    substitutions.insert(
        "classpath_separator".to_owned(),
        classpath_separator().to_owned(),
    );
    substitutions.insert(
        "library_directory".to_owned(),
        display_user_path(instance_root.join("libraries").as_path()),
    );
    substitutions.insert(
        "natives_directory".to_owned(),
        display_user_path(natives_dir),
    );
    substitutions.insert("launcher_name".to_owned(), "vertexlauncher".to_owned());
    substitutions.insert("launcher_version".to_owned(), "0.1".to_owned());
    substitutions.insert(
        "quickPlayPath".to_owned(),
        display_user_path(instance_root.join("quickPlay").as_path()),
    );
    if let Some(world) = quick_play_singleplayer
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        substitutions.insert("quickPlaySingleplayer".to_owned(), world.to_owned());
    }
    if let Some(server) = quick_play_multiplayer
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        substitutions.insert("quickPlayMultiplayer".to_owned(), server.to_owned());
    }
    let mut features = HashMap::new();
    features.insert("is_demo_user".to_owned(), false);
    features.insert("has_custom_resolution".to_owned(), false);
    let has_quick_play_singleplayer = quick_play_singleplayer
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_quick_play_multiplayer = quick_play_multiplayer
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    features.insert(
        "has_quick_plays_support".to_owned(),
        has_quick_play_singleplayer || has_quick_play_multiplayer,
    );
    features.insert(
        "is_quick_play_singleplayer".to_owned(),
        has_quick_play_singleplayer,
    );
    features.insert(
        "is_quick_play_multiplayer".to_owned(),
        has_quick_play_multiplayer,
    );
    features.insert("is_quick_play_realms".to_owned(), false);
    LaunchContext {
        substitutions,
        features,
    }
}

pub(crate) fn resolve_assets_index_name<'a>(
    chain: &'a [serde_json::Value],
    fallback: &'a str,
) -> &'a str {
    for profile in chain.iter().rev() {
        if let Some(id) = profile
            .get("assetIndex")
            .and_then(|value| value.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return id;
        }
    }
    fallback
}

pub(crate) fn collect_jvm_arguments(
    chain: &[serde_json::Value],
    context: &LaunchContext,
) -> Vec<String> {
    let mut args = Vec::new();
    for profile in chain {
        if let Some(values) = profile
            .get("arguments")
            .and_then(|v| v.get("jvm"))
            .and_then(serde_json::Value::as_array)
        {
            args.extend(collect_argument_array(values, context));
        }
    }
    if args.is_empty() {
        args.push("-Djava.library.path=${natives_directory}".to_owned());
        args.push("-cp".to_owned());
        args.push("${classpath}".to_owned());
    }
    args.into_iter()
        .map(|entry| substitute_tokens(entry.as_str(), context))
        .collect()
}

pub(crate) fn collect_game_arguments(
    chain: &[serde_json::Value],
    context: &LaunchContext,
) -> Vec<String> {
    let mut args = Vec::new();
    for profile in chain {
        if let Some(values) = profile
            .get("arguments")
            .and_then(|v| v.get("game"))
            .and_then(serde_json::Value::as_array)
        {
            args.extend(collect_argument_array(values, context));
        }
    }
    if args.is_empty() {
        for profile in chain.iter().rev() {
            if let Some(raw) = profile
                .get("minecraftArguments")
                .and_then(serde_json::Value::as_str)
            {
                args.extend(raw.split_whitespace().map(str::to_owned));
                break;
            }
        }
    }
    let resolved: Vec<String> = args
        .into_iter()
        .map(|entry| substitute_tokens(entry.as_str(), context))
        .collect();
    normalize_quick_play_arguments(resolved, context)
}

pub(crate) fn normalize_quick_play_arguments(
    args: Vec<String>,
    context: &LaunchContext,
) -> Vec<String> {
    let quick_play_path = context
        .substitutions
        .get("quickPlayPath")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let quick_play_singleplayer = context
        .substitutions
        .get("quickPlaySingleplayer")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let quick_play_multiplayer = context
        .substitutions
        .get("quickPlayMultiplayer")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let requested_quick_play_mode = quick_play_singleplayer
        .map(|world| ("--quickPlaySingleplayer", world))
        .or_else(|| quick_play_multiplayer.map(|server| ("--quickPlayMultiplayer", server)));

    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut has_quick_play_path = false;
    let mut quick_play_mode_selected = false;

    while cursor < args.len() {
        let current = args[cursor].as_str();
        let is_quick_play_flag = matches!(
            current,
            "--quickPlayPath"
                | "--quickPlaySingleplayer"
                | "--quickPlayMultiplayer"
                | "--quickPlayRealms"
        );
        if !is_quick_play_flag {
            out.push(args[cursor].clone());
            cursor += 1;
            continue;
        }

        let value = args.get(cursor + 1).map(String::as_str).unwrap_or_default();
        let unresolved_placeholder =
            value.starts_with("${quickPlay") && value.ends_with('}') && value.len() > 2;
        if value.trim().is_empty() || unresolved_placeholder {
            cursor = cursor.saturating_add(2);
            continue;
        }

        if current == "--quickPlayPath" {
            if has_quick_play_path {
                cursor = cursor.saturating_add(2);
                continue;
            }
            has_quick_play_path = true;
            out.push(args[cursor].clone());
            out.push(args[cursor + 1].clone());
            cursor += 2;
            continue;
        }

        if quick_play_mode_selected {
            cursor = cursor.saturating_add(2);
            continue;
        }

        out.push(args[cursor].clone());
        out.push(args[cursor + 1].clone());
        quick_play_mode_selected = true;
        cursor += 2;
    }

    if let Some((flag, value)) = requested_quick_play_mode
        && !quick_play_mode_selected
    {
        if !has_quick_play_path && let Some(path) = quick_play_path {
            out.push("--quickPlayPath".to_owned());
            out.push(path.to_owned());
        }
        out.push(flag.to_owned());
        out.push(value.to_owned());
    }

    out
}

pub(crate) fn collect_argument_array(
    values: &[serde_json::Value],
    context: &LaunchContext,
) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        if let Some(raw) = value.as_str() {
            out.push(substitute_tokens(raw, context));
            continue;
        }
        let Some(object) = value.as_object() else {
            continue;
        };
        if !rules_allow_for_launch(object.get("rules"), context) {
            continue;
        }
        let Some(arg_value) = object.get("value") else {
            continue;
        };
        if let Some(single) = arg_value.as_str() {
            out.push(substitute_tokens(single, context));
        } else if let Some(array) = arg_value.as_array() {
            for entry in array {
                if let Some(single) = entry.as_str() {
                    out.push(substitute_tokens(single, context));
                }
            }
        }
    }
    out
}

pub(crate) fn library_rules_allow(library: &serde_json::Value) -> bool {
    rules_allow_os_only(library.get("rules"))
}

pub(crate) fn rules_allow_for_launch(
    rules_value: Option<&serde_json::Value>,
    context: &LaunchContext,
) -> bool {
    let Some(rules) = rules_value.and_then(serde_json::Value::as_array) else {
        return true;
    };
    let mut allowed = false;
    for rule in rules {
        let Some(object) = rule.as_object() else {
            continue;
        };
        let action = object
            .get("action")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("allow");
        let applies = rule_applies_to_current_os(object.get("os"))
            && rule_features_match(object.get("features"), context);
        if applies {
            allowed = action == "allow";
        }
    }
    allowed
}

pub(crate) fn rules_allow_os_only(rules_value: Option<&serde_json::Value>) -> bool {
    let Some(rules) = rules_value.and_then(serde_json::Value::as_array) else {
        return true;
    };
    let mut allowed = false;
    for rule in rules {
        let Some(object) = rule.as_object() else {
            continue;
        };
        let action = object
            .get("action")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("allow");
        let applies = rule_applies_to_current_os(object.get("os"));
        if applies {
            allowed = action == "allow";
        }
    }
    allowed
}

pub(crate) fn rule_features_match(
    features_value: Option<&serde_json::Value>,
    context: &LaunchContext,
) -> bool {
    let Some(features) = features_value.and_then(serde_json::Value::as_object) else {
        return true;
    };
    for (feature, expected) in features {
        let Some(expected) = expected.as_bool() else {
            continue;
        };
        let actual = context.features.get(feature).copied().unwrap_or(false);
        if actual != expected {
            return false;
        }
    }
    true
}

pub(crate) fn rule_applies_to_current_os(os_value: Option<&serde_json::Value>) -> bool {
    let Some(os_object) = os_value.and_then(serde_json::Value::as_object) else {
        return true;
    };
    if let Some(name) = os_object.get("name").and_then(serde_json::Value::as_str)
        && name != current_os_natives_key()
    {
        return false;
    }
    if let Some(arch) = os_object.get("arch").and_then(serde_json::Value::as_str)
        && !arch_matches_current_target(arch)
    {
        return false;
    }
    true
}

pub(crate) fn substitute_tokens(raw: &str, context: &LaunchContext) -> String {
    let mut result = raw.to_owned();
    for (key, value) in &context.substitutions {
        let token = format!("${{{key}}}");
        result = result.replace(token.as_str(), value.as_str());
    }
    result
}

pub(crate) fn parse_user_args(raw: Option<&str>) -> Vec<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}
