# Proofs

`cargo xtask proofs` discharges every obligation of `verification/obligations.toml` and refuses each verifier's fixture under `xtask/proofs/`.
Every verifier owns one class of property, as `docs/adr/verification-portfolio.md` records, so no property is proved twice.

| Verifier | Pinned version | Obligations | Negative control |
| --- | --- | --- | --- |
| Kani | 0.68.0 | The harnesses of every `*_rules.rs` module, with the covers each declares | Every `rejects_*` probe, and `xtask/proofs/kani-negative`, whose harness asserts a false claim |
| Verus | 0.2026.10.04.426d8b0 on Rust 1.98.1 | The functions `verification/obligations.toml` lists under `verus`, verified for inputs of any size with `--no-cheating` | `xtask/proofs/verus-negative`, whose specification states the opposite of its function |
| Z3 | 5.1.0 | `specs/smt/millipoints.smt2`, generated from the core's constants: the fixed-point verdict agrees with the real one outside a band of one millipoint | `specs/smt/millipoints-negative.smt2`, a claim rounding breaks, which Z3 must find satisfiable |
| Miri | nightly-2026-08-21 | The tests of every crate that never calls Typst | `xtask/proofs/miri-fixture`, which reads past a slice |
| cargo-fuzz | 0.13.2 | Every target of `fuzz/` over its committed corpus | `xtask/proofs/fuzz-fixture`, whose corpus makes its target panic |
| cargo-mutants | 27.1.0 | The campaign over `sivi-core`, with zero missed and zero timed-out mutants | `xtask/proofs/mutants-fixture`, whose test asserts nothing |

Each run writes a receipt under `target/proofs/receipts/` with the verifier's version and a digest of the core sources and the manifest.

## What each rule module proves

| Module | Kani (bounded, with a rejecting probe) | Verus (unbounded) |
| --- | --- | --- |
| `fit_rules` | `fit` overflows exactly when the excess passes the tolerance, over every length within `BOUND`; a smaller measurement fits wherever a larger one fits; `pages` names the exact difference | `fit` reports `Fits` exactly when the excess is within the tolerance |
| `split_rules` | the verdict names the longest fitting prefix for up to six parts | the verdict names the longest fitting prefix for prefixes of any length |
| `geometry_rules` | three regions: a layout is valid exactly when no region is degenerate, outside, or overlapping; overlap is symmetric | — |
| `escape_rules` | every character's spelling reads back to the character and holds no raw control character or quote | — |
| `softbreak_rules` | a soft break vanishes exactly between two East Asian characters, for every pair | — |
| `attribute_rules` | a point belongs to the first region containing it; a height falls in the first part whose prefix passes it, for three regions or parts | — |

The Typst layout engine is an external boundary; `docs/verification/typst-boundary.md` lists the assumptions and the tests that keep them.
