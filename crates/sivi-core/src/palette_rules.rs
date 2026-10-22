//! How a palette grows from one seed, and what a palette must keep.
//!
//! Every role a theme colors takes a lightness and a chroma the mode sets, on a hue the harmony sets from the seed,
//! so a palette holds few hues, keeps the body on the paper at seven to one, and keeps every accent on the paper at three to one.

use crate::color::{self, Oklch};
use crate::length::Rect;
use crate::theme::{Color, ScrimEdge, Thousandths};

/// The lowest contrast of body text on its paper, as the accessibility guidelines ask of normal text.
pub const MIN_BODY_CONTRAST: Thousandths = Thousandths(4500);
/// The contrast a generated palette keeps between its ink and its paper.
pub const PALETTE_BODY_CONTRAST: Thousandths = Thousandths(7000);
/// The contrast a generated palette keeps between an accent and its paper, as the guidelines ask of large text.
pub const MIN_ACCENT_CONTRAST: Thousandths = Thousandths(3000);
/// How many hue families a palette may hold before it reads as a crowd.
pub const MAX_HUES: usize = 3;
/// The chroma a paper may carry before it tints every slide.
pub const MAX_PAPER_CHROMA: i64 = 60_000;
/// The chroma below which a color reads as neutral and joins no hue family.
pub const NEUTRAL_CHROMA: i64 = 40_000;
/// How far two hues may lie apart, in tenths of a degree, and still read as one family.
pub const HUE_SPAN: u16 = 300;

/// Whether text at `contrast` on its paper reads too faintly for a body.
#[must_use]
pub const fn is_faint(contrast: Thousandths) -> bool {
    contrast.0 < MIN_BODY_CONTRAST.0
}

/// Whether `families` are more hues than a slide reads well with.
#[must_use]
pub const fn is_crowd(families: usize) -> bool {
    families > MAX_HUES
}

/// Whether a paper at `chroma` tints every slide.
#[must_use]
pub const fn tints(chroma: i64) -> bool {
    chroma > MAX_PAPER_CHROMA
}

/// How opaque a scrim is at the lightest edge of `slot` over `backdrop`, in thousandths.
///
/// A scrim is solid at the edge it starts from and clear at the far edge, so a slot reads through the scrim at the strength the gradient has where the slot ends toward the clear edge; a slot past that edge reads none.
#[must_use]
pub fn scrim_strength(
    from: ScrimEdge,
    opacity: Thousandths,
    backdrop: Rect,
    slot: Rect,
) -> Option<Thousandths> {
    let height = backdrop.size.height.0;
    if height <= 0 {
        return None;
    }
    // The distance from the clear edge to the slot's edge nearest it, which the gradient covers at that fraction.
    let distance = match from {
        ScrimEdge::Bottom => slot.origin.y.0.checked_sub(backdrop.origin.y.0)?,
        ScrimEdge::Top => backdrop.bottom()?.0.checked_sub(slot.bottom()?.0)?,
    };
    let fraction = distance.clamp(0, height);
    Some(Thousandths(
        opacity.0.checked_mul(fraction)?.checked_div(height)?,
    ))
}
/// The tenths of a degree in a turn.
const TURN: u16 = color::TURN;
/// The chroma an accent is raised to, and the chroma it is held below.
const ACCENT_FLOOR: i64 = 100_000;
const ACCENT_CEILING: i64 = 160_000;
/// The number of roles a palette colors.
pub const ROLES: usize = 10;
/// The index of each role among the tones, in the order of `Palette::roles`.
const PAPER: usize = 0;
const INK: usize = 1;
const MUTED: usize = 2;
const RULE: usize = 3;
const CODE_BG: usize = 4;
const ACCENT: usize = 5;
const ACCENT2: usize = 6;
const ACCENT3: usize = 7;
const HIGHLIGHT: usize = 8;
const SOFT: usize = 9;
/// The hue of the second accent and the marker, as an offset from the seed, for each harmony.
const ANALOGOUS_STEP: u16 = 300;
const COMPLEMENT: u16 = 1800;
const SPLIT_STEP: u16 = 1500;
const TRIADIC_STEP: u16 = 1200;

