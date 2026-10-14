//! The one owner of a child process: the viewer the host opens a PDF with.

use std::io;
use std::path::Path;

/// The viewer of the host: `open` on macOS, `start` on Windows, and `xdg-open` elsewhere.
const MACOS_OPENER: &str = "open";
const WINDOWS_OPENER: &str = "cmd";
const WINDOWS_ARGUMENTS: [&str; 3] = ["/c", "start", ""];
const OTHER_OPENER: &str = "xdg-open";

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the interface's one process owner starts the host's viewer in this statement alone"
    )]

    use std::io;
    use std::path::Path;

    pub(super) fn spawn(program: &str, arguments: &[&str], path: &Path) -> io::Result<()> {
        std::process::Command::new(program)
            .args(arguments)
            .arg(path)
            .spawn()
            .map(|_child| ())
    }
}

/// Opens `path` with the host's viewer, without waiting for it.
///
/// # Errors
///
/// Returns the error when the viewer cannot be started.
pub(super) fn open(path: &Path) -> io::Result<()> {
    if cfg!(target_os = "macos") {
        raw::spawn(MACOS_OPENER, &[], path)
    } else if cfg!(windows) {
        raw::spawn(WINDOWS_OPENER, &WINDOWS_ARGUMENTS, path)
    } else {
        raw::spawn(OTHER_OPENER, &[], path)
    }
}
