# Terminal Host

Native one-terminal runtime and pinned Ghostty VT integration boundary for Orchbee's local Host.

## Current scope

This workspace owns the one-terminal native Host core, PTY lifecycle, live projection producer,
bounded history and selection authority, semantic input authority, and secure local IPC runtime
through Slice 7. Slice 7 changes no Host product ownership: Desktop consumes the existing bounded
inventory and idempotent Create contract to recover its opaque terminal identity. The launcher ABI is
`terminal-host serve --runtime-directory <absolute-path> --endpoint-id <32-lowercase-hex>`.
The runtime directory and endpoint identity are supplied by the future Desktop runtime manager;
the Host interprets no product placement or release-channel model.

The Host never owns Project, Workspace, Workbench, Tab, Panel, agent, or product-session models.
Electron lifecycle, rendering, product presentation, platform effects, and update orchestration
remain outside this application. It contains no TCP, HTTP, WebSocket, TLS, remote-authentication,
or sharing listener.

The Host retains at most one terminal. It parses PTY output into one Ghostty terminal with a
10,000,000-byte scrollback bound before any client transport. PTY reads are at most 256 KiB, queued
input is at most 4 MiB, and one input request is at most the negotiated 1 MiB. A terminal becomes
`exited` only after both child completion and PTY drain; an unexpected PTY read failure is not
classified as a drain. Disconnect removes that connection's attachments and never sends the
platform termination request.

`ListTerminals` exposes at most one opaque terminal summary: exact identity, lifecycle phase,
dimensions, revision, termination-requested fact, and optional exit result. The Host never assigns
that terminal to a product concept and never adopts, recreates, terminates, or disposes it during
Desktop reconciliation. Replaying an identical `CreationId` and launch in the same Host epoch is
idempotent; a different launch is a typed creation conflict.

On POSIX, the Host requires an existing absolute runtime directory owned by the effective user with
no group or other permissions. It takes an endpoint-specific lifetime file lock, publishes a `0600`
Unix-domain socket in that directory, and accepts only the same effective UID. On Windows, it
requires an owned, non-reparse runtime directory whose DACL grants access only to the current user,
Local System, and administrators; its endpoint-specific mutex and byte-mode named pipe use a
protected current-user DACL, reject remote clients, and compare the connected token SID with the
Host token SID. Endpoint names are identifiers, not secrets.

The Host services one local broker connection at a time while preserving the terminal across broker
reconnects. A protocol 1.1 readable attachment starts with projection demand paused. Activating it
lazily creates one reusable Ghostty render state and emits bounded format-2 snapshots or full-row
deltas with one frame in flight per attachment. Pausing suppresses capture and encoding while PTY
parsing and canonical revision tracking continue. Title, current directory, cursor, interaction
modes, and final revision travel with the live projection.

Protocol 1.2 adds attachment-local history demand, one row request in flight, and render- or
text-detail row windows bounded to 2 MiB and 4,096 rows. The Host resolves absolute, oldest, tail,
and committed-anchor origins against Ghostty's retained pages. A row window becomes the committed
anchor only after `RowsApplied`; eviction, resize, screen replacement, detach, and stale
revision/generation paths invalidate the relevant pins without naming replacement content.
Format-1.1 text windows omit the serialized mapping table only for verified narrow ASCII rows whose
byte, UTF-16, and terminal-column offsets are identical. Unicode and non-identity rows retain the
explicit table. Both forms share the 262,144 logical-entry ceiling, and the Host may return fewer
than the requested rows to preserve that bound.

Selection endpoints are tracked against Ghostty pages in Host memory. Gestures, select-all,
select-output, and clear return bounded selection state; eviction is reported explicitly. Plain,
HTML, and VT exports are capped at 16 MiB and emitted as one acknowledged chunk at a time, with each
chunk capped at 1 MiB. Search queries never enter the Host: a client may request bounded text-detail
rows for its own search implementation, but the Host remains unaware of query text and matches.

Keyboard, committed text, mouse, focus, and approved paste requests are encoded against the Host's
authoritative Ghostty state and share the same bounded ordered ingress as raw `WriteInput` and
terminal-generated replies. The Host sends no raw PTY output to a client and opens no network
listener. Electron routing, clipboard/external-navigation effects, search execution, product
recovery binding, and update activation remain outside this application. Electron's current binary
projection route uses one Main-to-Renderer structured clone and makes no ownership-transfer claim.

