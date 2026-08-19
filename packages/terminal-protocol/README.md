# Terminal Protocol

`@orchbee/terminal-protocol` is Orchbee's private, transport-neutral wire contract between the native Terminal Host and its local clients. It owns protocol identities, exact current-version validation, protocol safety limits, control schemas, attachment-prefixed binary input, packed terminal-frame and transport-envelope codecs, packed row-window and selection-export codecs, and language-neutral conformance fixtures.

It does not open sockets or named pipes, spawn processes, parse VT, route Electron IPC, render a terminal, or own terminal lifecycle, delivery, recovery, Project, Workspace, Workbench, Tab, Panel, agent, or product-session state.

## Public API

| Area                 | Public contract                                                                                                                                                                          |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Identity             | Strict non-zero 128-bit `HostEpoch`, `CreationId`, `TerminalId`, `ClientId`, and `AttachmentId` values                                                                                   |
| Compatibility        | Exact current protocol `0.0.11` validation; a different version is rejected rather than adapted                                                                                          |
| Message registry     | One current-version mapping from every kind to its legal role, payload encoding, and successful response kind                                                                            |
| Control              | Message-kind-selected request encoding plus strict handshake, configuration, inventory, lifecycle, attachment, projection/history, bell, clipboard-write, selection, and failure schemas |
| Transport            | A bounded 64-byte envelope that rejects producer-side role or encoding mismatches for JSON, input, terminal frames, and row windows                                                      |
| Projection           | OBTF format `0.0.2` and OBPB format `0.0.1` buffers with strict external decode and an explicit immutable-local zero-repack view                                                         |
| History              | OBRW format `0.0.1` render/text row windows, stable origins, applied anchors, and explicit eviction vocabulary                                                                           |
| Selection            | Gesture, command, fixed-header chunk-routing, and bounded acknowledged plain, HTML, or VT export wire shapes                                                                             |
| Input                | Message-kind-selected bounded raw, key, committed-text, mouse, focus, and reviewed-mode paste codecs scoped by `AttachmentId`                                                            |
| Lifecycle wire       | Mutually exclusive terminal phase, normal-exit, runtime-failure, containment, and failed-outcome schemas                                                                                 |
| Correctness evidence | Stable-seed codec properties, malformed corpora, generated-file comparison, and Zig conformance tests                                                                                    |

Every decoder returns a `ProtocolResult`. Malformed or unsupported peer input is an expected failure variant and never requires catching a thrown exception.

OBPB format `0.0.1` has two intentionally separate entry points. `decodeTerminalProjectionBuffer` and `decodeTerminalRowWindow` fully validate untrusted or Gateway bytes. `viewTerminalProjectionBuffer` and `viewTerminalRowWindow` skip the per-cell rescan only for an immutable, capability-scoped local Terminal Host transport. The structural-view functions are not a faster general-purpose decoder.

Mouse motion reports either no pressed buttons and a null active button, or a complete `pressedButtons` set plus one active button contained in that set. Press and release always identify their button; a wheel report is a button press and need not mark that button as held.

Semantic `KeyInput` carries one per-event Option-as-Alt bit. A macOS client resolves its local
off/left/right/both policy before sending the event, while Terminal Host keeps responsibility for
the current Ghostty legacy, modify-other-keys, and Kitty encoding modes.

```ts
import {
	decodeProtocolEnvelope,
	decodeTerminalFrame,
	decodeTerminalFrameRoute,
	inspectTerminalFrameHeader,
	PayloadEncoding,
} from '@orchbee/terminal-protocol';

const envelope = decodeProtocolEnvelope(record);
if (!envelope.ok) return closePeer(envelope.code);

if (envelope.value.payloadEncoding === PayloadEncoding.terminalFrame) {
	const route = decodeTerminalFrameRoute(envelope.value.payload);
	if (!route.ok) return closePeer(route.code);
	forwardToAttachment(route.value.attachmentId, envelope.value.payload);

	const header = inspectTerminalFrameHeader(envelope.value.payload);
	if (!header.ok) return requestSnapshot(header.code);
	if (isAlreadyCommitted(header.value)) return acknowledgeDuplicate(header.value);

	const frame = decodeTerminalFrame(envelope.value.payload);
	if (!frame.ok) return requestSnapshot(frame.code);
	projectTypedColumns(frame.value.cells, frame.value.rowData);
}
```

## Fixtures

The package explicitly exports:

- `@orchbee/terminal-protocol/fixtures/canonical.json`
- `@orchbee/terminal-protocol/fixtures/control-conformance-v0.0.11.json`
- `@orchbee/terminal-protocol/fixtures/malformed-frame-v0.0.2.json`
- `@orchbee/terminal-protocol/fixtures/malformed-row-window-v0.0.1.json`
- `@orchbee/terminal-protocol/fixtures/terminal-frame-v0.0.2.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-failed-v0.0.11.json`
- `@orchbee/terminal-protocol/fixtures/terminal-projection-buffer-v0.0.1.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-row-window-v0.0.1.bin`
- `@orchbee/terminal-protocol/fixtures/transport-envelope-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/write-input-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/key-input-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/text-input-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/mouse-input-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/focus-input-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/paste-input-v0.0.11.bin`
- `@orchbee/terminal-protocol/fixtures/ghostty-alternate-surface-v0.0.2.bin`
- `@orchbee/terminal-protocol/fixtures/ghostty-surface-v0.0.2.bin`

