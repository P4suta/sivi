//! Which region a laid-out item belongs to, and which part of a region's content a height falls in.

use crate::length::{Length, Point, Rect};

/// The first region whose box contains `point`, in declaration order.
#[must_use]
pub fn region_of(regions: &[Rect], point: Point) -> Option<usize> {
    regions.iter().position(|region| region.contains(point))
}

/// The first part whose cumulative height reaches past `offset` from the region's top.
#[must_use]
pub fn part_at(prefixes: &[Length], offset: Length) -> Option<usize> {
    prefixes.iter().position(|prefix| offset < *prefix)
}

/// Whether `outer` holds every edge of `inner`, each within `tolerance`; a box whose far edges overflow holds nothing and lies in nothing.
#[must_use]
pub fn holds(outer: Rect, inner: Rect, tolerance: Length) -> bool {
    let Some(outer_right) = outer.right() else {
        return false;
    };
    let Some(outer_bottom) = outer.bottom() else {
        return false;
    };
    let Some(inner_right) = inner.right() else {
        return false;
    };
    let Some(inner_bottom) = inner.bottom() else {
        return false;
    };
    let within = |edge: Length, bound: Length| edge.0.saturating_sub(bound.0) <= tolerance.0;
    within(outer.origin.x, inner.origin.x)
        && within(outer.origin.y, inner.origin.y)
        && within(inner_right, outer_right)
        && within(inner_bottom, outer_bottom)
}

/// Whether some backdrop holds `item`, within `tolerance`, so the item is the backdrop's own and belongs to no region.
#[must_use]
pub fn covered(backdrops: &[Rect], item: Rect, tolerance: Length) -> bool {
    backdrops
        .iter()
        .any(|backdrop| holds(*backdrop, item, tolerance))
}

