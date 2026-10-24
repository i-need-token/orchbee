# Terminal Protocol

`@orchbee/terminal-protocol` is Orchbee's private, transport-neutral wire contract between the native Terminal Host and its local clients. It owns protocol identities, exact current-version validation, protocol safety limits, control schemas, attachment-prefixed binary input, packed terminal-frame and transport-envelope codecs, packed row-window and selection-export codecs, and language-neutral conformance fixtures.

It does not open sockets or named pipes, spawn processes, parse VT, route Electron IPC, render a terminal, or own terminal lifecycle, delivery, recovery, Project, Workspace, Workbench, Tab, Panel, agent, or product-session state.

The unpublished independent Rust binding at
[`crates/terminal-protocol`](../../crates/terminal-protocol) currently mirrors the complete message
registry, fixed OBTP transport envelope, and the `Hello`, `HelloAccepted`, `ProtocolRejected`,
`Ping`, and `TypedFailure` connection-control payloads, plus the kind-10 `ListTerminals` request,
`TerminalList` response, nested inventory facts, and the existing non-creation lifecycle controls
and observations for kinds 12 through 19, attachment controls for kinds 20 through 25, the raw
`WriteInput`/`InputAccepted` exchange for kinds 30 and 31, and the `ResizeTerminal`/
`ResizeAccepted` control exchange for kinds 32 and 33, plus the semantic `KeyInput`, `TextInput`,
`MouseInput`, `FocusInput`, `PasteInput`, and `PasteAutoInput` requests for kinds 34 through 39. This package, its
reference, and its fixtures remain the sole wire authority. The unshipped
[`orchbee-worker`](../../apps/orchbee-worker/README.md) application consumes the Rust envelope, connection, launch/lifecycle, attachment, projection-demand,
and feedback codecs for its actual Host broker. Worker owns socket authentication, handshake state,
connection liveness, and viewer relay behavior. The binding includes kind-11 `CreateTerminal` and
kind-12 `TerminalCreated` launch controls. Its `inspect_terminal_frame_route` reads only the OBTF
0.1.0 routing prefix: exact framing/version, kind/base-revision relationship, nonzero Host/attachment
identity, sequence, generation, and revision. It does not validate presentation fields, tables,
cells, or extensions; Client projection decoding remains a separate boundary. Existing TypeScript
and real Ghostty fixtures verify this zero-copy route inspection. Beyond route inspection, the
binding decodes bounded OBRW 0.1.0 text-detail row windows into `TerminalTextRowWindow`; a legal
render window completes full header validation, then fails closed as outside the text-only API.
The binding also covers the
bidirectional `FrameApplied` and `NeedSnapshot` projection-feedback controls for kinds 43 and 44,
the projection-demand request, acceptance, and blocked event for kinds 47 through 49, and the
history-demand, change, and observation controls for kinds 51 through 54. Its row-access control
binding covers the kind-45 `ReadRows` request and both directions of kind-50 `RowsApplied`; binary
kind-46 `RowsSnapshot` is decoded through the separate text and render APIs. It also covers the event-only
`TerminalBell`, `TerminalClipboardWriteRequest`, `TerminalMetadataChanged`, and
`TerminalApplicationSignal` representations for kinds 55 through 58; every audible, clipboard,
metadata, application-signal, and notification effect remains outside the binding. The Rust crate
also covers the four `SelectionGesture` requests, four `SelectionCommand` requests, and both legal
roles of all three `SelectionChanged` states for kinds 60 through 62. Its selection-export control
binding covers kinds 63, 64, 66, 67, and 68 in every legal direction, and its raw binding covers
the exact kind-65 `SelectionExportChunk` header, route metadata, and opaque bounded body. The Rust
crate also covers both terminal-default and retained-terminal configuration exchanges for kinds 70
through 73. It does not implement handshake state, inventory pagination or reconciliation,
terminal creation or launch,
attachment, lifecycle, input, resize, projection-feedback, projection-demand, history, row capture
or anchor, terminal-effect delivery, gesture interpretation, selection state or export behavior,
export-chunk sequencing or acknowledgement behavior, queue state or effects, semantic-input
terminal encoding, terminal-default or configuration mutation and persistence, terminal output,
IPC, or Worker behavior.

