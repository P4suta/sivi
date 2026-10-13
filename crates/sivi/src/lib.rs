//! The command line of sivi and the adapters it composes.
//!
//! The binary reads files through `io`, writes the PDF through `publish`, and reaches the standard streams through `output`; everything else is a function of values.

pub mod args;
pub mod assets;
pub mod build;
pub mod cli;
pub mod digest;
pub mod fonts;
pub mod io;
pub mod output;
pub mod publish;
pub mod render;
pub mod themes;
