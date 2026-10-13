---
status: accepted
date: 2026-10-13
---
# Prove the unbounded properties of the core's rules with Verus through the standalone driver

## Context

Kani proves the core's rules for inputs up to a bound: six parts, three regions, one character.
The split rule and the geometry rule take slices of any length, and a bounded proof leaves the general claim to tests.
Verus verifies Rust code against specifications for all inputs, but it is usually driven through `cargo verus`, which adds `vstd` as a dependency of the verified crate.
The core must stay free of dependencies.

## Decision

The standalone `verus` driver of the pinned release `0.2026.10.04.426d8b0` verifies `crates/sivi-core/src/lib.rs` with `--no-cheating`.
Specifications sit behind `cfg_attr(verus_keep_ghost, verus_spec(..))`, spec functions behind `cfg(verus_keep_ghost)` inside `verus!` blocks, and the prelude import behind the same cfg.
`cargo` never sets that cfg, so the core compiles with no dependency and the structure gate stays satisfied.
`cargo xtask proofs` downloads the release by digest, runs the driver on the toolchain the release pins, requires every listed function to verify, and requires the fixture `xtask/proofs/verus-negative` with a false specification to fail.

The spike established what the driver accepts.
Specifications attach to free functions, not to methods, so the verified rules read the millipoint value through the public field of `Length`.
Loops are verified in isolation, so a loop restates the facts of the function's precondition in its invariants and ends by shrinking its bound rather than through `break`, which would discard those facts.
Equality on a verified enum goes through a `match`, because a derived `PartialEq` is opaque to the verifier.
The one indexing expression the proof needs carries an `expect` on the indexing lint, with the proof as its reason.

## Assurance boundary

Verus owns the unbounded algorithmic properties of `split_point` and `fit`.
Kani keeps the bounded arithmetic properties and the rejecting probes of the same modules; the two verifiers prove different properties, never the same one twice.
`verification/obligations.toml` lists every Verus-verified function with its contract.

## Alternatives

Lean 4 with a model of the split rule would need a refinement to the implementation that no tool checks.
Widening the Kani bounds would raise the proof time without reaching every length.

## Consequences

The proof gate downloads a release archive on first use and keeps it under the build directory.
A Verus release without a build for the host, such as Linux on arm64, runs the step in the pinned Linux container.
The `pub` field of `Length` is part of the API, because the specifications read it.
