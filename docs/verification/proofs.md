# Proofs

`cargo xtask proofs` discharges every obligation of `verification/obligations.toml` and refuses each verifier's fixture under `xtask/proofs/`.
Every verifier owns one class of property, as `docs/adr/verification-portfolio.md` records, so no property is proved twice.

| Verifier | Pinned version | Obligations | Negative control |
| --- | --- | --- | --- |
| Kani | 0.68.0 | The harnesses of every `*_rules.rs` module, with the covers each declares | Every `rejects_*` probe, and `xtask/proofs/kani-negative`, whose harness asserts a false claim |
| Verus | 0.2026.10.04.426d8b0 on Rust 1.98.1 | The functions `verification/obligations.toml` lists under `verus`, verified for inputs of any size with `--no-cheating` | `xtask/proofs/verus-negative`, whose specification states the opposite of its function |
| Z3 | 5.1.0 | `specs/smt/millipoints.smt2`, generated from the core's constants: the fixed-point verdict agrees with the real one outside a band of one millipoint | `specs/smt/millipoints-negative.smt2`, a claim rounding breaks, which Z3 must find satisfiable |
| Alloy | 6.2.0 in a pinned image, offline | `specs/theme.als`: under the theme validators a point lies in at most one slot interior, every section number names one slot, and every `follows` chain ends; each with an instance | Each assertion without its validator, which Alloy must refute |
| Quint | 0.33.0 with Apalache 0.62.1 in a pinned image, offline | `specs/binder.qnt`: the binding automaton holds `safe` in every reachable state under TLC with the recorded state count, witnesses every outcome, and reproduces the committed traces, which `crates/sivi/tests/binder_replay.rs` replays through `binder::step` | `binder_probe`, the automaton without capacities, which TLC must refute |
| Prolog | SWI-Prolog 10.0.2 | `specs/binding.pl`, an executable reference of the binding grammar written apart from the binder: every fixture slide and two hundred generated slides bind the same way under both | A tampered binding of one slide, which the comparison must refuse |
| Miri | nightly-2026-08-21 | The tests of every crate that never calls Typst; in the theme loader, the tests over the full plain theme and the bundled fonts are skipped, since Miri interprets each in minutes, and a small theme stands in for them | `xtask/proofs/miri-fixture`, which reads past a slice |
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
| `deck_rules` | a line is a separator, a code fence, a key and value, a blank, or content exactly as the deck format says; a fence closes only with its marker at its length or more; a table row declares its cells as GFM counts them, for lines of up to six bytes | — |
| `front_matter_rules` | identifiers, dates, and language tags follow their grammars, complete for dates and tags and up to seven bytes for identifiers; every key is allowed exactly where it belongs | — |
| `subset_rules` | what passes is exactly the Markdown subset, and raw markup never does, for every node kind | — |
| `theme_rules` | section numbers are one to their count each once; a `follows` chain ends inside its master or the fault is named, for three slots; a line box fits exactly when the pitch covers the face | — |
| `slot_rules` | a slot admits a block exactly when the kind and every measure fit; the follower of the last slot comes first and the first flow slot otherwise, for three slots | — |
| `master_rules` | the edit distance is zero only between equal names, symmetric, and bounded, for names of up to four bytes | — |

## What the fuzz targets replay

| Target | Entry point | Properties |
| --- | --- | --- |
| `deck_scanner` | `sivi_markdown::scanner::parse` | no panic; every slide's text covers exactly the lines it spans; the scan is deterministic |
| `front_matter` | `sivi_core::front_matter::parse` | no panic; a refusal names a diagnostic |
| `markdown_ir` | `sivi_markdown::parse` | no panic; the model is withheld exactly on an error; slides are numbered in order; the parse is deterministic |
| `theme_units` | `sivi_theme::units::parse` | no panic; a unit that reads prints back to text that reads to the same unit |
| `theme_toml` | `sivi_theme::parse` | no panic; a theme that loads has masters with slots; a refusal names a diagnostic; the load is deterministic |

The Typst layout engine is an external boundary; `docs/verification/typst-boundary.md` lists the assumptions and the tests that keep them.
