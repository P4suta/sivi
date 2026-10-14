//! The fixture decks through the whole pipeline with the real Typst: what passes builds the same bytes twice, and what fails names its numbers.

use sivi::build::{Request, Sink, run};
use sivi_core::diagnostic::{Code, has_error};
use std::path::{Path, PathBuf};

/// Where the fixture decks live, relative to the crate.
const DECKS: &str = "../../fixtures/decks";
/// How many masters the plain theme has, each used once by the passing fixture.
const MASTERS: u32 = 8;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DECKS)
        .join(name)
        .join("deck.md")
}

fn check(name: &str) -> sivi::build::Outcome {
    run(&Request {
        deck: &fixture(name),
        theme: None,
        sink: Sink::Nothing,
        timestamp: None,
        fonts_dir: None,
    })
}

#[test]
fn every_master_builds_once_each_to_the_same_bytes_twice() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("deck.pdf");
    let request = Request {
        deck: &fixture("pass-all-masters"),
        theme: None,
        sink: Sink::File(output.clone()),
        timestamp: None,
        fonts_dir: None,
    };
    let first = run(&request);
    assert!(!has_error(&first.diagnostics), "{:#?}", first.diagnostics);
    assert_eq!(first.failure, None);
    let product = first.product.unwrap();
    assert_eq!(product.pages, MASTERS);
    let bytes = sivi::io::read(&output).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let second = run(&request);
    assert_eq!(second.product.unwrap().sha256, product.sha256);
    assert_eq!(sivi::io::read(&output).unwrap(), bytes);
}

#[test]
fn a_body_that_overflows_is_refused_with_the_measured_excess_and_a_suggestion() {
    let outcome = check("fail-overflow-body");
    let overflow = outcome
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Code::VerticalOverflow)
        .unwrap_or_else(|| panic!("{:#?}", outcome.diagnostics));
    assert!(
        overflow
            .message
            .starts_with("content bound to slot `body` is "),
        "{}",
        overflow.message
    );
    assert!(overflow.message.ends_with("pt taller than its region"));
    assert!(
        overflow.notes.iter().any(|note| note
            .starts_with("region 840.000pt × 324.000pt at (60.000pt, 156.000pt); content needs ")),
        "{:#?}",
        overflow.notes
    );
    assert!(
        overflow
            .notes
            .iter()
            .any(|note| note.contains("rendered lines at 24.000pt × 1.450 = 34.800pt pitch;")),
        "{:#?}",
        overflow.notes
    );
    assert!(
        overflow
            .helps
            .iter()
            .any(|help| help.starts_with("remove ")),
        "{:#?}",
        overflow.helps
    );
    assert!(
        overflow
            .helps
            .iter()
            .any(|help| help.contains("no master in theme `plain` has a slot of")),
        "{:#?}",
        overflow.helps
    );
    assert_eq!(outcome.product, None);
}

#[test]
fn a_code_line_too_wide_is_refused_with_the_line_and_the_columns_that_fit() {
    let outcome = check("fail-code-width");
    let wide = outcome
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Code::HorizontalOverflow)
        .unwrap_or_else(|| panic!("{:#?}", outcome.diagnostics));
    assert!(
        wide.message.starts_with("a code line in slot `code` is "),
        "{}",
        wide.message
    );
    assert!(
        wide.notes
            .iter()
            .any(|note| note.starts_with("the longest line is line 2 of the block, ")),
        "{:#?}",
        wide.notes
    );
}

#[test]
fn the_earlier_fixtures_still_pass_or_fail_as_their_names_say() {
    for name in [
        "pass-basic",
        "pass-japanese",
        "pass-marks",
        "pass-code-theme",
        "pass-palette",
        "pass-images",
    ] {
        let outcome = check(name);
        assert!(
            !has_error(&outcome.diagnostics),
            "{name}: {:#?}",
            outcome.diagnostics
        );
    }
    for name in ["fail-front-matter", "fail-html"] {
        let outcome = check(name);
        assert!(has_error(&outcome.diagnostics), "{name}");
    }
}

/// Prints the generated Typst source and the measurement of a fixture, for a look at what the emitter wrote and the adapter read.
#[test]
#[ignore = "a tool for a person; run with --run-ignored only --no-capture"]
fn dump_the_source_and_the_measurement_of_a_fixture() {
    let bytes = sivi::io::read(&fixture("fail-code-width")).unwrap();
    let deck = sivi_markdown::parse(&bytes).deck.unwrap();
    let (theme, _) =
        sivi::themes::load(&sivi::themes::Choice::Builtin(String::from("plain")), None).unwrap();
    let bound = sivi_core::bind::bind(&deck, &theme.theme).unwrap();
    let boxes = sivi::fonts::line_boxes(&theme.faces);
    let emitted = sivi_core::emit::typst_source(&theme.theme, &deck, &bound, &boxes).unwrap();
    eprintln!("{}", emitted.source);
    let world =
        sivi_typst::SiviWorld::new(emitted.source.as_str(), Vec::new(), Vec::new()).unwrap();
    let compiled = sivi_typst::compile(&world).unwrap();
    let measurement = sivi_typst::measure(&compiled.document, &emitted.expected).unwrap();
    eprintln!("{:#?}", emitted.expected);
    eprintln!("{:?}", measurement.regions);
    for leaf in &measurement.leaves {
        eprintln!("{leaf:?}");
    }
}
