# process-wrap dependency correction

This directory retains the MIT/Apache-2.0 licensed `process-wrap` 10.0.1 registry
source (upstream commit `ca45003a831ac125e6673b8759430e5ef33cc1db`). Original licenses
and copyright notices remain beside the source. It is a third-party dependency,
excluded from the authored Cargo workspace and never published by Orchbee.

The native Client, Server, Windows launcher, and `orchbee-local-runtime` use its existing `std`,
`job-object` and `creation-flags` features. Windows Worker commands consume that shared
runtime. The root patch pins this corrected source without adding
a second Windows binding version. The manifest keeps the upstream dependencies and adds
`publish = false`; its `job-object` feature takes the `windows` bindings that
[thread resume](#thread-resume) needs in place of the Tool Help binding. Upstream test fixtures
and development artifacts are not copied. Unsafe FFI is the upstream implementation apart from
thread resume and the [completion](#completion-contract) fallback, where each call states the
conditions it relies on. The corrections do not change
the authored unsafe-code policy, which this crate stays outside.

## Completion contract

The upstream Windows completion-port reader accepts any Job notification as
completion. The correction waits for `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO` (code 4)
from the private Job port, propagates API errors, and respects the original timeout
across unrelated notifications. Once that timeout has passed, including a zero timeout, it
still reads the notifications already queued, up to a bound of 64, so one nonblocking poll
observes a completion that is already queued behind process start and exit notices. The standard child caches its exit status only after
that event. Its nonblocking poll reports `None` until root and Job exit both hold,
then returns the retained result without consuming another notification.

Windows does not guarantee delivery of Job notifications to the completion port, so a missing
`JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO` does not mean the Job still runs. Whenever the root has exited
and the port holds no completion, the standard child also settles once the Job's
`JOBOBJECT_BASIC_ACCOUNTING_INFORMATION` counts no active process and every member it terminated
has exited. A member that leaves by itself is counted out only after its exit, but
`TerminateJobObject` counts every member out at once: on the development machine (2026-11-22) the
count and the member list read zero 0.4 ms after the call while the root still ran, and the notice
came 3 ms later, after every member's exit. `start_kill` therefore lists the members first, opens a
synchronize handle to each one still in the Job, then terminates; when the Job's total process count
changed between that listing and the termination, a member may be untracked and only the notice
settles that child. The blocking wait reads the port in 50 ms slices and checks the same fact
between them, so a lost notice can no longer hold it forever.

The upstream `src/windows.rs` SHA-256 before correction is
`76214e10c89af6589af8fc9c2ae1d4c319596a07b2a2945afbc4a23b1d52a2b9`.
Only that file, `src/std.rs`, `src/std/job_object.rs` and the manifest's `job-object` feature
carry runtime changes. Other source files are retained unmodified; they are not newly admitted
runtime features.

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
assigned right after creation, with no thread to resume. The Windows launcher combines it with
`KillOnJobClose`: ending the launcher ends the Client it started, while the Server, Worker and
Terminal Host that Client starts keep running. Without the marker, the Job's limit flags and
suspension are unchanged.

The explicit `AllowBreakaway` marker opts one command into `JOB_OBJECT_LIMIT_BREAKAWAY_OK`, in
either registration order with `JobObject`: a process in the Job may create a child outside it by
asking for `CREATE_BREAKAWAY_FROM_JOB`, and every other child stays in the Job, so suspension is
unchanged. With `KillOnJobClose` it reproduces the Job Win32-OpenSSH runs each connection in, which
the `orchbee` process test of a remote Session start uses.

## Thread resume

A Job process without `SilentBreakaway` is created suspended, so that nothing it starts can leave
before it is assigned, and resumed only after the assignment. Upstream found its threads in a
snapshot of every thread on the system (`CreateToolhelp32Snapshot` with `TH32CS_SNAPTHREAD`),
which took 42 to 64 ms per start on the development machine with about 7,000 threads
(2026-11-20). The correction captures a snapshot of the new process alone (`PssCaptureSnapshot`
with `PSS_CAPTURE_THREADS`), opens each thread it lists, resumes a thread only while it still
reports that process's ID, and fails as before when no thread was suspended. A resume then takes
about 0.2 ms, and a Git `rev-parse` in a kill-on-close Job settled in 30 to 35 ms at the median
instead of 70 to 72 ms, level with the same start outside a Job.

Rust 1.97.0 offers neither alternative on stable: the main thread handle that process creation
returns is exposed only through `ChildExt::main_thread_handle`, and creating the process inside
its Job (`PROC_THREAD_ATTRIBUTE_JOB_LIST`) needs `ProcThreadAttributeList`; both are unstable. So
creation and assignment stay two calls, and a parent that ends between them, a window of
microseconds, leaves the new process suspended outside any Job until it is ended by its process ID
or Windows restarts; meanwhile it holds its program file open, so removing that version reports a
running program. A Server ended right after it created its Worker did this once in a fault-injection
rehearsal (2026-11-22). The
`Win32_System_Diagnostics_Debug` and `Win32_System_Kernel` bindings only complete the
`PSS_THREAD_ENTRY` type.

## Maintenance and evidence

The shared runtime owns native completion evidence in
[`command_child/windows/tests.rs`](../../crates/local-runtime/src/command_child/windows/tests.rs):
retain a live descendant's handle, release the root or cancel the Job, and require
the descendant handle already signaled at return. A separate native
oracle retains root and descendant process handles, verifies both are alive, drops a
`KillOnJobClose` wrapper without calling `start_kill`, and requires both handles to
signal within its bounded wait; its command starts suspended, so it also covers thread
resume. Worker discovery tests cover output bounds, cancellation and failure cleanup. Native Client integration tests cover its existing
provider process adapter. Run affected tests and the applicable root host gate in
[Verification](../../docs/VERIFICATION.md) before changing this patch.

Rebase against an explicitly reviewed upstream release; compare the complete source
diff, keep licenses, and rerun the native handle oracle. Remove this directory and
root patch when an upstream release provides equivalent completion semantics and
thread resume and passes those tests. Root-process exit, successful termination
initiation, an empty member list and a zero active count after a termination are not
substitutes for the completion event; the fallback above combines the count with each
terminated member's own exit.
