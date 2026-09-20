//! Resource pack format handling: reading `pack.mcmeta` in all its historical shapes,
//! normalizing it into one range, and checking it against game versions.

use std::fmt;

use serde_json::Value;

use crate::version::Ver;

/// A pack format with its minor version (`69.0`, `97.1`). Minors exist since 1.21.9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackFormat {
    pub major: u32,
    pub minor: u32,
}

impl PackFormat {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

/// The formats a pack declares support for, normalized from every metadata style:
/// `pack_format`, `supported_formats` (number, `[min,max]`, or `{min_inclusive,max_inclusive}`)
/// and `min_format`/`max_format` (number or `[major,minor]`).
///
/// An omitted minor means "any": `0` on the low end, unbounded on the high end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackSupport {
    pub min: PackFormat,
    pub max: PackFormat,
}

const ANY_MINOR: u32 = u32::MAX;

impl PackSupport {
    pub fn supports(&self, format: PackFormat) -> bool {
        format >= self.min && format <= self.max
    }

    /// Compact, stable text form: `34`, `15..999`, `65..97.1`. Round-trips through
    /// [`PackSupport::parse`], so it is safe to store and compare.
    pub fn serialize(&self) -> String {
        let low = if self.min.minor == 0 {
            self.min.major.to_string()
        } else {
            format!("{}.{}", self.min.major, self.min.minor)
        };
        let high = if self.max.minor == ANY_MINOR {
            self.max.major.to_string()
        } else {
            format!("{}.{}", self.max.major, self.max.minor)
        };
        if low == high {
            low
        } else {
            format!("{low}..{high}")
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (low, high) = text.split_once("..").unwrap_or((text, text));
        let parse_end = |part: &str, is_max: bool| -> Option<PackFormat> {
            let (major, minor) = match part.split_once('.') {
                Some((major, minor)) => (major, Some(minor.parse::<u32>().ok()?)),
                None => (part, None),
            };
            Some(PackFormat::new(
                major.trim().parse().ok()?,
                minor.unwrap_or(if is_max { ANY_MINOR } else { 0 }),
            ))
        };
        Some(Self {
            min: parse_end(low, false)?,
            max: parse_end(high, true)?,
        })
    }
}

impl fmt::Display for PackSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.serialize())
    }
}

/// `pack.mcmeta` may carry trailing comments or junk after the JSON object; read only the
/// first value.
fn first_json_value(text: &str) -> Option<Value> {
    let text = text.trim_start_matches('\u{feff}');
    serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .next()?
        .ok()
}

fn format_from_value(value: &Value, is_max: bool) -> Option<PackFormat> {
    match value {
        Value::Number(n) => Some(PackFormat::new(
            u32::try_from(n.as_u64()?).ok()?,
            if is_max { ANY_MINOR } else { 0 },
        )),
        Value::Array(items) => {
            let major = u32::try_from(items.first()?.as_u64()?).ok()?;
            let minor = items
                .get(1)
                .and_then(Value::as_u64)
                .and_then(|m| u32::try_from(m).ok())
                .unwrap_or(if is_max { ANY_MINOR } else { 0 });
            Some(PackFormat::new(major, minor))
        }
        _ => None,
    }
}

/// Reads the support range from `pack.mcmeta` text. Newer keys win over older ones, the same
/// order the game uses.
pub fn parse_pack_mcmeta(text: &str) -> Option<PackSupport> {
    let root = first_json_value(text)?;
    let pack = root.get("pack")?;

    if let (Some(min), Some(max)) = (pack.get("min_format"), pack.get("max_format"))
        && let (Some(min), Some(max)) =
            (format_from_value(min, false), format_from_value(max, true))
    {
        return Some(PackSupport { min, max });
    }
    if let Some(supported) = pack.get("supported_formats") {
        let range = match supported {
            Value::Object(map) => (
                map.get("min_inclusive")
                    .and_then(|v| format_from_value(v, false)),
                map.get("max_inclusive")
                    .and_then(|v| format_from_value(v, true)),
            ),
            Value::Array(items) if items.len() >= 2 => (
                format_from_value(&items[0], false),
                format_from_value(&items[1], true),
            ),
            single => (
                format_from_value(single, false),
                format_from_value(single, true),
            ),
        };
        if let (Some(min), Some(max)) = range {
            return Some(PackSupport { min, max });
        }
    }
    let single = pack.get("pack_format")?;
    Some(PackSupport {
        min: format_from_value(single, false)?,
        max: format_from_value(single, true)?,
    })
}

