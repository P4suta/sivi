//! Reading the images a deck and its theme name, each checked against the kind its extension claims and the resolution it prints at.

use crate::io::{self, ReadErrorKind};
use sivi_core::diagnostic::{Code, Diagnostic, SlideRef, has_error};
use sivi_core::emit::{AssetOrigin, AssetUse};
use sivi_core::image::{self, MIN_PIXELS_PER_INCH};
use sivi_core::ir::{AssetRef, BlockBody, Deck};
use sivi_core::length::Length;
use sivi_core::span::Span;
use sivi_typst::Asset;
use std::path::Path;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
const JPEG_SIGNATURE: [u8; 3] = [0xff, 0xd8, 0xff];
const UTF8_BOM: [u8; 3] = [0xef, 0xbb, 0xbf];

/// Where the deck and the theme live, which their relative paths start from.
#[derive(Clone, Copy, Debug)]
pub struct Dirs<'a> {
    pub deck: &'a Path,
    pub theme: Option<&'a Path>,
}

/// The files the source names, with the diagnostics their reading earned; no file when any diagnostic is an error.
#[derive(Debug, Default)]
pub struct Loaded {
    pub assets: Vec<Asset>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Whether `bytes` start as a file of `extension` does.
#[must_use]
pub fn kind_matches(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "png" => bytes.starts_with(&PNG_SIGNATURE),
        "jpg" | "jpeg" => bytes.starts_with(&JPEG_SIGNATURE),
        "svg" => bytes
            .strip_prefix(&UTF8_BOM)
            .unwrap_or(bytes)
            .trim_ascii_start()
            .starts_with(b"<"),
        _ => false,
    }
}

/// The first image block that names `path`, for the span and slide of its diagnostics.
fn site(deck: &Deck, path: &AssetRef) -> Option<(Span, SlideRef)> {
    deck.slides.iter().find_map(|slide| {
        slide.blocks.iter().find_map(|block| match &block.body {
            BlockBody::Image(image) if image.path == *path => Some((
                block.span,
                SlideRef {
                    number: slide.number,
                    id: slide.id.clone(),
                    master: slide.master.clone(),
                },
            )),
            BlockBody::Image(_)
            | BlockBody::Heading(_)
            | BlockBody::Paragraph(_)
            | BlockBody::List(_)
            | BlockBody::Code(_)
            | BlockBody::Quote(_)
            | BlockBody::Table(_) => None,
        })
    })
}

fn located(diagnostic: Diagnostic, site: Option<(Span, SlideRef)>) -> Box<Diagnostic> {
    Box::new(match site {
        Some((span, slide)) => diagnostic
            .at(span, String::from("this image names the file"))
            .on_slide(slide),
        None => diagnostic,
    })
}

fn read_error(error: &io::ReadError, path: &Path, shown: &str) -> Diagnostic {
    match &error.kind {
        ReadErrorKind::Io(inner) if inner.kind() == std::io::ErrorKind::NotFound => {
            Diagnostic::new(
                Code::ImageMissing,
                format!("the image `{shown}` is missing at {}", io::path_label(path)),
            )
            .help(String::from(
                "check the path, which is relative to the deck",
            ))
        }
        ReadErrorKind::Io(inner) => Diagnostic::new(
            Code::ImageUnreadable,
            format!("the image `{shown}` cannot be read: {inner}"),
        ),
        ReadErrorKind::TooLarge | ReadErrorKind::NotUtf8 => Diagnostic::new(
            Code::LimitExceeded,
            format!("the image `{shown}` holds more than {} bytes", io::MAX_READ),
        )
        .help(String::from("shrink the image")),
    }
}

/// The warning an image too coarse for its printed width earns, when its header names its pixels.
fn coarse(shown: &str, bytes: &[u8], width: Length) -> Option<Diagnostic> {
    let pixels = image::dimensions(bytes)?;
    let per_inch = image::pixels_per_inch(pixels.width, width)?;
    if !image::is_coarse(per_inch) {
        return None;
    }
    Some(
        Diagnostic::new(
            Code::LowResolution,
            format!(
                "the image `{shown}` prints at {per_inch} pixels per inch, below {MIN_PIXELS_PER_INCH}"
            ),
        )
        .note(format!(
            "the image is {} × {} pixels, shown {width} wide",
            pixels.width, pixels.height
        ))
        .help(String::from(
            "use an image with more pixels, or show it in a narrower slot",
        )),
    )
}

