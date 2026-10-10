//! The language-specific half of natural-language output.
//!
//! What differs between languages falls into two groups:
//!
//! - **Mechanics**, which are the same for every sentence: how a list is joined ("A, B, and C"
//!   against "A、B、C"), which plural form a number selects, how a number is grouped and
//!   punctuated, how an ordinal is written, and how a duration or size is phrased. [`Locale`]
//!   implements these directly.
//! - **Vocabulary and grammar**, which belong to the words being used: the plural of a noun, its
//!   gender, an article. Those cannot be derived by rule across languages, so a localized message
//!   supplies them from its own translation catalog, as [`PluralForms`] (one string per plural
//!   category) selected with [`Locale::select`].
//!
//! English-only helpers (rule-based pluralization, "a"/"an", possessives) stay in the parent
//! module because they are English grammar, not locale mechanics.
//!
//! The non-English data here covers the mechanics and has had no review by native speakers. It
//! follows CLDR conventions where they are well known; treat it as a starting point to review
//! before shipping a translation.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// The largest count spelled out as a word; anything above it reads as "several" (or the
/// language's equivalent).
pub const MAX_COUNT_WORD: usize = 10;

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;

/// A language the natural-language helpers can speak.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Locale {
    #[default]
    English,
    Spanish,
    French,
    German,
    Japanese,
}

/// How a list is combined: "A, B, and C" (`And`) or "A, B, or C" (`Or`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    And,
    Or,
}

/// The plural categories of CLDR. A language uses only some of them: English has `One` and
/// `Other`, Japanese only `Other`, while languages such as Russian or Arabic use more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

/// A noun or phrase in each plural form a message needs, chosen with [`Locale::select`].
///
/// ```ignore
/// let capes = PluralForms::new("{n} cape", "{n} capes");
/// Locale::English.select(&capes, 3); // "{n} capes"
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluralForms {
    zero: Option<String>,
    one: Option<String>,
    two: Option<String>,
    few: Option<String>,
    many: Option<String>,
    other: String,
}

impl PluralForms {
    /// A singular and a plural form, which is all most languages need.
    pub fn new(one: impl Into<String>, other: impl Into<String>) -> Self {
        Self {
            one: Some(one.into()),
            ..Self::other_only(other)
        }
    }

    /// One form for every count, for languages without grammatical number (Japanese).
    pub fn other_only(other: impl Into<String>) -> Self {
        Self {
            zero: None,
            one: None,
            two: None,
            few: None,
            many: None,
            other: other.into(),
        }
    }

    /// A form used for exactly zero ("no capes"), in any language.
    pub fn with_zero(mut self, form: impl Into<String>) -> Self {
        self.zero = Some(form.into());
        self
    }

    pub fn with_two(mut self, form: impl Into<String>) -> Self {
        self.two = Some(form.into());
        self
    }

    pub fn with_few(mut self, form: impl Into<String>) -> Self {
        self.few = Some(form.into());
        self
    }

    pub fn with_many(mut self, form: impl Into<String>) -> Self {
        self.many = Some(form.into());
        self
    }

    fn form(&self, category: PluralCategory) -> &str {
        let chosen = match category {
            PluralCategory::Zero => &self.zero,
            PluralCategory::One => &self.one,
            PluralCategory::Two => &self.two,
            PluralCategory::Few => &self.few,
            PluralCategory::Many => &self.many,
            PluralCategory::Other => return &self.other,
        };
        chosen.as_deref().unwrap_or(&self.other)
    }
}

/// A unit of time, from the largest to the smallest.
#[derive(Clone, Copy)]
enum TimeUnit {
    Day,
    Hour,
    Minute,
    Second,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

impl Locale {
    pub const ALL: [Locale; 5] = [
        Locale::English,
        Locale::Spanish,
        Locale::French,
        Locale::German,
        Locale::Japanese,
    ];

