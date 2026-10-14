//! Every showcase fixture rendered to PNG pages under `target/gallery/`, for a look at each theme; a tool for a person.

use sivi::build::{Request, Sink, judge};
use sivi::publish::{Disk, publish};
use sivi_core::diagnostic::has_error;
use sivi_typst::render::{GALLERY_PIXELS_PER_POINT, pngs};
use std::path::{Path, PathBuf};

/// Where the fixture decks live and where the pages go, relative to the crate.
const DECKS: &str = "../../fixtures/decks";
const GALLERY: &str = "../../target/gallery";

/// The fixtures that use every master of a carried theme once.
const SHOWCASES: [&str; 4] = [
    "pass-all-masters",
    "pass-showcase-blank",
    "pass-showcase-editorial",
    "pass-showcase-studio",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DECKS)
        .join(name)
        .join("deck.md")
}

#[test]
#[ignore = "a tool for a person; run with `just gallery`"]
fn render_every_showcase_to_png_pages() {
    let gallery = Path::new(env!("CARGO_MANIFEST_DIR")).join(GALLERY);
    sivi::io::create_dir_all(&gallery).unwrap();
    for name in SHOWCASES {
        let deck = fixture(name);
        let (outcome, judged) = judge(&Request {
            deck: &deck,
            theme: None,
            sink: Sink::Nothing,
            timestamp: None,
            fonts_dir: None,
        });
        assert!(
            !has_error(&outcome.diagnostics),
            "{name}: {:#?}",
            outcome.diagnostics
        );
        let judged = judged.unwrap_or_else(|| panic!("{name}: no pages"));
        let pages = pngs(&judged.compiled, GALLERY_PIXELS_PER_POINT).unwrap();
        for (index, png) in pages.iter().enumerate() {
            let path = gallery.join(format!("{name}-{:02}.png", index.saturating_add(1)));
            publish(&mut Disk::default(), &path, png).unwrap();
        }
    }
}
