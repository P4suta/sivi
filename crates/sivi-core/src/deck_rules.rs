//! How the scanner reads one line of a deck: a separator, a code fence, a key and value, a blank, or content.

/// The three dashes that separate slides and fence front matter, on a line of their own.
pub const SEPARATOR: &[u8; 3] = b"---";

/// How many leading spaces a code fence may carry, as CommonMark allows.
pub const MAX_FENCE_INDENT: usize = 3;

/// How many fence characters open a code block.
pub const MIN_FENCE: usize = 3;

/// The character a code fence repeats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Marker {
    Backtick,
    Tilde,
}

impl Marker {
    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            Self::Backtick => b'`',
            Self::Tilde => b'~',
        }
    }
}

/// An open code fence, which only a fence of the same marker at least as long closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct CodeFence {
    pub marker: Marker,
    pub length: usize,
}

/// Where the key and the value of a key-value line sit, as byte offsets into the line.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct KeyValue {
    /// One past the last byte of the key, which the colon follows.
    pub key_end: usize,
    /// The first byte of the value.
    pub value_start: usize,
    /// One past the last byte of the value, before any trailing blanks.
    pub value_end: usize,
}

/// What one line of a deck is to the scanner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LineKind {
    Blank,
    Separator,
    CodeFence(CodeFence),
    KeyValue(KeyValue),
    Content,
}

const SPACE: u8 = b' ';
const TAB: u8 = b'\t';
const COLON: u8 = b':';
const UNDERSCORE: u8 = b'_';

const fn is_blank(byte: u8) -> bool {
    byte == SPACE || byte == TAB
}

fn all_blank(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| is_blank(*byte))
}

const fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte == UNDERSCORE
}

/// A run of fence characters at the start of `bytes` after up to three spaces: the marker, its length, and the rest.
fn fence_run(bytes: &[u8]) -> Option<(Marker, usize, &[u8])> {
    let indent = bytes.iter().take_while(|byte| **byte == SPACE).count();
    if indent > MAX_FENCE_INDENT {
        return None;
    }
    let rest = bytes.get(indent..)?;
    let marker = match rest.first()? {
        b'`' => Marker::Backtick,
        b'~' => Marker::Tilde,
        _ => return None,
    };
    let length = rest
        .iter()
        .take_while(|byte| **byte == marker.byte())
        .count();
    if length < MIN_FENCE {
        return None;
    }
    Some((marker, length, rest.get(length..)?))
}

/// The key and value offsets of `line`, if it reads `key: value` with a lowercase key and a nonblank value.
fn key_value(line: &[u8]) -> Option<KeyValue> {
    let key_end = line.iter().take_while(|byte| is_key_byte(**byte)).count();
    if key_end == 0 {
        return None;
    }
    let (colon, after_colon) = line.get(key_end..)?.split_first()?;
    if *colon != COLON {
        return None;
    }
    let blanks = after_colon
        .iter()
        .take_while(|byte| is_blank(**byte))
        .count();
    if blanks == 0 {
        return None;
    }
    let value_start = key_end.checked_add(1)?.checked_add(blanks)?;
    let value = line.get(value_start..)?;
    let trailing = value
        .iter()
        .rev()
        .take_while(|byte| is_blank(**byte))
        .count();
    let value_end = line.len().checked_sub(trailing)?;
    if value_end <= value_start {
        return None;
    }
    Some(KeyValue {
        key_end,
        value_start,
        value_end,
    })
}

/// What `line`, given without its terminator, is to the scanner.
#[must_use]
pub fn classify(line: &[u8]) -> LineKind {
    if all_blank(line) {
        return LineKind::Blank;
    }
    if let Some(rest) = line.strip_prefix(SEPARATOR)
        && all_blank(rest)
    {
        return LineKind::Separator;
    }
    if let Some((marker, length, info)) = fence_run(line)
        && (marker == Marker::Tilde || !info.contains(&Marker::Backtick.byte()))
    {
        return LineKind::CodeFence(CodeFence { marker, length });
    }
    if let Some(found) = key_value(line) {
        return LineKind::KeyValue(found);
    }
    LineKind::Content
}

/// Whether `line` closes `open`: the same marker at least as long after up to three spaces, then only blanks.
#[must_use]
pub fn closes(open: CodeFence, line: &[u8]) -> bool {
    fence_run(line).is_some_and(|(marker, length, rest)| {
        marker == open.marker && length >= open.length && all_blank(rest)
    })
}

