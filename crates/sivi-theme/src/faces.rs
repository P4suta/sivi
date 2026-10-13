//! The faces a theme needs and the faces its fonts provide, read with their metrics.

use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::theme::{FamilyName, Theme, Thousandths, Weight};
use sivi_core::theme_rules;
use ttf_parser::name_id;

/// The regular weight, which code and marks without a weight are set at.
const REGULAR: u16 = 400;

/// One face a font file provides.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Face {
    pub family: FamilyName,
    pub weight: Weight,
    pub italic: bool,
    pub ascender: i16,
    pub descender: i16,
    pub units_per_em: u16,
}

/// Why a font file could not be read as a face.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FaceError {
    /// Not a font file this reader understands.
    Unreadable,
    /// The font names no family in Unicode.
    NoFamily,
    /// The font declares a weight outside one hundred to nine hundred.
    Weight,
}

/// The family the face names in Unicode, preferring the typographic family over the plain one.
fn family_text(face: &ttf_parser::Face<'_>) -> Option<String> {
    let names = face.names();
    let preferred = names
        .into_iter()
        .find(|name| name.name_id == name_id::TYPOGRAPHIC_FAMILY && name.is_unicode())
        .and_then(|name| name.to_string());
    let plain = names
        .into_iter()
        .find(|name| name.name_id == name_id::FAMILY && name.is_unicode())
        .and_then(|name| name.to_string());
    preferred.or(plain)
}

/// The face `bytes` holds, with the metrics the line box rule reads.
///
/// # Errors
///
/// Returns why the bytes are not a face.
pub fn face(bytes: &[u8]) -> Result<Face, FaceError> {
    let parsed = ttf_parser::Face::parse(bytes, 0).map_err(|_unreadable| FaceError::Unreadable)?;
    let text = family_text(&parsed).ok_or(FaceError::NoFamily)?;
    let family = FamilyName::new(&text).map_err(|_invalid| FaceError::NoFamily)?;
    let weight = Weight::new(parsed.weight().to_number()).ok_or(FaceError::Weight)?;
    Ok(Face {
        family,
        weight,
        italic: parsed.is_italic(),
        ascender: parsed
            .typographic_ascender()
            .unwrap_or_else(|| parsed.ascender()),
        descender: parsed
            .typographic_descender()
            .unwrap_or_else(|| parsed.descender()),
        units_per_em: parsed.units_per_em(),
    })
}

/// One face a theme needs: a family at a weight, upright or italic, named by the style that needs it.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Need {
    pub family: FamilyName,
    pub weight: Weight,
    pub italic: bool,
    /// The style or mark that needs the face, for the diagnostic.
    pub by: String,
    /// The line height the style sets, when the style sets one.
    pub line_height: Option<Thousandths>,
}

/// Every face `theme` needs.
#[must_use]
pub fn needs(theme: &Theme) -> Vec<Need> {
    let mut needs = Vec::new();
    for (name, style) in &theme.text {
        needs.push(Need {
            family: style.font.clone(),
            weight: style.weight,
            italic: false,
            by: format!("text.{name}"),
            line_height: Some(style.line_height),
        });
        if theme.inline.emphasis.italic {
            needs.push(Need {
                family: style.font.clone(),
                weight: style.weight,
                italic: true,
                by: format!("inline.emphasis in text.{name}"),
                line_height: None,
            });
        }
        if let Some(weight) = theme.inline.strong.weight {
            needs.push(Need {
                family: style.font.clone(),
                weight,
                italic: false,
                by: format!("inline.strong in text.{name}"),
                line_height: None,
            });
        }
    }
    if let Some(regular) = Weight::new(REGULAR) {
        needs.push(Need {
            family: theme.code.font.clone(),
            weight: regular,
            italic: false,
            by: String::from("code"),
            line_height: Some(theme.code.line_height),
        });
        if let Some(font) = &theme.inline.code.font {
            needs.push(Need {
                family: font.clone(),
                weight: regular,
                italic: false,
                by: String::from("inline.code"),
                line_height: None,
            });
        }
    }
    needs
}

