//! The closed key sets of the deck's front matter and the grammar of each value, decided over bytes.

/// Where a front matter block sits: at the head of the deck or at the start of a slide.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Position {
    Head,
    Slide,
}

/// Every key a front matter block may carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Key {
    Theme,
    DefaultMaster,
    Title,
    Author,
    Date,
    Lang,
    Master,
    Id,
}

impl Key {
    /// Every key, in the order the documentation lists them.
    pub const ALL: [Self; 8] = [
        Self::Theme,
        Self::DefaultMaster,
        Self::Title,
        Self::Author,
        Self::Date,
        Self::Lang,
        Self::Master,
        Self::Id,
    ];

    /// The key as it is written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::DefaultMaster => "default_master",
            Self::Title => "title",
            Self::Author => "author",
            Self::Date => "date",
            Self::Lang => "lang",
            Self::Master => "master",
            Self::Id => "id",
        }
    }

    /// The key written as `name`, if any.
    #[must_use]
    pub fn from_name(name: &[u8]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|key| key.name().as_bytes() == name)
    }

    /// Where the key belongs.
    #[must_use]
    pub const fn position(self) -> Position {
        match self {
            Self::Theme
            | Self::DefaultMaster
            | Self::Title
            | Self::Author
            | Self::Date
            | Self::Lang => Position::Head,
            Self::Master | Self::Id => Position::Slide,
        }
    }
}

/// Whether a key belongs where it was written.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Placement {
    Allowed,
    /// The key belongs at the head of the deck.
    HeadOnly,
    /// The key belongs at the start of a slide.
    SlideOnly,
}

/// Where `key` may be written, given where it was.
#[must_use]
pub const fn placement(key: Key, position: Position) -> Placement {
    match (key.position(), position) {
        (Position::Head, Position::Head) | (Position::Slide, Position::Slide) => Placement::Allowed,
        (Position::Head, Position::Slide) => Placement::HeadOnly,
        (Position::Slide, Position::Head) => Placement::SlideOnly,
    }
}

/// The most bytes an identifier holds.
pub const MAX_IDENTIFIER: usize = 32;

/// The most characters a title or an author holds.
pub const MAX_SHORT_TEXT: usize = 200;

const DATE_LENGTH: usize = 10;
const YEAR_END: usize = 4;
const MONTH_START: usize = 5;
const MONTH_END: usize = 7;
const DAY_START: usize = 8;
const MAX_MONTH: u8 = 12;
const MAX_DAY: u8 = 31;
const DASH: u8 = b'-';
const DECIMAL: u8 = 10;
const MIN_LANGUAGE: usize = 2;
const MAX_LANGUAGE: usize = 3;
const REGION_LENGTH: usize = 2;

const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == DASH
}

/// Whether `bytes` is an identifier: a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes.
#[must_use]
pub fn is_identifier(bytes: &[u8]) -> bool {
    let Some((first, rest)) = bytes.split_first() else {
        return false;
    };
    bytes.len() <= MAX_IDENTIFIER
        && first.is_ascii_lowercase()
        && rest.iter().all(|byte| is_identifier_byte(*byte))
}

/// The value of two decimal digits, if both bytes are digits.
fn two_digits(bytes: Option<&[u8]>) -> Option<u8> {
    let [tens, ones] = bytes? else {
        return None;
    };
    if !tens.is_ascii_digit() || !ones.is_ascii_digit() {
        return None;
    }
    tens.checked_sub(b'0')?
        .checked_mul(DECIMAL)?
        .checked_add(ones.checked_sub(b'0')?)
}

/// Whether `bytes` is a date written `YYYY-MM-DD` with a month of one to twelve and a day of one to thirty-one.
#[must_use]
pub fn is_date(bytes: &[u8]) -> bool {
    if bytes.len() != DATE_LENGTH {
        return false;
    }
    let year_digits = bytes
        .get(..YEAR_END)
        .is_some_and(|year| year.iter().all(u8::is_ascii_digit));
    let dashes = bytes.get(YEAR_END) == Some(&DASH) && bytes.get(MONTH_END) == Some(&DASH);
    let month = two_digits(bytes.get(MONTH_START..MONTH_END));
    let day = two_digits(bytes.get(DAY_START..DATE_LENGTH));
    year_digits
        && dashes
        && month.is_some_and(|month| (1..=MAX_MONTH).contains(&month))
        && day.is_some_and(|day| (1..=MAX_DAY).contains(&day))
}

