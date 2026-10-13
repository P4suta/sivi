//! The faces a theme needs and the faces its fonts provide, read with their metrics.

use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::theme::{
    FamilyName, FontStack, MAX_WEIGHT, MIN_WEIGHT, Theme, Thousandths, Weight, WeightRange,
};
use sivi_core::theme_rules;
use ttf_parser::name_id;

/// The regular weight, which code and marks without a weight are set at.
const REGULAR: u16 = 400;
const WEIGHT_AXIS: ttf_parser::Tag = ttf_parser::Tag::from_bytes(b"wght");

/// One face a font file provides.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Face {
    pub family: FamilyName,
    /// The weights the file provides: one for a static face, a span for a variable one.
    pub weights: WeightRange,
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

/// The family the face names in Unicode, as Typst reads it: the plain family record with its style words trimmed.
fn family_text(face: &ttf_parser::Face<'_>) -> Option<String> {
    face.names()
        .into_iter()
        .find(|name| name.name_id == name_id::FAMILY && name.is_unicode())
        .and_then(|name| name.to_string())
        .map(|family| String::from(typographic_family(&family)))
}

/// The family without the style words a plain family record carries, such as `Bold` or `Semi Condensed`, as Typst trims them.
///
/// The rule follows Typst's `typographic_family`, so a theme names a family exactly as Typst's font book lists it.
#[must_use]
pub fn typographic_family(family: &str) -> &str {
    /// Separators between names, modifiers, and styles.
    const SEPARATORS: [char; 3] = [' ', '-', '_'];
    /// Modifiers that appear in combination with suffixes.
    const MODIFIERS: &[&str] = &[
        "extra", "ext", "ex", "x", "semi", "sem", "sm", "demi", "dem", "ultra",
    ];
    /// Style suffixes.
    #[rustfmt::skip]
    const SUFFIXES: &[&str] = &[
        "normal", "italic", "oblique", "slanted",
        "thin", "th", "hairline", "light", "lt", "regular", "medium", "med",
        "md", "bold", "bd", "demi", "extb", "black", "blk", "bk", "heavy",
        "narrow", "condensed", "cond", "cn", "cd", "compressed", "expanded", "exp",
        "vf", "var", "variable",
    ];
    let family = family.trim().trim_start_matches('.');
    let lower = family.to_ascii_lowercase();
    let mut trimmed = lower.as_str();
    loop {
        let before = trimmed.len();
        let mut rest = trimmed;
        let mut shortened = false;
        while let Some(shorter) = SUFFIXES.iter().find_map(|suffix| rest.strip_suffix(suffix)) {
            shortened = true;
            rest = shorter;
        }
        if !shortened {
            break;
        }
        if let Some(shorter) = rest.strip_suffix(SEPARATORS) {
            trimmed = shorter;
            rest = shorter;
        }
        if let Some(shorter) = MODIFIERS
            .iter()
            .find_map(|modifier| rest.strip_suffix(modifier))
            && let Some(stripped) = shorter.strip_suffix(SEPARATORS)
        {
            trimmed = stripped;
        }
        // A style word nothing separates from the name belongs to the name, as in `Semibold` alone.
        if trimmed.len() == before {
            break;
        }
    }
    family.get(..trimmed.len()).unwrap_or(family)
}

/// Every weight a style may set, lowest first.
fn weight_scale() -> impl DoubleEndedIterator<Item = Weight> {
    (MIN_WEIGHT..=MAX_WEIGHT).filter_map(Weight::new)
}