const PIPE: u8 = b'|';
const ESCAPE: u8 = b'\\';

/// How many cells a table row line declares under GFM: its unescaped pipes plus one, less a leading and a trailing pipe.
#[must_use]
pub fn table_cells(line: &[u8]) -> usize {
    let leading_blanks = line.iter().take_while(|byte| is_blank(**byte)).count();
    let trailing_blanks = line
        .iter()
        .rev()
        .take_while(|byte| is_blank(**byte))
        .count();
    let Some(trimmed) = line.get(leading_blanks..line.len().saturating_sub(trailing_blanks)) else {
        return 0;
    };
    if trimmed.is_empty() {
        return 0;
    }
    let mut pipes: usize = 0;
    let mut escaped = false;
    let mut ends_with_pipe = false;
    for byte in trimmed {
        ends_with_pipe = false;
        if escaped {
            escaped = false;
        } else if *byte == ESCAPE {
            escaped = true;
        } else if *byte == PIPE {
            pipes = pipes.saturating_add(1);
            ends_with_pipe = true;
        }
    }
    let leading = usize::from(trimmed.first() == Some(&PIPE));
    let trailing = usize::from(ends_with_pipe && trimmed.len() > 1);
    pipes
        .saturating_add(1)
        .saturating_sub(leading)
        .saturating_sub(trailing)
}

/// What a separator means after the line before it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Separation {
    Separator,
    /// The dashes sit right under text, where CommonMark would read a setext heading.
    UnderlinesContent,
}

/// What a separator means after `previous`, the kind of the line before it, or `None` at the start of the deck.
#[must_use]
pub const fn separation(previous: Option<LineKind>) -> Separation {
    match previous {
        Some(LineKind::Content | LineKind::KeyValue(_)) => Separation::UnderlinesContent,
        Some(LineKind::Blank | LineKind::Separator | LineKind::CodeFence(_)) | None => {
            Separation::Separator
        }
    }
}

