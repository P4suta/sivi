---
status: accepted
date: 2026-10-13
---
# Report content that does not fit as an error with its numbers, and never shrink or split it

## Context

Slide tools that shrink text to fit produce slides nobody can read from the back of a room.
Tools that split overflowing content move the problem to a slide the author never designed.
Both hide the fact that the author wrote more than the master holds.

## Decision

When the measured height of a region's content exceeds its box by more than the tolerance, the compiler reports an error with the required height, the available height, and the excess.
When a box of a laid-out item leaves its region or the page, the compiler reports an error with the box and the item.
The report names the Markdown span of the content and computes a suggestion: the last part that fits, from the cumulative heights, and masters of the theme whose corresponding slot would hold the content.
No rule changes a size, a position, or the content.

## Assurance boundary

`fit_rules` and `split_rules` prove the verdict and the suggested split for every admitted input.

## Alternatives

A per-slide escape hatch that shrinks text would reintroduce the layout directive the Markdown record forbids.
Automatic splitting would need a notion of where content may break that depends on meaning the compiler does not have.

## Consequences

A deck that compiles has no overflowing slide, and a deck that does not compile says exactly how far each slide is from fitting.
Authors shorten, split, or pick a roomier master; the compiler tells them which.
