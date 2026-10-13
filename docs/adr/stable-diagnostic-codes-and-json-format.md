---
status: accepted
date: 2026-10-14
---
# Stable diagnostic codes and one versioned JSON document

## Context

Editors, scripts, and people read sivi's diagnostics.
A code that changes meaning between releases breaks every filter written against it, and a message that changes wording breaks every script that parsed prose.

## Decision

Every diagnostic carries a code of the form `E0000` or `W0000`, drawn from one enum in the core, which the catalog `docs/diagnostics.md` lists.
A code never changes meaning and is never reused; a withdrawn code stays reserved.
A new diagnostic takes the next free code of its hundred: `E01xx` input, `E02xx` forbidden syntax, `E03xx` structure, `E04xx` layout, `E05xx` theme, `E06xx` fonts and assets, `E09xx` defects of sivi, `W01xx` deck warnings, `W05xx` theme warnings.
The catalog is generated from the enum and its documentation by `cargo xtask gen`, and `cargo xtask check` refuses drift.
`--format json` writes one document of format `sivi.diagnostics` and version 1, described by `docs/schema/diagnostics-1.json`; a change that removes or retypes a field raises the version.
Messages, notes, and helps are for people; a program keys on the code, the spans, and the slide.

## Assurance boundary

The enum lists every code in `Code::ALL`, and a test holds each code to its hundred and its severity by its first letter.
The catalog generator reads the enum's source, so a variant without a documentation line fails the generation.
The JSON rendering is tested field by field, and the fixtures' documents are snapshotted.

## Alternatives

Free-form messages without codes read well but cannot be filtered.
A JSON Lines stream would let a build report as it goes, but the pipeline stops at its first failing stage, so one document is enough.

## Consequences

A deck's diagnostics can be compared across sivi versions by code.
Adding a diagnostic is one enum variant, one documentation line, and a regenerated catalog.
