---
status: accepted
date: 2026-10-13
---
# Assign each verifier one class of property and prove nothing twice

## Context

No single verifier covers every property of a compiler.
Kani proves bounded properties of executable Rust.
Verus proves unbounded properties of executable Rust with specifications.
Quint and TLA+ check transition systems.
Alloy explores relational models of data.
Z3 decides arithmetic lemmas.
An independent Prolog reference catches a shared misreading of the specification.
Miri, fuzzing, and mutation testing cover undefined behavior, input boundaries, and test strength.
Using every tool everywhere would prove the same property several times, cost hours per run, and let the proofs drift from each other.

## Decision

Every property has exactly one owning verifier, declared in `verification/obligations.toml`.

| Verifier | Class of property |
| --- | --- |
| Kani | Bounded and complete decisions over fixed-width integers and byte arrays, and the arithmetic safety of every rule |
| Verus | Unbounded algorithmic properties of rules over slices of any length |
| Quint | Invariants and reachability of the slot binding automaton, bound to `binder::step` by trace replay |
| TLA+ | Temporal safety and liveness of the atomic PDF publication under crashes, bound by fault injection |
| Alloy | Consistency of the theme model and the validity of its own constraints |
| Z3 | Rounding and tolerance lemmas about millipoints, generated from the core's constants |
| Prolog | An independent executable reference of slot binding, compared on fixtures and generated inputs |
| Miri | Undefined behavior in the pure crates and the Typst adapter's own units |
| cargo-fuzz | Every parser entry point, with a committed corpus replayed before each push |
| cargo-mutants | The strength of the tests of the pure crates, with a ceiling of zero missed and zero timed-out mutants |

Each verifier has a negative control: a fixture or probe that asserts a false claim, which `cargo xtask proofs` requires the verifier to refuse before any of its results count.
Lean 4 and Loom are not used in the first version.
Lean is reconsidered when a property appears that no implementation-level verifier closes.
Loom is reconsidered when a watch mode introduces threads.

## Alternatives

Kani alone would leave every slice-length property bounded.
A Lean model of the whole compiler would need a refinement to the implementation that no tool checks.

## Consequences

Adding a rule means choosing its verifier and declaring the obligation in the same change.
The proof gate runs several tools, some in pinned containers, so a full `just proofs` takes minutes rather than seconds.
Verus entered through a spike, which `docs/adr/verus-for-unbounded-algorithmic-properties.md` records as adopted: the standalone driver verifies the core without adding a dependency.
