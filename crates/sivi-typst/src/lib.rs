//! The Typst adapter of sivi.
//!
//! It compiles the generated source with the bundled fonts, reads every measurement and frame back as core values, and exports the PDF.
//! Every Typst type stays inside this crate; the rest of sivi sees only the values of `sivi-core`.

pub mod compile;
pub mod convert;
pub mod fonts;
pub mod frames;
pub mod introspect;
pub mod pdf;
pub mod raw;
pub mod render;
pub mod world;

pub use compile::{CompileError, Compiled, Diagnostic, compile};
pub use introspect::{ReadBackError, measure};
pub use pdf::{PdfError, PdfSettings, UtcTimestamp, export};
pub use world::{Asset, SiviWorld, WorldError};
