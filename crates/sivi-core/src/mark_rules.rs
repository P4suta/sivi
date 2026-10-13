//! How translucent a mark's decoration is: the alpha of its color, decided from the opacity the theme states.
//!
//! The theme states an opacity in thousandths, from clear to solid; the color Typst paints carries an alpha byte.
//! The alpha is the nearest byte, so a theme's `0.5` paints as `0x80` and only `1` paints solid.

use crate::theme::Thousandths;

/// The opacity of a solid color, in thousandths.
pub const SOLID: Thousandths = Thousandths::ONE;
/// The alpha of a solid color.
pub const OPAQUE: u8 = u8::MAX;
/// Half of one thousandth, which rounds a quotient to the nearest.
const HALF: i64 = 500;

/// The alpha byte of `opacity`, rounded to the nearest, or `None` when the opacity leaves clear to solid.
#[must_use]
pub fn alpha(opacity: Thousandths) -> Option<u8> {
    if opacity.0 < 0 || opacity.0 > SOLID.0 {
        return None;
    }
    let scaled = opacity.0.checked_mul(i64::from(OPAQUE))?;
    let rounded = scaled.checked_add(HALF)?.checked_div(SOLID.0)?;
    u8::try_from(rounded).ok()
}

/// The properties Kani proves over every opacity.
#[cfg(any(test, kani))]
mod proofs {
    use super::{OPAQUE, SOLID, alpha};
    use crate::theme::Thousandths;

    /// The decision restated: an alpha exactly within clear to solid, nearest to the exact scaling, and in order.
    pub(super) fn spec(opacity: i64, other: i64) {
        let found = alpha(Thousandths(opacity));
        assert_eq!(found.is_some(), (0..=SOLID.0).contains(&opacity));
        let Some(byte) = found else {
            return;
        };
        let exact = opacity.checked_mul(i64::from(OPAQUE));
        let painted = i64::from(byte).checked_mul(SOLID.0);
        assert!(exact.is_some() && painted.is_some(), "the scalings fit");
        if let (Some(exact), Some(painted)) = (exact, painted) {
            // Twice the distance from the exact scaling stays within one thousandth, so no nearer byte exists.
            let distance = exact.abs_diff(painted);
            assert!(
                distance
                    .checked_add(distance)
                    .is_some_and(|twice| twice <= SOLID.0.unsigned_abs())
            );
        }
        if opacity <= other
            && let Some(other_byte) = alpha(Thousandths(other))
        {
            assert!(byte <= other_byte, "the order of opacities is kept");
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    fn the_alpha_is_the_nearest_byte_within_clear_to_solid_and_keeps_the_order() {
        let opacity: i64 = kani::any();
        let other: i64 = kani::any();
        spec(opacity, other);
        kani::assert(alpha(Thousandths(0)) == Some(0), "clear paints nothing");
        kani::assert(alpha(SOLID) == Some(OPAQUE), "solid paints opaque");
        kani::cover!(
            alpha(Thousandths(opacity)).is_none(),
            "an opacity outside the range"
        );
        kani::cover!(
            alpha(Thousandths(opacity)).is_some_and(|byte| byte != 0 && byte != OPAQUE),
            "a translucent alpha"
        );
    }

    /// An opacity above solid must have no alpha; this probe claims it does and must fail.
    #[cfg(kani)]
    #[kani::proof]
    fn rejects_an_opacity_above_solid() {
        kani::assert(
            alpha(Thousandths(SOLID.0.saturating_add(1))).is_some(),
            "NEGATIVE_MARK_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::proofs::spec;
    use super::{OPAQUE, SOLID, alpha};
    use crate::theme::Thousandths;

    #[test]
    fn the_alpha_follows_the_opacity_to_the_nearest_byte() {
        assert_eq!(alpha(Thousandths(0)), Some(0));
        assert_eq!(alpha(SOLID), Some(OPAQUE));
        assert_eq!(alpha(Thousandths(500)), Some(128));
        assert_eq!(alpha(Thousandths(450)), Some(115));
        assert_eq!(alpha(Thousandths(1)), Some(0));
        assert_eq!(alpha(Thousandths(2)), Some(1));
        assert_eq!(alpha(Thousandths(999)), Some(255));
        assert_eq!(alpha(Thousandths(-1)), None);
        assert_eq!(alpha(Thousandths(1001)), None);
        assert_eq!(alpha(Thousandths(i64::MAX)), None);
        assert_eq!(alpha(Thousandths(i64::MIN)), None);
    }

    #[test]
    fn the_specification_holds_over_every_opacity_and_its_neighbors() {
        for opacity in -2..=1002 {
            for other in [-1, 0, 1, 499, 500, 501, 999, 1000, 1001, opacity] {
                spec(opacity, other);
            }
        }
    }
}
