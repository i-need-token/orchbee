# process-wrap dependency correction

This directory retains the MIT/Apache-2.0 licensed `process-wrap` 10.0.1 registry
source (upstream commit `ca45003a831ac125e6673b8759430e5ef33cc1db`). Original licenses
and copyright notices remain beside the source. It is a third-party dependency,
excluded from the authored Cargo workspace and never published by Orchbee.

The native Client, Server, Windows launcher, and `orchbee-local-runtime` use its existing `std`,
`job-object` and `creation-flags` features. Windows Worker commands consume that shared
runtime. The root patch pins this corrected source without adding
a second Windows binding version. The manifest keeps the upstream dependency and
feature graph and adds `publish = false`; upstream test fixtures and development
artifacts are not copied. Unsafe FFI remains the upstream implementation. Orchbee's
corrections introduce no unsafe block or change to the authored unsafe-code policy.

## Completion contract

The upstream Windows completion-port reader accepts any Job notification as
completion. The correction waits for `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO` (code 4)
from the private Job port, propagates API errors, and respects the original timeout
across unrelated notifications. Once that timeout has passed, including a zero timeout, it
still reads the notifications already queued, up to a bound of 64, so one nonblocking poll
observes a completion that is already queued behind process start and exit notices. The standard child caches its exit status only after
that event. Its nonblocking poll reports `None` until root and Job exit both hold,
then returns the retained result without consuming another notification.

The upstream `src/windows.rs` SHA-256 before correction is
`76214e10c89af6589af8fc9c2ae1d4c319596a07b2a2945afbc4a23b1d52a2b9`.
Only that file, `src/std.rs` and `src/std/job_object.rs` carry runtime changes. Other
source files are retained unmodified; they are not newly admitted runtime features.

The standard wrapper keeps upstream-compatible default behavior: `JobObject` alone
does not enable `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The explicit
`KillOnJobClose` marker opts one command into that policy, in either registration
order with `JobObject`. The Worker's Agent probe uses this opt-in so dropping its
final owned wrapper is a process-tree termination guard after normal termination
cannot be confirmed.

The explicit `SilentBreakaway` marker opts one command into
`JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK`, in either registration order with `JobObject`, so the
Job holds only the spawned process and the processes it starts are created outside it. Since no
descendant stays in the Job, the process is not created suspended: it starts at once and is
assigned right after creation, which spares the system-wide thread snapshot that resuming a
suspended process takes (about 36 ms per start on the development machine, 2026-11-12). The
Windows launcher combines it with `KillOnJobClose`: ending the launcher ends the Client it
started, while the Server, Worker and Terminal Host that Client starts keep running. Without the
marker, the Job's limit flags and suspension are unchanged.

## Maintenance and evidence

The shared runtime owns native completion evidence in
[`command_child/windows/tests.rs`](../../crates/local-runtime/src/command_child/windows/tests.rs):
retain a live descendant's handle, release the root or cancel the Job, and require
the descendant handle already signaled at return. Its failure fixture also proves that
a failed Job wait cannot become cached root-exit success on retry. A separate native
oracle retains root and descendant process handles, verifies both are alive, drops a
`KillOnJobClose` wrapper without calling `start_kill`, and requires both handles to
signal within its bounded wait. Worker discovery tests cover output bounds,
cancellation and failure cleanup. Native Client integration tests cover its existing
provider process adapter. Run affected tests and the applicable root host gate in
[Verification](../../docs/VERIFICATION.md) before changing this patch.

Rebase against an explicitly reviewed upstream release; compare the complete source
diff, keep licenses, and rerun the native handle oracle. Remove this directory and
root patch when an upstream release provides equivalent completion semantics and
passes those tests. Root-process exit, successful termination initiation and an
empty member list are not substitutes for the completion event.
