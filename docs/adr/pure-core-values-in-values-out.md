---
status: accepted
date: 2026-10-13
---
# Keep every decision in a dependency-free core that takes values and returns values

## Context

A slide compiler decides many things: how a deck splits into slides, which slot each block binds to, whether measured content fits its region, and what change would make it fit.
Each decision must be provable, and a proof covers only code whose inputs are values.
A decision that reads a file, a clock, or an environment escapes every verifier.

## Decision

`crates/sivi-core` holds every decision.
It is `#![no_std]`, declares no dependency in any dependency table, and denies the panic, indexing, and exit lints in its own crate root.
The core defines no port trait; the pipeline is a straight line, so each adapter gathers its facts and passes them in as values, and the core returns the next value.
Public core functions return named types, never bare text.
Lengths are integer millipoints, and the one conversion from Typst's floating point lives in `sivi-typst`.
`cargo xtask structure` refuses a breach, and the machine-wide structural catalog refuses an effect, a panic, shared state, or bare text in the core.

## Alternatives

A port trait in the core would let adapters be mocked, but a mock is a second implementation of the boundary that no proof covers, and the pipeline needs no fact midway.
Allowing `serde` in the core would save one conversion per type, at the price of a dependency whose semantics no verifier imports.

## Consequences

Every parser, loader, and renderer is an adapter that converts to and from core values.
Kani, Verus, and the other verifiers import production functions directly.
The core cannot format floating point, so measurements arrive already rounded.
