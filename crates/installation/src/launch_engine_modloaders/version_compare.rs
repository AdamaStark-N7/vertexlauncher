pub(crate) fn sort_loader_versions_desc(mut versions: Vec<String>) -> Vec<String> {
    sort_loader_versions_desc_in_place(&mut versions);
    versions
}

pub(crate) fn sort_loader_versions_desc_in_place(versions: &mut [String]) {
    versions.sort_by(|left, right| compare_version_like_desc(left.as_str(), right.as_str()));
}

pub(crate) fn compare_version_like_desc(left: &str, right: &str) -> std::cmp::Ordering {
    compare_version_like(left, right).reverse()
}

pub(crate) fn compare_version_like(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let left_tokens = version_like_tokens(left);
    let right_tokens = version_like_tokens(right);

    for (left_token, right_token) in left_tokens.iter().zip(right_tokens.iter()) {
        let ordering = match (left_token, right_token) {
            (VersionToken::Number(left), VersionToken::Number(right)) => left.cmp(right),
            (VersionToken::Text(left), VersionToken::Text(right)) => {
                left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
            }
            (VersionToken::Number(_), VersionToken::Text(_)) => Ordering::Greater,
            (VersionToken::Text(_), VersionToken::Number(_)) => Ordering::Less,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    left_tokens
        .len()
        .cmp(&right_tokens.len())
        .then_with(|| left.cmp(right))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VersionToken {
    Number(u64),
    Text(String),
}

pub(crate) fn version_like_tokens(raw: &str) -> Vec<VersionToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = None;

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            let is_digit = ch.is_ascii_digit();
            match current_is_digit {
                Some(previous) if previous != is_digit => {
                    push_version_token(&mut tokens, &mut current, previous);
                    current_is_digit = Some(is_digit);
                }
                None => current_is_digit = Some(is_digit),
                _ => {}
            }
            current.push(ch);
        } else if let Some(previous) = current_is_digit.take() {
            push_version_token(&mut tokens, &mut current, previous);
        }
    }

    if let Some(previous) = current_is_digit {
        push_version_token(&mut tokens, &mut current, previous);
    }

    tokens
}

pub(crate) fn push_version_token(
    tokens: &mut Vec<VersionToken>,
    current: &mut String,
    was_digit: bool,
) {
    if current.is_empty() {
        return;
    }
    let token = if was_digit {
        VersionToken::Number(current.parse::<u64>().unwrap_or(0))
    } else {
        VersionToken::Text(current.clone())
    };
    tokens.push(token);
    current.clear();
}