/// Whether `bytes` is a language tag: two or three lowercase letters, then an optional dash and two uppercase letters.
#[must_use]
pub fn is_lang(bytes: &[u8]) -> bool {
    let dash = bytes.iter().position(|byte| *byte == DASH);
    let primary = dash.map_or(Some(bytes), |at| bytes.get(..at));
    let region = dash.and_then(|at| bytes.get(at.saturating_add(1)..));
    let primary_fits = primary.is_some_and(|primary| {
        primary.len() >= MIN_LANGUAGE
            && primary.len() <= MAX_LANGUAGE
            && primary.iter().all(u8::is_ascii_lowercase)
    });
    let region_fits = region.is_none_or(|region| {
        region.len() == REGION_LENGTH && region.iter().all(u8::is_ascii_uppercase)
    });
    primary_fits && region_fits
}

/// What a `theme` value names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ThemeShape {
    /// An identifier, which names a theme the binary carries.
    Builtin,
    /// A relative path with a slash, which names a theme directory beside the deck.
    Relative,
    /// An absolute path, which a deck never names because it would tie the deck to one machine.
    Absolute,
    Invalid,
}

const SLASH: u8 = b'/';
const BACKSLASH: u8 = b'\\';
const COLON: u8 = b':';

const fn has_drive(bytes: &[u8]) -> bool {
    matches!(bytes, [letter, colon, ..] if letter.is_ascii_alphabetic() && *colon == COLON)
}

/// What `bytes` names as a theme.
#[must_use]
pub fn theme_shape(bytes: &[u8]) -> ThemeShape {
    let Some(first) = bytes.first() else {
        return ThemeShape::Invalid;
    };
    if *first == SLASH || *first == BACKSLASH || has_drive(bytes) {
        ThemeShape::Absolute
    } else if bytes.contains(&BACKSLASH) {
        ThemeShape::Invalid
    } else if bytes.contains(&SLASH) {
        ThemeShape::Relative
    } else if is_identifier(bytes) {
        ThemeShape::Builtin
    } else {
        ThemeShape::Invalid
    }
}

/// Whether a title or an author of `chars` characters fits: at least one and at most two hundred.
#[must_use]
pub const fn short_text_fits(chars: usize) -> bool {
    chars >= 1 && chars <= MAX_SHORT_TEXT
}

