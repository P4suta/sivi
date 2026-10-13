//! Lengths on the page as whole millipoints, so every comparison is exact on every host.

use core::fmt;

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// How many millipoints make one typographic point.
pub const MILLIPOINTS_PER_POINT: i64 = 1000;

/// The tolerance within which two lengths count as equal: one hundredth of a point.
pub const EPSILON: Length = Length::from_millipoints(10);

const DECIMALS: usize = 3;

/// A signed length in millipoints.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Length(pub i64);

#[cfg(verus_keep_ghost)]
verus! {

impl Length {
    /// The value in millipoints as an unbounded integer, for specifications.
    pub open spec fn value(self) -> int {
        self.0 as int
    }
}

} // verus!

impl Length {
    pub const ZERO: Self = Self(0);

    /// The length of `millipoints` thousandths of a point.
    #[must_use]
    pub const fn from_millipoints(millipoints: i64) -> Self {
        Self(millipoints)
    }

    /// The length of `points` whole points, or `None` when it overflows.
    #[must_use]
    pub const fn from_points(points: i64) -> Option<Self> {
        match points.checked_mul(MILLIPOINTS_PER_POINT) {
            Some(millipoints) => Some(Self(millipoints)),
            None => None,
        }
    }

    #[must_use]
    pub const fn millipoints(self) -> i64 {
        self.0
    }

    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    #[must_use]
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(sum) => Some(Self(sum)),
            None => None,
        }
    }

    #[must_use]
    pub const fn checked_sub(self, other: Self) -> Option<Self> {
        match self.0.checked_sub(other.0) {
            Some(difference) => Some(Self(difference)),
            None => None,
        }
    }

    /// `self - other`, clamped at the representable extremes.
    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }

    #[must_use]
    pub const fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }
}

impl fmt::Display for Length {
    /// Points with three decimals, such as `151.118pt` or `-0.010pt`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let magnitude = self.0.unsigned_abs();
        let per_point = MILLIPOINTS_PER_POINT.unsigned_abs();
        let (whole, fraction) = match (
            magnitude.checked_div(per_point),
            magnitude.checked_rem(per_point),
        ) {
            (Some(whole), Some(fraction)) => (whole, fraction),
            (None, _) | (_, None) => (0, 0),
        };
        write!(formatter, "{sign}{whole}.{fraction:0DECIMALS$}pt")
    }
}

/// A position on a page, from its top-left corner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Point {
    pub x: Length,
    pub y: Length,
}

/// The extent of a box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Size {
    pub width: Length,
    pub height: Length,
}

/// An axis-aligned box on a page.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    /// The box from `origin` with `size`.
    #[must_use]
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// The right edge, or `None` when it overflows.
    #[must_use]
    pub const fn right(self) -> Option<Length> {
        self.origin.x.checked_add(self.size.width)
    }

    /// The bottom edge, or `None` when it overflows.
    #[must_use]
    pub const fn bottom(self) -> Option<Length> {
        self.origin.y.checked_add(self.size.height)
    }

    /// Whether `point` lies inside or on the edge of the box; an overflowing box contains nothing.
    #[must_use]
    pub const fn contains(self, point: Point) -> bool {
        match (self.right(), self.bottom()) {
            (Some(right), Some(bottom)) => {
                point.x.0 >= self.origin.x.0
                    && point.x.0 <= right.0
                    && point.y.0 >= self.origin.y.0
                    && point.y.0 <= bottom.0
            }
            (None, _) | (_, None) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::{EPSILON, Length, MILLIPOINTS_PER_POINT, Point, Rect, Size};
    use alloc::format;

    #[test]
    fn lengths_display_as_points_with_three_decimals() {
        assert_eq!(
            format!("{}", Length::from_millipoints(151_118)),
            "151.118pt"
        );
        assert_eq!(format!("{}", Length::from_millipoints(-10)), "-0.010pt");
        assert_eq!(format!("{}", Length::ZERO), "0.000pt");
        assert_eq!(format!("{EPSILON}"), "0.010pt");
        assert_eq!(
            Length::from_points(2).unwrap().millipoints(),
            2 * MILLIPOINTS_PER_POINT
        );
        assert_eq!(Length::from_points(i64::MAX), None);
    }

    #[test]
    fn arithmetic_is_checked() {
        let max = Length::from_millipoints(i64::MAX);
        assert_eq!(max.checked_add(EPSILON), None);
        assert_eq!(
            Length::ZERO.checked_sub(EPSILON),
            Some(Length::from_millipoints(-10))
        );
        assert_eq!(
            Length::from_millipoints(i64::MIN)
                .saturating_sub(EPSILON)
                .millipoints(),
            i64::MIN
        );
        assert_eq!(EPSILON.max(Length::ZERO), EPSILON);
        assert_eq!(EPSILON.min(Length::ZERO), Length::ZERO);
        assert!(EPSILON.is_positive());
        assert!(!Length::ZERO.is_positive());
    }

    #[test]
    fn a_box_contains_its_edges_and_an_overflowing_box_contains_nothing() {
        let unit = |millipoints: i64| Length::from_millipoints(millipoints);
        let rect = Rect::new(
            Point {
                x: unit(10),
                y: unit(20),
            },
            Size {
                width: unit(30),
                height: unit(40),
            },
        );
        assert_eq!(rect.right(), Some(unit(40)));
        assert_eq!(rect.bottom(), Some(unit(60)));
        assert!(rect.contains(Point {
            x: unit(10),
            y: unit(20)
        }));
        assert!(rect.contains(Point {
            x: unit(40),
            y: unit(60)
        }));
        assert!(!rect.contains(Point {
            x: unit(41),
            y: unit(60)
        }));
        assert!(!rect.contains(Point {
            x: unit(9),
            y: unit(20)
        }));
        let overflowing = Rect::new(
            Point {
                x: Length::from_millipoints(i64::MAX),
                y: Length::ZERO,
            },
            Size {
                width: unit(1),
                height: unit(1),
            },
        );
        assert_eq!(overflowing.right(), None);
        assert!(!overflowing.contains(Point::default()));
    }
}
