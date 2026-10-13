//! The one owner of the standard streams: standard output carries only the PDF bytes or the JSON document, standard error the diagnostics.

use std::io;

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the binary's one stream owner reaches standard output in a single statement"
    )]

    use std::io::{self, Write as _};

    /// Writes `bytes` to standard output and flushes it.
    pub(super) fn stdout(bytes: &[u8]) -> io::Result<()> {
        let mut out = io::stdout().lock();
        out.write_all(bytes)?;
        out.flush()
    }
}

/// Writes the one document a caller reads: the PDF bytes or the JSON diagnostics.
///
/// # Errors
///
/// Returns the write error when standard output is closed or full.
pub fn stdout(bytes: &[u8]) -> io::Result<()> {
    raw::stdout(bytes)
}

/// Writes text for a person to standard error.
pub fn stderr(text: &str) {
    eprint!("{text}");
}