/// The properties Kani proves for every short line and the tests check over the lines they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{
        CodeFence, ESCAPE, LineKind, MAX_FENCE_INDENT, PIPE, SEPARATOR, classify, is_blank,
        is_key_byte,
    };
    #[cfg(kani)]
    use super::{MIN_FENCE, Marker, closes, table_cells};

    /// How many bytes the bounded proofs admit.
    #[cfg(kani)]
    const SAMPLE: usize = 6;

    /// The longest fence the bounded proofs open.
    #[cfg(kani)]
    const LONGEST_FENCE: usize = 5;

    /// A separator, stated as the bytes it holds.
    pub(super) fn spec_separator(line: &[u8]) -> bool {
        line.len() >= SEPARATOR.len()
            && line.iter().zip(SEPARATOR.iter()).all(|(a, b)| a == b)
            && line
                .iter()
                .skip(SEPARATOR.len())
                .all(|byte| is_blank(*byte))
    }

    /// A key-value verdict bounds a lowercase key, a colon, and a value trimmed at both ends.
    pub(super) fn spec_key_value(line: &[u8]) {
        if let LineKind::KeyValue(found) = classify(line) {
            assert!(0 < found.key_end);
            assert!(found.key_end < found.value_start);
            assert!(found.value_start < found.value_end);
            assert!(found.value_end <= line.len());
            assert!(
                line.get(..found.key_end)
                    .is_some_and(|key| key.iter().all(|byte| is_key_byte(*byte)))
            );
            assert_eq!(line.get(found.key_end), Some(&b':'));
            assert!(
                line.get(found.value_start)
                    .is_some_and(|byte| !is_blank(*byte))
            );
            assert!(
                line.get(found.value_end.wrapping_sub(1))
                    .is_some_and(|byte| !is_blank(*byte))
            );
        }
    }

    /// Whether `line` reads as a closing fence for `open` under CommonMark: the marker repeated enough, then blanks.
    pub(super) fn spec_closes(open: CodeFence, line: &[u8]) -> bool {
        let indent = line.iter().take_while(|byte| **byte == b' ').count();
        let rest = line.get(indent..).unwrap_or_default();
        let run = rest
            .iter()
            .take_while(|byte| **byte == open.marker.byte())
            .count();
        indent <= MAX_FENCE_INDENT
            && run >= open.length
            && rest.iter().skip(run).all(|byte| is_blank(*byte))
    }

    #[cfg(kani)]
    fn any_line() -> ([u8; SAMPLE], usize) {
        let bytes: [u8; SAMPLE] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= SAMPLE);
        (bytes, len)
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(8)]
    fn a_separator_is_three_dashes_and_blanks_and_nothing_else() {
        let (bytes, len) = any_line();
        let Some(line) = bytes.get(..len) else {
            return;
        };
        assert_eq!(classify(line) == LineKind::Separator, spec_separator(line));
        kani::cover!(classify(line) == LineKind::Separator);
        kani::cover!(classify(line) == LineKind::Content);
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(8)]
    fn a_key_value_line_bounds_a_lowercase_key_and_a_trimmed_value() {
        let (bytes, len) = any_line();
        let Some(line) = bytes.get(..len) else {
            return;
        };
        spec_key_value(line);
        kani::cover!(matches!(classify(line), LineKind::KeyValue(_)));
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(8)]
    fn a_fence_closes_only_with_its_marker_at_its_length_or_more() {
        let open: CodeFence = kani::any();
        kani::assume(open.length >= MIN_FENCE && open.length <= LONGEST_FENCE);
        let (bytes, len) = any_line();
        let Some(line) = bytes.get(..len) else {
            return;
        };
        assert_eq!(closes(open, line), spec_closes(open, line));
        kani::cover!(closes(open, line) && open.marker == Marker::Tilde);
        kani::cover!(closes(open, line) && open.marker == Marker::Backtick);
        kani::cover!(!closes(open, line) && classify(line) != LineKind::Content);
    }

    /// Whether the byte at `index` of `trimmed` is a pipe with an even run of backslashes right before it, which no escape reaches.
    fn unescaped_pipe(trimmed: &[u8], index: usize) -> bool {
        let backslashes = trimmed.get(..index).map_or(0, |before| {
            before
                .iter()
                .rev()
                .take_while(|byte| **byte == ESCAPE)
                .count()
        });
        trimmed.get(index) == Some(&PIPE) && backslashes & 1 == 0
    }

    /// The cell count as GFM reads a row, stated through the parity of the backslashes before each pipe.
    pub(super) fn spec_table_cells(line: &[u8]) -> usize {
        let start = line.iter().take_while(|byte| is_blank(**byte)).count();
        let end = line.len().saturating_sub(
            line.iter()
                .rev()
                .take_while(|byte| is_blank(**byte))
                .count(),
        );
        let Some(trimmed) = line.get(start..end) else {
            return 0;
        };
        if trimmed.is_empty() {
            return 0;
        }
        let pipes = (0..trimmed.len())
            .filter(|index| unescaped_pipe(trimmed, *index))
            .count();
        let leading = usize::from(unescaped_pipe(trimmed, 0));
        let trailing = usize::from(
            trimmed.len() > 1 && unescaped_pipe(trimmed, trimmed.len().saturating_sub(1)),
        );
        pipes
            .saturating_add(1)
            .saturating_sub(leading)
            .saturating_sub(trailing)
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(8)]
    fn a_row_declares_its_unescaped_pipes_plus_one_less_its_edge_pipes() {
        let (bytes, len) = any_line();
        let Some(line) = bytes.get(..len) else {
            return;
        };
        assert_eq!(table_cells(line), spec_table_cells(line));
        kani::cover!(table_cells(line) == 0);
        kani::cover!(table_cells(line) == 1);
        kani::cover!(table_cells(line) >= 2 && line.contains(&ESCAPE));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_dashes_followed_by_text_as_a_separator() {
        kani::assert(
            classify(b"--- x") == LineKind::Separator,
            "NEGATIVE_DECK_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CodeFence, KeyValue, LineKind, Marker, Separation, classify, closes, proofs, separation,
        table_cells,
    };

    #[test]
    fn each_line_kind_is_told_apart() {
        assert_eq!(classify(b""), LineKind::Blank);
        assert_eq!(classify(b" \t "), LineKind::Blank);
        assert_eq!(classify(b"---"), LineKind::Separator);
        assert_eq!(classify(b"---  "), LineKind::Separator);
        assert_eq!(classify(b"----"), LineKind::Content);
        assert_eq!(classify(b" ---"), LineKind::Content);
        assert_eq!(classify(b"--- x"), LineKind::Content);
        assert_eq!(classify(b"***"), LineKind::Content);
        assert_eq!(
            classify(b"```rust"),
            LineKind::CodeFence(CodeFence {
                marker: Marker::Backtick,
                length: 3
            })
        );
        assert_eq!(
            classify(b"   ~~~~"),
            LineKind::CodeFence(CodeFence {
                marker: Marker::Tilde,
                length: 4
            })
        );
        assert_eq!(classify(b"    ```"), LineKind::Content);
        assert_eq!(classify(b"``"), LineKind::Content);
        assert_eq!(classify(b"``` a ` b"), LineKind::Content);
        assert_eq!(
            classify(b"~~~ a ` b"),
            LineKind::CodeFence(CodeFence {
                marker: Marker::Tilde,
                length: 3
            })
        );
    }

    #[test]
    fn a_key_value_line_is_told_apart() {
        assert_eq!(
            classify(b"master: bullets  "),
            LineKind::KeyValue(KeyValue {
                key_end: 6,
                value_start: 8,
                value_end: 15
            })
        );
        assert_eq!(
            classify("title: 日本語".as_bytes()),
            LineKind::KeyValue(KeyValue {
                key_end: 5,
                value_start: 7,
                value_end: 16
            })
        );
        assert_eq!(classify(b"master:bullets"), LineKind::Content);
        assert_eq!(
            classify(b"id:\t \tx"),
            LineKind::KeyValue(KeyValue {
                key_end: 2,
                value_start: 6,
                value_end: 7
            })
        );
        assert_eq!(classify(b"master: "), LineKind::Content);
        assert_eq!(classify(b"Master: a"), LineKind::Content);
        assert_eq!(classify(b"m-aster: a"), LineKind::Content);
        assert_eq!(classify(b": a"), LineKind::Content);
        assert_eq!(classify(b"# Title"), LineKind::Content);
        for line in [
            &b"master: bullets  "[..],
            b"a: b",
            b"id: x y ",
            b"k:  v\t",
            b"--- x",
            b"",
        ] {
            proofs::spec_key_value(line);
            assert_eq!(
                classify(line) == LineKind::Separator,
                proofs::spec_separator(line)
            );
        }
    }

    #[test]
    fn a_fence_closes_with_its_own_marker_at_its_length_or_more() {
        let open = CodeFence {
            marker: Marker::Backtick,
            length: 3,
        };
        for (line, expected) in [
            (&b"```"[..], true),
            (b"````", true),
            (b"  ``` ", true),
            (b"    ```", false),
            (b"``", false),
            (b"~~~", false),
            (b"``` rust", false),
            (b"", false),
        ] {
            assert_eq!(closes(open, line), expected, "{line:?}");
            assert_eq!(
                closes(open, line),
                proofs::spec_closes(open, line),
                "{line:?}"
            );
        }
        let long = CodeFence {
            marker: Marker::Tilde,
            length: 4,
        };
        assert!(!closes(long, b"~~~"));
        assert!(closes(long, b"~~~~~"));
    }

    #[test]
    fn a_table_row_counts_its_cells_as_gfm_does() {
        for (line, expected) in [
            (&b"| a | b |"[..], 2),
            (b"a | b", 2),
            (b"| a | b", 2),
            (b"a | b |", 2),
            (b"| a \\| b |", 1),
            (b"| `a|b` |", 2),
            (b"|", 1),
            (b"||", 1),
            (b"a", 1),
            (b"  | a |  ", 1),
            (b"", 0),
            (b"   ", 0),
            (b"a \\|", 1),
            (b"a \\\\|", 1),
        ] {
            assert_eq!(table_cells(line), expected, "{line:?}");
            assert_eq!(
                table_cells(line),
                proofs::spec_table_cells(line),
                "{line:?}"
            );
        }
    }

    #[test]
    fn a_separator_under_text_is_told_from_one_after_a_blank() {
        assert_eq!(separation(None), Separation::Separator);
        assert_eq!(separation(Some(LineKind::Blank)), Separation::Separator);
        assert_eq!(separation(Some(LineKind::Separator)), Separation::Separator);
        assert_eq!(
            separation(Some(LineKind::CodeFence(CodeFence {
                marker: Marker::Tilde,
                length: 3
            }))),
            Separation::Separator
        );
        assert_eq!(
            separation(Some(LineKind::Content)),
            Separation::UnderlinesContent
        );
        assert_eq!(
            separation(Some(LineKind::KeyValue(KeyValue {
                key_end: 1,
                value_start: 3,
                value_end: 4
            }))),
            Separation::UnderlinesContent
        );
    }
}
