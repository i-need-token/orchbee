# Terminal Host

Native multi-terminal runtime and pinned Ghostty VT integration boundary for Orchbee's local Host.

## Current scope

This workspace owns native terminal, runtime, connection, creation, and attachment registries; PTY
lifecycle; live projection production; bounded history and selection authority; semantic input
authority; and secure local IPC. Desktop consumes terminal identities without moving product
topology into the Host. The launcher ABI is
`terminal-host serve --runtime-directory <absolute-path> --endpoint-id <32-lowercase-hex>`.
The runtime directory and endpoint identity are supplied by the future Desktop runtime manager;
the Host interprets no product placement or release-channel model.

The Host never owns Project, Workspace, Workbench, Tab, Panel, agent, or product-session models.
Electron lifecycle, rendering, product presentation, platform effects, and update orchestration
remain outside this application. It contains no TCP, HTTP, WebSocket, TLS, remote-authentication,
or sharing listener.

The Host retains each terminal until explicit disposal and binds exactly one native runtime to each
retained `TerminalId`. It defines no product count ceiling for terminals, attachments, or
connections; allocator, identifier-space, process, PTY, and operating-system exhaustion reject the
current operation without shrinking or corrupting retained registries. Each runtime parses PTY
output into its own Ghostty terminal with a 10,000,000-byte scrollback bound before any client
transport. PTY reads are at most 256 KiB, queued input is at most 4 MiB per terminal, and one input
request is at most the negotiated 1 MiB. A terminal becomes
`exited` only after both child completion and PTY drain; an unexpected PTY read failure is not
classified as a drain. Disconnect removes that connection's attachments and never sends the
platform termination request.

Protocol 1.4 separates graceful and force termination. The Host never owns the escalation timer:
Desktop first sends `TerminateTerminal`, may wait for completion, and only then sends
`ForceTerminateTerminal`. Both requests are independently idempotent, and neither disposes the
terminal; disposal remains legal only after child exit and PTY drain have both been observed. On
macOS, Linux, and FreeBSD, the process adapter enumerates current operating-system session members,
rechecks each membership immediately before signaling, and sends HUP for graceful termination. A
force request stops the current session, enumerates it again, and sends KILL, so shell job-control
groups are covered while a process that deliberately entered another session with `setsid()` is
excluded. On Windows, every child starts suspended, enters a kill-on-close Job Object before its
first instruction resumes, and force termination ends that Job and its descendants. Final runtime
destruction also force-cleans an incomplete child/drain pair before joining worker threads; a fully
completed retained terminal is never signaled again by numeric process identity during disposal.

Protocol 1.3 `ListTerminals` exposes opaque terminal summaries in stable membership pages of at most
128 terminals and 64 KiB. A cursor carries the inventory generation and next offset; any create or
dispose changes the generation, so a stale cursor fails instead of mixing memberships. The protocol
sets no total terminal count limit. Protocol 1.0-1.2 still returns its legacy zero-or-one summary,
but fails with `protocol-upgrade-required` instead of truncating when multiple terminals exist. A
1.0-1.2 client may replay an existing `CreationId`, but creating a distinct second terminal requires
1.3. Replaying an identical creation and launch in the same Host epoch remains idempotent; a
different launch is a typed creation conflict.

`ClientId` is wire metadata and may be equal on multiple physical connections. The Host assigns a
private monotonic `ConnectionId` to each accepted connection; attachment authority is owned by that
physical identity. Every attachment operation resolves the exact
`ConnectionId + AttachmentId -> TerminalId -> Runtime` route. Disconnect removes only that physical
connection's attachments and never terminates a terminal or affects another connection with the
same `ClientId`.

On POSIX, the Host requires an existing absolute runtime directory owned by the effective user with
no group or other permissions. It takes an endpoint-specific lifetime file lock, publishes a `0600`
Unix-domain socket in that directory, and accepts only the same effective UID. On Windows, it
requires an owned, non-reparse runtime directory whose DACL grants access only to the current user,
Local System, and administrators; its endpoint-specific mutex and byte-mode named pipe use a
protected current-user DACL, reject remote clients, and compare the connected token SID with the
Host token SID. Endpoint names are identifiers, not secrets.

The current local IPC listener services one broker connection at a time while preserving all
terminals across broker reconnects; the service/core identity model does not conflate reconnects
that reuse a `ClientId`. After a physical connection returns, the listener exits only when no
connection, terminal, or runtime remains. A retained terminal therefore keeps the Host available
for reconnect, while an empty Host does not become an orphan process. A protocol 1.1 readable
attachment starts with projection demand paused.
Activating it lazily creates one terminal-owned Ghostty render state and emits bounded format-2 snapshots or
full-row deltas with one frame in flight per attachment. Pausing abandons that attachment's
in-flight frame, invalidates its delta base, and suppresses dirty marking, capture, and encoding
while PTY parsing and canonical revision tracking continue. When no attachment for the terminal has
active projection demand, the Host releases the render state and projection caches; later activation
recreates them and emits a full snapshot. Projection cadence is tracked per terminal, while bounded
round-robin scans prevent one attachment or terminal from monopolizing a poll. Title, current
directory, cursor, interaction modes, and final revision travel with the live projection.

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

`test` exercises the current operating system's PTY/process adapter, VT runtimes, multi-terminal
route isolation, atomic spawn failure, paged inventory, two-phase terminal closure, POSIX session
termination, physical-connection isolation, projection capture
and backpressure, semantic input ordering, malformed local transport, connection cleanup, and
endpoint security. Repository CI is
configured to run these native tests on Linux, macOS, and Windows. Windows-native tests also verify
that a spawned descendant is retained in the terminal Job. `test:cross` additionally compiles the
complete test graph for Linux, FreeBSD, macOS, and Windows; cross-compilation alone
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
owns deterministic registries and state transitions; `src/host_service.zig` owns the per-terminal
runtime registry, exact request routing, and bounded fair scheduling while binding validated
protocol operations to the core. `src/terminal_projection.zig` owns the lazy Ghostty render state and format-2
capture/encoding scratch. `src/terminal_history.zig` owns attachment-local row anchors, bounded row
encoding, tracked selection, and chunked selection export. `src/terminal_runtime.zig`,
`src/pty_process.zig`, and `src/pty.zig` own
VT, process, and PTY effects; `src/benchmark.zig` and `src/benchmark_options.zig` are the repeatable
focused throughput harness and its tested CLI contract;
`src/windows_api.zig` is the narrow ConPTY and terminal-Job binding. `src/local_ipc.zig`
owns endpoint bootstrap and record dispatch;
`src/local_ipc_windows.c` and its header are the narrow Windows ACL, SID, mutex, and named-pipe
adapter. `src/process_session.c` owns POSIX session enumeration and signaling;
`src/process_job_windows.c` owns Windows Job creation, assignment, termination, and native test
inspection. `src/protocol.zig` is the private bounded native codec, while
`src/protocol_conformance_test.zig` is a test root and never enters the installed executable. The
Zig build graph is the visibility authority; no other workspace imports these source files, and
this application exports no native library module.
