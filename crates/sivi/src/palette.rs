//! The `palette` command: the carried palettes by name, or a palette grown from a seed, as theme text with swatches.

use crate::args::{Environment, Format, HarmonyChoice, ModeChoice, Rendering};
use crate::cli::{Run, SUCCESS_EXIT_CODE, USAGE_EXIT_CODE};
use sivi_core::palette_rules::{self, CURATED, Harmony, Mode, Named, Palette, Seed};
use sivi_core::theme::Color;
use sivi_theme::units;

/// The JSON listing's format name and version.
pub const LISTING_FORMAT: &str = "sivi.palette";
pub const LISTING_VERSION: u32 = 1;
/// The width of one swatch, in cells, and the escape that ends a styled run.
const SWATCH: &str = "  ";
const RESET: &str = "\x1b[0m";

/// What the command grows: a carried palette, or a seed color with a mode and a harmony.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub seed: String,
    pub mode: Option<ModeChoice>,
    pub harmony: Option<HarmonyChoice>,
}

/// The seed, mode, and harmony `request` names, or why it names none.
fn resolve(request: &Request) -> Result<(String, Seed, Mode, Harmony), String> {
    let (source, seed, mode, harmony) = if let Some(named) = palette_rules::curated(&request.seed) {
        let Named {
            name,
            seed,
            mode,
            harmony,
        } = named;
        (format!("name = \"{name}\""), seed, mode, harmony)
    } else {
        let color = units::color(&request.seed).ok_or_else(|| {
            format!(
                "`{}` is neither a carried palette nor a color as `#rrggbb`; `sivi palette` lists the palettes",
                request.seed
            )
        })?;
        let seed = Seed::of(color).ok_or_else(|| String::from("the seed has no hue"))?;
        (
            format!("seed = \"{color}\""),
            seed,
            Mode::Light,
            Harmony::Complementary,
        )
    };
    Ok((
        source,
        seed,
        request.mode.map_or(mode, ModeChoice::mode),
        request.harmony.map_or(harmony, HarmonyChoice::harmony),
    ))
}

/// The palette as theme text: the `[palette]` that grows it and the `[colors]` it grew.
#[must_use]
pub fn theme_text(source: &str, mode: Mode, harmony: Harmony, palette: &Palette) -> String {
    let mut lines = vec![
        String::from("[palette]\n"),
        format!("{source}\n"),
        format!("mode = \"{}\"\n", mode.name()),
        format!("harmony = \"{}\"\n\n", harmony.name()),
        String::from("[colors]\n"),
    ];
    for (role, color) in palette.roles() {
        lines.push(format!("{role} = \"{color}\"\n"));
    }
    lines.concat()
}

/// A run of cells on the background of `color`.
fn swatch(color: Color) -> String {
    format!(
        "\x1b[48;2;{};{};{}m{SWATCH}{RESET}",
        color.red, color.green, color.blue
    )
}

/// The palette for a person's eyes: one line per role with its swatch.
#[must_use]
pub fn swatches(palette: &Palette) -> String {
    let lines: Vec<String> = palette
        .roles()
        .iter()
        .map(|(role, color)| format!("{role:<10}{}  {color}\n", swatch(*color)))
        .collect();
    lines.concat()
}

/// The carried palettes for a person, with their swatches when styled.
#[must_use]
pub fn listing(styled: bool) -> String {
    let lines: Vec<String> = CURATED
        .iter()
        .map(|named| {
            let grown = palette_rules::palette(named.seed, named.mode, named.harmony);
            let preview = match (styled, grown) {
                (true, Some(palette)) => palette
                    .roles()
                    .iter()
                    .map(|(_, color)| swatch(*color))
                    .collect::<Vec<_>>()
                    .concat(),
                _ => String::new(),
            };
            format!(
                "{:<12}{:<6}{:<14}{preview}\n",
                named.name,
                named.mode.name(),
                named.harmony.name()
            )
        })
        .collect();
    lines.concat()
}

