//! Loading the theme a deck names or the command line chooses: the built-in theme, or a directory with `theme.toml` and its font files.

use crate::digest::sha256_hex;
use crate::fonts::{self, face_error};
use crate::io::{self, ReadError, ReadErrorKind};
use sivi_core::diagnostic::{Code, Diagnostic, has_error};
use sivi_core::text::ThemeRef;
use sivi_core::theme::{FontFile, Theme};
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

/// Loads and checks the chosen theme, with the warnings it earned.
///
/// # Errors
///
/// Returns every diagnostic when the theme cannot be read, does not validate, or lacks a font or face it needs.
pub fn load(choice: &Choice) -> Result<(LoadedTheme, Vec<Diagnostic>), Vec<Diagnostic>> {
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
    diagnostics.extend(faces::check(&loaded.theme, &loaded.faces));
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
        let (loaded, warnings) = load(&Choice::Builtin(String::from("plain"))).unwrap();
        assert_eq!(loaded.label, "plain");
        assert_eq!(loaded.faces.len(), 3);
        assert_eq!(warnings, Vec::new());
        let refused = load(&Choice::Builtin(String::from("nope"))).unwrap_err();
        assert_eq!(refused[0].code, Code::ThemeToml);
        let unreadable = load(&Choice::Dir(PathBuf::from("/nonexistent/sivi"))).unwrap_err();
        assert_eq!(unreadable[0].code, Code::ThemeToml);
    }
}
