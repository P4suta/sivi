//! Diagnostics for a person, in the style of rustc, and the one JSON document for a program.

use crate::build::Product;
use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use serde_json::{Value, json};
use sivi_core::diagnostic::{Code, Diagnostic, Severity};
use sivi_core::report::DIAGNOSTICS_DOC;
use sivi_core::span::{LineColumn, Span};
use std::ops::Range;

/// The format name of the JSON document.
pub const JSON_FORMAT: &str = "sivi.diagnostics";
/// The version of the JSON document.
pub const JSON_VERSION: u32 = 1;

/// The texts diagnostics quote and the names they carry.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sources<'a> {
    pub deck_path: &'a str,
    pub deck_text: &'a str,
    pub theme_label: &'a str,
    pub theme_text: &'a str,
}

/// Whether a code's spans point into the theme file rather than the deck.
#[must_use]
pub fn in_theme(code: Code) -> bool {
    let name = code.as_str();
    name.starts_with("E05")
        || name.starts_with("W05")
        || matches!(name, "E0601" | "E0602" | "E0603" | "E0608")
}

/// The byte offset of a one-based line and byte column in `text`.
fn offset(text: &str, position: LineColumn) -> usize {
    let mut passed: usize = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if index.saturating_add(1) == position.line {
            return passed
                .saturating_add(position.column.saturating_sub(1))
                .min(text.len());
        }
        passed = passed.saturating_add(line.len());
    }
    text.len()
}

/// The byte range of a span in `text`, at least one byte wide where the text allows.
#[must_use]
pub fn range(text: &str, span: Span) -> Range<usize> {
    let start = offset(text, span.start);
    let end = offset(text, span.end)
        .saturating_add(1)
        .min(text.len())
        .max(start);
    start..end
}

const fn level(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Error => Level::ERROR,
        Severity::Warning => Level::WARNING,
    }
}

fn slide_note(diagnostic: &Diagnostic) -> Option<String> {
    let slide = diagnostic.slide.as_ref()?;
    let id = slide
        .id
        .as_ref()
        .map_or(String::new(), |id| format!(" (`{id}`)"));
    let master = slide
        .master
        .as_ref()
        .map_or(String::new(), |master| format!(", master `{master}`"));
    Some(format!("slide {}{id}{master}", slide.number))
}

fn group<'a>(diagnostic: &'a Diagnostic, sources: &Sources<'a>) -> Group<'a> {
    let (path, text) = if in_theme(diagnostic.code) {
        (sources.theme_label, sources.theme_text)
    } else {
        (sources.deck_path, sources.deck_text)
    };
    let mut group = Group::with_title(
        level(diagnostic.severity())
            .primary_title(diagnostic.message.as_str())
            .id(diagnostic.code.as_str()),
    );
    if let Some(primary) = &diagnostic.primary {
        let mut snippet = Snippet::source(text)
            .path(path)
            .line_start(1)
            .fold(true)
            .annotation(
                AnnotationKind::Primary
                    .span(range(text, primary.span))
                    .label(primary.text.as_str()),
            );
        for secondary in &diagnostic.secondary {
            snippet = snippet.annotation(
                AnnotationKind::Context
                    .span(range(text, secondary.span))
                    .label(secondary.text.as_str()),
            );
        }
        group = group.element(snippet);
    }
    if let Some(note) = slide_note(diagnostic) {
        group = group.element(Level::NOTE.message(note));
    }
    for note in &diagnostic.notes {
        group = group.element(Level::NOTE.message(note.as_str()));
    }
    for help in &diagnostic.helps {
        group = group.element(Level::HELP.message(help.as_str()));
    }
    group
}

/// How many errors and warnings a list holds.
#[must_use]
pub fn summary(diagnostics: &[Diagnostic]) -> String {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity() == Severity::Error)
        .count();
    let warnings = diagnostics.len().saturating_sub(errors);
    match (errors, warnings) {
        (0, 0) => String::new(),
        (0, warnings) => format!("warning: {warnings} warnings emitted\n"),
        (errors, 0) => format!("error: aborting due to {errors} errors\n"),
        (errors, warnings) => {
            format!("error: aborting due to {errors} errors; {warnings} warnings emitted\n")
        }
    }
}

