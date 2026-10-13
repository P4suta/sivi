//! What the Typst adapter reports back about a compiled deck, as values the report rules decide on.

use crate::length::{Length, Rect};
use crate::protocol::{RegionIndex, SlideIndex};
use alloc::vec::Vec;

/// A page of the compiled document, counted from one.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct PageNumber(pub u32);

/// How many pages the compiled document has.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct PageCount(pub u32);

/// What the emitter placed on the page and expects the adapter to read back.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Expected {
    /// How many slides the deck has, each on its own page.
    pub slides: u32,
    pub regions: Vec<ExpectedRegion>,
}

/// One bound region the emitter placed, with the box it may fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExpectedRegion {
    pub slide: SlideIndex,
    pub region: RegionIndex,
    /// The box of the region on its page.
    pub bounds: Rect,
    /// How many split points the content has, which is how many prefixes the marker carries.
    pub split_points: u32,
}

/// Cumulative heights after each split point of a region's content, which never decrease.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Prefixes(Vec<Length>);

/// A prefix list that decreases somewhere, which no layout produces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotMonotonic {
    /// The index of the first prefix below its predecessor.
    pub index: usize,
}

impl Prefixes {
    /// The prefixes of `heights`, which must never decrease.
    ///
    /// # Errors
    ///
    /// Returns the index of the first prefix below its predecessor.
    pub fn new(heights: Vec<Length>) -> Result<Self, NotMonotonic> {
        let mut previous = Length::from_millipoints(i64::MIN);
        for (index, height) in heights.iter().enumerate() {
            if *height < previous {
                return Err(NotMonotonic { index });
            }
            previous = *height;
        }
        Ok(Self(heights))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Length] {
        &self.0
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
}

/// What the adapter measured for one bound region.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RegionMeasure {
    pub slide: SlideIndex,
    pub region: RegionIndex,
    /// The height the content needs at the region's width.
    pub required: Length,
    /// The cumulative heights after each split point, for the split suggestion.
    pub prefixes: Prefixes,
    /// The natural widths of the parts that never wrap, such as code lines, with the index of each part.
    pub natural_widths: Vec<(u32, Length)>,
}

/// What kind of laid-out item a leaf is.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ItemKind {
    Text,
    Shape,
    Image,
}

/// A laid-out item with its box in page coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Leaf {
    pub page: PageNumber,
    pub kind: ItemKind,
    pub bounds: Rect,
}

/// A character the bundled and theme fonts cannot show.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MissingGlyph {
    pub page: PageNumber,
    pub character: char,
}

/// Everything the adapter measured in one compiled deck.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Measurement {
    pub regions: Vec<RegionMeasure>,
    pub pages: PageCount,
    /// The page each slide marker landed on.
    pub slide_pages: Vec<(SlideIndex, PageNumber)>,
    /// Every text, shape, and image item of every page.
    pub leaves: Vec<Leaf>,
    pub missing_glyphs: Vec<MissingGlyph>,
}

#[cfg(test)]
mod tests {
    use super::{NotMonotonic, Prefixes};
    use crate::length::Length;
    use alloc::vec;

    #[test]
    fn prefixes_never_decrease() {
        let unit = |millipoints: i64| Length::from_millipoints(millipoints);
        let prefixes = Prefixes::new(vec![unit(10), unit(10), unit(30)]).unwrap();
        assert_eq!(prefixes.len(), 3);
        assert!(!prefixes.is_empty());
        assert_eq!(prefixes.as_slice()[2], unit(30));
        assert_eq!(
            Prefixes::new(vec![unit(10), unit(5)]),
            Err(NotMonotonic { index: 1 })
        );
        assert!(Prefixes::new(vec![]).unwrap().is_empty());
    }
}
