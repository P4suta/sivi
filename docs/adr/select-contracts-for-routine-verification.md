---
status: accepted
date: 2026-10-22
---
# Select changed contracts for routine verification

## Context

Running every proof and mutation before each push makes ordinary development depend on a long campaign.
Resource ceilings protect the host but don't shorten that feedback cycle.
The container boundary check also prepared every verifier before it could test the boundary.

## Decision

The local push gate runs the deterministic project checks.
Pull requests and merge groups also require the proofs selected by their changed contracts and dependencies.
The selection plan and completed receipts remain bound to the exact source, tool versions, assumptions, and scope.
Unknown inputs conservatively select all automatic verifiers.
Missing evidence, failed negative controls, OOM, and safety stops still prevent integration.

The full proof and mutation inventories remain available through an explicit `just proofs` request or the manual workflow's full-portfolio option.
They aren't prerequisites for an ordinary push or pull request.
An automatic selected campaign has a twenty-minute safety stop; the separate boundary check has fifteen minutes.
An explicitly requested full campaign retains its two-hour stop and requires resource permission on an owner's machine.
These deadlines limit occupancy and never establish a verdict.

The boundary image builds the worker without installing proof tools.
The tool stage is independent of the source stage, so source edits don't invalidate tool preparation in the BuildKit cache.
Official prebuilt nextest, cargo-fuzz, and cargo-mutants binaries replace source installation, with fixed release URLs and SHA-256 checksums.
The existing CPU, memory, swap, process, isolation, evidence, and cleanup contracts stay in force.

Automatic CI runs on pull requests and merge groups, without repeating the same checks on a push to main.
The manual full-portfolio option defaults to off.

This decision amends the execution policy in [the container boundary record](bounded-container-verification.md) and [the verification portfolio](verification-portfolio.md), without changing the properties assigned to each verifier.

## Consequences

A successful push establishes the deterministic checks, while the PR still needs its selected proof results before merging.
Selected results make no claim about the entire proof or mutation inventory.
Full mutation testing remains a deliberate investigation of test strength.
Cold preparation and selected proofs still need measurement; a timeout requires a smaller verified decomposition or a faster implementation, never an exemption.
