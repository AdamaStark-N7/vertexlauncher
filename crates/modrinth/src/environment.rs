use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Where a project or version runs, from Modrinth's `environment` field.
///
/// This replaces the deprecated `client_side` / `server_side` pair, which could not
/// express enough to build `.mrpack` files or to strip client-only mods from servers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Environment {
    ClientOnly,
    ServerOnly,
    SingleplayerOnly,
    DedicatedServerOnly,
    ClientAndServer,
    ClientOnlyServerOptional,
    ServerOnlyClientOptional,
    SingleplayerAndServer,
    #[default]
    Unknown,
}

impl Environment {
    /// Parses an API slug such as `client_only`; unrecognized values map to `Unknown`.
    pub fn from_slug(slug: &str) -> Self {
        match slug.trim().to_ascii_lowercase().as_str() {
            "client_only" => Self::ClientOnly,
            "server_only" => Self::ServerOnly,
            "singleplayer_only" => Self::SingleplayerOnly,
            "dedicated_server_only" => Self::DedicatedServerOnly,
            "client_and_server" => Self::ClientAndServer,
            "client_only_server_optional" => Self::ClientOnlyServerOptional,
            "server_only_client_optional" => Self::ServerOnlyClientOptional,
            "singleplayer_and_server" => Self::SingleplayerAndServer,
            _ => Self::Unknown,
        }
    }

    /// Whether the content should be installed on a dedicated server.
    /// `Unknown` returns true so unclassified mods are never silently dropped.
    pub fn runs_on_dedicated_server(self) -> bool {
        !matches!(
            self,
            Self::ClientOnly | Self::ClientOnlyServerOptional | Self::SingleplayerOnly
        )
    }

    /// Whether the content should be installed on a client.
    pub fn runs_on_client(self) -> bool {
        !matches!(self, Self::ServerOnly | Self::DedicatedServerOnly)
    }
}

/// Lenient deserializer: accepts a slug string or an array of slugs (first wins).
pub(crate) fn deserialize_environment<'de, D>(deserializer: D) -> Result<Environment, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(Value::String(slug)) => Environment::from_slug(slug.as_str()),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(Environment::from_slug)
            .find(|environment| *environment != Environment::Unknown)
            .unwrap_or_default(),
        _ => Environment::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Holder {
        #[serde(default, deserialize_with = "deserialize_environment")]
        environment: Environment,
    }

    #[test]
    fn parses_string_array_and_missing() {
        let string: Holder = serde_json::from_str(r#"{"environment":"client_only"}"#).unwrap();
        assert_eq!(string.environment, Environment::ClientOnly);
        let array: Holder = serde_json::from_str(r#"{"environment":["server_only"]}"#).unwrap();
        assert_eq!(array.environment, Environment::ServerOnly);
        let missing: Holder = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.environment, Environment::Unknown);
        assert!(!Environment::ClientOnly.runs_on_dedicated_server());
        assert!(Environment::Unknown.runs_on_dedicated_server());
    }
}