## Application screen control

The independent Rust `ScreenControl` binding mirrors kinds 74–76: ClearTerminal, ResetTerminal and TerminalControlAccepted. Clear accepts optional Boolean `preservePrompt`; omission retains screen erase/home. Reset stays closed to this field. Accepted generation is positive and revision retains the general canonical u64 wire domain; the prompt-preserving operation consumer separately requires a positive advanced revision. [Host runtime](../../apps/terminal-host/docs/runtime-model.md#prompt-preserving-clear) owns semantics; [native clear](../../apps/orchbee/docs/native-cli-query.md#clear-terminal-output) owns transient input capability, deadlines and uncertainty. Matching development cohorts are required.

## Rust text-frame binding

`decode_terminal_text_frame` consumes the unchanged OBTF 0.1.0 encoding for the private native TUI.
It validates the routing/header, geometry, strict UTF-8 string bounds, styles, sorted row/cell tables,
wide-cell and adjacent-wrap relationships, exact length, and bounded optional extension envelopes
before returning `TerminalTextFrame`. The output carries rows of shared `Arc<str>` graphemes and
text styles, cursor position/visibility, complete geometry, screen, colors, theme revision,
routing facts, and the validated `captured_interaction` modes word — exposed through the narrow
`TerminalInteraction` tracking/format/cursor-mode/bracketed-paste/focus-reporting queries rather than raw bits. Sharing strings
prevents repeated cell references from expanding one wire string into one allocation per cell.
Nullable `title` and `current_directory` retain the validated header string references as
shared `Arc<str>` values, including empty strings. The all-ones index means absent. Snapshot
and delta headers replace these observations completely; null clears a prior value. No URI,
filesystem, process or presentation interpretation occurs in this decoder.

Optional progress extension kind `3` is decoded into a closed `TerminalProgress` value.
Set/error/pause carry an optional validated `TerminalProgressValue` percentage; remove and
indeterminate cannot carry one. The frame retains extension presence as `Option`; absence has
the existing wire meaning of removed progress. Duplicate records, wrong lengths, invalid state
tags, noncanonical percentages and forbidden combinations reject the whole frame before publication.
This is a current sample, with no inference about provider identity or work completion.

This is a text-table decoder, not a full graphics/extension decoder or a Client state machine.
Other optional extension contents are skipped without interpretation or retention; every required
extension still rejects. It never executes VT, activates hyperlinks, performs clipboard effects,
applies deltas, sends feedback, or renders. Existing TypeScript and real Ghostty fixtures plus the
owner's malformed-frame corpus and byte-boundary tests maintain conformance. The native TUI owns
atomic snapshot/delta application, retained-memory admission, escape-safe cell rendering and exact
feedback after a successful draw. Worker and Server continue to use only route inspection.

## Rust text row-window binding

`decode_terminal_text_row_window` consumes unchanged OBRW 0.1.0 text detail for text-row consumers.
Interactive native history uses the render binding. The text binding validates the complete shared header — exact framing and
version, total and body lengths, required flags and reserved fields, window shape, detail
section lengths, and non-zero Host/attachment identities — before a legal render window fails
closed as outside this text-only API; its payload is never interpreted. For text detail it
validates every row's flags, prompt class, reserved fields, strict UTF-8 text, identity-ASCII
semantics, canonical mapping order, scalar-boundary byte and UTF-16 offsets, non-overlapping
columns, adjacent wrap agreement, per-cell span and cumulative entry limits, and exact body
and entry counts before returning `TerminalTextRowWindow`. The result carries absolute row
indexes, prompt classes, wrap relationships, identity-ASCII markers with empty mapping tables,
and explicit mapping fields; byte-order-mark code points are preserved and display width is
never inferred from text.

This is a text-detail decoder, not a render decoder, a history restoration path, or a client
state machine. It never parses render payloads, applies rows to a terminal, restores history
demand, or selects text. The shared malformed row-window corpus, packed projection
conformance cases, the canonical row-window fixture, and strict-prefix truncation plus
UTF-8/mapping boundary tests maintain conformance with the TypeScript owner and Zig
validator.

## Public API

| Area                 | Public contract                                                                                                                                                                                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Identity             | Strict non-zero 128-bit host, creation, terminal, client, attachment, correlation, and terminal-theme operation identities                                                                                                                  |
| Compatibility        | Exact current protocol `0.1.0` validation; a different version is rejected rather than adapted                                                                                                                                              |
| Message registry     | One current-version mapping from every kind to its legal role, payload encoding, and successful response kind                                                                                                                               |
| Control              | Message-kind-selected request encoding plus strict handshake, configuration/theme, inventory, lifecycle/launch-overlay, attachment, projection/history, bell, clipboard-write, metadata, application-signal, selection, and failure schemas |
| Transport            | A bounded 64-byte envelope that rejects producer-side role or encoding mismatches for JSON, input, terminal frames, and row windows                                                                                                         |
| Projection           | OBTF format `0.1.0` and OBPB format `0.1.0` buffers with strict external decode and an explicit immutable-local zero-repack view                                                                                                            |
| Graphics             | OBKG format `0.1.0` owned decoded images, resolved placements, signed-z layers, animation ticks, and strict snapshot/placement-update admission                                                                                             |
| History              | OBRW format `0.1.0` render/text row windows, render-detail graphics snapshots, stable origins, applied anchors, and explicit eviction vocabulary                                                                                            |
| Selection            | Gesture, command, fixed-header chunk-routing, and bounded acknowledged plain-text-only export wire shapes                                                                                                                                   |
| Input                | Message-kind-selected bounded raw, key, committed-text, mouse, focus, and reviewed-mode paste codecs scoped by `AttachmentId` and its positive capability revision                                                                          |
| Lifecycle wire       | Mutually exclusive terminal phase, normal-exit, runtime-failure, containment, and failed-outcome schemas                                                                                                                                    |
| Correctness evidence | Stable-seed codec properties, malformed corpora, generated-file comparison, and Zig conformance tests                                                                                                                                       |

Every decoder returns a `ProtocolResult`. Malformed or unsupported peer input is an expected failure variant and never requires catching a thrown exception.

`TerminalLaunch` carries either four positive geometry fields for immediate/headless startup or four
null geometry fields for deferred visual startup; mixed values are invalid. Deferred creation
returns identity in `awaiting-start` without an implicit grid. `StartTerminal` supplies the complete
geometry and receives `TerminalStartAccepted` with the exact identity, geometry, and `started` or
`already-started` transition. The protocol defines no visual timeout or 80x24 fallback.

`CreateTerminal.environmentOverlay` is a required, separate array of at most 16 non-null environment
values, each at most 4 KiB. It is not part of `TerminalLaunch.environment`: overlay names must be
unique under the target operating system's environment identity rule and must not overlap a durable
launch environment name. An empty overlay and a supplied overlay remain distinguishable inputs so
the Host can implement recovery validation without placing capability values in durable launch
identity. The protocol owns this bounded shape only; Host and application owners define its
in-memory lifetime, digest comparison, revocation, and recovery behavior.

`ApplyTerminalTheme` carries a terminal identity, unique operation identity, expected canonical
uint64 theme revision, and one complete terminal theme: color scheme, default foreground/background,
nullable cursor RGB, and exactly 256 palette entries. `TerminalThemeResult` reports `applied`,
`already-applied`, or `stale` plus the complete current state. Revision zero is the launch baseline
and is the only state with a null operation identity. Every `TerminalSummary` carries that same
complete state so reconnect recovery does not depend on a live attachment or client-local color
defaults.

Every OBTF and OBPB record carries the complete Host `themeRevision` captured with its rows and
metadata. OBRW render detail nests the same revision-tagged OBPB snapshot. A client uses that
revision as a presentation barrier; it is not a content revision or a substitute for the complete
theme state returned by control and inventory.

Every attachment starts at capability revision `1`. `AttachmentCapabilities` returns the current
positive revision, and `UpdateAttachmentInputCapability` kind `24` may change only the input bit at
the exact next revision. An exact same-value replay is idempotent; a same-revision conflict, an old
revision, a gap, or a next revision that does not change the bit is rejected. Every raw or semantic
input carries the exact revision in its binary authority header, and `InputAccepted` echoes it. The
initial `AttachTerminal` request must be nonempty, while the complete current capability value may
be all false after an input-only attachment is downgraded.

`ClaimResizeLease` kind `25` carries one attachment identity and returns its complete current
`AttachmentCapabilities`. A running resize-capable attachment may claim; replay by the current
owner is idempotent. A transfer publishes the prior owner's complete `resizeLease: false` fact
before the new owner's `resizeLease: true` fact and then returns the response. Claiming changes
neither admitted capabilities nor capability revision, and `ResizeTerminal` is never an implicit
claim.

`SelectionExport` and `SelectionExportStarted` retain an explicit format field whose only valid
value is `plain`. HTML and raw VT are rejected by both language decoders and are not product wire
capabilities. Export chunks remain bounded raw bytes because framing is binary; consumers decode
the admitted plain-text bytes as UTF-8 before a platform clipboard effect.

OBPB format `0.1.0` has two intentionally separate entry points. `decodeTerminalProjectionBuffer` and `decodeTerminalRowWindow` fully validate untrusted external bytes. `viewTerminalProjectionBuffer` and `viewTerminalRowWindow` skip the per-cell rescan only for an immutable, capability-scoped local Terminal Host transport. The structural-view functions are not a faster general-purpose decoder.

`materializeTerminalProjectionFrame` is a post-validation local composition helper. It creates one bounded snapshot from matching decoded OBPB snapshots and OBRW render sources, rebases the selected rows, and can suppress the live cursor. Missing rows or mismatched Host epoch, generation, revision, theme revision, or geometry fail closed. The result carries no extensions and is only a render projection; it does not restore VT, process, PTY, selection, or command state. Callers must still use the strict decoders for untrusted bytes.

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
	if (!header.ok) return closePeer(header.code);
	if (!isExpectedAttachmentScope(header.value)) return closePeer('invalid-identifier');

	const frame = decodeTerminalFrame(envelope.value.payload);
	if (!frame.ok) return requestSnapshot(header.value.sequence, frame.code);
	if (isAlreadyCommitted(frame.value)) return acknowledgeDuplicate(frame.value);
	projectTypedColumns(frame.value.cells, frame.value.rowData);
}
```

## Fixtures

The package explicitly exports:

- `@orchbee/terminal-protocol/fixtures/canonical.json`
- `@orchbee/terminal-protocol/fixtures/control-conformance.json`
- `@orchbee/terminal-protocol/fixtures/focus-input.bin`
- `@orchbee/terminal-protocol/fixtures/malformed-frame.json`
- `@orchbee/terminal-protocol/fixtures/malformed-row-window.json`
- `@orchbee/terminal-protocol/fixtures/packed-projection-conformance.json`
- `@orchbee/terminal-protocol/fixtures/terminal-frame.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-failed.json`
- `@orchbee/terminal-protocol/fixtures/terminal-projection-buffer.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-row-window.bin`
- `@orchbee/terminal-protocol/fixtures/transport-envelope.bin`
- `@orchbee/terminal-protocol/fixtures/write-input.bin`
- `@orchbee/terminal-protocol/fixtures/key-input.bin`
- `@orchbee/terminal-protocol/fixtures/text-input.bin`
- `@orchbee/terminal-protocol/fixtures/mouse-input.bin`
- `@orchbee/terminal-protocol/fixtures/paste-auto-input.bin`
- `@orchbee/terminal-protocol/fixtures/paste-input.bin`
- `@orchbee/terminal-protocol/fixtures/selection-export-chunk.bin`
- `@orchbee/terminal-protocol/fixtures/utf8-bom-conformance.json`
- `@orchbee/terminal-protocol/fixtures/ghostty-alternate-surface.bin`
- `@orchbee/terminal-protocol/fixtures/ghostty-render-row-window.bin`
- `@orchbee/terminal-protocol/fixtures/ghostty-surface.bin`

`canonical.json` is only the language-neutral generator input for the current binary formats, so it has no compatibility version. `control-conformance.json` contains schema-produced command, reply, event, discriminated-union, failure, and complete kind/role/encoding/success-response contracts consumed by the native typed adapter. `fixtures:generate` uses the built TypeScript codecs and schemas to produce those tracked records, the canonical `TerminalFailed` control record, the protocol-`0.1.0` selection-export-chunk record, the current OBTF format-`0.1.0`, OBPB format-`0.1.0`, OBRW format-`0.1.0`, and OBKG format-`0.1.0` fixtures, their strict conformance corpus, and the frame and row-window malformed corpora. `test:fixtures` rejects every missing or byte-stale generated output. Fixture names are stable and unversioned; their contents and implementation constants carry the exact owning wire or format version.

`packed-projection-conformance.json` is the language-neutral strict-boundary oracle for OBPB format `0.1.0` and OBRW format `0.1.0`. Each named case stores complete canonical bytes, its owning strict boundary, and a contract-authored outcome; accepted cases also store all observations required by the corpus schema. Neither decoder generates the expected result. The TypeScript strict decoders consume every case, while `apps/terminal-host` resolves the declared package export, passes the bounded JSON bytes into Zig conformance, and runs every case through the matching native strict validator.

`TerminalProfile` carries explicit `defaultCursorShape` and `defaultCursorBlinking` values. Missing
fields normalize to `block` and `false` so previously persisted profile objects retain the product
default, while current encoders always emit both fields. Runtime escape sequences may override the
active cursor; Terminal Host owns applying these profile defaults again on RIS or application reset.

`ghostty-surface.bin`, `ghostty-alternate-surface.bin`, and
`ghostty-render-row-window.bin` are the exceptions to TypeScript fixture generation. Terminal
Host produces the OBTF pair from one pinned native Ghostty VT and its real projection encoder; a
separate native terminal replays the primary corpus, adds real DECSCA protected/unprotected cells,
and produces the OBRW fixture through `row_encoding.encode` and
`projection_buffer.encodeRenderSnapshot`. Host conformance regenerates and byte-compares all three.
`@orchbee/terminal` consumes the OBTF pair through both renderer backends and the packed store, and
consumes the OBRW bytes through the strict decoder, trusted structural view, and packed store. Its
contract-authored 6x8 oracle checks every row and cell column, raw string range, decoded string,
color, style flag, underline kind, width, semantic value, protected flag, and commit metadata;
wire-valid semantic mutations prove that strict admission alone cannot satisfy that oracle. The
protocol package owns the stable bytes and exports, Terminal Host owns native production, and the
terminal package owns consumer semantics. This evidence does not grant VT, lifecycle, history, or
trust-boundary authority to the protocol package.

Live OBTF snapshots and deltas may carry optional extension kind `1`, whose one-byte payload reports
close activity as `idle=1` or `active=2`. The producer omits the extension when the observation is
unknown. The extension is transient projection metadata only; lifecycle and close policy remain
consumer-owned. `NeedSnapshot` reason `activity-stale` abandons the exact in-flight frame when a
consumer has locally invalidated its close-activity observation and cannot prove that frame is
newer than the invalidating input; the replacement is a complete snapshot. This ordering proves
only that the replacement reflects the Host cache after invalidation, not that the Host accepted
the input or that the terminal process consumed it. In protocol `0.1.0`, Host-accepted non-focus
input invalidates the cached observation immediately. A later bounded current-state sample may
publish `active`, `idle`, or `unknown`, so a causal replacement may legitimately restore fresh
`idle` in the same terminal lifecycle. This changes no wire value or codec.

Optional extension kind `2` is one strictly decoded OBKG format-`0.1.0` record for the bounded
Kitty graphics projection. A resource snapshot carries Host-owned decoded gray, gray-alpha, RGB,
or RGBA bytes, the monotonic Ghostty animation tick, and current placements after Host resolves
file, temporary-file, shared-memory, virtual, relative, crop, and history relationships. A
placement-only update names the exact image revision it reuses. Signed z values select the three
renderer layers below backgrounds, between backgrounds and text, or above text. Duplicate records,
stale revisions, bad pixel lengths, missing image references, noncanonical flags, or limit
violations reject the complete OBTF frame. OBKG carries no source path, handle, shared-memory name
or mapping, encoded container, compressed stream, or unresolved Ghostty pin.

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
HTML, image, persistence, trust, or effect capability; the application owns the commit policy and
platform write.

`TerminalApplicationSignal` kind `58` is a separate event-only control record carrying the exact
terminal identity, the literal channel `orchbee.agent.v1`, a positive terminal-local sequence, and
at most 4 KiB of valid UTF-8 payload without NUL or unpaired surrogates. The record is an untrusted
transport hint decoded by Terminal Host from the private APC prefix `orchbee.agent.v1;`; the
literal channel is only a historical Host-to-Desktop discriminator, not the child wire protocol.
The protocol grants no provider identity, Agent lifecycle, attention, idle, outcome, notification,
or action semantics. An application must apply its separately owned stricter payload schema.

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

Run `pnpm --filter @orchbee/terminal-protocol fixtures:generate` only after an intentional reviewed
TypeScript-owned fixture change. For the three native surface fixtures, run
`pnpm --filter @orchbee/terminal-host surface-fixtures:generate`. Their respective conformance
checks compare tracked files without rewriting them.

## Rust render row-window binding

`decode_terminal_projection_buffer` strictly validates borrowed OBPB 0.1.0 snapshots and deltas,
including every canonical section, metadata/string range, cell/style value and row/wide/wrap
invariant. It exposes immutable borrowed rows/cells using the existing `TerminalTextStyle` colors
and flags, plus the captured default foreground/background, without expanding repeated string aliases. This is a full decoder, not the trusted local
TypeScript structural-view fast path.

`decode_terminal_render_row_window` shares the existing OBRW header owner, requires render detail,
and validates one complete OBPB snapshot and OBKG resource snapshot. Projection identity, sequence,
revision, generation, geometry and row origin must agree with the outer window. Graphics pixels
are bounded and validated without copying; resource identity, length, placement bounds and crop
references must be valid even though the CLI does not replay graphics. The original text decoder
still rejects render payloads. Neither decoder executes VT, applies state or owns read retries.
The native ANSI snapshot consumer owns presentation; the shared packed conformance corpus and
render fixture remain the cross-language oracles. No wire version or Host encoding changes.

Packed OBTF and OBPB geometry preserves unknown cell pixels as `(0, 0)`, matching the Host and
control geometry contract. Otherwise both dimensions are positive; mixed zero/positive pairs are
invalid. Grid and pixel-extent bounds remain unchanged. A terminal snapshot never invents pixel
dimensions merely to export text or styles.

## Bounded image decoding envelopes

The message registry includes read-only PNG request/result kinds 80/81 and graphics-image payload encoding (5). Its maximum payload is 32 MiB plus the 56-byte result header, independently of keyboard/paste and frame limits. Runtime producers validate the OBPI/OBPX body and physical attachment; consumers cannot treat an arbitrary decoded envelope as an authorized image. Native Host, Worker and Server own this operation’s current consumers.
