//! Whether measured content fits the box it was given, and whether a deck's pages match its slides.
//!
//! The admitted inputs are lengths within [`BOUND`] millipoints of zero, which every length the adapter converts satisfies.

use crate::length::Length;

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// The largest magnitude of a length these rules admit: about a billion points.
pub const BOUND: i64 = 1 << 40;

#[cfg(verus_keep_ghost)]
verus! {

/// Whether `measured` exceeds `limit` by at most `epsilon`, as a specification over unbounded integers.
pub open spec fn fits(measured: Length, limit: Length, epsilon: Length) -> bool {
    measured.value() - limit.value() <= epsilon.value()
}

/// Whether the excess of `measured` over `limit` is representable, which `fit` requires.
pub open spec fn admits(measured: Length, limit: Length) -> bool {
    measured.value() - limit.value() >= i64::MIN as int
        && measured.value() - limit.value() <= i64::MAX as int
}

} // verus!

/// Whether a measured extent fits within a limit.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Fit {
    Fits,
    /// The extent exceeds the limit by more than the tolerance; `by` is the full excess.
    Overflows {
        by: Length,
    },
}

/// How a deck's page count relates to its slide count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PageVerdict {
    Matches,
    /// The document has this many pages more than slides, so some slide broke across pages.
    Extra(u32),
    /// The document has this many pages fewer than slides, so some slide produced no page.
    Missing(u32),
}

/// `Fits` when `measured` exceeds `limit` by at most `epsilon`, else the full excess.
///
/// A difference the integers cannot hold counts as the largest excess; the admitted domain never reaches one.
#[must_use]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        admits(measured, limit),
    ensures
        (r == Fit::Fits) == fits(measured, limit, epsilon),
))]
pub const fn fit(measured: Length, limit: Length, epsilon: Length) -> Fit {
    let excess = match measured.0.checked_sub(limit.0) {
        Some(difference) => difference,
        None => i64::MAX,
    };
    if excess > epsilon.0 {
        Fit::Overflows { by: Length(excess) }
    } else {
        Fit::Fits
    }
}

/// How `pages` relates to `slides`.
#[must_use]
pub const fn pages(slides: u32, pages: u32) -> PageVerdict {
    if pages > slides {
        PageVerdict::Extra(pages.saturating_sub(slides))
    } else if pages < slides {
        PageVerdict::Missing(slides.saturating_sub(pages))
    } else {
        PageVerdict::Matches
    }
}

