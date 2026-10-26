# Windows terminal streams

## Scope and ownership

`terminal session observe <terminal>` emits NDJSON from one existing Windows terminal binding.
It never activates a Session, starts a terminal, reads stdin, acquires control, or changes Host geometry.
`terminal session control <terminal>` adds exclusive input, resize and history commands.
Interactive attach remains a dependent surface. Both commands select existing terminal IDs/selectors.

## Observation model

| State           | Action and next state                                                               | Authority and invariant                                                  |
| --------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| Parsed          | Validate all operands before effects                                                | CLI owns grammar; default viewport is 120×40                             |
| Admitting       | Discover existing Session, read Snapshot, attach exact binding                      | One five-second budget; no fallback target                               |
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
Every JSON record names `orchbee.terminal.stream` version 1 and its kind. Frame bytes use base64;
sequence/generation/revision identities are decimal strings. Both format selections emit NDJSON.

Windows stdout is mediated by an invocation-owned system PowerShell process running a fixed
script with profiles disabled. Redirected output uses a hidden child and preserves exact bytes.
Native console output retains the existing console in a separate Ctrl+C process group and decodes
UTF-8 records for Unicode console writes; it does not create a window or alter console modes. One bounded record is in flight; the child acknowledges only
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
`lines`, with the supplied modifiers. On an untracked alternate screen, DEC 1007 enabled in
the latest validated frame sends one cursor arrow; disabled routes to retained history without
sending application input. Mode changes replace the fact with each frame. Cell-only scroll and
attach pointer events use SGR cell fallback when the child requests pixel format; they preserve
cell coordinates, modifiers and release identity without requiring host pixel geometry. Exact
pixel input remains separate work. History requests render-detail rows and synthesizes bounded
ANSI from validated cells, preserving captured colors, style attributes and wide-cell geometry.
It shares the live cell painter and never replays raw PTY output. Graphics resources are strictly
validated but are not displayed; graphics history remains a separate obligation.

Every stream output record uses version 1. `control` reports command receipts; `history` reports
a retained row surface; ordinary `frame` and `reset` retain their projection meanings. While
history is active, a frame acknowledgement follows output of the active history surface or a
pending reset, not display of a hidden live frame. EOF and release are successful only after
confirmed detach. Unconfirmed cleanup becomes outcome-unknown. Ctrl+C and total timeout retain
the observation exit classes and bounded cleanup reserve.

## Interactive attach

`terminal attach <terminal>` and `agent attach <target>` are Windows terminal interfaces.
They accept `--takeover`, `--timeout <ms>` and `--mouse on|off` (default on); Agent selectors also
accept `--agent-session`, `--name` or `--panel`. Grammar validation precedes admission of inherited
interactive stdin/stdout. Neither interface starts a Session, changes Workbench focus or falls back
to another terminal. Agent resolution selects one live binding under the original cancellable
admission budget. Output remains an interactive screen even with a global format selection.

| Phase            | Owned action and release rule                                                                                 |
| ---------------- | ------------------------------------------------------------------------------------------------------------- |
| TTY admitted     | Read inherited console geometry without changing modes                                                        |
| Binding selected | One Snapshot and exact Viewer; Agent resolution is read-only                                                  |
| Granted          | Acquire or explicitly take over the controller; acknowledge initial resize                                    |
| Mode owned       | Existing TerminalModeGuard owns raw mode, alternate screen, cursor, paste, focus and optional pointer capture |
| Attached         | Share the control Session and history core; flush the displayed surface before frame/row acknowledgement      |
| Ending           | Detach the exact Viewer once; restore console modes before diagnostics, including errors and unwinding        |

`Ctrl+B`, then `q`, detaches without stopping the terminal. Pressing `Ctrl+B` twice sends one
literal prefix. Other followers preserve the held prefix and their own order. A paste is one
semantic operation and never participates in detach matching. Invalid, oversized or timed-out
pastes are rejected without submitting fragments; draining retains paste ownership through the
terminator. Mouse reports with `--mouse off` do not consume a held prefix. Raw-mode `Ctrl+C` reaches the
application; an external console interrupt cancels the invocation. Plain PageUp/PageDown and
wheel input share control's history/application routing; other mouse events use the validated
pane tracking contract. Unsupported input produces a bounded localized notice. Resizes use the
observed console geometry; transient zero geometry keeps the last valid size without resizing.
An old-size frame cannot authorize new pointer/history coordinates.

The in-process display seam shares frame/history rendering with NDJSON without encoding and
then parsing that protocol. Console writes use the existing inherited Unicode TUI writer; this
is not a hard bound on an unresponsive operating-system console. IPC and cleanup keep their
bounded budgets. Explicit detach succeeds only after confirmed release and mode restoration;
transport loss never fabricates normal terminal exit. Pixel/graphics and extended
keyboard-mode residuals remain obligations until their own implementation and Windows evidence.

## Verification and limits

Deterministic core and scripted Windows transport checks cover exact binding, repeated frames,
resets, partial output, cancellation, expiry and cleanup. Pipe tests must prove a blocked consumer
cannot strand the relay. Same-author tests do not establish independent review, TLA execution,
remote/named Session routing, other platforms or graphics streams. Control evidence must
cover ordered input, partial acceptance, invalid-record continuation, resize, release/EOF, history
feedback ordering and cancellation of an in-flight controller exchange.

Cleanup reports `not-attached` before any Viewer attach attempt, `detached` only after its
acknowledgement, or `session-released` when the socket is dropped without confirmed detach.
Packaged Windows console witnesses cover bracketed Unicode paste containing the detach prefix,
raw Ctrl+C delivery, mouse-off entry, busy-controller refusal, takeover fencing, history paging,
normal detach, timeout restoration, re-acquisition and admission at different console sizes.
They also exercise shared input in the Workspace TUI. A real Windows child requesting `?1016`
receives exact SGR cell bytes for control wheel/modifiers and attach press/drag/release injected
through the console; this proves cell fallback, not exact pixel capture or physical mouse visuals.
Packaged TUI and attach witnesses retain history RGB colors, bold/italic/underline and wide
characters; attach also preserves decomposed combining text. TUI history chrome clears underlying
styles and footer text. Typing restores live output and delivers each input once, and normal
detachment leaves the child running. These witnesses do not establish complete native TUI glyph
fidelity or physical visual acceptance. Dynamic host-window resizing, visual mouse
interaction and authenticated Agent attachment remain separate acceptance obligations.
Stream timeouts use a stream-specific diagnostic; refusals retain the controller reason and do
not imply that previously delivered input was undone. Scripted evidence does not replace final
acceptance against the packaged Windows runtime.

Direct attach owns a fixed Ctrl+B/q escape independently of main-TUI key profiles. Extended-key, graphics, pixel-input and Windows visual acceptance remain separate verification obligations.
