//! The design lints over a loaded theme: text that reads too faintly on what lies under it, too many hues, a paper that tints every slide, and a slot over a backdrop whose image has no scrim.
//!
//! The judgments are the core's; this module names each finding by its path in the theme.

use sivi_core::color;
use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::geometry_rules::overlaps;
use sivi_core::palette_rules::{
    MAX_HUES, MIN_BODY_CONTRAST, hue_count, is_crowd, is_faint, scrim_strength, tints,
};
use sivi_core::theme::{Color, Layer, Master, Slot, StyleName, Theme, Thousandths};

/// One in thousandths, the thousandths in one tenth, the micro-units in one hundredth, and the thousandths in one percent, for spelling ratios, chromas, and strengths.
const THOUSAND: i64 = 1000;
const TENTH: i64 = 100;
const HUNDREDTH: i64 = 10_000;
const PERCENT: i64 = 10;

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

/// A strength in thousandths spelled as whole percent, such as `42%`.
fn percent_text(strength: Thousandths) -> String {
    format!("{}%", strength.0.checked_div(PERCENT).unwrap_or(0))
}

/// What the text of a slot is read on.
enum Ground<'a> {
    /// The paper.
    Paper,
    /// The scrim of a backdrop at the strength the slot reads through, over the mid gray the lints assume.
    Scrim(Scrimmed<'a>),
    /// A backdrop without a scrim, which no rule can read on; the bare backdrop warning covers it.
    Bare,
}

/// A backdrop with a scrim, and how strong the scrim is where a slot reads through it.
#[derive(Clone, Copy)]
struct Scrimmed<'a> {
    backdrop: &'a Slot,
    strength: Thousandths,
}

/// The backdrop of `master` whose region lies under `slot`, if any.
fn backdrop_under<'a>(master: &'a Master, slot: &Slot) -> Option<&'a Slot> {
    if slot.layer != Layer::Front {
        return None;
    }
    master
        .slots
        .iter()
        .find(|other| other.layer == Layer::Back && overlaps(other.region, slot.region))
}

