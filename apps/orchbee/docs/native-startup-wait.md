# Native Startup Wait

Status: accepted implementation and correctness contract for the bounded too-small startup
wait in the native `orchbee` command: when the inherited terminal reports a geometry that is
not drawable, startup waits for a real drawable geometry instead of failing admission or
inventing one. The wait runs entirely inside the original startup deadline measured from
`run` entry; it never extends, resets, or replaces that deadline, and it performs no cohort,
Session, server, session, or terminal work while waiting.

Authority: this record owns the waiting model, the outcome contract, the deadline-precedence
rule, the injected-dependency surface, and the input/lifecycle discipline implemented under
`src/startup_wait.rs` and wired in `src/local_startup.rs`. Where prose and code disagree, the
behavior is a bug to fix, not a fork to keep.

## Model

TTY admission verifies terminal identity and descriptor safety and probes that the geometry
query itself works; it does not classify cell drawability. Startup samples the inherited
terminal's reported dimensions once after admission.

- A drawable report proceeds directly to the existing startup path with no mode transition,
  no extra draw, and no added flicker — the fast path is unchanged.
- A report that is not drawable, including a temporary zero-cell report, enters the wait.
  A zero-cell report is retryable, not a fault; real I/O faults still fail immediately.
- The wait enters terminal modes once (raw mode, alternate screen, cursor hidden, panic
  restoration) through the existing `TerminalModeGuard` owner, draws the localized waiting
  notice and exit hint, and polls input and geometry until one outcome resolves.

Outcomes:

- `Available(geometry)`: a real drawable geometry was observed. Modes are restored before
  the wait returns; the normal startup path then computes wire geometry and proceeds to
  cohort, Session, connection, and session work exactly as before.
- `Cancelled`: the user issued the existing prefix-quit binding (`Ctrl+b q` or `Ctrl+b d`). The
  process returns success; no cohort, Session, connection, session, or service is created.
- `TimedOut`: the original startup deadline expired. The localized `TimedOut` startup
  failure is returned; modes are restored first.
- `Fault`: an I/O or mode-owner failure. The mapped startup failure is returned after mode
  restoration is attempted.

The deadline is checked before cancel and before resize resolution at every decision point,
so a resize and expiry at the same instant resolve to `TimedOut`.

## Boundaries

- No geometry is invented. The wait emits no `1×1` or other synthetic dimensions; wire
  geometry is always derived from a real drawable report.
- Presentability is pure layout math (`startup_presentable_area`), the same seam the
  shared layout projection exposes for the native shell; it is not a width-only check.
- Input consumed during the wait is not replayed. Exit-key polling reuses the production
  `InputDispatcher` against the unavailable terminal-input port, so bytes consumed while
  waiting cannot reach the later terminal session.
- The wait uses the inherited TTY only: it duplicates the inherited input/output handles and
  never reopens `/dev/tty`. Mode ownership stays with the existing lifecycle guard.
- The wait never creates cohort, Session, server, session, or terminal objects; those start
  only after `Available` returns, through the unchanged startup sequence.

## Testing contract

Unit coverage is fully deterministic: every dependency (viewport sampler, input, output,
mode-owner factory, clock, deadline, prompt, presentability) is injected, so waiting,
cancel, timeout, availability, fault, deadline precedence, zero-report retry, mode
restoration ordering, and non-replay are exercised without a terminal.

The focused PTY journey covers the real path: a too-small or initially zero-cell spawn shows
the localized notice; exiting during the wait returns success with terminal modes restored and
creates no Session processes; resizing to a drawable size continues to the full Ready workbench —
verified on the frame's real last row — and exits cleanly. A drawable spawn that reports zero
pixels still reaches Ready, covering the legal pixels-unknown state. The PTY observer is a
fixed-capacity canvas independent of the initial winsize, and witness resizes beyond that
capacity are refused.
