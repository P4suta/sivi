//! Colors as numbers: linear light, luminance, contrast, and the Oklab and OKLCH spaces, in fixed point.
//!
//! Values are micro-units of one, the Oklab matrices carry ten decimals, and every operation is checked,
//! so a result is `None` only where the arithmetic would leave the integers, which the rules module proves it never does for a color.

use crate::color_tables::{COSINE, LINEAR};
use crate::theme::{Color, Thousandths};

/// One, in the micro-units every value holds.
pub const UNIT: i64 = 1_000_000;
/// One, in the units the matrix coefficients hold.
const COEFFICIENT_UNIT: i64 = 10_000_000_000;
/// The tenths of a degree in a turn, and in a quarter of one.
pub const TURN: u16 = 3600;
const QUARTER_TURN: u16 = 900;
/// Five hundredths, which the contrast ratio adds to each luminance.
const CONTRAST_OFFSET: u32 = 50_000;
/// One, in the thousandths a contrast ratio holds.
const THOUSAND: u32 = 1000;
/// How far outside zero to one a linear channel may fall and still count as inside the gamut.
const GAMUT_TOLERANCE: i64 = 100;
/// The widest chroma any color of the gamut reaches, which bounds the search for a fitting chroma.
pub const MAX_CHROMA: i64 = 400_000;
/// The number of channels a color has, and the rows a matrix has.
const CHANNELS: usize = 3;
/// Two, which halves a divisor for rounding to the nearest and a search span for its middle.
const HALVES: i64 = 2;
const HALVES_NARROW: u32 = 2;
const HALVES_WIDE: i128 = 2;
/// How far past one the root searches reach: twice one for a cube root, four times one for a square root.
const CUBE_ROOT_REACH: i128 = 2;
const SQUARE_ROOT_REACH: i128 = 4;
/// The halvings a search may take, which is more than any span here needs, so every search ends.
const SEARCH_STEPS: u32 = 64;

/// A triple of micro-unit values: a linear color, or the three long, medium, and short cone responses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Triple {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

/// A color in Oklab: its lightness and its two opponent axes, in micro-units.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Oklab {
    pub lightness: i64,
    pub a: i64,
    pub b: i64,
}

/// A color in OKLCH: its lightness and chroma in micro-units, and its hue in tenths of a degree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Oklch {
    pub lightness: i64,
    pub chroma: i64,
    pub hue: u16,
}

/// A row of a matrix, with coefficients in ten decimals.
type Row = [i64; CHANNELS];

/// The weights of linear red, green, and blue in luminance.
const LUMINANCE: Row = [2_126_000_000, 7_152_000_000, 722_000_000];
/// Linear light to the cone responses.
const CONES: [Row; CHANNELS] = [
    [4_122_214_708, 5_363_325_363, 514_459_929],
    [2_119_034_982, 6_806_995_451, 1_073_969_566],
    [883_024_619, 2_817_188_376, 6_299_787_005],
];
/// The cube roots of the cone responses to Oklab.
const OPPONENTS: [Row; CHANNELS] = [
    [2_104_542_553, 7_936_177_850, -40_720_468],
    [19_779_984_951, -24_285_922_050, 4_505_937_099],
    [259_040_371, 7_827_717_662, -8_086_757_660],
];
/// Oklab back to the cube roots of the cone responses.
const CONE_ROOTS: [Row; CHANNELS] = [
    [COEFFICIENT_UNIT, 3_963_377_774, 2_158_037_573],
    [COEFFICIENT_UNIT, -1_055_613_458, -638_541_728],
    [COEFFICIENT_UNIT, -894_841_775, -12_914_855_480],
];
/// The cone responses back to linear light.
const LIGHTS: [Row; CHANNELS] = [
    [40_767_416_621, -33_077_115_913, 2_309_699_292],
    [-12_684_380_046, 26_097_574_011, -3_413_193_965],
    [-41_960_863, -7_034_186_147, 17_076_147_010],
];

