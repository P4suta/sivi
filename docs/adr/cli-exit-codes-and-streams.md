---
status: accepted
date: 2026-10-13
---
# Exit codes and streams of the command line

## Context

A build runs in editors, in scripts, and in continuous integration, each of which reads a different signal.
A person reads the diagnostics; a program reads the exit status and, when asked, one JSON document.
A stray line on the stream a program reads corrupts the document it parses.

## Decision

`sivi build DECK` writes the PDF beside the deck as `DECK.pdf`, or where `-o` names, or to standard output for `-o -`.
`sivi check DECK` runs the same pipeline through the layout verdicts and writes nothing.
`sivi theme check THEME` validates a built-in theme by name or a directory holding `theme.toml`.
Standard output carries only the PDF bytes or the JSON document; diagnostics for a person go to standard error.
`--format json` writes one document of format `sivi.diagnostics` version 1 to standard output, so it cannot be combined with `-o -`.
The exit status is 0 when no error was reported, 1 when an error was reported or a warning under `--deny-warnings`, 2 for a usage error or an input that could not be read, and 3 when a diagnostic names a defect of sivi or its adapters, the codes `E0001` and `E09xx`.
`SOURCE_DATE_EPOCH` fixes the PDF creation date; without it the PDF carries no date.
`NO_COLOR` and `--color` decide styling; standard error alone is tested for a terminal.
The binary reads its environment once, in `args`, before any decision.

## Assurance boundary

The exit code decision and the sink choice are unit tests over the diagnostic lists and the paths.
The streams have one owner, `output`, which the lints keep as the only caller of standard output.
The fixtures under `fixtures/decks/` run through the real pipeline in `crates/sivi/tests/decks.rs`.

## Alternatives

Writing diagnostics to standard output would let a program parse them without a flag, but every PDF to standard output would then need a separate channel.
A single exit status for every failure would hide a defect of sivi behind a deck's error.

## Consequences

A script tells "fix the deck" from "fix the tool" by the exit status alone.
The JSON document is the contract for editors, and `docs/schema/diagnostics-1.json` will describe it.
