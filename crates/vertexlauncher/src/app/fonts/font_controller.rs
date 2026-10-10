use super::*;

pub struct FontController {
    pub(super) font_catalog: FontCatalog,
    pub(super) available_ui_fonts: Vec<UiFontFamily>,
    pub(super) available_ui_font_labels: Vec<String>,
    pub(super) available_emoji_fonts: Vec<UiEmojiFontFamily>,
    pub(super) available_emoji_font_labels: Vec<String>,
    pub(super) available_cjk_fonts: Vec<UiCjkFontFamily>,
    pub(super) available_cjk_font_labels: Vec<String>,
    /// The launcher-wide CJK and emoji fonts last handed to the text engine.
    pub(super) applied_script_fonts: Option<(UiCjkFontFamily, UiEmojiFontFamily)>,
    /// System families already offered to the text engine, so each is registered once.
    pub(super) registered_script_families: std::collections::HashSet<String>,
    pub(super) applied_font_signature: Option<AppliedFontSignature>,
    pub(super) applied_text_signature: Option<AppliedTextSignature>,
    pub(super) effective_ui_font_family: UiFontFamily,
}

impl FontController {
    pub fn new(initial_family: UiFontFamily) -> Self {
        let mut catalog = FontCatalog::new();
        catalog.load_system();
        let available_ui_fonts = detect_available_ui_fonts(&catalog);
        let available_ui_font_labels = available_ui_fonts
            .iter()
            .map(UiFontFamily::settings_label)
            .collect();
        let available_emoji_fonts = detect_available_emoji_fonts(&catalog);
        let available_emoji_font_labels = available_emoji_fonts
            .iter()
            .map(UiEmojiFontFamily::settings_label)
            .collect();

        let available_cjk_fonts = detect_available_cjk_fonts(&catalog);
        let available_cjk_font_labels = available_cjk_fonts
            .iter()
            .map(UiCjkFontFamily::settings_label)
            .collect();

        Self {
            available_cjk_fonts,
            available_cjk_font_labels,
            available_ui_fonts,
            available_ui_font_labels,
            available_emoji_fonts,
            available_emoji_font_labels,
            font_catalog: catalog,
            applied_font_signature: None,
            applied_text_signature: None,
            applied_script_fonts: None,
            registered_script_families: std::collections::HashSet::new(),
            effective_ui_font_family: initial_family,
        }
    }

    /// Registers the fonts built into the executable. They are static, so the text engine shares
    /// the embedded bytes instead of copying them, which matters for the CJK font (about 20 MB).
    pub fn register_included_fonts(text_ui: &mut TextUi) {
        text_ui.register_static_font_data(MAPLE_MONO_NF_REGULAR_TTF);
        text_ui.register_static_font_data(MAPLE_MONO_NF_CN_REGULAR_TTF);
        text_ui.register_static_font_data(NOTO_COLOR_EMOJI_TTF);
    }

    pub fn available_ui_fonts(&self) -> &[UiFontFamily] {
        &self.available_ui_fonts
    }

    pub fn available_ui_font_labels(&self) -> &[String] {
        &self.available_ui_font_labels
    }

    pub fn available_emoji_fonts(&self) -> &[UiEmojiFontFamily] {
        &self.available_emoji_fonts
    }

    pub fn available_emoji_font_labels(&self) -> &[String] {
        &self.available_emoji_font_labels
    }

    pub fn available_cjk_fonts(&self) -> &[UiCjkFontFamily] {
        &self.available_cjk_fonts
    }

    pub fn available_cjk_font_labels(&self) -> &[String] {
        &self.available_cjk_font_labels
    }

    /// Makes sure the text engine can find `family_name`, handing it the system font's bytes the
    /// first time if it cannot. Included fonts are registered at startup and need nothing.
    fn ensure_script_family_registered(&mut self, text_ui: &mut TextUi, family_name: &str) {
        if text_ui.has_font_family(family_name)
            || !self
                .registered_script_families
                .insert(family_name.to_lowercase())
        {
            return;
        }
        let families = [family_name];
        let spec = FontSpec::new(&families)
            .weight(Weight(400))
            .slant(Slant::Upright)
            .stretch(Stretch::Normal);
        match self.font_catalog.query_bytes(&spec) {
            Ok((bytes, _)) => text_ui.register_font_data(bytes),
            Err(_) => tracing::warn!(
                target: "vertexlauncher/app/fonts",
                configured_font = family_name,
                "configured CJK/emoji font not available; the text engine will fall back"
            ),
        }
    }

