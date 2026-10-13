//! Exporting the compiled pages as a PDF whose bytes depend on the inputs alone.

use crate::compile::Diagnostic;
use std::fmt;
use typst::foundations::{Datetime, Smart};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

/// A creation time in UTC, which the caller derives from `SOURCE_DATE_EPOCH` and never from a clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct UtcTimestamp {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// What the PDF records about itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PdfSettings {
    /// A stable identity of the deck, such as a digest of every input, from which the document identifier derives.
    pub ident: String,
    /// The creation time, or `None` for a PDF without a date.
    pub timestamp: Option<UtcTimestamp>,
}

/// Why the PDF could not be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfError {
    Typst(Vec<Diagnostic>),
    InvalidTimestamp(UtcTimestamp),
}

impl fmt::Display for PdfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Typst(diagnostics) => {
                write!(formatter, "the PDF could not be written:")?;
                for diagnostic in diagnostics {
                    write!(formatter, "\n  {}", diagnostic.message)?;
                }
                Ok(())
            }
            Self::InvalidTimestamp(timestamp) => {
                write!(
                    formatter,
                    "the timestamp {timestamp:?} is not a calendar date and time"
                )
            }
        }
    }
}

impl std::error::Error for PdfError {}

const CREATOR: &str = concat!("sivi ", env!("CARGO_PKG_VERSION"));

fn timestamp(stamp: UtcTimestamp) -> Result<Timestamp, PdfError> {
    let datetime = Datetime::from_ymd_hms(
        stamp.year,
        stamp.month,
        stamp.day,
        stamp.hour,
        stamp.minute,
        stamp.second,
    )
    .ok_or(PdfError::InvalidTimestamp(stamp))?;
    Ok(Timestamp::new_utc(datetime))
}

/// The PDF bytes of `document`.
///
/// # Errors
///
/// Returns an error when the timestamp is not a date or Typst cannot write the document.
pub fn export(document: &PagedDocument, settings: &PdfSettings) -> Result<Vec<u8>, PdfError> {
    let options = PdfOptions {
        ident: Smart::Custom(settings.ident.clone()),
        creator: Smart::Custom(Some(CREATOR.to_owned())),
        timestamp: settings.timestamp.map(timestamp).transpose()?,
        page_ranges: None,
        standards: PdfStandards::default(),
        tagged: true,
        pretty: false,
    };
    typst_pdf::pdf(document, &options).map_err(|errors| {
        PdfError::Typst(
            errors
                .iter()
                .map(|error| Diagnostic {
                    message: error.message.to_string(),
                    range: None,
                    hints: error.hints.iter().map(|hint| hint.v.to_string()).collect(),
                })
                .collect(),
        )
    })
}