/// The bytes of a deck image, with the warning its resolution earns.
fn deck_image(
    deck: &Deck,
    dir: &Path,
    used: &AssetUse,
    reference: &AssetRef,
) -> Result<(Vec<u8>, Option<Diagnostic>), Box<Diagnostic>> {
    let site = site(deck, reference);
    let path = dir.join(reference.as_str());
    let bytes = io::read(&path)
        .map_err(|error| located(read_error(&error, &path, reference.as_str()), site.clone()))?;
    if !kind_matches(reference.extension(), &bytes) {
        return Err(located(
            Diagnostic::new(
                Code::ImageKind,
                format!(
                    "the image `{}` is not a {} file",
                    reference.as_str(),
                    reference.extension()
                ),
            )
            .help(String::from(
                "name the file by the kind it holds: png, jpg, jpeg, or svg",
            )),
            site,
        ));
    }
    let warning =
        coarse(reference.as_str(), &bytes, used.width).map(|warning| *located(warning, site));
    Ok((bytes, warning))
}

fn theme_image(theme_dir: Option<&Path>, path: &str) -> Result<Vec<u8>, Box<Diagnostic>> {
    let Some(dir) = theme_dir else {
        return Err(Box::new(Diagnostic::new(
            Code::ImageMissing,
            format!("the built-in theme names the image `{path}`, which it cannot carry"),
        )));
    };
    let full = dir.join(path);
    io::read(&full).map_err(|error| {
        Box::new(read_error(&error, &full, path).note(String::from("the theme names the image")))
    })
}

fn load_one(
    deck: &Deck,
    dirs: Dirs<'_>,
    used: &AssetUse,
) -> Result<(Asset, Option<Diagnostic>), Box<Diagnostic>> {
    let (bytes, warning) = match &used.origin {
        AssetOrigin::Deck(reference) => deck_image(deck, dirs.deck, used, reference)?,
        AssetOrigin::Theme(path) => {
            let bytes = theme_image(dirs.theme, path)?;
            let warning = coarse(path, &bytes, used.width)
                .map(|warning| warning.note(String::from("the theme names the image")));
            (bytes, warning)
        }
    };
    Ok((
        Asset {
            path: used.path.clone(),
            bytes,
        },
        warning,
    ))
}

/// Reads every file the source names, with a diagnostic for each that is missing, unreadable, too large, not of its claimed kind, or too coarse.
#[must_use]
pub fn load(uses: &[AssetUse], deck: &Deck, dirs: Dirs<'_>) -> Loaded {
    let mut loaded = Loaded::default();
    for used in uses {
        match load_one(deck, dirs, used) {
            Ok((asset, warning)) => {
                loaded.assets.push(asset);
                loaded.diagnostics.extend(warning);
            }
            Err(diagnostic) => loaded.diagnostics.push(*diagnostic),
        }
    }
    if has_error(&loaded.diagnostics) {
        loaded.assets.clear();
    }
    loaded
}

#[cfg(test)]
mod tests {
    use super::{Dirs, kind_matches, load};
    use sivi_core::diagnostic::Code;
    use sivi_core::emit::{AssetOrigin, AssetUse};
    use sivi_core::ir::{AssetRef, Deck, Head};
    use sivi_core::length::Length;
    use std::path::Path;

    #[test]
    fn each_kind_is_told_by_its_first_bytes() {
        assert!(kind_matches("png", b"\x89PNG\r\n\x1a\nrest"));
        assert!(kind_matches("jpg", b"\xff\xd8\xff\xe0"));
        assert!(kind_matches("jpeg", b"\xff\xd8\xff\xe1"));
        assert!(kind_matches("svg", b"\xef\xbb\xbf  <svg/>"));
        assert!(!kind_matches("svg", b"\x89PNG"));
        assert!(!kind_matches("png", b"<svg/>"));
        assert!(!kind_matches("gif", b"GIF89a"));
    }

    #[test]
    fn a_missing_image_is_named_with_its_path() {
        let deck = Deck {
            head: Head::default(),
            slides: Vec::new(),
        };
        let width = Length::from_millipoints(100_000);
        let used = AssetUse {
            path: String::from("deck/missing.png"),
            origin: AssetOrigin::Deck(AssetRef::new("missing.png").unwrap()),
            width,
        };
        let dirs = Dirs {
            deck: Path::new("/nonexistent/sivi"),
            theme: None,
        };
        let loaded = load(&[used], &deck, dirs);
        assert_eq!(loaded.assets, Vec::new());
        assert_eq!(loaded.diagnostics[0].code, Code::ImageMissing);
        assert!(loaded.diagnostics[0].message.contains("missing.png"));
        let themed = AssetUse {
            path: String::from("theme/logo.png"),
            origin: AssetOrigin::Theme(String::from("logo.png")),
            width,
        };
        assert_eq!(
            load(&[themed], &deck, dirs).diagnostics[0].code,
            Code::ImageMissing
        );
    }
}
