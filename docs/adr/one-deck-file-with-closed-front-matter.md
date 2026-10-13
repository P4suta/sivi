---
status: accepted
date: 2026-10-13
---
# Keep a deck in one file and name each slide's master in a closed front matter block

## Context

A slide must name the master that typesets it, because the compiler never guesses a layout from content.
The name can live in a separate manifest that maps one file per slide to a master, or in the deck itself.
A forty-slide deck is reordered, extended, and diffed constantly, and an editor shows one file far better than forty.

## Decision

A deck is one Markdown file.
A line holding only `---` separates slides.
A deck-level front matter block at the start of the file names the theme, the default master, the title, the author, the date, and the language.
A slide may start with a front matter block, opened by the separator line itself, that names its master and an identifier.
Both blocks admit only their closed key set; an unknown key, a deck key in a slide block, or a slide key in the deck block is an error.
A slide without a master uses the deck's default master, and a deck without a default requires one on every slide.
Front matter lines are replaced by spaces of the same length before parsing, so every span points into the file as the author sees it.

## Alternatives

A `deck.toml` manifest with one file per slide keeps the Markdown pure of any key, at the price of two files to keep in order and forty files to edit.
Selecting the master from the block structure removes every key, but two masters with the same structure cannot be told apart, and adding a bullet would change a slide's layout.

## Consequences

The master name is the only structural key a slide carries, and it names an interface, not a style.
The scanner must recognize a front matter block without mistaking a setext underline or a fenced code line for a separator, and the diagnostics name each such case.
