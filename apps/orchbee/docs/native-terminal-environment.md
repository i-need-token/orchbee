# Native terminal environment

## Scope and ownership

This reference governs the parts of the host-terminal contract that the native Client owns at
runtime: retained Windows endpoints, raw-record input and Unicode VT output, viewport observation
against transient zero geometry, synchronized-update framing of each drawn frame, and the
pointer-capture mode pair. Maintain it with `terminal_device.rs`, `terminal_device/windows.rs`,
`windows_console_input/records.rs`, `startup_wait.rs`, `terminal_lifecycle.rs`, `tui_backend.rs`,
`tui_shell.rs`, and the owning `crates/windows-console-host` boundary. Admission verifies TTY identity
and descriptor safety (interactive TTY on both streams, one shared terminal identity, a working
geometry query); obtaining a drawable cell geometry is a separate readiness concern governed by
[`native-startup-wait.md`](native-startup-wait.md). Host-reported pixel geometry that arrives
as zero is reported honestly as "pixels unknown" — a normal state, never a fabricated one.
Maintain pointer capability settlement with `terminal_pointer_admission.rs` and the outer-host
decoders under `terminal_input/` as well.

## Windows input ownership

The Windows adapter duplicates the admitted standard input and output handles once into the
`OwnedHandle`-backed `ConsoleInput` and `ConsoleOutput` ports supplied by
[`orchbee-windows-console-host`](../../../crates/windows-console-host/README.md). Mode operations,
raw record intake, output-window observation and Unicode output all use those exact retained
handles; the adapter does not reopen `CONIN$` or `CONOUT$`, nor resolve a process-standard endpoint
again after admission. The boundary crate's private `sys` module is the reviewed unsafe boundary
for these Win32 calls and tagged-union reads. The Client consumes its safe records and ports.

The admitted inherited console owns one canonical record reader across startup wait, the TUI and
direct attach. It retains UTF-16 surrogate and bounded repeat state through those phase changes;
there is no competing Crossterm event reader or background input thread. Each read yields at most
4096 bytes with a four-millisecond active-drain budget. Zero-virtual-key VT characters, including
C0 and bracketed-paste delimiters, reach the semantic decoder unchanged. Physical key and mouse
records reuse the existing byte encoding; raw Ctrl+C stays application input. The native readiness
count and following one-record read are separate Win32 calls, not an atomic reservation against an
external consumer. The bounded-read contract therefore requires that no outside reader drain the
same console input buffer between them; the canonical Client reader enforces that condition inside
the process.

