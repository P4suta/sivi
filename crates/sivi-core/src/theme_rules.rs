//! The structural decisions over a theme's masters: section numbering, follows chains, backdrops, and line boxes.

use crate::ir::BlockKind;
use crate::theme::{Binding, BlockKindSet, Thousandths};

/// Why a slot cannot lie behind the others.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BackdropFault {
    /// The slot binds a heading, which a backdrop never holds.
    Binds,
    /// The slot accepts something other than exactly an image.
    Accepts,
    /// The slot holds more or fewer than one block.
    Blocks,
}

/// How many blocks a backdrop holds: one image.
pub const BACKDROP_BLOCKS: u32 = 1;

/// The first reason a slot with `binds`, `accepts`, and `blocks` cannot be a backdrop, or `None` when it can.
///
/// A backdrop is a flow slot that accepts exactly one image, so the content over it never measures against it.
#[must_use]
pub fn backdrop_fault(
    binds: Binding,
    accepts: BlockKindSet,
    blocks: Option<u32>,
) -> Option<BackdropFault> {
    if binds != Binding::Flow {
        return Some(BackdropFault::Binds);
    }
    if accepts != BlockKindSet::EMPTY.with(BlockKind::Image) {
        return Some(BackdropFault::Accepts);
    }
    if blocks != Some(BACKDROP_BLOCKS) {
        return Some(BackdropFault::Blocks);
    }
    None
}

/// The smallest section number a master's slots do not bind, when the numbers are not exactly one to their count.
///
/// `None` means the numbers are a permutation of one to the count, so every `##` of a slide finds exactly one slot.
#[must_use]
pub fn missing_section(sections: &[u32]) -> Option<u32> {
    let count = u32::try_from(sections.len()).unwrap_or(u32::MAX);
    let mut wanted: u32 = 1;
    while wanted <= count {
        if !sections.contains(&wanted) {
            return Some(wanted);
        }
        wanted = wanted.saturating_add(1);
    }
    None
}

/// Why a `follows` reference is refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FollowsFault {
    /// The slot at this index follows a slot the master does not have.
    Unknown(usize),
    /// The slot at this index follows itself.
    SelfReference(usize),
    /// The slot at this index starts a chain that never ends.
    Cycle(usize),
}

/// The first fault among `follows`, where each entry is the index of the slot the slot at that position follows.
#[must_use]
pub fn follows_fault(follows: &[Option<usize>]) -> Option<FollowsFault> {
    for (index, target) in follows.iter().enumerate() {
        let Some(target) = target else {
            continue;
        };
        if *target >= follows.len() {
            return Some(FollowsFault::Unknown(index));
        }
        if *target == index {
            return Some(FollowsFault::SelfReference(index));
        }
    }
    for start in 0..follows.len() {
        // A chain without a loop visits each slot at most once, so after as many steps as there are slots it has ended.
        let mut at = Some(start);
        for _ in 0..follows.len() {
            at = at.and_then(|here| follows.get(here).copied().flatten());
        }
        if at.is_some() {
            return Some(FollowsFault::Cycle(start));
        }
    }
    None
}

/// The thousandths in one whole unit, as an integer wide enough for the face arithmetic.
const THOUSAND: i64 = 1000;

/// Whether a line pitch of `line_height` times the size covers the face's natural line box.
///
/// The box is `ascender - descender` font units of `units_per_em`, so the size cancels out.
#[must_use]
pub fn line_box_fits(
    line_height: Thousandths,
    ascender: i16,
    descender: i16,
    units_per_em: u16,
) -> bool {
    let pitch = line_height.0.checked_mul(i64::from(units_per_em));
    let face = i64::from(ascender)
        .checked_sub(i64::from(descender))
        .and_then(|box_units| box_units.checked_mul(THOUSAND));
    match (pitch, face) {
        (Some(pitch), Some(face)) => pitch >= face,
        (None, _) | (_, None) => false,
    }
}

