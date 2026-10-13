//! Compiling the generated source into pages, with every Typst diagnostic mapped to a byte range of that source.

use crate::world::SiviWorld;
use std::fmt;
use std::ops::Range;
use typst::WorldExt;
use typst::diag::{SourceDiagnostic, Warned};
use typst_layout::PagedDocument;

/// A compiled deck and the warnings Typst raised while compiling it.
#[derive(Debug)]
pub struct Compiled {
    pub document: PagedDocument,
    pub warnings: Vec<Diagnostic>,
}

/// One message from Typst, located in the generated source when it has a location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    /// The byte range in the generated source, when the message points into it.
    pub range: Option<Range<usize>>,
    pub hints: Vec<String>,
}

/// Why the generated source did not compile, which is a sivi defect or a theme defect, never a deck defect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    Typst(Vec<Diagnostic>),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Typst(diagnostics) => {
                write!(formatter, "the generated Typst source failed to compile:")?;
                for diagnostic in diagnostics {
                    write!(formatter, "\n  {}", diagnostic.message)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CompileError {}

fn diagnostic(world: &SiviWorld, source: &SourceDiagnostic) -> Diagnostic {
    Diagnostic {
        message: source.message.to_string(),
        range: world.range(source.span),
        hints: source.hints.iter().map(|hint| hint.v.to_string()).collect(),
    }
}

/// Compiles the world's main source into pages.
///
/// # Errors
///
/// Returns every error Typst reported when the source does not compile.
pub fn compile(world: &SiviWorld) -> Result<Compiled, CompileError> {
    let Warned { output, warnings } = typst::compile::<PagedDocument>(world);
    let warnings = warnings
        .iter()
        .map(|warning| diagnostic(world, warning))
        .collect();
    match output {
        Ok(document) => Ok(Compiled { document, warnings }),
        Err(errors) => Err(CompileError::Typst(
            errors
                .iter()
                .map(|error| diagnostic(world, error))
                .collect(),
        )),
    }
}
