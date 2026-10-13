//! Reading the markers the emitter placed back out of the compiled document.

use crate::convert;
use crate::frames;
use sivi_core::length::Length;
use sivi_core::measure::{Expected, Measurement, PageCount, PageNumber, Prefixes, RegionMeasure};
use sivi_core::protocol::{
    MeasureLabel, PREFIXES_KEY, REQUIRED_KEY, RegionIndex, SLIDE_KEY, SlideIndex, SlideLabel,
    WIDTHS_KEY,
};
use std::fmt;
use typst::foundations::{Content, Dict, Label, Selector, Value};
use typst::introspection::{Introspector, MetadataElem};
use typst::utils::{Numeric, PicoStr};
use typst_layout::PagedDocument;

/// Why a marker could not be read, which means the adapter has no evidence and never a verdict.
#[derive(Clone, Debug, PartialEq)]
pub enum ReadBackError {
    /// The label the emitter promised is absent from the document.
    MissingMarker { label: String },
    /// The label appears more than once, so neither occurrence is the region's.
    DuplicateMarker { label: String },
    /// The marker carries something other than the dictionary the protocol defines.
    MalformedMarker { label: String, reason: String },
    /// A measured length is not finite or lies beyond what the core represents.
    Unrepresentable { label: String, points: f64 },
    /// A cumulative height list decreases, which no layout produces.
    NotMonotonic { label: String, index: usize },
    /// A laid-out item has a box the core cannot represent.
    UnrepresentableItem { page: PageNumber, points: f64 },
    /// The document has more pages than the core counts.
    TooManyPages { pages: usize },
}

impl fmt::Display for ReadBackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMarker { label } => write!(formatter, "the marker <{label}> is missing"),
            Self::DuplicateMarker { label } => {
                write!(formatter, "the marker <{label}> appears more than once")
            }
            Self::MalformedMarker { label, reason } => {
                write!(formatter, "the marker <{label}> is malformed: {reason}")
            }
            Self::Unrepresentable { label, points } => write!(
                formatter,
                "the marker <{label}> carries the length {points}pt, which cannot be represented"
            ),
            Self::NotMonotonic { label, index } => write!(
                formatter,
                "the marker <{label}> carries a cumulative height that decreases at index {index}"
            ),
            Self::UnrepresentableItem { page, points } => write!(
                formatter,
                "page {} holds an item at {points}pt, which cannot be represented",
                page.0
            ),
            Self::TooManyPages { pages } => {
                write!(
                    formatter,
                    "the document has {pages} pages, more than the core counts"
                )
            }
        }
    }
}

impl std::error::Error for ReadBackError {}

fn malformed(label: &str, reason: impl Into<String>) -> ReadBackError {
    ReadBackError::MalformedMarker {
        label: label.to_owned(),
        reason: reason.into(),
    }
}

/// The one element labelled `label`.
fn marker(document: &PagedDocument, label: &str) -> Result<Content, ReadBackError> {
    let typst_label =
        Label::new(PicoStr::intern(label)).ok_or_else(|| malformed(label, "an empty label"))?;
    let found = document.introspector().query(&Selector::Label(typst_label));
    match found.as_slice() {
        [] => Err(ReadBackError::MissingMarker {
            label: label.to_owned(),
        }),
        [only] => Ok(only.clone()),
        [_, _, ..] => Err(ReadBackError::DuplicateMarker {
            label: label.to_owned(),
        }),
    }
}

fn dictionary<'a>(content: &'a Content, label: &str) -> Result<&'a Dict, ReadBackError> {
    let metadata = content
        .to_packed::<MetadataElem>()
        .ok_or_else(|| malformed(label, "not a metadata element"))?;
    if let Value::Dict(dict) = &metadata.value {
        Ok(dict)
    } else {
        Err(malformed(
            label,
            format!("its value is {}, not a dictionary", metadata.value.ty()),
        ))
    }
}

fn length(value: &Value, label: &str) -> Result<Length, ReadBackError> {
    let Value::Length(length) = value else {
        return Err(malformed(
            label,
            format!("a {} where a length belongs", value.ty()),
        ));
    };
    if !length.em.is_zero() {
        return Err(malformed(
            label,
            "a length with an em part, which no measurement yields",
        ));
    }
    convert::length(length.abs).map_err(|error| ReadBackError::Unrepresentable {
        label: label.to_owned(),
        points: error.points,
    })
}