/// The properties Kani proves for every admitted input and the tests check over the inputs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{covered, holds, part_at, region_of};
    use crate::length::{Length, Point, Rect};

    /// `outer` holds `inner` exactly when, over wide integers, each edge of `inner` lies at most `tolerance` past the matching edge of `outer`, and both boxes have far edges.
    pub(super) fn spec_holds(outer: Rect, inner: Rect, tolerance: Length) -> bool {
        let far = |rect: Rect| {
            let right = i128::from(rect.origin.x.0).wrapping_add(i128::from(rect.size.width.0));
            let bottom = i128::from(rect.origin.y.0).wrapping_add(i128::from(rect.size.height.0));
            (rect.right().is_some() && rect.bottom().is_some()).then_some((right, bottom))
        };
        let (Some((outer_right, outer_bottom)), Some((inner_right, inner_bottom))) =
            (far(outer), far(inner))
        else {
            return false;
        };
        let slack = i128::from(tolerance.0);
        i128::from(outer.origin.x.0).wrapping_sub(i128::from(inner.origin.x.0)) <= slack
            && i128::from(outer.origin.y.0).wrapping_sub(i128::from(inner.origin.y.0)) <= slack
            && inner_right.wrapping_sub(outer_right) <= slack
            && inner_bottom.wrapping_sub(outer_bottom) <= slack
    }

    pub(super) fn spec_covered(backdrops: &[Rect], item: Rect, tolerance: Length) -> bool {
        let actual = covered(backdrops, item, tolerance);
        let mut expected = false;
        for backdrop in backdrops {
            let contained = spec_holds(*backdrop, item, tolerance);
            assert_eq!(holds(*backdrop, item, tolerance), contained);
            expected |= contained;
        }
        assert_eq!(actual, expected);
        actual
    }

    pub(super) fn spec_region_of(regions: &[Rect], point: Point) {
        match region_of(regions, point) {
            Some(index) => {
                assert!(
                    regions
                        .get(index)
                        .is_some_and(|region| region.contains(point))
                );
                assert!(
                    regions
                        .iter()
                        .take(index)
                        .all(|region| !region.contains(point))
                );
            }
            None => assert!(regions.iter().all(|region| !region.contains(point))),
        }
    }

    pub(super) fn spec_part_at(prefixes: &[Length], offset: Length) {
        match part_at(prefixes, offset) {
            Some(index) => {
                assert!(prefixes.get(index).is_some_and(|prefix| offset < *prefix));
                assert!(prefixes.iter().take(index).all(|prefix| offset >= *prefix));
            }
            None => assert!(prefixes.iter().all(|prefix| offset >= *prefix)),
        }
    }

    #[cfg(kani)]
    /// Two to the thirtieth: the proof domain in millipoints, written without an operator so no mutant touches it.
    const BOUND: i64 = 0x4000_0000;

    #[cfg(kani)]
    fn any_length() -> Length {
        let value: i64 = kani::any();
        kani::assume(value >= -BOUND && value <= BOUND);
        Length::from_millipoints(value)
    }

    #[cfg(kani)]
    fn any_rect() -> Rect {
        Rect::new(
            Point {
                x: any_length(),
                y: any_length(),
            },
            crate::length::Size {
                width: any_length(),
                height: any_length(),
            },
        )
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(5)]
    fn a_point_belongs_to_the_first_region_that_contains_it() {
        let regions = [any_rect(), any_rect(), any_rect()];
        let count: usize = kani::any();
        kani::assume(count <= regions.len());
        let point = Point {
            x: any_length(),
            y: any_length(),
        };
        spec_region_of(&regions[..count], point);
        kani::cover!(region_of(&regions[..count], point) == Some(1));
        kani::cover!(region_of(&regions[..count], point).is_none());
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(5)]
    fn a_height_falls_in_the_first_part_whose_prefix_passes_it() {
        let prefixes = [any_length(), any_length(), any_length()];
        let count: usize = kani::any();
        kani::assume(count <= prefixes.len());
        let offset = any_length();
        spec_part_at(&prefixes[..count], offset);
        kani::cover!(part_at(&prefixes[..count], offset) == Some(2));
        kani::cover!(part_at(&prefixes[..count], offset).is_none());
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(5)]
    fn an_item_is_covered_exactly_when_a_backdrop_holds_each_of_its_edges() {
        let backdrops = [any_rect(), any_rect(), any_rect()];
        let count: usize = kani::any();
        kani::assume(count <= backdrops.len());
        let item = any_rect();
        let tolerance = any_length();
        kani::assume(tolerance.0 >= 0);
        let covered = spec_covered(&backdrops[..count], item, tolerance);
        kani::cover!(covered);
        kani::cover!(!covered);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_attributing_a_height_to_a_later_part() {
        let prefixes = [
            Length::from_millipoints(10),
            Length::from_millipoints(20),
            Length::from_millipoints(30),
        ];
        kani::assert(
            part_at(&prefixes, Length::from_millipoints(5)) == Some(1),
            "NEGATIVE_ATTRIBUTE_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{covered, holds, part_at, proofs, region_of};
    use crate::length::{Length, Point, Rect, Size};

    #[test]
    fn a_backdrop_holds_an_item_within_the_tolerance_and_an_overflowing_box_holds_nothing() {
        let backdrops = [rect(0, 0, 100, 100), rect(200, 0, 50, 50)];
        let samples = [
            rect(0, 0, 100, 100),
            rect(10, 10, 20, 20),
            rect(-1, 0, 10, 10),
            rect(0, -1, 10, 10),
            rect(95, 0, 10, 10),
            rect(0, 95, 10, 10),
            rect(200, 0, 50, 50),
            rect(210, 10, 50, 50),
            rect(i64::MAX, 0, 1, 1),
            rect(0, 0, i64::MAX, 1),
        ];
        for item in samples {
            for tolerance in [0, 1, 5, 10] {
                for count in 0..=backdrops.len() {
                    proofs::spec_covered(&backdrops[..count], item, unit(tolerance));
                }
            }
        }
        assert!(covered(&backdrops, rect(10, 10, 20, 20), unit(0)));
        assert!(!covered(&backdrops, rect(-1, 0, 10, 10), unit(0)));
        assert!(covered(&backdrops, rect(-1, 0, 10, 10), unit(1)));
        assert!(covered(&backdrops, rect(95, 0, 10, 10), unit(5)));
        assert!(!covered(&backdrops, rect(95, 0, 10, 10), unit(4)));
        assert!(!covered(&backdrops, rect(210, 10, 50, 50), unit(9)));
        assert!(covered(&backdrops, rect(210, 10, 50, 50), unit(10)));
        assert!(!covered(&[], rect(10, 10, 20, 20), unit(100)));
        assert!(holds(rect(0, 0, i64::MAX, 1), rect(0, 0, 1, 1), unit(0)));
        assert!(!holds(rect(1, 0, i64::MAX, 1), rect(1, 0, 1, 1), unit(0)));
        assert!(!holds(
            rect(0, 0, 100, 100),
            rect(1, 0, i64::MAX, 1),
            unit(0)
        ));
        assert!(!holds(rect(0, 1, 1, i64::MAX), rect(0, 1, 1, 1), unit(0)));
        assert!(!holds(
            rect(0, 0, 100, 100),
            rect(0, 1, 1, i64::MAX),
            unit(0)
        ));
    }

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

    #[test]
    fn every_listed_input_meets_the_specification() {
        let regions = [
            rect(0, 0, 50, 50),
            rect(50, 0, 50, 50),
            rect(25, 25, 50, 50),
        ];
        for x in [-1, 0, 25, 50, 75, 100, 101] {
            for y in [-1, 0, 25, 50, 75, 100, 101] {
                let point = Point {
                    x: unit(x),
                    y: unit(y),
                };
                for count in 0..=regions.len() {
                    proofs::spec_region_of(&regions[..count], point);
                }
            }
        }
        let prefixes = [unit(10), unit(20), unit(20), unit(30)];
        for offset in [-1, 0, 9, 10, 19, 20, 29, 30, 31] {
            for count in 0..=prefixes.len() {
                proofs::spec_part_at(&prefixes[..count], unit(offset));
            }
        }
    }

    #[test]
    fn the_first_match_wins() {
        let regions = [rect(0, 0, 50, 50), rect(25, 25, 50, 50)];
        assert_eq!(
            region_of(
                &regions,
                Point {
                    x: unit(30),
                    y: unit(30)
                }
            ),
            Some(0)
        );
        assert_eq!(
            region_of(
                &regions,
                Point {
                    x: unit(60),
                    y: unit(60)
                }
            ),
            Some(1)
        );
        assert_eq!(
            region_of(
                &regions,
                Point {
                    x: unit(90),
                    y: unit(90)
                }
            ),
            None
        );
        let prefixes = [unit(10), unit(20), unit(30)];
        assert_eq!(part_at(&prefixes, unit(5)), Some(0));
        assert_eq!(part_at(&prefixes, unit(10)), Some(1));
        assert_eq!(part_at(&prefixes, unit(29)), Some(2));
        assert_eq!(part_at(&prefixes, unit(30)), None);
    }
}
