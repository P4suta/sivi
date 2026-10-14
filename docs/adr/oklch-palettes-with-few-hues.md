---
status: accepted
date: 2026-10-14
---

# OKLCH palettes with few hues

## Context

The palettes of presentation software read as a crowd: many hues at full chroma, each chosen by eye on an RGB wheel, with no rule holding them together.
A designer builds a palette the other way round: one seed, a near-neutral paper and ink, two or three accents at set lightnesses on hues the seed decides, and a marker color that stands out without shouting.
Such a palette needs a perceptual color space, where equal steps look equal; OKLCH is that space, and its lightness tracks what the eye sees so a contrast rule can be kept by construction.
The core reaches no floating point, so the space has to be built on integers.

## Decision

The core carries a fixed-point color engine in `color.rs`: linear light, luminance, the contrast ratio, Oklab, and OKLCH, with values in micro-units of one, matrices with ten decimals, cube and square roots by search, and every operation checked.
The two curves it needs, the sRGB transfer and the cosine, are tables `cargo xtask gen` writes into the core from the standard formulas, and `gen --check` holds the tables to the generator.
A hue is a tenth of a degree, which keeps a vivid primary within a few channels of itself through a trip to OKLCH and back.

`palette_rules` grows a palette from a seed: ten roles, each at a lightness and a chroma the mode sets, on a hue the harmony sets.
The roles are the paper, the ink, a muted ink, a rule, a code background, three accents, a marker, and a soft tint.
Every accent sits on one of three hues, so a palette holds at most three hue families, and the marker shares the second accent's hue.
The binary carries twelve palettes by name, each a seed, a mode, and a harmony chosen to read well.

A theme grows its colors under `[palette]` from a seed color or a carried name, with the mode and the harmony beside it, and any color it names under `[colors]` wins over the role of the same name.
The loader lints every theme: a text color below four and a half to one on the paper is an error, more than three hue families is a warning, and a paper with a chroma above six hundredths is a warning.
`sivi palette` prints a grown palette as theme text, with swatches on a color terminal, and lists the carried palettes without a seed.

## Assurance boundary

Kani proves that the light table rises and every channel reads back, that the contrast ratio is symmetric and lies from one to twenty-one for every pair of colors, and that every gray has an Oklab form whose light fits the gamut; and that every harmony turns within the circle and hue families count the hues a span apart once.
The exhaustive tests grow a palette for every tenth hue, four chromas, both modes, and every harmony, and hold each to the contrasts and the hue count, and grow every carried palette the same way.
The generator's tables are checked against the standard formulas in the task's own tests, and the sampled round trips through Oklab are exact.
The lightnesses and chromas of the roles are a design judgment, which the exhaustive test keeps honest rather than a proof.

## Consequences

A theme author starts from one color or one word and receives colors that keep their contrasts, instead of ten choices that may clash.
A theme written by hand keeps every freedom, and the lints name a faint text color or a crowd of hues at its path.
The engine is a few thousand lines of integer arithmetic rather than a dependency, so the core stays pure and the proofs reach it.
A hue in tenths of a degree is finer than any palette needs and coarser than a color library's floats, which the sampled round trips bound at a few channels for the most vivid primaries.
