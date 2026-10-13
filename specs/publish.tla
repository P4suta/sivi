---- MODULE publish ----
\* The atomic publication of the PDF, as `sivi::publish` performs it and `publish_rules` orders it.
\* The output path never names a partial file, a finished run leaves the new file, and a failed run leaves the old one.
\* With Safe = FALSE the module writes the output in place, which a crash leaves partial; TLC must refute that variant.

CONSTANT Safe

VARIABLES
    output,  \* what the output path names: "old", "new", or "partial"
    staging, \* the staging file beside it: "none", "partial", "complete", or "synced"
    pc       \* where the publication stands

vars == <<output, staging, pc>>

Init ==
    /\ output = "old"
    /\ staging = "none"
    /\ pc = "start"

\* The four steps of the safe protocol, in the order publish_rules::next decides.
Create ==
    /\ pc = "start"
    /\ staging' = "partial"
    /\ pc' = "created"
    /\ UNCHANGED output

Write ==
    /\ pc = "created"
    /\ staging' = "complete"
    /\ pc' = "written"
    /\ UNCHANGED output

Sync ==
    /\ pc = "written"
    /\ staging' = "synced"
    /\ pc' = "synced"
    /\ UNCHANGED output

Rename ==
    /\ pc = "synced"
    /\ output' = "new"
    /\ staging' = "none"
    /\ pc' = "done"

\* Any step may fail; the owner then removes the staging file, as publish_rules::cleanup decides.
Fail ==
    /\ pc \in {"start", "created", "written", "synced"}
    /\ staging' = "none"
    /\ pc' = "failed"
    /\ UNCHANGED output

\* The machine may lose power before the sync; the staging file then holds anything, and the output is untouched.
Crash ==
    /\ pc \in {"created", "written"}
    /\ pc' = "crashed"
    /\ UNCHANGED <<output, staging>>

\* The probe writes the output in place instead of staging it.
UnsafeWrite ==
    /\ ~Safe
    /\ pc = "start"
    /\ output' = "partial"
    /\ pc' = "created"
    /\ UNCHANGED staging

UnsafeFinish ==
    /\ ~Safe
    /\ pc = "created"
    /\ output' = "new"
    /\ pc' = "done"
    /\ UNCHANGED staging

UnsafeCrash ==
    /\ ~Safe
    /\ pc = "created"
    /\ pc' = "crashed"
    /\ UNCHANGED <<output, staging>>

\* A finished publication stays where it is.
Rest ==
    /\ pc \in {"done", "failed", "crashed"}
    /\ UNCHANGED vars

SafeNext == Create \/ Write \/ Sync \/ Rename \/ Fail \/ Crash \/ Rest
ProbeNext == UnsafeWrite \/ UnsafeFinish \/ UnsafeCrash \/ Fail \/ Rest

Next == IF Safe THEN SafeNext ELSE ProbeNext

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(Create \/ Write \/ Sync \/ Rename \/ UnsafeWrite \/ UnsafeFinish)

\* The path a reader opens never names a partial file.
OutputNeverPartial == output \in {"old", "new"}

\* A publication that reports success left the new file.
DoneMeansNew == pc = "done" => output = "new"

\* A publication that reports failure left the old file and no staging file.
FailedLeavesOld == pc = "failed" => output = "old" /\ staging = "none"

\* Every publication ends.
Terminates == <>(pc \in {"done", "failed", "crashed"})
====
