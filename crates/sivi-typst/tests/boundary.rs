//! The assumptions sivi makes about Typst, each checked against the real compiler on every run.
//!
//! `docs/verification/typst-boundary.md` names every test here beside the assumption it keeps.

use sivi_core::length::{Length, MILLIPOINTS_PER_POINT, Point, Rect, Size};
use sivi_core::measure::{Expected, ExpectedRegion, ItemKind, PageCount, PageNumber};
use sivi_core::protocol::{RegionIndex, SlideIndex};
use sivi_typst::{
    CompileError, Compiled, PdfSettings, ReadBackError, SiviWorld, UtcTimestamp, compile, export,
    measure,
};

/// A page without margins whose text boxes cover the whole ink of every glyph, as the emitter sets them.
const PAGE: &str = "#set page(width: 300pt, height: 200pt, margin: 0pt)\n#set text(font: \"BIZ UDPGothic\", size: 12pt, top-edge: \"bounds\", bottom-edge: \"bounds\")\n#show raw: set text(font: \"JetBrains Mono\")\n";

const fn pt(points: i64) -> Length {
    Length::from_millipoints(points.saturating_mul(MILLIPOINTS_PER_POINT))
}

/// A region on the slide, as `(x, y, width, height)` in points.
const fn region(
    slide: u32,
    index: u32,
    bounds: (i64, i64, i64, i64),
    split_points: u32,
) -> ExpectedRegion {
    let (x, y, width, height) = bounds;
    ExpectedRegion {
        slide: SlideIndex(slide),
        region: RegionIndex(index),
        bounds: Rect::new(
            Point { x: pt(x), y: pt(y) },
            Size {
                width: pt(width),
                height: pt(height),
            },
        ),
        split_points,
    }
}

/// A slide marker followed by one placed region at `(x, y, width)` whose parts are measured as the emitter will measure them.
fn scaffold(slide: u32, region: u32, place: (i64, i64, i64), parts: &[&str]) -> String {
    let (x, y, width) = place;
    let parts_source = parts
        .iter()
        .map(|part| format!("[{part}]"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "#metadata((slide: {slide})) <sivi-slide-{slide}>\n\
         #place(top + left, dx: {x}pt, dy: {y}pt, block(width: {width}pt, {{\n\
           let parts = ({parts_source},)\n\
           let body = parts.join()\n\
           context {{\n\
             let m = measure(width: {width}pt, body)\n\
             let prefixes = range(1, parts.len() + 1).map(k => measure(width: {width}pt, parts.slice(0, k).join()).height)\n\
             let widths = parts.map(p => 0pt)\n\
             [#metadata((required: m.height, prefixes: prefixes, widths: widths)) <sivi-m-{slide}-{region}>]\n\
           }}\n\
           body\n\
         }}))\n"
    )
}

fn build(source: &str) -> Result<Compiled, CompileError> {
    let world = SiviWorld::new(source, Vec::new(), Vec::new()).map_err(|error| {
        CompileError::Typst(vec![sivi_typst::Diagnostic {
            message: error.to_string(),
            range: None,
            hints: Vec::new(),
        }])
    })?;
    compile(&world)
}

#[test]
fn measure_at_the_region_width_reports_the_required_height_and_rising_prefixes() {
    let source = format!(
        "{PAGE}{}",
        scaffold(
            0,
            0,
            (10, 10, 200),
            &["#lorem(20)", "#lorem(20)", "#lorem(20)"]
        )
    );
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (10, 10, 200, 50), 3)],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    let [region] = measurement.regions.as_slice() else {
        panic!("one region")
    };
    assert!(region.required > pt(50), "{}", region.required);
    let prefixes = region.prefixes.as_slice();
    assert_eq!(prefixes.len(), 3);
    assert!(
        prefixes[0] < prefixes[1] && prefixes[1] < prefixes[2],
        "{prefixes:?}"
    );
    assert_eq!(prefixes[2], region.required);
    assert_eq!(region.natural_widths.len(), 0);
    assert_eq!(measurement.pages, PageCount(1));
    assert_eq!(measurement.slide_pages, [(SlideIndex(0), PageNumber(1))]);
    assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
}

#[test]
fn japanese_text_wraps_at_the_region_width_without_spaces() {
    let text = "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ。何でも薄暗いじめじめした所でニャーニャー泣いていた事だけは記憶している。";
    let source = format!(
        "{PAGE}#set text(lang: \"ja\")\n{}",
        scaffold(0, 0, (10, 10, 200), &[text])
    );
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (10, 10, 200, 100), 1)],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    let required = measurement.regions[0].required;
    let two_lines = pt(24);
    assert!(required > two_lines, "{required}");
    assert!(
        measurement.missing_glyphs.is_empty(),
        "{:?}",
        measurement.missing_glyphs
    );
}

#[test]
fn a_placed_region_that_overflows_the_page_never_adds_a_page() {
    let source = format!("{PAGE}{}", scaffold(0, 0, (10, 10, 200), &["#lorem(400)"]));
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (10, 10, 200, 100), 1)],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    assert_eq!(measurement.pages, PageCount(1));
    assert!(measurement.regions[0].required > pt(200));
    let lowest = measurement
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == ItemKind::Text)
        .map(|leaf| leaf.bounds.bottom().unwrap())
        .max()
        .unwrap();
    assert!(lowest > pt(200), "{lowest}");
}

