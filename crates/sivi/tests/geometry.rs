//! The geometry the frame walk reads back from the passing fixtures: a snapshot of every item, and the line-head prohibition of Japanese.

use anyhow::{Context, Result, anyhow};
use sivi_core::measure::{ItemKind, Leaf, Measurement};
use std::path::PathBuf;

/// The fixtures, relative to the crate, which is the working directory of a test.
const DECKS: &str = "../../fixtures/decks";

/// The characters JIS X 4051 keeps off the head of a line: closing marks, punctuation, the prolonged sound mark, iteration marks, and small kana.
const NEVER_AT_LINE_HEAD: &str = "、。，．・：；？！゛゜´｀¨＾￣＿ヽヾゝゞ〃仝々〆〇ー―‐／＼～∥｜…‥’”）〕］｝〉》」』】°′″℃￠％ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ";

/// The fewest lines the Japanese fixtures must wrap into for the check to mean anything.
const FEWEST_LINES: usize = 5;

fn deck(name: &str) -> PathBuf {
    PathBuf::from(DECKS).join(name).join("deck.md")
}

/// Compiles a fixture with the built-in theme and reads its layout back.
fn measure(name: &str) -> Result<Measurement> {
    let bytes = sivi::io::read(&deck(name))?;
    let parsed = sivi_markdown::parse(&bytes)
        .deck
        .context("the fixture parses")?;
    let (theme, _) =
        sivi::themes::load(&sivi::themes::Choice::Builtin(String::from("plain")), None)
            .map_err(|diagnostics| anyhow!("the theme loads: {diagnostics:?}"))?;
    let bound = sivi_core::bind::bind(&parsed, &theme.theme)
        .map_err(|diagnostics| anyhow!("the fixture binds: {diagnostics:?}"))?;
    let boxes = sivi::fonts::line_boxes(&theme.faces);
    let emitted = sivi_core::emit::typst_source(&theme.theme, &parsed, &bound, &boxes)
        .map_err(|diagnostics| anyhow!("the fixture emits: {diagnostics:?}"))?;
    let dirs = sivi::assets::Dirs {
        deck: &PathBuf::from(DECKS).join(name),
        theme: None,
    };
    let loaded = sivi::assets::load(&emitted.assets, &parsed, dirs);
    if let Some(error) = loaded
        .diagnostics
        .iter()
        .find(|found| found.severity() == sivi_core::diagnostic::Severity::Error)
    {
        return Err(anyhow!("an asset of the fixture: {}", error.message));
    }
    let world = sivi_typst::SiviWorld::new(emitted.source.as_str(), loaded.assets, Vec::new())?;
    let compiled = sivi_typst::compile(&world)?;
    Ok(sivi_typst::measure(&compiled.document, &emitted.expected)?)
}

/// One line per item: page, kind, and the box in millipoints.
fn dump(measurement: &Measurement) -> String {
    let lines: Vec<String> = measurement
        .leaves
        .iter()
        .map(|leaf| {
            let kind = match leaf.kind {
                ItemKind::Text => "text",
                ItemKind::Shape => "shape",
                ItemKind::Image => "image",
            };
            format!(
                "{} {kind} {} {} {} {}\n",
                leaf.page.0,
                leaf.bounds.origin.x.0,
                leaf.bounds.origin.y.0,
                leaf.bounds.size.width.0,
                leaf.bounds.size.height.0
            )
        })
        .collect();
    lines.concat()
}

#[test]
fn the_geometry_of_every_passing_fixture_is_the_snapshot() {
    let mut settings = insta::Settings::clone_current();
    settings.set_prepend_module_to_snapshot(false);
    settings.bind(|| {
        for name in [
            "pass-all-masters",
            "pass-basic",
            "pass-japanese",
            "pass-japanese-kinsoku",
        ] {
            let measurement = measure(name).unwrap();
            assert_eq!(measurement.missing_glyphs, Vec::new(), "{name}");
            insta::assert_snapshot!(format!("{name}-geometry"), dump(&measurement));
        }
    });
}

/// The leftmost text item of every line, by page and top edge.
fn line_heads(leaves: &[Leaf]) -> Vec<&Leaf> {
    let mut heads: Vec<&Leaf> = Vec::new();
    for leaf in leaves.iter().filter(|leaf| leaf.kind == ItemKind::Text) {
        let same_line = heads.iter().position(|head| {
            head.page == leaf.page && head.bounds.origin.y == leaf.bounds.origin.y
        });
        match same_line.and_then(|index| heads.get_mut(index)) {
            Some(head) => {
                if leaf.bounds.origin.x < head.bounds.origin.x {
                    *head = leaf;
                }
            }
            None => heads.push(leaf),
        }
    }
    heads
}

#[test]
fn no_line_of_the_japanese_fixtures_starts_with_a_prohibited_character() {
    for name in ["pass-japanese", "pass-japanese-kinsoku"] {
        let measurement = measure(name).unwrap();
        let heads = line_heads(&measurement.leaves);
        assert!(heads.len() >= FEWEST_LINES, "{name}: {} lines", heads.len());
        for head in heads {
            let text = head.text.as_deref().unwrap_or("");
            let first = text.chars().next().unwrap_or(' ');
            assert!(
                !NEVER_AT_LINE_HEAD.contains(first),
                "{name}: a line starts with `{first}` in `{text}`"
            );
        }
    }
}