fn lengths(value: &Value, label: &str) -> Result<Vec<Length>, ReadBackError> {
    let Value::Array(array) = value else {
        return Err(malformed(
            label,
            format!("a {} where an array belongs", value.ty()),
        ));
    };
    array.iter().map(|item| length(item, label)).collect()
}

fn field<'a>(dict: &'a Dict, key: &str, label: &str) -> Result<&'a Value, ReadBackError> {
    dict.get(key)
        .map_err(|_absent| malformed(label, format!("no `{key}` entry")))
}

fn page_of(
    document: &PagedDocument,
    content: &Content,
    label: &str,
) -> Result<PageNumber, ReadBackError> {
    let location = content
        .location()
        .ok_or_else(|| malformed(label, "no location"))?;
    let position = document
        .introspector()
        .position(location)
        .ok_or_else(|| malformed(label, "no position on any page"))?;
    let page =
        u32::try_from(position.page.get()).map_err(|_overflow| ReadBackError::TooManyPages {
            pages: position.page.get(),
        })?;
    Ok(PageNumber(page))
}

/// The page each slide marker landed on.
///
/// # Errors
///
/// Returns an error when a slide marker is missing, duplicated, or malformed.
pub fn slide_pages(
    document: &PagedDocument,
    expected: &Expected,
) -> Result<Vec<(SlideIndex, PageNumber)>, ReadBackError> {
    let mut pages = Vec::new();
    for index in 0..expected.slides {
        let slide = SlideIndex(index);
        let label = SlideLabel::new(slide);
        let content = marker(document, label.as_str())?;
        let dict = dictionary(&content, label.as_str())?;
        let entry = field(dict, SLIDE_KEY, label.as_str())?;
        if !matches!(entry, Value::Int(value) if i64::from(index) == *value) {
            return Err(malformed(
                label.as_str(),
                format!("its slide entry is {entry:?}, not {index}"),
            ));
        }
        pages.push((slide, page_of(document, &content, label.as_str())?));
    }
    Ok(pages)
}

/// The measurement of every region the emitter placed.
///
/// # Errors
///
/// Returns an error when a measurement marker is missing, duplicated, malformed, or carries unrepresentable lengths.
pub fn regions(
    document: &PagedDocument,
    expected: &Expected,
) -> Result<Vec<RegionMeasure>, ReadBackError> {
    let mut regions = Vec::new();
    for region in &expected.regions {
        let label = MeasureLabel::new(region.slide, region.region);
        let name = label.as_str();
        let content = marker(document, name)?;
        let dict = dictionary(&content, name)?;
        let required = length(field(dict, REQUIRED_KEY, name)?, name)?;
        let heights = lengths(field(dict, PREFIXES_KEY, name)?, name)?;
        let split_points = usize::try_from(region.split_points)
            .map_err(|_overflow| malformed(name, "too many split points"))?;
        if heights.len() != split_points {
            return Err(malformed(
                name,
                format!(
                    "{} prefixes where {} split points were placed",
                    heights.len(),
                    region.split_points
                ),
            ));
        }
        let prefixes = Prefixes::new(heights).map_err(|error| ReadBackError::NotMonotonic {
            label: name.to_owned(),
            index: error.index,
        })?;
        let widths = lengths(field(dict, WIDTHS_KEY, name)?, name)?;
        let natural_widths = widths
            .into_iter()
            .enumerate()
            .filter(|(_, width)| width.is_positive())
            .map(|(index, width)| {
                u32::try_from(index)
                    .map(|part| (part, width))
                    .map_err(|_overflow| malformed(name, "too many parts"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        regions.push(RegionMeasure {
            slide: region.slide,
            region: RegionIndex(region.region.0),
            required,
            prefixes,
            natural_widths,
        });
    }
    Ok(regions)
}

/// Everything the adapter measures in a compiled deck.
///
/// # Errors
///
/// Returns the first marker or item that cannot be read back as a core value.
pub fn measure(
    document: &PagedDocument,
    expected: &Expected,
) -> Result<Measurement, ReadBackError> {
    let pages =
        u32::try_from(document.pages().len()).map_err(|_overflow| ReadBackError::TooManyPages {
            pages: document.pages().len(),
        })?;
    let (leaves, missing_glyphs) = frames::leaves(document)?;
    Ok(Measurement {
        regions: regions(document, expected)?,
        pages: PageCount(pages),
        slide_pages: slide_pages(document, expected)?,
        leaves,
        missing_glyphs,
    })
}