/// Whether the palette sits on light paper or dark.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    /// The mode as a theme spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// The mode named `text`.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        [Self::Light, Self::Dark]
            .into_iter()
            .find(|mode| mode.name() == text)
    }
}

/// How the accents relate to the seed on the hue circle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Harmony {
    /// Every accent on the seed's hue.
    Mono,
    /// The accents a step to either side of the seed.
    Analogous,
    /// The second accent across the circle, the third a step beside the seed.
    Complementary,
    /// The accents to either side of the seed's complement.
    Split,
    /// The accents a third of a turn apart.
    Triadic,
}

impl Harmony {
    /// Every harmony, in the order a listing shows them.
    pub const ALL: [Self; 5] = [
        Self::Mono,
        Self::Analogous,
        Self::Complementary,
        Self::Split,
        Self::Triadic,
    ];

    /// The harmony as a theme spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mono => "mono",
            Self::Analogous => "analogous",
            Self::Complementary => "complementary",
            Self::Split => "split",
            Self::Triadic => "triadic",
        }
    }

    /// The harmony named `text`.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|harmony| harmony.name() == text)
    }
}

/// What a palette grows from: a hue and the chroma the seed color carried.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Seed {
    pub hue: u16,
    pub chroma: i64,
}

impl Seed {
    /// The seed of a color.
    #[must_use]
    pub fn of(color: Color) -> Option<Self> {
        let lch = color::oklch(color)?;
        Some(Self {
            hue: lch.hue,
            chroma: lch.chroma,
        })
    }
}

/// A palette the binary carries by name: a seed, a mode, and a harmony chosen to read well together.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Named {
    pub name: &'static str,
    pub seed: Seed,
    pub mode: Mode,
    pub harmony: Harmony,
}

/// The number of palettes the binary carries by name.
pub const CURATED_COUNT: usize = 12;