/// `value / divisor` rounded to the nearest, with a tie away from zero.
fn divide(value: i64, divisor: i64) -> Option<i64> {
    let half = divisor.checked_div(HALVES)?;
    let adjusted = value.checked_add(half.checked_mul(value.signum())?)?;
    adjusted.checked_div(divisor)
}

/// `value / divisor` rounded to the nearest, for the wide products of a cube.
fn divide_wide(value: i128, divisor: i128) -> Option<i64> {
    let half = divisor.checked_div(HALVES_WIDE)?;
    let adjusted = value.checked_add(half.checked_mul(value.signum())?)?;
    i64::try_from(adjusted.checked_div(divisor)?).ok()
}

/// The dot product of `row` and `input`, back in micro-units; a value within a few units times a coefficient fits the integers.
fn dot(row: Row, input: Triple) -> Option<i64> {
    let [first, second, third] = row;
    let sum = input
        .x
        .checked_mul(first)?
        .checked_add(input.y.checked_mul(second)?)?
        .checked_add(input.z.checked_mul(third)?)?;
    divide(sum, COEFFICIENT_UNIT)
}

/// `matrix` applied to `input`.
fn apply(matrix: [Row; CHANNELS], input: Triple) -> Option<Triple> {
    let [top, middle, bottom] = matrix;
    Some(Triple {
        x: dot(top, input)?,
        y: dot(middle, input)?,
        z: dot(bottom, input)?,
    })
}

/// The linear light of a channel.
#[must_use]
pub fn linear(channel: u8) -> i64 {
    LINEAR
        .get(usize::from(channel))
        .map_or(UNIT, |light| *light)
}

/// The channel whose linear light lies nearest to `light`, with light outside the gamut taking the nearest end.
#[must_use]
pub fn channel(light: i64) -> u8 {
    let above = match LINEAR.binary_search(&light) {
        Ok(exact) => return u8::try_from(exact).unwrap_or(u8::MAX),
        Err(above) => above,
    };
    let Some(upper) = LINEAR.get(above) else {
        return u8::MAX;
    };
    let Some(lower_index) = above.checked_sub(1) else {
        return 0;
    };
    let lower = LINEAR.get(lower_index).map_or(0, |entry| *entry);
    // A light midway between two channels takes the lower, so a faint light stays dark.
    let nearest = if upper.abs_diff(light) < light.abs_diff(lower) {
        above
    } else {
        lower_index
    };
    u8::try_from(nearest).unwrap_or(u8::MAX)
}

/// The linear light of a color.
#[must_use]
pub fn lights(color: Color) -> Triple {
    Triple {
        x: linear(color.red),
        y: linear(color.green),
        z: linear(color.blue),
    }
}

/// The relative luminance of a color, in micro-units.
#[must_use]
pub fn luminance(color: Color) -> Option<i64> {
    dot(LUMINANCE, lights(color))
}

/// The contrast ratio of a lighter luminance over a darker one, in thousandths, to the nearest.
///
/// The arithmetic stays in thirty-two bits, which keeps the division narrow for the prover.
#[must_use]
pub fn ratio(lighter: u32, darker: u32) -> Option<Thousandths> {
    let numerator = lighter
        .checked_add(CONTRAST_OFFSET)?
        .checked_mul(THOUSAND)?;
    let denominator = darker.checked_add(CONTRAST_OFFSET)?;
    let quotient = numerator
        .checked_add(denominator.checked_div(HALVES_NARROW)?)?
        .checked_div(denominator)?;
    Some(Thousandths(i64::from(quotient)))
}

/// The contrast ratio of two colors, in thousandths, from one for equal luminance to twenty-one for black on white.
#[must_use]
pub fn contrast(first: Color, second: Color) -> Option<Thousandths> {
    let one = u32::try_from(luminance(first)?).ok()?;
    let other = u32::try_from(luminance(second)?).ok()?;
    ratio(one.max(other), one.min(other))
}

/// The mid gray the design lints assume as the lightest image under a scrim.
pub const MID_GRAY: Color = Color {
    red: 128,
    green: 128,
    blue: 128,
};