`canonical.json` is only the language-neutral generator input for the current binary formats, so it has no compatibility version. `control-conformance-v0.0.11.json` contains schema-produced command, reply, event, discriminated-union, failure, and complete kind/role/encoding/success-response contracts consumed by the native typed adapter. `fixtures:generate` uses the built TypeScript codecs and schemas to produce those tracked records, the canonical `TerminalFailed` control record, the current OBTF format-`0.0.2` fixture, and the current OBPB and OBRW format-`0.0.1` fixtures, plus the frame and row-window malformed corpora. Unit tests reject stale generated files, and `apps/terminal-host` resolves only these declared package exports before running the Zig validator against the same bytes. Every compatibility fixture carries the exact semantic version of its owning wire protocol or binary format; no wildcard or major-only fixture name is used.

`TerminalProfile` carries explicit `defaultCursorShape` and `defaultCursorBlinking` values. Missing
fields normalize to `block` and `false` so previously persisted profile objects retain the product
default, while current encoders always emit both fields. Runtime escape sequences may override the
active cursor; Terminal Host owns applying these profile defaults again on RIS or application reset.

`ghostty-surface-v0.0.2.bin` and `ghostty-alternate-surface-v0.0.2.bin` are the exceptions to
TypeScript fixture generation: Terminal Host produces both from one pinned native Ghostty VT and
its real projection encoder. Host conformance regenerates and byte-compares the pair, while
`@orchbee/terminal` consumes the primary snapshot through both renderer backends and commits the
two snapshots in sequence through the packed store. The primary cells cover grapheme/width/wrap,
OSC 8, OSC 133 prompt/input/output boundaries, OSC 4/10/11/12 palette and dynamic colors, cursor
color, style flags, explicit colors, and underline variants. The pair additionally covers atomic
title, current-directory, active-screen, cursor, interaction-mode, generation, and row replacement.
The protocol package owns only the stable OBTF bytes and exports; Host and renderer records own the
semantic assertions.

Live OBTF snapshots and deltas may carry optional extension kind `1`, whose one-byte payload reports
close activity as `idle=1` or `active=2`. The producer omits the extension when the observation is
unknown. The extension is transient projection metadata only; lifecycle and close policy remain
consumer-owned.

Optional extension kind `2` is one strictly decoded `OBKG` record for supported Kitty direct-image
output. A resource snapshot carries already decoded gray, gray-alpha, RGB, or RGBA bytes plus the
current visible placements. A placement-only update names the exact image revision it reuses.
Duplicate records, stale revisions, bad pixel lengths, missing image references, noncanonical
flags, or limit violations reject the complete OBTF frame. The format does not carry paths,
compressed containers, shared memory, virtual placements, negative-z composition, glyph graphics,
or animation.

Optional extension kind `3` is the current terminal progress sample. Its two-byte payload carries
`remove`, `set`, `error`, `indeterminate`, or `pause` plus either a canonical percentage from
`0` through `100` or the null sentinel `255`. `remove` and `indeterminate` cannot carry a
percentage. Absence means `remove`, so a snapshot never invents or replays a completed task.

Optional extension kind `4` is the latest semantic command completion. Its 25-byte payload carries
a positive terminal-local `u64` sequence, `u64` duration milliseconds, positive `u64` Unix-epoch
completion milliseconds, and normalized `u8` exit code. It never carries command text, arguments,
directory, environment, or output. Duplicate or malformed records reject the frame; clients own
freshness, focus, permission, deduplication, rate limiting, and notification effects.

`TerminalClipboardWriteRequest` is a separate event-only control record for one Host-admitted OSC
52 plain-text write. It carries the exact attachment, a positive sequence, and at most 16 KiB of
valid UTF-8 without NUL or unpaired surrogates. The schema grants no clipboard read, selection kind,
HTML, image, persistence, trust, or effect capability; the application must obtain an explicit user
decision before performing the platform write.

## Ownership

This package defines how a value is represented and validated on the wire. It deliberately does not define which lifecycle transition is legal, when an effect occurs, how delivery is committed, or how a product recovers. Those authorities live with their state owners:

- [`apps/terminal-host/docs/runtime-model.md`](../../apps/terminal-host/docs/runtime-model.md) owns terminal lifecycle, runtime containment, attachment delivery, retry, and Host concurrency.
- `apps/desktop` owns product binding, close escalation, reconnection, publication, and update policy.

Protocol schemas make illegal wire combinations unrepresentable where possible. Consumer-owned models and tests decide whether a valid wire value is legal in the consumer's current state.

The complete wire layouts, message registry, bounds, and evidence limits are in [`references/protocol.md`](references/protocol.md). [`docs/version-history.md`](docs/version-history.md) owns the pre-release policy and, beginning with `1.0.0`, every released wire-protocol and binary-format version change. The NPM package version is independent release metadata and is not a wire-compatibility version.

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

Run `pnpm --filter @orchbee/terminal-protocol fixtures:generate` only after an intentional reviewed fixture change. `test:fixtures` compares the tracked files without rewriting them.
