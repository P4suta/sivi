//! The deck scanner: slides split at separators outside code fences, with front matter read and masked in place.
//!
//! Every slide keeps the byte width of each of its lines, so the positions the parser reports map straight back to the deck.

use sivi_core::deck_rules::{self, CodeFence, KeyValue, LineKind, Separation};
use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::front_matter::Entry;
use sivi_core::front_matter_rules::{Key, Position};
use sivi_core::span::{LineColumn, Span};

/// One slide as scanned: where it starts, its text with the front matter masked, and the entries the front matter held.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawSlide {
    /// The deck line the slide's text starts on, counted from one.
    pub first_line: usize,
    /// The slide's lines, each ended by a newline, with every front matter line replaced by spaces of the same width.
    pub text: String,
    pub front: Vec<Entry>,
    /// From the slide's first line to its last, in the deck.
    pub span: Span,
}

/// What the scanner found: the head entries, the slides, and every diagnostic of the deck's shape.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Scan {
    pub head: Vec<Entry>,
    pub slides: Vec<RawSlide>,
    pub diagnostics: Vec<Diagnostic>,
}

/// One line of the deck without its terminator.
#[derive(Clone, Copy, Debug)]
struct Line<'a> {
    /// Counted from one.
    number: usize,
    text: &'a str,
    kind: LineKind,
}

/// The lines of `text`, numbered from one, with `\r\n` and `\n` both read as a line end.
fn lines(text: &str) -> Vec<Line<'_>> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut pieces: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        pieces.pop();
    }
    pieces
        .into_iter()
        .enumerate()
        .map(|(index, piece)| {
            let without_return = piece.strip_suffix('\r').unwrap_or(piece);
            Line {
                number: index.saturating_add(1),
                text: without_return,
                kind: deck_rules::classify(without_return.as_bytes()),
            }
        })
        .collect()
}

fn entry(line: Line<'_>, found: KeyValue) -> Entry {
    Entry {
        key: String::from(line.text.get(..found.key_end).unwrap_or_default()),
        value: String::from(
            line.text
                .get(found.value_start..found.value_end)
                .unwrap_or_default(),
        ),
        span: Span::line(line.number, line.text.len()),
        value_span: Span::new(
            LineColumn::new(line.number, found.value_start.saturating_add(1)),
            LineColumn::new(line.number, found.value_end),
        ),
    }
}

fn unclosed(line: Line<'_>, what: &str) -> Diagnostic {
    Diagnostic::new(
        Code::UnclosedFrontMatter,
        format!("{what} has no closing `---`"),
    )
    .at(
        Span::line(line.number, line.text.len()),
        String::from("the front matter starts here"),
    )
    .help(String::from(
        "close the front matter with `---` on its own line right after its last `key: value` line",
    ))
}

fn malformed(line: Line<'_>) -> Diagnostic {
    Diagnostic::new(
        Code::MalformedKeyValue,
        String::from("a front matter line is `key: value`"),
    )
    .at(
        Span::line(line.number, line.text.len()),
        String::from("this line is neither `key: value` nor the closing `---`"),
    )
    .help(String::from(
        "write a lowercase key, a colon, a space, and the value, or close the front matter with `---`",
    ))
}

fn underlines_content(line: Line<'_>) -> Diagnostic {
    Diagnostic::new(
        Code::SetextHeading,
        String::from("`---` sits right under text, where CommonMark reads a heading underline"),
    )
    .at(
        Span::line(line.number, line.text.len()),
        String::from("this separator follows text without a blank line"),
    )
    .help(String::from(
        "put a blank line before `---` to separate slides, or write `##` before the text to make it a section",
    ))
}

struct Scanner<'a> {
    lines: &'a [Line<'a>],
    /// The index of the next line to read.
    at: usize,
    scan: Scan,
}

