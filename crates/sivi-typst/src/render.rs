//! The pages of a compiled deck as raster images, for a gallery of a theme and for a preview on a terminal.

use crate::compile::Compiled;
use sivi_core::mosaic::Raster;
use typst::utils::Scalar;
use typst_render::{RenderOptions, render};

/// The pixels per point a gallery renders at: twice the slide's own size, crisp on a dense display.
pub const GALLERY_PIXELS_PER_POINT: f64 = 2.0;

/// Why a page could not be encoded.
#[derive(Debug)]
pub struct EncodeError {
    /// The page, counted from one.
    pub page: usize,
    pub reason: String,
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "page {} could not be encoded as PNG: {}",
            self.page, self.reason
        )
    }
}

impl std::error::Error for EncodeError {}

/// The page at `index`, counted from zero, as a raster at `pixels_per_point`, or none past the last page.
#[must_use]
pub fn raster(compiled: &Compiled, index: usize, pixels_per_point: f64) -> Option<Raster> {
    let page = compiled.document.pages().get(index)?;
    let options = RenderOptions {
        pixel_per_pt: Scalar::new(pixels_per_point),
        ..RenderOptions::default()
    };
    let pixmap = render(page, &options);
    let unmultiplied: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    let (Ok(width), Ok(height)) = (
        usize::try_from(pixmap.width()),
        usize::try_from(pixmap.height()),
    ) else {
        return None;
    };
    Some(Raster {
        width,
        height,
        rgba: unmultiplied,
    })
}

/// A page as a PNG with its size in pixels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Picture {
    pub png: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

/// The page at `index`, counted from zero, as a picture at `pixels_per_point`, or none past the last page.
///
/// # Errors
///
/// Returns the encoder's refusal, which no page of a slide deck should earn.
pub fn picture(
    compiled: &Compiled,
    index: usize,
    pixels_per_point: f64,
) -> Result<Option<Picture>, EncodeError> {
    let Some(page) = compiled.document.pages().get(index) else {
        return Ok(None);
    };
    let options = RenderOptions {
        pixel_per_pt: Scalar::new(pixels_per_point),
        ..RenderOptions::default()
    };
    let pixmap = render(page, &options);
    let (Ok(width), Ok(height)) = (
        usize::try_from(pixmap.width()),
        usize::try_from(pixmap.height()),
    ) else {
        return Ok(None);
    };
    let png = pixmap.encode_png().map_err(|error| EncodeError {
        page: index.saturating_add(1),
        reason: error.to_string(),
    })?;
    Ok(Some(Picture { png, width, height }))
}

/// Every page of `compiled` as a PNG at `pixels_per_point`, in page order.
///
/// # Errors
///
/// Returns the first page the encoder refuses, which no page of a slide deck should be.
pub fn pngs(compiled: &Compiled, pixels_per_point: f64) -> Result<Vec<Vec<u8>>, EncodeError> {
    let options = RenderOptions {
        pixel_per_pt: Scalar::new(pixels_per_point),
        ..RenderOptions::default()
    };
    compiled
        .document
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            render(page, &options)
                .encode_png()
                .map_err(|error| EncodeError {
                    page: index.saturating_add(1),
                    reason: error.to_string(),
                })
        })
        .collect()
}
