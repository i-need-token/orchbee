# Native terminal environment

## Scope and ownership

This reference governs the parts of the host-terminal contract that the native Client owns at
runtime: viewport observation against transient zero geometry, synchronized-update framing of
each drawn frame, and the pointer-capture mode pair. Maintain it with `terminal_device.rs`,
`terminal_lifecycle.rs`, `tui_backend.rs`, and `tui_shell.rs`. Admission verifies TTY identity
and descriptor safety (interactive TTY on both streams, one shared terminal identity, a working
geometry query); obtaining a drawable cell geometry is a separate readiness concern governed by
[`native-startup-wait.md`](native-startup-wait.md). Host-reported pixel geometry that arrives
as zero is reported honestly as "pixels unknown" — a normal state, never a fabricated one.

## Windows input ownership

The admitted inherited console owns one shared record reader across startup wait, the TUI and
direct attach. It retains UTF-16 surrogate and bounded repeat state through those phase changes;
there is no competing Crossterm event reader or background input thread. Each read yields at most
4096 bytes with a four-millisecond active-drain budget. Zero-virtual-key VT characters, including
C0 and bracketed-paste delimiters, reach the semantic decoder unchanged. Physical key and mouse
records reuse the existing byte encoding; raw Ctrl+C stays application input.

Raw mode keeps `ENABLE_VIRTUAL_TERMINAL_INPUT` enabled and disables cooked processing. Turning VT
input off lets ConPTY discard paste delimiters; interpreting its zero-key-code ESC through a
keyboard-layout lookup also loses the boundary. The pointer lease separately owns the native
mouse-input bit and its VT reporting writes, so raw admission with `--mouse off` does not capture
the mouse. Restoration reinstates the original saved console input/output modes. See Microsoft's
[console-mode contract](https://learn.microsoft.com/en-us/windows/console/setconsolemode) and
[paste delimiter filtering](https://github.com/microsoft/terminal/issues/18094).

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

## Pointer capture pair

Pointer reporting is one leased mode pair: button-event tracking (`?1002h`) together with SGR
encoding (`?1006h`), armed at startup unless `tui --mouse off` declined it, and released by the
matching `?1002l`/`?1006l`. The pair rides the same `ModeLease` machinery as bracketed paste —
a successful arm registers the restoration obligation, and a failed arm owes nothing. Runtime
release and re-arm through `Ctrl+B m` and startup arming share the identical lease, so the
settled client flag and the status indicator always describe a mode write that actually
succeeded. Gesture routing, hit testing, and passthrough semantics live in
[native mouse input](native-mouse-input.md); this section owns only the mode lifecycle.

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
the control channel at restoration.

Restoration is lifecycle-owned and runs in a fixed order: it first blocks further frame output,
then ends a still-open pair through the terminal control path inside the same bounded retry
budget as every other mode, and only then disables bracketed paste, disables pointer capture,
shows the cursor, leaves the alternate screen, and restores raw mode. The frame writer is restoration-gated: once restoration
begins, buffered frame bytes are discarded instead of reaching a terminal that is already being
restored, which also covers a panic-path unwind flush. After an output failure the same fence
is applied before the frame writer is dropped, so the writer's drop-time retry cannot emit
the abandoned frame either. A clean close owes nothing and emits nothing extra. This contract
promises neither a single operating-system write per frame nor visual atomicity on hosts that
do not support the synchronized-update mode.