    /// Hands the launcher-wide CJK and emoji fonts to the text engine, and makes sure any font a
    /// text group names for itself can be found.
    fn apply_script_fonts(&mut self, config: &Config, text_ui: &mut TextUi) {
        for role in config::TextRole::ALL {
            let role_override = config.typography().override_for(role);
            for family in [
                role_override.cjk_font_family,
                role_override.emoji_font_family,
            ]
            .into_iter()
            .flatten()
            {
                self.ensure_script_family_registered(text_ui, &family);
            }
        }

        let desired = (config.ui_cjk_font_family(), config.ui_emoji_font_family());
        if self.applied_script_fonts.as_ref() == Some(&desired) {
            return;
        }
        self.ensure_script_family_registered(text_ui, desired.0.family_name());
        self.ensure_script_family_registered(text_ui, desired.1.family_name());
        text_ui.apply_script_fonts(&[desired.0.family_name()], &[desired.1.family_name()]);
        self.applied_script_fonts = Some(desired);
    }

    pub fn ensure_selected_font_is_available(&self, config: &mut Config) {
        let available_ui_fonts = &self.available_ui_fonts;
        config.for_each_dropdown_mut(|setting, value| {
            if setting.id != DropdownSettingId::UiFontFamily {
                return;
            }

            if let Some(matching_font) = matching_available_font(available_ui_fonts, value) {
                if matching_font != value {
                    *value = matching_font.clone();
                }
            } else {
                *value = UiFontFamily::included_default();
            }
        });
    }

    pub fn apply_from_config(
        &mut self,
        ctx: &egui::Context,
        config: &Config,
        text_ui: &mut TextUi,
    ) {
        let desired_font = AppliedFontSignature {
            family: config.ui_font_family(),
            size: config.ui_font_size(),
            weight: config.ui_font_weight(),
        };

        if self.applied_font_signature.as_ref() != Some(&desired_font) {
            let should_register_text_font = self
                .applied_font_signature
                .as_ref()
                .is_none_or(|previous| !previous.family.matches(&desired_font.family));
            let mut applied_family = desired_font.family.clone();
            if desired_font.family.is_included_default() {
                install_included_maple_font(ctx, desired_font.size);
            } else {
                let family_candidates = desired_font.family.query_families();
                let spec = FontSpec::new(&family_candidates)
                    .weight(Weight(desired_font.weight.clamp(100, 900) as u16))
                    .slant(Slant::Upright)
                    .stretch(Stretch::Normal);

                if let Ok((bytes, _face_index)) = self.font_catalog.query_bytes(&spec) {
                    if should_register_text_font {
                        text_ui.register_font_data(bytes.clone());
                    }
                    let font_key = font_key(&desired_font.family);
                    fontloader::egui_integration::install_font_as_primary(
                        ctx,
                        &font_key,
                        bytes,
                        desired_font.size,
                    );
                } else {
                    tracing::warn!(
                        target: "vertexlauncher/app/fonts",
                        configured_font = desired_font.family.label(),
                        "configured UI font not available; falling back to included default"
                    );
                    install_included_maple_font(ctx, desired_font.size);
                    applied_family = UiFontFamily::included_default();
                }
            }

            self.effective_ui_font_family = applied_family;
            self.applied_font_signature = Some(desired_font);
        }

        self.apply_script_fonts(config, text_ui);

        let desired_text = AppliedTextSignature {
            family: self.effective_ui_font_family.clone(),
            size: config.ui_font_size(),
            weight: config.ui_font_weight(),
            open_type_features_enabled: config.open_type_features_enabled(),
            open_type_features_to_enable: config.open_type_features_to_enable().to_owned(),
        };
        let open_type_features_changed =
            self.applied_text_signature
                .as_ref()
                .is_some_and(|previous| {
                    previous.open_type_features_enabled != desired_text.open_type_features_enabled
                        || previous.open_type_features_to_enable
                            != desired_text.open_type_features_to_enable
                });

        if self.applied_text_signature == Some(desired_text.clone()) {
            return;
        }

        let family_candidates = self.effective_ui_font_family.query_families();
        text_ui.apply_typography(&family_candidates, desired_text.size, desired_text.weight);
        text_ui.apply_open_type_features(
            desired_text.open_type_features_enabled,
            &desired_text.open_type_features_to_enable,
            &family_candidates,
        );
        self.applied_text_signature = Some(desired_text);
        if open_type_features_changed {
            console::mark_text_for_redraw();
            ctx.request_repaint();
        }
    }
}
