---
status: accepted
date: 2026-10-13
---
# Publish the PDF atomically through a staged file

## Context

A build may fail after it started writing, and a reader may open the output while it is written.
A path that names a half-written PDF is worse than a path that names the previous one.

## Decision

The PDF is written to a staging file beside the output, synced to the disk, and renamed into place.
Before the rename the output is untouched; after it the output is complete.
A failure at any step removes the staging file and keeps the primary error.
The order of the steps and what a failure undoes are decisions of `publish_rules` in the core; `sivi::publish` is the one owner of the file writes and follows them step by step through an `Fs` capability.

## Assurance boundary

`publish_rules` proves under Kani that the rename comes only after the sync, that every stage reaches the end, and that a failure removes the staging file exactly when it exists.
`specs/publish.tla` models the protocol with a failure and a crash at every step; TLC checks that the output never names a partial file, that success leaves the new file, that failure leaves the old one, and that every run ends.
The probe variant of the model writes the output in place, and TLC must refute it.
The fault injection tests of `sivi::publish` drive every failing step through a fake file system and check the same invariants on the files it leaves.

## Alternatives

Writing the output in place is simpler and leaves a partial file on any failure.
Writing to a temporary directory and moving across file systems is not atomic.

## Consequences

The output directory must allow a second file beside the output, named after the process.
A crash before the sync may leave a staging file, which the next run overwrites.
