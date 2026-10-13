//! Turning author text into a Typst string literal that Typst reads back as exactly that text.
//!
//! Author text is emitted only inside string literals in code mode, so this is the one place markup injection could happen.
//! Every character is escaped on its own by [`escaped`], and a literal is the concatenation of those spellings between quotes.

use alloc::string::String;
use core::fmt;

/// A Typst string literal, delimited and escaped, which Typst evaluates to the text it was made from.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TypstString(String);

impl TypstString {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TypstString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

const QUOTE: char = '"';
const BACKSLASH: char = '\\';
const HEX_RADIX: u32 = 16;
const LOW_NIBBLE: u32 = 0xf;
/// The shifts that bring each nibble of a code point to the low bits, most significant first.
const NIBBLE_SHIFTS: [u32; 8] = [28, 24, 20, 16, 12, 8, 4, 0];

/// The most characters one escaped character takes: `\u{10ffff}`.
pub const MAX_ESCAPE: usize = 10;

/// The spelling of one character inside a literal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Escaped {
    chars: [char; MAX_ESCAPE],
    len: usize,
}

impl Escaped {
    const fn empty() -> Self {
        Self {
            chars: ['\0'; MAX_ESCAPE],
            len: 0,
        }
    }

    fn push(mut self, character: char) -> Self {
        if let Some(slot) = self.chars.get_mut(self.len) {
            *slot = character;
            self.len = self.len.saturating_add(1);
        }
        self
    }

    /// The character at `index`, or the null character past the end.
    fn at(&self, index: usize) -> char {
        self.as_chars().get(index).copied().unwrap_or_default()
    }

    /// The characters of the spelling, in order.
    #[must_use]
    pub fn as_chars(&self) -> &[char] {
        self.chars.get(..self.len).map_or(&[], |chars| chars)
    }
}

/// How one character is written inside the literal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Written {
    /// The character itself.
    Plain,
    /// A backslash and the character itself.
    Escaped,
    /// A named escape, such as `\n`.
    Named(char),
    /// A `\u{..}` escape with the code point in hexadecimal.
    CodePoint,
}

const fn written(character: char) -> Written {
    match character {
        QUOTE | BACKSLASH => Written::Escaped,
        '\n' => Written::Named('n'),
        '\r' => Written::Named('r'),
        '\t' => Written::Named('t'),
        _ if character.is_control() => Written::CodePoint,
        _ => Written::Plain,
    }
}

/// The lowercase hexadecimal digit of a value below sixteen.
fn hex_digit(nibble: u32) -> char {
    char::from_digit(nibble, HEX_RADIX).unwrap_or('0')
}

/// The hexadecimal digits of `code`, most significant first and without leading zeros.
///
/// The walk visits every nibble once, so no edit to its conditions can keep it running.
fn hex_digits(code: u32) -> Escaped {
    let mut digits = Escaped::empty();
    for shift in NIBBLE_SHIFTS {
        let nibble = code.wrapping_shr(shift) & LOW_NIBBLE;
        if nibble != 0 || digits.len != 0 || shift == 0 {
            digits = digits.push(hex_digit(nibble));
        }
    }
    digits
}

/// The spelling of `character` inside a literal.
#[must_use]
pub fn escaped(character: char) -> Escaped {
    match written(character) {
        Written::Plain => Escaped::empty().push(character),
        Written::Escaped => Escaped::empty().push(BACKSLASH).push(character),
        Written::Named(name) => Escaped::empty().push(BACKSLASH).push(name),
        Written::CodePoint => {
            let mut spelling = Escaped::empty().push(BACKSLASH).push('u').push('{');
            let digits = hex_digits(u32::from(character));
            let mut index = 0;
            while index < digits.len {
                spelling = spelling.push(digits.at(index));
                index = index.saturating_add(1);
            }
            spelling.push('}')
        }
    }
}

/// The literal whose value is `text`.
#[must_use]
pub fn string_literal(text: &str) -> TypstString {
    let mut literal = String::new();
    literal.push(QUOTE);
    for character in text.chars() {
        literal.extend(escaped(character).as_chars());
    }
    literal.push(QUOTE);
    TypstString(literal)
}

