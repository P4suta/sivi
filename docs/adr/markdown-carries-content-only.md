---
status: accepted
date: 2026-10-13
---
# Let Markdown carry content only and make every layout directive a compile error

## Context

Markdown slide tools let authors reach for HTML, CSS classes, or comment directives whenever a slide does not look right.
From that moment the content depends on one rendering, every slide needs a visual check, and a theme change breaks decks.
A compiler can only verify what it controls.

## Decision

A deck is CommonMark with the GFM table and strikethrough extensions and a closed set of front matter keys.
Raw HTML blocks, inline HTML, and HTML comments are errors.
Hard line breaks, heading levels below two, footnotes, task lists, math, indented code, nested quotes, and reference links that do not resolve are errors, each with its own diagnostic code.
Images stand alone in a paragraph and name a file inside the deck's directory.
A fenced block whose info string is `notes` holds the speaker's notes and never reaches the page.
The front matter keys name a theme, a default master, a title, an author, a date, a language, and per slide a master and an identifier; no key changes a size, a position, or a color.

## Alternatives

Admitting HTML comments as directives, as Marp does, reintroduces the escape hatch under another spelling.
Admitting a per-slide style key would make a slide depend on one theme again.

## Consequences

Authors who need a different look change the theme, not the deck.
The compiler knows every construct it typesets, so it can measure and refuse all of them.
Some Markdown that other tools accept is refused here, and each refusal names its reason.