## Pinned inputs

| Input                | Value                                                            |
| -------------------- | ---------------------------------------------------------------- |
| Zig                  | `0.15.2` exactly                                                 |
| Ghostty revision     | `91f66da24527fa02d92b5fd0b41cd020f553a64c`                       |
| Ghostty package hash | `ghostty-1.3.2-dev-5UdBCx16GwVpqni9yfmuNj13woRRCOiIvLQ64uB3CnbG` |

`build.zig.zon` is the dependency authority. Builds reject another Zig version or Ghostty archive
content and never read a sibling checkout.

## Development

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host format:check
pnpm --filter @orchbee/terminal-host build
pnpm --filter @orchbee/terminal-host test
pnpm --filter @orchbee/terminal-host test:protocol
pnpm --filter @orchbee/terminal-host test:cross
pnpm --filter @orchbee/terminal-host benchmark -- --bytes 67108864 --columns 120 --rows 40 --mode no-demand
pnpm --filter @orchbee/terminal-host benchmark -- --bytes 67108864 --columns 120 --rows 40 --mode active
pnpm --filter @orchbee/terminal-host benchmark -- --file /absolute/path/to/output.txt --columns 113 --rows 38 --cell-pixel-width 18 --cell-pixel-height 36 --mode active
pnpm verify
```

`test:protocol` resolves the canonical frame, row-window, envelope, and attachment-prefixed input only through
`@orchbee/terminal-protocol`'s declared package exports, injects those exact bytes into the Zig test
build, and checks valid and malformed records. It does not depend on a sibling source import or a
machine-local checkout.

`test` exercises the current operating system's PTY/process adapter, VT runtime, one-terminal
service, projection capture and backpressure, semantic input ordering, malformed local transport,
connection cleanup, and endpoint security. Repository CI is
configured to run these native tests on Linux, macOS, and Windows. `test:cross` additionally
compiles the complete test graph for one maintained target in each family; cross-compilation alone
does not claim runtime behavior. Packaging and installer behavior remain outside this slice. Build
output remains under ignored `.zig-cache/` and `zig-out/` directories.

`benchmark` spawns its deterministic byte generator through the real PTY and reports JSON for wall
time, CPU time, peak-RSS delta, PTY batches, revisions, frames, and projection counters. Run the
same geometry and byte count in `no-demand` and `active` modes. The no-demand result must keep
render-state creation, capture, encoding, and frame counts at zero; active mode samples at a 16 ms
display cadence and immediately acknowledges its one frame base. It is focused Host evidence, not
an Electron or Ghostty GUI end-to-end comparison.

`ORCHBEE_TERMINAL_HOST_TRACE=1` additionally emits bounded interaction timing records for the
Desktop benchmark harness. It is a controlled-test diagnostic switch, not a product telemetry or
logging contract.

On POSIX, `--file <absolute-path>` replaces the generated fixture with a numbered `/bin/cat` under
`/usr/bin/time -p`, so the focused Host path can be measured against a geometry-matched Desktop
run. It is mutually exclusive with `--bytes`; explicit cell-pixel dimensions make Retina and
non-Retina comparisons auditable. File-fixture mode is not available on Windows.

## Native module visibility

`src/main.zig` is the executable composition root and private launcher ABI. `src/host_core.zig`
owns deterministic state transitions; `src/host_service.zig` binds validated protocol operations to
the core and runtime. `src/terminal_projection.zig` owns the lazy Ghostty render state and format-2
capture/encoding scratch. `src/terminal_history.zig` owns attachment-local row anchors, bounded row
encoding, tracked selection, and chunked selection export. `src/terminal_runtime.zig`,
`src/pty_process.zig`, and `src/pty.zig` own
VT, process, and PTY effects; `src/benchmark.zig` and `src/benchmark_options.zig` are the repeatable
focused throughput harness and its tested CLI contract;
`src/windows_api.zig` is the narrow ConPTY binding. `src/local_ipc.zig`
owns endpoint bootstrap and record dispatch;
`src/local_ipc_windows.c` and its header are the narrow Windows ACL, SID, mutex, and named-pipe
adapter. `src/protocol.zig` is the private bounded native codec, while
`src/protocol_conformance_test.zig` is a test root and never enters the installed executable. The
Zig build graph is the visibility authority; no other workspace imports these source files, and
this application exports no native library module.