impl<'a> Scanner<'a> {
    const fn new(lines: &'a [Line<'a>]) -> Self {
        Self {
            lines,
            at: 0,
            scan: Scan {
                head: Vec::new(),
                slides: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    fn line(&self, index: usize) -> Option<Line<'a>> {
        self.lines.get(index).copied()
    }

    /// Reads the head of the deck when the first line is `---`: every line until the closing `---` is `key: value`.
    fn head_matter(&mut self) {
        let Some(opening) = self.line(0) else {
            return;
        };
        if opening.kind != LineKind::Separator {
            return;
        }
        self.at = 1;
        loop {
            let Some(line) = self.line(self.at) else {
                self.scan
                    .diagnostics
                    .push(unclosed(opening, "the head of the deck"));
                return;
            };
            self.at = self.at.saturating_add(1);
            match line.kind {
                LineKind::Separator => return,
                LineKind::KeyValue(found) => self.scan.head.push(entry(line, found)),
                LineKind::Blank | LineKind::Content | LineKind::CodeFence(_) => {
                    self.scan.diagnostics.push(malformed(line));
                }
            }
        }
    }

    /// Reads a slide's front matter when the slide starts with `key: value` lines closed by `---`.
    ///
    /// Lines that start with a slide key but never close are read as front matter too, with a diagnostic.
    fn front_matter(&mut self) -> (Vec<Entry>, usize) {
        let start = self.at;
        let mut count = 0;
        while let Some(line) = self.line(start.saturating_add(count))
            && matches!(line.kind, LineKind::KeyValue(_))
        {
            count = count.saturating_add(1);
        }
        if count == 0 {
            return (Vec::new(), 0);
        }
        let closed = self
            .line(start.saturating_add(count))
            .is_some_and(|line| line.kind == LineKind::Separator);
        let starts_with_slide_key = self.line(start).is_some_and(|line| match line.kind {
            LineKind::KeyValue(found) => Key::from_name(
                line.text
                    .get(..found.key_end)
                    .unwrap_or_default()
                    .as_bytes(),
            )
            .is_some_and(|key| key.position() == Position::Slide),
            LineKind::Blank | LineKind::Separator | LineKind::CodeFence(_) | LineKind::Content => {
                false
            }
        });
        if !closed && !starts_with_slide_key {
            return (Vec::new(), 0);
        }
        let mut entries = Vec::new();
        for index in start..start.saturating_add(count) {
            if let Some(line) = self.line(index)
                && let LineKind::KeyValue(found) = line.kind
            {
                entries.push(entry(line, found));
            }
        }
        let masked = if closed {
            count.saturating_add(1)
        } else {
            if let Some(first) = self.line(start) {
                self.scan
                    .diagnostics
                    .push(unclosed(first, "this slide's front matter"));
            }
            count
        };
        self.at = start.saturating_add(masked);
        (entries, masked)
    }

    /// Reads one slide up to the next separator outside a code fence, or to the end of the deck.
    fn slide(&mut self) -> bool {
        let Some(first) = self.line(self.at) else {
            return false;
        };
        let first_line = first.number;
        let start = self.at;
        let (front, masked) = self.front_matter();
        let mut text = String::new();
        for index in start..start.saturating_add(masked) {
            if let Some(line) = self.line(index) {
                text.push_str(&" ".repeat(line.text.len()));
                text.push('\n');
            }
        }
        let mut last = first;
        let mut fence: Option<CodeFence> = None;
        let mut previous: Option<LineKind> = None;
        while let Some(line) = self.line(self.at) {
            if let Some(open) = fence {
                if deck_rules::closes(open, line.text.as_bytes()) {
                    fence = None;
                    previous = Some(LineKind::CodeFence(open));
                } else {
                    previous = Some(LineKind::Content);
                }
            } else {
                match line.kind {
                    LineKind::Separator => {
                        if deck_rules::separation(previous) == Separation::UnderlinesContent {
                            self.scan.diagnostics.push(underlines_content(line));
                        }
                        self.at = self.at.saturating_add(1);
                        break;
                    }
                    LineKind::CodeFence(open) => {
                        fence = Some(open);
                        previous = Some(line.kind);
                    }
                    LineKind::Blank | LineKind::KeyValue(_) | LineKind::Content => {
                        previous = Some(line.kind);
                    }
                }
            }
            text.push_str(line.text);
            text.push('\n');
            last = line;
            self.at = self.at.saturating_add(1);
        }
        self.scan.slides.push(RawSlide {
            first_line,
            text,
            front,
            span: Span::new(
                LineColumn::new(first_line, 1),
                Span::line(last.number, last.text.len()).end,
            ),
        });
        true
    }
}

/// The scan of `text`: the head entries, every slide with its front matter masked, and the diagnostics of the deck's shape.
#[must_use]
pub fn parse(text: &str) -> Scan {
    let lines = lines(text);
    let mut scanner = Scanner::new(&lines);
    scanner.head_matter();
    while scanner.slide() {}
    scanner.scan
}

#[cfg(test)]
mod tests {
    use super::{Scan, parse};
    use sivi_core::diagnostic::Code;
    use sivi_core::span::{LineColumn, Span};

    fn codes(scan: &Scan) -> Vec<Code> {
        scan.diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_deck_splits_into_slides_at_separators_outside_code_fences() {
        let deck = "---\ntheme: plain\n---\nmaster: title\n---\n# One\n\n---\n# Two\n\n```\n---\n```\n\n---\n# Three\n";
        let scan = parse(deck);
        assert_eq!(codes(&scan), Vec::<Code>::new());
        assert_eq!(scan.head.len(), 1);
        assert_eq!(scan.head[0].key, "theme");
        assert_eq!(scan.head[0].value, "plain");
        assert_eq!(
            scan.head[0].value_span,
            Span::new(LineColumn::new(2, 8), LineColumn::new(2, 12))
        );
        assert_eq!(scan.slides.len(), 3);
        let first = &scan.slides[0];
        assert_eq!(first.first_line, 4);
        assert_eq!(first.front.len(), 1);
        assert_eq!(first.front[0].value, "title");
        assert_eq!(first.text, "             \n   \n# One\n\n");
        assert_eq!(
            first.span,
            Span::new(LineColumn::new(4, 1), LineColumn::new(7, 1))
        );
        let second = &scan.slides[1];
        assert_eq!(second.first_line, 9);
        assert_eq!(second.text, "# Two\n\n```\n---\n```\n\n");
        let third = &scan.slides[2];
        assert_eq!(third.first_line, 16);
        assert_eq!(third.text, "# Three\n");
        assert_eq!(third.span.end, LineColumn::new(16, 7));
    }

    #[test]
    fn line_ends_tabs_and_multibyte_text_keep_their_widths() {
        let scan = parse("---\r\ntitle: 発表\r\n---\r\nmaster: x\r\n---\r\n# 見出し\r\n\ttab\r\n");
        assert_eq!(codes(&scan), Vec::<Code>::new());
        assert_eq!(scan.head[0].value, "発表");
        assert_eq!(scan.head[0].value_span.end.column, 13);
        assert_eq!(scan.slides[0].text, "         \n   \n# 見出し\n\ttab\n");
        assert_eq!(scan.slides[0].first_line, 4);
    }

    #[test]
    fn a_slide_front_matter_is_read_only_when_it_closes_or_starts_with_a_slide_key() {
        let plain = parse("note: this is text\n\nmore\n");
        assert_eq!(codes(&plain), Vec::<Code>::new());
        assert_eq!(plain.slides[0].front.len(), 0);
        assert_eq!(plain.slides[0].text, "note: this is text\n\nmore\n");
        let unclosed = parse("id: cover\n# Title\n");
        assert_eq!(codes(&unclosed), vec![Code::UnclosedFrontMatter]);
        assert_eq!(unclosed.slides[0].front.len(), 1);
        assert_eq!(unclosed.slides[0].text, "         \n# Title\n");
        let later = parse("# One\n\n---\nmaster: two\nid: b\n---\n# Two\n");
        assert_eq!(codes(&later), Vec::<Code>::new());
        assert_eq!(later.slides[1].front.len(), 2);
        assert_eq!(later.slides[1].first_line, 4);
    }

    #[test]
    fn each_breach_of_the_deck_shape_is_named() {
        assert_eq!(
            codes(&parse("---\ntheme: plain\n")),
            vec![Code::UnclosedFrontMatter]
        );
        assert_eq!(
            codes(&parse("---\ntheme: plain\nbad line\n\n---\n# A\n")),
            vec![Code::MalformedKeyValue, Code::MalformedKeyValue]
        );
        let underlined = parse("# A\n\nText\n---\n# B\n");
        assert_eq!(codes(&underlined), vec![Code::SetextHeading]);
        assert_eq!(underlined.slides.len(), 2);
        assert_eq!(
            underlined.diagnostics[0]
                .primary
                .as_ref()
                .map(|label| label.span.start.line),
            Some(4)
        );
        let after_fence = parse("```\ncode\n```\n---\n# B\n");
        assert_eq!(codes(&after_fence), Vec::<Code>::new());
        assert_eq!(after_fence.slides.len(), 2);
        assert_eq!(parse("").slides.len(), 0);
        assert_eq!(parse("# A\n---\n").slides.len(), 1);
        assert_eq!(parse("# A\n\n---\n\n").slides.len(), 2);
    }
}
