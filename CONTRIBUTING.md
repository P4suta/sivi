# Contributing

Install the versions in `mise.toml` and run commands through `mise x --`.
The [architecture](docs/architecture.md) and [engineering rules](docs/engineering.md) describe the crate boundaries and required source shapes.

## Fast gate

Run `mise x -- just push-check` before pushing.
The pre-push chain is Lefthook, `just push-check`, then `cargo xtask push-check`.
It runs formatting, lints and Clippy, structure checks, decision records, generated repository files, workflow validation, tests on the current host, and dependency checks.
It runs no proofs, Miri, fuzzing, mutation campaigns, stress repeats, or tests on other hosts.

Pull-request CI runs the same fast gate on `ubuntu-26.04` through `cargo xtask ci check`.
That command already runs the workspace tests, so CI needs no second Linux test job.
The `ci-success` and `required` summaries keep their names and depend only on the fast gate.
CodeQL and dependency review retain their existing workflows.

Keep harnesses, models, fuzz targets, corpora, and obligation declarations with the changes they describe.
The fast structure and assurance shape checks still require them.
A heavy check that has not run is unverified.

## Release gate

Dispatch [Release check](.github/workflows/release-check.yml) for the exact candidate ref before publishing it.
The Linux acceptance job runs `just release-check`: the fast gate followed by the complete `just proofs` inventory, including mutation testing.
The workflow also runs native tests on `macos-26` and `windows-2025`.
`release-success` requires every candidate job to succeed.
The workflow publishes nothing, and a failed or inconclusive check blocks release acceptance.
There is no existing release-tag or release-PR workflow to extend, so the trigger is `workflow_dispatch`.

The [proof inventory](docs/verification/proofs.md) lists Kani, Verus, SMT, model checks, Miri, fuzz corpus replays, mutation testing, and each negative control.
The release workflow installs their pinned tools on its dedicated hosted runner, including the additional Rust toolchains and checksum-verified SWI-Prolog.

## On-demand heavy checks

Use `just proofs`, `just proofs --only STEP`, `just mutants`, and `just fuzz TARGET RUNS` when their results guide development.
These commands are outside the push and pull-request gates.
`just proofs-ci` remains a reduced on-demand inventory without mutation testing; it is not the release gate.

On owner machines, run heavy tools through the [bounded container policy](https://github.com/P4suta/dotfiles/issues/309).
`just bounded IMAGE COMMAND...` invokes `dotfiles bounded run` with this checkout mounted as the project.
Use a prepared Linux image identified by an immutable image ID or `name@sha256:digest`, with GNU timeout and the pinned tools and dependencies required by the selected command.
For example, with that image in `SIVI_VERIFICATION_IMAGE`:

```sh
mise x -- just bounded "$SIVI_VERIFICATION_IMAGE" cargo xtask proofs --only kani
mise x -- just bounded "$SIVI_VERIFICATION_IMAGE" mise x -- just mutants
mise x -- just bounded "$SIVI_VERIFICATION_IMAGE" cargo xtask fuzz deck_scanner 20000
```

The runner applies the owner's memory, swap, CPU, process, and time limits.
A resource stop is inconclusive and supplies no passing evidence.
Do not invoke Kani, Verus, Miri, cargo-fuzz, cargo-mutants, Z3, or other protected verifiers natively on an owner machine.
Use the hosted release workflow for the complete inventory and its Docker-based model adapters.

The [decision record](docs/adr/run-heavy-verification-at-the-release-gate.md) explains the split and its costs.
