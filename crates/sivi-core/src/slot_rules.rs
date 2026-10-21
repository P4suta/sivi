//! Which slot of a master takes a block: the follower of the last slot first, then the first flow slot in order.

use crate::ir::BlockKind;
use crate::theme::{Binding, BlockKindSet, Capacity};

/// The most slots a master holds, which bounds the binder's state.
pub const MAX_SLOTS: usize = 8;

/// What the binder knows about a slot: the grammar of the theme's slot, with `follows` as an index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct SlotGrammar {
    pub binds: Binding,
    pub accepts: BlockKindSet,
    pub required: bool,
    /// The index of the slot whose content this slot's content follows directly.
    pub follows: Option<usize>,
    pub capacity: Capacity,
}

/// What the binder knows about a block: its kind and the measures the capacities bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct BlockFacts {
    pub kind: BlockKind,
    /// How many items a list holds, over every level.
    pub items: u32,
    /// How deep a list nests.
    pub depth: u32,
    pub rows: u32,
    pub columns: u32,
    /// How many lines a code block holds.
    pub lines: u32,
}

/// Why a slot does not take a block.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Refusal {
    /// The slot does not accept the block's kind.
    Kind,
    /// The slot accepts the kind but the block or the slot's content would pass a capacity.
    Capacity,
}

const fn within(measure: u32, limit: Option<u32>) -> bool {
    match limit {
        Some(limit) => measure <= limit,
        None => true,
    }
}

/// Whether `slot`, holding `filled` blocks, takes `block`.
///
/// # Errors
///
/// Returns why the slot does not take the block.
pub const fn admits(slot: SlotGrammar, filled: u32, block: BlockFacts) -> Result<(), Refusal> {
    if !slot.accepts.contains(block.kind) {
        return Err(Refusal::Kind);
    }
    let room = match slot.capacity.blocks {
        Some(limit) => filled < limit,
        None => true,
    };
    let fits = room
        && within(block.items, slot.capacity.items)
        && within(block.depth, slot.capacity.depth)
        && within(block.rows, slot.capacity.rows)
        && within(block.columns, slot.capacity.columns)
        && within(block.lines, slot.capacity.lines);
    if fits { Ok(()) } else { Err(Refusal::Capacity) }
}

/// Where a block of the flow goes, or why nowhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Choice {
    /// The index of the slot that takes the block.
    Slot(usize),
    /// No flow slot accepts the block's kind.
    NoSlot,
    /// A flow slot accepts the kind, but every such slot is full or the block passes a capacity.
    Full,
}

/// Whether the slot at `index` is a flow slot that admits `block`, given `filled` blocks per slot.
fn flow_admits(
    slots: &[SlotGrammar],
    filled: &[u32],
    index: usize,
    block: BlockFacts,
) -> Result<(), Refusal> {
    match (slots.get(index), filled.get(index)) {
        (Some(slot), Some(count)) if slot.binds == Binding::Flow => admits(*slot, *count, block),
        (Some(_), Some(_)) | (None, _) | (_, None) => Err(Refusal::Kind),
    }
}

/// The flow slot that takes `block` after the slot `last` took the previous block.
///
/// A slot that follows `last` and admits the block comes first; otherwise the first flow slot in order that admits it.
#[must_use]
pub fn choose(
    slots: &[SlotGrammar],
    filled: &[u32],
    last: Option<usize>,
    block: BlockFacts,
) -> Choice {
    if let Some(last) = last
        && let Some(follower) = slots.iter().position(|slot| slot.follows == Some(last))
        && flow_admits(slots, filled, follower, block).is_ok()
    {
        return Choice::Slot(follower);
    }
    let mut accepted = false;
    for index in 0..slots.len() {
        match flow_admits(slots, filled, index, block) {
            Ok(()) => return Choice::Slot(index),
            Err(Refusal::Capacity) => accepted = true,
            Err(Refusal::Kind) => {}
        }
    }
    if accepted {
        Choice::Full
    } else {
        Choice::NoSlot
    }
}

