---
status: amended
date: 2026-10-13
---
# Deliver the first version through local commits without a hosted repository

## Status

The [release-gate decision](run-heavy-verification-at-the-release-gate.md) amends the gate timing from 2026-10-22.
`just push-check` now runs only the fast gate, and release acceptance requires the heavy inventory.
The original local-delivery decision below records the initial workflow.

## Context

The owner's other repositories are developed through issues, draft pull requests, hosted checks, and a review tool.
This project is built to completion on one machine first, and the owner asked that no issue, pull request, or hosted repository be involved.

## Decision

The checkout is a local Git repository with no remote.
Each phase ends in one or more Conventional Commits on `main`, and nothing is pushed.
The machine-wide hooks still run on every commit: the assurance shapes, the structural catalog, the comment and prose checks, and this repository's `lefthook.yml`.
The hosted workflow files are written to the owner's standard and validated with actionlint and zizmor, but no hosted run exists, and the records say so.
`just push-check` is the complete gate that a push would require, run before each phase's commit.

## Alternatives

Creating the hosted repository first would let the review tool and the hosted checks run, at the price of a workflow the owner declined for this project.

## Consequences

Evidence of other operating systems comes from the owner's machines through `multi-machine`, when they are available, and is reported as such.
Publishing the repository later needs the governance baseline applied after the first hosted run, as the owner's repository skill describes.