/// The properties Kani proves for every character and the tests check over the texts they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{BACKSLASH, HEX_RADIX, QUOTE, escaped, string_literal};
    #[cfg(test)]
    use alloc::string::String;

    /// Reads one escaped character back the way Typst does, or `None` when the spelling is not exactly one character.
    pub(super) fn decode_one(spelling: &[char]) -> Option<char> {
        let mut characters = spelling.iter().copied();
        let first = characters.next()?;
        let decoded = if first == BACKSLASH {
            match characters.next()? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => {
                    if characters.next()? != '{' {
                        return None;
                    }
                    let mut value: u32 = 0;
                    let mut closed = false;
                    for digit in characters.by_ref() {
                        if digit == '}' {
                            closed = true;
                            break;
                        }
                        value = value
                            .checked_mul(HEX_RADIX)?
                            .checked_add(digit.to_digit(HEX_RADIX)?)?;
                    }
                    if !closed {
                        return None;
                    }
                    char::from_u32(value)?
                }
                escaped @ (QUOTE | BACKSLASH) => escaped,
                _ => return None,
            }
        } else {
            first
        };
        if characters.next().is_some() {
            return None;
        }
        Some(decoded)
    }

    /// Reads a literal back, or `None` when it is not one complete literal.
    #[cfg(test)]
    pub(super) fn decode(literal: &str) -> Option<String> {
        let inner = literal.strip_prefix(QUOTE)?.strip_suffix(QUOTE)?;
        let mut text = String::new();
        let mut characters = inner.chars();
        while let Some(character) = characters.next() {
            if character == QUOTE {
                return None;
            }
            if character != BACKSLASH {
                text.push(character);
                continue;
            }
            let mut spelling = alloc::vec![BACKSLASH];
            let escaped = characters.next()?;
            spelling.push(escaped);
            if escaped == 'u' {
                for next in characters.by_ref() {
                    spelling.push(next);
                    if next == '}' {
                        break;
                    }
                }
            }
            text.push(decode_one(&spelling)?);
        }
        Some(text)
    }

    pub(super) fn spec_escaped(character: char) {
        let spelling = escaped(character);
        let chars = spelling.as_chars();
        assert_ne!(chars.len(), 0);
        assert_eq!(decode_one(chars), Some(character));
        for written in chars {
            assert!(!written.is_control());
        }
        let quotes = chars.iter().filter(|written| **written == QUOTE).count();
        assert!(quotes == 0 || (chars.len() == 2 && chars.first() == Some(&BACKSLASH)));
    }

    #[cfg(test)]
    pub(super) fn spec_round_trip(text: &str) {
        let literal = string_literal(text);
        let written = literal.as_str();
        assert!(written.starts_with(QUOTE) && written.ends_with(QUOTE));
        assert!(written.len() >= 2);
        assert_eq!(decode(written).as_deref(), Some(text));
        for character in written.chars() {
            assert!(!character.is_control());
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(14)]
    fn every_character_reads_back_from_its_spelling_without_a_raw_control_or_quote() {
        let character: char = kani::any();
        spec_escaped(character);
        kani::cover!(character == QUOTE);
        kani::cover!(character == '\n');
        kani::cover!(character == '\u{9f}');
        kani::cover!(character == '日');
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_literal_with_an_unescaped_quote() {
        let literal = string_literal("a\"b");
        kani::assert(literal.as_str() == "\"a\"b\"", "NEGATIVE_ESCAPE_CONTROL");
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_ESCAPE, TypstString, escaped, proofs, string_literal};
    use alloc::format;

    #[test]
    fn every_listed_character_and_text_round_trips() {
        for character in [
            'a',
            '"',
            '\\',
            '\n',
            '\r',
            '\t',
            '\0',
            '\u{1}',
            '\u{1f}',
            '\u{7f}',
            '\u{80}',
            '\u{9f}',
            '\u{a0}',
            '日',
            '🦀',
            char::MAX,
        ] {
            proofs::spec_escaped(character);
            assert!(escaped(character).as_chars().len() <= MAX_ESCAPE);
        }
        for text in [
            "",
            "plain",
            "say \"hi\"",
            "back\\slash",
            "line\nbreak\ttab\rreturn",
            "control\u{1}\u{7f}\u{85}end",
            "日本語の「引用」と、記号。",
            "#not[markup] *at* _all_ `raw` <label> @ref $math$ \\",
        ] {
            proofs::spec_round_trip(text);
        }
    }

    #[test]
    fn markup_characters_stay_inert_and_escapes_are_spelled_for_typst() {
        assert_eq!(string_literal("a\"b").as_str(), "\"a\\\"b\"");
        assert_eq!(string_literal("a\\b").as_str(), "\"a\\\\b\"");
        assert_eq!(string_literal("a\nb").as_str(), "\"a\\nb\"");
        assert_eq!(string_literal("a\rb").as_str(), "\"a\\rb\"");
        assert_eq!(string_literal("a\tb").as_str(), "\"a\\tb\"");
        assert_eq!(string_literal("\u{1}").as_str(), "\"\\u{1}\"");
        assert_eq!(string_literal("\u{9f}").as_str(), "\"\\u{9f}\"");
        assert_eq!(string_literal("\0").as_str(), "\"\\u{0}\"");
        assert_eq!(string_literal("#f[x]").as_str(), "\"#f[x]\"");
        let literal: TypstString = string_literal("x");
        assert_eq!(format!("{literal}"), "\"x\"");
        assert_eq!(proofs::decode("\"a"), None);
        assert_eq!(proofs::decode("\"a\"b\""), None);
        assert_eq!(proofs::decode("\"\\q\""), None);
        assert_eq!(proofs::decode("\"\\u{1\""), None);
        assert_eq!(proofs::decode_one(&['a', 'b']), None);
    }
}