/// One channel of `above` at `opacity` over the same channel of `below`, to the nearest, as a viewer composites a translucent fill.
///
/// `None` for an opacity outside clear to solid.
#[must_use]
pub fn mix(above: u8, opacity: Thousandths, below: u8) -> Option<u8> {
    let cover = u32::try_from(opacity.0).ok()?;
    let rest = THOUSAND.checked_sub(cover)?;
    let sum = u32::from(above)
        .checked_mul(cover)?
        .checked_add(u32::from(below).checked_mul(rest)?)?;
    let nearest = sum
        .checked_add(THOUSAND.checked_div(HALVES_NARROW)?)?
        .checked_div(THOUSAND)?;
    u8::try_from(nearest).ok()
}

/// `top` at `opacity` over `under`, channel by channel.
///
/// `None` for an opacity outside clear to solid.
#[must_use]
pub fn over(top: Color, opacity: Thousandths, under: Color) -> Option<Color> {
    Some(Color {
        red: mix(top.red, opacity, under.red)?,
        green: mix(top.green, opacity, under.green)?,
        blue: mix(top.blue, opacity, under.blue)?,
    })
}

/// The largest `root` from zero to `reach` times one whose `rises(root)` stays at or below `target`, by a bounded halving search.
fn floor_root(target: i128, reach: i128, rises: impl Fn(i128) -> Option<i128>) -> Option<i128> {
    let mut low: i128 = 0;
    let mut high: i128 = i128::from(UNIT).checked_mul(reach)?.checked_add(1)?;
    for _ in 0..SEARCH_STEPS {
        let middle = low.checked_add(high.checked_sub(low)?.checked_div(HALVES_WIDE)?)?;
        if middle == low {
            break;
        }
        if rises(middle)? <= target {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(low)
}

/// The cube root of a micro-unit value, in micro-units, rounded toward zero.
#[must_use]
pub fn cbrt(value: i64) -> Option<i64> {
    let target = i128::from(value.unsigned_abs())
        .checked_mul(i128::from(UNIT))?
        .checked_mul(i128::from(UNIT))?;
    let root = floor_root(target, CUBE_ROOT_REACH, |root| {
        root.checked_mul(root)?.checked_mul(root)
    })?;
    i64::try_from(root).ok()?.checked_mul(value.signum())
}

/// The square root of a value, rounded down, for a chroma from its axes.
#[must_use]
pub fn isqrt(value: i64) -> Option<i64> {
    if value.is_negative() {
        return None;
    }
    let root = floor_root(i128::from(value), SQUARE_ROOT_REACH, |root| {
        root.checked_mul(root)
    })?;
    i64::try_from(root).ok()
}

/// The cosine of a hue in tenths of a degree, in micro-units.
#[must_use]
pub fn cosine(tenths: u16) -> i64 {
    COSINE
        .get(usize::from(tenths.checked_rem(TURN).unwrap_or(0)))
        .map_or(UNIT, |value| *value)
}

/// The sine of a hue, in micro-units: the cosine a quarter turn behind.
#[must_use]
pub fn sine(tenths: u16) -> i64 {
    cosine(
        tenths
            .checked_add(TURN)
            .and_then(|turned| turned.checked_sub(QUARTER_TURN))
            .unwrap_or(0),
    )
}

/// The color in Oklab.
#[must_use]
pub fn oklab(color: Color) -> Option<Oklab> {
    let cones = apply(CONES, lights(color))?;
    let roots = Triple {
        x: cbrt(cones.x)?,
        y: cbrt(cones.y)?,
        z: cbrt(cones.z)?,
    };
    let lab = apply(OPPONENTS, roots)?;
    Some(Oklab {
        lightness: lab.x,
        a: lab.y,
        b: lab.z,
    })
}

/// The hue of an Oklab color: the tenth of a degree whose direction lies nearest to its axes, or zero without chroma.
#[must_use]
pub fn hue(lab: Oklab) -> Option<u16> {
    let mut best = 0;
    let mut best_score = i64::MIN;
    for tenths in 0..TURN {
        let score = lab
            .a
            .checked_mul(cosine(tenths))?
            .checked_add(lab.b.checked_mul(sine(tenths))?)?;
        if score > best_score {
            best_score = score;
            best = tenths;
        }
    }
    Some(best)
}

/// The color in OKLCH.
#[must_use]
pub fn oklch(color: Color) -> Option<Oklch> {
    let lab = oklab(color)?;
    let chroma = isqrt(
        lab.a
            .checked_mul(lab.a)?
            .checked_add(lab.b.checked_mul(lab.b)?)?,
    )?;
    Some(Oklch {
        lightness: lab.lightness,
        chroma,
        hue: if chroma == 0 { 0 } else { hue(lab)? },
    })
}

/// The OKLCH color in Oklab.
#[must_use]
pub fn to_oklab(lch: Oklch) -> Option<Oklab> {
    let axis = |direction: i64| divide(lch.chroma.checked_mul(direction)?, UNIT);
    Some(Oklab {
        lightness: lch.lightness,
        a: axis(cosine(lch.hue))?,
        b: axis(sine(lch.hue))?,
    })
}

/// The linear light of an Oklab color, which may leave zero to one when the color leaves the gamut.
#[must_use]
pub fn lights_of(lab: Oklab) -> Option<Triple> {
    let roots = apply(
        CONE_ROOTS,
        Triple {
            x: lab.lightness,
            y: lab.a,
            z: lab.b,
        },
    )?;
    let cube = |root: i64| {
        let cubed = i128::from(root)
            .checked_mul(i128::from(root))?
            .checked_mul(i128::from(root))?;
        divide_wide(cubed, i128::from(UNIT).checked_mul(i128::from(UNIT))?)
    };
    apply(
        LIGHTS,
        Triple {
            x: cube(roots.x)?,
            y: cube(roots.y)?,
            z: cube(roots.z)?,
        },
    )
}

/// Whether linear light lies within the gamut, allowing the tolerance.
#[must_use]
pub fn in_gamut(lights: Triple) -> bool {
    let low = GAMUT_TOLERANCE.checked_neg().unwrap_or(0);
    let high = UNIT.checked_add(GAMUT_TOLERANCE).unwrap_or(UNIT);
    [lights.x, lights.y, lights.z]
        .into_iter()
        .all(|light| light >= low && light <= high)
}

/// The sRGB color of linear light, with each channel at its nearest value.
#[must_use]
pub fn srgb(lights: Triple) -> Color {
    Color {
        red: channel(lights.x),
        green: channel(lights.y),
        blue: channel(lights.z),
    }
}

/// The sRGB color of an OKLCH color, with the chroma lowered to the widest the gamut holds at that lightness and hue.
#[must_use]
pub fn fitted(lch: Oklch) -> Option<Color> {
    let lightness = lch.lightness.clamp(0, UNIT);
    let within = |chroma: i64| {
        lights_of(to_oklab(Oklch {
            lightness,
            chroma,
            hue: lch.hue,
        })?)
    };
    let wanted = lch.chroma.clamp(0, MAX_CHROMA);
    let direct = within(wanted)?;
    if in_gamut(direct) {
        return Some(srgb(direct));
    }
    let mut low: i64 = 0;
    let mut high = wanted;
    for _ in 0..SEARCH_STEPS {
        let middle = low.checked_add(high.checked_sub(low)?.checked_div(HALVES)?)?;
        if middle == low {
            break;
        }
        if in_gamut(within(middle)?) {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(srgb(within(low)?))
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_CHROMA, Oklch, UNIT, cbrt, channel, contrast, cosine, divide, fitted, hue, in_gamut,
        isqrt, lights_of, linear, luminance, oklab, oklch, sine, srgb, to_oklab,
    };
    use crate::theme::{Color, Thousandths};

    const fn rgb(red: u8, green: u8, blue: u8) -> Color {
        Color { red, green, blue }
    }

    #[test]
    fn linear_light_and_channels_read_back_and_forth() {
        assert_eq!(linear(0), 0);
        assert_eq!(linear(255), UNIT);
        assert_eq!(linear(128), 215_861);
        for value in 0..=u8::MAX {
            assert_eq!(channel(linear(value)), value, "{value}");
        }
        assert_eq!(channel(-5), 0);
        assert_eq!(channel(UNIT * 2), 255);
        assert_eq!(channel(216_000), 128);
        assert_eq!(channel(100), 0);
        assert_eq!(channel(200), 1);
        assert_eq!(channel(152), 0);
        assert_eq!(channel(153), 1);
        assert_eq!(channel(759), 2);
        assert_eq!(channel(304), 1);
    }

    #[test]
    fn luminance_and_contrast_follow_the_standard() {
        assert_eq!(luminance(rgb(255, 255, 255)), Some(UNIT));
        assert_eq!(luminance(rgb(0, 0, 0)), Some(0));
        assert_eq!(luminance(rgb(255, 0, 0)), Some(212_600));
        assert_eq!(
            contrast(rgb(0, 0, 0), rgb(255, 255, 255)),
            Some(Thousandths(21_000))
        );
        assert_eq!(
            contrast(rgb(255, 255, 255), rgb(0, 0, 0)),
            Some(Thousandths(21_000))
        );
        assert_eq!(
            contrast(rgb(31, 31, 31), rgb(31, 31, 31)),
            Some(Thousandths(1000))
        );
        // A deep blue on white, which the guidelines rate near 6.4 to one.
        let blue_on_white = contrast(rgb(0x0b, 0x57, 0xd0), rgb(255, 255, 255)).unwrap();
        assert!(
            (6_300..=6_500).contains(&blue_on_white.0),
            "{blue_on_white:?}"
        );
    }

    #[test]
    fn roots_and_waves_are_exact_at_their_known_points() {
        assert_eq!(cbrt(UNIT), Some(UNIT));
        assert_eq!(cbrt(0), Some(0));
        assert_eq!(cbrt(8 * UNIT), Some(2 * UNIT));
        assert_eq!(cbrt(-8 * UNIT), Some(-2 * UNIT));
        assert_eq!(cbrt(125_000), Some(500_000));
        assert_eq!(cbrt(1), Some(10_000));
        assert_eq!(cbrt(2), Some(12_599));
        assert_eq!(cbrt(-2), Some(-12_599));
        assert_eq!(isqrt(0), Some(0));
        assert_eq!(isqrt(1), Some(1));
        assert_eq!(isqrt(3), Some(1));
        assert_eq!(isqrt(4), Some(2));
        assert_eq!(isqrt(UNIT * UNIT), Some(UNIT));
        assert_eq!(divide(7, 10), Some(1));
        assert_eq!(divide(5, 10), Some(1));
        assert_eq!(divide(4, 10), Some(0));
        assert_eq!(divide(0, 10), Some(0));
        assert_eq!(divide(-4, 10), Some(0));
        assert_eq!(divide(-5, 10), Some(-1));
        assert_eq!(divide(-15, 10), Some(-2));
        assert_eq!(hue(super::Oklab::default()), Some(0));
        assert_eq!(
            hue(super::Oklab {
                lightness: 0,
                a: 0,
                b: UNIT
            }),
            Some(900)
        );
        assert_eq!(isqrt(-1), None);
    }

    #[test]
    fn waves_and_hues_are_exact_at_their_known_points() {
        assert_eq!(cosine(0), UNIT);
        assert_eq!(cosine(1800), -UNIT);
        assert_eq!(cosine(3600), UNIT);
        assert_eq!(sine(900), UNIT);
        assert_eq!(sine(2700), -UNIT);
        assert_eq!(sine(0), 0);
    }

    #[test]
    fn the_primaries_land_where_oklab_puts_them() {
        let white = oklab(rgb(255, 255, 255)).unwrap();
        assert!(
            (999_000..=1_001_000).contains(&white.lightness),
            "{white:?}"
        );
        assert!(white.a.abs() < 1000 && white.b.abs() < 1000, "{white:?}");
        assert_eq!(oklab(rgb(0, 0, 0)), Some(super::Oklab::default()));
        let red = oklch(rgb(255, 0, 0)).unwrap();
        assert!((627_000..=629_000).contains(&red.lightness), "{red:?}");
        assert!((257_000..=259_000).contains(&red.chroma), "{red:?}");
        assert!((288..=296).contains(&red.hue), "{red:?}");
        let green = oklch(rgb(0, 255, 0)).unwrap();
        assert!((866_000..=867_000).contains(&green.lightness), "{green:?}");
        assert!((1420..=1430).contains(&green.hue), "{green:?}");
        let blue = oklch(rgb(0, 0, 255)).unwrap();
        assert!((452_000..=453_000).contains(&blue.lightness), "{blue:?}");
        assert!((2636..=2646).contains(&blue.hue), "{blue:?}");
        assert_eq!(oklch(rgb(128, 128, 128)).unwrap().hue, 0);
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "a thousand hue searches of thirty-six hundred steps take minutes under Miri"
    )]
    fn colors_round_trip_through_oklab_exactly_and_through_oklch_within_the_hue_step() {
        let steps = [0, 51, 128, 187, 255];
        for red in steps {
            for green in steps {
                for blue in steps {
                    let color = rgb(red, green, blue);
                    let lab = oklab(color).unwrap();
                    let lights = lights_of(lab).unwrap();
                    assert!(in_gamut(lights), "{color:?} {lights:?}");
                    assert_eq!(srgb(lights), color);
                    // A tenth of a degree of hue moves a vivid primary by a few channels once its light is clipped.
                    let back = srgb(lights_of(to_oklab(oklch(color).unwrap()).unwrap()).unwrap());
                    let distance = [
                        back.red.abs_diff(color.red),
                        back.green.abs_diff(color.green),
                        back.blue.abs_diff(color.blue),
                    ];
                    assert!(distance.iter().all(|gap| *gap <= 4), "{color:?} {back:?}");
                }
            }
        }
    }

    #[test]
    fn a_chroma_beyond_the_gamut_is_lowered_until_the_color_fits() {
        // At no lightness a little chroma still leaves a trace of red light, which is a channel at most.
        let faint = lights_of(
            to_oklab(Oklch {
                lightness: 0,
                chroma: 73_000,
                hue: 0,
            })
            .unwrap(),
        )
        .unwrap();
        assert!(faint.x > 0 && faint.x < 200 && faint.y < 0, "{faint:?}");
        let vivid = Oklch {
            lightness: 600_000,
            chroma: MAX_CHROMA,
            hue: 292,
        };
        let vivid_color = fitted(vivid).unwrap();
        assert!(
            vivid_color.red > 200 && vivid_color.green < 80 && vivid_color.blue < 80,
            "{vivid_color:?}"
        );
        let back = oklch(vivid_color).unwrap();
        assert!((580_000..=620_000).contains(&back.lightness), "{back:?}");
        assert!(back.chroma < MAX_CHROMA, "{back:?}");
        let gray = fitted(Oklch {
            lightness: 500_000,
            chroma: 0,
            hue: 2000,
        })
        .unwrap();
        assert_eq!(gray.red, gray.green);
        assert_eq!(gray.green, gray.blue);
        assert_eq!(
            fitted(Oklch {
                lightness: UNIT,
                chroma: 0,
                hue: 0
            }),
            Some(rgb(255, 255, 255))
        );
        let near_black = fitted(Oklch {
            lightness: 0,
            chroma: 100_000,
            hue: 0,
        })
        .unwrap();
        assert!(
            near_black.red <= 1 && near_black.green == 0 && near_black.blue == 0,
            "{near_black:?}"
        );
        assert_eq!(
            fitted(Oklch {
                lightness: 0,
                chroma: 0,
                hue: 0
            }),
            Some(rgb(0, 0, 0))
        );
    }
}
