//! What the fixed-point color engine promises for every color.
//!
//! The light table rises and every channel reads back; every color has a luminance within one;
//! the contrast ratio of any two luminances lies from one to twenty-one and is one for equal luminances;
//! and every color has an Oklab form whose light fits the gamut.

use crate::theme::Thousandths;

/// The contrast of a color with itself, and of black with white, in thousandths.
pub const EQUAL_CONTRAST: Thousandths = Thousandths(1000);
pub const EXTREME_CONTRAST: Thousandths = Thousandths(21_000);

/// Whether `contrast` lies within what the ratio can reach.
#[must_use]
pub const fn is_contrast(contrast: Thousandths) -> bool {
    contrast.0 >= EQUAL_CONTRAST.0 && contrast.0 <= EXTREME_CONTRAST.0
}

/// The properties Kani proves over every channel, every color, and every pair of luminances.
#[cfg(any(test, kani))]
mod proofs {
    use super::{EQUAL_CONTRAST, EXTREME_CONTRAST, is_contrast};
    use crate::color::{self, UNIT};
    use crate::theme::Color;

    /// The light table rises with every channel, and every channel reads back from its light.
    pub(super) fn spec_channel(channel: u8) {
        let light = color::linear(channel);
        assert!((0..=UNIT).contains(&light));
        if let Some(next) = channel.checked_add(1) {
            assert!(light < color::linear(next));
        }
        assert_eq!(color::channel(light), channel);
    }

    /// Every color has a luminance from zero to one.
    pub(super) fn spec_luminance(color: Color) {
        let luminance = color::luminance(color);
        assert!(luminance.is_some(), "every color has a luminance");
        let Some(luminance) = luminance else {
            return;
        };
        assert!((0..=UNIT).contains(&luminance));
    }

    /// The ratio of a lighter luminance over a darker one is defined and lies from one to twenty-one, and is one for equal luminances.
    pub(super) fn spec_ratio(lighter: u32, darker: u32) {
        let ratio = color::ratio(lighter, darker);
        assert!(ratio.is_some(), "the ratio of two luminances is defined");
        let Some(ratio) = ratio else {
            return;
        };
        assert!(is_contrast(ratio));
        assert!(ratio >= EQUAL_CONTRAST && ratio <= EXTREME_CONTRAST);
        if lighter == darker {
            assert_eq!(ratio, EQUAL_CONTRAST);
        }
    }

    /// The contrast of two colors is the same in either order and one for a color on itself, which the tests hold over samples.
    #[cfg(test)]
    pub(super) fn spec_contrast(first: Color, second: Color) {
        assert_eq!(
            color::contrast(first, second),
            color::contrast(second, first)
        );
        assert_eq!(color::contrast(first, first), Some(EQUAL_CONTRAST));
        assert!(color::contrast(first, second).is_some_and(is_contrast));
    }

    /// Every color has an Oklab form within lightness zero to one whose light fits the gamut, which the tests hold since its root searches lie beyond the solver's budget.
    #[cfg(test)]
    pub(super) fn spec_oklab(color: Color) {
        let lab = color::oklab(color);
        assert!(lab.is_some(), "every color has an Oklab form");
        let Some(lab) = lab else {
            return;
        };
        assert!((0..=UNIT).contains(&lab.lightness));
        let lights = color::lights_of(lab);
        assert!(lights.is_some(), "every Oklab form has light");
        let Some(lights) = lights else {
            return;
        };
        assert!(color::in_gamut(lights));
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(10)]
    fn every_channel_rises_and_reads_back() {
        let channel: u8 = kani::any();
        spec_channel(channel);
        kani::cover!(channel == 0, "the darkest channel");
        kani::cover!(channel == u8::MAX, "the brightest channel");
    }

    #[cfg(kani)]
    #[kani::proof]
    fn every_color_has_a_luminance_within_one() {
        let color = Color {
            red: kani::any(),
            green: kani::any(),
            blue: kani::any(),
        };
        spec_luminance(color);
        kani::cover!(color::luminance(color) == Some(UNIT), "white");
        kani::cover!(color::luminance(color) == Some(0), "black");
    }

    #[cfg(kani)]
    #[kani::proof]
    fn every_ratio_of_luminances_lies_from_one_to_twenty_one() {
        let lighter: u32 = kani::any();
        let darker: u32 = kani::any();
        kani::assume(darker <= lighter);
        kani::assume(i64::from(lighter) <= UNIT);
        spec_ratio(lighter, darker);
        kani::cover!(
            color::ratio(lighter, darker) == Some(EXTREME_CONTRAST),
            "black on white"
        );
        kani::cover!(
            color::ratio(lighter, darker) == Some(EQUAL_CONTRAST),
            "a luminance on itself"
        );
    }

    /// Black on white must be the widest contrast; this probe claims it falls short and must fail.
    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_contrast_below_the_extreme_for_black_on_white() {
        let black = Color {
            red: 0,
            green: 0,
            blue: 0,
        };
        let white = Color {
            red: u8::MAX,
            green: u8::MAX,
            blue: u8::MAX,
        };
        kani::assert(
            color::contrast(black, white).is_some_and(|ratio| ratio.0 < EXTREME_CONTRAST.0),
            "NEGATIVE_COLOR_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::proofs::{spec_channel, spec_contrast, spec_luminance, spec_oklab, spec_ratio};
    use super::{EQUAL_CONTRAST, EXTREME_CONTRAST, is_contrast};
    use crate::color::UNIT;
    use crate::theme::{Color, Thousandths};

    #[test]
    fn every_channel_and_every_gray_meet_the_specification() {
        for level in 0..=u8::MAX {
            spec_channel(level);
            let gray = Color {
                red: level,
                green: level,
                blue: level,
            };
            spec_luminance(gray);
            spec_oklab(gray);
        }
    }

    #[test]
    fn sampled_pairs_meet_the_contrast_specification() {
        let steps = [0, 40, 128, 200, 255];
        for first in steps {
            for second in steps {
                for third in steps {
                    let one = Color {
                        red: first,
                        green: second,
                        blue: third,
                    };
                    let other = Color {
                        red: third,
                        green: first,
                        blue: second,
                    };
                    spec_luminance(one);
                    spec_contrast(one, other);
                    spec_oklab(one);
                }
            }
        }
        let luminances = [0, 1, 50_000, 212_600, 500_000, 999_999, 1_000_000];
        for lighter in luminances {
            for darker in luminances.into_iter().filter(|darker| *darker <= lighter) {
                spec_ratio(lighter, darker);
            }
        }
        assert_eq!(u32::try_from(UNIT).ok(), Some(1_000_000));
        assert!(is_contrast(EQUAL_CONTRAST));
        assert!(is_contrast(EXTREME_CONTRAST));
        assert!(!is_contrast(Thousandths(999)));
        assert!(!is_contrast(Thousandths(21_001)));
    }
}
