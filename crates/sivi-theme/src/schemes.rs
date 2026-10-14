//! The code schemes the binary carries, generated from tinted-theming/schemes at a pinned commit.
//!
//! A theme names a scheme by its slug under `[code] theme`; the loader hands the scheme to the core, which paints code with it.

use crate::units;
use serde::Deserialize;
use sivi_core::syntax_theme::{PALETTE_SIZE, Scheme, Variant};
use sivi_core::theme::Color;
use std::sync::OnceLock;

/// The schemes text the binary carries.
pub const TEXT: &str = include_str!("../../../themes/code/schemes.toml");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    commit: String,
    license_sha256: String,
    scheme: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    slug: String,
    name: String,
    author: String,
    variant: String,
    palette: Vec<String>,
}

/// Every scheme the binary carries, by slug, with the commit they come from.
#[derive(Debug)]
pub struct Schemes {
    /// The commit of tinted-theming/schemes every scheme comes from.
    pub commit: String,
    /// The digest of the license the schemes carry beside them.
    pub license_sha256: String,
    entries: Vec<Scheme>,
}

impl Schemes {
    /// Every scheme, in slug order.
    #[must_use]
    pub fn all(&self) -> &[Scheme] {
        &self.entries
    }

    /// The scheme with the exact `slug`.
    #[must_use]
    pub fn find(&self, slug: &str) -> Option<&Scheme> {
        self.entries.iter().find(|scheme| scheme.slug == slug)
    }

    /// Every slug, in order.
    pub fn slugs(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|scheme| scheme.slug.as_str())
    }
}

/// The scheme of one entry, or why the entry is not one.
fn scheme(entry: Entry) -> Result<Scheme, String> {
    let variant = Variant::from_name(&entry.variant).ok_or_else(|| {
        format!(
            "{}: the variant {} is not dark or light",
            entry.slug, entry.variant
        )
    })?;
    let mut palette = [Color::default(); PALETTE_SIZE];
    if entry.palette.len() != PALETTE_SIZE {
        return Err(format!(
            "{}: {} colors, not {PALETTE_SIZE}",
            entry.slug,
            entry.palette.len()
        ));
    }
    for (slot, text) in palette.iter_mut().zip(&entry.palette) {
        *slot =
            units::color(text).ok_or_else(|| format!("{}: {text} is not a color", entry.slug))?;
    }
    Ok(Scheme {
        slug: entry.slug,
        name: entry.name,
        author: entry.author,
        variant,
        palette,
    })
}

/// The schemes `text` holds, or why it is not a schemes file.
///
/// # Errors
///
/// Returns why the text does not parse or an entry is not a scheme.
pub fn parse(text: &str) -> Result<Schemes, String> {
    let document: Document = toml::from_str(text).map_err(|error| error.to_string())?;
    let schemes = document
        .scheme
        .into_iter()
        .map(scheme)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Schemes {
        commit: document.commit,
        license_sha256: document.license_sha256,
        entries: schemes,
    })
}

/// The schemes the binary carries, parsed once.
///
/// # Errors
///
/// Returns why the carried text does not parse, which is a defect of the build.
pub fn schemes() -> Result<&'static Schemes, String> {
    static PARSED: OnceLock<Result<Schemes, String>> = OnceLock::new();
    PARSED
        .get_or_init(|| parse(TEXT))
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(test)]
mod tests {
    use super::{parse, schemes};
    use sivi_core::syntax_theme::{BACKGROUND, Variant};

    const SMALL: &str = "commit = \"abc\"\nlicense_sha256 = \"00\"\n\n[[scheme]]\nslug = \"dim\"\nname = \"Dim\"\nauthor = \"a\"\nvariant = \"dark\"\npalette = [\"#000000\", \"#111111\", \"#222222\", \"#333333\", \"#444444\", \"#555555\", \"#666666\", \"#777777\", \"#888888\", \"#999999\", \"#aaaaaa\", \"#bbbbbb\", \"#cccccc\", \"#dddddd\", \"#eeeeee\", \"#ffffff\"]\n";

    #[test]
    fn a_schemes_file_reads_its_entries_and_refuses_a_broken_one() {
        let found = parse(SMALL).unwrap();
        assert_eq!(found.commit, "abc");
        assert_eq!(found.all().len(), 1);
        assert_eq!(found.slugs().collect::<Vec<_>>(), vec!["dim"]);
        let dim = found.find("dim").unwrap();
        assert_eq!(dim.variant, Variant::Dark);
        assert_eq!(dim.color(BACKGROUND).to_string(), "#000000");
        assert_eq!(dim.foreground().to_string(), "#555555");
        assert!(found.find("Dim").is_none());
        parse(&SMALL.replace("\"dark\"", "\"dim\"")).unwrap_err();
        parse(&SMALL.replace("\"#ffffff\"", "\"#fffff\"")).unwrap_err();
        parse(&SMALL.replace(", \"#ffffff\"", "")).unwrap_err();
        parse(&SMALL.replace("slug", "slag")).unwrap_err();
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "the carried schemes are twenty times the plain theme, which parses in minutes under Miri"
    )]
    fn the_carried_schemes_parse_in_slug_order_with_the_famous_names_among_them() {
        let carried = schemes().unwrap();
        assert_ne!(carried.commit, "");
        let slugs: Vec<&str> = carried.slugs().collect();
        let mut sorted = slugs.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(slugs, sorted);
        for famous in [
            "ayu-dark",
            "ayu-light",
            "catppuccin-mocha",
            "dracula",
            "github",
            "gruvbox-dark-medium",
            "nord",
            "one-light",
            "solarized-light",
            "tokyo-night-dark",
        ] {
            assert!(carried.find(famous).is_some(), "{famous}");
        }
        let dracula = carried.find("dracula").unwrap();
        assert_eq!(dracula.variant, Variant::Dark);
        assert_eq!(dracula.background().to_string(), "#282a36");
        assert_eq!(dracula.foreground().to_string(), "#f8f8f2");
        assert!(
            carried
                .all()
                .iter()
                .any(|scheme| scheme.variant == Variant::Light)
        );
    }
}
