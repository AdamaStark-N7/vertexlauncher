//! Scrubbing secrets out of text before it is logged: URLs keep only their parameter names,
//! bearer tokens and credential-looking `key=value` pairs are replaced with `[redacted]`.
//!
//! One implementation shared by the auth crate, the webview sign-in helper and the UI.

use std::collections::BTreeSet;

use url::Url;

/// Sanitizes free text (error messages, payloads) for logging: URLs are reduced to their
/// parameter names, bearer tokens and sensitive key/value pairs are redacted, and the result
/// is cut to `max_chars`.
pub fn sanitize_message_for_log(value: &str, max_chars: usize) -> String {
    let sanitized_urls = sanitize_urls_in_text(value);
    let sanitized_bearer = redact_bearer_tokens(&sanitized_urls);
    let sanitized_pairs = redact_sensitive_key_values(&sanitized_bearer);
    truncate_for_log(&sanitized_pairs, max_chars)
}

pub fn truncate_for_log(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut chars = value.chars();

    for _ in 0..max_chars {
        let Some(ch) = chars.next() else {
            return value.to_owned();
        };
        out.push(ch);
    }

    if chars.next().is_some() {
        out.push_str("...");
    }

    out
}

pub fn sanitize_url_for_log(value: &str) -> String {
    let Ok(parsed) = Url::parse(value) else {
        return truncate_for_log(value, 160);
    };

    let mut out = String::new();
    out.push_str(parsed.scheme());
    out.push_str("://");
    out.push_str(parsed.host_str().unwrap_or("<no-host>"));
    out.push_str(parsed.path());

    let query_keys: BTreeSet<String> = parsed.query_pairs().map(|(key, _)| key.into()).collect();
    if !query_keys.is_empty() {
        out.push_str("?params=");
        for (i, key) in query_keys.into_iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&key);
        }
    }

    if parsed.fragment().is_some() {
        out.push_str("#fragment");
    }

    out
}

pub fn sanitize_urls_in_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut remaining = value;

    while let Some(url_start) = find_next_url_start(remaining) {
        out.push_str(&remaining[..url_start]);
        let url_candidate = &remaining[url_start..];
        let url_end = find_url_end(url_candidate);
        let (url, trailing) = split_trailing_url_punctuation(&url_candidate[..url_end]);
        out.push_str(&sanitize_url_for_log(url));
        out.push_str(trailing);
        remaining = &url_candidate[url_end..];
    }

    out.push_str(remaining);
    out
}

fn find_next_url_start(value: &str) -> Option<usize> {
    ["https://", "http://", "file://"]
        .into_iter()
        .filter_map(|prefix| value.find(prefix))
        .min()
}

