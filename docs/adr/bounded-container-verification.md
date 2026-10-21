---
status: accepted
date: 2026-10-21
---
# Bound image preparation and verification at the Docker boundary

## Context

A proof must establish the same claim on a quiet machine and a busy hosted runner.
The previous Kani cutoff discarded valid work after sixty seconds, while its host had no enforced resource ceiling.
Several other verifiers depended on tools installed outside their declared environment.

## Decision

The repository runs expensive verification in pinned Linux containers through `xtask::containers`.
Each worker has at most two CPUs, four GiB of memory, no swap, and 256 processes and threads.
One reservation named `sivi-proof-campaign` excludes other sivi campaigns on the same Docker engine, including campaigns from other worktrees.
The controller refuses missing cgroup v2 capabilities and inspects the limits before starting a worker.
The Rust worker checks the kernel's cgroup limits again before invoking a verifier.
The source mount is read-only, the network is disabled, and writable outputs belong to that job.

Image preparation uses a dedicated rootless BuildKit container with the same resource ceilings and one build step at a time.
Rootless BuildKit removes the executor's separate cgroup configuration, so its children inherit the bounded parent container.
The runner checks the builder's non-root user and refuses privileged operation.
It connects through Docker's local transport, exposes no port, and stops the builder before verification starts.
The maintained upstream implementations are trust assumptions: [BuildKit rootless conversion](https://github.com/moby/buildkit/blob/f5b59a1f2fb25a0cb7072c398cab9aed7835da4e/util/rootless/specconv/specconv_linux.go) and [Docker's remote builder transport](https://docs.docker.com/build/builders/drivers/remote/).
Docker's daemon, image transport, and kernel resource accounting remain outside the repository's implementation.

Kani runs one harness at a time without a per-harness correctness deadline.
Input domains, unwinding bounds, assertions, covers, and named negative controls remain authoritative.
A two-hour campaign deadline limits occupancy rather than deciding correctness.
OOM, cancellation, infrastructure errors, and safety stops leave incomplete evidence and stop the campaign.
The runner preserves logs, immutable container and image identities, and terminal state before removing owned objects.
It retains the reservation when it can't confirm cleanup.
It neither retries expensive failures nor raises resource limits automatically.

Affected selection unions each changed input's verifiers.
Known model files select their owning checker, documentation selects none, and every other input selects the full portfolio.
The default command still selects every verifier, including mutation testing.
Automatic hosted checks omit the mutation campaign and preserve an explicit selection plan.
The manual workflow can request the full portfolio, including mutation testing.
Receipts bind the source digest, selected scope, tool versions, image identity, limits, and completed result.
No receipt is reused across source changes.

## Consequences

Contributors need a Docker engine with cgroup v2 CPU, memory, swap, and process controls.
On an Apple Silicon Mac, the Rust proof image uses linux/amd64 emulation because Kani's Linux release is x86_64.
Image preparation needs network access and keeps a shared BuildKit cache with a ten-GB garbage-collection target.
The first preparation can take substantially longer than later runs.
The resource boundary is checked with `cargo xtask proofs --self-check`, whose final fixture intentionally exceeds the memory ceiling.
That boundary check is separate from the proof portfolio and never supplies a proof receipt.
