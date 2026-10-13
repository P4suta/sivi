//! What a soft line break in a paragraph becomes: a space between most characters, nothing between two East Asian ones.
//!
//! Markdown authors wrap Japanese prose freely, and a space inside a Japanese sentence would be a visible defect.

/// What replaces a soft line break.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Joiner {
    /// The two characters join directly.
    Nothing,
    /// One space separates them.
    Space,
}

const CJK_SYMBOLS_AND_PUNCTUATION: (u32, u32) = (0x3000, 0x303F);
const HIRAGANA: (u32, u32) = (0x3040, 0x309F);
const KATAKANA: (u32, u32) = (0x30A0, 0x30FF);
const KATAKANA_PHONETIC_EXTENSIONS: (u32, u32) = (0x31F0, 0x31FF);
const CJK_UNIFIED_IDEOGRAPHS_EXTENSION_A: (u32, u32) = (0x3400, 0x4DBF);
const CJK_UNIFIED_IDEOGRAPHS: (u32, u32) = (0x4E00, 0x9FFF);
const HANGUL_SYLLABLES: (u32, u32) = (0xAC00, 0xD7AF);
const CJK_COMPATIBILITY_IDEOGRAPHS: (u32, u32) = (0xF900, 0xFAFF);
const HALFWIDTH_AND_FULLWIDTH_FORMS: (u32, u32) = (0xFF00, 0xFFEF);
const CJK_UNIFIED_IDEOGRAPHS_EXTENSIONS_B_TO_I: (u32, u32) = (0x20000, 0x323AF);

/// The code point ranges whose characters join without a space.
pub const EAST_ASIAN_RANGES: [(u32, u32); 10] = [
    CJK_SYMBOLS_AND_PUNCTUATION,
    HIRAGANA,
    KATAKANA,
    KATAKANA_PHONETIC_EXTENSIONS,
    CJK_UNIFIED_IDEOGRAPHS_EXTENSION_A,
    CJK_UNIFIED_IDEOGRAPHS,
    HANGUL_SYLLABLES,
    CJK_COMPATIBILITY_IDEOGRAPHS,
    HALFWIDTH_AND_FULLWIDTH_FORMS,
    CJK_UNIFIED_IDEOGRAPHS_EXTENSIONS_B_TO_I,
];

/// Whether `character` is written without spaces between words: Han, kana, Hangul, and their punctuation.
#[must_use]
pub fn is_east_asian(character: char) -> bool {
    let code = u32::from(character);
    EAST_ASIAN_RANGES
        .iter()
        .any(|(first, last)| code >= *first && code <= *last)
}

/// What a soft break between `before` and `after` becomes.
#[must_use]
pub fn joiner(before: char, after: char) -> Joiner {
    if is_east_asian(before) && is_east_asian(after) {
        Joiner::Nothing
    } else {
        Joiner::Space
    }
}

/// The properties Kani proves for every pair of characters and the tests check over the pairs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{EAST_ASIAN_RANGES, Joiner, is_east_asian, joiner};

    pub(super) fn spec_joiner(before: char, after: char) {
        let verdict = joiner(before, after);
        assert_eq!(
            verdict == Joiner::Nothing,
            is_east_asian(before) && is_east_asian(after)
        );
        assert_eq!(verdict, joiner(after, before));
    }

    pub(super) fn spec_east_asian(character: char) {
        let code = u32::from(character);
        let in_a_range = EAST_ASIAN_RANGES
            .iter()
            .any(|(first, last)| code >= *first && code <= *last);
        assert_eq!(is_east_asian(character), in_a_range);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn a_soft_break_vanishes_exactly_between_two_east_asian_characters() {
        let before: char = kani::any();
        let after: char = kani::any();
        spec_east_asian(before);
        spec_joiner(before, after);
        kani::cover!(joiner(before, after) == Joiner::Nothing);
        kani::cover!(joiner(before, after) == Joiner::Space && is_east_asian(before));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_joining_latin_words_without_a_space() {
        kani::assert(
            joiner('a', 'b') == Joiner::Nothing,
            "NEGATIVE_SOFTBREAK_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Joiner, is_east_asian, joiner, proofs};

    #[test]
    fn every_listed_pair_meets_the_specification() {
        let samples = [
            'a',
            'Z',
            ' ',
            '。',
            '、',
            'あ',
            'ン',
            'ー',
            '漢',
            '字',
            // A Hangul syllable, spelled by code point because this machine writes no Korean.
            '\u{D55C}',
            'Ａ',
            '𠀋',
            '\u{2FFF}',
            '\u{32400}',
            '\u{3000}',
            '\u{303F}',
            '\u{FFEF}',
            '\u{FFF0}',
        ];
        for before in samples {
            proofs::spec_east_asian(before);
            for after in samples {
                proofs::spec_joiner(before, after);
            }
        }
    }

    #[test]
    fn japanese_prose_joins_and_latin_prose_keeps_its_space() {
        assert_eq!(joiner('る', '。'), Joiner::Nothing);
        assert_eq!(joiner('漢', 'あ'), Joiner::Nothing);
        assert_eq!(joiner('a', 'b'), Joiner::Space);
        assert_eq!(joiner('語', 'a'), Joiner::Space);
        assert_eq!(joiner('a', '語'), Joiner::Space);
        assert!(is_east_asian('ー'));
        assert!(!is_east_asian('-'));
    }
}
