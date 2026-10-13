//! The marker protocol between the emitted Typst source and the adapter that reads the layout back.
//!
//! The emitter writes one slide marker per slide and one measurement marker per bound region.
//! The adapter queries each label and reads the dictionary the marker carries.
//! Both sides spell every label and key through this module, so neither can drift alone.

use alloc::string::String;
use core::fmt;

/// The position of a slide in its deck, counted from zero.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct SlideIndex(pub u32);

/// The position of a bound region in its slide, counted from zero.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct RegionIndex(pub u32);

/// The label of a slide marker, `sivi-slide-<slide>`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SlideLabel(String);

/// The label of a measurement marker, `sivi-m-<slide>-<region>`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MeasureLabel(String);

const SLIDE_PREFIX: &str = "sivi-slide-";
const MEASURE_PREFIX: &str = "sivi-m-";

/// The key of the slide index in a slide marker's dictionary.
pub const SLIDE_KEY: &str = "slide";

/// The key of the required height in a measurement marker's dictionary.
pub const REQUIRED_KEY: &str = "required";

/// The key of the cumulative heights after each split point in a measurement marker's dictionary.
pub const PREFIXES_KEY: &str = "prefixes";

/// The key of the natural widths of unbreakable parts in a measurement marker's dictionary.
pub const WIDTHS_KEY: &str = "widths";

impl SlideLabel {
    #[must_use]
    pub fn new(slide: SlideIndex) -> Self {
        Self(alloc::format!("{SLIDE_PREFIX}{}", slide.0))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl MeasureLabel {
    #[must_use]
    pub fn new(slide: SlideIndex, region: RegionIndex) -> Self {
        Self(alloc::format!("{MEASURE_PREFIX}{}-{}", slide.0, region.0))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SlideLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for MeasureLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{MeasureLabel, RegionIndex, SlideIndex, SlideLabel};

    #[test]
    fn labels_spell_their_indices() {
        assert_eq!(SlideLabel::new(SlideIndex(7)).as_str(), "sivi-slide-7");
        assert_eq!(
            MeasureLabel::new(SlideIndex(7), RegionIndex(2)).as_str(),
            "sivi-m-7-2"
        );
        assert_eq!(
            alloc::format!("{}", SlideLabel::new(SlideIndex(0))),
            "sivi-slide-0"
        );
        assert_eq!(
            alloc::format!("{}", MeasureLabel::new(SlideIndex(0), RegionIndex(0))),
            "sivi-m-0-0"
        );
    }
}
