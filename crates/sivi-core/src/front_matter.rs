//! Typing of the key-value entries the scanner found in a front matter block.

use crate::diagnostic::{Code, Diagnostic};
use crate::front_matter_rules::{Key, Placement, Position, placement};
use crate::ir::Head;
use crate::span::Span;
use crate::text::{Date, Identifier, InvalidText, Lang, ShortText, ThemeRef};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// One `key: value` line as the scanner found it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    /// The span of the whole line.
    pub span: Span,
    /// The span of the value alone.
    pub value_span: Span,
}

/// What a slide's front matter declares.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SlideMeta {
    pub master: Option<Identifier>,
    pub id: Option<Identifier>,
}

/// A typed front matter block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrontMatter {
    Head(Head),
    Slide(SlideMeta),
}

const fn describe(position: Position) -> &'static str {
    match position {
        Position::Head => "the head of the deck",
        Position::Slide => "a slide's front matter",
    }
}

fn known_keys(position: Position) -> String {
    let names: Vec<&str> = Key::ALL
        .into_iter()
        .filter(|key| key.position() == position)
        .map(Key::name)
        .collect();
    names.join(", ")
}

fn unknown_key(entry: &Entry, position: Position) -> Diagnostic {
    Diagnostic::new(
        Code::UnknownKey,
        format!("unknown key `{}` in {}", entry.key, describe(position)),
    )
    .at(
        entry.span,
        String::from("this key is not one the deck reads"),
    )
    .help(format!(
        "the keys of {} are {}",
        describe(position),
        known_keys(position)
    ))
}

fn misplaced_key(entry: &Entry, key: Key, position: Position) -> Diagnostic {
    let diagnostic = Diagnostic::new(
        Code::UnknownKey,
        format!("`{}` does not belong in {}", key.name(), describe(position)),
    )
    .at(entry.span, format!("`{}` is set here", key.name()));
    match key {
        Key::Master => diagnostic.help(String::from(
            "set `default_master` in the head of the deck, or `master:` in each slide's front matter",
        )),
        Key::Id => diagnostic.help(String::from(
            "set `id:` in the front matter of the slide it names",
        )),
        Key::Theme | Key::DefaultMaster | Key::Title | Key::Author | Key::Date | Key::Lang => {
            diagnostic.help(format!(
                "move `{}` to the front matter at the head of the deck",
                key.name()
            ))
        }
    }
}

fn duplicate_key(entry: &Entry, key: Key, first: Span) -> Diagnostic {
    Diagnostic::new(Code::DuplicateKey, format!("`{}` is set twice", key.name()))
        .at(entry.span, String::from("set again here"))
        .also_at(first, String::from("first set here"))
}

const fn grammar(key: Key) -> &'static str {
    match key {
        Key::Theme => "`plain` or a relative path with a slash, such as `themes/corp`",
        Key::DefaultMaster | Key::Master | Key::Id => {
            "a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes"
        }
        Key::Title | Key::Author => "one to two hundred characters on one line",
        Key::Date => "a date written `YYYY-MM-DD`",
        Key::Lang => "a language tag such as `ja`, `en`, or `en-US`",
    }
}

fn invalid_value(entry: &Entry, key: Key, invalid: InvalidText) -> Diagnostic {
    let diagnostic = Diagnostic::new(
        Code::InvalidValue,
        format!("`{}` is not {}", key.name(), grammar(key)),
    )
    .at(entry.value_span, format!("`{}` is this value", key.name()));
    match invalid {
        InvalidText::AbsoluteTheme => diagnostic.note(String::from(
            "an absolute path would tie the deck to one machine; name the theme directory relative to the deck",
        )),
        InvalidText::Identifier
        | InvalidText::ShortText
        | InvalidText::Date
        | InvalidText::Lang
        | InvalidText::Theme => diagnostic,
    }
}

fn assign(
    key: Key,
    value: &str,
    head: &mut Head,
    slide: &mut SlideMeta,
) -> Result<(), InvalidText> {
    match key {
        Key::Theme => head.theme = Some(ThemeRef::new(value)?),
        Key::DefaultMaster => head.default_master = Some(Identifier::new(value)?),
        Key::Title => head.title = Some(ShortText::new(value)?),
        Key::Author => head.author = Some(ShortText::new(value)?),
        Key::Date => head.date = Some(Date::new(value)?),
        Key::Lang => head.lang = Some(Lang::new(value)?),
        Key::Master => slide.master = Some(Identifier::new(value)?),
        Key::Id => slide.id = Some(Identifier::new(value)?),
    }
    Ok(())
}