/// Every face the theme needs that `provided` lacks, and every line height below its face's line box.
#[must_use]
pub fn check(theme: &Theme, provided: &[Face]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for need in needs(theme) {
        let found = provided.iter().find(|face| {
            face.family == need.family && face.weight == need.weight && face.italic == need.italic
        });
        let Some(face) = found else {
            let shape = if need.italic { "italic" } else { "upright" };
            diagnostics.push(
                Diagnostic::new(
                    Code::MissingFace,
                    format!(
                        "no font provides `{}` at weight {} {shape}, which `{}` needs",
                        need.family,
                        need.weight.value(),
                        need.by
                    ),
                )
                .help(String::from(
                    "add a font file with that face under `[[fonts]]`, or set the style to a face the fonts provide",
                )),
            );
            continue;
        };
        if let Some(line_height) = need.line_height
            && !theme_rules::line_box_fits(
                line_height,
                face.ascender,
                face.descender,
                face.units_per_em,
            )
        {
            diagnostics.push(
                Diagnostic::new(
                    Code::LineHeightBelowFace,
                    format!(
                        "the line height of `{}` is below the natural line box of `{}`",
                        need.by, need.family
                    ),
                )
                .note(format!(
                    "the face spans {} to {} of {} units per em",
                    face.ascender, face.descender, face.units_per_em
                ))
                .help(String::from(
                    "raise `line_height` until the lines no longer touch",
                )),
            );
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::{Face, FaceError, check, face, needs};
    use sivi_core::diagnostic::Code;
    use sivi_core::theme::{FamilyName, Thousandths, Weight};

    const REGULAR: &[u8] = include_bytes!("../../../fonts/BIZUDPGothic-Regular.ttf");
    const BOLD: &[u8] = include_bytes!("../../../fonts/BIZUDPGothic-Bold.ttf");
    const MONO: &[u8] = include_bytes!("../../../fonts/JetBrainsMono-Regular.ttf");

    fn bundled() -> Vec<Face> {
        vec![
            face(REGULAR).unwrap(),
            face(BOLD).unwrap(),
            face(MONO).unwrap(),
        ]
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "the bundled fonts and the plain theme take minutes under Miri"
    )]
    fn the_bundled_fonts_read_as_the_faces_the_plain_theme_needs() {
        let faces = bundled();
        assert_eq!(faces[0].family.as_str(), "BIZ UDPGothic");
        assert_eq!(faces[0].weight.value(), 400);
        assert_eq!(faces[1].weight.value(), 700);
        assert!(!faces[1].italic);
        assert_eq!(faces[2].family.as_str(), "JetBrains Mono");
        assert!(faces[2].units_per_em > 0);
        assert!(faces[2].ascender > faces[2].descender);
        assert_eq!(face(b"not a font"), Err(FaceError::Unreadable));
        let theme = crate::parse(crate::PLAIN).unwrap();
        let wanted = needs(&theme);
        assert!(
            wanted
                .iter()
                .any(|need| need.family.as_str() == "JetBrains Mono")
        );
        assert!(wanted.iter().all(|need| !need.italic));
        assert_eq!(check(&theme, &faces), Vec::new());
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "the bundled fonts and the plain theme take minutes under Miri"
    )]
    fn a_missing_face_and_a_tight_line_height_are_named() {
        let theme = crate::parse(crate::PLAIN).unwrap();
        let only_regular = vec![face(REGULAR).unwrap()];
        let found = check(&theme, &only_regular);
        assert!(
            found
                .iter()
                .any(|d| d.code == Code::MissingFace && d.message.contains("700"))
        );
        assert!(
            found
                .iter()
                .any(|d| d.code == Code::MissingFace && d.message.contains("JetBrains Mono"))
        );
        let mut squashed = bundled();
        for face in &mut squashed {
            face.ascender = i16::MAX;
            face.descender = i16::MIN;
        }
        let tight = check(&theme, &squashed);
        assert!(tight.iter().all(|d| d.code == Code::LineHeightBelowFace));
        assert_ne!(tight.len(), 0);
        let fake = Face {
            family: FamilyName::new("Other").unwrap(),
            weight: Weight::new(400).unwrap(),
            italic: false,
            ascender: 800,
            descender: -200,
            units_per_em: 1000,
        };
        assert!(sivi_core::theme_rules::line_box_fits(
            Thousandths(1000),
            fake.ascender,
            fake.descender,
            fake.units_per_em
        ));
    }
}