/// The palettes the binary carries by name, each tuned so a theme can start from a word.
pub const CURATED: [Named; CURATED_COUNT] = [
    Named {
        name: "ink",
        seed: Seed {
            hue: 2600,
            chroma: 0,
        },
        mode: Mode::Light,
        harmony: Harmony::Mono,
    },
    Named {
        name: "indigo",
        seed: Seed {
            hue: 2650,
            chroma: 160_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Complementary,
    },
    Named {
        name: "sakura",
        seed: Seed {
            hue: 3500,
            chroma: 120_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Analogous,
    },
    Named {
        name: "matcha",
        seed: Seed {
            hue: 1300,
            chroma: 100_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Analogous,
    },
    Named {
        name: "sunset",
        seed: Seed {
            hue: 400,
            chroma: 160_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Split,
    },
    Named {
        name: "ocean",
        seed: Seed {
            hue: 2300,
            chroma: 140_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Analogous,
    },
    Named {
        name: "wine",
        seed: Seed {
            hue: 150,
            chroma: 130_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Complementary,
    },
    Named {
        name: "sand",
        seed: Seed {
            hue: 700,
            chroma: 60_000,
        },
        mode: Mode::Light,
        harmony: Harmony::Analogous,
    },
    Named {
        name: "slate-night",
        seed: Seed {
            hue: 2550,
            chroma: 60_000,
        },
        mode: Mode::Dark,
        harmony: Harmony::Mono,
    },
    Named {
        name: "neon-night",
        seed: Seed {
            hue: 3000,
            chroma: 160_000,
        },
        mode: Mode::Dark,
        harmony: Harmony::Complementary,
    },
    Named {
        name: "forest",
        seed: Seed {
            hue: 1500,
            chroma: 100_000,
        },
        mode: Mode::Dark,
        harmony: Harmony::Analogous,
    },
    Named {
        name: "coral",
        seed: Seed {
            hue: 250,
            chroma: 160_000,
        },
        mode: Mode::Dark,
        harmony: Harmony::Split,
    },
];

/// The carried palette named `name`.
#[must_use]
pub fn curated(name: &str) -> Option<Named> {
    CURATED.into_iter().find(|named| named.name == name)
}

/// The colors a theme names, grown from one seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Palette {
    pub paper: Color,
    pub ink: Color,
    pub muted: Color,
    pub rule: Color,
    pub code_bg: Color,
    pub accent: Color,
    pub accent2: Color,
    pub accent3: Color,
    pub highlight: Color,
    pub soft: Color,
}

impl Palette {
    /// Every role with its color, in the order a theme lists them.
    #[must_use]
    pub const fn roles(&self) -> [(&'static str, Color); ROLES] {
        [
            ("paper", self.paper),
            ("ink", self.ink),
            ("muted", self.muted),
            ("rule", self.rule),
            ("code_bg", self.code_bg),
            ("accent", self.accent),
            ("accent2", self.accent2),
            ("accent3", self.accent3),
            ("highlight", self.highlight),
            ("soft", self.soft),
        ]
    }
}

/// The lightness and chroma of a role, in micro-units.
#[derive(Clone, Copy)]
struct Tone {
    lightness: i64,
    chroma: i64,
}

/// The tones of the roles on light paper, in the order of `Palette::roles`, with the accent's chroma from the seed.
const LIGHT: [Tone; ROLES] = [
    Tone {
        lightness: 985_000,
        chroma: 10_000,
    },
    Tone {
        lightness: 220_000,
        chroma: 20_000,
    },
    Tone {
        lightness: 500_000,
        chroma: 30_000,
    },
    Tone {
        lightness: 900_000,
        chroma: 15_000,
    },
    Tone {
        lightness: 960_000,
        chroma: 12_000,
    },
    Tone {
        lightness: 520_000,
        chroma: 0,
    },
    Tone {
        lightness: 560_000,
        chroma: 130_000,
    },
    Tone {
        lightness: 600_000,
        chroma: 120_000,
    },
    Tone {
        lightness: 900_000,
        chroma: 130_000,
    },
    Tone {
        lightness: 940_000,
        chroma: 50_000,
    },
];
/// The tones of the roles on dark paper.
const DARK: [Tone; ROLES] = [
    Tone {
        lightness: 180_000,
        chroma: 20_000,
    },
    Tone {
        lightness: 950_000,
        chroma: 10_000,
    },
    Tone {
        lightness: 720_000,
        chroma: 25_000,
    },
    Tone {
        lightness: 300_000,
        chroma: 20_000,
    },
    Tone {
        lightness: 230_000,
        chroma: 20_000,
    },
    Tone {
        lightness: 760_000,
        chroma: 0,
    },
    Tone {
        lightness: 780_000,
        chroma: 120_000,
    },
    Tone {
        lightness: 800_000,
        chroma: 110_000,
    },
    Tone {
        lightness: 720_000,
        chroma: 140_000,
    },
    Tone {
        lightness: 280_000,
        chroma: 60_000,
    },
];

/// A hue turned by `offset`, back within the turn.
#[must_use]
pub fn turned(hue: u16, offset: u16) -> u16 {
    hue.checked_rem(TURN)
        .and_then(|within| within.checked_add(offset))
        .and_then(|sum| sum.checked_rem(TURN))
        .unwrap_or(0)
}

/// The hues of the seed, the second accent, and the third accent under `harmony`.
#[must_use]
pub fn hues(seed: u16, harmony: Harmony) -> [u16; 3] {
    let seed = turned(seed, 0);
    match harmony {
        Harmony::Mono => [seed, seed, seed],
        Harmony::Analogous => [
            seed,
            turned(seed, ANALOGOUS_STEP),
            turned(seed, TURN.saturating_sub(ANALOGOUS_STEP)),
        ],
        Harmony::Complementary => [seed, turned(seed, COMPLEMENT), turned(seed, ANALOGOUS_STEP)],
        Harmony::Split => [
            seed,
            turned(seed, SPLIT_STEP),
            turned(seed, TURN.saturating_sub(SPLIT_STEP)),
        ],
        Harmony::Triadic => [
            seed,
            turned(seed, TRIADIC_STEP),
            turned(seed, TURN.saturating_sub(TRIADIC_STEP)),
        ],
    }
}

/// The shortest distance between two hues around the circle.
#[must_use]
pub fn hue_distance(first: u16, second: u16) -> u16 {
    let apart = turned(first, 0).abs_diff(turned(second, 0));
    apart.min(TURN.saturating_sub(apart))
}

/// How many families `hues` form, each family holding hues within `HUE_SPAN` of its first.
#[must_use]
pub fn hue_families(hues: &[u16]) -> usize {
    let mut leaders: [Option<u16>; ROLES] = [None; ROLES];
    let mut count: usize = 0;
    for hue in hues {
        let known = leaders
            .iter()
            .flatten()
            .any(|leader| hue_distance(*leader, *hue) <= HUE_SPAN);
        if !known {
            if let Some(slot) = leaders.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(*hue);
            }
            count = count.saturating_add(1);
        }
    }
    count
}

/// How many hue families the chromatic colors among `colors` form.
#[must_use]
pub fn hue_count(colors: &[Color]) -> Option<usize> {
    let mut chromatic = [0; ROLES];
    let mut found = 0;
    for color in colors {
        let lch = color::oklch(*color)?;
        if lch.chroma >= NEUTRAL_CHROMA
            && let Some(slot) = chromatic.get_mut(found)
        {
            *slot = lch.hue;
            found = found.saturating_add(1);
        }
    }
    Some(hue_families(chromatic.get(..found)?))
}

/// The color of a role at `tone` on `hue`, within the gamut.
fn role(tone: Tone, hue: u16) -> Option<Color> {
    color::fitted(Oklch {
        lightness: tone.lightness,
        chroma: tone.chroma,
        hue,
    })
}

/// The chroma of the main accent: the seed's, raised to the floor unless the seed is neutral, and held below the ceiling.
#[must_use]
pub fn accent_chroma(seed: Seed) -> i64 {
    if seed.chroma < NEUTRAL_CHROMA {
        0
    } else {
        seed.chroma.clamp(ACCENT_FLOOR, ACCENT_CEILING)
    }
}

/// The palette grown from `seed` in `mode` under `harmony`.
#[must_use]
pub fn palette(seed: Seed, mode: Mode, harmony: Harmony) -> Option<Palette> {
    let tones = match mode {
        Mode::Light => LIGHT,
        Mode::Dark => DARK,
    };
    let [base, second, third] = hues(seed.hue, harmony);
    let accent = Tone {
        lightness: tones[ACCENT].lightness,
        chroma: accent_chroma(seed),
    };
    // A neutral seed grows neutral accents too, so a gray palette stays gray but for its marker.
    let companion = |tone: Tone| Tone {
        lightness: tone.lightness,
        chroma: if accent.chroma == 0 { 0 } else { tone.chroma },
    };
    Some(Palette {
        paper: role(tones[PAPER], base)?,
        ink: role(tones[INK], base)?,
        muted: role(tones[MUTED], base)?,
        rule: role(tones[RULE], base)?,
        code_bg: role(tones[CODE_BG], base)?,
        accent: role(accent, base)?,
        accent2: role(companion(tones[ACCENT2]), second)?,
        accent3: role(companion(tones[ACCENT3]), third)?,
        highlight: role(tones[HIGHLIGHT], second)?,
        soft: role(tones[SOFT], base)?,
    })
}

/// Whether `palette` keeps what every grown palette must: its contrasts and its few hues.
#[must_use]
pub fn keeps(palette: &Palette) -> Option<bool> {
    let body = color::contrast(palette.ink, palette.paper)?;
    let accents = [palette.accent, palette.accent2, palette.accent3];
    let mut accent_contrast = true;
    for accent in accents {
        accent_contrast &= color::contrast(accent, palette.paper)? >= MIN_ACCENT_CONTRAST;
    }
    let families = hue_count(&[
        palette.accent,
        palette.accent2,
        palette.accent3,
        palette.highlight,
        palette.soft,
    ])?;
    Some(body >= PALETTE_BODY_CONTRAST && accent_contrast && families <= MAX_HUES)
}

/// The properties Kani proves over every hue and harmony.
#[cfg(any(test, kani))]
mod proofs {
    use super::{
        COMPLEMENT, HUE_SPAN, Harmony, MAX_HUES, TURN, hue_distance, hue_families, hues, turned,
    };

    /// The hues of a harmony lie within the turn, start at the seed, and read as the harmony says.
    pub(super) fn spec_hues(seed: u16, harmony: Harmony) {
        let [first, second, third] = hues(seed, harmony);
        assert!(first < TURN && second < TURN && third < TURN);
        assert_eq!(first, turned(seed, 0));
        match harmony {
            Harmony::Mono => assert!(second == first && third == first),
            Harmony::Complementary => assert_eq!(second, turned(first, COMPLEMENT)),
            Harmony::Analogous | Harmony::Split | Harmony::Triadic => {
                assert_eq!(hue_distance(first, second), hue_distance(first, third));
            }
        }
        assert!(hue_families(&[first, second, third]) <= MAX_HUES);
    }

    /// Hue families count at most one per hue, one for hues within the span, and none for no hue.
    pub(super) fn spec_families(hues: [u16; 3]) {
        let count = hue_families(&hues);
        assert!(count <= hues.len());
        assert!(count >= 1);
        if hues
            .iter()
            .all(|hue| hue_distance(*hue, hues[0]) <= HUE_SPAN)
        {
            assert_eq!(count, 1);
        }
        assert_eq!(hue_families(&[]), 0);
        assert_eq!(
            hue_distance(hues[0], hues[1]),
            hue_distance(hues[1], hues[0])
        );
        assert!(hue_distance(hues[0], hues[1]) <= TURN / 2);
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(12)]
    fn every_harmony_turns_within_the_circle_and_holds_at_most_three_hues() {
        let seed: u16 = kani::any();
        kani::assume(seed < TURN);
        for harmony in Harmony::ALL {
            spec_hues(seed, harmony);
        }
        kani::cover!(seed == 0, "the first hue");
        kani::cover!(seed == TURN - 1, "the last hue");
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(12)]
    fn hue_families_count_the_hues_a_span_apart_once() {
        let hues: [u16; 3] = [kani::any(), kani::any(), kani::any()];
        kani::assume(hues.iter().all(|hue| *hue < TURN));
        spec_families(hues);
        kani::cover!(hue_families(&hues) == 3, "three families");
        kani::cover!(hue_families(&hues) == 1, "one family");
    }

    /// Four hues a quarter turn apart are four families; this probe claims they fit three and must fail.
    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(12)]
    fn rejects_a_fourth_hue_family() {
        kani::assert(
            hue_families(&[0, 900, 1800, 2700]) <= MAX_HUES,
            "NEGATIVE_PALETTE_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::proofs::{spec_families, spec_hues};
    use super::{
        Harmony, MAX_HUES, Mode, Palette, Seed, TURN, accent_chroma, hue_count, hue_families, hues,
        keeps, palette,
    };
    use crate::color;
    use crate::theme::Color;
    use alloc::vec::Vec;

    /// The steps of the sweeps, kept coarse so the mutation campaign runs the suite in well under a second.
    const HUE_SWEEP: usize = 300;
    const SEED_SWEEP: usize = 37;
    const FIRST_SWEEP: usize = 97;
    const SECOND_SWEEP: usize = 131;

    #[test]
    #[cfg_attr(
        miri,
        ignore = "every hue and every harmony take minutes under Miri; the sampled tests stand in"
    )]
    fn every_hue_and_harmony_meets_the_specification() {
        for seed in (0..TURN).step_by(SEED_SWEEP) {
            for harmony in Harmony::ALL {
                spec_hues(seed, harmony);
            }
        }
        for first in (0..TURN).step_by(FIRST_SWEEP) {
            for second in (0..TURN).step_by(SECOND_SWEEP) {
                spec_families([first, second, 0]);
                spec_families([first, second, 3599]);
            }
        }
        assert_eq!(hues(3900, Harmony::Mono), [300, 300, 300]);
        assert_eq!(hues(3500, Harmony::Analogous), [3500, 200, 3200]);
        assert_eq!(hues(0, Harmony::Triadic), [0, 1200, 2400]);
        assert_eq!(hue_families(&[0, 900, 1800, 2700]), 4);
        assert_eq!(hue_families(&[100, 200, 3500]), 1);
        assert_eq!(hue_families(&[0, 310]), 2);
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "thousands of grown palettes take minutes under Miri; the carried palettes stand in"
    )]
    fn every_grown_palette_keeps_its_contrasts_and_its_few_hues() {
        for hue in (0..TURN).step_by(HUE_SWEEP) {
            for chroma in [0, 30_000, 100_000, 250_000] {
                for mode in [Mode::Light, Mode::Dark] {
                    for harmony in Harmony::ALL {
                        let seed = Seed { hue, chroma };
                        let grown = palette(seed, mode, harmony).unwrap();
                        assert_eq!(
                            keeps(&grown),
                            Some(true),
                            "{seed:?} {mode:?} {harmony:?} {grown:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_carried_palette_keeps_its_rules_under_a_name_of_its_own() {
        let tested = [
            "ink",
            "indigo",
            "sakura",
            "matcha",
            "sunset",
            "ocean",
            "wine",
            "sand",
            "slate-night",
            "neon-night",
            "forest",
            "coral",
        ];
        let mut names: Vec<&str> = Vec::new();
        for named in super::CURATED {
            assert!(!names.contains(&named.name), "{}", named.name);
            assert!(
                named
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
                "{}",
                named.name
            );
            names.push(named.name);
            assert!(tested.contains(&named.name));
            assert_eq!(super::curated(named.name), Some(named));
        }
        assert_eq!(names.len(), super::CURATED_COUNT);
        assert_eq!(names.len(), tested.len());
        assert_eq!(super::curated("Sakura"), None);
    }

    fn carried(name: &str) {
        let named = super::curated(name).unwrap();
        let grown = palette(named.seed, named.mode, named.harmony).unwrap();
        assert_eq!(keeps(&grown), Some(true), "{named:?} {grown:?}");
    }

    #[test]
    fn ink_keeps_its_rules() {
        carried("ink");
    }
    #[test]
    fn indigo_keeps_its_rules() {
        carried("indigo");
    }
    #[test]
    fn sakura_keeps_its_rules() {
        carried("sakura");
    }
    #[test]
    fn matcha_keeps_its_rules() {
        carried("matcha");
    }
    #[test]
    fn sunset_keeps_its_rules() {
        carried("sunset");
    }
    #[test]
    fn ocean_keeps_its_rules() {
        carried("ocean");
    }
    #[test]
    fn wine_keeps_its_rules() {
        carried("wine");
    }
    #[test]
    fn sand_keeps_its_rules() {
        carried("sand");
    }
    #[test]
    fn slate_night_keeps_its_rules() {
        carried("slate-night");
    }
    #[test]
    fn neon_night_keeps_its_rules() {
        carried("neon-night");
    }
    #[test]
    fn forest_keeps_its_rules() {
        carried("forest");
    }
    #[test]
    fn coral_keeps_its_rules() {
        carried("coral");
    }

    #[test]
    fn a_scrim_reads_at_the_gradient_where_the_slot_ends_toward_the_clear_edge() {
        use super::scrim_strength;
        use crate::length::{Length, Point, Rect, Size};
        use crate::theme::{ScrimEdge, Thousandths};
        let rect = |x: i64, y: i64, width: i64, height: i64| {
            Rect::new(
                Point {
                    x: Length::from_millipoints(x),
                    y: Length::from_millipoints(y),
                },
                Size {
                    width: Length::from_millipoints(width),
                    height: Length::from_millipoints(height),
                },
            )
        };
        let backdrop = rect(0, 0, 960, 540);
        let opacity = Thousandths(800);
        // From the bottom, a slot whose top lies at the middle reads at half the opacity.
        assert_eq!(
            scrim_strength(
                ScrimEdge::Bottom,
                opacity,
                backdrop,
                rect(60, 270, 840, 100)
            ),
            Some(Thousandths(400))
        );
        assert_eq!(
            scrim_strength(ScrimEdge::Bottom, opacity, backdrop, rect(60, 0, 840, 100)),
            Some(Thousandths(0))
        );
        assert_eq!(
            scrim_strength(
                ScrimEdge::Bottom,
                opacity,
                backdrop,
                rect(60, 540, 840, 100)
            ),
            Some(opacity)
        );
        assert_eq!(
            scrim_strength(
                ScrimEdge::Bottom,
                opacity,
                backdrop,
                rect(60, -10, 840, 100)
            ),
            Some(Thousandths(0))
        );
        assert_eq!(
            scrim_strength(
                ScrimEdge::Bottom,
                opacity,
                backdrop,
                rect(60, 600, 840, 100)
            ),
            Some(opacity)
        );
        // From the top, the slot's bottom edge is the one nearest the clear edge.
        assert_eq!(
            scrim_strength(ScrimEdge::Top, opacity, backdrop, rect(60, 170, 840, 100)),
            Some(Thousandths(400))
        );
        assert_eq!(
            scrim_strength(ScrimEdge::Top, opacity, backdrop, rect(60, 0, 840, 540)),
            Some(Thousandths(0))
        );
        assert_eq!(
            scrim_strength(ScrimEdge::Top, opacity, backdrop, rect(60, 0, 840, 1)),
            Some(Thousandths(798))
        );
        assert_eq!(
            scrim_strength(
                ScrimEdge::Top,
                opacity,
                rect(0, 100, 960, 540),
                rect(60, 100, 840, 1)
            ),
            Some(Thousandths(798))
        );
        assert_eq!(
            scrim_strength(
                ScrimEdge::Bottom,
                opacity,
                rect(0, 0, 960, 0),
                rect(0, 0, 1, 1)
            ),
            None
        );
        assert_eq!(
            scrim_strength(ScrimEdge::Top, opacity, backdrop, rect(0, i64::MAX, 1, 1)),
            None
        );
    }

    #[test]
    fn the_judgments_turn_exactly_at_their_thresholds() {
        use super::{is_crowd, is_faint, tints};
        use crate::theme::Thousandths;
        assert!(is_faint(Thousandths(4499)));
        assert!(!is_faint(Thousandths(4500)));
        assert!(!is_faint(Thousandths(4501)));
        assert!(!is_crowd(3));
        assert!(is_crowd(4));
        assert!(!tints(60_000));
        assert!(tints(60_001));
    }

    #[test]
    fn a_palette_is_refused_for_each_breach_alone() {
        let blue = Seed {
            hue: 2650,
            chroma: 160_000,
        };
        let sound = palette(blue, Mode::Light, Harmony::Complementary).unwrap();
        assert_eq!(keeps(&sound), Some(true));
        // Faint ink alone: the body contrast fails while every accent and the hue count pass.
        let faint = Palette {
            ink: sound.rule,
            ..sound
        };
        assert_eq!(keeps(&faint), Some(false));
        // One pale accent alone: yellow on the paper reads near one to one, with few hues.
        let pale = Palette {
            accent2: Color {
                red: 255,
                green: 255,
                blue: 0,
            },
            ..sound
        };
        assert_eq!(keeps(&pale), Some(false));
        // Four hue families alone: every accent keeps its contrast, but a crowd of hues gathers.
        let crowd = Palette {
            accent2: Color {
                red: 0,
                green: 100,
                blue: 0,
            },
            accent3: Color {
                red: 160,
                green: 0,
                blue: 0,
            },
            highlight: Color {
                red: 200,
                green: 0,
                blue: 200,
            },
            ..sound
        };
        assert!(color::contrast(crowd.accent2, crowd.paper).unwrap() >= super::MIN_ACCENT_CONTRAST);
        assert!(color::contrast(crowd.accent3, crowd.paper).unwrap() >= super::MIN_ACCENT_CONTRAST);
        assert_eq!(keeps(&crowd), Some(false));
    }

    #[test]
    fn the_seed_reads_from_a_color_and_the_accent_chroma_stays_vivid_but_restrained() {
        let blue = Seed::of(Color {
            red: 0x0b,
            green: 0x57,
            blue: 0xd0,
        })
        .unwrap();
        assert!((2550..=2700).contains(&blue.hue), "{blue:?}");
        assert!(blue.chroma > 150_000, "{blue:?}");
        assert_eq!(accent_chroma(blue), 160_000);
        assert_eq!(accent_chroma(Seed { hue: 0, chroma: 0 }), 0);
        assert_eq!(
            accent_chroma(Seed {
                hue: 0,
                chroma: 50_000
            }),
            100_000
        );
        assert_eq!(
            accent_chroma(Seed {
                hue: 0,
                chroma: 120_000
            }),
            120_000
        );
        assert_eq!(
            accent_chroma(Seed {
                hue: 0,
                chroma: super::NEUTRAL_CHROMA
            }),
            100_000
        );
        let gray = palette(Seed { hue: 0, chroma: 0 }, Mode::Light, Harmony::Mono).unwrap();
        assert_eq!(
            hue_count(&[gray.accent, gray.accent2, gray.accent3, gray.paper]),
            Some(0)
        );
        let light = palette(blue, Mode::Light, Harmony::Complementary).unwrap();
        assert_eq!(hue_count(&[light.accent2]), Some(1));
        assert!(
            color::luminance(light.paper).unwrap() > 900_000,
            "{light:?}"
        );
        assert!(color::luminance(light.ink).unwrap() < 60_000, "{light:?}");
        let names: Vec<&str> = light.roles().iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "paper",
                "ink",
                "muted",
                "rule",
                "code_bg",
                "accent",
                "accent2",
                "accent3",
                "highlight",
                "soft"
            ]
        );
        let dark = palette(blue, Mode::Dark, Harmony::Complementary).unwrap();
        assert!(color::luminance(dark.paper).unwrap() < 60_000, "{dark:?}");
        assert_ne!(light, dark);
        let crowd: Palette = Palette {
            accent: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            accent2: Color {
                red: 0,
                green: 255,
                blue: 0,
            },
            accent3: Color {
                red: 0,
                green: 0,
                blue: 255,
            },
            highlight: Color {
                red: 255,
                green: 255,
                blue: 0,
            },
            ..light
        };
        assert_eq!(keeps(&crowd), Some(false));
        assert!(
            hue_count(&[crowd.accent, crowd.accent2, crowd.accent3, crowd.highlight]).unwrap()
                > MAX_HUES
        );
        assert_eq!(Mode::from_name("dark"), Some(Mode::Dark));
        assert_eq!(Harmony::from_name("split"), Some(Harmony::Split));
        assert_eq!(Harmony::from_name("tetradic"), None);
    }
}
