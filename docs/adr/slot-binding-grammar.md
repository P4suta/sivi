---
status: accepted
date: 2026-10-13
---
# Bind a slide's blocks to a master's slots by one deterministic grammar

## Context

A master declares slots that bind the title, the subtitle, one numbered section each, or the flow of blocks.
A slide is a list of blocks.
Which block goes to which slot must be decided the same way by the compiler, by the author reading the error, and by the reference implementation, with no layout knowledge.

## Decision

A slide reads as `[H1 [subtitle-paragraph]] section0 *( H2 blocks )`.
The first block may be `#`, which binds the title slot; a `#` anywhere else, or a second one, reports `E0307`.
A paragraph right after the title binds the subtitle slot when the master has one.
Each `##` opens the next section and binds the slot of that number without filling it; a section the master lacks reports `E0305`.
A block inside a section goes to the section's slot, which must accept its kind and have room.
A block of the flow goes to the slot that follows the slot of the previous block when that slot admits it, else to the first flow slot in declaration order that admits it.
A block no flow slot accepts reports `E0303`; a block every accepting slot refuses by capacity reports `E0306`.
A required slot left empty reports `E0304`.
`binder::step` decides one block at a time from the automaton's state, and the state carries only the phase, the fill counts, and the last slot, so a refusal changes nothing.

## Assurance boundary

`slot_rules` proves the choice of slot and the admission rule for every small master.
`specs/binder.qnt` models the automaton over a master of five slots, holds `safe` in every reachable state, and its traces replay through `binder::step` state for state; its probe without capacities breaks `safe`.
`specs/binding.pl` is an independent reference the differential test compares with on fixtures and generated slides.

## Alternatives

Binding by best fit over the whole slide would let a later block move an earlier one, which no author can predict from the error.
Binding by position only would make a master's slot order a hidden layout directive.

## Consequences

A master with two slots of the same grammar is refused as ambiguous, since the grammar would bind every slide the same way.
A theme that wants richer placement adds a slot with a narrower `accepts` or a `follows`, never a rule in the deck.
