---
status: accepted
date: 2026-10-14
---
# Fixture decks through the real Typst, with their outputs snapshotted

## Context

The thesis of sivi is a verdict: a slide either fits its master or earns a diagnostic with measured numbers.
Unit tests over synthetic measurements show the rules, but only the real compiler shows the numbers a person will see.

## Decision

Every deck under `fixtures/decks/` runs through the whole pipeline with the pinned Typst and the bundled fonts in `crates/sivi/tests/`.
A `pass-` deck builds, a `warn-` deck builds with warnings, and a `fail-` deck is refused with the error its name states.
Themes under `fixtures/themes/` are checked the same way by `theme check`.
The diagnostics of every fixture, rendered plain for a person and as the JSON document, are snapshots under `crates/sivi/tests/snapshots/`, reviewed with `cargo insta`.
The geometry of every passing deck, as the frame walk reads it back, is a snapshot too: page, kind, and box of every item, in millipoints.
No raster image is snapshotted; the geometry is what sivi decides on, and a rasterizer's pixels drift without a decision changing.
The Japanese fixtures are checked for line-head prohibition by the first character of every line in the geometry.

## Assurance boundary

A snapshot test fails on any change of a verdict, a number, or a wording, so a change must be reviewed and accepted with the snapshot.
The fixtures also feed the Prolog reference of the binding and the fuzz corpora of the parsers.

## Alternatives

Comparing PDF bytes would catch every change but explain none.
Comparing rendered pixels would catch rasterizer upgrades that change no decision.

## Consequences

A Typst upgrade that moves a line by a hair shows up as a geometry diff to review.
Each new diagnostic ships with a fixture that earns it.
