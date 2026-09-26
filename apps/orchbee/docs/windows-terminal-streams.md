# Windows terminal streams

## Scope and ownership

`terminal session observe <terminal>` emits NDJSON from one existing Windows terminal binding.
It never activates a Site, starts a terminal, reads stdin, acquires control, or changes Host geometry.
The next dependent surfaces are machine control and interactive attach; this observation contract
alone does not establish their delivery. Binding selection uses existing terminal IDs/selectors.

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
Every JSON record names `orchbee.terminal.stream` version 1 and its kind. Frame bytes use base64;
sequence/generation/revision identities are decimal strings. Both format selections emit NDJSON.

Windows stdout is mediated by an invocation-owned, hidden system PowerShell process running a
fixed script with profiles disabled. One bounded record is in flight; the child acknowledges only
after writing and flushing it. The parent polls nonblocking pipes with cancellation/deadlines.
No payload is interpolated into a command. Child stdin uses a synchronous pipe handle; merely
switching an overlapped IPC handle to blocking mode is insufficient for standard child readers. On stalled output or any exit, the owner stops and joins
the child. This cannot retract bytes already delivered to the downstream reader.

## Verification and limits

Deterministic core and scripted Windows transport checks cover exact binding, repeated frames,
resets, partial output, cancellation, expiry and cleanup. Pipe tests must prove a blocked consumer
cannot strand the relay. Same-author tests do not establish independent review, TLA execution,
remote/named Site routing, other platforms, graphics streams or the later control/attach slices.

Cleanup reports `not-attached` before any Viewer attach attempt, `detached` only after its
acknowledgement, or `session-released` when the socket is dropped without confirmed detach.
Scripted evidence does not replace final acceptance against the packaged Windows runtime.
