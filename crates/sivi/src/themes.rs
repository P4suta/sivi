//! Loading the theme a deck names or the command line chooses: the built-in theme, or a directory with `theme.toml` and its font files.

use crate::digest::sha256_hex;
use crate::fonts::{self, face_error};
use crate::io::{self, ReadError, ReadErrorKind};
use crate::library;
use sivi_core::diagnostic::{Code, Diagnostic, has_error};
use sivi_core::text::ThemeRef;
use sivi_core::theme::{FamilyName, FontFile, Theme};
use sivi_theme::faces::{self, Face};
use std::path::{Path, PathBuf};

/// The file a theme directory holds.
pub const THEME_FILE: &str = "theme.toml";

/// Which theme to load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Choice {
    Builtin(String),
    Dir(PathBuf),
}

/// A theme with its text, its label for diagnostics, its directory, its font files, and every face available to it.
#[derive(Clone, Debug)]
pub struct LoadedTheme {
    pub theme: Theme,
    pub text: String,
    /// The built-in name or the path of the theme file, as diagnostics name it.
    pub label: String,
    pub dir: Option<PathBuf>,
    pub fonts: Vec<Vec<u8>>,
    pub faces: Vec<Face>,
}

/// The theme to load: the command line's choice, else the deck's `theme:`, else the built-in theme.
#[must_use]
pub fn choose(option: Option<&str>, head: Option<&ThemeRef>, deck_dir: &Path) -> Choice {
    if let Some(chosen) = option {
        return if sivi_theme::builtin(chosen).is_some() {
            Choice::Builtin(String::from(chosen))
        } else {
            Choice::Dir(PathBuf::from(chosen))
        };
    }
    match head {
        Some(ThemeRef::Builtin(name)) => Choice::Builtin(String::from(name.as_str())),
        Some(ThemeRef::Path(path)) => Choice::Dir(deck_dir.join(path.as_str())),
        None => Choice::Builtin(String::from(sivi_theme::PLAIN_NAME)),
    }
}

fn font_read_error(file: &FontFile, path: &Path, error: &ReadError) -> Diagnostic {
    let missing = matches!(&error.kind, ReadErrorKind::Io(inner) if inner.kind() == std::io::ErrorKind::NotFound);
    if missing {
        Diagnostic::new(
            Code::FontMissing,
            format!(
                "the font file `{}` of family `{}` is missing at {}",
                file.path,
                file.family,
                path.display()
            ),
        )
        .help(String::from(
            "put the file beside the theme, or correct its `path`",
        ))
    } else {
        Diagnostic::new(
            Code::FontUnreadable,
            format!("the font file `{}` cannot be read: {error}", file.path),
        )
    }
}

/// Reads one font file of the theme, checking its digest, its face, and its family.
fn font(dir: &Path, file: &FontFile, loaded: &mut LoadedTheme) -> Option<Diagnostic> {
    let path = dir.join(&file.path);
    let bytes = match io::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return Some(font_read_error(file, &path, &error)),
    };
    let digest = sha256_hex(&[&bytes]);
    if digest != file.sha256.as_str() {
        return Some(
            Diagnostic::new(
                Code::FontDigest,
                format!(
                    "the font file `{}` does not match the `sha256` the theme declares",
                    file.path
                ),
            )
            .note(format!("the file's digest is {digest}"))
            .help(String::from(
                "write the digest of the file you ship, as `shasum -a 256` prints it",
            )),
        );
    }
    let face = match faces::face(&bytes) {
        Ok(face) => face,
        Err(error) => {
            return Some(Diagnostic::new(
                Code::FontUnreadable,
                format!("the font file `{}` is {}", file.path, face_error(error)),
            ));
        }
    };
    if face.family != file.family {
        return Some(
            Diagnostic::new(
                Code::FamilyMismatch,
                format!(
                    "the font file `{}` names the family `{}`, not `{}`",
                    file.path, face.family, file.family
                ),
            )
            .help(String::from(
                "set `family` to the name the file carries, which the theme's styles then use",
            )),
        );
    }
    loaded.faces.push(face);
    loaded.fonts.push(bytes);
    None
}

