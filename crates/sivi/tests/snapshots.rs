//! Every fixture's diagnostics, rendered for a person and as the JSON document, held to their snapshots.

use sivi::build::{Outcome, Request, Sink, run};
use sivi::render::{self, Sources};
use sivi::themes::{self, Choice};
use std::path::{Path, PathBuf};

/// The fixtures, relative to the crate, which is the working directory of a test, so the snapshots name no machine.
const DECKS: &str = "../../fixtures/decks";
const THEMES: &str = "../../fixtures/themes";

fn settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    settings.set_prepend_module_to_snapshot(false);
    settings
}

/// The fixture directories under `root`, by name and in order.
fn fixtures(root: &str) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        found.push((
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        ));
    }
    found.sort();
    Ok(found)
}

fn check(deck: &Path) -> Outcome {
    run(&Request {
        deck,
        theme: None,
        sink: Sink::Nothing,
        timestamp: None,
        fonts_dir: None,
    })
}

fn sources<'a>(deck: &'a str, outcome: &'a Outcome) -> Sources<'a> {
    Sources {
        deck_path: deck,
        deck_text: &outcome.deck_text,
        theme_label: &outcome.theme_label,
        theme_text: &outcome.theme_text,
    }
}

#[test]
fn every_deck_fixture_renders_as_its_snapshots_say() {
    settings().bind(|| {
        for (name, dir) in fixtures(DECKS).unwrap() {
            let path = dir.join("deck.md");
            let shown = format!("{DECKS}/{name}/deck.md");
            let outcome = check(&path);
            assert_eq!(outcome.failure, None, "{name}");
            let sources = sources(&shown, &outcome);
            insta::assert_snapshot!(
                format!("{name}-human"),
                render::human(&outcome.diagnostics, &sources, false)
            );
            insta::assert_snapshot!(
                format!("{name}-json"),
                render::json(&outcome.diagnostics, &sources, outcome.product.as_ref())
            );
        }
    });
}

#[test]
fn every_theme_fixture_renders_as_its_snapshot_says() {
    settings().bind(|| {
        for (name, dir) in fixtures(THEMES).unwrap() {
            let (label, text, diagnostics) = match themes::load(&Choice::Dir(dir.clone()), None) {
                Ok((loaded, warnings)) => {
                    (format!("{THEMES}/{name}/theme.toml"), loaded.text, warnings)
                }
                Err(diagnostics) => (format!("{THEMES}/{name}"), String::new(), diagnostics),
            };
            let sources = Sources {
                deck_path: "",
                deck_text: "",
                theme_label: &label,
                theme_text: &text,
            };
            insta::assert_snapshot!(
                format!("theme-{name}-human"),
                render::human(&diagnostics, &sources, false)
            );
        }
    });
}
