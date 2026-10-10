use super::*;

/// The font used for CJK (Chinese, Japanese, Korean) characters, launcher-wide or for one text
/// group. Mirrors [`UiEmojiFontFamily`]: the included Maple Mono NF CN is the default.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UiCjkFontFamily(pub(super) String);

impl UiCjkFontFamily {
    pub fn included_default() -> Self {
        Self(INCLUDED_CJK_FONT_SETTINGS_LABEL.to_owned())
    }

    pub fn new(name: impl Into<String>) -> Self {
        Self(normalize_cjk_font_family_name(name.into()))
    }

    pub fn is_included_default(&self) -> bool {
        let key = normalize_cjk_font_family_key(&self.0);
        key == normalize_cjk_font_family_key(INCLUDED_CJK_FONT_SETTINGS_LABEL)
            || key == normalize_cjk_font_family_key(INCLUDED_DEFAULT_CJK_FONT_FAMILY)
    }

    pub fn label(&self) -> &str {
        &self.0
    }

    pub fn settings_label(&self) -> String {
        if self.is_included_default() {
            INCLUDED_CJK_FONT_SETTINGS_LABEL.to_owned()
        } else {
            self.0.clone()
        }
    }

    /// The installed family name to ask the text engine for.
    pub fn family_name(&self) -> &str {
        if self.is_included_default() {
            INCLUDED_DEFAULT_CJK_FONT_FAMILY
        } else {
            &self.0
        }
    }

    pub fn matches(&self, other: &Self) -> bool {
        normalize_cjk_font_family_key(self.family_name())
            == normalize_cjk_font_family_key(other.family_name())
    }

    pub fn normalize(&mut self) {
        self.0 = normalize_cjk_font_family_name(std::mem::take(&mut self.0));
    }
}

impl Serialize for UiCjkFontFamily {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.settings_label())
    }
}

impl<'de> Deserialize<'de> for UiCjkFontFamily {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::new(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_included_font_is_recognised_under_either_name() {
        for name in [
            "",
            "Maple Mono NF CN",
            "maple mono nf cn (included)",
            "  Maple Mono NF CN ",
        ] {
            let family = UiCjkFontFamily::new(name);
            assert!(family.is_included_default(), "{name:?}");
            assert_eq!(family.family_name(), "Maple Mono NF CN");
            assert_eq!(family.settings_label(), "Maple Mono NF CN (Included)");
        }
    }

    #[test]
    fn other_families_are_kept_as_typed() {
        let family = UiCjkFontFamily::new(" Noto Sans CJK JP ");
        assert!(!family.is_included_default());
        assert_eq!(family.family_name(), "Noto Sans CJK JP");
        assert!(family.matches(&UiCjkFontFamily::new("noto sans cjk jp")));
    }

    #[test]
    fn serializes_as_the_settings_label_and_round_trips() {
        let json = serde_json::to_string(&UiCjkFontFamily::included_default()).unwrap();
        assert_eq!(json, "\"Maple Mono NF CN (Included)\"");
        let back: UiCjkFontFamily = serde_json::from_str(&json).unwrap();
        assert!(back.is_included_default());
    }
}
