//! Token and casing helpers for turning installed file names into lookup queries.

pub(super) fn split_lookup_tokens(value: &str) -> Vec<&str> {
    value
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect()
}

pub(super) fn trim_ignorable_lookup_suffix<'a>(tokens: &'a [&'a str]) -> &'a [&'a str] {
    let trimmed_len = tokens
        .iter()
        .rposition(|token| !is_ignorable_lookup_suffix_token(token))
        .map(|index| index + 1)
        .unwrap_or(tokens.len());
    &tokens[..trimmed_len]
}

pub(super) fn is_ignorable_lookup_suffix_token(token: &str) -> bool {
    matches!(
        token,
        "fabric"
            | "forge"
            | "neoforge"
            | "quilt"
            | "rift"
            | "liteloader"
            | "loader"
            | "mod"
            | "mods"
            | "shader"
            | "shaders"
            | "shaderpack"
            | "shaderpacks"
            | "resourcepack"
            | "resourcepacks"
            | "texturepack"
            | "texturepacks"
            | "datapack"
            | "datapacks"
            | "minecraft"
            | "mc"
            | "client"
            | "server"
    )
}

pub(super) fn split_camel_case_words(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 8);
    let mut chars = value.chars().peekable();
    let mut previous: Option<char> = None;

    while let Some(ch) = chars.next() {
        let next = chars.peek().copied();
        if let Some(prev) = previous
            && should_insert_camel_case_boundary(prev, ch, next)
            && !result.ends_with(' ')
        {
            result.push(' ');
        }
        result.push(ch);
        previous = Some(ch);
    }

    result
}

pub(super) fn should_insert_camel_case_boundary(
    previous: char,
    current: char,
    next: Option<char>,
) -> bool {
    (previous.is_ascii_lowercase() && current.is_ascii_uppercase())
        || (previous.is_ascii_uppercase()
            && current.is_ascii_uppercase()
            && next.is_some_and(|next| next.is_ascii_lowercase()))
}

pub(super) fn query_has_only_ignorable_suffix_tokens(
    query_tokens: &[&str],
    candidate_tokens: &[&str],
) -> bool {
    query_tokens.starts_with(candidate_tokens)
        && query_tokens[candidate_tokens.len()..]
            .iter()
            .all(|token| is_ignorable_lookup_suffix_token(token))
}
