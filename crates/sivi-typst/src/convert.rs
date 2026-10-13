//! The one place a Typst floating-point length becomes a core millipoint length.

use sivi_core::length::{Length, Point, Rect, Size};
use std::fmt;
use typst::layout::{Abs, Point as TypstPoint, Rect as TypstRect};

/// The largest magnitude in points the core represents, far beyond any page.
pub const MAX_POINTS: f64 = 1_000_000_000.0;

const MILLIPOINTS_PER_POINT_F64: f64 = 1000.0;

/// A length that is not finite or lies beyond [`MAX_POINTS`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unrepresentable {
    pub points: f64,
}

impl fmt::Display for Unrepresentable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the length {}pt cannot be represented",
            self.points
        )
    }
}

impl std::error::Error for Unrepresentable {}

/// `abs` rounded to the nearest millipoint.
///
/// # Errors
///
/// Returns the offending value when it is not finite or lies beyond [`MAX_POINTS`].
pub fn length(abs: Abs) -> Result<Length, Unrepresentable> {
    let points = abs.to_pt();
    if !points.is_finite() || points.abs() > MAX_POINTS {
        return Err(Unrepresentable { points });
    }
    let millipoints = (points * MILLIPOINTS_PER_POINT_F64).round();
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "the value is finite and within MAX_POINTS, so it fits an i64 exactly after rounding"
    )]
    let whole = millipoints as i64;
    Ok(Length::from_millipoints(whole))
}

/// `point` with both coordinates rounded to millipoints.
///
/// # Errors
///
/// Returns the first coordinate that cannot be represented.
pub fn point(point: TypstPoint) -> Result<Point, Unrepresentable> {
    Ok(Point {
        x: length(point.x)?,
        y: length(point.y)?,
    })
}

/// `rect` as an origin and a size, rounded to millipoints.
///
/// # Errors
///
/// Returns the first coordinate that cannot be represented.
pub fn rect(rect: TypstRect) -> Result<Rect, Unrepresentable> {
    let min = point(rect.min)?;
    let max = point(rect.max)?;
    Ok(Rect::new(
        min,
        Size {
            width: max.x.saturating_sub(min.x),
            height: max.y.saturating_sub(min.y),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{MAX_POINTS, MILLIPOINTS_PER_POINT_F64, length, rect};
    use sivi_core::length::{Length, MILLIPOINTS_PER_POINT};
    use typst::layout::{Abs, Point, Rect};

    #[test]
    fn the_float_scale_matches_the_core_scale() {
        assert_eq!(
            f64::from(u32::try_from(MILLIPOINTS_PER_POINT).unwrap()),
            MILLIPOINTS_PER_POINT_F64
        );
    }

    #[test]
    fn lengths_round_to_the_nearest_millipoint() {
        assert_eq!(
            length(Abs::pt(151.118_4)).unwrap(),
            Length::from_millipoints(151_118)
        );
        assert_eq!(
            length(Abs::pt(-0.0105)).unwrap(),
            Length::from_millipoints(-11)
        );
        assert_eq!(length(Abs::zero()).unwrap(), Length::ZERO);
        length(Abs::inf()).unwrap_err();
        length(Abs::pt(f64::INFINITY)).unwrap_err();
        length(Abs::pt(MAX_POINTS * 2.0)).unwrap_err();
    }

    #[test]
    fn rects_become_an_origin_and_a_size() {
        let converted = rect(Rect::new(
            Point::new(Abs::pt(10.0), Abs::pt(20.0)),
            Point::new(Abs::pt(40.0), Abs::pt(25.5)),
        ))
        .unwrap();
        assert_eq!(converted.origin.x, Length::from_points(10).unwrap());
        assert_eq!(converted.size.width, Length::from_points(30).unwrap());
        assert_eq!(converted.size.height, Length::from_millipoints(5_500));
    }
}
