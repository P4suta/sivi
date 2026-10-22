---
status: accepted
date: 2026-10-22
---
# Keep proof campaigns manual

## Context

Contract selection reduces some workloads, but an infrastructure or core change still selects every automatic verifier.
Requiring that campaign on each pull request keeps long jobs on the ordinary development path.

## Decision

Commits, pushes, pull requests, and merges require the deterministic project checks, without proof, Miri, fuzz, or mutation campaigns.
Pull requests retain the supported-platform tests, dependency review, CodeQL, and aggregate gates.
The aggregate gate requires exactly its declared jobs and refuses missing, skipped, or failed results.

Verification remains available through explicit project commands and a separate manual `Verification` workflow.
Affected-contract selection supports targeted investigation; the full inventory remains available for deliberate scrutiny.
Every requested verifier retains its properties, fixed bounds, negative controls, source-bound receipts, and resource limits.
A safety stop or missing evidence leaves that investigation incomplete.
An ordinary CI pass makes no claim that an unrun proof campaign passed.

This decision supersedes [requiring selected proofs during routine verification](select-contracts-for-routine-verification.md).
The container preparation, isolation, evidence, and cleanup decisions in that record remain in force.

## Consequences

Ordinary development no longer depends on preparing and running the verification portfolio.
Requiring only a selected campaign was rejected because broad changes still select the expensive inventory.
Proof and mutation coverage need explicit investigation and must be reported separately from the routine CI result.
