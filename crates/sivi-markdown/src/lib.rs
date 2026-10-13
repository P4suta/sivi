//! The Markdown front end of sivi.
//!
//! It scans a deck into slides, parses each slide as CommonMark, and hands the core a slide model whose every block carries its source span.

pub mod convert;
pub mod scanner;

use sivi_core::diagnostic::{Code, Diagnostic, SlideRef, has_error};
use sivi_core::front_matter::{self, FrontMatter, SlideMeta};
use sivi_core::front_matter_rules::Position;
use sivi_core::ir::{Deck, Head, Slide};
use sivi_core::span::Span;
use sivi_core::text::Identifier;

/// The most bytes a deck may hold.
pub const DECK_LIMIT: usize = 4 * 1024 * 1024;

/// The most slides a deck may hold.
pub const SLIDE_LIMIT: usize = 1000;

/// What the front end made of a deck: the model when nothing is wrong, and every diagnostic either way.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Parsed {
    /// The deck, present only when no diagnostic is an error.
    pub deck: Option<Deck>,
    pub diagnostics: Vec<Diagnostic>,
}

fn limit_exceeded(what: &str, size: usize, limit: usize) -> Diagnostic {
    Diagnostic::new(
        Code::LimitExceeded,
        format!("{what} holds {size}, beyond the limit of {limit}"),
    )
    .help(String::from("split the deck, or move content out of it"))
}

fn not_utf8(text: &[u8], valid_up_to: usize) -> Diagnostic {
    let line = text
        .get(..valid_up_to)
        .map_or(1, |prefix| prefix.split(|byte| *byte == b'\n').count());
    Diagnostic::new(Code::NotUtf8, String::from("the deck is not UTF-8"))
        .at(
            Span::line(line, 1),
            format!("the first byte that is not UTF-8 is at offset {valid_up_to}"),
        )
        .help(String::from(
            "save the deck as UTF-8 without a byte order mark",
        ))
}

fn slide_ref(slide: &Slide) -> SlideRef {
    SlideRef {
        number: slide.number,
        id: slide.id.clone(),
        master: slide.master.clone(),
    }
}

fn head_of(entries: &[front_matter::Entry], diagnostics: &mut Vec<Diagnostic>) -> Head {
    match front_matter::parse(Position::Head, entries) {
        Ok(FrontMatter::Head(head)) => head,
        Ok(FrontMatter::Slide(_)) => Head::default(),
        Err(found) => {
            diagnostics.extend(found);
            Head::default()
        }
    }
}

fn meta_of(entries: &[front_matter::Entry], diagnostics: &mut Vec<Diagnostic>) -> SlideMeta {
    match front_matter::parse(Position::Slide, entries) {
        Ok(FrontMatter::Slide(meta)) => meta,
        Ok(FrontMatter::Head(_)) => SlideMeta::default(),
        Err(found) => {
            diagnostics.extend(found);
            SlideMeta::default()
        }
    }
}

fn empty_slide(slide: &Slide) -> Diagnostic {
    Diagnostic::new(Code::EmptySlide, String::from("a slide with no content"))
        .at(slide.span, String::from("this slide is empty"))
        .help(String::from(
            "write the slide's content, or remove the separator that opens it",
        ))
        .on_slide(slide_ref(slide))
}

fn duplicate_id(slide: &Slide, id: &Identifier, first: &Slide) -> Diagnostic {
    Diagnostic::new(
        Code::DuplicateSlideId,
        format!("two slides share the id `{id}`"),
    )
    .at(slide.span, String::from("this slide carries the id again"))
    .also_at(first.span, String::from("this slide carries it first"))
    .help(String::from("give each slide its own id"))
    .on_slide(slide_ref(slide))
}

fn unknown_slide_id(slide: &Slide, id: &Identifier, span: Span) -> Diagnostic {
    Diagnostic::new(
        Code::UnknownSlideId,
        format!("a link to `#{id}`, which no slide carries as its id"),
    )
    .at(span, String::from("this link names the slide"))
    .help(format!(
        "set `id: {id}` in the front matter of the slide the link should reach"
    ))
    .on_slide(slide_ref(slide))
}

