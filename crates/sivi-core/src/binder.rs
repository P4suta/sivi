//! The slot binding automaton: each step binds one block of a slide to a slot of its master, or refuses it.
//!
//! `specs/binder.qnt` models the same automaton, and the replay test runs its traces through `step`.

use crate::diagnostic::Code;
use crate::ir::{BlockKind, HeadingLevel};
use crate::slot_rules::{BlockFacts, Choice, MAX_SLOTS, SlotGrammar, admits, choose};
use crate::theme::Binding;

/// Where the automaton stands in a slide.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum Phase {
    /// Before any block.
    #[default]
    Start,
    /// Right after the title, where a paragraph may be the subtitle.
    Titled,
    /// In the blocks before any `##`.
    Flow,
    /// Inside the section of this number, counted from one.
    Section(u32),
}

/// The state of the automaton: the phase, how many blocks each slot holds, the slot of the last block, and the block count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct BindState {
    pub phase: Phase,
    /// How many blocks each slot holds, by slot index.
    pub filled: [u32; MAX_SLOTS],
    /// The slot that took the last block, which `follows` reads.
    pub last: Option<usize>,
}

/// What a block the automaton reads looks like: a heading of a level, or a block with its measures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Input {
    Heading(HeadingLevel),
    Block(BlockFacts),
}

/// What one step decides.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Step {
    /// The block went to the slot at this index.
    Bound { slot: usize, state: BindState },
    /// The block has no place, for this reason; the state does not change.
    Refused { code: Code, state: BindState },
}

/// The index of the slot that binds `wanted`.
fn slot_binding(slots: &[SlotGrammar], wanted: Binding) -> Option<usize> {
    slots.iter().position(|slot| slot.binds == wanted)
}

/// The state after the block went to the slot at `index`; a section heading labels its slot without filling it.
fn bound(mut state: BindState, index: usize, phase: Phase, fills: bool) -> Step {
    if fills && let Some(count) = state.filled.get_mut(index) {
        *count = count.saturating_add(1);
    }
    state.last = Some(index);
    state.phase = phase;
    Step::Bound { slot: index, state }
}

/// The step for a title heading: only the first block of a slide, in a master with a title slot.
fn title(slots: &[SlotGrammar], state: BindState) -> Step {
    match (state.phase, slot_binding(slots, Binding::Title)) {
        (Phase::Start, Some(index)) => bound(state, index, Phase::Titled, true),
        (Phase::Start, None) | (Phase::Titled | Phase::Flow | Phase::Section(_), _) => {
            Step::Refused {
                code: Code::TitlePosition,
                state,
            }
        }
    }
}

/// The step for a section heading: the next section number, in a master with a slot for it.
fn section(slots: &[SlotGrammar], state: BindState) -> Step {
    let number = match state.phase {
        Phase::Section(current) => current.saturating_add(1),
        Phase::Start | Phase::Titled | Phase::Flow => 1,
    };
    slot_binding(slots, Binding::Section(number)).map_or_else(
        || Step::Refused {
            code: Code::SectionCount,
            state,
        },
        |index| bound(state, index, Phase::Section(number), false),
    )
}

/// The step for a block inside a section: the section's slot takes it or refuses it.
fn in_section(slots: &[SlotGrammar], state: BindState, number: u32, block: BlockFacts) -> Step {
    let Some(index) = slot_binding(slots, Binding::Section(number)) else {
        return Step::Refused {
            code: Code::SectionCount,
            state,
        };
    };
    let filled = state.filled.get(index).copied().unwrap_or(0);
    match slots.get(index).map(|slot| admits(*slot, filled, block)) {
        Some(Ok(())) => bound(state, index, Phase::Section(number), true),
        Some(Err(crate::slot_rules::Refusal::Capacity)) => Step::Refused {
            code: Code::CapacityExceeded,
            state,
        },
        Some(Err(crate::slot_rules::Refusal::Kind)) | None => Step::Refused {
            code: Code::NoSlotForBlock,
            state,
        },
    }
}

/// The step for a block of the flow: the subtitle right after the title, else the slot the slot rules choose.
fn flow(slots: &[SlotGrammar], state: BindState, block: BlockFacts) -> Step {
    if state.phase == Phase::Titled
        && block.kind == BlockKind::Paragraph
        && let Some(index) = slot_binding(slots, Binding::Subtitle)
        && state.filled.get(index).is_some_and(|count| *count == 0)
    {
        return bound(state, index, Phase::Flow, true);
    }
    match choose(slots, &state.filled, state.last, block) {
        Choice::Slot(index) => bound(state, index, Phase::Flow, true),
        Choice::Full => Step::Refused {
            code: Code::CapacityExceeded,
            state,
        },
        Choice::NoSlot => Step::Refused {
            code: Code::NoSlotForBlock,
            state,
        },
    }
}

/// One step of the automaton over the slots of a master.
#[must_use]
pub fn step(slots: &[SlotGrammar], state: BindState, input: Input) -> Step {
    match input {
        Input::Heading(HeadingLevel::Title) => title(slots, state),
        Input::Heading(HeadingLevel::Section) => section(slots, state),
        Input::Block(block) => match state.phase {
            Phase::Section(number) => in_section(slots, state, number, block),
            Phase::Start | Phase::Titled | Phase::Flow => flow(slots, state, block),
        },
    }
}

