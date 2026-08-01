# Terminal Protocol

`@orchbee/terminal-protocol` is Orchbee's private, transport-neutral contract between the native
terminal Host and its local clients. It owns protocol identities, version negotiation, hard bounds,
control schemas, attachment-prefixed binary input, packed terminal-frame and transport-envelope
codecs, packed row-window and selection-export codecs, lifecycle transitions, and language-neutral
conformance fixtures.

It does not open sockets or named pipes, spawn processes, parse VT, route Electron IPC, render a
terminal, or own Project, Workspace, Workbench, Tab, Panel, agent, or product-session state.

## Public API

| Area                 | Public contract                                                                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------ |
| Identity             | Strict non-zero 128-bit `HostEpoch`, `CreationId`, `TerminalId`, `ClientId`, and `AttachmentId` values |
| Compatibility        | Protocol `1.0`/`1.1`/`1.2` validation and deterministic highest-overlap minor negotiation              |
| Control              | Handshake, lifecycle, attachment, projection/history demand, row reads, selection/export, and failures |
| Transport            | A bounded 64-byte envelope for JSON, raw/semantic input, terminal frames, and row-window payloads      |
| Projection           | Format-2 full-row snapshots/deltas decoded into typed columns rather than per-cell JavaScript objects  |
| History              | Format-1.1 render/text row windows, stable origins, applied anchors, and explicit eviction semantics   |
| Selection            | Host-owned gestures/commands and bounded acknowledged plain, HTML, or VT export chunks                 |
| Input                | Bounded raw, key, committed-text, mouse, focus, and paste codecs scoped by `AttachmentId`              |
| Lifecycle            | A serializable deterministic transition function that returns commands without performing effects      |
| Correctness evidence | Stable-seed property tests, malformed corpus, checked TLA+ models, and Zig conformance tests           |

Every decoder returns a `ProtocolResult`. Malformed or unsupported peer input is an expected
failure variant and never requires catching a thrown exception.

```ts
import {
	decodeProtocolEnvelope,
	decodeTerminalFrame,
	PayloadEncoding,
} from '@orchbee/terminal-protocol';

const envelope = decodeProtocolEnvelope(record);
if (!envelope.ok) return closePeer(envelope.code);

if (envelope.value.payloadEncoding === PayloadEncoding.terminalFrame) {
	const frame = decodeTerminalFrame(envelope.value.payload);
	if (!frame.ok) return requestSnapshot(frame.code);
	projectTypedColumns(frame.value.cells, frame.value.rowData);
}
```

## Fixtures

The package explicitly exports:

- `@orchbee/terminal-protocol/fixtures/canonical-v1.json`
- `@orchbee/terminal-protocol/fixtures/malformed-frame-v2.json`
- `@orchbee/terminal-protocol/fixtures/malformed-row-window-v1.json`
- `@orchbee/terminal-protocol/fixtures/terminal-frame-v2.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-row-window-v1.bin`
- `@orchbee/terminal-protocol/fixtures/transport-envelope-v1.bin`
- `@orchbee/terminal-protocol/fixtures/write-input-v1.bin`
- `@orchbee/terminal-protocol/fixtures/key-input-v1_1.bin`
- `@orchbee/terminal-protocol/fixtures/text-input-v1_1.bin`
- `@orchbee/terminal-protocol/fixtures/mouse-input-v1_1.bin`
- `@orchbee/terminal-protocol/fixtures/focus-input-v1_1.bin`
- `@orchbee/terminal-protocol/fixtures/paste-input-v1_1.bin`

`canonical-v1.json` is the language-neutral logical input across the compatible protocol-minor
line. `fixtures:generate` uses the built TypeScript codecs to produce the tracked binary records and
format-2 frame and format-1.1 row-window malformed corpora. Unit tests reject stale generated files, and `apps/terminal-host`
resolves only these declared package exports before running the Zig validator against the same
bytes.

## Lifecycle model

`transitionProtocolLifecycle` parses a serializable state and event, applies one deterministic
transition, validates every next-state invariant, and returns serializable commands for a later
adapter. It never executes a command itself. The current model covers Host launch/drain, idempotent
terminal creation, reordered exit completion, client detach, attachment capabilities, resize lease,
bounded input, one-frame backpressure, snapshot recovery, and deferred Host upgrade. The refined
frame model additionally covers mutation/capture separation, projection demand, generation changes,
capture/encode failure, blocked recovery, duplicates, finalization, and two independent attachments.
The history-interaction model covers one bounded row flight, pending-to-committed anchor promotion,
coalesced history observation, selection eviction, chunked export, stale search pages, and detach.

The complete current contract, wire layouts, transition record, refinement mapping, claims, and
evidence limits are in [`references/protocol-v1.md`](references/protocol-v1.md).

## Development

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-protocol typecheck
pnpm --filter @orchbee/terminal-protocol build
pnpm --filter @orchbee/terminal-protocol test
pnpm --filter @orchbee/terminal-protocol test:exports
pnpm --filter @orchbee/terminal-host test:protocol
pnpm verify
```

Run `pnpm --filter @orchbee/terminal-protocol fixtures:generate` only after an intentional reviewed
fixture change. `test:fixtures` compares the tracked files without rewriting them. TLA+ uses the
same pinned `tla2tools` revision as other modeled workspaces and keeps its downloaded jar under the
ignored package-local `.tools/` directory.
