//! Key names: LWJGL 2 numeric codes (Minecraft ≤ 1.12) versus GLFW-style strings (≥ 1.13).

/// `(LWJGL 2 code, 1.13+ name)`.
static KEYS: &[(i32, &str)] = &[
    (0, "key.keyboard.unknown"),
    (1, "key.keyboard.escape"),
    (2, "key.keyboard.1"),
    (3, "key.keyboard.2"),
    (4, "key.keyboard.3"),
    (5, "key.keyboard.4"),
    (6, "key.keyboard.5"),
    (7, "key.keyboard.6"),
    (8, "key.keyboard.7"),
    (9, "key.keyboard.8"),
    (10, "key.keyboard.9"),
    (11, "key.keyboard.0"),
    (12, "key.keyboard.minus"),
    (13, "key.keyboard.equal"),
    (14, "key.keyboard.backspace"),
    (15, "key.keyboard.tab"),
    (16, "key.keyboard.q"),
    (17, "key.keyboard.w"),
    (18, "key.keyboard.e"),
    (19, "key.keyboard.r"),
    (20, "key.keyboard.t"),
    (21, "key.keyboard.y"),
    (22, "key.keyboard.u"),
    (23, "key.keyboard.i"),
    (24, "key.keyboard.o"),
    (25, "key.keyboard.p"),
    (26, "key.keyboard.left.bracket"),
    (27, "key.keyboard.right.bracket"),
    (28, "key.keyboard.enter"),
    (29, "key.keyboard.left.control"),
    (30, "key.keyboard.a"),
    (31, "key.keyboard.s"),
    (32, "key.keyboard.d"),
    (33, "key.keyboard.f"),
    (34, "key.keyboard.g"),
    (35, "key.keyboard.h"),
    (36, "key.keyboard.j"),
    (37, "key.keyboard.k"),
    (38, "key.keyboard.l"),
    (39, "key.keyboard.semicolon"),
    (40, "key.keyboard.apostrophe"),
    (41, "key.keyboard.grave.accent"),
    (42, "key.keyboard.left.shift"),
    (43, "key.keyboard.backslash"),
    (44, "key.keyboard.z"),
    (45, "key.keyboard.x"),
    (46, "key.keyboard.c"),
    (47, "key.keyboard.v"),
    (48, "key.keyboard.b"),
    (49, "key.keyboard.n"),
    (50, "key.keyboard.m"),
    (51, "key.keyboard.comma"),
    (52, "key.keyboard.period"),
    (53, "key.keyboard.slash"),
    (54, "key.keyboard.right.shift"),
    (55, "key.keyboard.keypad.multiply"),
    (56, "key.keyboard.left.alt"),
    (57, "key.keyboard.space"),
    (58, "key.keyboard.caps.lock"),
    (59, "key.keyboard.f1"),
    (60, "key.keyboard.f2"),
    (61, "key.keyboard.f3"),
    (62, "key.keyboard.f4"),
    (63, "key.keyboard.f5"),
    (64, "key.keyboard.f6"),
    (65, "key.keyboard.f7"),
    (66, "key.keyboard.f8"),
    (67, "key.keyboard.f9"),
    (68, "key.keyboard.f10"),
    (69, "key.keyboard.num.lock"),
    (70, "key.keyboard.scroll.lock"),
    (71, "key.keyboard.keypad.7"),
    (72, "key.keyboard.keypad.8"),
    (73, "key.keyboard.keypad.9"),
    (74, "key.keyboard.keypad.subtract"),
    (75, "key.keyboard.keypad.4"),
    (76, "key.keyboard.keypad.5"),
    (77, "key.keyboard.keypad.6"),
    (78, "key.keyboard.keypad.add"),
    (79, "key.keyboard.keypad.1"),
    (80, "key.keyboard.keypad.2"),
    (81, "key.keyboard.keypad.3"),
    (82, "key.keyboard.keypad.0"),
    (83, "key.keyboard.keypad.decimal"),
    (87, "key.keyboard.f11"),
    (88, "key.keyboard.f12"),
    (100, "key.keyboard.f13"),
    (101, "key.keyboard.f14"),
    (102, "key.keyboard.f15"),
    (156, "key.keyboard.keypad.enter"),
    (157, "key.keyboard.right.control"),
    (181, "key.keyboard.keypad.divide"),
    (183, "key.keyboard.print.screen"),
    (184, "key.keyboard.right.alt"),
    (197, "key.keyboard.pause"),
    (199, "key.keyboard.home"),
    (200, "key.keyboard.up"),
    (201, "key.keyboard.page.up"),
    (203, "key.keyboard.left"),
    (205, "key.keyboard.right"),
    (207, "key.keyboard.end"),
    (208, "key.keyboard.down"),
    (209, "key.keyboard.page.down"),
    (210, "key.keyboard.insert"),
    (211, "key.keyboard.delete"),
    (219, "key.keyboard.left.win"),
    (220, "key.keyboard.right.win"),
    (221, "key.keyboard.menu"),
];

