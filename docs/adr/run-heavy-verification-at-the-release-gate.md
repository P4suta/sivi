---
status: accepted
date: 2026-10-22
---
# Run heavy verification at the release gate

## Status

Accepted on 2026-10-22 under the [shared verification boundary](https://github.com/P4suta/dotfiles/pull/325) and [issue 11](https://github.com/P4suta/sivi/issues/11).
This amends the check timing in [the verification portfolio](verification-portfolio.md), [the Markdown subset](markdown-subset-for-v1.md), and [local delivery](local-only-delivery.md).

## Context

The pre-push hook ran every verifier and the mutation campaigns.
Pull-request CI repeated proofs and tests on Linux, macOS, and Windows.
Daily development waited for the campaigns needed to accept a release candidate.
The proof inventory and its negative controls remain necessary, and owner machines require heavy tools to run in bounded containers.

## Decision

Lefthook continues to call `just push-check`, which calls `cargo xtask push-check`.
That entry point runs only the fast gate on the current host: formatting, lints, architecture checks, type checks through Clippy, tests, and the existing dependency checks.
Pull-request CI runs the same gate on `ubuntu-26.04`.
The `ci-success` and `required` summaries retain their names and depend only on fast jobs.
The existing CodeQL and dependency-review workflows remain in place.

[Release check](../../.github/workflows/release-check.yml) is a non-publishing workflow dispatched for the exact candidate ref.
Its Linux acceptance job runs the fast gate and the entire proof inventory, including Kani, Verus, SMT, model checks, Miri, fuzz corpus replays, mutation testing, and every negative control.
Separate jobs run native macOS and Windows tests, and `release-success` requires all candidate jobs.
No release-tag or release-PR workflow exists to extend.
Future publication must require successful candidate verification.

Harnesses, models, fuzz targets, and their sources remain unchanged.
The fast structural gates continue to require their source shapes.
Heavy commands remain available on demand through the bounded route in [Contributing](../../CONTRIBUTING.md#on-demand-heavy-checks), with immutable images and the owner's resource policy.
Dedicated GitHub Actions runners retain native verification.

## Alternatives

Selecting proofs, fuzzing, mutation, or operating systems by changed paths would keep heavy work in the daily gate and retain its delay.
Removing heavy verification would leave releases without its evidence.
Keeping a second Linux test job would repeat tests already run by the fast gate.

## Consequences

Pushes and pull requests wait only for the fast gate.
A proof failure, fuzz crash, surviving mutant, or platform defect can reach the default branch and must be fixed before release acceptance.
An unrun or inconclusive heavy check remains unverified.
The first hosted release run must establish the complete candidate result; workflow validation alone does not supply it.