/// The weights a face provides: the span of its weight axis within the scale, or its one static weight.
fn weights(parsed: &ttf_parser::Face<'_>) -> Option<WeightRange> {
    let axis = parsed
        .variation_axes()
        .into_iter()
        .find(|axis| axis.tag == WEIGHT_AXIS);
    match axis {
        Some(axis) => {
            let lowest =
                weight_scale().find(|weight| f32::from(weight.value()) >= axis.min_value)?;
            let highest = weight_scale()
                .rev()
                .find(|weight| f32::from(weight.value()) <= axis.max_value)?;
            Some(WeightRange { lowest, highest })
        }
        None => Weight::new(parsed.weight().to_number()).map(WeightRange::single),
    }
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
    let weights = weights(&parsed).ok_or(FaceError::Weight)?;
    Ok(Face {
        family,
        weights,
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
    /// The line height the style sets, when the style sets one and the family is the primary.
    pub line_height: Option<Thousandths>,
    /// Whether the family is a fallback of its stack, which only the glyphs of the primary lack reach.
    pub fallback: bool,
}

/// What one style wants of every family of its stack.
#[derive(Clone, Copy, Debug)]
struct Want<'a> {
    weight: Weight,
    italic: bool,
    /// The style or mark that wants the face.
    by: &'a str,
    /// The line height the style sets, which only the primary family is held to.
    line_height: Option<Thousandths>,
}

/// The needs of every family of `stack` under `want`: the primary with the line height, each fallback without.
fn stack_needs(stack: &FontStack, want: Want<'_>) -> Vec<Need> {
    let mut needs = vec![Need {
        family: stack.primary().clone(),
        weight: want.weight,
        italic: want.italic,
        by: String::from(want.by),
        line_height: want.line_height,
        fallback: false,
    }];
    for family in stack.fallbacks() {
        needs.push(Need {
            family: family.clone(),
            weight: want.weight,
            italic: want.italic,
            by: String::from(want.by),
            line_height: None,
            fallback: true,
        });
    }
    needs
}

/// Every face `theme` needs.
#[must_use]
pub fn needs(theme: &Theme) -> Vec<Need> {
    let mut needs = Vec::new();
    for (name, style) in &theme.text {
        needs.extend(stack_needs(
            &style.font,
            Want {
                weight: style.weight,
                italic: false,
                by: &format!("text.{name}"),
                line_height: Some(style.line_height),
            },
        ));
        if theme.inline.emphasis.italic {
            needs.extend(stack_needs(
                &style.font,
                Want {
                    weight: style.weight,
                    italic: true,
                    by: &format!("inline.emphasis in text.{name}"),
                    line_height: None,
                },
            ));
        }
        if let Some(weight) = theme.inline.strong.weight {
            needs.extend(stack_needs(
                &style.font,
                Want {
                    weight,
                    italic: false,
                    by: &format!("inline.strong in text.{name}"),
                    line_height: None,
                },
            ));
        }
    }
    if let Some(regular) = Weight::new(REGULAR) {
        needs.extend(stack_needs(
            &theme.code.font,
            Want {
                weight: regular,
                italic: false,
                by: "code",
                line_height: Some(theme.code.line_height),
            },
        ));
        if let Some(font) = &theme.inline.code.font {
            needs.extend(stack_needs(
                font,
                Want {
                    weight: regular,
                    italic: false,
                    by: "inline.code",
                    line_height: None,
                },
            ));
        }
    }
    needs
}

/// The diagnostic of a need no face meets: an error for a primary family, a warning for a fallback the fonts provide at other weights.
fn unmet(need: &Need, provided: &[Face]) -> Option<Diagnostic> {
    let shape = if need.italic { "italic" } else { "upright" };
    if !need.fallback {
        return Some(
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
    }
    let known = provided.iter().any(|face| face.family == need.family);
    known.then(|| {
        Diagnostic::new(
            Code::FallbackWeight,
            format!(
                "the fallback family `{}` has no face at weight {} {shape}, which `{}` sets; its nearest face stands in",
                need.family,
                need.weight.value(),
                need.by
            ),
        )
        .help(String::from(
            "add the face, or accept the nearest weight for the glyphs that reach the fallback",
        ))
    })
}

/// Every face the theme needs that `provided` lacks, and every line height below its face's line box.
#[must_use]
pub fn check(theme: &Theme, provided: &[Face]) -> Vec<Diagnostic> {
    check_except(theme, provided, &[])
}

/// As `check`, leaving out the families of `absent`, which an earlier diagnostic already named as a whole.
#[must_use]
pub fn check_except(theme: &Theme, provided: &[Face], absent: &[FamilyName]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for need in needs(theme) {
        if absent.contains(&need.family) {
            continue;
        }
        let found = provided.iter().find(|face| {
            face.family == need.family
                && face.weights.contains(need.weight)
                && face.italic == need.italic
        });
        let Some(face) = found else {
            diagnostics.extend(unmet(&need, provided));
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
    use super::{Face, FaceError, check, check_except, face, needs, typographic_family};
    use sivi_core::diagnostic::Code;
    use sivi_core::theme::{FamilyName, FontStack, Thousandths, Weight, WeightRange};

    const REGULAR: &[u8] = include_bytes!("../../../fonts/BIZUDPGothic-Regular.ttf");
    const BOLD: &[u8] = include_bytes!("../../../fonts/BIZUDPGothic-Bold.ttf");
    const MONO: &[u8] = include_bytes!("../../../fonts/JetBrainsMono-Regular.ttf");
    const DISPLAY: &[u8] = include_bytes!("../../../fonts/PlayfairDisplay-Italic[wght].ttf");

    #[test]
    fn the_family_loses_its_style_words_as_typst_trims_them() {
        assert_eq!(typographic_family("Atma Light"), "Atma");
        assert_eq!(typographic_family("eras bold"), "eras");
        assert_eq!(typographic_family("footlight mt light"), "footlight mt");
        assert_eq!(typographic_family("times new roman"), "times new roman");
        assert_eq!(
            typographic_family("noto sans mono cond sembd"),
            "noto sans mono"
        );
        assert_eq!(typographic_family("noto serif SEMCOND sembd"), "noto serif");
        assert_eq!(
            typographic_family("Rounded Mplus 1c Bold"),
            "Rounded Mplus 1c"
        );
        assert_eq!(typographic_family("WDXL 滑油字 JP N"), "WDXL 滑油字 JP N");
        assert_eq!(typographic_family(" .Apple Thin "), "Apple");
        assert_eq!(typographic_family("Foo Bold Italic"), "Foo");
        assert_eq!(typographic_family("Foo-Semibold"), "Foo");
        assert_eq!(typographic_family("Semibold"), "Semibold");
        assert_eq!(typographic_family("Bold"), "Bold");
        let display = face(DISPLAY).unwrap();
        assert_eq!(display.family.as_str(), "Playfair Display");
        assert_eq!(
            display.weights,
            WeightRange {
                lowest: Weight::new(400).unwrap(),
                highest: Weight::new(900).unwrap()
            }
        );
        assert!(display.italic);
    }

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
        assert_eq!(faces[0].weights.lowest.value(), 400);
        assert_eq!(
            faces[1].weights,
            WeightRange::single(Weight::new(700).unwrap())
        );
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
            weights: WeightRange::single(Weight::new(400).unwrap()),
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

    #[test]
    #[cfg_attr(
        miri,
        ignore = "the bundled fonts and the plain theme take minutes under Miri"
    )]
    fn an_absent_family_is_left_out_of_the_face_check_and_a_fallback_weight_is_a_warning() {
        let mut theme = crate::parse(crate::PLAIN).unwrap();
        let gothic = FamilyName::new("BIZ UDPGothic").unwrap();
        let without = check_except(
            &theme,
            &[face(MONO).unwrap()],
            std::slice::from_ref(&gothic),
        );
        assert_eq!(without, Vec::new());
        let with = check_except(&theme, &[face(MONO).unwrap()], &[]);
        assert!(with.iter().all(|d| d.code == Code::MissingFace));
        assert_ne!(with.len(), 0);
        let mono = FamilyName::new("JetBrains Mono").unwrap();
        for (_, style) in &mut theme.text {
            style.font = FontStack::new(vec![gothic.clone(), mono.clone()]).unwrap();
        }
        let found = check(&theme, &bundled());
        assert!(
            found
                .iter()
                .all(|d| d.code == Code::FallbackWeight && d.message.contains("JetBrains Mono")),
            "{found:?}"
        );
        assert_ne!(found.len(), 0);
        let wanted = needs(&theme);
        assert!(
            wanted
                .iter()
                .any(|need| need.fallback && need.line_height.is_none())
        );
        let nowhere = FamilyName::new("Nowhere").unwrap();
        for (_, style) in &mut theme.text {
            style.font = FontStack::new(vec![gothic.clone(), nowhere.clone()]).unwrap();
        }
        // A fallback no font provides at any weight is the catalog check's finding, not a face warning.
        assert_eq!(check(&theme, &bundled()), Vec::new());
    }
}
