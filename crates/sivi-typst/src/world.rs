//! The in-memory `World` the compiler reads: one main source, the assets the deck names, and the fonts, nothing else.

use crate::fonts;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// The virtual path of the generated source.
pub const MAIN_PATH: &str = "main.typ";

/// A file the deck names, served to the compiler by its virtual path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    /// The path as the generated source names it, relative to the project root, with forward slashes.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Why a world could not be built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldError {
    /// A path the compiler cannot address.
    InvalidPath { path: String, reason: String },
}

impl fmt::Display for WorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { path, reason } => {
                write!(formatter, "the path `{path}` cannot be addressed: {reason}")
            }
        }
    }
}

impl std::error::Error for WorldError {}

/// The compiler's view of one deck: a main source, its assets, and the fonts, with no file system, clock, or network.
pub struct SiviWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
    files: HashMap<FileId, Bytes>,
}

impl fmt::Debug for SiviWorld {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SiviWorld")
            .field("fonts", &self.fonts.len())
            .field("files", &self.files.len())
            .finish_non_exhaustive()
    }
}

fn file_id(path: &str) -> Result<FileId, WorldError> {
    let virtual_path = VirtualPath::new(path).map_err(|error| WorldError::InvalidPath {
        path: path.to_owned(),
        reason: error.to_string(),
    })?;
    Ok(FileId::new(RootedPath::new(
        VirtualRoot::Project,
        virtual_path,
    )))
}

impl SiviWorld {
    /// A world over `source`, serving `assets` by path and the bundled fonts followed by `theme_fonts`.
    ///
    /// # Errors
    ///
    /// Returns an error when an asset path cannot be addressed by the compiler.
    pub fn new(
        source: &str,
        assets: Vec<Asset>,
        theme_fonts: Vec<Vec<u8>>,
    ) -> Result<Self, WorldError> {
        let mut fonts = fonts::bundled();
        for file in theme_fonts {
            fonts.extend(fonts::faces(file));
        }
        let book = fonts::book(&fonts);
        let main = Source::new(file_id(MAIN_PATH)?, source.to_owned());
        let mut files = HashMap::new();
        for asset in assets {
            files.insert(file_id(&asset.path)?, Bytes::new(asset.bytes));
        }
        Ok(Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            main,
            files,
        })
    }

    /// The generated source the world compiles.
    #[must_use]
    pub const fn source_text(&self) -> &Source {
        &self.main
    }
}

impl World for SiviWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_with_slash(),
            )))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).map_or_else(
            || {
                Err(FileError::NotFound(PathBuf::from(
                    id.vpath().get_with_slash(),
                )))
            },
            |bytes| Ok(bytes.clone()),
        )
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    /// No date: a theme that asks for today fails to compile, which keeps every build time-independent.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
