---
status: accepted
date: 2026-10-13
---
# Bundle the default fonts in the binary and never read the host's fonts

## Context

The same deck must produce the same PDF on every machine.
A font that differs between hosts changes line breaks, so it changes whether a slide fits.
Typst's own embedded fonts cover no Japanese, and the host's fonts differ by operating system and installation.

## Decision

The binary embeds BIZ UDPGothic Regular and Bold for text and JetBrains Mono Regular for code, all under the SIL Open Font License 1.1, with the license texts beside them under `fonts/`.
`fonts/manifest.toml` pins each file's SHA-256 and source, and `cargo xtask fonts verify` refuses a file that drifts, a file the manifest does not name, or a font without its license.
The Typst font book holds the bundled fonts and the fonts a theme ships, nothing else.
A theme font is a file pinned by its SHA-256 in the theme.
A character no font covers is reported as a missing glyph, never substituted silently.

## Alternatives

Noto Sans JP covers more rare kanji at the same size, but BIZ UDPGothic is designed for legibility at projection distance and ships on Windows and macOS, so decks look familiar.
Downloading fonts on first use would keep the binary small at the price of a network dependency and a cache that can drift.

## Consequences

The binary grows by about ten megabytes.
A deck that needs another face adds it to its theme with a digest, and the compiler refuses an unpinned or altered file.
Changing a bundled font is a visible change to the manifest and to every fixture snapshot.
