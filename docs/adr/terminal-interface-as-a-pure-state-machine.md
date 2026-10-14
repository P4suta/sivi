---
status: accepted
date: 2026-10-14
---

# The terminal interface as a pure state machine

## Context

A person writing a deck builds it, reads the diagnostics, looks at the pages, and tries a theme, a font, a palette, or a code theme, in a loop that the command line serves one call at a time.
A terminal interface serves the loop in one place, but an interface is effects all the way down: the terminal, a watcher, a build that takes a second, a viewer, and the network for a font.
The project holds every decision in the core as a value function, and an interface that mixed its decisions into its effects would escape every verifier.

## Decision

The interface is a state machine in the core, `ui_rules::step`, over a state of the screen, the job, the watch flag, the selections, the chosen theme, and the last verdict, and over the events a shell reads: a key, the end of a build, a change of a file, and a tick.
Each step hands back the state after it and at most one effect: start a build, open the viewer, install a font, quit, or show a note.
The shell in the binary owns the terminal, the watcher, the build thread, and the viewer, carries out the one effect, and feeds what comes back to the machine; it decides nothing.

The machine refuses a second build while one runs, keeps a change that arrives during a build for the next one, moves every selection within its count, and leaves every state quitting on the quit key.
`specs/ui.qnt` models the machine with the same events and effects, the model checker holds those four properties over every reachable state, a probe without the guard breaks them, and the committed traces replay through the core's step in a test.
Kani proves the same properties over every state and every event, which the model checker reaches only within its bounds.

The page shows through the terminal's image protocol where it speaks one, and as a mosaic of half blocks elsewhere, which the core shrinks from the raster the Typst adapter renders.

## Assurance boundary

The structure gate admits a protocol step in every module the assurance manifest names as a protocol owner, so a second machine beside the binder keeps the same owner, model, and replay.
The shell's effects are kept by their owners: the terminal in one module that takes standard output once, the viewer in one module that starts a child once, the watcher through its library, and the build through the same request `sivi build` serves.
What the terminal draws is not verified; the layout is a function of the state, and a person reviews it.

## Consequences

Every key of the interface is a value a test feeds to the machine, so a change of the keys or the screens is a change of the model first.
A terminal without an image protocol still shows the pages, coarsely.
The binary carries a terminal library, a watcher, and a base64 encoder, which the command line never uses.
