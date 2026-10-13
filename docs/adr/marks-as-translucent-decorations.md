---
status: accepted
date: 2026-10-14
---

# Marks as translucent decorations

## Context

Bold and italic carry little weight on a slide, where the audience reads from a distance and the designer wants one phrase to stand out.
A highlighter band behind the words and a thick translucent line under them are the two decorations presentation designers reach for most.
The deck format admits only plain inline marks, and the theme format could paint a mark as a solid box or a plain underline alone.
A decoration with transparency needs a color with an alpha, which Typst takes as `rgb("#rrggbbaa")`, and the theme states opacities as decimals.

## Decision

The deck format admits `==text==`, which the Markdown parser yields as its highlight node and the model holds as the inline `Mark`.
The theme styles it under `[inline] mark`, beside `emphasis`, `strong`, `code`, and `link`, and every theme names it.

Every mark may carry a `marker` and an `underline` besides its text changes.
A marker is a band behind the text: a color, an opacity from zero to one, a top edge and a bottom edge as Typst names the edges of a run, an extent past the text, and a corner radius.
An underline is `true` for the plain line, or a table of a color, an opacity, a thickness, an offset, an extent, and `behind`, which lays the line behind the glyphs and across their descenders.
An opacity needs a color beside it, since a line without a color takes the text color, whose alpha the theme never states.

The opacity becomes the alpha byte of the color in the core's `mark_rules`, rounded to the nearest byte, so `0.5` paints as `0x80` and only `1` paints solid.
The emitter spells a marker as Typst's `highlight` and an underline as Typst's `underline`, and leaves every argument out that holds its default, so a plain underline reads as before.

## Assurance boundary

Kani proves that the alpha is a byte exactly within clear to solid, the byte nearest to the exact scaling, and never decreasing as the opacity grows, over every pair of opacities, and a probe shows the harness refuses an opacity above solid.
The loader refuses an opacity outside clear to solid, an edge outside Typst's names, and an opacity without a color, each at its path.
That Typst keeps the alpha through layout to the raster is an assumption of the Typst boundary, which a boundary test keeps by rendering a translucent band over white and reading the blended pixel.

## Consequences

A theme author decorates `emphasis`, `strong`, `link`, and `mark` alike, so a highlighter can stand behind `**strong**` in one theme and behind `==mark==` in another.
A deck written for one theme shows its marks in every theme, since every theme names `mark`.
The paint order follows Typst: a background box, then the marker band, then the underline, each behind the glyphs when it is a background decoration.
A rotated or hand-drawn band is outside the format, since the decoration is a Typst text decoration and never a drawn shape.