/// The typed block the entries of a front matter at `position` declare.
///
/// # Errors
///
/// Returns every diagnostic of the block, which the caller reports together.
pub fn parse(position: Position, entries: &[Entry]) -> Result<FrontMatter, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut seen: Vec<(Key, Span)> = Vec::new();
    let mut head = Head::default();
    let mut slide = SlideMeta::default();
    for entry in entries {
        let Some(key) = Key::from_name(entry.key.as_bytes()) else {
            diagnostics.push(unknown_key(entry, position));
            continue;
        };
        match placement(key, position) {
            Placement::Allowed => {}
            Placement::HeadOnly | Placement::SlideOnly => {
                diagnostics.push(misplaced_key(entry, key, position));
                continue;
            }
        }
        if let Some((_, first)) = seen.iter().find(|(earlier, _)| *earlier == key) {
            diagnostics.push(duplicate_key(entry, key, *first));
            continue;
        }
        seen.push((key, entry.span));
        if let Err(invalid) = assign(key, &entry.value, &mut head, &mut slide) {
            diagnostics.push(invalid_value(entry, key, invalid));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(match position {
        Position::Head => FrontMatter::Head(head),
        Position::Slide => FrontMatter::Slide(slide),
    })
}

#[cfg(test)]
mod tests {
    use super::{Entry, FrontMatter, parse};
    use crate::diagnostic::Code;
    use crate::front_matter_rules::Position;
    use crate::span::Span;
    use crate::text::Identifier;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    fn entry(line: usize, key: &str, value: &str) -> Entry {
        Entry {
            key: String::from(key),
            value: String::from(value),
            span: Span::line(line, 20),
            value_span: Span::line(line, 20),
        }
    }

    fn codes(entries: &[Entry], position: Position) -> Vec<Code> {
        match parse(position, entries) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics.iter().map(|d| d.code).collect(),
        }
    }

    #[test]
    fn a_head_types_every_key_it_knows() {
        let entries = [
            entry(2, "theme", "plain"),
            entry(3, "default_master", "bullets"),
            entry(4, "title", "発表"),
            entry(5, "author", "Yasunobu"),
            entry(6, "date", "2026-10-13"),
            entry(7, "lang", "ja"),
        ];
        let Ok(FrontMatter::Head(head)) = parse(Position::Head, &entries) else {
            panic!("a head");
        };
        assert_eq!(
            head.default_master,
            Some(Identifier::new("bullets").unwrap())
        );
        assert_eq!(
            head.title.map(|title| String::from(title.as_str())),
            Some(String::from("発表"))
        );
        assert!(head.lang.is_some_and(|lang| lang.is_east_asian()));
        assert_eq!(
            parse(Position::Head, &[]),
            Ok(FrontMatter::Head(crate::ir::Head::default()))
        );
    }

    #[test]
    fn a_slide_types_its_master_and_id_and_refuses_the_rest() {
        let entries = [entry(5, "master", "bullets"), entry(6, "id", "revenue")];
        let Ok(FrontMatter::Slide(meta)) = parse(Position::Slide, &entries) else {
            panic!("a slide");
        };
        assert_eq!(meta.master, Some(Identifier::new("bullets").unwrap()));
        assert_eq!(meta.id, Some(Identifier::new("revenue").unwrap()));
        assert_eq!(
            codes(&[entry(5, "title", "x")], Position::Slide),
            vec![Code::UnknownKey]
        );
        assert_eq!(
            codes(&[entry(2, "master", "x")], Position::Head),
            vec![Code::UnknownKey]
        );
        assert_eq!(
            codes(&[entry(2, "colour", "x")], Position::Head),
            vec![Code::UnknownKey]
        );
    }

    #[test]
    fn each_breach_names_its_code() {
        assert_eq!(
            codes(
                &[entry(2, "lang", "ja"), entry(3, "lang", "en")],
                Position::Head
            ),
            vec![Code::DuplicateKey]
        );
        assert_eq!(
            codes(&[entry(2, "date", "2026-13-01")], Position::Head),
            vec![Code::InvalidValue]
        );
        assert_eq!(
            codes(&[entry(2, "theme", "/abs")], Position::Head),
            vec![Code::InvalidValue]
        );
        assert_eq!(
            codes(&[entry(2, "master", "Bullets")], Position::Slide),
            vec![Code::InvalidValue]
        );
        let Err(diagnostics) = parse(Position::Head, &[entry(2, "master", "x")]) else {
            panic!("refused");
        };
        assert!(diagnostics[0].helps[0].contains("default_master"));
        let Err(unknown) = parse(Position::Head, &[entry(2, "colour", "x")]) else {
            panic!("refused");
        };
        assert!(unknown[0].helps[0].ends_with("theme, default_master, title, author, date, lang"));
        let Err(unknown_on_slide) = parse(Position::Slide, &[entry(2, "colour", "x")]) else {
            panic!("refused");
        };
        assert!(unknown_on_slide[0].helps[0].ends_with("master, id"));
        let Err(absolute) = parse(Position::Head, &[entry(2, "theme", "/abs")]) else {
            panic!("refused");
        };
        assert!(absolute[0].notes[0].contains("absolute"));
    }
}
