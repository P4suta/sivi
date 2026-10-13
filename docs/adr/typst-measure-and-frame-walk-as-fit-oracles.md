---
status: accepted
date: 2026-10-13
---
# Decide whether content fits from Typst's measure function and a walk over its frames

## Context

sivi refuses a slide whose content does not fit its region, so it needs the height the content requires and the boxes it actually occupies.
Typst offers `measure`, which lays content out in a given width and reports its size, and it exposes the laid-out frames of every page.
Experiments against `typst 0.15.1` showed three things.
`measure(width: W, body)` reports the required height at the region's width.
`measure` clamps the width to W, so a code line that cannot break reports the region's width while its glyphs extend beyond it.
Content placed with `place` never adds a page, however far it overflows.

## Decision

The emitter wraps every region's content in a `context` that measures the whole body and each prefix of its parts at the region's width, and writes the results into a `metadata` marker with a label the protocol module of the core spells.
The adapter reads each marker back as millipoints and reports the required height and the cumulative heights.
The adapter also walks every frame of every page, accumulating the group transforms, and reports the box of every text, shape, and image item in page coordinates.
Text boxes use the glyph bounds, so the emitter sets the text edges to `bounds`.
The core decides vertical overflow from the required height and horizontal overflow from the boxes, and it compares the two oracles: an item box that extends further than the measured height admits is an adapter error, not a verdict.
A marker that is missing, duplicated, or malformed is an error and never a "fits".

## Assurance boundary

Every assumption above is a test in `crates/sivi-typst/tests/boundary.rs`, listed in `docs/verification/typst-boundary.md`.
The decisions over the measured values are pure core rules with their own proofs.

## Alternatives

Reading only the frames would make the required height a reconstruction from glyph boxes, which depends on line spacing conventions that `measure` already applies.
Reading only `measure` would miss horizontal overflow.

## Consequences

Every slide is compiled once and measured in the same pass that produces the PDF, so what was checked is what is exported.
The emitter's scaffold and the adapter's reader share the protocol module, so neither can drift alone.