fn find_url_end(value: &str) -> usize {
    value
        .char_indices()
        .find_map(|(index, ch)| {
            if ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
                )
            {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(value.len())
}

fn split_trailing_url_punctuation(value: &str) -> (&str, &str) {
    let trimmed = value.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    let trailing = &value[trimmed.len()..];
    (trimmed, trailing)
}

pub fn redact_bearer_tokens(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let lower = value.to_ascii_lowercase();
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("bearer ") {
        let start = cursor + relative_start;
        let token_start = start + "bearer ".len();
        out.push_str(&value[cursor..token_start]);

        let token_end = value[token_start..]
            .char_indices()
            .find_map(|(offset, ch)| {
                if ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | ';' | ')' | ']' | '}') {
                    Some(token_start + offset)
                } else {
                    None
                }
            })
            .unwrap_or(value.len());

        if token_end > token_start {
            out.push_str("[redacted]");
        }
        cursor = token_end;
    }

    out.push_str(&value[cursor..]);
    out
}

pub fn redact_sensitive_key_values(value: &str) -> String {
    const SENSITIVE_KEYS: [&str; 11] = [
        "authorization_code",
        "access_token",
        "refresh_token",
        "client_secret",
        "code_verifier",
        "authorization",
        "id_token",
        "state",
        "code",
        "token",
        "xuid",
    ];

    let lower = value.to_ascii_lowercase();
    let mut out = String::with_capacity(value.len());
    let mut cursor = 0;

    while let Some((value_start, value_end)) =
        find_next_sensitive_value_range(value, &lower, cursor, &SENSITIVE_KEYS)
    {
        out.push_str(&value[cursor..value_start]);
        out.push_str("[redacted]");
        cursor = value_end;
    }

    out.push_str(&value[cursor..]);
    out
}

fn find_next_sensitive_value_range(
    value: &str,
    lower: &str,
    cursor: usize,
    sensitive_keys: &[&str],
) -> Option<(usize, usize)> {
    let bytes = value.as_bytes();
    let lower_bytes = lower.as_bytes();
    let mut index = cursor;

    while index < value.len() {
        if !value.is_char_boundary(index) {
            index += 1;
            continue;
        }

        for key in sensitive_keys {
            let key_bytes = key.as_bytes();
            let key_end = index + key_bytes.len();
            if key_end > value.len() || lower_bytes[index..key_end] != *key_bytes {
                continue;
            }

            if index > 0 {
                let previous = bytes[index - 1];
                if previous.is_ascii_alphanumeric() || previous == b'_' {
                    continue;
                }
            }

            if key_end < value.len() {
                let next = bytes[key_end];
                if next.is_ascii_alphanumeric() || next == b'_' {
                    continue;
                }
            }

            let mut separator_index = key_end;
            if separator_index < value.len() && matches!(bytes[separator_index], b'"' | b'\'') {
                separator_index += 1;
            }
            while separator_index < value.len() && bytes[separator_index].is_ascii_whitespace() {
                separator_index += 1;
            }
            if separator_index >= value.len() || !matches!(bytes[separator_index], b'=' | b':') {
                continue;
            }

            let mut value_start = separator_index + 1;
            while value_start < value.len() && bytes[value_start].is_ascii_whitespace() {
                value_start += 1;
            }
            if value_start >= value.len() {
                continue;
            }

            let value_end = find_sensitive_value_end(bytes, value_start);
            if value_end > value_start {
                return Some((value_start, value_end));
            }
        }

        index += 1;
    }

    None
}

fn find_sensitive_value_end(value: &[u8], value_start: usize) -> usize {
    if value_start >= value.len() {
        return value_start;
    }

    if matches!(value[value_start], b'"' | b'\'') {
        let quote = value[value_start];
        for index in value_start + 1..value.len() {
            if value[index] == quote {
                return index + 1;
            }
        }
        return value.len();
    }

    let mut index = value_start;
    while index < value.len() {
        let ch = value[index];
        if ch.is_ascii_whitespace()
            || matches!(ch, b'&' | b',' | b';' | b')' | b']' | b'}' | b'"' | b'\'')
        {
            break;
        }
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_keep_only_parameter_names() {
        assert_eq!(
            sanitize_url_for_log(
                "https://login.live.com/oauth20_desktop.srf?code=abc&state=secret&error=none#frag"
            ),
            "https://login.live.com/oauth20_desktop.srf?params=code,error,state#fragment"
        );
    }

    #[test]
    fn messages_redact_urls_bearer_tokens_and_pairs() {
        let actual = sanitize_message_for_log(
            r#"failed https://login.live.com/x?code=abc&state=secret refresh_token=refresh access_token='a' Bearer super-secret {"code":"abc"}"#,
            500,
        );
        assert_eq!(
            actual,
            r#"failed https://login.live.com/x?params=code,state refresh_token=[redacted] access_token=[redacted] Bearer [redacted] {"code":[redacted]}"#
        );
    }

    #[test]
    fn every_known_secret_key_is_redacted_including_the_merged_ones() {
        // `code_verifier` (PKCE secret) and `token`/`xuid` each used to be missing from one copy.
        for key in [
            "code_verifier",
            "token",
            "xuid",
            "client_secret",
            "id_token",
        ] {
            let actual = sanitize_message_for_log(&format!("{key}=hunter2 done"), 200);
            assert_eq!(actual, format!("{key}=[redacted] done"), "{key}");
        }
    }

    #[test]
    fn keys_only_match_whole_words() {
        // `token` must not chew into `tokenizer`, and `access_token` is one key, not `token`.
        assert_eq!(
            sanitize_message_for_log("tokenizer=fast", 100),
            "tokenizer=fast"
        );
        assert_eq!(
            sanitize_message_for_log("access_token=abc", 100),
            "access_token=[redacted]"
        );
    }

    #[test]
    fn truncation_adds_an_ellipsis_only_when_cut() {
        assert_eq!(truncate_for_log("abcdef", 3), "abc...");
        assert_eq!(truncate_for_log("abc", 3), "abc");
    }
}
