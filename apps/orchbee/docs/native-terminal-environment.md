# Native terminal environment

## Scope and ownership

This reference governs the parts of the host-terminal contract that the native Client owns at
runtime: viewport observation against transient zero geometry, and synchronized-update framing of
each drawn frame. Maintain it with `terminal_device.rs`, `terminal_lifecycle.rs`,
`tui_backend.rs`, and `tui_shell.rs`. Admission-time requirements (interactive TTY on both
streams, one shared terminal identity, nonzero initial cell geometry) stay unchanged and are not
restated here. Host-reported pixel geometry that arrives as zero is reported honestly as
"pixels unknown" — a normal state, never a fabricated one.

## Viewport observation

The runtime observation of the inherited terminal distinguishes three outcomes. The distinction
is made on typed results, never on error text.

| Observation      | Meaning                                                                   | Runtime effect                                                                                     |
| ---------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Valid geometry   | The host reported nonzero rows and columns                                | A change applies a resize and a redraw; an equal geometry is a no-op                               |
| Zero or absent   | The host transiently reported zero rows or columns                        | No resize is sent, the Session is not retired, the last valid geometry stays in effect, and the next valid geometry resumes normally |
| Inspection error | The descriptor or the query itself failed (permission, closed, invalid fd) | Fatal: the failure propagates and the interface exits                                              |

A pixel extent of zero is a normal observation, not an error: the cell-pixel pair is reported as
`(0, 0)` — "pixels unknown" — while the character grid keeps working. When the host reports only
one pixel axis, or a derived cell-pixel axis rounds down to zero, the whole pair is reported
unknown rather than fabricating the missing axis. A cell-size change whose pixel extent reads
zero is likewise never resolved by reusing the previous pixel values — old pixels are never a
measure of a new size. Known → unknown → known transitions ride the same geometry path; every
state is reported as measured, and downstream pixel-dependent features gate on the pair being
known rather than on any estimate.

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
budget as every other mode, and only then disables bracketed paste, shows the cursor, leaves the
alternate screen, and restores raw mode. The frame writer is restoration-gated: once restoration
begins, buffered frame bytes are discarded instead of reaching a terminal that is already being
restored, which also covers a panic-path unwind flush. After an output failure the same fence
is applied before the frame writer is dropped, so the writer's drop-time retry cannot emit
the abandoned frame either. A clean close owes nothing and emits nothing extra. This contract
promises neither a single operating-system write per frame nor visual atomicity on hosts that
do not support the synchronized-update mode.
