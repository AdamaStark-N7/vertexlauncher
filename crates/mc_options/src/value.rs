//! Parsing and formatting of raw `options.txt` values.

/// Splits surrounding double quotes off a raw value; returns whether it was quoted.
pub fn unquote(raw: &str) -> (&str, bool) {
    match raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(inner) => (inner, true),
        None => (raw, false),
    }
}

pub fn quote(value: &str) -> String {
    format!("\"{value}\"")
}

pub fn parse_bool(raw: &str) -> Option<bool> {
    match unquote(raw).0.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn parse_i64(raw: &str) -> Option<i64> {
    unquote(raw).0.trim().parse().ok()
}

pub fn parse_f64(raw: &str) -> Option<f64> {
    unquote(raw).0.trim().parse().ok()
}

pub fn format_bool(value: bool) -> String {
    value.to_string()
}

/// Formats like Java's `Float.toString`: always at least one decimal.
pub fn format_float(value: f64) -> String {
    let single = value as f32;
    if single.fract() == 0.0 && single.abs() < 1e7 {
        format!("{single:.1}")
    } else {
        format!("{single}")
    }
}

/// Parses a `["a","b"]` list value (as used by `resourcePacks`).
pub fn parse_list(raw: &str) -> Vec<String> {
    let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in inner.chars() {
        if in_string {
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                items.push(std::mem::take(&mut current));
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_string = true;
        }
    }
    items
}

pub fn format_list(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|item| format!("\"{}\"", item.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", quoted.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_match_java_formatting() {
        assert_eq!(format_float(1.0), "1.0");
        assert_eq!(format_float(0.4375), "0.4375");
        assert_eq!(format_float(0.0), "0.0");
        assert_eq!(format_float(0.1), "0.1");
    }

    #[test]
    fn lists_round_trip() {
        let items = vec!["vanilla".to_owned(), "file/My Pack.zip".to_owned()];
        assert_eq!(parse_list(&format_list(&items)), items);
        assert_eq!(parse_list("[]"), Vec::<String>::new());
    }
}
