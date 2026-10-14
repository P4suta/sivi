//! The design lints over a loaded theme: text that reads too faintly on the paper, too many hues, and a paper that tints every slide.
//!
//! The judgments are the core's; this module names each finding by its path in the theme.

use sivi_core::color;
use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::palette_rules::{MAX_HUES, MIN_BODY_CONTRAST, hue_count, is_crowd, is_faint, tints};
use sivi_core::theme::{Color, StyleName, Theme, Thousandths};

/// One in thousandths, the thousandths in one tenth, and the micro-units in one hundredth, for spelling ratios and chromas.
const THOUSAND: i64 = 1000;
const TENTH: i64 = 100;
const HUNDREDTH: i64 = 10_000;

/// A ratio in thousandths spelled with one decimal, such as `4.5`.
#[must_use]
pub fn ratio_text(ratio: Thousandths) -> String {
    let whole = ratio.0.checked_div(THOUSAND).unwrap_or(0);
    let tenths = ratio
        .0
        .checked_rem(THOUSAND)
        .and_then(|rest| rest.checked_div(TENTH))
        .unwrap_or(0);
    format!("{whole}.{tenths}")
}

/// A chroma in micro-units spelled with two decimals, such as `0.08`.
#[must_use]
pub fn chroma_text(chroma: i64) -> String {
    let whole = chroma.checked_div(color::UNIT).unwrap_or(0);
    let hundredths = chroma
        .checked_rem(color::UNIT)
        .and_then(|rest| rest.checked_div(HUNDREDTH))
        .unwrap_or(0);
    format!("{whole}.{hundredths:02}")
}

/// The text colors of the theme with the path each is set at: every text style and every mark that sets a color.
fn text_colors(theme: &Theme) -> Vec<(String, &StyleName)> {
    let mut found: Vec<(String, &StyleName)> = theme
        .text
        .iter()
        .map(|(name, style)| (format!("text.{name}.color"), &style.color))
        .collect();
    for (name, mark) in [
        ("emphasis", &theme.inline.emphasis),
        ("strong", &theme.inline.strong),
        ("code", &theme.inline.code),
        ("link", &theme.inline.link),
        ("mark", &theme.inline.mark),
    ] {
        if let Some(color) = &mark.color {
            found.push((format!("inline.{name}.color"), color));
        }
    }
    found
}

/// A finding at a path of the theme.
fn finding(code: Code, message: String, path: &str, help: &str) -> Diagnostic {
    Diagnostic::new(code, message)
        .note(format!("at `{path}` in the theme"))
        .help(String::from(help))
}

/// Every text color that reads below the body contrast on the paper.
fn faint_text(theme: &Theme, paper: Color) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for (path, name) in text_colors(theme) {
        let Some(ink) = theme.color(name) else {
            continue;
        };
        let Some(ratio) = color::contrast(ink, paper) else {
            continue;
        };
        if is_faint(ratio) {
            found.push(finding(
                Code::LowContrast,
                format!(
                    "`{name}` reads at {} to one on the paper, below {} to one",
                    ratio_text(ratio),
                    ratio_text(MIN_BODY_CONTRAST)
                ),
                &path,
                "darken the text or lighten the paper; a palette grown from a seed keeps seven to one",
            ));
        }
    }
    found
}

/// Whether the theme's colors form more hue families than a slide reads well with.
fn crowd(theme: &Theme) -> Option<Diagnostic> {
    let colors: Vec<Color> = theme.colors.iter().map(|(_, color)| *color).collect();
    let families = hue_count(&colors)?;
    is_crowd(families).then(|| {
        finding(
            Code::ManyHues,
            format!("the colors form {families} hue families; a slide reads best with {MAX_HUES} or fewer"),
            "colors",
            "grow the colors from one seed with `[palette]`, or drop a hue",
        )
    })
}

/// Whether the paper carries a chroma that tints every slide.
fn vivid_paper(theme: &Theme, paper: Color) -> Option<Diagnostic> {
    let chroma = color::oklch(paper)?.chroma;
    tints(chroma).then(|| {
        finding(
            Code::VividPaper,
            format!(
                "the paper `{}` carries a chroma of {}, which tints every slide",
                theme.page.background,
                chroma_text(chroma)
            ),
            "page.background",
            "keep the paper near neutral and put the hue in the accents",
        )
    })
}

/// Every design finding of the theme: faint text as errors, a crowd of hues and a vivid paper as warnings.
#[must_use]
pub fn lints(theme: &Theme) -> Vec<Diagnostic> {
    let Some(paper) = theme.color(&theme.page.background) else {
        return Vec::new();
    };
    let mut found = faint_text(theme, paper);
    found.extend(crowd(theme));
    found.extend(vivid_paper(theme, paper));
    found
}

#[cfg(test)]
mod tests {
    use super::{chroma_text, lints, ratio_text};
    use sivi_core::diagnostic::Code;
    use sivi_core::theme::Thousandths;

    const SMALL: &str = include_str!("../../../fuzz/corpus/theme_toml/minimal.toml");

    fn codes(theme: &str) -> Vec<Code> {
        let loaded = crate::parse(theme).unwrap();
        lints(&loaded).iter().map(|d| d.code).collect()
    }

    #[test]
    fn ratios_spell_with_one_decimal() {
        assert_eq!(ratio_text(Thousandths(4500)), "4.5");
        assert_eq!(ratio_text(Thousandths(21_000)), "21.0");
        assert_eq!(ratio_text(Thousandths(1049)), "1.0");
        assert_eq!(ratio_text(Thousandths(0)), "0.0");
        assert_eq!(chroma_text(80_000), "0.08");
        assert_eq!(chroma_text(123_456), "0.12");
        assert_eq!(chroma_text(0), "0.00");
    }

    #[test]
    fn a_sound_theme_has_no_finding_and_each_breach_is_named_at_its_path() {
        assert_eq!(codes(SMALL), Vec::<Code>::new());
        let faint = SMALL.replacen("ink = \"#000000\"", "ink = \"#9a9a9a\"", 1);
        let found = lints(&crate::parse(&faint).unwrap());
        assert!(
            found.iter().all(|d| d.code == Code::LowContrast),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.notes.iter().any(|note| note.contains("text.body.color"))),
            "{found:?}"
        );
        assert!(
            found[0].message.contains("to one on the paper"),
            "{found:?}"
        );
        let vivid = SMALL.replacen("paper = \"#ffffff\"", "paper = \"#ffb0b0\"", 1);
        assert_eq!(codes(&vivid), vec![Code::VividPaper]);
        let crowd = SMALL.replacen(
            "ink = \"#000000\"",
            "ink = \"#000000\"\nred = \"#ff0000\"\ngreen = \"#00c000\"\nblue = \"#0000ff\"\nyellow = \"#f0e000\"",
            1,
        );
        assert_eq!(codes(&crowd), vec![Code::ManyHues]);
        assert_eq!(
            lints(&crate::parse(&crowd).unwrap())[0].message,
            "the colors form 4 hue families; a slide reads best with 3 or fewer"
        );
    }
}