fn text_of(choice: &Choice) -> Result<(String, String, Option<PathBuf>), Vec<Diagnostic>> {
    match choice {
        Choice::Builtin(name) => sivi_theme::builtin(name).map_or_else(
            || {
                Err(vec![
                    Diagnostic::new(
                        Code::ThemeToml,
                        format!("no built-in theme is named `{name}`"),
                    )
                    .help(format!(
                        "the built-in theme is `{}`; a directory with `{THEME_FILE}` holds a custom theme",
                        sivi_theme::PLAIN_NAME
                    )),
                ])
            },
            |text| Ok((String::from(text), name.clone(), None)),
        ),
        Choice::Dir(dir) => {
            let path = dir.join(THEME_FILE);
            match io::read_to_string(&path) {
                Ok(text) => Ok((text, path.display().to_string(), Some(dir.clone()))),
                Err(error) => Err(vec![
                    Diagnostic::new(
                        Code::ThemeToml,
                        format!("the theme file cannot be read: {error}"),
                    )
                    .help(format!(
                        "name a directory that holds `{THEME_FILE}`, or a built-in theme"
                    )),
                ]),
            }
        }
    }
}

/// What reading the catalog families left: the diagnostics, and the families no face could be read for.
struct CatalogFonts {
    diagnostics: Vec<Diagnostic>,
    absent: Vec<FamilyName>,
}

/// Reads every family the theme sets text in that neither the bundled fonts nor its own files provide, from the library.
fn catalog_fonts(loaded: &mut LoadedTheme, fonts_dir: Option<&Path>) -> CatalogFonts {
    let mut found = CatalogFonts {
        diagnostics: Vec::new(),
        absent: Vec::new(),
    };
    let catalog = match sivi_theme::catalog::catalog() {
        Ok(catalog) => catalog,
        Err(message) => {
            found
                .diagnostics
                .push(Diagnostic::new(Code::Internal, message));
            return found;
        }
    };
    let declared: Vec<FamilyName> = loaded
        .theme
        .fonts
        .iter()
        .map(|file| file.family.clone())
        .collect();
    let wanted: Vec<FamilyName> = loaded.theme.families().into_iter().cloned().collect();
    let bundled = library::bundled_names();
    for family in wanted {
        if declared.contains(&family) || loaded.faces.iter().any(|face| face.family == family) {
            continue;
        }
        let Some(entry) = catalog.family(family.as_str()) else {
            found.diagnostics.push(
                library::unknown_family(family.as_str(), catalog, &bundled)
                    .note(String::from("the theme sets text in it")),
            );
            found.absent.push(family);
            continue;
        };
        let Some(dir) = fonts_dir else {
            found.diagnostics.push(library::not_installed(entry, None));
            found.absent.push(family);
            continue;
        };
        match library::read_installed(entry, dir) {
            Ok(files) => {
                for bytes in files {
                    match faces::face(&bytes) {
                        Ok(face) => {
                            loaded.faces.push(face);
                            loaded.fonts.push(bytes);
                        }
                        Err(error) => found.diagnostics.push(Diagnostic::new(
                            Code::FontUnreadable,
                            format!(
                                "an installed file of `{}` is {}",
                                entry.name,
                                face_error(error)
                            ),
                        )),
                    }
                }
            }
            Err(diagnostic) => {
                found.diagnostics.push(*diagnostic);
                found.absent.push(family);
            }
        }
    }
    found
}

/// Loads and checks the chosen theme, with the warnings it earned, reading catalog families from the library under `fonts_dir`.
///
/// # Errors
///
/// Returns every diagnostic when the theme cannot be read, does not validate, or lacks a font or face it needs.
pub fn load(
    choice: &Choice,
    fonts_dir: Option<&Path>,
) -> Result<(LoadedTheme, Vec<Diagnostic>), Vec<Diagnostic>> {
    let (text, label, dir) = text_of(choice)?;
    let theme = sivi_theme::parse(&text)?;
    let faces =
        fonts::bundled_faces().map_err(|message| vec![Diagnostic::new(Code::Internal, message)])?;
    let mut loaded = LoadedTheme {
        theme,
        text,
        label,
        dir,
        fonts: Vec::new(),
        faces,
    };
    let mut diagnostics = Vec::new();
    let files = loaded.theme.fonts.clone();
    for file in &files {
        let Some(theme_dir) = loaded.dir.clone() else {
            diagnostics.push(Diagnostic::new(
                Code::FontMissing,
                format!(
                    "the built-in theme names the font file `{}`, which it cannot carry",
                    file.path
                ),
            ));
            continue;
        };
        if let Some(diagnostic) = font(&theme_dir, file, &mut loaded) {
            diagnostics.push(diagnostic);
        }
    }
    let from_catalog = catalog_fonts(&mut loaded, fonts_dir);
    diagnostics.extend(from_catalog.diagnostics);
    diagnostics.extend(faces::check_except(
        &loaded.theme,
        &loaded.faces,
        &from_catalog.absent,
    ));
    diagnostics.extend(sivi_theme::design::lints(&loaded.theme));
    if has_error(&diagnostics) {
        Err(diagnostics)
    } else {
        Ok((loaded, diagnostics))
    }
}

