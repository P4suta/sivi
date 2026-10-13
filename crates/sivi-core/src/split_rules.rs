//! Where content that overflows its region could be split so that the first part fits.
//!
//! The admitted inputs are cumulative heights that never decrease, as [`crate::measure::Prefixes`] guarantees.

use crate::fit_rules::{Fit, fit};
use crate::length::Length;

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Where the content can be split.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SplitVerdict {
    /// Every part fits, so nothing needs to move.
    AllFit,
    /// The parts up to and including this index fit, and the next one does not.
    SplitAfter(usize),
    /// Not even the first part fits.
    NothingFits,
}

/// The split that keeps the most parts in place: the last prefix that fits within `available`.
///
/// Verus proves for prefixes of any length that the verdict names the longest fitting prefix.
#[must_use]
#[cfg_attr(verus_keep_ghost, verus_spec(verdict =>
    requires
        forall|i: int| 0 <= i < prefixes.len() ==> crate::fit_rules::admits(#[trigger] prefixes[i], available),
    ensures
        match verdict {
            SplitVerdict::AllFit => forall|i: int| 0 <= i < prefixes.len() ==> crate::fit_rules::fits(#[trigger] prefixes[i], available, epsilon),
            SplitVerdict::SplitAfter(last) => last + 1 < prefixes.len()
                && (forall|i: int| 0 <= i <= last ==> crate::fit_rules::fits(#[trigger] prefixes[i], available, epsilon))
                && !crate::fit_rules::fits(prefixes[last + 1], available, epsilon),
            SplitVerdict::NothingFits => prefixes.len() > 0 && !crate::fit_rules::fits(prefixes[0], available, epsilon),
        },
))]
#[expect(
    clippy::indexing_slicing,
    reason = "Verus proves the index below the length from the loop condition, and the proof runs in `cargo xtask proofs`"
)]
pub const fn split_point(prefixes: &[Length], available: Length, epsilon: Length) -> SplitVerdict {
    let mut index: usize = 0;
    let mut end = prefixes.len();
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= prefixes.len(),
            end <= prefixes.len(),
            forall|i: int| 0 <= i < prefixes.len() ==> crate::fit_rules::admits(#[trigger] prefixes[i], available),
            forall|i: int| 0 <= i < index && i < end ==> crate::fit_rules::fits(#[trigger] prefixes[i], available, epsilon),
            end < prefixes.len() ==> !crate::fit_rules::fits(prefixes[end as int], available, epsilon),
        decreases prefixes.len() - index,
    ))]
    while index < end {
        if let Fit::Overflows { .. } = fit(prefixes[index], available, epsilon) {
            end = index;
        }
        index = index.wrapping_add(1);
    }
    if end == prefixes.len() {
        SplitVerdict::AllFit
    } else if end == 0 {
        SplitVerdict::NothingFits
    } else {
        SplitVerdict::SplitAfter(end.wrapping_sub(1))
    }
}

/// The properties Kani proves for every admitted input and the tests check over the inputs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{SplitVerdict, split_point};
    use crate::fit_rules::{Fit, fit};
    use crate::length::Length;

    pub(super) fn spec_split(prefixes: &[Length], available: Length, epsilon: Length) {
        let fits = |index: usize| {
            prefixes
                .get(index)
                .is_some_and(|prefix| fit(*prefix, available, epsilon) == Fit::Fits)
        };
        match split_point(prefixes, available, epsilon) {
            SplitVerdict::AllFit => {
                assert!(
                    prefixes
                        .last()
                        .is_none_or(|last| fit(*last, available, epsilon) == Fit::Fits)
                );
            }
            SplitVerdict::SplitAfter(last) => {
                assert!(fits(last));
                assert!(!fits(last.saturating_add(1)));
                assert!(last.saturating_add(1) < prefixes.len());
            }
            SplitVerdict::NothingFits => {
                assert_ne!(prefixes.len(), 0);
                assert!(!fits(0));
            }
        }
    }

    #[cfg(kani)]
    const PARTS: usize = 6;

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(8)]
    fn the_split_keeps_the_longest_fitting_prefix() {
        let raw: [i64; PARTS] = kani::any();
        let count: usize = kani::any();
        kani::assume(count <= PARTS);
        let mut prefixes = [Length::ZERO; PARTS];
        let mut index = 0;
        let mut previous = i64::MIN;
        while index < count {
            let value = raw[index];
            kani::assume(value >= previous);
            prefixes[index] = Length::from_millipoints(value);
            previous = value;
            index += 1;
        }
        let available = Length::from_millipoints(kani::any());
        let epsilon: i64 = kani::any();
        kani::assume(epsilon >= 0);
        let epsilon = Length::from_millipoints(epsilon);
        let verdict = split_point(&prefixes[..count], available, epsilon);
        spec_split(&prefixes[..count], available, epsilon);
        kani::cover!(verdict == SplitVerdict::AllFit);
        kani::cover!(verdict == SplitVerdict::NothingFits);
        kani::cover!(matches!(verdict, SplitVerdict::SplitAfter(_)));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_split_that_is_not_the_longest_fitting_prefix() {
        let prefixes = [
            Length::from_millipoints(10),
            Length::from_millipoints(20),
            Length::from_millipoints(30),
        ];
        let verdict = split_point(&prefixes, Length::from_millipoints(25), Length::ZERO);
        kani::assert(
            verdict == SplitVerdict::SplitAfter(0),
            "NEGATIVE_SPLIT_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{SplitVerdict, proofs, split_point};
    use crate::length::{EPSILON, Length};
    use alloc::vec::Vec;

    fn millipoints(values: &[i64]) -> Vec<Length> {
        values
            .iter()
            .map(|value| Length::from_millipoints(*value))
            .collect()
    }

    #[test]
    fn every_listed_prefix_list_meets_the_split_specification() {
        let lists: [&[i64]; 6] = [
            &[],
            &[10],
            &[10, 20],
            &[10, 20, 30],
            &[0, 0, 0],
            &[5, 5, 100],
        ];
        for list in lists {
            let prefixes = millipoints(list);
            for available in [-1, 0, 5, 10, 15, 25, 30, 100] {
                for epsilon in [0, 10] {
                    proofs::spec_split(
                        &prefixes,
                        Length::from_millipoints(available),
                        Length::from_millipoints(epsilon),
                    );
                }
            }
        }
    }

    #[test]
    fn the_split_falls_after_the_last_part_that_fits() {
        let prefixes = millipoints(&[100_000, 200_000, 300_000, 400_000]);
        let unit = |points: i64| Length::from_points(points).unwrap();
        assert_eq!(
            split_point(&prefixes, unit(250), EPSILON),
            SplitVerdict::SplitAfter(1)
        );
        assert_eq!(
            split_point(&prefixes, unit(400), EPSILON),
            SplitVerdict::AllFit
        );
        assert_eq!(
            split_point(&prefixes, unit(50), EPSILON),
            SplitVerdict::NothingFits
        );
        assert_eq!(
            split_point(&prefixes, unit(100), EPSILON),
            SplitVerdict::SplitAfter(0)
        );
        assert_eq!(split_point(&[], unit(0), EPSILON), SplitVerdict::AllFit);
    }
}
