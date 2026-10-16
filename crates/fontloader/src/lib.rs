//! runtime_fontloader
//!
//! Runtime discovery + querying + extraction of system fonts using `fontdb`.
//!
//! - Scans system font directories (best-effort).
//! - Queries faces using CSS-like parameters.
//! - Extracts owned bytes for a matched face (plus face index for TTC/OTC).

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub use fontdb::ID as FaceId;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("no matching font found")]
    NotFound,
    #[error("font face data unavailable")]
    NoData,
}

/// Font weight (CSS-like, 100..=900).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Weight(pub u16);

impl Weight {
    pub const THIN: Self = Self(100);
    pub const EXTRA_LIGHT: Self = Self(200);
    pub const LIGHT: Self = Self(300);
    pub const REGULAR: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMI_BOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const EXTRA_BOLD: Self = Self(800);
    pub const BLACK: Self = Self(900);
}

/// Font style (slant).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slant {
    Upright,
    Italic,
    Oblique,
}

/// Font stretch (width).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

/// Query specification for selecting a single best-matching face.
#[derive(Clone, Debug)]
pub struct FontSpec<'a> {
    /// Ordered fallback list (first wins if multiple match equally).
    pub families: &'a [&'a str],
    pub weight: Weight,
    pub slant: Slant,
    pub stretch: Stretch,
}

impl<'a> FontSpec<'a> {
    /// Creates a query with default style parameters and ordered family fallbacks.
    pub fn new(families: &'a [&'a str]) -> Self {
        Self {
            families,
            weight: Weight::REGULAR,
            slant: Slant::Upright,
            stretch: Stretch::Normal,
        }
    }

    /// Sets desired font weight.
    pub fn weight(mut self, w: Weight) -> Self {
        self.weight = w;
        self
    }

    /// Sets desired slant/style.
    pub fn slant(mut self, s: Slant) -> Self {
        self.slant = s;
        self
    }

    /// Sets desired stretch/width.
    pub fn stretch(mut self, s: Stretch) -> Self {
        self.stretch = s;
        self
    }
}

/// A runtime system-font catalog (faces + metadata + sources).
pub struct FontCatalog {
    db: fontdb::Database,
}

impl fmt::Debug for FontCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontCatalog").finish_non_exhaustive()
    }
}

impl Default for FontCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl FontCatalog {
    /// Create an empty catalog.
    pub fn new() -> Self {
        Self {
            db: fontdb::Database::new(),
        }
    }

    /// Scan common system font directories (Windows/macOS/Linux).
    pub fn load_system(&mut self) {
        self.db.load_system_fonts();

        #[cfg(target_os = "linux")]
        {
            for host_dir in [
                "/run/host/fonts",
                "/run/host/local-fonts",
                "/run/host/user-fonts",
            ] {
                let path = std::path::Path::new(host_dir);
                if path.exists() {
                    self.db.load_fonts_dir(path);
                }
            }
        }
    }

    /// Load fonts from a directory (recursively).
    pub fn load_dir<P: AsRef<std::path::Path>>(&mut self, dir: P) {
        self.db.load_fonts_dir(dir);
    }

    /// Return all known family names (distinct).
    ///
    /// `fontdb::FaceInfo` stores `families: Vec<(String, Language)>`, not a single `family`.
    /// We include *all* localized family names. :contentReference[oaicite:1]{index=1}
    pub fn family_names(&self) -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        for face in self.db.faces() {
            for (name, _lang) in &face.families {
                out.insert(name.as_str());
            }
        }
        out
    }

    /// Return a deduplicated list of family names suitable for UI selectors.
    ///
    /// `fontdb` stores multiple faces per family and may expose localized names.
    /// For the selector we prefer the primary family name from each face and
    /// collapse duplicates case-insensitively.
    /// Family names (sorted, deduplicated) whose first face has a glyph for at least one of
    /// `probes`.
    ///
    /// Only each family's first face is checked, which is almost always the regular weight and
    /// keeps the scan to one lookup per family. Choose probes that only fonts meant for the
    /// script contain: a Han ideograph, a kana, a Hangul syllable, an emoji.
    pub fn families_covering_any(&self, probes: &[char]) -> Vec<String> {
        let mut covered: BTreeSet<String> = BTreeSet::new();
        let mut checked: BTreeSet<String> = BTreeSet::new();
        for face in self.db.faces() {
            let Some((family, _)) = face.families.first() else {
                continue;
            };
            if !checked.insert(family.clone()) {
                continue;
            }
            let has_glyph = self
                .db
                .with_face_data(face.id, |data, index| {
                    skrifa::FontRef::from_index(data, index)
                        .map(|font| {
                            use skrifa::MetadataProvider;
                            let charmap = font.charmap();
                            probes.iter().any(|probe| charmap.map(*probe).is_some())
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if has_glyph {
                covered.insert(family.clone());
            }
        }
        covered.into_iter().collect()
    }

    pub fn deduplicated_family_names(&self) -> Vec<String> {
        let mut out = BTreeMap::new();
        for face in self.db.faces() {
            let Some((name, _lang)) = face.families.first() else {
                continue;
            };
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            out.entry(normalized_family_key(trimmed))
                .or_insert_with(|| trimmed.to_owned());
        }
        out.into_values().collect()
    }

    /// Return the best-matching face id for the given spec.
    pub fn query(&self, spec: &FontSpec<'_>) -> Result<FaceId, FontError> {
        let families: Vec<fontdb::Family<'_>> = spec
            .families
            .iter()
            .copied()
            .map(fontdb::Family::Name)
            .collect();

        let weight = fontdb::Weight(spec.weight.0);
        let style = match spec.slant {
            Slant::Upright => fontdb::Style::Normal,
            Slant::Italic => fontdb::Style::Italic,
            Slant::Oblique => fontdb::Style::Oblique,
        };
        let stretch = match spec.stretch {
            Stretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
            Stretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
            Stretch::Condensed => fontdb::Stretch::Condensed,
            Stretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
            Stretch::Normal => fontdb::Stretch::Normal,
            Stretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
            Stretch::Expanded => fontdb::Stretch::Expanded,
            Stretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
            Stretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
        };

        let q = fontdb::Query {
            families: &families,
            weight,
            stretch,
            style,
        };

        self.db.query(&q).ok_or(FontError::NotFound)
    }

    /// Copy the *entire* font binary backing this face to owned bytes.
    ///
    /// Returns `(bytes, face_index)` where `face_index` is the face index within TTC/OTC collections.
    pub fn face_bytes(&self, id: FaceId) -> Result<(Vec<u8>, u32), FontError> {
        self.db
            .with_face_data(id, |data, face_index| (data.to_vec(), face_index))
            .ok_or(FontError::NoData)
    }

    /// Convenience: query + get bytes in one call.
    pub fn query_bytes(&self, spec: &FontSpec<'_>) -> Result<(Vec<u8>, u32), FontError> {
        let id = self.query(spec)?;
        self.face_bytes(id)
    }

    /// Expose the underlying database for advanced inspection.
    pub fn db(&self) -> &fontdb::Database {
        &self.db
    }

    /// Mutable access to the underlying database for advanced loading/querying.
    pub fn db_mut(&mut self) -> &mut fontdb::Database {
        &mut self.db
    }
}

fn normalized_family_key(name: &str) -> String {
    name.trim().to_lowercase()
}
