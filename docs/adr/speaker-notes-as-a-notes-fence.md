---
status: accepted
date: 2026-10-13
---
# Carry speaker notes in a fenced block whose info string is `notes`

## Context

Presenters write notes beside each slide's content.
Other tools carry them in HTML comments, which the Markdown record forbids, or in a parallel file, which drifts from the deck.

## Decision

A fenced code block whose info string is `notes` holds the slide's speaker notes.
A slide carries at most one; a second reports `E0215`.
The notes reach the JSON report as text and never the page.

## Assurance boundary

The front end's tests and the fuzz corpus cover a slide with one notes block, with two, and with none.

## Alternatives

An HTML comment would reintroduce the comment directive the record refuses.
A `notes:` key in the front matter would hold one line at most.

## Consequences

A later version may render notes into a presenter document; the model already carries them with their span.
