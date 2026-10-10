pub(super) fn sanitize_float_text(text: &mut String, allow_negative: bool) {
    if text.is_empty() {
        return;
    }

    let mut out = String::with_capacity(text.len());
    let mut seen_dot = false;
    let mut seen_sign = false;

    for (index, ch) in text.chars().enumerate() {
        if ch.is_ascii_digit() {
            out.push(ch);
            continue;
        }

        if ch == '.' && !seen_dot {
            seen_dot = true;
            out.push(ch);
            continue;
        }

        if allow_negative && ch == '-' && index == 0 && !seen_sign {
            seen_sign = true;
            out.push(ch);
        }
    }

    *text = out;
}

pub(super) fn sanitize_int_text(text: &mut String, allow_negative: bool) {
    if text.is_empty() {
        return;
    }

    let mut out = String::with_capacity(text.len());
    let mut seen_sign = false;

    for (index, ch) in text.chars().enumerate() {
        if ch.is_ascii_digit() {
            out.push(ch);
            continue;
        }

        if allow_negative && ch == '-' && index == 0 && !seen_sign {
            seen_sign = true;
            out.push(ch);
        }
    }

    *text = out;
}

pub(super) fn sanitize_u128_text(text: &mut String) {
    if text.is_empty() {
        return;
    }
    text.retain(|ch| ch.is_ascii_digit());
}

pub(super) fn parse_float_text(text: &str) -> Option<f32> {
    if text.is_empty() || text == "-" || text == "." || text == "-." {
        None
    } else {
        text.parse::<f32>().ok()
    }
}

pub(super) fn parse_int_text(text: &str) -> Option<i32> {
    if text.is_empty() || text == "-" {
        None
    } else {
        text.parse::<i32>().ok()
    }
}

pub(super) fn parse_u128_text(text: &str) -> Option<u128> {
    if text.is_empty() {
        None
    } else {
        text.parse::<u128>().ok()
    }
}

pub(super) fn format_float(value: f32) -> String {
    let mut formatted = format!("{value:.3}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}