/// The first required slot the slide left empty, once every block is bound.
#[must_use]
pub fn unfilled(slots: &[SlotGrammar], state: BindState) -> Option<usize> {
    slots
        .iter()
        .enumerate()
        .find(|(index, slot)| {
            slot.required && state.filled.get(*index).is_none_or(|count| *count == 0)
        })
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::{BindState, Input, Phase, Step, step, unfilled};
    use crate::diagnostic::Code;
    use crate::ir::{BlockKind, HeadingLevel};
    use crate::slot_rules::{BlockFacts, SlotGrammar};
    use crate::theme::{Binding, BlockKindSet, Capacity};
    use alloc::vec;
    use alloc::vec::Vec;

    const fn slot(
        binds: Binding,
        accepts: BlockKindSet,
        required: bool,
        follows: Option<usize>,
        blocks: Option<u32>,
    ) -> SlotGrammar {
        SlotGrammar {
            binds,
            accepts,
            required,
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

    const fn block(kind: BlockKind) -> Input {
        Input::Block(BlockFacts {
            kind,
            items: 0,
            depth: 0,
            rows: 0,
            columns: 0,
            lines: 0,
        })
    }

    /// Title, subtitle, a figure slot, and a caption that follows it.
    fn figure_master() -> [SlotGrammar; 4] {
        let paragraph = BlockKindSet::EMPTY.with(BlockKind::Paragraph);
        [
            slot(Binding::Title, BlockKindSet::EMPTY, true, None, None),
            slot(Binding::Subtitle, paragraph, false, None, None),
            slot(
                Binding::Flow,
                BlockKindSet::EMPTY.with(BlockKind::Image),
                true,
                None,
                Some(1),
            ),
            slot(Binding::Flow, paragraph, false, Some(2), Some(1)),
        ]
    }

    /// Title and two sections.
    fn comparison_master() -> [SlotGrammar; 3] {
        let text = BlockKindSet::EMPTY
            .with(BlockKind::Paragraph)
            .with(BlockKind::List);
        [
            slot(Binding::Title, BlockKindSet::EMPTY, true, None, None),
            slot(Binding::Section(1), text, false, None, None),
            slot(Binding::Section(2), text, false, None, Some(1)),
        ]
    }

    fn run(slots: &[SlotGrammar], inputs: &[Input]) -> (BindState, Vec<Result<usize, Code>>) {
        let mut state = BindState::default();
        let mut outcomes = Vec::new();
        for input in inputs {
            match step(slots, state, *input) {
                Step::Bound { slot, state: next } => {
                    state = next;
                    outcomes.push(Ok(slot));
                }
                Step::Refused { code, state: same } => {
                    assert_eq!(same, state);
                    outcomes.push(Err(code));
                }
            }
        }
        (state, outcomes)
    }

    #[test]
    fn a_title_a_subtitle_a_figure_and_its_caption_each_find_their_slot() {
        let slots = figure_master();
        let (state, outcomes) = run(
            &slots,
            &[
                Input::Heading(HeadingLevel::Title),
                block(BlockKind::Paragraph),
                block(BlockKind::Image),
                block(BlockKind::Paragraph),
            ],
        );
        assert_eq!(outcomes, vec![Ok(0), Ok(1), Ok(2), Ok(3)]);
        assert_eq!(state.phase, Phase::Flow);
        assert_eq!(state.last, Some(3));
        assert_eq!(unfilled(&slots, state), None);
        let (_, without_subtitle) = run(
            &slots,
            &[
                Input::Heading(HeadingLevel::Title),
                block(BlockKind::Image),
                block(BlockKind::Paragraph),
                block(BlockKind::Paragraph),
            ],
        );
        assert_eq!(
            without_subtitle,
            vec![Ok(0), Ok(2), Ok(3), Err(Code::CapacityExceeded)]
        );
    }

    #[test]
    fn headings_out_of_place_and_blocks_without_a_slot_are_refused() {
        let slots = figure_master();
        let (state, outcomes) = run(
            &slots,
            &[
                block(BlockKind::Image),
                Input::Heading(HeadingLevel::Title),
                block(BlockKind::Code),
                block(BlockKind::Image),
            ],
        );
        assert_eq!(
            outcomes,
            vec![
                Ok(2),
                Err(Code::TitlePosition),
                Err(Code::NoSlotForBlock),
                Err(Code::CapacityExceeded)
            ]
        );
        assert_eq!(unfilled(&slots, state), Some(0));
        let (_, twice) = run(
            &slots,
            &[
                Input::Heading(HeadingLevel::Title),
                Input::Heading(HeadingLevel::Title),
            ],
        );
        assert_eq!(twice, vec![Ok(0), Err(Code::TitlePosition)]);
        let (_, no_section) = run(&slots, &[Input::Heading(HeadingLevel::Section)]);
        assert_eq!(no_section, vec![Err(Code::SectionCount)]);
    }

    #[test]
    fn sections_take_their_own_blocks_in_order() {
        let slots = comparison_master();
        let (state, outcomes) = run(
            &slots,
            &[
                Input::Heading(HeadingLevel::Title),
                Input::Heading(HeadingLevel::Section),
                block(BlockKind::List),
                block(BlockKind::Paragraph),
                Input::Heading(HeadingLevel::Section),
                block(BlockKind::Paragraph),
                block(BlockKind::Paragraph),
                block(BlockKind::Code),
                Input::Heading(HeadingLevel::Section),
            ],
        );
        assert_eq!(
            outcomes,
            vec![
                Ok(0),
                Ok(1),
                Ok(1),
                Ok(1),
                Ok(2),
                Ok(2),
                Err(Code::CapacityExceeded),
                Err(Code::NoSlotForBlock),
                Err(Code::SectionCount)
            ]
        );
        assert_eq!(state.phase, Phase::Section(2));
        assert_eq!(state.filled[1], 2);
        assert_eq!(state.filled[2], 1);
        assert_eq!(state.last, Some(2));
        let (_, flow_first) = run(&slots, &[block(BlockKind::Paragraph)]);
        assert_eq!(flow_first, vec![Err(Code::NoSlotForBlock)]);
    }
}
