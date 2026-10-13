//! The fonts the binary bundles, which every build uses instead of any font of the host.

use typst::foundations::Bytes;
use typst::text::{Font, FontBook};

/// One bundled font file.
#[derive(Clone, Copy, Debug)]
pub struct BundledFont {
    /// The family name the theme refers to.
    pub family: &'static str,
    /// The file under `fonts/`, pinned by `fonts/manifest.toml`.
    pub file: &'static str,
    pub bytes: &'static [u8],
}

/// Every bundled font, in the order the font book lists them.
pub const BUNDLED: [BundledFont; 3] = [
    BundledFont {
        family: "BIZ UDPGothic",
        file: "BIZUDPGothic-Regular.ttf",
        bytes: include_bytes!("../../../fonts/BIZUDPGothic-Regular.ttf"),
    },
    BundledFont {
        family: "BIZ UDPGothic",
        file: "BIZUDPGothic-Bold.ttf",
        bytes: include_bytes!("../../../fonts/BIZUDPGothic-Bold.ttf"),
    },
    BundledFont {
        family: "JetBrains Mono",
        file: "JetBrainsMono-Regular.ttf",
        bytes: include_bytes!("../../../fonts/JetBrainsMono-Regular.ttf"),
    },
];

/// The parsed bundled fonts, one per face.
#[must_use]
pub fn bundled() -> Vec<Font> {
    BUNDLED
        .iter()
        .flat_map(|font| Font::iter(Bytes::new(font.bytes)))
        .collect()
}

/// The faces of a font file a theme loads, in the order the file lists them.
#[must_use]
pub fn faces(bytes: Vec<u8>) -> Vec<Font> {
    Font::iter(Bytes::new(bytes)).collect()
}

/// The font book over `fonts`, which Typst consults for family and variant lookups.
#[must_use]
pub fn book(fonts: &[Font]) -> FontBook {
    FontBook::from_fonts(fonts)
}

#[cfg(test)]
mod tests {
    use super::{BUNDLED, book, bundled};

    #[test]
    fn every_bundled_file_parses_as_one_face_with_its_declared_family() {
        let fonts = bundled();
        assert_eq!(fonts.len(), BUNDLED.len());
        for (font, bundled) in fonts.iter().zip(BUNDLED.iter()) {
            assert_eq!(font.info().family, bundled.family, "{}", bundled.file);
        }
        let book = book(&fonts);
        assert!(book.contains_family("biz udpgothic"));
        assert!(book.contains_family("jetbrains mono"));
        assert!(!book.contains_family("libertinus serif"));
    }
}
