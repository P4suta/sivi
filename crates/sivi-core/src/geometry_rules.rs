//! Whether a master's regions lie inside the page without overlapping one another.

use crate::length::{Rect, Size};

/// Why a set of regions is not a valid layout, naming the first offender in declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum GeometryError {
    /// The region has no positive width or height.
    Degenerate(usize),
    /// The region leaves the page, or its edges cannot be computed.
    OutsidePage(usize),
    /// The two regions share interior points; the first index is the smaller one.
    Overlap(usize, usize),
}

const fn is_degenerate(region: Rect) -> bool {
    !region.size.width.is_positive() || !region.size.height.is_positive()
}

const fn is_outside(page: Size, region: Rect) -> bool {
    match (region.right(), region.bottom()) {
        (Some(right), Some(bottom)) => {
            region.origin.x.millipoints() < 0
                || region.origin.y.millipoints() < 0
                || right.millipoints() > page.width.millipoints()
                || bottom.millipoints() > page.height.millipoints()
        }
        (None, _) | (_, None) => true,
    }
}

/// Whether two boxes share interior points; boxes that only touch along an edge do not.
#[must_use]
pub const fn overlaps(first: Rect, second: Rect) -> bool {
    match (
        first.right(),
        first.bottom(),
        second.right(),
        second.bottom(),
    ) {
        (Some(first_right), Some(first_bottom), Some(second_right), Some(second_bottom)) => {
            first.origin.x.millipoints() < second_right.millipoints()
                && second.origin.x.millipoints() < first_right.millipoints()
                && first.origin.y.millipoints() < second_bottom.millipoints()
                && second.origin.y.millipoints() < first_bottom.millipoints()
        }
        _ => false,
    }
}

/// Refuses a degenerate region, a region outside the page, or two overlapping regions, in that order of discovery.
///
/// # Errors
///
/// Returns the first offending region in declaration order, or the first overlapping pair.
pub fn validate(page: Size, regions: &[Rect]) -> Result<(), GeometryError> {
    for (index, region) in regions.iter().enumerate() {
        if is_degenerate(*region) {
            return Err(GeometryError::Degenerate(index));
        }
        if is_outside(page, *region) {
            return Err(GeometryError::OutsidePage(index));
        }
    }
    for (first, region) in regions.iter().enumerate() {
        for (offset, other) in regions.iter().skip(first.saturating_add(1)).enumerate() {
            if overlaps(*region, *other) {
                return Err(GeometryError::Overlap(
                    first,
                    first.saturating_add(1).saturating_add(offset),
                ));
            }
        }
    }
    Ok(())
}

