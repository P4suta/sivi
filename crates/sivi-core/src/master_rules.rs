//! Which master a slide uses: its own `master:`, else the deck's default, else none; and the closest name to a misspelling.

use crate::front_matter_rules;
use crate::text::Identifier;

/// Where a slide's master comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Resolution<'a> {
    /// The slide names its master.
    Named(&'a Identifier),
    /// The deck's default stands in.
    Defaulted(&'a Identifier),
    /// Neither the slide nor the deck names one.
    Missing,
}

/// The master a slide uses, given its own `master:` and the deck's `default_master`.
#[must_use]
pub const fn resolve<'a>(
    own: Option<&'a Identifier>,
    default: Option<&'a Identifier>,
) -> Resolution<'a> {
    match (own, default) {
        (Some(name), _) => Resolution::Named(name),
        (None, Some(name)) => Resolution::Defaulted(name),
        (None, None) => Resolution::Missing,
    }
}

/// The most edits between a misspelling and the name it suggests.
pub const MAX_SUGGESTION_DISTANCE: usize = 2;

/// The cells of one row of the edit distance: one per byte of a name and one for the empty prefix.
const ROW: usize = front_matter_rules::MAX_IDENTIFIER + 1;

/// The Levenshtein distance between two names: how many insertions, deletions, and replacements turn one into the other.
#[must_use]
pub fn distance(a: &Identifier, b: &Identifier) -> usize {
    edits(a.as_str().as_bytes(), b.as_str().as_bytes()).unwrap_or(usize::MAX)
}

/// The Levenshtein distance between `a` and `b`, or `None` when either holds more bytes than a name may.
///
/// A cost never exceeds the longer side, so a byte holds every cell of the rows.
fn edits(a: &[u8], b: &[u8]) -> Option<usize> {
    if a.len() >= ROW || b.len() >= ROW {
        return None;
    }
    let mut previous = [0u8; ROW];
    let mut base = 0u8;
    for cell in previous.iter_mut().take(b.len().saturating_add(1)) {
        *cell = base;
        base = base.saturating_add(1);
    }
    let mut dropped = 0u8;
    for left in a {
        dropped = dropped.saturating_add(1);
        let mut current = [0u8; ROW];
        if let Some(first) = current.first_mut() {
            *first = dropped;
        }
        for (column, right) in b.iter().enumerate() {
            let replace = previous
                .get(column)
                .map_or(u8::MAX, |cost| cost.saturating_add(u8::from(left != right)));
            let delete = previous
                .get(column.saturating_add(1))
                .map_or(u8::MAX, |cost| cost.saturating_add(1));
            let insert = current
                .get(column)
                .map_or(u8::MAX, |cost| cost.saturating_add(1));
            if let Some(cell) = current.get_mut(column.saturating_add(1)) {
                *cell = replace.min(delete).min(insert);
            }
        }
        previous = current;
    }
    previous.get(b.len()).map(|cost| usize::from(*cost))
}

/// The known name closest to `wanted` within two edits, when there is exactly one closest.
#[must_use]
pub fn closest<'a>(wanted: &Identifier, known: &'a [Identifier]) -> Option<&'a Identifier> {
    let mut best: Option<(&Identifier, usize)> = None;
    let mut tied = false;
    for candidate in known {
        let far = distance(wanted, candidate);
        if far > MAX_SUGGESTION_DISTANCE {
            continue;
        }
        match best {
            Some((_, nearest)) if far > nearest => {}
            Some((_, nearest)) if far == nearest => tied = true,
            Some(_) | None => {
                best = Some((candidate, far));
                tied = false;
            }
        }
    }
    match best {
        Some((candidate, _)) if !tied => Some(candidate),
        Some(_) | None => None,
    }
}

/// The properties Kani proves for every short name and the tests check over the names they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::edits;

    /// The most bytes the bounded proofs admit for a name.
    #[cfg(kani)]
    const PROOF_BYTES: usize = 4;

    /// The distance is found, zero exactly for equal names, symmetric, at least the length difference, and at most the longer length.
    pub(super) fn spec_edits(a: &[u8], b: &[u8]) -> Option<usize> {
        let far = edits(a, b);
        assert_eq!(far, edits(b, a));
        assert_eq!(far == Some(0), a == b);
        let (left, right) = (a.len(), b.len());
        assert!(far.is_some_and(|far| far <= left.max(right)));
        assert!(far.is_some_and(|far| far >= left.abs_diff(right)));
        far
    }

    /// Every pair of names of up to `PROOF_BYTES` bytes: the bytes and the lengths are symbolic, the storage is fixed on the stack.
    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(6)]
    fn an_edit_distance_is_zero_only_between_equal_names_symmetric_and_bounded() {
        let a: [u8; PROOF_BYTES] = kani::any();
        let b: [u8; PROOF_BYTES] = kani::any();
        let a_len: usize = kani::any();
        let b_len: usize = kani::any();
        kani::assume(a_len <= PROOF_BYTES && b_len <= PROOF_BYTES);
        let (Some(left), Some(right)) = (a.get(..a_len), b.get(..b_len)) else {
            return;
        };
        let far = spec_edits(left, right);
        kani::cover!(far == Some(0) && a_len == PROOF_BYTES);
        kani::cover!(far == Some(PROOF_BYTES));
        kani::cover!(far == Some(1));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_different_names_as_zero_edits_apart() {
        kani::assert(edits(b"xy", b"yx") == Some(0), "NEGATIVE_MASTER_CONTROL");
    }
}