#[cfg(test)]
mod tests {
    use super::{Choice, choose, load};
    use sivi_core::diagnostic::Code;
    use sivi_core::text::ThemeRef;
    use std::path::{Path, PathBuf};

    #[test]
    fn the_choice_prefers_the_option_then_the_deck_then_the_built_in_theme() {
        let deck_dir = Path::new("decks");
        assert_eq!(
            choose(Some("plain"), None, deck_dir),
            Choice::Builtin(String::from("plain"))
        );
        assert_eq!(
            choose(Some("themes/mine"), None, deck_dir),
            Choice::Dir(PathBuf::from("themes/mine"))
        );
        let relative = ThemeRef::new("../themes/mine").unwrap();
        assert_eq!(
            choose(None, Some(&relative), deck_dir),
            Choice::Dir(PathBuf::from("decks/../themes/mine"))
        );
        assert_eq!(
            choose(None, None, deck_dir),
            Choice::Builtin(String::from("plain"))
        );
    }

    #[test]
    fn the_built_in_theme_loads_with_the_bundled_faces_and_an_unknown_name_is_refused() {
        let (loaded, warnings) = load(&Choice::Builtin(String::from("plain")), None).unwrap();
        assert_eq!(loaded.label, "plain");
        assert_eq!(loaded.faces.len(), 12);
        assert_eq!(warnings, Vec::new());
        let refused = load(&Choice::Builtin(String::from("nope")), None).unwrap_err();
        assert_eq!(refused[0].code, Code::ThemeToml);
        let unreadable = load(&Choice::Dir(PathBuf::from("/nonexistent/sivi")), None).unwrap_err();
        assert_eq!(unreadable[0].code, Code::ThemeToml);
    }

    /// A copy of the plain theme whose styles are set in `family`, written under a fresh directory.
    fn theme_in(family: &str) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("theme");
        crate::io::create_dir_all(&dir).unwrap();
        let text = sivi_theme::PLAIN.replace(
            "font = \"BIZ UDPGothic\"",
            &format!("font = [\"{family}\", \"BIZ UDPGothic\"]"),
        );
        crate::publish::publish(
            &mut crate::publish::Disk::default(),
            &dir.join(super::THEME_FILE),
            text.as_bytes(),
        )
        .unwrap();
        (root, dir)
    }

    #[test]
    fn a_catalog_family_that_is_not_installed_is_named_once_with_its_command() {
        let (root, dir) = theme_in("Zen Maru Gothic");
        let empty = root.path().join("fonts");
        let refused = load(&Choice::Dir(dir.clone()), Some(&empty)).unwrap_err();
        let codes: Vec<Code> = refused.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec![Code::FontNotInstalled], "{refused:?}");
        assert!(
            refused[0]
                .helps
                .iter()
                .any(|help| help.contains("sivi fonts install \"Zen Maru Gothic\""))
        );
        let nowhere = load(&Choice::Dir(dir), None).unwrap_err();
        assert_eq!(nowhere[0].code, Code::FontNotInstalled);
        assert!(
            nowhere[0]
                .helps
                .iter()
                .any(|help| help.contains("SIVI_HOME"))
        );
    }

    #[test]
    fn a_family_outside_every_source_is_named_once_with_its_closest_name() {
        let (_root, dir) = theme_in("Zen Maru Gothik");
        let refused = load(&Choice::Dir(dir), None).unwrap_err();
        let codes: Vec<Code> = refused.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec![Code::FamilyUnknown], "{refused:?}");
        assert!(
            refused[0]
                .helps
                .iter()
                .any(|help| help.contains("`Zen Maru Gothic`"))
        );
        assert!(
            refused[0]
                .notes
                .iter()
                .any(|note| note.contains("sets text in it"))
        );
    }
}
