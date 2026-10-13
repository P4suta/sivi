---
status: accepted
date: 2026-10-13
---
# Describe a theme as declarative TOML that the compiler turns into Typst

## Context

A theme is where every visual decision lives.
Typst templates would let a theme author run code, which makes a theme impossible to validate before a deck compiles and lets one theme break the measurement protocol.
A theme written as data can be validated as a whole, compared across versions, and reasoned about in a model.

## Decision

A theme is one `theme.toml` of schema one: a page, colors, text styles, inline marks, list, code, table, and quote styles, optional font files, and masters.
A master is a map of slots in declaration order, each binding the title, the subtitle, one numbered section, or the flow of blocks, with the block kinds it accepts, its capacity, its region, and the text style it sets.
Decor draws rectangles, images, page numbers, and head fields without binding content.
The loader refuses unknown keys, values outside their grammar, references to names the theme does not define, regions off the page or overlapping, sizes below the minimum, and bindings that contradict each other, each by its path in the file.
The compiler carries one theme, `plain`, with eight masters over the bundled fonts.
The theme's font faces are checked against the faces its fonts provide, weight by weight, and every line height against its face's natural line box.

## Assurance boundary

`theme_rules` proves section numbering, `follows` chains, and the line box inequality for every small master.
`geometry_rules` proves the region checks.
`specs/theme.als` shows that under the validators a point lies in at most one slot interior, every section number names one slot, and every `follows` chain ends, which is what the binder and the attribution rules rely on.

## Alternatives

A Typst file per theme would be more expressive and impossible to validate without running it.
A CSS-like cascade would hide which rule set a value, which is the Marp failure the project exists to avoid.

## Consequences

A theme author learns one TOML schema and gets every fault named by path before any deck compiles.
New visual features enter as schema versions, with the loader refusing a schema it does not read.
