//! The one owner of file reads in the binary: every read is bounded and reports its path.

use std::fmt;
use std::path::{Path, PathBuf};

/// The most bytes one read admits, which is the image limit of the deck format.
pub const MAX_READ: u64 = 32 * 1024 * 1024;

/// The path label used by diagnostics, with forward slashes between native components.
#[must_use]
pub fn path_label(path: &Path) -> String {
    path.display()
        .to_string()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

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

    /// Whether a file sits at `path`, telling absence from any other failure.
    pub(super) fn is_file(path: &Path) -> std::io::Result<bool> {
        match std::fs::metadata(path) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Creates the directory at `path` and every parent it lacks.
    pub(super) fn create_dir_all(path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    /// The paths directly under the directory at `path`, in name order.
    pub(super) fn entries(path: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(path)? {
            found.push(entry?.path());
        }
        found.sort();
        Ok(found)
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
            ReadErrorKind::Io(error) if error.kind() == std::io::ErrorKind::NotFound => write!(
                formatter,
                "cannot read {}: file or directory not found",
                path_label(&self.path)
            ),
            ReadErrorKind::Io(error) => {
                write!(formatter, "cannot read {}: {error}", path_label(&self.path))
            }
            ReadErrorKind::TooLarge => write!(
                formatter,
                "{} holds more than {MAX_READ} bytes",
                path_label(&self.path)
            ),
            ReadErrorKind::NotUtf8 => write!(formatter, "{} is not UTF-8", path_label(&self.path)),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ReadErrorKind::Io(error) => Some(error),
            ReadErrorKind::TooLarge | ReadErrorKind::NotUtf8 => None,
        }
    }
}

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

/// Whether a file sits at `path`; absence is `false`, any other failure an error.
///
/// # Errors
///
/// Returns the path with why the file's presence could not be told.
pub fn is_file(path: &Path) -> Result<bool, ReadError> {
    raw::is_file(path).map_err(|error| ReadError {
        path: path.to_path_buf(),
        kind: ReadErrorKind::Io(error),
    })
}

/// Creates the directory at `path` and every parent it lacks.
///
/// # Errors
///
/// Returns the path with why a directory could not be created.
pub fn create_dir_all(path: &Path) -> Result<(), ReadError> {
    raw::create_dir_all(path).map_err(|error| ReadError {
        path: path.to_path_buf(),
        kind: ReadErrorKind::Io(error),
    })
}

/// The paths directly under the directory at `path`, in name order.
///
/// # Errors
///
/// Returns the path with why the directory could not be read.
pub fn entries(path: &Path) -> Result<Vec<PathBuf>, ReadError> {
    raw::entries(path).map_err(|error| ReadError {
        path: path.to_path_buf(),
        kind: ReadErrorKind::Io(error),
    })
}

#[cfg(test)]
mod tests {
    use super::{ReadError, ReadErrorKind, path_label, read, read_to_string};
    use std::error::Error as _;
    use std::path::Path;

    #[test]
    fn diagnostic_paths_use_slashes_between_native_components() {
        let path = Path::new("slides").join("図").join("deck.md");
        assert_eq!(path_label(&path), "slides/図/deck.md");
        #[cfg(unix)]
        assert_eq!(path_label(Path::new(r"literal\name")), r"literal\name");
        #[cfg(windows)]
        assert_eq!(
            path_label(Path::new(r"C:\slides\deck.md")),
            "C:/slides/deck.md"
        );
    }

    #[test]
    fn missing_entries_have_one_diagnostic_and_keep_the_native_source() {
        for native in [
            "No such file or directory (os error 2)",
            "The system cannot find the path specified. (os error 3)",
        ] {
            let error = ReadError {
                path: Path::new("themes").join("missing").join("theme.toml"),
                kind: ReadErrorKind::Io(std::io::Error::new(std::io::ErrorKind::NotFound, native)),
            };
            assert_eq!(
                error.to_string(),
                "cannot read themes/missing/theme.toml: file or directory not found"
            );
            assert_eq!(error.source().unwrap().to_string(), native);
        }
    }

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