    /// The launcher's active language. English until [`Locale::set_current`] says otherwise.
    pub fn current() -> Self {
        Self::ALL
            .get(CURRENT.load(Ordering::Relaxed) as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_current(self) {
        let index = Self::ALL
            .iter()
            .position(|locale| *locale == self)
            .unwrap_or(0);
        CURRENT.store(index as u8, Ordering::Relaxed);
    }

    /// The BCP 47 language subtag: "en", "es", "fr", "de", "ja".
    pub const fn tag(self) -> &'static str {
        match self {
            Locale::English => "en",
            Locale::Spanish => "es",
            Locale::French => "fr",
            Locale::German => "de",
            Locale::Japanese => "ja",
        }
    }

    /// The language named by a locale tag or POSIX locale name: "en", "en-US", "es_MX",
    /// "ja_JP.UTF-8", "DE". `None` for a language that is not supported.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let language = tag.split(['-', '_', '.', '@']).next()?.to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|locale| locale.tag() == language)
    }

    /// The language of the user's environment (`LC_ALL`, `LC_MESSAGES`, then `LANG`), if it is
    /// one we support. The first variable that is set decides.
    pub fn detect_system() -> Option<Self> {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
            .and_then(|value| Self::from_tag(&value))
    }

    // -----------------------------------------------------------------------------------------
    // Plurals
    // -----------------------------------------------------------------------------------------

    /// Which plural form a whole number selects, per CLDR.
    pub fn plural_category(self, count: u64) -> PluralCategory {
        let is_round_million = count != 0 && count % 1_000_000 == 0;
        match self {
            Locale::English | Locale::German => {
                if count == 1 {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
            Locale::Spanish => match count {
                1 => PluralCategory::One,
                _ if is_round_million => PluralCategory::Many,
                _ => PluralCategory::Other,
            },
            Locale::French => match count {
                0 | 1 => PluralCategory::One,
                _ if is_round_million => PluralCategory::Many,
                _ => PluralCategory::Other,
            },
            Locale::Japanese => PluralCategory::Other,
        }
    }

    /// The form of `forms` that `count` selects. An explicit zero form wins for exactly zero.
    pub fn select<'a>(self, forms: &'a PluralForms, count: u64) -> &'a str {
        if count == 0
            && let Some(zero) = forms.zero.as_deref()
        {
            return zero;
        }
        forms.form(self.plural_category(count))
    }

    // -----------------------------------------------------------------------------------------
    // Numbers
    // -----------------------------------------------------------------------------------------

    /// A count as a word: "one" through "ten", then "several" so a big number never reads as a
    /// precise claim. Words for 1-10 use the plain counting form; gendered languages need the
    /// message's own catalog when the noun's gender matters.
    pub fn count_word(self, count: usize) -> String {
        let (words, several): (&[&str; 11], &str) = match self {
            Locale::English => (
                &[
                    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
                    "ten",
                ],
                "several",
            ),
            Locale::Spanish => (
                &[
                    "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho",
                    "nueve", "diez",
                ],
                "varios",
            ),
            Locale::French => (
                &[
                    "zéro", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf",
                    "dix",
                ],
                "plusieurs",
            ),
            Locale::German => (
                &[
                    "null", "ein", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht",
                    "neun", "zehn",
                ],
                "mehrere",
            ),
            Locale::Japanese => (
                &[
                    "零", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十",
                ],
                "数",
            ),
        };
        words
            .get(count)
            .map_or_else(|| several.to_owned(), |word| (*word).to_owned())
    }

    /// A count as a word up to ten, and as exact digits above that.
    pub fn count_or_digits(self, count: usize) -> String {
        if count <= MAX_COUNT_WORD {
            self.count_word(count)
        } else {
            self.thousands(count as u64)
        }
    }

    /// The character between digit groups and the fewest digits that get grouped at all.
    fn grouping(self) -> (&'static str, usize) {
        match self {
            Locale::English | Locale::Japanese => (",", 4),
            // Spanish only groups from five digits: "1234", but "12.345".
            Locale::Spanish => (".", 5),
            // A narrow no-break space, so a number never splits across lines.
            Locale::French => ("\u{202f}", 4),
            Locale::German => (".", 4),
        }
    }

    fn decimal_separator(self) -> &'static str {
        match self {
            Locale::English | Locale::Japanese => ".",
            Locale::Spanish | Locale::French | Locale::German => ",",
        }
    }

