# Proofs

`cargo xtask proofs` discharges every obligation of `verification/obligations.toml` and refuses each verifier's fixture under `xtask/proofs/`.

| Verifier | Pinned version | Obligations | Negative control |
| --- | --- | --- | --- |
| Kani | 0.68.0 | The harnesses of every `*_rules.rs` module, with the covers each declares | Every `rejects_*` probe, and `xtask/proofs/kani-negative`, whose harness asserts a false claim |
| Miri | nightly-2026-08-21 | The tests of `sivi-core`, `sivi-markdown`, and `sivi-theme` | `xtask/proofs/miri-fixture`, which reads past a slice |
| cargo-fuzz | 0.13.2 | Every target of `fuzz/` over its committed corpus | `xtask/proofs/fuzz-fixture`, whose corpus makes its target panic |
| cargo-mutants | 27.1.0 | The campaign over `sivi-core`, with zero missed and zero timed-out mutants | `xtask/proofs/mutants-fixture`, whose test asserts nothing |

Each run writes a receipt under `target/proofs/receipts/` with the verifier's version and a digest of the core sources and the manifest.
No obligation exists yet; the first rule modules arrive with the core rules, and this page lists them as they land.