/// The properties Kani proves for every short byte string and the tests check over the values they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{
        DASH, Key, MAX_DAY, MAX_IDENTIFIER, MAX_LANGUAGE, MAX_MONTH, MIN_LANGUAGE, Placement,
        Position, REGION_LENGTH, placement,
    };
    #[cfg(kani)]
    use super::{DATE_LENGTH, is_date, is_identifier, is_lang};

    const DECIMAL: u16 = 10;

    /// How many bytes the bounded proofs admit, which covers every language tag and every short identifier.
    #[cfg(kani)]
    const SAMPLE: usize = 7;

    /// An identifier, stated as the characters it may hold.
    pub(super) fn spec_identifier(bytes: &[u8]) -> bool {
        let starts_well = bytes.first().is_some_and(u8::is_ascii_lowercase);
        let continues_well = bytes
            .iter()
            .skip(1)
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == DASH);
        starts_well && continues_well && bytes.len() <= MAX_IDENTIFIER
    }

    /// A date, stated digit by digit.
    pub(super) fn spec_date(bytes: &[u8]) -> bool {
        let [y1, y2, y3, y4, d1, m1, m2, d2, a1, a2] = bytes else {
            return false;
        };
        let digits = [y1, y2, y3, y4, m1, m2, a1, a2]
            .iter()
            .all(|byte| byte.is_ascii_digit());
        let month = u16::from(m1.wrapping_sub(b'0'))
            .wrapping_mul(DECIMAL)
            .wrapping_add(u16::from(m2.wrapping_sub(b'0')));
        let day = u16::from(a1.wrapping_sub(b'0'))
            .wrapping_mul(DECIMAL)
            .wrapping_add(u16::from(a2.wrapping_sub(b'0')));
        digits
            && *d1 == DASH
            && *d2 == DASH
            && month >= 1
            && month <= u16::from(MAX_MONTH)
            && day >= 1
            && day <= u16::from(MAX_DAY)
    }

    /// A language tag, stated as its two parts.
    pub(super) fn spec_lang(bytes: &[u8]) -> bool {
        let dash = bytes.iter().position(|byte| *byte == DASH);
        let primary_end = dash.unwrap_or(bytes.len());
        let primary = bytes.get(..primary_end).unwrap_or_default();
        let region = dash.map(|at| bytes.get(at.saturating_add(1)..).unwrap_or_default());
        primary.len() >= MIN_LANGUAGE
            && primary.len() <= MAX_LANGUAGE
            && primary.iter().all(u8::is_ascii_lowercase)
            && region.is_none_or(|region| {
                region.len() == REGION_LENGTH && region.iter().all(u8::is_ascii_uppercase)
            })
    }

    /// The keys the head of the deck carries, as the documentation lists them.
    pub(super) const fn is_head_key(key: Key) -> bool {
        matches!(
            key,
            Key::Theme | Key::DefaultMaster | Key::Title | Key::Author | Key::Date | Key::Lang
        )
    }

    /// The placement follows the key's position and nothing else.
    pub(super) fn spec_placement(key: Key, position: Position) {
        let verdict = placement(key, position);
        let at_home = match position {
            Position::Head => is_head_key(key),
            Position::Slide => !is_head_key(key),
        };
        assert_eq!(verdict == Placement::Allowed, at_home);
        if !at_home {
            let expected = if is_head_key(key) {
                Placement::HeadOnly
            } else {
                Placement::SlideOnly
            };
            assert_eq!(verdict, expected);
        }
        assert_eq!(Key::from_name(key.name().as_bytes()), Some(key));
    }

    #[cfg(kani)]
    fn any_bytes() -> ([u8; SAMPLE], usize) {
        let bytes: [u8; SAMPLE] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= SAMPLE);
        (bytes, len)
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(9)]
    fn an_identifier_is_a_lowercase_letter_then_lowercase_letters_digits_or_dashes() {
        let (bytes, len) = any_bytes();
        let Some(slice) = bytes.get(..len) else {
            return;
        };
        assert_eq!(is_identifier(slice), spec_identifier(slice));
        kani::cover!(is_identifier(slice));
        kani::cover!(!is_identifier(slice) && !slice.is_empty());
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(12)]
    fn a_date_is_four_two_and_two_digits_with_a_month_and_a_day_in_range() {
        let bytes: [u8; DATE_LENGTH] = kani::any();
        assert_eq!(is_date(&bytes), spec_date(&bytes));
        kani::cover!(is_date(&bytes));
        kani::cover!(!is_date(&bytes));
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(9)]
    fn a_language_tag_is_two_or_three_letters_with_an_optional_region() {
        let (bytes, len) = any_bytes();
        let Some(slice) = bytes.get(..len) else {
            return;
        };
        assert_eq!(is_lang(slice), spec_lang(slice));
        kani::cover!(is_lang(slice) && slice.contains(&DASH));
        kani::cover!(is_lang(slice) && !slice.contains(&DASH));
        kani::cover!(!is_lang(slice));
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(16)]
    fn every_key_is_allowed_exactly_where_it_belongs() {
        let key: Key = kani::any();
        let position: Position = kani::any();
        spec_placement(key, position);
        kani::cover!(placement(key, position) == Placement::Allowed);
        kani::cover!(placement(key, position) == Placement::HeadOnly);
        kani::cover!(placement(key, position) == Placement::SlideOnly);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_capitalized_name_as_an_identifier() {
        kani::assert(is_identifier(b"Main"), "NEGATIVE_FRONT_MATTER_CONTROL");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Key, MAX_IDENTIFIER, MAX_SHORT_TEXT, Position, ThemeShape, is_date, is_identifier, is_lang,
        proofs, short_text_fits, theme_shape,
    };
    use alloc::string::String;

    #[test]
    fn identifiers_dates_and_language_tags_follow_their_grammars() {
        let longest = "a".repeat(MAX_IDENTIFIER);
        let too_long = "a".repeat(MAX_IDENTIFIER.saturating_add(1));
        for sample in [
            "", "a", "bullets", "two-up", "x9", "9x", "Main", "two_up", "-a", "a-", &longest,
            &too_long, "日本",
        ] {
            assert_eq!(
                is_identifier(sample.as_bytes()),
                proofs::spec_identifier(sample.as_bytes()),
                "{sample:?}"
            );
        }
        assert!(is_identifier(b"two-up"));
        assert!(!is_identifier(b"two_up"));
        for sample in [
            "2026-10-13",
            "2026-13-01",
            "2026-00-10",
            "2026-02-31",
            "2026-02-32",
            "2026-1-13",
            "26-10-13",
            "2026/10/13",
            "2026-10-13 ",
            "0000-01-01",
            "2026-0:-13",
            "2026-10-0:",
            "2026-10x13",
            "2026x10-13",
            "2026-:0-13",
            "202a-10-13",
        ] {
            assert_eq!(
                is_date(sample.as_bytes()),
                proofs::spec_date(sample.as_bytes()),
                "{sample:?}"
            );
        }
        assert!(is_date(b"2026-10-13"));
        assert!(!is_date(b"2026-02-32"));
        for sample in [
            "ja", "en-US", "zho", "yue-HK", "j", "japanese", "ja-us", "ja-", "-US", "JA", "ja-USA",
        ] {
            assert_eq!(
                is_lang(sample.as_bytes()),
                proofs::spec_lang(sample.as_bytes()),
                "{sample:?}"
            );
        }
        assert!(is_lang(b"en-US"));
        assert!(!is_lang(b"ja-us"));
    }

    #[test]
    fn keys_belong_to_the_head_or_to_a_slide() {
        for key in Key::ALL {
            proofs::spec_placement(key, Position::Head);
            proofs::spec_placement(key, Position::Slide);
        }
        assert_eq!(Key::from_name(b"master"), Some(Key::Master));
        assert_eq!(Key::from_name(b"Master"), None);
        assert_eq!(Key::from_name(b""), None);
    }

    #[test]
    fn a_theme_is_a_built_in_name_or_a_relative_path() {
        assert_eq!(theme_shape(b"plain"), ThemeShape::Builtin);
        assert_eq!(theme_shape(b"themes/corp"), ThemeShape::Relative);
        assert_eq!(theme_shape(b"./corp"), ThemeShape::Relative);
        assert_eq!(theme_shape(b"/themes/corp"), ThemeShape::Absolute);
        assert_eq!(theme_shape(b"\\themes"), ThemeShape::Absolute);
        assert_eq!(theme_shape(b"C:/themes"), ThemeShape::Absolute);
        assert_eq!(theme_shape(b"c:\\themes"), ThemeShape::Absolute);
        assert_eq!(theme_shape(b"themes\\corp"), ThemeShape::Invalid);
        assert_eq!(theme_shape(b"Plain"), ThemeShape::Invalid);
        assert_eq!(theme_shape(b""), ThemeShape::Invalid);
        assert_eq!(
            theme_shape(String::from("corp").as_bytes()),
            ThemeShape::Builtin
        );
    }

    #[test]
    fn a_short_text_holds_one_to_two_hundred_characters() {
        assert!(!short_text_fits(0));
        assert!(short_text_fits(1));
        assert!(short_text_fits(MAX_SHORT_TEXT));
        assert!(!short_text_fits(MAX_SHORT_TEXT.saturating_add(1)));
    }
}
