use super::*;

impl TextUi {
    /// Registers additional font bytes for rendering.
    ///
    /// This clears cached textures/input states so new faces are picked up.
    pub fn register_font_data(&mut self, bytes: Vec<u8>) {
        self.register_shared_font(Arc::new(bytes));
    }

    /// Registers font bytes that live for the whole program, such as an embedded font, without
    /// copying them. Every text worker shares the one allocation, which matters for large fonts
    /// (a CJK font alone is tens of megabytes).
    pub fn register_static_font_data(&mut self, bytes: &'static [u8]) {
        self.register_shared_font(Arc::new(bytes));
    }

    fn register_shared_font(&mut self, bytes: SharedFontBytes) {
        if let Some(tx) = self.async_raster.tx.as_ref() {
            let _ = tx.send(AsyncRasterWorkerMessage::RegisterFont(Arc::clone(&bytes)));
        }
        self.glyph_atlas.register_font(Arc::clone(&bytes));
        self.font_system
            .db_mut()
            .load_font_source(cosmic_text::fontdb::Source::Binary(bytes));
        self.invalidate_text_caches(true);
    }

    /// Applies font family/size/weight preferences for subsequent text renders.
    pub fn apply_typography(&mut self, family_candidates: &[&str], size_points: f32, weight: i32) {
        let family = self.resolve_family_candidate(family_candidates);
        let size_scale = (size_points / 18.0).clamp(0.50, 3.00);
        let clamped_weight = weight.clamp(100, 900);

        if self.ui_font_family == family
            && (self.ui_font_size_scale - size_scale).abs() <= f32::EPSILON
            && self.ui_font_weight == clamped_weight
        {
            return;
        }

        self.ui_font_family = family;
        self.ui_font_size_scale = size_scale;
        self.ui_font_weight = clamped_weight;
        self.invalidate_text_caches(false);
    }

    /// Whether the text engine can find a font family by this name.
    pub fn has_font_family(&self, name: &str) -> bool {
        self.resolve_family_candidate(&[name]).is_some()
    }

    /// Sets the launcher-wide fonts for CJK characters and emoji. A name that is not an installed
    /// family is ignored, leaving that script to cosmic-text's own fallback.
    pub fn apply_script_fonts(&mut self, cjk_candidates: &[&str], emoji_candidates: &[&str]) {
        let cjk = self.resolve_family_candidate(cjk_candidates);
        let emoji = self.resolve_family_candidate(emoji_candidates);
        if self.ui_cjk_font_family == cjk && self.ui_emoji_font_family == emoji {
            return;
        }
        self.ui_cjk_font_family = cjk;
        self.ui_emoji_font_family = emoji;
        self.invalidate_text_caches(false);
    }

