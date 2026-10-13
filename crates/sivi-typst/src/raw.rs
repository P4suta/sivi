//! The languages the pinned Typst highlights, which a code fence may name after its opening.

use typst::text::RawElem;

/// Whether Typst highlights `language`, by the name or an extension of a syntax it ships.
#[must_use]
pub fn knows_language(language: &str) -> bool {
    RawElem::languages().iter().any(|(name, extensions)| {
        name.eq_ignore_ascii_case(language)
            || extensions
                .iter()
                .any(|extension| extension.eq_ignore_ascii_case(language))
    })
}

#[cfg(test)]
mod tests {
    use super::knows_language;

    #[test]
    fn the_common_fences_are_known_and_a_made_up_one_is_not() {
        for language in ["rust", "rs", "python", "py", "typ", "json", "toml", "sh"] {
            assert!(knows_language(language), "{language}");
        }
        assert!(!knows_language("klingon"));
        assert!(!knows_language(""));
    }
}
