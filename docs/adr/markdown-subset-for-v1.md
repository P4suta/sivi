---
status: amended
date: 2026-10-13
---
# Admit a closed Markdown subset and refuse everything else with a named code

## Status

The [release-gate decision](run-heavy-verification-at-the-release-gate.md) moves corpus replays from every push to release acceptance and on-demand runs from 2026-10-22.
The original assurance boundary below records the earlier cadence.

## Context

CommonMark and its common extensions hold constructs a slide cannot carry: raw HTML, footnotes, task lists, math, deep headings, hard breaks, indented code, and thematic breaks that look like separators.
A tool that renders them anyway makes the result depend on the renderer, and a tool that drops them silently loses content.
Authors coming from other tools write HTML comments as directives, which the Markdown record forbids.

## Decision

A slide holds `#` for its title, `##` for a section, paragraphs, bullet and ordered lists of items that hold one paragraph and an optional nested list, fenced code with a language, block quotes of one paragraph, images alone in their paragraph, GFM tables with a header row, and the inline marks emphasis, strong, code, strikethrough, and links to web, mail, or slide addresses.
`subset_rules::admit` decides every node kind the parser can yield, mirrored in full, so a parser upgrade that adds a kind fails to compile until the decision names it.
Every refused construct reports a stable code from `E0201` to `E0221` with the span of the construct and a help that names the Markdown to write instead.
An HTML comment reports `E0203` with the help to name the master in the slide's front matter.
Front matter keys and values follow closed grammars, which `front_matter_rules` decides over bytes.

## Assurance boundary

Kani proves `admit` over every node kind, `front_matter_rules` over every short byte string, and `deck_rules` over every short line.
The fuzz targets replay their corpora before each push and keep the front end from panicking on any byte string.

## Alternatives

Passing unknown constructs through to Typst would make the layout depend on Typst's reading of HTML, which it does not have.
A configurable subset would make a deck compile under one configuration and fail under another.

## Consequences

Math is not part of this version; a later record reconsiders it with a Typst mapping.
Footnotes, task lists, and description lists stay out until a master declares a slot for them.
