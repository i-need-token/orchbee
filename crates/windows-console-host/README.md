# Windows Console Host

Private, experimental Rust boundary for using caller-admitted Windows console handles without
reopening process-global standard streams. The Orchbee native application is its current consumer.

## Retained-handle contract

`ConsoleInput::admit` and `ConsoleOutput::admit` take ownership of exact `OwnedHandle` values cloned
by the caller. Every mode, readiness, record, geometry, and output operation uses those retained
handles. The crate never calls `GetStdHandle`, opens `CONIN$` or `CONOUT$`, uses
`std::io::stdout`, or exposes a raw Windows handle or Win32 representation type.

The caller owns the original handles, mode restoration journal, terminal lifecycle, and output
ordering. Dropping an endpoint closes only the admitted duplicate after its last retained adapter
is gone. The input endpoint is not `Clone`; the application retains one canonical reader across
startup and TUI phases.

## Input records

`read_ready_record` first asks `GetNumberOfConsoleInputEvents`. A zero count returns `None` without
calling `ReadConsoleInputW`; a positive count requests one record and admits it only when Win32
reports exactly one result. The readiness observation and synchronous read are not an atomic
reservation against an external process or another independently aliased OS reader. The caller
must ensure single-reader ownership; competing readers can consume the observed record and make
the subsequent Win32 read block.

The safe record types preserve the raw Unicode UTF-16 unit, repeat count, virtual key and scan
codes, control flags, absolute screen-buffer mouse coordinates, button state, event flags, focus,
resize, menu, and unknown event tag. Product key decoding, surrogate and repeat state, AltGr
handling, mouse-button history, window-origin subtraction, and terminal-byte encoding remain with
the application.

## Unicode output

Each `ConsoleWriter` is one independent ordered UTF-8 stream over the retained output handle. It
keeps at most three incomplete UTF-8 bytes and one pending low surrogate. `WriteConsoleW` calls are
bounded to 4096 UTF-16 units, and every returned count is mapped back through Unicode scalar
boundaries before the writer reports consumed UTF-8 bytes. A short write after a high surrogate
retains only the low surrogate, so the accepted high surrogate is never replayed.

A zero count, invalid count, invalid or incomplete UTF-8, or native failure latches that writer.
Later calls on it perform no native output. `ConsoleWriteError` records trusted complete-scalar or
split-scalar progress and marks a failing native call as unknown when Win32 cannot prove its effect.
A fresh writer starts with empty state and may attempt bounded lifecycle cleanup; it never resumes
the failed writer's payload. Callers keep one writer for each semantic stream and flush it before
normal drop.

## Limits

This crate does not interpret terminal input, render a TUI, choose modes, journal restoration,
serialize independent writers, host a pseudoconsole, launch a process, or adapt redirected byte
streams. Window geometry is reported in cells with a signed screen-buffer origin; pixel dimensions
remain outside this boundary.

## Verification

Run the native Windows checks from the repository root:

```sh
node scripts/run-rust-workspace.mjs test --package orchbee-windows-console-host
pnpm lint:rust
pnpm test:rust-runner
pnpm verify
```
