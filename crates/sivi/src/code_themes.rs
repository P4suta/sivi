//! The `code-themes` command: every scheme the binary carries, for a person with swatches or for a program as JSON.

use crate::args::{Environment, Format, Rendering, VariantChoice};
use crate::cli::{Run, SUCCESS_EXIT_CODE, USAGE_EXIT_CODE};
use sivi_core::syntax_theme::{BACKGROUND, FOREGROUND, PALETTE_SIZE, Scheme, Variant};
use sivi_theme::schemes::{Schemes, schemes};

/// The JSON listing's format name and version.
pub const LISTING_FORMAT: &str = "sivi.code-themes";
pub const LISTING_VERSION: u32 = 1;
/// The first accent color of a palette, after the eight shades of background and foreground.
const FIRST_ACCENT: usize = 0x08;
/// The width of one swatch, in cells.
const SWATCH: &str = "  ";
/// The escape that ends a styled run.
const RESET: &str = "\x1b[0m";

/// The schemes `variant` keeps, or every scheme.
#[must_use]
pub fn rows(all: &Schemes, variant: Option<Variant>) -> Vec<&Scheme> {
    all.all()
        .iter()
        .filter(|scheme| variant.is_none_or(|wanted| scheme.variant == wanted))
        .collect()
}

/// A run of cells on the background of `color`.
fn swatch(color: sivi_core::theme::Color) -> String {
    format!(
        "\x1b[48;2;{};{};{}m{SWATCH}{RESET}",
        color.red, color.green, color.blue
    )
}

/// The listing for a person: one line per scheme, with the palette as swatches when styled.
#[must_use]
pub fn human_listing(found: &[&Scheme], styled: bool) -> String {
    let width = found
        .iter()
        .map(|scheme| scheme.slug.chars().count())
        .max()
        .unwrap_or(0);
    let lines: Vec<String> = found
        .iter()
        .map(|scheme| {
            let swatches = if styled {
                (BACKGROUND..PALETTE_SIZE)
                    .filter(|role| {
                        *role == BACKGROUND || *role == FOREGROUND || *role >= FIRST_ACCENT
                    })
                    .map(|role| swatch(scheme.color(role)))
                    .collect::<Vec<_>>()
                    .concat()
            } else {
                String::new()
            };
            let separator = if styled { "  " } else { "" };
            format!(
                "{:<width$}  {:<6}{swatches}{separator}{}  {}\n",
                scheme.slug,
                scheme.variant.name(),
                scheme.name,
                scheme.author,
            )
        })
        .collect();
    lines.concat()
}

/// The listing as one JSON document, with every color of every palette.
#[must_use]
pub fn json_listing(all: &Schemes, found: &[&Scheme]) -> String {
    let entries: Vec<serde_json::Value> = found
        .iter()
        .map(|scheme| {
            serde_json::json!({
                "slug": scheme.slug,
                "name": scheme.name,
                "author": scheme.author,
                "variant": scheme.variant.name(),
                "palette": scheme.palette.iter().map(ToString::to_string).collect::<Vec<_>>(),
            })
        })
        .collect();
    let document = serde_json::json!({
        "format": LISTING_FORMAT,
        "version": LISTING_VERSION,
        "commit": all.commit,
        "license_sha256": all.license_sha256,
        "schemes": entries,
    });
    format!("{document}\n")
}

/// Runs `code-themes`.
#[must_use]
pub fn run(variant: Option<VariantChoice>, rendering: Rendering, environment: &Environment) -> Run {
    let all = match schemes() {
        Ok(all) => all,
        Err(why) => {
            return Run {
                stdout: Vec::new(),
                stderr: format!("error: the carried code schemes do not parse: {why}\n"),
                code: USAGE_EXIT_CODE,
            };
        }
    };
    let found = rows(all, variant.map(VariantChoice::variant));
    let stdout = match rendering.format {
        Format::Human => human_listing(&found, rendering.color.styled(environment)),
        Format::Json => json_listing(all, &found),
    };
    Run {
        stdout: stdout.into_bytes(),
        stderr: String::new(),
        code: SUCCESS_EXIT_CODE,
    }
}

#[cfg(test)]
mod tests {
    use super::{human_listing, json_listing, rows};
    use sivi_core::syntax_theme::Variant;
    use sivi_theme::schemes::parse;

    const TWO: &str = "commit = \"abc\"\nlicense_sha256 = \"00\"\n\n[[scheme]]\nslug = \"dim\"\nname = \"Dim\"\nauthor = \"a\"\nvariant = \"dark\"\npalette = [\"#000000\", \"#111111\", \"#222222\", \"#333333\", \"#444444\", \"#555555\", \"#666666\", \"#777777\", \"#888888\", \"#999999\", \"#aaaaaa\", \"#bbbbbb\", \"#cccccc\", \"#dddddd\", \"#eeeeee\", \"#ffffff\"]\n\n[[scheme]]\nslug = \"paper-white\"\nname = \"Paper\"\nauthor = \"b\"\nvariant = \"light\"\npalette = [\"#ffffff\", \"#eeeeee\", \"#dddddd\", \"#cccccc\", \"#bbbbbb\", \"#aaaaaa\", \"#999999\", \"#888888\", \"#777777\", \"#666666\", \"#555555\", \"#444444\", \"#333333\", \"#222222\", \"#111111\", \"#000000\"]\n";

    #[test]
    fn the_listing_filters_by_variant_and_shows_swatches_only_when_styled() {
        let all = parse(TWO).unwrap();
        let every = rows(&all, None);
        assert_eq!(every.len(), 2);
        let light = rows(&all, Some(Variant::Light));
        assert_eq!(light.len(), 1);
        assert_eq!(light[0].slug, "paper-white");
        let plain = human_listing(&every, false);
        assert_eq!(
            plain,
            "dim          dark  Dim  a\npaper-white  light Paper  b\n"
        );
        let styled = human_listing(&every, true);
        assert!(styled.contains("\x1b[48;2;0;0;0m  \x1b[0m"), "{styled:?}");
        assert!(
            styled.contains("\x1b[48;2;255;255;255m  \x1b[0m"),
            "{styled:?}"
        );
        assert_eq!(styled.matches("\x1b[48;2;").count(), 20);
        let json = json_listing(&all, &light);
        assert!(json.contains("\"format\":\"sivi.code-themes\""), "{json}");
        assert!(json.contains("\"commit\":\"abc\""), "{json}");
        assert!(json.contains("\"slug\":\"paper-white\""), "{json}");
        assert!(json.contains("\"palette\":[\"#ffffff\","), "{json}");
        assert!(!json.contains("\"dim\""), "{json}");
    }
}
