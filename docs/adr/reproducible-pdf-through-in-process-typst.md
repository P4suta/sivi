---
status: accepted
date: 2026-10-13
---
# Produce the PDF through the Typst crates in process, with no clock, file system, or network in the compiler

## Context

sivi needs a typesetter that measures what it lays out and writes a PDF.
Driving the Typst binary would mean a second installation to pin, text diagnostics to parse, and no access to the laid-out frames.
Reproducible output needs every input to be explicit: the source, the fonts, the assets, the identity, and the date.

## Decision

`sivi-typst` depends on `typst`, `typst-layout`, and `typst-pdf` at the exact version `0.15.1` and implements Typst's `World` over values alone.
The world serves one generated main source and the asset bytes the caller loaded, keyed by their virtual paths; it reads no file.
`World::today` returns `None`, so no build depends on the day it ran.
The PDF carries a document identifier derived from an identity string the caller supplies and a creation date only when the caller supplies one from `SOURCE_DATE_EPOCH`.
The creator string names sivi and its version.

## Assurance boundary

`the_pdf_bytes_depend_on_the_inputs_alone` in `crates/sivi-typst/tests/boundary.rs` exports the same document twice and requires identical bytes.
The remaining determinism, across operating systems, is checked by the fixture decks once the command line exists.

## Alternatives

A Chromium-based renderer would offer HTML's layout model, but no way to measure a region's required height without a browser process, and no path to a PDF that matches a preview.
Typst's `measure` and frames give both in one pass.

## Consequences

The Typst crates pin the Rust toolchain floor at 1.92 and add minutes to a cold build.
A Typst upgrade is one pull request across the whole family, checked by the boundary tests.