/// Every diagnostic rendered for a person, each with its source lines, then the summary.
#[must_use]
pub fn human(diagnostics: &[Diagnostic], sources: &Sources<'_>, styled: bool) -> String {
    let renderer = if styled {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    let mut text = String::new();
    for diagnostic in diagnostics {
        text.push_str(&renderer.render(&[group(diagnostic, sources)]));
        text.push_str("\n\n");
    }
    text.push_str(&summary(diagnostics));
    text
}

fn position(position: LineColumn) -> Value {
    json!({ "line": position.line, "column": position.column })
}

fn span_value(file: &str, span: Span, text: &str, primary: bool) -> Value {
    json!({
        "file": file,
        "start": position(span.start),
        "end": position(span.end),
        "text": text,
        "primary": primary,
    })
}

fn diagnostic_value(diagnostic: &Diagnostic, sources: &Sources<'_>) -> Value {
    let file = if in_theme(diagnostic.code) {
        sources.theme_label
    } else {
        sources.deck_path
    };
    let mut spans = Vec::new();
    if let Some(primary) = &diagnostic.primary {
        spans.push(span_value(file, primary.span, &primary.text, true));
    }
    for secondary in &diagnostic.secondary {
        spans.push(span_value(file, secondary.span, &secondary.text, false));
    }
    let severity = match diagnostic.severity() {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    json!({
        "code": diagnostic.code.as_str(),
        "severity": severity,
        "message": diagnostic.message,
        "slide": diagnostic.slide.as_ref().map(|slide| json!({
            "number": slide.number,
            "id": slide.id.as_ref().map(ToString::to_string),
            "master": slide.master.as_ref().map(ToString::to_string),
        })),
        "spans": spans,
        "notes": diagnostic.notes,
        "helps": diagnostic.helps,
        "doc": format!("{DIAGNOSTICS_DOC}#{}", diagnostic.code.as_str().to_lowercase()),
    })
}

/// The one JSON document of a run.
#[must_use]
pub fn json(
    diagnostics: &[Diagnostic],
    sources: &Sources<'_>,
    product: Option<&Product>,
) -> String {
    let output = product.map(|product| {
        json!({
            "path": product.path.as_ref().map(|path| path.display().to_string()),
            "pages": product.pages,
            "sha256": product.sha256,
        })
    });
    let document = json!({
        "format": JSON_FORMAT,
        "version": JSON_VERSION,
        "deck": sources.deck_path,
        "diagnostics": diagnostics.iter().map(|diagnostic| diagnostic_value(diagnostic, sources)).collect::<Vec<_>>(),
        "output": output,
    });
    let mut text = document.to_string();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::{Sources, human, in_theme, json, range, summary};
    use sivi_core::diagnostic::{Code, Diagnostic, SlideRef};
    use sivi_core::span::{LineColumn, Span};
    use sivi_core::text::Identifier;

    const DECK: &str = "# Title\n\n- one\n- two\n";

    fn overflow() -> Diagnostic {
        Diagnostic::new(
            Code::VerticalOverflow,
            String::from("content bound to slot `body` is 82.800pt taller than its region"),
        )
        .at(
            Span::new(LineColumn::new(3, 1), LineColumn::new(3, 5)),
            String::from("content starts here"),
        )
        .also_at(Span::line(4, 5), String::from("and ends here"))
        .note(String::from("content needs 406.800pt"))
        .help(String::from("remove 3 lines"))
        .on_slide(SlideRef {
            number: 1,
            id: Some(Identifier::new("intro").unwrap()),
            master: Some(Identifier::new("bullets").unwrap()),
        })
    }

    #[test]
    fn spans_become_byte_ranges_and_theme_codes_point_at_the_theme() {
        assert_eq!(range(DECK, Span::line(3, 5)), 9..14);
        assert_eq!(range(DECK, Span::line(1, 1)), 0..1);
        assert_eq!(range(DECK, Span::line(9, 1)), DECK.len()..DECK.len());
        assert!(in_theme(Code::RegionOverlap));
        assert!(in_theme(Code::FontDigest));
        assert!(in_theme(Code::UnusedFont));
        assert!(!in_theme(Code::MissingGlyph));
        assert!(!in_theme(Code::VerticalOverflow));
    }

    #[test]
    fn the_human_rendering_shows_the_code_the_lines_the_notes_and_the_summary() {
        let sources = Sources {
            deck_path: "deck.md",
            deck_text: DECK,
            theme_label: "plain",
            theme_text: "",
        };
        let text = human(&[overflow()], &sources, false);
        assert!(
            text.contains("error[E0401]: content bound to slot `body`"),
            "{text}"
        );
        assert!(text.contains("deck.md:3:1"), "{text}");
        assert!(text.contains("- one"), "{text}");
        assert!(text.contains("content starts here"), "{text}");
        assert!(text.contains("and ends here"), "{text}");
        assert!(
            text.contains("note: slide 1 (`intro`), master `bullets`"),
            "{text}"
        );
        assert!(text.contains("note: content needs 406.800pt"), "{text}");
        assert!(text.contains("help: remove 3 lines"), "{text}");
        assert!(
            text.ends_with("error: aborting due to 1 errors\n"),
            "{text}"
        );
        assert_eq!(summary(&[]), "");
    }

    #[test]
    fn the_json_document_carries_every_field_of_a_diagnostic() {
        let sources = Sources {
            deck_path: "deck.md",
            deck_text: DECK,
            theme_label: "plain",
            theme_text: "",
        };
        let text = json(&[overflow()], &sources, None);
        let document: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["format"], "sivi.diagnostics");
        assert_eq!(document["version"], 1);
        assert_eq!(document["output"], serde_json::Value::Null);
        let diagnostic = &document["diagnostics"][0];
        assert_eq!(diagnostic["code"], "E0401");
        assert_eq!(diagnostic["severity"], "error");
        assert_eq!(diagnostic["slide"]["id"], "intro");
        assert_eq!(diagnostic["spans"][0]["start"]["line"], 3);
        assert_eq!(diagnostic["spans"][1]["primary"], false);
        assert_eq!(diagnostic["doc"], "docs/diagnostics.md#e0401");
        assert_eq!(diagnostic["helps"][0], "remove 3 lines");
    }
}
