//! The faces and line boxes of the bundled fonts and of a theme's files, as the theme check and the emitter read them.

use sivi_core::emit::LineBoxes;
use sivi_core::theme::Thousandths;
use sivi_theme::faces::{self, Face, FaceError};
use sivi_typst::fonts::BUNDLED;

const THOUSAND: i64 = 1000;

/// Why a font file could not be read as a face, for a person.
#[must_use]
pub const fn face_error(error: FaceError) -> &'static str {
    match error {
        FaceError::Unreadable => "not a font file",
        FaceError::NoFamily => "a font file that names no family",
        FaceError::Weight => "a font file whose weight lies outside 100 to 900",
    }
}

/// The faces of the bundled fonts.
///
/// # Errors
///
/// Returns which bundled file does not parse, which is a defect of the build.
pub fn bundled_faces() -> Result<Vec<Face>, String> {
    BUNDLED
        .iter()
        .map(|font| {
            faces::face(font.bytes)
                .map_err(|error| format!("the bundled font {} is {}", font.file, face_error(error)))
        })
        .collect()
}

/// The natural line box of a face as a multiple of its size, in thousandths.
fn line_box(face: &Face) -> Thousandths {
    let units = i64::from(face.ascender).saturating_sub(i64::from(face.descender));
    Thousandths(
        units
            .saturating_mul(THOUSAND)
            .checked_div(i64::from(face.units_per_em))
            .unwrap_or(THOUSAND),
    )
}

/// The line box of each family, from the first face listed for it.
#[must_use]
pub fn line_boxes(faces: &[Face]) -> LineBoxes {
    let mut boxes = Vec::new();
    for face in faces {
        if boxes.iter().any(|(family, _)| family == &face.family) {
            continue;
        }
        boxes.push((face.family.clone(), line_box(face)));
    }
    LineBoxes::new(boxes)
}

#[cfg(test)]
mod tests {
    use super::{bundled_faces, line_boxes};
    use sivi_core::theme::{FamilyName, Thousandths};

    #[test]
    fn the_bundled_fonts_describe_two_families_with_line_boxes_of_at_least_one_em() {
        let faces = bundled_faces().unwrap();
        assert_eq!(faces.len(), 3);
        let boxes = line_boxes(&faces);
        let gothic = boxes.of(&FamilyName::new("BIZ UDPGothic").unwrap());
        let mono = boxes.of(&FamilyName::new("JetBrains Mono").unwrap());
        assert!(gothic >= Thousandths::ONE, "{gothic:?}");
        assert!(mono > Thousandths::ONE, "{mono:?}");
        assert_eq!(
            boxes.of(&FamilyName::new("Nowhere").unwrap()),
            Thousandths::ONE
        );
    }
}