/// The properties Kani proves for every admitted input and the tests check over the inputs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{BOUND, Fit, PageVerdict, fit, pages};
    use crate::length::Length;

    pub(super) const fn admitted(length: Length) -> bool {
        length.millipoints() >= -BOUND && length.millipoints() <= BOUND
    }

    pub(super) fn spec_fit(measured: Length, limit: Length, epsilon: Length) {
        let verdict = fit(measured, limit, epsilon);
        let excess = measured
            .millipoints()
            .checked_sub(limit.millipoints())
            .unwrap_or(i64::MAX);
        let overflows = excess > epsilon.millipoints();
        assert_eq!(verdict == Fit::Fits, !overflows);
        if let Fit::Overflows { by } = verdict {
            assert_eq!(by.millipoints(), excess);
            assert!(by.millipoints() > epsilon.millipoints());
        }
    }

    pub(super) fn spec_monotone(smaller: Length, larger: Length, limit: Length, epsilon: Length) {
        if smaller <= larger && fit(larger, limit, epsilon) == Fit::Fits {
            assert_eq!(fit(smaller, limit, epsilon), Fit::Fits);
        }
    }

    pub(super) fn spec_pages(slides: u32, count: u32) {
        let verdict = pages(slides, count);
        assert_eq!(verdict == PageVerdict::Matches, slides == count);
        match verdict {
            PageVerdict::Matches => {}
            PageVerdict::Extra(extra) => {
                assert!(count > slides);
                assert_eq!(slides.checked_add(extra), Some(count));
            }
            PageVerdict::Missing(missing) => {
                assert!(count < slides);
                assert_eq!(count.checked_add(missing), Some(slides));
            }
        }
    }

    #[cfg(kani)]
    fn any_admitted() -> Length {
        let length = Length::from_millipoints(kani::any());
        kani::assume(admitted(length));
        length
    }

    #[cfg(kani)]
    #[kani::proof]
    fn content_overflows_exactly_when_its_excess_passes_the_tolerance() {
        let measured = any_admitted();
        let limit = any_admitted();
        let epsilon = any_admitted();
        kani::assume(epsilon.millipoints() >= 0);
        spec_fit(measured, limit, epsilon);
        let verdict = fit(measured, limit, epsilon);
        kani::cover!(verdict == Fit::Fits);
        kani::cover!(matches!(verdict, Fit::Overflows { .. }));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn less_content_never_overflows_where_more_content_fits() {
        let smaller = any_admitted();
        let larger = any_admitted();
        let limit = any_admitted();
        let epsilon = any_admitted();
        kani::assume(epsilon.millipoints() >= 0);
        spec_monotone(smaller, larger, limit, epsilon);
        kani::cover!(smaller < larger && fit(larger, limit, epsilon) == Fit::Fits);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn pages_match_slides_or_name_the_exact_difference() {
        let slides: u32 = kani::any();
        let count: u32 = kani::any();
        spec_pages(slides, count);
        kani::cover!(pages(slides, count) == PageVerdict::Matches);
        kani::cover!(matches!(pages(slides, count), PageVerdict::Extra(_)));
        kani::cover!(matches!(pages(slides, count), PageVerdict::Missing(_)));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_overflow_within_the_tolerance_as_fitting() {
        let limit = Length::from_millipoints(100_000);
        let epsilon = Length::from_millipoints(10);
        let measured = Length::from_millipoints(100_020);
        kani::assert(
            fit(measured, limit, epsilon) == Fit::Fits,
            "NEGATIVE_FIT_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Fit, PageVerdict, fit, pages, proofs};
    use crate::length::{EPSILON, Length};

    const SAMPLES: [i64; 9] = [
        -super::BOUND,
        -1_000_000,
        -1,
        0,
        1,
        10,
        11,
        1_000_000,
        super::BOUND,
    ];

    #[test]
    fn every_sample_meets_the_fit_specification() {
        for measured in SAMPLES {
            for limit in SAMPLES {
                for epsilon in [0, 10, 1_000] {
                    let (measured, limit, epsilon) = (
                        Length::from_millipoints(measured),
                        Length::from_millipoints(limit),
                        Length::from_millipoints(epsilon),
                    );
                    assert!(proofs::admitted(measured));
                    proofs::spec_fit(measured, limit, epsilon);
                    for other in SAMPLES {
                        proofs::spec_monotone(
                            measured,
                            Length::from_millipoints(other),
                            limit,
                            epsilon,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_sample_meets_the_page_specification() {
        for slides in [0, 1, 2, 40, u32::MAX] {
            for count in [0, 1, 2, 41, u32::MAX] {
                proofs::spec_pages(slides, count);
            }
        }
    }

    #[test]
    fn an_excess_past_the_tolerance_overflows_by_the_full_excess() {
        let limit = Length::from_points(340).unwrap();
        let required = Length::from_points(392).unwrap();
        assert_eq!(
            fit(required, limit, EPSILON),
            Fit::Overflows {
                by: Length::from_points(52).unwrap()
            }
        );
        assert_eq!(fit(limit, limit, EPSILON), Fit::Fits);
        assert_eq!(
            fit(limit.checked_add(EPSILON).unwrap(), limit, EPSILON),
            Fit::Fits
        );
        assert_eq!(
            fit(
                limit.checked_add(Length::from_millipoints(11)).unwrap(),
                limit,
                EPSILON
            ),
            Fit::Overflows {
                by: Length::from_millipoints(11)
            }
        );
        assert_eq!(pages(3, 3), PageVerdict::Matches);
        assert_eq!(pages(3, 5), PageVerdict::Extra(2));
        assert_eq!(pages(3, 1), PageVerdict::Missing(2));
    }

    #[test]
    fn a_difference_beyond_the_representable_range_never_fits() {
        let largest = Length::from_millipoints(i64::MAX);
        let smallest = Length::from_millipoints(i64::MIN);
        let below_zero = Length::from_millipoints(-1);
        let above_zero = Length::from_millipoints(1);
        assert_eq!(
            fit(largest, below_zero, EPSILON),
            Fit::Overflows { by: largest }
        );
        assert_eq!(
            fit(smallest, above_zero, EPSILON),
            Fit::Overflows { by: largest }
        );
        assert_eq!(fit(smallest, Length::ZERO, EPSILON), Fit::Fits);
        assert_eq!(
            fit(largest, Length::ZERO, EPSILON),
            Fit::Overflows { by: largest }
        );
    }
}