/// Converts a legacy numeric binding (`17`, `-100`) to the modern name.
pub fn legacy_to_modern(code: i32) -> Option<String> {
    if code <= -90 {
        // -100 left, -99 right, -98 middle, then buttons 4+.
        return Some(match code {
            -100 => "key.mouse.left".to_owned(),
            -99 => "key.mouse.right".to_owned(),
            -98 => "key.mouse.middle".to_owned(),
            other => format!("key.mouse.{}", other + 101),
        });
    }
    KEYS.iter()
        .find(|(legacy, _)| *legacy == code)
        .map(|(_, name)| (*name).to_owned())
}

/// Converts a modern binding name to the legacy numeric code.
pub fn modern_to_legacy(name: &str) -> Option<i32> {
    match name {
        "key.mouse.left" => return Some(-100),
        "key.mouse.right" => return Some(-99),
        "key.mouse.middle" => return Some(-98),
        _ => {}
    }
    if let Some(button) = name.strip_prefix("key.mouse.") {
        return button.parse::<i32>().ok().map(|n| n - 101);
    }
    KEYS.iter()
        .find(|(_, modern)| *modern == name)
        .map(|(legacy, _)| *legacy)
}

/// All keyboard names, for pickers.
pub fn all_keyboard_names() -> impl Iterator<Item = &'static str> {
    KEYS.iter().map(|(_, name)| *name)
}

/// Human label for a modern binding name (`key.keyboard.left.shift` → `Left Shift`).
pub fn display_name(name: &str) -> String {
    let (kind, rest) = if let Some(rest) = name.strip_prefix("key.keyboard.") {
        ("", rest)
    } else if let Some(rest) = name.strip_prefix("key.mouse.") {
        ("Mouse ", rest)
    } else {
        return name.to_owned();
    };
    if rest == "unknown" {
        return "Not Bound".to_owned();
    }
    let words: Vec<String> = rest
        .split('.')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    format!("{kind}{}", words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_both_ways() {
        assert_eq!(legacy_to_modern(17).as_deref(), Some("key.keyboard.w"));
        assert_eq!(legacy_to_modern(-100).as_deref(), Some("key.mouse.left"));
        assert_eq!(legacy_to_modern(-97).as_deref(), Some("key.mouse.4"));
        assert_eq!(modern_to_legacy("key.keyboard.left.shift"), Some(42));
        assert_eq!(modern_to_legacy("key.mouse.4"), Some(-97));
        assert_eq!(modern_to_legacy("key.keyboard.f25"), None);
        assert_eq!(display_name("key.keyboard.left.shift"), "Left Shift");
        assert_eq!(display_name("key.mouse.middle"), "Mouse Middle");
    }

    #[test]
    fn table_has_no_duplicates() {
        let mut codes = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for (code, name) in KEYS {
            assert!(codes.insert(*code) && names.insert(*name), "{name}");
        }
    }
}