#[test]
fn each_slide_marker_lands_on_its_own_page() {
    let source = format!(
        "{PAGE}{}#pagebreak()\n{}",
        scaffold(0, 0, (10, 10, 200), &["#lorem(5)"]),
        scaffold(1, 0, (10, 10, 200), &["#lorem(5)"])
    );
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 2,
        regions: vec![
            region(0, 0, (10, 10, 200, 100), 1),
            region(1, 0, (10, 10, 200, 100), 1),
        ],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    assert_eq!(measurement.pages, PageCount(2));
    assert_eq!(
        measurement.slide_pages,
        [
            (SlideIndex(0), PageNumber(1)),
            (SlideIndex(1), PageNumber(2))
        ]
    );
    assert!(
        measurement
            .leaves
            .iter()
            .any(|leaf| leaf.page == PageNumber(2))
    );
}

#[test]
fn text_leaves_land_inside_the_region_that_placed_them() {
    let source = format!("{PAGE}{}", scaffold(0, 0, (100, 50, 150), &["#lorem(3)"]));
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (100, 50, 150, 100), 1)],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    let texts: Vec<_> = measurement
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == ItemKind::Text)
        .collect();
    assert_ne!(texts.len(), 0);
    for leaf in texts {
        assert!(leaf.bounds.origin.x >= pt(100), "{:?}", leaf.bounds);
        assert!(leaf.bounds.origin.y >= pt(50), "{:?}", leaf.bounds);
        assert!(leaf.bounds.right().unwrap() <= pt(250), "{:?}", leaf.bounds);
        assert!(
            leaf.bounds.bottom().unwrap() <= pt(150),
            "{:?}",
            leaf.bounds
        );
    }
}

#[test]
fn an_unbreakable_code_line_leaves_the_region_to_the_right() {
    let source = format!(
        "{PAGE}{}",
        scaffold(0, 0, (10, 10, 100), &["#raw(\"x\" * 80, block: true)"])
    );
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (10, 10, 100, 100), 1)],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    let widest = measurement
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == ItemKind::Text)
        .map(|leaf| leaf.bounds.right().unwrap())
        .max()
        .unwrap();
    assert!(widest > pt(110), "{widest}");
}

#[test]
fn a_character_no_bundled_font_has_is_reported_as_a_missing_glyph() {
    let source = format!("{PAGE}{}", scaffold(0, 0, (10, 10, 200), &["crab 🦀 here"]));
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (10, 10, 200, 100), 1)],
    };
    let measurement = measure(&compiled.document, &expected).unwrap();
    assert!(
        measurement
            .missing_glyphs
            .iter()
            .any(|glyph| glyph.character == '🦀'),
        "{:?}",
        measurement.missing_glyphs
    );
}

#[test]
fn a_family_outside_the_bundle_is_a_warning_the_adapter_surfaces() {
    let source = format!(
        "{PAGE}#set text(font: \"Nonexistent Family\")\n{}",
        scaffold(0, 0, (10, 10, 200), &["x"])
    );
    let compiled = build(&source).unwrap();
    assert!(
        compiled
            .warnings
            .iter()
            .any(|warning| warning.message.contains("unknown font family")),
        "{:?}",
        compiled.warnings
    );
}

#[test]
fn a_missing_marker_is_an_error_and_never_a_verdict() {
    let source = format!("{PAGE}#metadata((slide: 0)) <sivi-slide-0>\n#lorem(3)\n");
    let compiled = build(&source).unwrap();
    let expected = Expected {
        slides: 1,
        regions: vec![region(0, 0, (10, 10, 200, 100), 1)],
    };
    assert_eq!(
        measure(&compiled.document, &expected),
        Err(ReadBackError::MissingMarker {
            label: "sivi-m-0-0".to_owned()
        })
    );
    let duplicated = format!(
        "{PAGE}#metadata((slide: 0)) <sivi-slide-0>\n#metadata((slide: 0)) <sivi-slide-0>\n"
    );
    let twice = build(&duplicated).unwrap();
    let only_slides = Expected {
        slides: 1,
        regions: Vec::new(),
    };
    assert_eq!(
        measure(&twice.document, &only_slides),
        Err(ReadBackError::DuplicateMarker {
            label: "sivi-slide-0".to_owned()
        })
    );
}

#[test]
fn the_pdf_bytes_depend_on_the_inputs_alone() {
    let source = format!("{PAGE}{}", scaffold(0, 0, (10, 10, 200), &["#lorem(10)"]));
    let settings = PdfSettings {
        ident: "deck-digest".to_owned(),
        timestamp: Some(UtcTimestamp {
            year: 2026,
            month: 10,
            day: 13,
            hour: 0,
            minute: 0,
            second: 0,
        }),
    };
    let first = export(&build(&source).unwrap().document, &settings).unwrap();
    let second = export(&build(&source).unwrap().document, &settings).unwrap();
    assert_eq!(first, second);
    assert!(first.starts_with(b"%PDF-"));
    let undated = export(
        &build(&source).unwrap().document,
        &PdfSettings {
            ident: "deck-digest".to_owned(),
            timestamp: None,
        },
    )
    .unwrap();
    assert_ne!(first, undated);
}