/// First game version of each resource pack format (Minecraft Wiki "Pack format").
static RESOURCE_FORMATS: &[(Ver, PackFormat)] = &[
    (Ver(1, 6, 1), PackFormat::new(1, 0)),
    (Ver(1, 9, 0), PackFormat::new(2, 0)),
    (Ver(1, 11, 0), PackFormat::new(3, 0)),
    (Ver(1, 13, 0), PackFormat::new(4, 0)),
    (Ver(1, 15, 0), PackFormat::new(5, 0)),
    (Ver(1, 16, 2), PackFormat::new(6, 0)),
    (Ver(1, 17, 0), PackFormat::new(7, 0)),
    (Ver(1, 18, 0), PackFormat::new(8, 0)),
    (Ver(1, 19, 0), PackFormat::new(9, 0)),
    (Ver(1, 19, 3), PackFormat::new(12, 0)),
    (Ver(1, 19, 4), PackFormat::new(13, 0)),
    (Ver(1, 20, 0), PackFormat::new(15, 0)),
    (Ver(1, 20, 2), PackFormat::new(18, 0)),
    (Ver(1, 20, 3), PackFormat::new(22, 0)),
    (Ver(1, 20, 5), PackFormat::new(32, 0)),
    (Ver(1, 21, 0), PackFormat::new(34, 0)),
    (Ver(1, 21, 2), PackFormat::new(42, 0)),
    (Ver(1, 21, 4), PackFormat::new(46, 0)),
    (Ver(1, 21, 5), PackFormat::new(55, 0)),
    (Ver(1, 21, 6), PackFormat::new(63, 0)),
    (Ver(1, 21, 7), PackFormat::new(64, 0)),
    (Ver(1, 21, 9), PackFormat::new(69, 0)),
    (Ver(1, 21, 11), PackFormat::new(75, 0)),
    (Ver(26, 1, 0), PackFormat::new(84, 0)),
    (Ver(26, 2, 0), PackFormat::new(88, 0)),
    (Ver(26, 3, 0), PackFormat::new(97, 1)),
];

/// Resource pack format a game version uses; `None` before 1.6.1 or for unplaceable versions.
pub fn resource_format_for(version: Ver) -> Option<PackFormat> {
    RESOURCE_FORMATS
        .iter()
        .rev()
        .find(|(start, _)| version >= *start)
        .map(|(_, format)| *format)
}

/// Whether a pack is expected to work in a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compat {
    Compatible,
    /// The pack's declared range doesn't include the version's format.
    Incompatible {
        version_format: PackFormat,
    },
    /// The pack has no readable metadata, or the version's format is unknown.
    Unknown,
}

pub fn compatibility(support: Option<PackSupport>, version: Option<Ver>) -> Compat {
    let (Some(support), Some(version_format)) = (support, version.and_then(resource_format_for))
    else {
        return Compat::Unknown;
    };
    if support.supports(version_format) {
        Compat::Compatible
    } else {
        Compat::Incompatible { version_format }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn support(text: &str) -> PackSupport {
        parse_pack_mcmeta(text).unwrap()
    }

    #[test]
    fn reads_every_metadata_style_seen_in_the_wild() {
        // Legacy single format.
        assert_eq!(support(r#"{"pack":{"pack_format":34}}"#).serialize(), "34");
        // supported_formats as an object, alongside min/max (newest wins).
        let s = support(
            r#"{"pack":{"pack_format":55,"supported_formats":{"min_inclusive":55,"max_inclusive":64},"min_format":55,"max_format":99}}"#,
        );
        assert_eq!(s.serialize(), "55..99");
        // Array min/max with minors, plus a legacy array.
        let s = support(
            r#"{"pack":{"pack_format":34,"min_format":[15,0],"max_format":[1000,0],"supported_formats":[15,1000]}}"#,
        );
        assert_eq!(s, PackSupport::parse("15..1000.0").unwrap());
        // supported_formats alone, as an array and as a single number.
        assert_eq!(
            support(r#"{"pack":{"pack_format":9,"supported_formats":[9,12]}}"#).serialize(),
            "9..12"
        );
        assert_eq!(
            support(r#"{"pack":{"pack_format":9,"supported_formats":10}}"#).serialize(),
            "10"
        );
        // Trailing comment after the JSON, and a BOM.
        assert_eq!(
            support("\u{feff}{\"pack\":{\"pack_format\":15}}\n//Made by someone\n").serialize(),
            "15"
        );
        assert!(parse_pack_mcmeta("not json").is_none());
        assert!(parse_pack_mcmeta(r#"{"other":1}"#).is_none());
    }

    #[test]
    fn serialization_round_trips_and_treats_missing_minor_as_any() {
        for text in ["34", "15..999", "65..97.1", "69.1..75", "1"] {
            assert_eq!(PackSupport::parse(text).unwrap().serialize(), text);
        }
        let s = PackSupport::parse("69..75").unwrap();
        assert!(s.supports(PackFormat::new(69, 0)) && s.supports(PackFormat::new(75, 3)));
        assert!(!s.supports(PackFormat::new(76, 0)) && !s.supports(PackFormat::new(68, 9)));
    }

    #[test]
    fn versions_map_to_formats_and_compatibility() {
        assert_eq!(
            resource_format_for(Ver(1, 21, 4)),
            Some(PackFormat::new(46, 0))
        );
        assert_eq!(
            resource_format_for(Ver(1, 21, 8)),
            Some(PackFormat::new(64, 0))
        );
        assert_eq!(
            resource_format_for(Ver(26, 3, 0)),
            Some(PackFormat::new(97, 1))
        );
        assert_eq!(resource_format_for(Ver(1, 5, 2)), None);
        let spbr = PackSupport::parse("65..120").unwrap();
        assert_eq!(
            compatibility(Some(spbr), Some(Ver(26, 3, 0))),
            Compat::Compatible
        );
        assert!(matches!(
            compatibility(Some(spbr), Some(Ver(1, 21, 4))),
            Compat::Incompatible { .. }
        ));
        assert_eq!(compatibility(None, Some(Ver(1, 21, 4))), Compat::Unknown);
        assert_eq!(compatibility(Some(spbr), None), Compat::Unknown);
    }
}
