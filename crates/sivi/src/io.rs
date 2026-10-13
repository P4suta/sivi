//! The one owner of file reads in the binary: every read is bounded and reports its path.

use std::fmt;
use std::path::{Path, PathBuf};

/// The most bytes one read admits, which is the image limit of the deck format.
pub const MAX_READ: u64 = 32 * 1024 * 1024;

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the binary's one file owner performs each read in a single statement"
    )]

    use std::io::Read as _;
    use std::path::Path;

    /// The first `limit` bytes of the file at `path`, and whether the file holds more.
    pub(super) fn read(path: &Path, limit: u64) -> std::io::Result<(Vec<u8>, bool)> {
        let file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
        let over = u64::try_from(bytes.len()).is_ok_and(|length| length > limit);
        if over {
            bytes.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        Ok((bytes, over))
    }
}

/// Why a file could not be read.
#[derive(Debug)]
pub struct ReadError {
    pub path: PathBuf,
    pub kind: ReadErrorKind,
}

#[derive(Debug)]
pub enum ReadErrorKind {
    /// The operating system refused the read.
    Io(std::io::Error),
    /// The file holds more than `MAX_READ` bytes.
    TooLarge,
    /// The file is not UTF-8, which a text read requires.
    NotUtf8,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ReadErrorKind::Io(error) => {
                write!(formatter, "cannot read {}: {error}", self.path.display())
            }
            ReadErrorKind::TooLarge => write!(
                formatter,
                "{} holds more than {MAX_READ} bytes",
                self.path.display()
            ),
            ReadErrorKind::NotUtf8 => write!(formatter, "{} is not UTF-8", self.path.display()),
        }
    }
}

impl std::error::Error for ReadError {}

/// The bytes of the file at `path`, up to `MAX_READ`.
///
/// # Errors
///
/// Returns the path with why the read failed.
pub fn read(path: &Path) -> Result<Vec<u8>, ReadError> {
    match raw::read(path, MAX_READ) {
        Ok((_, true)) => Err(ReadError {
            path: path.to_path_buf(),
            kind: ReadErrorKind::TooLarge,
        }),
        Ok((bytes, false)) => Ok(bytes),
        Err(error) => Err(ReadError {
            path: path.to_path_buf(),
            kind: ReadErrorKind::Io(error),
        }),
    }
}

/// The text of the file at `path`, up to `MAX_READ`.
///
/// # Errors
///
/// Returns the path with why the read failed, or that the file is not UTF-8.
pub fn read_to_string(path: &Path) -> Result<String, ReadError> {
    String::from_utf8(read(path)?).map_err(|_invalid| ReadError {
        path: path.to_path_buf(),
        kind: ReadErrorKind::NotUtf8,
    })
}

#[cfg(test)]
mod tests {
    use super::{ReadErrorKind, read, read_to_string};
    use std::path::Path;

    #[test]
    fn a_read_returns_the_bytes_and_names_a_missing_file() {
        let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(read_to_string(&own).unwrap().contains("[package]"));
        assert_eq!(
            read(&own).unwrap(),
            read_to_string(&own).unwrap().into_bytes()
        );
        let missing = read(Path::new("/nonexistent/sivi/file"));
        let Err(error) = missing else {
            panic!("a missing file");
        };
        assert!(matches!(error.kind, ReadErrorKind::Io(_)));
        assert!(format!("{error}").contains("nonexistent"));
    }
}