/// The properties Kani proves for every small master and the tests check over the masters they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{BlockFacts, Choice, Refusal, SlotGrammar, admits, choose, flow_admits};
    #[cfg(kani)]
    use crate::theme::Binding;

    /// How many slots the bounded proofs admit.
    #[cfg(kani)]
    const SLOTS: usize = 3;

    /// The choice names a flow slot that admits the block, prefers the follower of the last slot, and otherwise the first in order.
    pub(super) fn spec_choice(
        slots: &[SlotGrammar],
        filled: &[u32],
        last: Option<usize>,
        block: BlockFacts,
    ) -> Choice {
        let choice = choose(slots, filled, last, block);
        let admits = |index| flow_admits(slots, filled, index, block).is_ok();
        let first = (0..slots.len()).find(|index| admits(*index));
        match choice {
            Choice::Slot(index) => {
                assert!(admits(index));
                let follower =
                    last.and_then(|last| slots.iter().position(|slot| slot.follows == Some(last)));
                match follower {
                    Some(follower) if admits(follower) => assert_eq!(index, follower),
                    Some(_) | None => assert_eq!(first, Some(index)),
                }
            }
            Choice::Full => {
                assert!(first.is_none());
                assert!((0..slots.len()).any(|index| {
                    flow_admits(slots, filled, index, block) == Err(Refusal::Capacity)
                }));
            }
            Choice::NoSlot => {
                assert!((0..slots.len()).all(|index| {
                    flow_admits(slots, filled, index, block) == Err(Refusal::Kind)
                }));
            }
        }
        choice
    }

    /// A slot admits a block exactly when it accepts the kind and every measure stays within its capacity.
    pub(super) fn spec_admits(slot: SlotGrammar, filled: u32, block: BlockFacts) {
        let verdict = admits(slot, filled, block);
        let kind = slot.accepts.contains(block.kind);
        let within = |measure: u32, limit: Option<u32>| limit.is_none_or(|limit| measure <= limit);
        let room = slot.capacity.blocks.is_none_or(|limit| filled < limit);
        let fits = room
            && within(block.items, slot.capacity.items)
            && within(block.depth, slot.capacity.depth)
            && within(block.rows, slot.capacity.rows)
            && within(block.columns, slot.capacity.columns)
            && within(block.lines, slot.capacity.lines);
        let expected = if !kind {
            Err(Refusal::Kind)
        } else if fits {
            Ok(())
        } else {
            Err(Refusal::Capacity)
        };
        assert_eq!(verdict, expected);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn a_slot_admits_a_block_exactly_when_the_kind_and_every_measure_fit() {
        let slot: SlotGrammar = kani::any();
        let filled: u32 = kani::any();
        let block: BlockFacts = kani::any();
        spec_admits(slot, filled, block);
        kani::cover!(admits(slot, filled, block).is_ok());
        kani::cover!(admits(slot, filled, block) == Err(Refusal::Capacity));
        kani::cover!(admits(slot, filled, block) == Err(Refusal::Kind));
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(5)]
    fn the_follower_of_the_last_slot_comes_first_and_the_first_flow_slot_otherwise() {
        let slots: [SlotGrammar; SLOTS] = kani::any();
        for slot in slots {
            kani::assume(slot.follows.is_none_or(|index| index < SLOTS));
        }
        let filled: [u32; SLOTS] = kani::any();
        let last: Option<usize> = kani::any();
        kani::assume(last.is_none_or(|index| index < SLOTS));
        let block: BlockFacts = kani::any();
        let choice = spec_choice(&slots, &filled, last, block);
        kani::cover!(
            matches!(choice, Choice::Slot(index) if slots[index].follows == last && last.is_some())
        );
        kani::cover!(matches!(choice, Choice::Slot(index) if slots[index].follows != last));
        kani::cover!(choice == Choice::Full);
        kani::cover!(choice == Choice::NoSlot);
        kani::cover!(slots.iter().any(|slot| slot.binds == Binding::Flow));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_title_slot_as_a_home_for_flow_content() {
        use crate::ir::BlockKind;
        use crate::theme::{BlockKindSet, Capacity};
        let title = SlotGrammar {
            binds: Binding::Title,
            accepts: BlockKindSet::EMPTY.with(BlockKind::Paragraph),
            required: true,
            follows: None,
            capacity: Capacity::default(),
        };
        let block = BlockFacts {
            kind: BlockKind::Paragraph,
            items: 0,
            depth: 0,
            rows: 0,
            columns: 0,
            lines: 0,
        };
        kani::assert(
            choose(&[title], &[0], None, block) == Choice::Slot(0),
            "NEGATIVE_SLOT_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockFacts, Choice, Refusal, SlotGrammar, admits, choose, proofs};
    use crate::ir::BlockKind;
    use crate::theme::{Binding, BlockKindSet, Capacity};

    const fn flow(
        accepts: BlockKindSet,
        follows: Option<usize>,
        blocks: Option<u32>,
    ) -> SlotGrammar {
        SlotGrammar {
            binds: Binding::Flow,
            accepts,
            required: false,
            follows,
            capacity: Capacity {
                blocks,
                items: None,
                depth: None,
                rows: None,
                columns: None,
                lines: None,
            },
        }
    }

    const fn block(kind: BlockKind) -> BlockFacts {
        BlockFacts {
            kind,
            items: 0,
            depth: 0,
            rows: 0,
            columns: 0,
            lines: 0,
        }
    }

    #[test]
    fn a_slot_admits_by_kind_then_by_every_capacity() {
        let text = BlockKindSet::EMPTY
            .with(BlockKind::Paragraph)
            .with(BlockKind::List);
        let slot = SlotGrammar {
            binds: Binding::Flow,
            accepts: text,
            required: false,
            follows: None,
            capacity: Capacity {
                blocks: Some(2),
                items: Some(8),
                depth: Some(2),
                rows: None,
                columns: None,
                lines: Some(14),
            },
        };
        assert_eq!(admits(slot, 0, block(BlockKind::Paragraph)), Ok(()));
        assert_eq!(
            admits(slot, 2, block(BlockKind::Paragraph)),
            Err(Refusal::Capacity)
        );
        assert_eq!(admits(slot, 0, block(BlockKind::Code)), Err(Refusal::Kind));
        let deep = BlockFacts {
            kind: BlockKind::List,
            items: 3,
            depth: 3,
            ..block(BlockKind::List)
        };
        assert_eq!(admits(slot, 0, deep), Err(Refusal::Capacity));
        let many = BlockFacts {
            items: 9,
            depth: 1,
            ..deep
        };
        assert_eq!(admits(slot, 0, many), Err(Refusal::Capacity));
        let fine = BlockFacts {
            items: 8,
            depth: 2,
            ..deep
        };
        assert_eq!(admits(slot, 1, fine), Ok(()));
        for (filled, facts) in [(0, deep), (2, fine), (0, block(BlockKind::Code)), (1, fine)] {
            proofs::spec_admits(slot, filled, facts);
        }
    }

    #[test]
    fn the_follower_is_the_slot_that_names_the_last_slot_and_not_the_first_other_slot() {
        let paragraph = BlockKindSet::EMPTY.with(BlockKind::Paragraph);
        let slots = [
            flow(paragraph, None, Some(2)),
            flow(paragraph, Some(0), Some(2)),
        ];
        assert_eq!(
            choose(&slots, &[1, 0], Some(0), block(BlockKind::Paragraph)),
            Choice::Slot(1)
        );
    }

    #[test]
    fn the_follower_comes_first_then_the_first_flow_slot_in_order() {
        let paragraph = BlockKindSet::EMPTY.with(BlockKind::Paragraph);
        let image = BlockKindSet::EMPTY.with(BlockKind::Image);
        let slots = [
            SlotGrammar {
                binds: Binding::Title,
                ..flow(paragraph, None, None)
            },
            flow(image, None, Some(1)),
            flow(paragraph, Some(1), Some(1)),
            flow(paragraph, None, None),
        ];
        let empty = [0, 0, 0, 0];
        assert_eq!(
            choose(&slots, &empty, None, block(BlockKind::Paragraph)),
            Choice::Slot(2)
        );
        assert_eq!(
            choose(&slots, &empty, Some(1), block(BlockKind::Paragraph)),
            Choice::Slot(2)
        );
        assert_eq!(
            choose(&slots, &[0, 1, 1, 0], Some(1), block(BlockKind::Paragraph)),
            Choice::Slot(3)
        );
        assert_eq!(
            choose(&slots, &empty, None, block(BlockKind::Image)),
            Choice::Slot(1)
        );
        assert_eq!(
            choose(&slots, &[0, 1, 0, 0], None, block(BlockKind::Image)),
            Choice::Full
        );
        assert_eq!(
            choose(&slots, &empty, None, block(BlockKind::Code)),
            Choice::NoSlot
        );
        assert_eq!(
            choose(&slots, &empty, Some(0), block(BlockKind::Paragraph)),
            Choice::Slot(2)
        );
        for (filled, last, facts) in [
            (empty, None, block(BlockKind::Paragraph)),
            (empty, Some(1), block(BlockKind::Paragraph)),
            ([0, 1, 1, 0], Some(1), block(BlockKind::Paragraph)),
            (empty, None, block(BlockKind::Image)),
            ([0, 1, 0, 0], None, block(BlockKind::Image)),
            (empty, None, block(BlockKind::Code)),
        ] {
            proofs::spec_choice(&slots, &filled, last, facts);
        }
    }
}
