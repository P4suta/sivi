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

/// The properties Kani proves for every admitted input and the tests check over the inputs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{part_at, region_of};
    use crate::length::{Length, Point, Rect};

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
    use super::{part_at, proofs, region_of};
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
