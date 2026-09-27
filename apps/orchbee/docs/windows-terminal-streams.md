# Windows terminal streams

## Scope and ownership

`terminal session observe <terminal>` emits NDJSON from one existing Windows terminal binding.
It never activates a Site, starts a terminal, reads stdin, acquires control, or changes Host geometry.
`terminal session control <terminal>` adds exclusive input, resize and history commands.
Interactive attach remains a dependent surface. Both commands select existing terminal IDs/selectors.

## Observation model

| State           | Action and next state                                                               | Authority and invariant                                                  |
| --------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| Parsed          | Validate all operands before effects                                                | CLI owns grammar; default viewport is 120×40                             |
| Admitting       | Discover existing Site, read Snapshot, attach exact binding                         | One five-second budget; no fallback target                               |
| Observing       | Poll one retained Viewer, validate frame, emit and flush record, then acknowledge   | TerminalProjection owns sequence/base/epoch checks; no ACK before output |
| Resynchronizing | Emit reset, request snapshot, resume observation                                    | Never present a missing frame interval as continuous                     |
| Ending          | Detach once, drop connection, emit one terminal record when output remains writable | Two-second cleanup reserve; never stop the underlying terminal           |
| Finished        | Stop and join relay, release handles and interrupt owner                            | No retained task, thread, pipe or temporary file                         |

No total timeout is the default. `--timeout <ms>` sets a positive total bound up to 3600000 ms.
Individual exchanges and output flushes have a five-second ceiling. Cancellation exits 130;
timeout exits 7, unavailable 4, transport/protocol loss 5, and local/output failure 1.
Missing, ambiguous and refused targets retain the ordinary CLI refusal classes. A transport EOF
never proves successful terminal exit. A failed output may leave a truncated last line; no second
record is appended after partial output, and no successful-stream claim is made.

`--cols` and `--rows` select the observer viewport without resizing the terminal. Full ANSI frames
are synthesized from validated styled cells, clipped/padded to that viewport. Each record reports
source dimensions separately. The stream is a screen projection, not raw PTY bytes or graphics.
Every JSON record names `orchbee.terminal.stream` version 2 and its kind. Frame bytes use base64;
sequence/generation/revision identities are decimal strings. Both format selections emit NDJSON.

Windows stdout is mediated by an invocation-owned, hidden system PowerShell process running a
fixed script with profiles disabled. One bounded record is in flight; the child acknowledges only
after writing and flushing it. The parent polls nonblocking pipes with cancellation/deadlines.
No payload is interpolated into a command. Child stdin uses a synchronous pipe handle; merely
switching an overlapped IPC handle to blocking mode is insufficient for standard child readers. On stalled output or any exit, the owner stops and joins
the child. This cannot retract bytes already delivered to the downstream reader.

## Control model

Control attaches the exact Viewer, acquires its exclusive controller (or transfers an active grant
with `--takeover`), resizes to `--cols`/`--rows` (default 120×40), then emits `opened` with
`mode: control` and `controllerId`. Takeover follows the Server's
[controller transfer contract](../../orchbee-server/docs/terminal-controller-takeover.md).
A failed admission or resize still releases the owned Viewer; it never retargets or retries input.

Stdin is UTF-8 NDJSON governed by `orchbee.terminal.control.input`. A separate invocation-owned
hidden PowerShell byte reader uses a fixed script, nonblocking parent pipe and bounded buffer.
The owner kills and waits for both reader and output relay on every exit. Blank lines are ignored;
a first-line BOM, CRLF and a final unterminated line are accepted. One line is limited to 2 MiB;
a framing overflow terminates the invocation. Other invalid records emit a typed rejection and
allow the next record. Unknown/duplicate fields reject before any command effect.

| Input type         | Fields and effect                                                                                                                                            |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `terminal.input`   | Exactly one of `text` or base64 `bytes`, at most 256 KiB decoded; restore live demand, submit ordered 4096-byte chunks without replay                        |
| `terminal.resize`  | `cols`, `rows`, optional paired `cell_width_px`/`cell_height_px`; acknowledge valid Host geometry before updating local viewport                             |
| `terminal.scroll`  | `direction: up/down`, positive u16 `lines`, optional `source: wheel/page_key`, zero-based `column`/`row` (each defaults to zero), crossterm `modifiers` bits |
| `terminal.release` | Finish this stream, detach the exact Viewer and release its grant; leave the terminal running                                                                |

Geometry uses the existing 4096-per-axis, 262144-cell and 65535-pixel-product bounds; pixels must
both be zero or both positive. Text bounds count UTF-8 bytes. The input schema documents the
shape; the parser additionally enforces byte/product bounds. Modifier flags are Shift=1,
Control=2, Alt=4, Super=8, Hyper=16, Meta=32; only defined bits are accepted.

The serialized loop polls the Viewer and advances one history step between input records.
Input receipts identify the record ordinal and acknowledged byte prefix; a failed later chunk
never replays the prefix and does not claim the remaining delivery succeeded. A receipt is
published only after the corresponding operation acknowledgement. Cancellation or output loss
can prevent a receipt even when an input effect already occurred.

History uses the same retained viewport state machine as the native TUI: set history demand,
pause live projection, validate exact Host rows and facts, flush the active surface, then send
RowsApplied. Typing restores following-bottom/active demand before input. Page keys use history
only on an untracked normal screen with application-cursor mode off or bracketed paste on;
otherwise they send PageUp/PageDown. Scroll rejects with `frame-resizing` until the current frame
matches the acknowledged dimensions. Mouse tracking sends one wheel report, independent of
`lines`, with the supplied modifiers. Alternate-screen fallback sends one cursor arrow; the
current frame contract does not expose DEC 1007, so disabling alternate-scroll remains a parity
obligation. Pixel mouse reports remain explicitly refused until their coordinate capability is
available. History output is safe text ANSI; source styling and graphics are not reconstructed.

Every stream output record uses version 2. `control` reports command receipts; `history` reports
a retained row surface; ordinary `frame` and `reset` retain their projection meanings. While
history is active, a frame acknowledgement follows output of the active history surface or a
pending reset, not display of a hidden live frame. EOF and release are successful only after
confirmed detach. Unconfirmed cleanup becomes outcome-unknown. Ctrl+C and total timeout retain
the observation exit classes and bounded cleanup reserve.

## Verification and limits

Deterministic core and scripted Windows transport checks cover exact binding, repeated frames,
resets, partial output, cancellation, expiry and cleanup. Pipe tests must prove a blocked consumer
cannot strand the relay. Same-author tests do not establish independent review, TLA execution,
remote/named Site routing, other platforms, graphics streams or the later interactive attach slice. Control evidence must
cover ordered input, partial acceptance, invalid-record continuation, resize, release/EOF, history
feedback ordering and cancellation of an in-flight controller exchange.

Cleanup reports `not-attached` before any Viewer attach attempt, `detached` only after its
acknowledgement, or `session-released` when the socket is dropped without confirmed detach.
Scripted evidence does not replace final acceptance against the packaged Windows runtime.
