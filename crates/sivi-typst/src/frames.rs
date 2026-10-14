//! Walking the laid-out frames of every page into leaf boxes in page coordinates, and finding glyphs no font had.
//!
//! A group that clips its frame shows only what lies inside its clip, so every box under it is cut to the clip's box:
//! an image that covers a box reaches past it as an item, and reads back as the box it shows through.

use crate::convert;
use crate::introspect::ReadBackError;
use sivi_core::length::{Rect, Size};
use sivi_core::measure::{ItemKind, Leaf, MissingGlyph, PageNumber};
use typst::layout::{Frame, FrameItem, Point as TypstPoint, Rect as TypstRect, Transform};
use typst::text::TextItem;
use typst_layout::PagedDocument;

const NOTDEF_GLYPH: u16 = 0;

/// Where a walk stands: the transform to the page and the box, in page coordinates, that the enclosing groups show.
#[derive(Clone, Copy)]
struct Place {
    transform: Transform,
    clip: Option<TypstRect>,
}

/// What a walk over the pages found.
#[derive(Debug, Default)]
struct Found {
    leaves: Vec<Leaf>,
    missing: Vec<MissingGlyph>,
}

/// Every text, shape, and image box of every page in page coordinates, and every character shown with the missing glyph.
///
/// # Errors
///
/// Returns an error when a page count or a box cannot be represented by the core.
pub fn leaves(document: &PagedDocument) -> Result<(Vec<Leaf>, Vec<MissingGlyph>), ReadBackError> {
    let mut found = Found::default();
    for (index, page) in document.pages().iter().enumerate() {
        let number = u32::try_from(index.saturating_add(1)).map_err(|_overflow| {
            ReadBackError::TooManyPages {
                pages: document.pages().len(),
            }
        })?;
        walk(
            &page.frame,
            Place {
                transform: Transform::identity(),
                clip: None,
            },
            PageNumber(number),
            &mut found,
        )?;
    }
    Ok((found.leaves, found.missing))
}

/// The box `local` lands in on the page under `transform`, as its least and greatest corners.
fn on_page(local: TypstRect, transform: Transform) -> TypstRect {
    let corners = [
        TypstPoint::new(local.min.x, local.min.y),
        TypstPoint::new(local.max.x, local.min.y),
        TypstPoint::new(local.min.x, local.max.y),
        TypstPoint::new(local.max.x, local.max.y),
    ]
    .map(|corner| corner.transform(transform));
    let min = corners
        .iter()
        .fold(corners[0], |low, corner| low.min(*corner));
    let max = corners
        .iter()
        .fold(corners[0], |high, corner| high.max(*corner));
    TypstRect::new(min, max)
}

/// The part of `shown` inside `clip`, or an empty box at the clip's near corner when nothing of it shows.
fn clipped(shown: TypstRect, clip: Option<TypstRect>) -> TypstRect {
    let Some(clip) = clip else {
        return shown;
    };
    let min = shown.min.max(clip.min);
    let max = shown.max.min(clip.max).max(min);
    TypstRect::new(min, max)
}

fn bounds(local: TypstRect, place: Place, page: PageNumber) -> Result<Rect, ReadBackError> {
    let shown = clipped(on_page(local, place.transform), place.clip);
    let unrepresentable = |error: convert::Unrepresentable| ReadBackError::UnrepresentableItem {
        page,
        points: error.points,
    };
    let origin = convert::point(shown.min).map_err(unrepresentable)?;
    let far = convert::point(shown.max).map_err(unrepresentable)?;
    Ok(Rect::new(
        origin,
        Size {
            width: far.x.saturating_sub(origin.x),
            height: far.y.saturating_sub(origin.y),
        },
    ))
}

fn missing_glyphs(text: &TextItem, page: PageNumber, missing: &mut Vec<MissingGlyph>) {
    for glyph in &text.glyphs {
        if glyph.id != NOTDEF_GLYPH {
            continue;
        }
        if let Some(character) = text
            .text
            .get(glyph.range())
            .and_then(|slice| slice.chars().next())
        {
            missing.push(MissingGlyph { page, character });
        }
    }
}

fn walk(
    frame: &Frame,
    place: Place,
    page: PageNumber,
    found: &mut Found,
) -> Result<(), ReadBackError> {
    for (position, item) in frame.items() {
        let placed = Place {
            transform: place
                .transform
                .pre_concat(Transform::translate(position.x, position.y)),
            clip: place.clip,
        };
        match item {
            FrameItem::Group(group) => {
                let transform = placed.transform.pre_concat(group.transform);
                let clip = group.clip.as_ref().map_or(placed.clip, |curve| {
                    Some(clipped(on_page(curve.bbox(None), transform), placed.clip))
                });
                walk(&group.frame, Place { transform, clip }, page, found)?;
            }
            FrameItem::Text(text) => {
                missing_glyphs(text, page, &mut found.missing);
                found.leaves.push(Leaf {
                    page,
                    kind: ItemKind::Text,
                    bounds: bounds(text.bbox(), placed, page)?,
                    text: Some(text.text.to_string()),
                });
            }
            FrameItem::Shape(shape, _) => {
                found.leaves.push(Leaf {
                    page,
                    kind: ItemKind::Shape,
                    bounds: bounds(shape.bbox(true), placed, page)?,
                    text: None,
                });
            }
            FrameItem::Image(_, size, _) => {
                let local = TypstRect::new(TypstPoint::zero(), TypstPoint::new(size.x, size.y));
                found.leaves.push(Leaf {
                    page,
                    kind: ItemKind::Image,
                    bounds: bounds(local, placed, page)?,
                    text: None,
                });
            }
            FrameItem::Link(..) | FrameItem::Tag(_) => {}
        }
    }
    Ok(())
}