/// The marks that set a color, with the path each is set at.
fn mark_colors(theme: &Theme) -> Vec<(String, &StyleName)> {
    let mut found = Vec::new();
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

/// The low-contrast finding for `name` at `ratio` on the paper, at `path`.
fn faint_on_paper(name: &StyleName, ratio: Thousandths, path: &str) -> Diagnostic {
    finding(
        Code::LowContrast,
        format!(
            "`{name}` reads at {} to one on the paper, below {} to one",
            ratio_text(ratio),
            ratio_text(MIN_BODY_CONTRAST)
        ),
        path,
        "darken the text or lighten the paper; a palette grown from a seed keeps seven to one",
    )
}

/// The slots of every master with the path of each.
fn slots(theme: &Theme) -> impl Iterator<Item = (String, &Master, &Slot)> {
    theme.masters.iter().flat_map(|master| {
        master.slots.iter().map(move |slot| {
            (
                format!("masters.{}.slots.{}", master.name, slot.name),
                master,
                slot,
            )
        })
    })
}

/// The low-contrast finding for `name` over the scrim of `backdrop` at `strength`, on a mid-gray image, at `path`.
fn faint_on_scrim(
    theme: &Theme,
    name: &StyleName,
    over: Scrimmed<'_>,
    path: &str,
) -> Option<Diagnostic> {
    let Scrimmed { backdrop, strength } = over;
    let ink = theme.color(name)?;
    let scrim = theme.color(&backdrop.image.scrim.as_ref()?.color)?;
    let under = color::over(scrim, strength, color::MID_GRAY)?;
    let ratio = color::contrast(ink, under)?;
    is_faint(ratio).then(|| {
        finding(
            Code::LowContrast,
            format!(
                "`{name}` reads at {} to one over the scrim of `{}` at {} on a mid-gray image, below {} to one",
                ratio_text(ratio),
                backdrop.name,
                percent_text(strength),
                ratio_text(MIN_BODY_CONTRAST)
            ),
            path,
            "deepen the scrim, darken its color, or move the slot toward the edge the scrim starts from",
        )
    })
}

/// Every text color that reads below the body contrast on the paper or on a scrim.
///
/// A text style is read on the paper wherever a slot sets it in front of nothing, or where no slot sets it, and on a scrim where a slot over a backdrop with a scrim sets it; the marks are read on the paper.
fn faint_text(theme: &Theme, paper: Color) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    let mut on_paper: Vec<(String, &StyleName)> = Vec::new();
    for (name, style) in &theme.text {
        // A backdrop sets no text, so only the front slots read their style.
        let uses: Vec<(String, Ground<'_>)> = slots(theme)
            .filter(|(_, _, slot)| slot.text == *name && slot.layer == Layer::Front)
            .map(|(path, master, slot)| {
                let Some(backdrop) = backdrop_under(master, slot) else {
                    return (path, Ground::Paper);
                };
                let ground = backdrop
                    .image
                    .scrim
                    .as_ref()
                    .and_then(|scrim| {
                        scrim_strength(scrim.from, scrim.opacity, backdrop.region, slot.region)
                    })
                    .map_or(Ground::Bare, |strength| {
                        Ground::Scrim(Scrimmed { backdrop, strength })
                    });
                (path, ground)
            })
            .collect();
        let style_path = format!("text.{name}.color");
        if uses.is_empty()
            || uses
                .iter()
                .any(|(_, ground)| matches!(ground, Ground::Paper))
        {
            on_paper.push((style_path, &style.color));
        }
        for (path, ground) in uses {
            if let Ground::Scrim(over) = ground {
                found.extend(faint_on_scrim(theme, &style.color, over, &path));
            }
        }
    }
    on_paper.extend(mark_colors(theme));
    for (path, name) in on_paper {
        let Some(ink) = theme.color(name) else {
            continue;
        };
        let Some(ratio) = color::contrast(ink, paper) else {
            continue;
        };
        if is_faint(ratio) {
            found.push(faint_on_paper(name, ratio, &path));
        }
    }
    found
}

/// Every front slot over a backdrop whose image has no scrim.
fn bare_backdrops(theme: &Theme) -> Vec<Diagnostic> {
    slots(theme)
        .filter_map(|(path, master, slot)| {
            let backdrop = backdrop_under(master, slot)?;
            backdrop.image.scrim.is_none().then(|| {
                finding(
                    Code::BareBackdrop,
                    format!(
                        "the slot `{}` lies over the backdrop `{}`, whose image has no scrim",
                        slot.name, backdrop.name
                    ),
                    &path,
                    "give the backdrop's `image` a `scrim`, so the text reads on any image, or move the slot off it",
                )
            })
        })
        .collect()
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

/// Every design finding of the theme: faint text as errors; a crowd of hues, a vivid paper, and a bare backdrop as warnings.
#[must_use]
pub fn lints(theme: &Theme) -> Vec<Diagnostic> {
    let Some(paper) = theme.color(&theme.page.background) else {
        return Vec::new();
    };
    let mut found = faint_text(theme, paper);
    found.extend(crowd(theme));
    found.extend(vivid_paper(theme, paper));
    found.extend(bare_backdrops(theme));
    found
}

#[cfg(test)]
mod tests {
    use super::{chroma_text, lints, percent_text, ratio_text};
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
        assert_eq!(percent_text(Thousandths(417)), "41%");
        assert_eq!(percent_text(Thousandths(1000)), "100%");
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

    /// The small theme with a backdrop under its one slot, which sits in the lower two fifths, whose text is set in a pale style, and the scrim given.
    fn over_backdrop(scrim: &str, text_color: &str) -> String {
        let pale = format!("ink = \"#000000\"\npale = \"{text_color}\"");
        let themed = SMALL
            .replacen("ink = \"#000000\"", &pale, 1)
            .replacen("color = \"ink\"", "color = \"pale\"", 1)
            .replacen(
                "region = { x = \"0pt\", y = \"0pt\", width = \"100%\", height = \"100%\" }",
                "region = { x = \"0pt\", y = \"60%\", width = \"100%\", height = \"40%\" }",
                1,
            );
        format!(
            "{themed}\n[masters.only.slots.backdrop]\nbinds = \"flow\"\naccepts = [\"image\"]\nlayer = \"back\"\nregion = {{ x = \"0pt\", y = \"0pt\", width = \"100%\", height = \"100%\" }}\ntext = \"body\"\nmax_blocks = 1\nimage = {{ fit = \"cover\"{scrim} }}\n"
        )
    }

    #[test]
    fn text_over_a_scrim_reads_on_the_scrim_and_a_bare_backdrop_warns() {
        let scrimmed = over_backdrop(", scrim = { color = \"ink\", opacity = 0.9 }", "#ffffff");
        assert_eq!(codes(&scrimmed), Vec::<Code>::new(), "{scrimmed}");
        let weak = over_backdrop(", scrim = { color = \"ink\", opacity = 0.05 }", "#ffffff");
        let found = lints(&crate::parse(&weak).unwrap());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, Code::LowContrast);
        assert!(
            found[0].message.contains("over the scrim of `backdrop` at"),
            "{}",
            found[0].message
        );
        assert!(
            found[0]
                .notes
                .iter()
                .any(|note| note.contains("masters.only.slots.")),
            "{found:?}"
        );
        let bare = over_backdrop("", "#ffffff");
        let warned = lints(&crate::parse(&bare).unwrap());
        assert_eq!(warned.len(), 1, "{warned:?}");
        assert_eq!(warned[0].code, Code::BareBackdrop);
        assert!(
            warned[0].message.ends_with("whose image has no scrim"),
            "{}",
            warned[0].message
        );
    }
}
