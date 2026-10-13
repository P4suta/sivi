//! The units a theme writes: lengths in points, fractions of an em, percentages of a page side, and colors.

use core::fmt;
use sivi_core::length::Length;
use sivi_core::theme::{Color, Thousandths};

/// A measure a theme writes, with its suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Unit {
    /// `<n>pt`
    Points(Length),
    /// `<n>em`
    Em(Thousandths),
    /// `<n>%`
    Percent(Thousandths),
}

/// Why a unit was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UnitError {
    /// No digits before the suffix.
    Empty,
    /// A character that is not a digit or one decimal point.
    Number,
    /// More than three digits after the decimal point.
    Precision,
    /// A number too large to hold.
    Range,
    /// Neither `pt`, `em`, nor `%`.
    Suffix,
}

/// How many digits after the decimal point a unit may carry.
pub const FRACTION_DIGITS: usize = 3;
const DECIMAL: i64 = 10;
const SCALE: i64 = 1000;
const POINTS: &str = "pt";
const EM: &str = "em";
const PERCENT: &str = "%";

fn digits(text: &str) -> Result<i64, UnitError> {
    let mut value: i64 = 0;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(UnitError::Number);
        }
        value = value
            .checked_mul(DECIMAL)
            .and_then(|shifted| shifted.checked_add(i64::from(byte.wrapping_sub(b'0'))))
            .ok_or(UnitError::Range)?;
    }
    Ok(value)
}

/// The thousandths `text` spells as a decimal with up to three digits after the point, such as `1.45`.
///
/// # Errors
///
/// Returns why `text` is not such a decimal.
pub fn decimal(text: &str) -> Result<Thousandths, UnitError> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && fraction.is_empty() {
        return Err(UnitError::Empty);
    }
    if fraction.len() > FRACTION_DIGITS {
        return Err(UnitError::Precision);
    }
    let whole = digits(whole)?;
    let mut fraction_value = digits(fraction)?;
    for _ in fraction.len()..FRACTION_DIGITS {
        fraction_value = fraction_value
            .checked_mul(DECIMAL)
            .ok_or(UnitError::Range)?;
    }
    whole
        .checked_mul(SCALE)
        .and_then(|scaled| scaled.checked_add(fraction_value))
        .map(Thousandths)
        .ok_or(UnitError::Range)
}

/// How a suffix turns a decimal into its unit.
type Make = fn(Thousandths) -> Unit;

/// The unit `text` spells: a decimal followed by `pt`, `em`, or `%`.
///
/// # Errors
///
/// Returns why `text` is not a unit.
pub fn parse(text: &str) -> Result<Unit, UnitError> {
    let suffixes: [(&str, Make); 3] = [
        (POINTS, |value| {
            Unit::Points(Length::from_millipoints(value.0))
        }),
        (EM, Unit::Em),
        (PERCENT, Unit::Percent),
    ];
    let (number, make) = suffixes
        .into_iter()
        .find_map(|(suffix, make)| text.strip_suffix(suffix).map(|number| (number, make)))
        .ok_or(UnitError::Suffix)?;
    decimal(number).map(make)
}

fn write_decimal(formatter: &mut fmt::Formatter<'_>, value: i64) -> fmt::Result {
    let whole = value.checked_div(SCALE).unwrap_or_default();
    let fraction = value.checked_rem(SCALE).unwrap_or_default();
    if fraction == 0 {
        write!(formatter, "{whole}")
    } else {
        let padded = format!("{fraction:03}");
        write!(formatter, "{whole}.{}", padded.trim_end_matches('0'))
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Points(length) => {
                write_decimal(formatter, length.millipoints())?;
                formatter.write_str(POINTS)
            }
            Self::Em(value) => {
                write_decimal(formatter, value.0)?;
                formatter.write_str(EM)
            }
            Self::Percent(value) => {
                write_decimal(formatter, value.0)?;
                formatter.write_str(PERCENT)
            }
        }
    }
}

const HEX: u8 = 16;
/// The value of the hexadecimal digit `a`.
const LETTER_BASE: u8 = 10;

/// The value of one hexadecimal digit, in either case.
const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte.wrapping_sub(b'0')),
        b'a'..=b'f' => Some(byte.wrapping_sub(b'a').wrapping_add(LETTER_BASE)),
        b'A'..=b'F' => Some(byte.wrapping_sub(b'A').wrapping_add(LETTER_BASE)),
        _ => None,
    }
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    hex_digit(high)?
        .checked_mul(HEX)?
        .checked_add(hex_digit(low)?)
}

/// The color `text` spells as `#rrggbb`.
#[must_use]
pub fn color(text: &str) -> Option<Color> {
    let hex = text.strip_prefix('#')?;
    let [r1, r2, g1, g2, b1, b2] = hex.as_bytes() else {
        return None;
    };
    Some(Color {
        red: hex_pair(*r1, *r2)?,
        green: hex_pair(*g1, *g2)?,
        blue: hex_pair(*b1, *b2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{Unit, UnitError, color, decimal, parse};
    use sivi_core::length::Length;
    use sivi_core::theme::{Color, Thousandths};

    #[test]
    fn decimals_and_units_read_and_print_back() {
        assert_eq!(decimal("1.45"), Ok(Thousandths(1450)));
        assert_eq!(decimal("24"), Ok(Thousandths(24_000)));
        assert_eq!(decimal("0.5"), Ok(Thousandths(500)));
        assert_eq!(decimal(".5"), Ok(Thousandths(500)));
        assert_eq!(decimal("5."), Ok(Thousandths(5000)));
        assert_eq!(decimal(""), Err(UnitError::Empty));
        assert_eq!(decimal("."), Err(UnitError::Empty));
        assert_eq!(decimal("1.2345"), Err(UnitError::Precision));
        assert_eq!(decimal("-1"), Err(UnitError::Number));
        assert_eq!(decimal("1e3"), Err(UnitError::Number));
        assert_eq!(decimal("99999999999999999999"), Err(UnitError::Range));
    }

    #[test]
    fn units_read_their_suffix_and_print_back() {
        assert_eq!(
            parse("24pt"),
            Ok(Unit::Points(Length::from_millipoints(24_000)))
        );
        assert_eq!(parse("0.6em"), Ok(Unit::Em(Thousandths(600))));
        assert_eq!(parse("50%"), Ok(Unit::Percent(Thousandths(50_000))));
        assert_eq!(parse("24"), Err(UnitError::Suffix));
        assert_eq!(parse("24px"), Err(UnitError::Suffix));
        assert_eq!(parse("pt"), Err(UnitError::Empty));
        for text in ["24pt", "0.6em", "50%", "12.5pt", "1.005em"] {
            let unit = parse(text).unwrap();
            assert_eq!(format!("{unit}"), text);
            assert_eq!(parse(&format!("{unit}")), Ok(unit));
        }
        assert_eq!(
            format!("{}", Unit::Points(Length::from_millipoints(1_050))),
            "1.05pt"
        );
    }

    #[test]
    fn colors_read_six_hexadecimal_digits() {
        assert_eq!(
            color("#0b57d0"),
            Some(Color {
                red: 0x0b,
                green: 0x57,
                blue: 0xd0
            })
        );
        assert_eq!(
            color("#FFFFFF"),
            Some(Color {
                red: 255,
                green: 255,
                blue: 255
            })
        );
        assert_eq!(color("0b57d0"), None);
        assert_eq!(color("#0b57d"), None);
        assert_eq!(color("#0b57d0ff"), None);
        assert_eq!(color("#0g57d0"), None);
        assert_eq!(color("#日本語日本語"), None);
    }
}
