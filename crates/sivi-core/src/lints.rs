//! The warnings a deck earns before any layout: a head without a title or a language, and East Asian text under another language.

use crate::diagnostic::{Code, Diagnostic, SlideRef};
use crate::ir::{self, Block, Deck, Slide};
use crate::softbreak_rules::is_east_asian;
use crate::span::Span;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The line the head of a deck starts on, where a diagnostic about the head points.
const HEAD_LINE: usize = 1;

fn reference(slide: &Slide) -> SlideRef {
    SlideRef {
        number: slide.number,
        id: slide.id.clone(),
        master: slide.master.clone(),
    }
}

/// The first block whose text holds an East Asian character, with its slide.
fn first_east_asian(deck: &Deck) -> Option<(&Slide, &Block)> {
    deck.slides.iter().find_map(|slide| {
        slide
            .blocks
            .iter()
            .find(|block| ir::block_text(block).as_str().chars().any(is_east_asian))
            .map(|block| (slide, block))
    })
}

/// The warnings the head and the script of a deck earn.
#[must_use]
pub fn deck_warnings(deck: &Deck) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    let head = Span::line(HEAD_LINE, 1);
    if deck.head.title.is_none() {
        found.push(
            Diagnostic::new(Code::NoTitle, String::from("the deck has no `title`"))
                .at(head, String::from("the head starts here"))
                .help(String::from(
                    "write `title: ...` in the head; the PDF and the title decor of a master show it",
                )),
        );
    }
    match &deck.head.lang {
        None => found.push(
            Diagnostic::new(Code::NoLang, String::from("the deck has no `lang`"))
                .at(head, String::from("the head starts here"))
                .help(String::from(
                    "write `lang: en`, `lang: ja`, or the language of the deck in the head; line breaking follows it",
                )),
        ),
        Some(lang) if !lang.is_east_asian() => {
            if let Some((slide, block)) = first_east_asian(deck) {
                found.push(
                    Diagnostic::new(
                        Code::CjkWithoutLang,
                        format!("East Asian text in a deck whose `lang` is `{lang}`"),
                    )
                    .at(block.span, String::from("this block holds East Asian text"))
                    .help(String::from(
                        "set `lang: ja`, `lang: zh`, or `lang: ko` in the head so line breaking follows the script",
                    ))
                    .on_slide(reference(slide)),
                );
            }
        }
        Some(_) => {}
    }
    found
}

#[cfg(test)]
mod tests {
    use super::deck_warnings;
    use crate::diagnostic::Code;
    use crate::ir::{Block, BlockBody, Deck, Head, Inline, Paragraph, Slide};
    use crate::span::Span;
    use crate::text::{Lang, ShortText};
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    fn deck(head: Head, text: &str) -> Deck {
        Deck {
            head,
            slides: vec![Slide {
                number: 1,
                span: Span::line(3, 1),
                master: None,
                id: None,
                blocks: vec![Block {
                    body: BlockBody::Paragraph(Paragraph {
                        inlines: vec![Inline::Text(String::from(text))],
                    }),
                    span: Span::line(4, 1),
                }],
                notes: None,
            }],
        }
    }

    fn codes(deck: &Deck) -> Vec<Code> {
        deck_warnings(deck).iter().map(|found| found.code).collect()
    }

    #[test]
    fn a_bare_head_earns_the_title_and_language_warnings() {
        assert_eq!(
            codes(&deck(Head::default(), "plain")),
            vec![Code::NoTitle, Code::NoLang]
        );
        let full = Head {
            title: Some(ShortText::new("t").unwrap()),
            lang: Some(Lang::new("en").unwrap()),
            ..Head::default()
        };
        assert_eq!(codes(&deck(full, "plain")), Vec::new());
    }

    #[test]
    fn east_asian_text_under_another_language_is_warned_at_its_block() {
        let english = Head {
            title: Some(ShortText::new("t").unwrap()),
            lang: Some(Lang::new("en").unwrap()),
            ..Head::default()
        };
        let found = deck_warnings(&deck(english, "猫"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, Code::CjkWithoutLang);
        assert_eq!(found[0].primary.as_ref().unwrap().span, Span::line(4, 1));
        assert_eq!(found[0].slide.as_ref().map(|slide| slide.number), Some(1));
        let japanese = Head {
            title: Some(ShortText::new("t").unwrap()),
            lang: Some(Lang::new("ja").unwrap()),
            ..Head::default()
        };
        assert_eq!(codes(&deck(japanese, "猫")), Vec::new());
    }
}