/// The properties Kani proves for every admitted input and the tests check over the inputs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{GeometryError, is_degenerate, is_outside, overlaps, validate};
    use crate::length::{Rect, Size};

    pub(super) fn spec_validate(page: Size, regions: &[Rect]) {
        let result = validate(page, regions);
        let any_bad = regions
            .iter()
            .any(|region| is_degenerate(*region) || is_outside(page, *region));
        let any_overlap = regions.iter().enumerate().any(|(first, region)| {
            regions
                .iter()
                .skip(first.saturating_add(1))
                .any(|other| overlaps(*region, *other))
        });
        assert_eq!(result.is_ok(), !any_bad && !any_overlap);
        match result {
            Ok(()) => {}
            Err(GeometryError::Degenerate(index)) => {
                assert!(
                    regions
                        .get(index)
                        .is_some_and(|region| is_degenerate(*region))
                );
            }
            Err(GeometryError::OutsidePage(index)) => {
                assert!(
                    regions
                        .get(index)
                        .is_some_and(|region| is_outside(page, *region))
                );
                assert!(
                    regions
                        .get(index)
                        .is_some_and(|region| !is_degenerate(*region))
                );
            }
            Err(GeometryError::Overlap(first, second)) => {
                assert!(first < second);
                assert!(!any_bad);
                let pair = (regions.get(first), regions.get(second));
                assert!(matches!(pair, (Some(_), Some(_))));
                if let (Some(one), Some(two)) = pair {
                    assert!(overlaps(*one, *two));
                    assert!(overlaps(*two, *one));
                }
            }
        }
    }

    pub(super) fn spec_overlap_symmetric(first: Rect, second: Rect) {
        assert_eq!(overlaps(first, second), overlaps(second, first));
        assert!(!overlaps(first, first) || !is_degenerate(first));
    }

    #[cfg(kani)]
    /// Two to the twentieth: the proof domain in millipoints, written without an operator so no mutant touches it.
    const COORDINATE_BOUND: i64 = 0x10_0000;

    #[cfg(kani)]
    fn any_length() -> crate::length::Length {
        let value: i64 = kani::any();
        kani::assume(value >= -COORDINATE_BOUND && value <= COORDINATE_BOUND);
        crate::length::Length::from_millipoints(value)
    }

    #[cfg(kani)]
    fn any_rect() -> Rect {
        Rect::new(
            crate::length::Point {
                x: any_length(),
                y: any_length(),
            },
            Size {
                width: any_length(),
                height: any_length(),
            },
        )
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(5)]
    fn a_layout_is_valid_exactly_when_no_region_is_degenerate_outside_or_overlapping() {
        let page = Size {
            width: any_length(),
            height: any_length(),
        };
        kani::assume(page.width.millipoints() >= 0);
        kani::assume(page.height.millipoints() >= 0);
        let regions = [any_rect(), any_rect(), any_rect()];
        spec_validate(page, &regions);
        let result = validate(page, &regions);
        kani::cover!(result.is_ok());
        kani::cover!(matches!(result, Err(GeometryError::Overlap(0, 2))));
        kani::cover!(matches!(result, Err(GeometryError::OutsidePage(1))));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn overlap_is_symmetric_and_a_region_overlaps_itself_only_when_it_has_area() {
        let first = any_rect();
        let second = any_rect();
        spec_overlap_symmetric(first, second);
        kani::cover!(overlaps(first, second));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_overlapping_regions_as_a_valid_layout() {
        let unit = |millipoints: i64| crate::length::Length::from_millipoints(millipoints);
        let page = Size {
            width: unit(100),
            height: unit(100),
        };
        let regions = [
            Rect::new(
                crate::length::Point {
                    x: unit(0),
                    y: unit(0),
                },
                Size {
                    width: unit(60),
                    height: unit(60),
                },
            ),
            Rect::new(
                crate::length::Point {
                    x: unit(50),
                    y: unit(50),
                },
                Size {
                    width: unit(40),
                    height: unit(40),
                },
            ),
        ];
        kani::assert(
            validate(page, &regions).is_ok(),
            "NEGATIVE_GEOMETRY_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{GeometryError, overlaps, proofs, validate};
    use crate::length::{Length, Point, Rect, Size};

    const fn unit(millipoints: i64) -> Length {
        Length::from_millipoints(millipoints)
    }

    const fn rect(x: i64, y: i64, width: i64, height: i64) -> Rect {
        Rect::new(
            Point {
                x: unit(x),
                y: unit(y),
            },
            Size {
                width: unit(width),
                height: unit(height),
            },
        )
    }

    const PAGE: Size = Size {
        width: unit(100),
        height: unit(100),
    };

    #[test]
    fn every_listed_layout_meets_the_specification() {
        let samples = [
            rect(0, 0, 50, 50),
            rect(50, 50, 50, 50),
            rect(40, 40, 30, 30),
            rect(0, 0, 0, 10),
            rect(90, 90, 20, 20),
            rect(-1, 0, 10, 10),
            rect(i64::MAX, 0, 1, 1),
        ];
        for first in samples {
            proofs::spec_overlap_symmetric(first, first);
            for second in samples {
                proofs::spec_overlap_symmetric(first, second);
                proofs::spec_validate(PAGE, &[first, second]);
                for third in samples {
                    proofs::spec_validate(PAGE, &[first, second, third]);
                }
            }
        }
        proofs::spec_validate(PAGE, &[]);
    }

    #[test]
    fn each_breach_names_its_region_and_touching_edges_do_not_overlap() {
        assert_eq!(
            validate(PAGE, &[rect(0, 0, 50, 50), rect(50, 0, 50, 50)]),
            Ok(())
        );
        assert!(!overlaps(rect(0, 0, 50, 50), rect(50, 0, 50, 50)));
        assert!(!overlaps(rect(0, 0, 50, 50), rect(0, 50, 50, 50)));
        assert!(!overlaps(rect(0, 50, 50, 50), rect(0, 0, 50, 50)));
        assert!(overlaps(rect(0, 0, 50, 50), rect(0, 49, 50, 50)));
        assert!(overlaps(rect(0, 0, 50, 50), rect(49, 0, 50, 50)));
        assert_eq!(validate(PAGE, &[rect(50, 50, 50, 50)]), Ok(()));
        assert_eq!(
            validate(PAGE, &[rect(50, 50, 51, 50)]),
            Err(GeometryError::OutsidePage(0))
        );
        assert_eq!(
            validate(PAGE, &[rect(50, 50, 50, 51)]),
            Err(GeometryError::OutsidePage(0))
        );
        assert_eq!(
            validate(PAGE, &[rect(0, 0, 50, 50), rect(40, 40, 20, 20)]),
            Err(GeometryError::Overlap(0, 1))
        );
        assert_eq!(
            validate(PAGE, &[rect(0, 0, 50, 50), rect(0, 0, 10, 0)]),
            Err(GeometryError::Degenerate(1))
        );
        assert_eq!(
            validate(PAGE, &[rect(60, 0, 50, 50)]),
            Err(GeometryError::OutsidePage(0))
        );
        assert_eq!(
            validate(PAGE, &[rect(-1, 0, 50, 50)]),
            Err(GeometryError::OutsidePage(0))
        );
        assert_eq!(
            validate(PAGE, &[rect(i64::MAX, 0, 1, 1)]),
            Err(GeometryError::OutsidePage(0))
        );
        assert_eq!(
            validate(
                PAGE,
                &[rect(0, 0, 10, 10), rect(20, 0, 10, 10), rect(5, 5, 10, 10)]
            ),
            Err(GeometryError::Overlap(0, 2))
        );
    }
}