/// The slide model of `input`, or every diagnostic that keeps it from being one.
#[must_use]
pub fn parse(input: &[u8]) -> Parsed {
    let mut diagnostics = Vec::new();
    if input.len() > DECK_LIMIT {
        diagnostics.push(limit_exceeded("the deck", input.len(), DECK_LIMIT));
        return Parsed {
            deck: None,
            diagnostics,
        };
    }
    let text = match std::str::from_utf8(input) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(not_utf8(input, error.valid_up_to()));
            return Parsed {
                deck: None,
                diagnostics,
            };
        }
    };
    let scan = scanner::parse(text);
    diagnostics.extend(scan.diagnostics);
    let head = head_of(&scan.head, &mut diagnostics);
    if scan.slides.len() > SLIDE_LIMIT {
        diagnostics.push(
            limit_exceeded("the deck", scan.slides.len(), SLIDE_LIMIT)
                .note(String::from("the limit counts slides")),
        );
    }
    if scan.slides.is_empty() {
        diagnostics.push(
            Diagnostic::new(Code::NoSlides, String::from("the deck holds no slide")).help(
                String::from("write the first slide after the head of the deck"),
            ),
        );
    }
    let mut slides = Vec::new();
    let mut links = Vec::new();
    for (index, raw) in scan.slides.iter().enumerate() {
        let meta = meta_of(&raw.front, &mut diagnostics);
        let converted = convert::convert(raw, index.saturating_add(1), meta);
        let reference = slide_ref(&converted.slide);
        diagnostics.extend(
            converted
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.on_slide(reference.clone())),
        );
        if converted.slide.blocks.is_empty() && converted.slide.notes.is_none() {
            diagnostics.push(empty_slide(&converted.slide));
        }
        links.push(converted.links);
        slides.push(converted.slide);
    }
    for (index, slide) in slides.iter().enumerate() {
        if let Some(id) = &slide.id
            && let Some(first) = slides
                .iter()
                .take(index)
                .find(|earlier| earlier.id.as_ref() == Some(id))
        {
            diagnostics.push(duplicate_id(slide, id, first));
        }
    }
    for (slide, slide_links) in slides.iter().zip(&links) {
        for (id, span) in slide_links {
            if !slides.iter().any(|target| target.id.as_ref() == Some(id)) {
                diagnostics.push(unknown_slide_id(slide, id, *span));
            }
        }
    }
    let deck = if has_error(&diagnostics) {
        None
    } else {
        Some(Deck { head, slides })
    };
    Parsed { deck, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::{DECK_LIMIT, Parsed, parse};
    use sivi_core::diagnostic::Code;
    use sivi_core::text::Identifier;

    fn codes(parsed: &Parsed) -> Vec<Code> {
        parsed.diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_whole_deck_reads_into_its_head_and_slides() {
        let parsed = parse(b"---\ntitle: Deck\ndefault_master: bullets\n---\nid: one\n---\n# One\n\nSee [two](#two).\n\n---\nid: two\n---\n# Two\n");
        assert_eq!(codes(&parsed), Vec::<Code>::new());
        let deck = parsed.deck.unwrap();
        assert_eq!(
            deck.head.title.map(|title| String::from(title.as_str())),
            Some(String::from("Deck"))
        );
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[1].number, 2);
        assert_eq!(
            deck.slides[1].id.as_ref().map(Identifier::as_str),
            Some("two")
        );
    }

    #[test]
    fn deck_level_breaches_are_named_and_keep_the_model_back() {
        assert_eq!(codes(&parse(b"")), vec![Code::NoSlides]);
        assert_eq!(codes(&parse(b"\xff\xfe")), vec![Code::NotUtf8]);
        let utf8 = parse(b"# A\n\xff");
        assert_eq!(
            utf8.diagnostics[0]
                .primary
                .as_ref()
                .map(|label| label.span.start.line),
            Some(2)
        );
        let big = vec![b'a'; DECK_LIMIT.saturating_add(1)];
        assert_eq!(codes(&parse(&big)), vec![Code::LimitExceeded]);
        let empty = parse(b"# A\n\n---\n\n---\n# B\n");
        assert_eq!(codes(&empty), vec![Code::EmptySlide]);
        assert!(empty.deck.is_some());
        assert_eq!(
            empty.diagnostics[0]
                .slide
                .as_ref()
                .map(|slide| slide.number),
            Some(2)
        );
        let ids = parse(b"id: a\n---\n# One\n\n---\nid: a\n---\n# Two\n\n[x](#b)\n");
        assert_eq!(
            codes(&ids),
            vec![Code::DuplicateSlideId, Code::UnknownSlideId]
        );
        assert!(ids.deck.is_none());
        let head = parse(b"---\nmaster: x\n---\n# A\n");
        assert_eq!(codes(&head), vec![Code::UnknownKey]);
        assert!(head.deck.is_none());
    }
}