Raw mode keeps `ENABLE_VIRTUAL_TERMINAL_INPUT` enabled and disables cooked processing. Turning VT
input off lets ConPTY discard paste delimiters; interpreting its zero-key-code ESC through a
keyboard-layout lookup also loses the boundary. The pointer lease separately owns the native
mouse-input bit and its VT reporting writes, so raw admission with `--mouse off` does not capture
the mouse. Restoration reinstates the original saved console input/output modes. See Microsoft's
[console-mode contract](https://learn.microsoft.com/en-us/windows/console/setconsolemode) and
[paste delimiter filtering](https://github.com/microsoft/terminal/issues/18094).

## VT command output

After Windows has successfully enabled VT output processing, the lifecycle, startup-wait prompt,
and fixed-viewport renderer encode Crossterm commands with `Command::write_ansi` directly into the
writer supplied by their caller. This path does not run Crossterm's process-global Windows ANSI
probe, open `CONOUT$`, or fall back to a WinAPI operation against a different console. The encoder
allocates no intermediate command buffer, retains the first writer error, and never chooses a
flush boundary: lifecycle effects flush after their command, while the prompt and renderer flush
only at their existing complete-output boundary. A reported write error may follow an emitted
prefix; the encoder never replays that prefix or advances to later command fragments.

The exact-handle Unicode adapter uses `WriteConsoleW`. Each acquired writer owns one ordered
stream's incomplete UTF-8 tail, pending low surrogate and latched fault. A trusted short native
count is mapped back to a UTF-8 prefix; a split surrogate is completed exactly once before that
same writer accepts later bytes. A zero or impossible count, invalid UTF-8, or native error faults
that writer and causes later calls on it to return the retained error without another native
write. A native error has unknown effect and can follow an emitted prefix, so neither the adapter
nor its caller replays the failed payload. A fresh writer starts with empty encoding state, which
lets bounded lifecycle cleanup attempt complete ASCII control sequences after the frame writer has
faulted. It does not repair the failed stream, prove that no prefix escaped, or make console output
atomic.

Direct encoding is gated by the successful native VT-mode mutation. If that mutation fails before
the alternate-screen command is attempted, restoration emits no ANSI alternate-screen cleanup
that could appear as literal text. On an admitted path, cursor-show and alternate-screen leave are
attempted while VT processing is still active; restoring the saved output and input mode words is
the final native step. The two native restorations settle independently: both pending words are
attempted, and a word whose restoration succeeded is not written again merely because the other
word failed. Mode capture, VT admission, output, cleanup and restoration all address the same
retained input/output ports; the lifecycle owner, rather than the boundary crate, records and
settles the original-mode restoration debt.

## Viewport observation

The runtime observation of the inherited terminal distinguishes three outcomes. The distinction
is made on typed results, never on error text.

| Observation      | Meaning                                                                    | Runtime effect                                                                                                                       |
| ---------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Valid geometry   | The host reported nonzero rows and columns                                 | A change applies a resize and a redraw; an equal geometry is a no-op                                                                 |
| Zero or absent   | The host transiently reported zero rows or columns                         | No resize is sent, the Session is not retired, the last valid geometry stays in effect, and the next valid geometry resumes normally |
| Inspection error | The descriptor or the query itself failed (permission, closed, invalid fd) | Fatal: the failure propagates and the interface exits                                                                                |

A pixel extent of zero is a normal observation, not an error: the cell-pixel pair is reported as
`(0, 0)` — "pixels unknown" — while the character grid keeps working. When the host reports only
one pixel axis, or a derived cell-pixel axis rounds down to zero, the whole pair is reported
unknown rather than fabricating the missing axis. A cell-size change whose pixel extent reads
zero is likewise never resolved by reusing the previous pixel values — old pixels are never a
measure of a new size. Known → unknown → known transitions ride the same geometry path; every
state is reported as measured, and downstream pixel-dependent features gate on the pair being
known rather than on any estimate.

## Pointer capture lifecycle

Pointer reporting owns one tracking level plus SGR cell encoding. Admission first establishes the
button-event baseline by disabling any-motion (`?1003l`) and enabling `?1002h` with `?1006h`.
After that complete baseline, the lifecycle-controlled output path issues one ordered `?1003$p`
DECRQM followed by DA1 as a completion sentinel. Only a `Reset (2)` DECRPM reply admits any-motion. A
`Set (1)` reply contradicts the established baseline and settles unknown; `0`, `3`, and `4` settle
unavailable; DA1 arriving first or the bounded deadline expiring settles unknown. Because the
reply has no query identifier, an endpoint is never probed again: late or duplicate replies are
consumed but cannot change its settled result.

The ordinary and modal host decoders recognize the same exact replies in both ESC-CSI and C1-CSI
form. They accept only `?1003;[0-4]$y` as DECRPM and a DA1 response made from one or more nonempty
decimal parameters. A complete private CSI is host metadata only when it has the DA1 `c` or DECRPM
`$y` reply final; malformed reply-shaped frames are consumed without becoming keys, editor input,
or pane bytes. Other complete private CSI remains generic non-decision input and preserves raw
passthrough. A run that reaches its time or size bound before a reply final likewise returns to the
bounded generic drain instead of extending metadata ownership indefinitely. Bracketed-paste
payloads stay literal and are never inspected for these replies.

An admitted result switches from `1002` to `1003` by disabling the old level before enabling the
new one. Unknown and unavailable results retain `1002`, so click, drag, and wheel work while idle
hover remains honestly absent. Exactly one tracking level is active; `1006` remains its encoding.
Capture release and re-arm reuse the endpoint's settled result rather than issuing another query.
An admitted switch is published to pointer interaction state only after the complete mode write
succeeds. A failed switch publishes no hover authority and is never retried by the running input
loop; the fatal path proceeds to full lifecycle cleanup. Global admitted any-motion serves every
painted chrome control, including action menus. There is no menu-scoped `1003` lease; ButtonOnly
fallback therefore has click, drag, and wheel but no claimed idle hover anywhere.

The tracking level and encoding share one `ModeLease`, armed at startup unless `tui --mouse off`
declined it. A pending restoration obligation is registered before every native or VT mutation.
A partial enable or level switch therefore enters transition-unknown state and cannot attempt a
runtime fallback; only the complete release sequence (`?1003l`, `?1006l`, `?1002l`) settles it.
Runtime release through `Ctrl+B m`, re-arm, startup arming, error cleanup, and panic cleanup use
that same lease. Gesture routing, hit testing, state clearing on downgrade or release, and pane
passthrough live in [native mouse input](native-mouse-input.md); this section owns capability
settlement and the mode lifecycle.

## Synchronized frame output

Each drawn frame — the complete cell diff plus its cursor placement — is wrapped in exactly one
synchronized-update pair and published by one flush. `terminal_lifecycle` is the single owner of
the pair: it defines the begin and end sequences, tracks the open-pair obligation on a shared
lease, and owns recovery. The backend participates through that lease only: it registers the
obligation before the begin marker is attempted, so a partially written begin still owes an end,
and it clears the obligation only after both the end marker and the frame flush have been
accepted by the writer. Whenever delivery cannot be confirmed, the pair stays pending.

| State                 | Effect                                                                                             |
| --------------------- | -------------------------------------------------------------------------------------------------- |
| Update closed         | A draw operation registers the obligation, emits the begin marker, and moves to update open        |
| Update open           | Cell, attribute, and cursor writes accumulate inside the pair                                      |
| Update close          | A flush emits the end marker and flushes the writer once; both must succeed to confirm the closure |
| Update open + failure | The first failure is retained; a later flush still attempts the end marker                         |
| Begin write failure   | The obligation stays registered; the next flush still attempts the end marker                      |
| Unconfirmed end       | The pair stays pending; the next successful flush re-emits the end marker, or restoration ends it  |

Failure keeps its first cause. A flush failure still attempts the end marker; an end marker
that itself fails is reported after the attempt, never silently retried inside one frame. A
draw failure abandons the incomplete frame instead: on the error path the frame writer is
fenced before it is dropped, so neither its buffered residue nor its drop-time retry reaches
the terminal. Bytes already written cannot be retracted; the still-open pair is ended through
the control channel at restoration. The shell also invalidates the prior frame's hit map and
armed controls before returning the fatal draw error. Neither the incomplete candidate nor
the old frame authorizes another interaction. This loop exits without replay; a recovery
implementation would have to establish a new terminal baseline and complete a full redraw.

Restoration is lifecycle-owned and runs in a fixed order: it first blocks further frame output,
then ends a still-open pair through the terminal control path inside the same bounded retry
budget as every other mode, and only then disables bracketed paste, disables pointer capture,
shows the cursor, leaves the alternate screen, and restores raw mode. On Windows, VT output stays
enabled through cursor and alternate-screen cleanup, then the raw restore reinstates both saved
native mode words. The frame writer is restoration-gated: once restoration begins, buffered frame
bytes are discarded instead of reaching a terminal that is already being restored, which also
covers a panic-path unwind flush. After an output failure the same fence is applied before the
frame writer is dropped, so the writer's drop-time retry cannot emit the abandoned frame either.
A clean close owes nothing and emits nothing extra. This contract promises neither a single
operating-system write per frame nor visual atomicity on hosts that do not support the
synchronized-update mode.

## Outer title restoration

[Windows title publication](windows-terminal-title.md) has its own lifecycle lease. The first title write captures the inherited title; restoration repays this lease after ending synchronized output and before disabling bracketed paste. Capture or write failure reports a localized status notice and keeps the TUI usable; any already-owed restoration remains registered. No title writes occur after restoration starts. Before an effective configured or explicit title is delivered, no title capture or write occurs.