    /// Digits grouped the way the language groups them: "12,345" in English, "12.345" in German,
    /// "12 345" (narrow no-break space) in French.
    pub fn thousands(self, value: u64) -> String {
        let digits = value.to_string();
        let (separator, group_from) = self.grouping();
        if digits.len() < group_from {
            return digits;
        }
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                grouped.push_str(separator);
            }
            grouped.push(digit);
        }
        grouped
    }

    /// `value` with exactly `places` decimals, using the language's decimal separator.
    pub fn decimal(self, value: f64, places: usize) -> String {
        format!("{value:.places$}").replace('.', self.decimal_separator())
    }

    /// An ordinal in digits: "3rd", "3.º", "3e", "3.", "第3".
    pub fn ordinal_number(self, value: u64) -> String {
        match self {
            Locale::English => {
                let suffix = match (value % 100, value % 10) {
                    (11..=13, _) => "th",
                    (_, 1) => "st",
                    (_, 2) => "nd",
                    (_, 3) => "rd",
                    _ => "th",
                };
                format!("{value}{suffix}")
            }
            Locale::Spanish => format!("{value}.º"),
            Locale::French if value == 1 => "1er".to_owned(),
            Locale::French => format!("{value}e"),
            Locale::German => format!("{value}."),
            Locale::Japanese => format!("第{value}"),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Lists
    // -----------------------------------------------------------------------------------------

    /// Items as a list in this language: "A, B, and C", "A, B y C", "A、B、C".
    pub fn join<S: AsRef<str>>(self, items: &[S], kind: ListKind) -> String {
        let parts: Vec<&str> = items.iter().map(AsRef::as_ref).collect();
        self.join_parts(&parts, kind)
    }

    /// Like [`Locale::join`], but shows at most `max` items and folds the rest into one "N more"
    /// item. A single leftover item is named instead.
    pub fn join_limited<S: AsRef<str>>(self, items: &[S], kind: ListKind, max: usize) -> String {
        let mut parts: Vec<String> = items.iter().map(|item| item.as_ref().to_owned()).collect();
        if parts.len() > max.saturating_add(1) {
            let hidden = parts.len() - max;
            parts.truncate(max);
            parts.push(self.remainder_item(hidden));
        }
        self.join(&parts, kind)
    }

    /// The list item that stands for `hidden` omitted items: "3 more", "3 más", "他3件".
    pub fn remainder_item(self, hidden: usize) -> String {
        let count = self.thousands(hidden as u64);
        match self {
            Locale::English => format!("{count} more"),
            Locale::Spanish => format!("{count} más"),
            Locale::French => format!("{count} autres"),
            Locale::German => format!("{count} weitere"),
            Locale::Japanese => format!("他{count}件"),
        }
    }

    fn join_parts(self, parts: &[&str], kind: ListKind) -> String {
        let Some((last, head)) = parts.split_last() else {
            return String::new();
        };
        if head.is_empty() {
            return (*last).to_owned();
        }

        if self == Locale::Japanese {
            return match kind {
                ListKind::And => parts.join("、"),
                ListKind::Or if head.len() == 1 => format!("{}または{last}", head[0]),
                ListKind::Or => format!("{}、または{last}", head.join("、")),
            };
        }

        let conjunction = self.conjunction(kind, last);
        if head.len() == 1 {
            return format!("{} {conjunction} {last}", head[0]);
        }
        // Only English puts a comma before the conjunction ("A, B, and C").
        let comma = if self == Locale::English { "," } else { "" };
        format!("{}{comma} {conjunction} {last}", head.join(", "))
    }

    /// The word before the last list item. Spanish swaps "y" for "e" before an "i" sound and "o"
    /// for "u" before an "o" sound, to keep the vowels from running together.
    fn conjunction(self, kind: ListKind, next_item: &str) -> &'static str {
        let next = next_item.trim_start_matches(|c: char| !c.is_alphanumeric());
        let next = next.to_lowercase();
        match (self, kind) {
            (Locale::English, ListKind::And) => "and",
            (Locale::English, ListKind::Or) => "or",
            (Locale::Spanish, ListKind::And) => {
                let i_sound = next.starts_with('i')
                    || (next.starts_with("hi")
                        && !next.starts_with("hia")
                        && !next.starts_with("hie"));
                if i_sound { "e" } else { "y" }
            }
            (Locale::Spanish, ListKind::Or) => {
                if next.starts_with('o') || next.starts_with("ho") {
                    "u"
                } else {
                    "o"
                }
            }
            (Locale::French, ListKind::And) => "et",
            (Locale::French, ListKind::Or) => "ou",
            (Locale::German, ListKind::And) => "und",
            (Locale::German, ListKind::Or) => "oder",
            // Japanese is joined without a spoken word before the last item; see `join_parts`.
            (Locale::Japanese, ListKind::And) => "",
            (Locale::Japanese, ListKind::Or) => "または",
        }
    }

    // -----------------------------------------------------------------------------------------
    // Time and size
    // -----------------------------------------------------------------------------------------

    fn time_unit(self, unit: TimeUnit) -> PluralForms {
        match (self, unit) {
            (Locale::English, TimeUnit::Day) => PluralForms::new("day", "days"),
            (Locale::English, TimeUnit::Hour) => PluralForms::new("hour", "hours"),
            (Locale::English, TimeUnit::Minute) => PluralForms::new("minute", "minutes"),
            (Locale::English, TimeUnit::Second) => PluralForms::new("second", "seconds"),
            (Locale::Spanish, TimeUnit::Day) => PluralForms::new("día", "días"),
            (Locale::Spanish, TimeUnit::Hour) => PluralForms::new("hora", "horas"),
            (Locale::Spanish, TimeUnit::Minute) => PluralForms::new("minuto", "minutos"),
            (Locale::Spanish, TimeUnit::Second) => PluralForms::new("segundo", "segundos"),
            (Locale::French, TimeUnit::Day) => PluralForms::new("jour", "jours"),
            (Locale::French, TimeUnit::Hour) => PluralForms::new("heure", "heures"),
            (Locale::French, TimeUnit::Minute) => PluralForms::new("minute", "minutes"),
            (Locale::French, TimeUnit::Second) => PluralForms::new("seconde", "secondes"),
            (Locale::German, TimeUnit::Day) => PluralForms::new("Tag", "Tage"),
            (Locale::German, TimeUnit::Hour) => PluralForms::new("Stunde", "Stunden"),
            (Locale::German, TimeUnit::Minute) => PluralForms::new("Minute", "Minuten"),
            (Locale::German, TimeUnit::Second) => PluralForms::new("Sekunde", "Sekunden"),
            (Locale::Japanese, TimeUnit::Day) => PluralForms::other_only("日"),
            (Locale::Japanese, TimeUnit::Hour) => PluralForms::other_only("時間"),
            (Locale::Japanese, TimeUnit::Minute) => PluralForms::other_only("分"),
            (Locale::Japanese, TimeUnit::Second) => PluralForms::other_only("秒"),
        }
    }

    /// "5 seconds", "1 día", "3日": a number with its unit noun, spaced as the language spaces it.
    fn counted_unit(self, count: u64, forms: &PluralForms) -> String {
        let number = self.thousands(count);
        let unit = self.select(forms, count);
        if self == Locale::Japanese {
            format!("{number}{unit}")
        } else {
            format!("{number} {unit}")
        }
    }

    fn less_than_a_second(self) -> &'static str {
        match self {
            Locale::English => "less than a second",
            Locale::Spanish => "menos de un segundo",
            Locale::French => "moins d'une seconde",
            Locale::German => "weniger als eine Sekunde",
            Locale::Japanese => "1秒未満",
        }
    }

    /// A duration with its two largest non-zero units: "2 minutes and 5 seconds".
    pub fn duration_words(self, duration: Duration) -> String {
        let total = duration.as_secs();
        if total == 0 {
            return self.less_than_a_second().to_owned();
        }
        let units = [
            (total / SECONDS_PER_DAY, TimeUnit::Day),
            (total % SECONDS_PER_DAY / SECONDS_PER_HOUR, TimeUnit::Hour),
            (
                total % SECONDS_PER_HOUR / SECONDS_PER_MINUTE,
                TimeUnit::Minute,
            ),
            (total % SECONDS_PER_MINUTE, TimeUnit::Second),
        ];
        let parts: Vec<String> = units
            .into_iter()
            .filter(|(value, _)| *value > 0)
            .take(2)
            .map(|(value, unit)| self.counted_unit(value, &self.time_unit(unit)))
            .collect();
        if self == Locale::Japanese {
            parts.concat()
        } else {
            self.join(&parts, ListKind::And)
        }
    }

    fn about(self, amount: &str) -> String {
        match self {
            Locale::English => format!("about {amount}"),
            Locale::Spanish => format!("aproximadamente {amount}"),
            Locale::French => format!("environ {amount}"),
            Locale::German => format!("etwa {amount}"),
            Locale::Japanese => format!("約{amount}"),
        }
    }

    /// A duration as one rounded unit, with "about" when rounding changed it.
    pub fn duration_approx(self, duration: Duration) -> String {
        let total = duration.as_secs();
        if total == 0 {
            return self.less_than_a_second().to_owned();
        }
        let (unit_seconds, unit) = match total {
            0..SECONDS_PER_MINUTE => (1, TimeUnit::Second),
            SECONDS_PER_MINUTE..SECONDS_PER_HOUR => (SECONDS_PER_MINUTE, TimeUnit::Minute),
            SECONDS_PER_HOUR..SECONDS_PER_DAY => (SECONDS_PER_HOUR, TimeUnit::Hour),
            _ => (SECONDS_PER_DAY, TimeUnit::Day),
        };
        let rounded = (total + unit_seconds / 2) / unit_seconds;
        let amount = self.counted_unit(rounded, &self.time_unit(unit));
        if total % unit_seconds == 0 {
            amount
        } else {
            self.about(&amount)
        }
    }

    /// How long ago something happened: "just now" within five seconds, then "about 5 minutes
    /// ago", "hace 2 horas", "il y a 3 jours", "vor 45 Sekunden", "5分前".
    pub fn time_ago(self, elapsed: Duration) -> String {
        if elapsed < Duration::from_secs(5) {
            return match self {
                Locale::English => "just now",
                Locale::Spanish => "ahora mismo",
                Locale::French => "à l'instant",
                Locale::German => "gerade eben",
                Locale::Japanese => "たった今",
            }
            .to_owned();
        }
        let span = self.duration_approx(elapsed);
        match self {
            Locale::English => format!("{span} ago"),
            Locale::Spanish => format!("hace {span}"),
            Locale::French => format!("il y a {span}"),
            Locale::German => format!("vor {span}"),
            Locale::Japanese => format!("{span}前"),
        }
    }

    /// A byte count for people: "512 bytes", "1.5 KB", "20 MB". Units step by 1024.
    pub fn bytes_words(self, bytes: u64) -> String {
        let byte_forms = match self {
            Locale::English | Locale::Spanish => PluralForms::new("byte", "bytes"),
            Locale::French => PluralForms::new("octet", "octets"),
            Locale::German => PluralForms::new("Byte", "Bytes"),
            Locale::Japanese => PluralForms::other_only("バイト"),
        };
        if bytes < 1024 {
            return self.counted_unit(bytes, &byte_forms);
        }

        let units: [&str; 4] = match self {
            Locale::French => ["Ko", "Mo", "Go", "To"],
            _ => ["KB", "MB", "GB", "TB"],
        };
        let mut value = bytes as f64 / 1024.0;
        let mut unit = units[0];
        for next in &units[1..] {
            if value < 1024.0 {
                break;
            }
            value /= 1024.0;
            unit = next;
        }
        let number = if value < 10.0 {
            let text = self.decimal(value, 1);
            let separator = self.decimal_separator();
            // "1,0" becomes "1": drop a trailing ".0" in the language's own punctuation.
            text.strip_suffix(&format!("{separator}0"))
                .map_or(text.clone(), str::to_owned)
        } else {
            self.decimal(value, 0)
        };
        format!("{number} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use Locale::{English, French, German, Japanese, Spanish};

    fn secs(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    #[test]
    fn locale_tags_parse_and_round_trip() {
        for (tag, expected) in [
            ("en", English),
            ("en-US", English),
            ("es_MX", Spanish),
            ("fr-CA", French),
            ("DE", German),
            ("ja_JP.UTF-8", Japanese),
            ("de_DE@euro", German),
        ] {
            assert_eq!(Locale::from_tag(tag), Some(expected), "{tag}");
        }
        for tag in ["", "pt", "C", "POSIX", "zh-CN"] {
            assert_eq!(Locale::from_tag(tag), None, "{tag:?}");
        }
        for locale in Locale::ALL {
            assert_eq!(Locale::from_tag(locale.tag()), Some(locale));
        }
    }

    #[test]
    fn the_current_locale_can_be_switched_and_defaults_to_english() {
        assert_eq!(Locale::default(), English);
        Japanese.set_current();
        assert_eq!(Locale::current(), Japanese);
        English.set_current();
        assert_eq!(Locale::current(), English);
    }

    #[test]
    fn plural_categories_follow_cldr() {
        use PluralCategory::{Many, One, Other};
        for (locale, count, expected) in [
            (English, 0, Other),
            (English, 1, One),
            (English, 2, Other),
            (German, 1, One),
            (German, 0, Other),
            (Spanish, 1, One),
            (Spanish, 5, Other),
            (Spanish, 1_000_000, Many),
            (French, 0, One),
            (French, 1, One),
            (French, 2, Other),
            (French, 2_000_000, Many),
            (Japanese, 1, Other),
            (Japanese, 0, Other),
        ] {
            assert_eq!(
                locale.plural_category(count),
                expected,
                "{locale:?} {count}"
            );
        }
    }

    #[test]
    fn plural_forms_select_by_the_languages_own_rules() {
        let capes = PluralForms::new("cape", "capes");
        assert_eq!(English.select(&capes, 1), "cape");
        assert_eq!(English.select(&capes, 0), "capes");
        assert_eq!(English.select(&capes, 5), "capes");
        // French treats zero as singular.
        let french = PluralForms::new("cape", "capes");
        assert_eq!(French.select(&french, 0), "cape");
        // An explicit zero form wins for exactly zero, in any language.
        let with_zero = PluralForms::new("cape", "capes").with_zero("no capes");
        assert_eq!(English.select(&with_zero, 0), "no capes");
        assert_eq!(French.select(&with_zero, 0), "no capes");
        // Forms a language never selects are ignored.
        let with_two = PluralForms::new("cape", "capes").with_two("a pair of capes");
        assert_eq!(English.select(&with_two, 2), "capes");
        // Spanish and French use "many" for round millions.
        let millions = PluralForms::new("cape", "capes").with_many("de capes");
        assert_eq!(Spanish.select(&millions, 1_000_000), "de capes");
        assert_eq!(Spanish.select(&millions, 999), "capes");
        // No grammatical number: one form for everything.
        let counter = PluralForms::other_only("枚");
        assert_eq!(Japanese.select(&counter, 1), "枚");
        assert_eq!(Japanese.select(&counter, 9), "枚");
    }

    #[test]
    fn count_words_exist_in_every_language() {
        for (locale, two, ten, several) in [
            (English, "two", "ten", "several"),
            (Spanish, "dos", "diez", "varios"),
            (French, "deux", "dix", "plusieurs"),
            (German, "zwei", "zehn", "mehrere"),
            (Japanese, "二", "十", "数"),
        ] {
            assert_eq!(locale.count_word(2), two, "{locale:?}");
            assert_eq!(locale.count_word(10), ten, "{locale:?}");
            assert_eq!(locale.count_word(11), several, "{locale:?}");
        }
        assert_eq!(German.count_or_digits(7), "sieben");
        assert_eq!(German.count_or_digits(1_500), "1.500");
    }

    #[test]
    fn lists_use_each_languages_conjunction_and_punctuation() {
        let abc = ["A", "B", "C"];
        let ab = ["A", "B"];
        for (locale, kind, three, two) in [
            (English, ListKind::And, "A, B, and C", "A and B"),
            (English, ListKind::Or, "A, B, or C", "A or B"),
            (Spanish, ListKind::And, "A, B y C", "A y B"),
            (Spanish, ListKind::Or, "A, B o C", "A o B"),
            (French, ListKind::And, "A, B et C", "A et B"),
            (French, ListKind::Or, "A, B ou C", "A ou B"),
            (German, ListKind::And, "A, B und C", "A und B"),
            (German, ListKind::Or, "A, B oder C", "A oder B"),
            (Japanese, ListKind::And, "A、B、C", "A、B"),
            (Japanese, ListKind::Or, "A、B、またはC", "AまたはB"),
        ] {
            assert_eq!(locale.join(&abc, kind), three, "{locale:?} {kind:?}");
            assert_eq!(locale.join(&ab, kind), two, "{locale:?} {kind:?}");
            assert_eq!(locale.join(&["A"], kind), "A");
            assert_eq!(locale.join::<&str>(&[], kind), "");
        }
    }

    #[test]
    fn english_locale_lists_match_the_english_helpers() {
        let items = ["A", "B", "C", "D"];
        assert_eq!(
            English.join(&items, ListKind::And),
            super::super::join_natural(&items)
        );
        assert_eq!(
            English.join(&items, ListKind::Or),
            super::super::join_or(&items)
        );
    }

    #[test]
    fn spanish_conjunctions_avoid_repeated_vowel_sounds() {
        assert_eq!(
            Spanish.join(&["Pan", "Irene"], ListKind::And),
            "Pan e Irene"
        );
        assert_eq!(Spanish.join(&["Pan", "Hilo"], ListKind::And), "Pan e Hilo");
        assert_eq!(
            Spanish.join(&["Sol", "Hierro"], ListKind::And),
            "Sol y Hierro"
        );
        assert_eq!(
            Spanish.join(&["Sol", "Hielo"], ListKind::And),
            "Sol y Hielo"
        );
        assert_eq!(Spanish.join(&["Uno", "Oro"], ListKind::Or), "Uno u Oro");
        assert_eq!(
            Spanish.join(&["Uno", "Hombre"], ListKind::Or),
            "Uno u Hombre"
        );
        assert_eq!(Spanish.join(&["Uno", "Dos"], ListKind::Or), "Uno o Dos");
    }

    #[test]
    fn long_lists_fold_the_rest_into_a_localized_count() {
        let items = ["A", "B", "C", "D", "E", "F"];
        for (locale, expected) in [
            (English, "A, B, C, and 3 more"),
            (Spanish, "A, B, C y 3 más"),
            (French, "A, B, C et 3 autres"),
            (German, "A, B, C und 3 weitere"),
            (Japanese, "A、B、C、他3件"),
        ] {
            assert_eq!(locale.join_limited(&items, ListKind::And, 3), expected);
        }
        // A single leftover is named, not counted.
        assert_eq!(
            English.join_limited(&items[..4], ListKind::And, 3),
            "A, B, C, and D"
        );
    }

    #[test]
    fn numbers_are_grouped_and_punctuated_per_language() {
        assert_eq!(English.thousands(1_234), "1,234");
        assert_eq!(English.thousands(999), "999");
        assert_eq!(German.thousands(1_234_567), "1.234.567");
        assert_eq!(French.thousands(1_234_567), "1\u{202f}234\u{202f}567");
        assert_eq!(Japanese.thousands(1_234), "1,234");
        // Spanish leaves four-digit numbers alone.
        assert_eq!(Spanish.thousands(1_234), "1234");
        assert_eq!(Spanish.thousands(12_345), "12.345");

        assert_eq!(English.decimal(1.5, 1), "1.5");
        assert_eq!(German.decimal(1.5, 1), "1,5");
        assert_eq!(French.decimal(2.25, 2), "2,25");
    }

    #[test]
    fn ordinals_are_written_per_language() {
        assert_eq!(English.ordinal_number(22), "22nd");
        assert_eq!(English.ordinal_number(13), "13th");
        assert_eq!(Spanish.ordinal_number(3), "3.º");
        assert_eq!(French.ordinal_number(1), "1er");
        assert_eq!(French.ordinal_number(2), "2e");
        assert_eq!(German.ordinal_number(3), "3.");
        assert_eq!(Japanese.ordinal_number(3), "第3");
    }

    #[test]
    fn durations_name_their_units_in_each_language() {
        assert_eq!(English.duration_words(secs(125)), "2 minutes and 5 seconds");
        assert_eq!(Spanish.duration_words(secs(125)), "2 minutos y 5 segundos");
        assert_eq!(Spanish.duration_words(secs(1)), "1 segundo");
        assert_eq!(French.duration_words(secs(3_600)), "1 heure");
        assert_eq!(French.duration_words(secs(5_400)), "1 heure et 30 minutes");
        assert_eq!(German.duration_words(secs(125)), "2 Minuten und 5 Sekunden");
        assert_eq!(German.duration_words(secs(86_400)), "1 Tag");
        assert_eq!(Japanese.duration_words(secs(125)), "2分5秒");
        assert_eq!(Japanese.duration_words(secs(5_400)), "1時間30分");
        for (locale, expected) in [
            (English, "less than a second"),
            (Spanish, "menos de un segundo"),
            (French, "moins d'une seconde"),
            (German, "weniger als eine Sekunde"),
            (Japanese, "1秒未満"),
        ] {
            assert_eq!(locale.duration_words(Duration::from_millis(300)), expected);
            assert_eq!(locale.duration_approx(Duration::ZERO), expected);
        }
    }

    #[test]
    fn approximate_durations_and_time_ago_follow_each_languages_phrasing() {
        assert_eq!(English.duration_approx(secs(310)), "about 5 minutes");
        assert_eq!(
            Spanish.duration_approx(secs(310)),
            "aproximadamente 5 minutos"
        );
        assert_eq!(French.duration_approx(secs(300)), "5 minutes");
        assert_eq!(French.duration_approx(secs(310)), "environ 5 minutes");
        assert_eq!(German.duration_approx(secs(310)), "etwa 5 Minuten");
        assert_eq!(Japanese.duration_approx(secs(310)), "約5分");

        assert_eq!(English.time_ago(secs(45)), "45 seconds ago");
        assert_eq!(Spanish.time_ago(secs(45)), "hace 45 segundos");
        assert_eq!(French.time_ago(secs(7_200)), "il y a 2 heures");
        assert_eq!(German.time_ago(secs(45)), "vor 45 Sekunden");
        assert_eq!(Japanese.time_ago(secs(300)), "5分前");
        assert_eq!(Japanese.time_ago(secs(310)), "約5分前");
        for (locale, expected) in [
            (English, "just now"),
            (Spanish, "ahora mismo"),
            (French, "à l'instant"),
            (German, "gerade eben"),
            (Japanese, "たった今"),
        ] {
            assert_eq!(locale.time_ago(secs(2)), expected);
        }
    }

    #[test]
    fn byte_sizes_use_local_units_and_decimal_marks() {
        assert_eq!(English.bytes_words(1), "1 byte");
        assert_eq!(English.bytes_words(1_536), "1.5 KB");
        assert_eq!(Spanish.bytes_words(20 * 1024 * 1024), "20 MB");
        assert_eq!(Spanish.bytes_words(1_536), "1,5 KB");
        assert_eq!(French.bytes_words(0), "0 octet", "French zero is singular");
        assert_eq!(French.bytes_words(512), "512 octets");
        assert_eq!(French.bytes_words(1_536), "1,5 Ko");
        assert_eq!(German.bytes_words(1_536), "1,5 KB");
        assert_eq!(German.bytes_words(512), "512 Bytes");
        assert_eq!(Japanese.bytes_words(1), "1バイト");
        assert_eq!(Japanese.bytes_words(512), "512バイト");
        assert_eq!(English.bytes_words(2 * 1024), "2 KB");
    }
}