    /// The CJK and emoji families to use for text with `fundamentals`: the group's own choice,
    /// else the launcher-wide preference.
    pub(crate) fn script_fonts<'a>(
        &'a self,
        fundamentals: &'a TextFundamentals,
    ) -> ScriptFonts<'a> {
        ScriptFonts::resolve(
            fundamentals.cjk_font_family.as_deref(),
            fundamentals.emoji_font_family.as_deref(),
            self.ui_cjk_font_family.as_deref(),
            self.ui_emoji_font_family.as_deref(),
        )
    }

    /// Enables/disables OpenType features and updates active tag selection.
    pub fn apply_open_type_features(
        &mut self,
        enabled: bool,
        feature_tags_csv: &str,
        family_candidates: &[&str],
    ) {
        let normalized_csv = feature_tags_csv.trim().to_owned();
        let parsed_tags = parse_feature_tag_list(&normalized_csv);
        let active_tags = if enabled {
            if parsed_tags.is_empty() {
                let default_tags = parse_feature_tag_list(DEFAULT_OPEN_TYPE_FEATURE_TAGS);
                if default_tags.is_empty() {
                    self.collect_available_feature_tags_for_family(family_candidates)
                } else {
                    default_tags
                }
            } else {
                parsed_tags
            }
        } else {
            Vec::new()
        };
        let active_features = if enabled && !active_tags.is_empty() {
            Some(build_font_features(&active_tags))
        } else {
            None
        };

        if self.open_type_features_enabled == enabled
            && self.open_type_features_to_enable == normalized_csv
            && self.open_type_feature_tags == active_tags
            && self.open_type_features == active_features
        {
            return;
        }

        self.open_type_features_enabled = enabled;
        self.open_type_features_to_enable = normalized_csv;
        self.open_type_feature_tags = active_tags;
        self.open_type_features = active_features;
        self.invalidate_text_caches(false);
    }

    pub(super) fn resolve_family_candidate(&self, family_candidates: &[&str]) -> Option<String> {
        for candidate in family_candidates {
            if self.font_system.db().faces().any(|face| {
                face.families
                    .iter()
                    .any(|(family, _)| family.eq_ignore_ascii_case(candidate))
            }) {
                return Some((*candidate).to_owned());
            }
        }
        None
    }

    pub(super) fn resolve_face_id_for_family(
        &self,
        family_candidates: &[&str],
    ) -> Option<cosmic_text::fontdb::ID> {
        for candidate in family_candidates {
            if let Some(face) = self.font_system.db().faces().find(|face| {
                face.families
                    .iter()
                    .any(|(family, _)| family.eq_ignore_ascii_case(candidate))
            }) {
                return Some(face.id);
            }
        }
        None
    }

    pub(super) fn collect_available_feature_tags_for_family(
        &self,
        family_candidates: &[&str],
    ) -> Vec<[u8; 4]> {
        let Some(face_id) = self.resolve_face_id_for_family(family_candidates) else {
            return Vec::new();
        };

        let mut tags = BTreeSet::new();
        let _ = self
            .font_system
            .db()
            .with_face_data(face_id, |font_data, face_index| {
                let Ok(face) = SkrifaFontRef::from_index(font_data, face_index) else {
                    return Some(());
                };

                if let Ok(gsub) = face.gsub() {
                    if let Ok(feature_list) = gsub.feature_list() {
                        for record in feature_list.feature_records().iter() {
                            tags.insert(record.feature_tag().into_bytes());
                        }
                    }
                }

                if let Ok(gpos) = face.gpos() {
                    if let Ok(feature_list) = gpos.feature_list() {
                        for record in feature_list.feature_records().iter() {
                            tags.insert(record.feature_tag().into_bytes());
                        }
                    }
                }

                Some(())
            });

        tags.into_iter().collect()
    }

    pub(super) fn hash_typography<H: Hasher>(&self, state: &mut H) {
        self.ui_font_family.hash(state);
        self.ui_cjk_font_family.hash(state);
        self.ui_emoji_font_family.hash(state);
        self.ui_font_size_scale.to_bits().hash(state);
        self.ui_font_weight.hash(state);
        self.open_type_features_enabled.hash(state);
        self.open_type_features_to_enable.hash(state);
        self.max_texture_side_px.hash(state);
    }

    pub(super) fn effective_font_size(&self, size_points: f32) -> f32 {
        (size_points * self.ui_font_size_scale).max(1.0)
    }

    pub(super) fn effective_line_height(&self, line_height_points: f32) -> f32 {
        (line_height_points * self.ui_font_size_scale).max(1.0)
    }

    pub(super) fn effective_weight(&self, base_weight: u16) -> u16 {
        let delta = self.ui_font_weight - 400;
        (i32::from(base_weight) + delta).clamp(100, 900) as u16
    }

    pub(super) fn build_text_attrs_owned(
        &self,
        style: &SpanStyle,
        font_size_points: f32,
        line_height_points: f32,
        fundamentals: &TextFundamentals,
    ) -> AttrsOwned {
        let mut attrs = Attrs::new()
            .color(to_cosmic_text_color(style.color))
            .weight(Weight(self.effective_weight(style.weight)))
            .metrics(Metrics::new(
                self.effective_font_size(font_size_points),
                self.effective_line_height(line_height_points),
            ));

        if style.monospace {
            attrs = attrs.family(Family::Monospace);
        } else if let Some(family) = fundamentals
            .font_family
            .as_deref()
            .or(self.ui_font_family.as_deref())
        {
            attrs = attrs.family(Family::Name(family));
        }

        if style.italic {
            attrs = attrs.style(FontStyle::Italic);
        }
        if let Some(features) = compose_font_features(&self.open_type_feature_tags, fundamentals) {
            attrs = attrs.font_features(features);
        }

        AttrsOwned::new(&attrs)
    }
}