#[cfg(test)]
mod tests {
    use super::{ROW, Resolution, closest, distance, edits, proofs, resolve};
    use crate::front_matter_rules::MAX_IDENTIFIER;
    use crate::text::Identifier;
    use alloc::vec::Vec;

    fn wanted(name: &str) -> Identifier {
        Identifier::new(name).unwrap()
    }

    fn names(list: &[&str]) -> Vec<Identifier> {
        list.iter()
            .map(|name| Identifier::new(name).unwrap())
            .collect()
    }

    #[test]
    fn a_slide_uses_its_own_master_then_the_default_then_none() {
        let own = Identifier::new("code").unwrap();
        let default = Identifier::new("bullets").unwrap();
        assert_eq!(resolve(Some(&own), Some(&default)), Resolution::Named(&own));
        assert_eq!(resolve(Some(&own), None), Resolution::Named(&own));
        assert_eq!(
            resolve(None, Some(&default)),
            Resolution::Defaulted(&default)
        );
        assert_eq!(resolve(None, None), Resolution::Missing);
    }

    #[test]
    fn the_edit_distance_counts_insertions_deletions_and_replacements() {
        let far = |a: &str, b: &str| distance(&wanted(a), &wanted(b));
        assert_eq!(far("bullets", "bullets"), 0);
        assert_eq!(far("bullet", "bullets"), 1);
        assert_eq!(far("bulets", "bullets"), 1);
        assert_eq!(far("ballets", "bullets"), 1);
        assert_eq!(far("xy", "yx"), 2);
        assert_eq!(far("kitten", "sitting"), 3);
        assert_eq!(far("abc", "a"), 2);
        assert_eq!(far("a", "abc"), 2);
        assert_eq!(edits(b"abc", b""), Some(3));
        for (a, b) in [
            (&b""[..], &b""[..]),
            (b"a", b""),
            (b"a", b"b"),
            (b"xy", b"yx"),
            (b"xyz", b"xyw"),
            (b"figure", b"figures"),
        ] {
            proofs::spec_edits(a, b);
        }
    }

    #[test]
    fn a_name_beyond_the_row_has_no_distance() {
        let fits = [b'a'; MAX_IDENTIFIER];
        let beyond = [b'a'; ROW];
        assert_eq!(edits(&fits, b""), Some(MAX_IDENTIFIER));
        assert_eq!(edits(b"", &fits), Some(MAX_IDENTIFIER));
        assert_eq!(edits(&fits, &fits), Some(0));
        assert_eq!(edits(&beyond, b"a"), None);
        assert_eq!(edits(b"a", &beyond), None);
    }

    #[test]
    fn the_closest_known_name_is_suggested_only_when_it_is_near_and_alone() {
        let known = names(&["title", "bullets", "bullet-list", "code", "cod"]);
        assert_eq!(
            closest(&wanted("bullet"), &known).map(Identifier::as_str),
            Some("bullets")
        );
        assert_eq!(
            closest(&wanted("titles"), &known).map(Identifier::as_str),
            Some("title")
        );
        assert_eq!(
            closest(&wanted("tie"), &known).map(Identifier::as_str),
            Some("title")
        );
        assert_eq!(
            closest(&wanted("co"), &known).map(Identifier::as_str),
            Some("cod")
        );
        let tied = names(&["ab", "ac"]);
        assert_eq!(closest(&wanted("ad"), &tied), None);
        assert_eq!(
            closest(&wanted("ab"), &tied).map(Identifier::as_str),
            Some("ab")
        );
        assert_eq!(closest(&wanted("quote"), &known), None);
        assert_eq!(
            closest(&wanted("code"), &known).map(Identifier::as_str),
            Some("code")
        );
        assert_eq!(closest(&wanted("x"), &[]), None);
    }
}