/// The palette as one JSON document.
#[must_use]
pub fn json(source: &str, mode: Mode, harmony: Harmony, palette: &Palette) -> String {
    let colors: serde_json::Map<String, serde_json::Value> = palette
        .roles()
        .iter()
        .map(|(role, color)| {
            (
                (*role).to_owned(),
                serde_json::Value::from(color.to_string()),
            )
        })
        .collect();
    let document = serde_json::json!({
        "format": LISTING_FORMAT,
        "version": LISTING_VERSION,
        "source": source,
        "mode": mode.name(),
        "harmony": harmony.name(),
        "colors": colors,
    });
    format!("{document}\n")
}

/// Runs `palette`: the carried palettes without a seed, or the palette the seed grows.
#[must_use]
pub fn run(request: Option<&Request>, rendering: Rendering, environment: &Environment) -> Run {
    let styled = rendering.color.styled(environment);
    let Some(request) = request else {
        return Run {
            stdout: listing(styled).into_bytes(),
            stderr: String::new(),
            code: SUCCESS_EXIT_CODE,
        };
    };
    let (source, seed, mode, harmony) = match resolve(request) {
        Ok(resolved) => resolved,
        Err(why) => {
            return Run {
                stdout: Vec::new(),
                stderr: format!("error: {why}\n"),
                code: USAGE_EXIT_CODE,
            };
        }
    };
    let Some(palette) = palette_rules::palette(seed, mode, harmony) else {
        return Run {
            stdout: Vec::new(),
            stderr: String::from("error: the palette could not be grown from its seed\n"),
            code: USAGE_EXIT_CODE,
        };
    };
    let stdout = match rendering.format {
        Format::Human => theme_text(&source, mode, harmony, &palette),
        Format::Json => json(&source, mode, harmony, &palette),
    };
    Run {
        stdout: stdout.into_bytes(),
        stderr: if styled && rendering.format == Format::Human {
            swatches(&palette)
        } else {
            String::new()
        },
        code: SUCCESS_EXIT_CODE,
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, json, listing, resolve, swatches, theme_text};
    use crate::args::{HarmonyChoice, ModeChoice};
    use sivi_core::palette_rules::{self, Harmony, Mode};

    #[test]
    fn a_seed_or_a_name_resolves_and_the_text_names_what_grew_it() {
        let (source, seed, mode, harmony) = resolve(&Request {
            seed: String::from("#0b57d0"),
            mode: None,
            harmony: None,
        })
        .unwrap();
        assert_eq!(source, "seed = \"#0b57d0\"");
        assert!(seed.chroma > 100_000);
        assert_eq!((mode, harmony), (Mode::Light, Harmony::Complementary));
        let (named_source, _, named_mode, named_harmony) = resolve(&Request {
            seed: String::from("sakura"),
            mode: Some(ModeChoice::Dark),
            harmony: Some(HarmonyChoice::Triadic),
        })
        .unwrap();
        assert_eq!(named_source, "name = \"sakura\"");
        assert_eq!((named_mode, named_harmony), (Mode::Dark, Harmony::Triadic));
        let unknown = resolve(&Request {
            seed: String::from("mauve"),
            mode: None,
            harmony: None,
        })
        .unwrap_err();
        assert!(unknown.contains("neither"), "{unknown}");
        let palette = palette_rules::palette(seed, named_mode, named_harmony).unwrap();
        let text = theme_text(&named_source, named_mode, named_harmony, &palette);
        assert!(text.starts_with("[palette]\nname = \"sakura\"\nmode = \"dark\"\nharmony = \"triadic\"\n\n[colors]\npaper = \"#"), "{text}");
        assert!(text.contains("\nhighlight = \"#"), "{text}");
        let document = json(&named_source, named_mode, named_harmony, &palette);
        assert!(
            document.contains("\"format\":\"sivi.palette\""),
            "{document}"
        );
        assert!(document.contains("\"paper\":\"#"), "{document}");
        let shown = swatches(&palette);
        assert_eq!(shown.lines().count(), 10);
        assert!(shown.contains("\x1b[48;2;"), "{shown}");
        let plain = listing(false);
        assert!(plain.starts_with("ink         light mono"), "{plain}");
        assert_eq!(plain.lines().count(), palette_rules::CURATED_COUNT);
        assert!(listing(true).contains("\x1b[48;2;"));
    }
}