/// The properties Kani proves for every short master and the tests check over the masters they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{BACKDROP_BLOCKS, Thousandths};
    #[cfg(kani)]
    use super::{
        BackdropFault, FollowsFault, backdrop_fault, follows_fault, line_box_fits, missing_section,
    };
    use crate::ir::BlockKind;
    use crate::theme::{Binding, BlockKindSet};

    /// How many slots the bounded proofs admit.
    #[cfg(kani)]
    const SLOTS: usize = 3;

    /// A slot is a backdrop exactly when it flows exactly one image: the kinds it accepts are the image alone.
    pub(super) fn spec_backdrop(
        binds: Binding,
        accepts: BlockKindSet,
        blocks: Option<u32>,
    ) -> bool {
        let image_only = accepts.contains(BlockKind::Image)
            && [
                BlockKind::Heading,
                BlockKind::Paragraph,
                BlockKind::List,
                BlockKind::Code,
                BlockKind::Quote,
                BlockKind::Table,
            ]
            .iter()
            .all(|kind| !accepts.contains(*kind));
        binds == Binding::Flow && image_only && blocks == Some(BACKDROP_BLOCKS)
    }

    /// The largest section number the bounded proofs admit, which is twice the slot count so a gap can appear.
    #[cfg(kani)]
    const LARGEST_SECTION: u32 = 6;

    /// The numbers are exactly one to their count, each once.
    pub(super) fn spec_permutation(sections: &[u32]) -> bool {
        let count = u32::try_from(sections.len()).unwrap_or(u32::MAX);
        let in_range = sections
            .iter()
            .all(|section| *section >= 1 && *section <= count);
        let mut distinct = true;
        for (first, a) in sections.iter().enumerate() {
            for (second, b) in sections.iter().enumerate() {
                if first != second && a == b {
                    distinct = false;
                }
            }
        }
        in_range && distinct
    }

    /// Every reference names another slot of the master, and following references from any slot ends.
    pub(super) fn spec_follows_ok(follows: &[Option<usize>]) -> bool {
        let in_range = follows.iter().enumerate().all(|(index, target)| {
            target.is_none_or(|target| target < follows.len() && target != index)
        });
        let ends = (0..follows.len()).all(|start| {
            let mut at = Some(start);
            for _ in 0..=follows.len() {
                at = at.and_then(|here| follows.get(here).copied().flatten());
            }
            at.is_none()
        });
        in_range && ends
    }

    /// The line box inequality, over integers wide enough never to overflow.
    pub(super) fn spec_line_box(
        line_height: Thousandths,
        ascender: i16,
        descender: i16,
        units_per_em: u16,
    ) -> bool {
        let pitch = i128::from(line_height.0).wrapping_mul(i128::from(units_per_em));
        let face = i128::from(ascender)
            .wrapping_sub(i128::from(descender))
            .wrapping_mul(i128::from(super::THOUSAND));
        pitch >= face
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(6)]
    fn sections_are_exactly_one_to_their_count_each_once() {
        let sections: [u32; SLOTS] = kani::any();
        for section in sections {
            kani::assume(section <= LARGEST_SECTION);
        }
        let slice: &[u32] = &sections;
        assert_eq!(missing_section(slice).is_none(), spec_permutation(slice));
        if let Some(missing) = missing_section(slice) {
            assert!(!slice.contains(&missing));
        }
        kani::cover!(missing_section(slice).is_none());
        kani::cover!(missing_section(slice).is_some());
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(7)]
    fn a_follows_chain_ends_inside_its_master_or_names_the_fault() {
        let follows: [Option<usize>; SLOTS] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= SLOTS);
        let Some(slice) = follows.get(..len) else {
            return;
        };
        let fault = follows_fault(slice);
        assert_eq!(fault.is_none(), spec_follows_ok(slice));
        kani::cover!(fault.is_none() && slice.iter().any(Option::is_some));
        kani::cover!(matches!(fault, Some(FollowsFault::Cycle(_))));
        kani::cover!(matches!(fault, Some(FollowsFault::Unknown(_))));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn a_line_box_fits_exactly_when_the_pitch_covers_the_face() {
        let line_height = Thousandths(kani::any());
        let ascender: i16 = kani::any();
        let descender: i16 = kani::any();
        let units_per_em: u16 = kani::any();
        kani::assume(line_height.0 >= 0 && line_height.0 <= 10 * super::THOUSAND);
        assert_eq!(
            line_box_fits(line_height, ascender, descender, units_per_em),
            spec_line_box(line_height, ascender, descender, units_per_em)
        );
        kani::cover!(line_box_fits(
            line_height,
            ascender,
            descender,
            units_per_em
        ));
        kani::cover!(!line_box_fits(
            line_height,
            ascender,
            descender,
            units_per_em
        ));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn a_backdrop_is_exactly_a_flow_slot_of_one_image() {
        let binds: Binding = kani::any();
        // Every set of kinds, built kind by kind, so no bit beyond the kinds is set.
        let mut accepts = BlockKindSet::EMPTY;
        for kind in [
            BlockKind::Heading,
            BlockKind::Paragraph,
            BlockKind::List,
            BlockKind::Code,
            BlockKind::Quote,
            BlockKind::Image,
            BlockKind::Table,
        ] {
            if kani::any() {
                accepts = accepts.with(kind);
            }
        }
        let blocks: Option<u32> = kani::any();
        let fault = backdrop_fault(binds, accepts, blocks);
        assert_eq!(fault.is_none(), spec_backdrop(binds, accepts, blocks));
        match fault {
            None => {}
            Some(BackdropFault::Binds) => assert!(binds != Binding::Flow),
            Some(BackdropFault::Accepts) => {
                assert!(binds == Binding::Flow);
                assert!(accepts != BlockKindSet::EMPTY.with(BlockKind::Image));
            }
            Some(BackdropFault::Blocks) => {
                assert!(binds == Binding::Flow);
                assert!(blocks != Some(BACKDROP_BLOCKS));
            }
        }
        kani::cover!(fault.is_none());
        kani::cover!(fault == Some(BackdropFault::Accepts));
        kani::cover!(fault == Some(BackdropFault::Blocks));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_gap_in_the_section_numbers() {
        kani::assert(missing_section(&[1, 3]).is_none(), "NEGATIVE_THEME_CONTROL");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackdropFault, FollowsFault, Thousandths, backdrop_fault, follows_fault, line_box_fits,
        missing_section, proofs,
    };
    use crate::ir::BlockKind;
    use crate::theme::{Binding, BlockKindSet};

    #[test]
    fn a_backdrop_flows_exactly_one_image_and_each_fault_is_named() {
        let image = BlockKindSet::EMPTY.with(BlockKind::Image);
        let text = BlockKindSet::EMPTY.with(BlockKind::Paragraph);
        let both = image.with(BlockKind::Paragraph);
        for (binds, accepts, blocks) in [
            (Binding::Flow, image, Some(1)),
            (Binding::Flow, image, Some(2)),
            (Binding::Flow, image, Some(0)),
            (Binding::Flow, image, None),
            (Binding::Flow, text, Some(1)),
            (Binding::Flow, both, Some(1)),
            (Binding::Flow, BlockKindSet::EMPTY, Some(1)),
            (Binding::Title, image, Some(1)),
            (Binding::Subtitle, image, Some(1)),
            (Binding::Section(1), image, Some(1)),
            (Binding::Section(1), text, None),
        ] {
            assert_eq!(
                backdrop_fault(binds, accepts, blocks).is_none(),
                proofs::spec_backdrop(binds, accepts, blocks),
                "{binds:?} {accepts:?} {blocks:?}"
            );
        }
        assert_eq!(backdrop_fault(Binding::Flow, image, Some(1)), None);
        assert_eq!(
            backdrop_fault(Binding::Title, image, Some(1)),
            Some(BackdropFault::Binds)
        );
        assert_eq!(
            backdrop_fault(Binding::Section(1), text, None),
            Some(BackdropFault::Binds)
        );
        assert_eq!(
            backdrop_fault(Binding::Flow, both, Some(1)),
            Some(BackdropFault::Accepts)
        );
        assert_eq!(
            backdrop_fault(Binding::Flow, text, Some(1)),
            Some(BackdropFault::Accepts)
        );
        assert_eq!(
            backdrop_fault(Binding::Flow, image, Some(2)),
            Some(BackdropFault::Blocks)
        );
        assert_eq!(
            backdrop_fault(Binding::Flow, image, None),
            Some(BackdropFault::Blocks)
        );
    }

    #[test]
    fn section_numbers_must_be_one_to_their_count() {
        for sections in [
            &[][..],
            &[1],
            &[2, 1],
            &[1, 3],
            &[0],
            &[1, 1],
            &[3, 1, 2],
            &[2, 2, 1],
        ] {
            assert_eq!(
                missing_section(sections).is_none(),
                proofs::spec_permutation(sections),
                "{sections:?}"
            );
        }
        assert_eq!(missing_section(&[1, 3]), Some(2));
        assert_eq!(missing_section(&[1, 1]), Some(2));
        assert_eq!(missing_section(&[0]), Some(1));
        assert_eq!(missing_section(&[2, 1]), None);
    }

    #[test]
    fn follows_references_stay_in_the_master_and_end() {
        for follows in [
            &[][..],
            &[None],
            &[None, Some(0)],
            &[Some(1), None],
            &[Some(1), Some(0)],
            &[Some(0)],
            &[Some(5)],
            &[Some(1), Some(2), None],
            &[Some(1), Some(2), Some(1)],
        ] {
            assert_eq!(
                follows_fault(follows).is_none(),
                proofs::spec_follows_ok(follows),
                "{follows:?}"
            );
        }
        assert_eq!(
            follows_fault(&[Some(0)]),
            Some(FollowsFault::SelfReference(0))
        );
        assert_eq!(follows_fault(&[Some(5)]), Some(FollowsFault::Unknown(0)));
        assert_eq!(
            follows_fault(&[Some(1), Some(0)]),
            Some(FollowsFault::Cycle(0))
        );
        assert_eq!(
            follows_fault(&[Some(1), Some(2), Some(1)]),
            Some(FollowsFault::Cycle(0))
        );
        assert_eq!(follows_fault(&[Some(1), Some(2), None]), None);
    }

    #[test]
    fn a_line_box_fits_when_the_pitch_reaches_the_face() {
        let cases = [
            (Thousandths(1450), 880_i16, -120_i16, 1000_u16, true),
            (Thousandths(1000), 880, -120, 1000, true),
            (Thousandths(999), 880, -120, 1000, false),
            (Thousandths(1200), 1800, -400, 2048, true),
            (Thousandths(1000), 1800, -400, 2048, false),
            (Thousandths(0), 0, 0, 1000, true),
            (Thousandths(i64::MAX), 1, -1, 2, false),
        ];
        for (line_height, ascender, descender, units_per_em, expected) in cases {
            assert_eq!(
                line_box_fits(line_height, ascender, descender, units_per_em),
                expected,
                "{line_height:?} {ascender} {descender} {units_per_em}"
            );
            if line_height.0 < 100_000 {
                assert_eq!(
                    line_box_fits(line_height, ascender, descender, units_per_em),
                    proofs::spec_line_box(line_height, ascender, descender, units_per_em)
                );
            }
        }
    }
}
