//! The named values of the front matter, each admitted by its constructor through the front matter rules.

use crate::front_matter_rules::{self, ThemeShape};
use alloc::string::String;
use core::fmt;

/// Why a value failed its grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InvalidText {
    /// Not a lowercase letter followed by up to thirty-one lowercase letters, digits, or dashes.
    Identifier,
    /// Not one to two hundred characters.
    ShortText,
    /// Not a date written `YYYY-MM-DD`.
    Date,
    /// Not a language tag such as `ja` or `en-US`.
    Lang,
    /// Neither a built-in theme name nor a relative path with a slash.
    Theme,
    /// An absolute path, which would tie the deck to one machine.
    AbsoluteTheme,
}

/// A name of a master or a slide: a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Identifier(String);

impl Identifier {
    /// # Errors
    ///
    /// Returns `InvalidText::Identifier` when `text` leaves the grammar.
    pub fn new(text: &str) -> Result<Self, InvalidText> {
        if front_matter_rules::is_identifier(text.as_bytes()) {
            Ok(Self(String::from(text)))
        } else {
            Err(InvalidText::Identifier)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A title or an author: one to two hundred characters on one line.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ShortText(String);

impl ShortText {
    /// # Errors
    ///
    /// Returns `InvalidText::ShortText` when `text` is empty, too long, or spans lines.
    pub fn new(text: &str) -> Result<Self, InvalidText> {
        let on_one_line = !text.contains(['\n', '\r']);
        if on_one_line && front_matter_rules::short_text_fits(text.chars().count()) {
            Ok(Self(String::from(text)))
        } else {
            Err(InvalidText::ShortText)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShortText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

const YEAR_END: usize = 4;
const MONTH_START: usize = 5;
const MONTH_END: usize = 7;
const DAY_START: usize = 8;
const DAY_END: usize = 10;

const DECIMAL: u8 = 10;

/// The value of one decimal digit.
const fn digit(byte: u8) -> Option<u8> {
    if byte.is_ascii_digit() {
        Some(byte.wrapping_sub(b'0'))
    } else {
        None
    }
}

/// The number the two decimal digits of `text` spell.
fn two_digits(text: Option<&str>) -> Option<u8> {
    let [tens, ones] = text?.as_bytes() else {
        return None;
    };
    digit(*tens)?
        .checked_mul(DECIMAL)?
        .checked_add(digit(*ones)?)
}

/// The number the decimal digits of `text` spell, if every byte is a digit and the number fits.
fn decimal(text: Option<&str>) -> Option<u16> {
    let text = text?;
    if text.is_empty() {
        return None;
    }
    let mut value: u16 = 0;
    for byte in text.bytes() {
        value = value
            .checked_mul(u16::from(DECIMAL))?
            .checked_add(u16::from(digit(byte)?))?;
    }
    Some(value)
}

/// A calendar date written `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// # Errors
    ///
    /// Returns `InvalidText::Date` when `text` is not `YYYY-MM-DD` with a month of one to twelve and a day of one to thirty-one.
    pub fn new(text: &str) -> Result<Self, InvalidText> {
        if !front_matter_rules::is_date(text.as_bytes()) {
            return Err(InvalidText::Date);
        }
        let year = decimal(text.get(..YEAR_END));
        let month = two_digits(text.get(MONTH_START..MONTH_END));
        let day = two_digits(text.get(DAY_START..DAY_END));
        match (year, month, day) {
            (Some(year), Some(month), Some(day)) => Ok(Self { year, month, day }),
            (None, _, _) | (_, None, _) | (_, _, None) => Err(InvalidText::Date),
        }
    }

    #[must_use]
    pub const fn year(self) -> u16 {
        self.year
    }

    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }
}

impl fmt::Display for Date {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// The primary languages whose text the compiler lays out with East Asian line breaking.
const EAST_ASIAN_LANGUAGES: [&str; 3] = ["ja", "zh", "ko"];

/// A language tag: two or three lowercase letters, then an optional dash and two uppercase letters.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Lang(String);

impl Lang {
    /// # Errors
    ///
    /// Returns `InvalidText::Lang` when `text` leaves the grammar.
    pub fn new(text: &str) -> Result<Self, InvalidText> {
        if front_matter_rules::is_lang(text.as_bytes()) {
            Ok(Self(String::from(text)))
        } else {
            Err(InvalidText::Lang)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The language before any region, such as `en` in `en-US`.
    #[must_use]
    pub fn primary(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }

    /// Whether the language is Japanese, Chinese, or Korean.
    #[must_use]
    pub fn is_east_asian(&self) -> bool {
        EAST_ASIAN_LANGUAGES.contains(&self.primary())
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A relative path with a slash, which names a theme directory beside the deck.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ThemePath(String);

impl ThemePath {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where the theme comes from.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ThemeRef {
    /// A theme the binary carries, named by its identifier.
    Builtin(Identifier),
    /// A theme directory beside the deck.
    Path(ThemePath),
}

impl ThemeRef {
    /// # Errors
    ///
    /// Returns `InvalidText::AbsoluteTheme` for an absolute path and `InvalidText::Theme` for any other shape.
    pub fn new(text: &str) -> Result<Self, InvalidText> {
        match front_matter_rules::theme_shape(text.as_bytes()) {
            ThemeShape::Builtin => Identifier::new(text).map(Self::Builtin),
            ThemeShape::Relative => Ok(Self::Path(ThemePath(String::from(text)))),
            ThemeShape::Absolute => Err(InvalidText::AbsoluteTheme),
            ThemeShape::Invalid => Err(InvalidText::Theme),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Date, Identifier, InvalidText, Lang, ShortText, ThemeRef};
    use alloc::format;
    use alloc::string::String;

    #[test]
    fn each_value_is_admitted_by_its_grammar_and_reads_back() {
        assert_eq!(Identifier::new("two-up").unwrap().as_str(), "two-up");
        assert_eq!(Identifier::new("Two"), Err(InvalidText::Identifier));
        assert_eq!(format!("{}", Identifier::new("a").unwrap()), "a");
        assert_eq!(ShortText::new("発表").unwrap().as_str(), "発表");
        assert_eq!(ShortText::new(""), Err(InvalidText::ShortText));
        assert_eq!(ShortText::new("a\nb"), Err(InvalidText::ShortText));
        assert_eq!(
            ShortText::new(&"x".repeat(201)),
            Err(InvalidText::ShortText)
        );
        assert_eq!(
            ShortText::new(&"x".repeat(200)).map(|text| text.as_str().len()),
            Ok(200)
        );
        let date = Date::new("2026-10-13").unwrap();
        assert_eq!((date.year(), date.month(), date.day()), (2026, 10, 13));
        assert_eq!(format!("{date}"), "2026-10-13");
        assert_eq!(Date::new("2026-13-13"), Err(InvalidText::Date));
        let lang = Lang::new("en-US").unwrap();
        assert_eq!(lang.primary(), "en");
        assert_eq!(lang.as_str(), "en-US");
        assert!(!lang.is_east_asian());
        assert!(Lang::new("ja").unwrap().is_east_asian());
        assert!(Lang::new("zh-TW").unwrap().is_east_asian());
        assert_eq!(format!("{lang}"), "en-US");
        assert_eq!(Lang::new("JA"), Err(InvalidText::Lang));
    }

    #[test]
    fn a_theme_reference_is_a_built_in_name_or_a_relative_path() {
        assert_eq!(
            ThemeRef::new("plain"),
            Ok(ThemeRef::Builtin(Identifier::new("plain").unwrap()))
        );
        assert!(
            matches!(ThemeRef::new("themes/corp"), Ok(ThemeRef::Path(path)) if path.as_str() == "themes/corp")
        );
        assert_eq!(ThemeRef::new("/themes"), Err(InvalidText::AbsoluteTheme));
        assert_eq!(ThemeRef::new("Corp"), Err(InvalidText::Theme));
        assert_eq!(
            ThemeRef::new(String::new().as_str()),
            Err(InvalidText::Theme)
        );
    }
}
