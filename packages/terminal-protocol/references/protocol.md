# Terminal Protocol Current Contract and Correctness Record

| Field           | Current authority                                                                                                                                                   |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status          | Current wire-contract and codec authority; this package implements no lifecycle state machine, PTY, IPC listener, renderer, Electron, recovery, or network behavior |
| Wire owner      | `@orchbee/terminal-protocol`                                                                                                                                        |
| Native consumer | `apps/terminal-host` consumes declared schemas and fixture exports for bounded native dispatch and Zig conformance                                                  |
| Reference scope | Exact protocol `0.0.2`; OBTF, OBPB, and OBRW format `0.0.1`; input/selection codecs; failure vocabulary; bounds; and pinned fixtures                                |
| Change rule     | A changed field, message meaning, wire byte, bound, or fixture updates this record, both language implementations, every producer/consumer, and evidence            |

## Modeling record

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A native Host and TypeScript clients need one bounded, versioned, inspectable wire contract and allocation-conscious binary codecs without sharing business state.                                                                                                                                                                                                                                                                                                                        |
| Inputs       | Raw, gateway, and external records are untrusted until their exported strict decoder or schema accepts them. The same-user capability-scoped local Host data plane may use the projection-buffer structural view: fixed metadata and section bounds are checked once, while Host-produced per-cell columns are trusted by contract and are not rescanned. A local client may also inspect only a frame header to recognize an already committed immutable duplicate. Absence is explicit. |
| Data         | Protocol versions and identities; transport envelopes; control records; terminal frames; row windows; input, projection, history, selection, configuration, and runtime-failure wire vocabulary.                                                                                                                                                                                                                                                                                          |
| Outputs      | Successfully decoded or encoded values, typed `ProtocolFailure`, bounded binary payloads, and strict control values.                                                                                                                                                                                                                                                                                                                                                                      |
| Invariants   | Accepted records use the exact current version, canonical identifiers and integers, registered message roles and encodings, valid discriminants, and declared hard bounds. Once one encoded frame is handed off for `(HostEpoch, AttachmentId, sequence)`, every byte and all header metadata for that scoped sequence are immutable and the sequence is never reused.                                                                                                                    |
| Decisions    | Exact-version handshake, message requiredness and encoding, binary layouts, safety limits, strict control shapes, and mutually exclusive outcome representation.                                                                                                                                                                                                                                                                                                                          |
| Model        | Strict normalization at external boundaries; deterministic JSON and binary codecs; and a separately named, capability-scoped structural-view path for immutable local Host projection buffers.                                                                                                                                                                                                                                                                                            |
| Dependencies | Zod validation and standard typed-array/TextEncoder APIs in TypeScript; no clock, randomness, storage, process, network, DOM, Electron, GPU, PTY, or domain transition capability.                                                                                                                                                                                                                                                                                                        |
| Ownership    | The package owns wire meaning only. Terminal Host owns lifecycle, delivery, retry, and concurrency; Desktop owns binding, recovery, close escalation, publication, and update policy.                                                                                                                                                                                                                                                                                                     |
| Verification | Named examples, stable-seed codec properties, strict-prefix exploration, generated malformed corpora, generated-file comparison, export checks, Zig decoding of canonical binary inputs, and consumer tests that distinguish immutable duplicate recognition from strict admission of a new sequence.                                                                                                                                                                                     |
| Correctness  | Codec evidence validates wire representation and bounds. Consumer-owned state models and integration tests validate whether a well-formed message is legal and correctly acted on in current state.                                                                                                                                                                                                                                                                                       |

## Identity and versioning

Every protocol identity is exactly 16 non-zero bytes. JSON renders it as exactly 32 lowercase
hexadecimal characters without separators. All-zero bytes are reserved for an absent envelope
identity and are rejected where an identity is required.

| Identity        | Owner and lifetime                                                                                                               |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `HostEpoch`     | Random per Host process; scopes every Host-owned reference                                                                       |
| `CreationId`    | Application-created durable identity; an idempotency key while its terminal remains retained by one Host epoch                   |
| `TerminalId`    | Random Host identity retained until explicit disposal                                                                            |
| `ClientId`      | Wire-supplied diagnostic broker identity; it does not grant authority independently of the active physical session               |
| `AttachmentId`  | One physical-connection-owned attachment carrying negotiated capabilities                                                        |
| `CorrelationId` | Required on every request and response after `Hello`; all-zero envelope bytes mean absence only for handshake messages or events |

The current protocol version is exactly `0.0.2`. `Hello`, `HelloAccepted`, and every transport
envelope carry that exact version. A different major, minor, or patch is rejected; this pre-release
contract contains no negotiated range, compatibility mode, or historical message shape. Packed
OBTF frame, OBPB projection-buffer, and OBRW row-window formats each have an independent current
version of `0.0.1`, and their decoders also accept only that exact format. While these contracts have
major version `0`, the repository retains only the current unpublished contract and does not keep
changelog entries for development revisions. The first public contract becomes `1.0.0`; every
later externally observable contract change bumps the owning semantic version and records the
change in [`../docs/version-history.md`](../docs/version-history.md). The Host build version is a
separate semantic version identifying the executable. `HelloAccepted.hostVersion` reports it for
diagnostics without making build and protocol releases advance together.

## Control contract

Low-rate control payloads are UTF-8 JSON bounded by the envelope and parsed with the message's Zod
schema. Unknown keys and duplicate JSON object member names (including escaped-equivalent names)
are rejected. Invalid UTF-8, invalid JSON, unpaired UTF-16 surrogates in JSON strings, duplicate
environment names, invalid identities, and out-of-range sizes fail before state mutation.
TypeScript request producers use the message-kind-selected `encodeControlRequest` boundary; callers
cannot pair a request kind with a different valid control schema. Native request consumers decode
the same payload into typed Host commands before dispatch.

The message registry also owns whether each kind may be a request, response, event, or a declared
combination; its payload encoding in each legal role; and the successful response kind for every
request. TypeScript connection code derives those facts instead of accepting them as call-site
arguments. Native production mirrors the registry and rejects a wrong role or encoding before a
record is handed off. The generated conformance corpus compares every mirrored field, not only the
numeric kind set.

`Hello` and `HelloAccepted` carry at most 64 `supportedOptionalFeatures` names, each non-empty and
at most 64 UTF-8 bytes. An optional feature name is compared by its decoded JSON string value;
Unicode code points may therefore use more than one byte while remaining within the byte bound.

`TerminalLaunch` contains a literal executable plus an argument array; it never contains a
shell-joined command string. `currentDirectory: null` means inherit the Host launch context.
Environment entries are a delta: `value: null` removes or leaves the variable absent, while `""`
sets an explicitly empty value. Names cannot contain `=`. No launch string may contain
U+0000 because executable, argument, directory, name, and value fields cross an operating-system
process boundary. Product secrets and values remain payload data and are excluded from normal
diagnostics.

Environment names are unique on the wire exactly as written: two entries whose names are identical
as written are rejected, while names that differ only by case (for example `Path` and `PATH`) remain
distinct wire names. Environment names are then target-operating-system environment identifiers, and
the Host applies the target OS's identity rule when it prepares a launch: on a POSIX target, names
are case-sensitive, so `Path` and `PATH` are distinct variables; on Windows, names are
case-insensitive, so the same two entries are duplicates and the Host rejects the `CreateTerminal`
with a typed `invalid-field` failure before any process or PTY effect. The `CreationId` launch
fingerprint uses the same normalization as the target identity rule, so equivalent folded inputs
produce a stable, platform-consistent digest.

`TerminalConfiguration` currently contains `scrollbackBytes`. Its built-in default is 10,000,000
decimal bytes, zero disables retained scrollback, and the safety maximum is 1,000,000,000 decimal
bytes. A `TerminalLaunch` carries either an explicit configuration or `null`; `null` resolves to the
Host's current terminal default at first creation. The unresolved launch value participates in the
`CreationId` digest, so an idempotent retry remains stable even if the Host default changes later.
`SetTerminalDefaults` affects only later terminals. `UpdateTerminalConfiguration` changes one
retained terminal. Increasing the scrollback budget preserves history; decreasing it evicts the
oldest rows needed to satisfy the bound. When rows are evicted, the Host advances content revision
and generation and invalidates history anchors, selection/export state, and projection bases.
Inventory summaries report the effective configuration and its independent configuration revision.

Canonical unsigned 64-bit integers inside JSON use decimal text with no sign, whitespace, exponent,
or leading zero except the value `0`. Packed binary uses little-endian unsigned integers.

## Runtime binding

Each connection sends exactly one `Hello` before any other request. `Hello` and `HelloAccepted`
negotiate `frameBytes`, `inputBytes`, and `rowWindowBytes`. Every post-handshake request carries a
non-zero `CorrelationId` and the current `HostEpoch`; every response preserves the correlation.
Events carry no correlation. Stale epochs, a repeated `Hello`, and pre-handshake operations fail
without state mutation.

Named response kinds are authoritative: `HelloAccepted`, `ProtocolRejected`,
`AttachmentCapabilities`, `InputAccepted`, `ResizeAccepted`, `ProjectionDemandAccepted`,
`TerminalDefaultsAccepted`, and `TerminalConfigurationAccepted` answer their corresponding request
kinds. Other successful operations respond with their request kind.
`TypedFailure` may answer any request and preserves its correlation.

| Operation                     | Successful payload                                                                                                                                                                                                                                                |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ListTerminals`               | Generation-scoped page of effective terminal summaries                                                                                                                                                                                                            |
| `CreateTerminal`              | `TerminalIdentity`                                                                                                                                                                                                                                                |
| `SetTerminalDefaults`         | Effective default configuration and default revision                                                                                                                                                                                                              |
| `UpdateTerminalConfiguration` | Terminal identity, effective configuration and revision, content revision/generation, and evicted-row count                                                                                                                                                       |
| `TerminateTerminal`           | Target `TerminalId`                                                                                                                                                                                                                                               |
| `ForceTerminateTerminal`      | Target `TerminalId`                                                                                                                                                                                                                                               |
| `DisposeTerminal`             | Target `TerminalId`                                                                                                                                                                                                                                               |
| `AttachTerminal`              | Attachment and terminal identities, granted input/read/resize capabilities, and resize-lease fact                                                                                                                                                                 |
| `DetachTerminal`              | Target `AttachmentId`                                                                                                                                                                                                                                             |
| raw or semantic input         | Attachment identity and encoded terminal-byte count committed to ordered ingress                                                                                                                                                                                  |
| `ResizeTerminal`              | Attachment identity, accepted canonical cell/pixel geometry, and resulting revision                                                                                                                                                                               |
| projection demand             | Attachment identity plus accepted `active` or `paused` demand                                                                                                                                                                                                     |
| frame/recovery                | OBTF format `0.0.1` snapshot/delta event, exact `FrameApplied`, typed `NeedSnapshot`, or typed `ProjectionBlocked`                                                                                                                                                |
| history/read rows             | Accepted history demand, bounded render/text `RowsSnapshot`, exact `RowsApplied`, `HistoryChanged`, and `HistoryObserved`                                                                                                                                         |
| selection/export              | Current selection state plus bounded started/chunk/acknowledgment/finished or cancellation flow                                                                                                                                                                   |
| terminal start / outcome      | Start carries terminal identity. Normal exit carries final revision and an exit-code, signal, or unknown result. Runtime failure carries final revision, typed failure, and process observation. Inventory uses mutually exclusive phase-specific summary fields. |

`RuntimeFailure.stage` is the discriminant for a primary terminal failure; `kind` is its required
coarser category, not an independent field. Its legal pairs are
`admit-terminal-reply/terminal-reply`, `apply-resize/resize`,
`dispatch-termination/process-control`,
`observe-process/process-observation`, `read-output/pty-read`,
`synchronize-worker/worker-integrity`, and `write-input/pty-write`. Every other pair is malformed.
`ProcessObservation.kind = unavailable` always carries the `observe-process/process-observation`
pair. `FailureContainment.kind = blocked` instead carries one contextual cleanup pair:
`contain-process/process-control`, `release-output/pty-read`, or
`release-process-identity/process-observation`. Cleanup pairs are malformed as primary failures,
and primary pairs are malformed in either contextual position.

`ListTerminals` accepts `{ cursor, maximumTerminals }`. `cursor` is `null` for the
first page or contains canonical decimal `inventoryGeneration` and `offset` values from the prior
response. `maximumTerminals` is `1-128`. The response repeats `inventoryGeneration`, returns at most
the requested number of summaries, and supplies either the next generation-scoped cursor or
`null`. Create and dispose advance membership generation; lifecycle, geometry, revision, and exit
changes do not. A generation mismatch returns `stale-inventory-cursor`, so one traversal never
combines two memberships. Every page remains within the 64 KiB control bound, and the protocol sets
no total terminal count limit.

`TerminateTerminal` and `ForceTerminateTerminal` are distinct request kinds. The wire permits
Desktop to send force directly; the Host-owned lifecycle decides whether the current phase accepts
the request and guarantees that force subsumes graceful intent. Neither request disposes retained
identity. Desktop owns escalation timing and waits for either `TerminalExited` or `TerminalFailed`
before requesting disposal. Missing identities and invalid current-state requests are reported as
typed failures without being reinterpreted by the codec.

The wire does not expose the Host's physical broker-session identity. The Host binds attachments to
its one active broker session and resolves each `AttachmentId` through that session. `ClientId` is
diagnostic metadata and does not merge, transfer, or independently grant authority.

Every attachment requests at least one of input, read, or resize authority. A resize lease is legal
only when resize authority is also requested and granted. Empty capability bindings and a resize
lease without resize authority are malformed at the protocol boundary.

`WriteInput` is a raw-byte payload containing exactly 16 non-zero `AttachmentId` bytes followed by
one to the negotiated `inputBytes` terminal bytes. The hard terminal-byte maximum is 1 MiB, so the
largest raw payload is 1 MiB plus 16 bytes. The attachment resolves the terminal and input
capability; `TerminalId` never appears in this payload. `InputAccepted` means the complete terminal
byte suffix was committed once to the ordered bounded Host queue. It is queue-admission-only and
does not promise the bytes have reached the terminal writer, so terminal-generated replies may be
delivered ahead of accepted client bytes under the Host's bounded reply-priority policy. A broken
connection before that
response leaves the outcome unknown and never authorizes replay.

TypeScript producers use `encodeInputRequest`; the message kind selects the raw or semantic binary
codec. Native consumers route the same registered kinds through one semantic-input decoder, so
neither side maintains a business-layer kind-to-format table.

`KeyInput`, `TextInput`, `MouseInput`, `FocusInput`, and `PasteInput` are bounded packed payloads
prefixed by the same attachment identity. They carry browser-normalized semantic intent, not DOM
objects or preselected escape sequences. A mouse press or release always identifies its button.
Mouse motion with no pressed buttons carries a null button; motion with pressed buttons identifies
one active button that is present in `pressedButtons`, whose bit set describes the complete current
pressed-button state. Wheel reports remain button presses and need not describe a held button.
Host dispatch reserves ingress order and snapshots authoritative Ghostty encoder modes before
encoding, so output coalescing or a frame in flight
cannot make key, mouse, paste, or focus encoding stale. The negotiated `inputBytes` limit applies
to the encoded terminal-byte output, not the packed semantic payload; `acceptedBytes` reports that
output length and may be zero when the current terminal mode intentionally emits nothing. The Host
must preflight zero-output and paste bounds before queue admission, then charge only the encoded
bytes to the ordered ingress budget.

## Message registry

| Range   | Group                   | Kinds                                                                                                                                                                                                                                                                                                        |
| ------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `1-5`   | Connection              | `Hello=1`, `HelloAccepted=2`, `ProtocolRejected=3`, `Ping=4`, `TypedFailure=5`                                                                                                                                                                                                                               |
| `10-19` | Inventory and lifecycle | `ListTerminals=10`, `CreateTerminal=11`, `TerminateTerminal=12`, `DisposeTerminal=13`, `TerminalStarted=14`, `TerminalExited=15`, `ForceTerminateTerminal=18`, `TerminalFailed=19`                                                                                                                           |
| `20-22` | Attachment              | `AttachTerminal=20`, `DetachTerminal=21`, `AttachmentCapabilities=22`                                                                                                                                                                                                                                        |
| `30-38` | Input and size          | `WriteInput=30`, `InputAccepted=31`, `ResizeTerminal=32`, `ResizeAccepted=33`, `KeyInput=34`, `TextInput=35`, `MouseInput=36`, `FocusInput=37`, `PasteInput=38`                                                                                                                                              |
| `41-54` | Projection and history  | `SnapshotFrame=41`, `DeltaFrame=42`, `FrameApplied=43`, `NeedSnapshot=44`, `ReadRows=45`, `RowsSnapshot=46`, `SetProjectionDemand=47`, `ProjectionDemandAccepted=48`, `ProjectionBlocked=49`, `RowsApplied=50`, `SetHistoryDemand=51`, `HistoryDemandAccepted=52`, `HistoryChanged=53`, `HistoryObserved=54` |
| `60-68` | Selection and export    | `SelectionGesture=60`, `SelectionCommand=61`, `SelectionChanged=62`, `SelectionExport=63`, `SelectionExportStarted=64`, `SelectionExportChunk=65`, `SelectionExportChunkApplied=66`, `SelectionExportFinished=67`, `SelectionExportCancel=68`                                                                |
| `70-73` | Terminal configuration  | `SetTerminalDefaults=70`, `TerminalDefaultsAccepted=71`, `UpdateTerminalConfiguration=72`, `TerminalConfigurationAccepted=73`                                                                                                                                                                                |

Unlisted numbers are unassigned, including `16`, `17`, and `23`. Title and current-directory
changes travel in terminal frames; resize-lease authority is returned by attachment admission and
is not a separately implemented event.

`MESSAGE_CONTRACTS` is the executable TypeScript registry behind this table. Its native mirror is
checked field-for-field through `control-conformance-v0.0.2.json`. Adding or changing a kind therefore
requires one atomic protocol change to the registry, native mirror, codecs, fixture, and consumers.

Requests and responses use the response flag and `CorrelationId`; the named pairs in
[runtime binding](#runtime-binding) use their registered response kind and other
responses reuse the request kind. Events use the event flag. An unknown kind is rejected unless
the optional-message flag is present, in which case the bounded payload may be skipped without
state change.

## Current transport envelope

Every record begins with this fixed 64-byte little-endian header:

| Offset | Bytes | Field                                                                           |
| ------ | ----: | ------------------------------------------------------------------------------- |
| `0`    |     4 | ASCII `OBTP`                                                                    |
| `4`    |     2 | Header length, exactly `64`                                                     |
| `6`    |     2 | Flags                                                                           |
| `8`    |     4 | Total record bytes                                                              |
| `12`   |     4 | Payload bytes, exactly total minus header                                       |
| `16`   |     2 | Protocol major                                                                  |
| `18`   |     2 | Protocol minor                                                                  |
| `20`   |     2 | Message kind                                                                    |
| `22`   |     1 | Encoding: control JSON `1`, raw bytes `2`, terminal frame `3`, row window `4`   |
| `23`   |     1 | Reserved zero                                                                   |
| `24`   |    16 | Correlation identity; all zero means absent                                     |
| `40`   |    16 | Host epoch; all zero is allowed only before an epoch-bearing semantic operation |
| `56`   |     2 | Protocol patch                                                                  |
| `58`   |     6 | Reserved zero                                                                   |
| `64`   |     N | Payload                                                                         |

Flag bit `0` marks an optional message, bit `1` a response, and bit `2` an event. Bits `3-7` are
the optional flag partition and unknown values are reported but ignored. Bits `8-15` are the
required partition and any unknown value closes decoding with `unknown-required-flag`. Event and
response cannot both be set.

## Current packed terminal frame

The frame begins with a fixed 160-byte header:

| Offset | Bytes | Field                                                                                              |
| ------ | ----: | -------------------------------------------------------------------------------------------------- |
| `0`    |     4 | ASCII `OBTF`                                                                                       |
| `4`    |     1 | Format major `0`                                                                                   |
| `5`    |     1 | Format minor `0`                                                                                   |
| `6`    |     1 | Snapshot `1` or delta `2`                                                                          |
| `7`    |     1 | Frame flags; currently zero                                                                        |
| `8`    |     4 | Total frame bytes                                                                                  |
| `12`   |     2 | Header length `160`                                                                                |
| `14`   |     2 | Reserved zero                                                                                      |
| `16`   |    16 | Host epoch                                                                                         |
| `32`   |    16 | Attachment identity                                                                                |
| `48`   |     8 | Attachment-local frame sequence                                                                    |
| `56`   |     8 | Base revision; all ones means no base                                                              |
| `64`   |     8 | Captured canonical terminal revision                                                               |
| `72`   |     8 | Projection generation                                                                              |
| `80`   |     8 | Retained scrollback row count                                                                      |
| `88`   |     2 | Columns                                                                                            |
| `90`   |     2 | Rows                                                                                               |
| `92`   |     2 | Cursor column                                                                                      |
| `94`   |     2 | Cursor row                                                                                         |
| `96`   |     1 | Cursor shape: block `1`, bar `2`, underline `3`                                                    |
| `97`   |     1 | Cursor flags: visible bit `0`, blinking bit `1`, wide-tail bit `2`                                 |
| `98`   |     1 | Active screen: primary `1` or alternate `2`                                                        |
| `99`   |     1 | Reserved zero                                                                                      |
| `100`  |     4 | Application-keypad/cursor, paste, focus, mouse, modifyOtherKeys, and Kitty interaction flags       |
| `104`  |    12 | Resolved default foreground, background, and cursor RGB; cursor default uses its declared sentinel |
| `116`  |     8 | Title and current-directory string indices; all ones means absent                                  |
| `124`  |     4 | String count                                                                                       |
| `128`  |     2 | Style count                                                                                        |
| `130`  |     2 | Row-record count                                                                                   |
| `132`  |     2 | Extension count                                                                                    |
| `134`  |     2 | Reserved zero                                                                                      |
| `136`  |     4 | Cell-pixel width and height as two `u16` values                                                    |
| `140`  |     4 | Total encoded cell count                                                                           |
| `144`  |     1 | Format patch `1`                                                                                   |
| `145`  |    15 | Reserved zero                                                                                      |

Variable sections follow in this exact order:

1. Each string is `u32 byteLength` followed by strict UTF-8 bytes.
2. Each 20-byte style contains foreground, background, and underline color codes (`u32`), hyperlink
   string index `u32`, explicit style flags `u16`, underline kind `u8`, and reserved zero `u8`.
   RGB occupies the low 24 bits; declared default and none sentinels are field-specific.
3. Each eight-byte row header contains row index `u16`, wrap flags `u8`, OSC 133 prompt class `u8`,
   cell count `u16`, and reserved zero `u16`. Exactly `columns` eight-byte cells follow: grapheme
   string index `u32`, style index `u16`, packed width/OSC 133 semantic/protected flags `u8`, and
   reserved zero `u8`.
4. Each extension has kind `u16`, flags `u16`, byte length `u32`, and bytes. Flag bit `15` means
   required; an unknown required extension is rejected. Unknown optional extensions remain bounded
   and are returned as opaque data.

A snapshot has no base and contains every visible row in canonical order. A delta names the exact
acknowledged base and replaces complete, unique dirty rows in increasing row order; it never carries
partial cells. Dimension, active-screen, or generation changes require a snapshot. The TypeScript
decoder first validates and scans all tables and rows, then allocates exact typed columns and reads
cells without constructing a JavaScript object per cell. The projected store stages a full snapshot
or every delta row before mutating the committed grid. Frame sequences are monotonically emitted,
but an abandoned frame may leave a gap in the committed client sequence. A delta must be the exact
next sequence and name the exact committed base. A complete snapshot may use any sequence greater
than the last committed sequence and atomically re-establish the client base across such a gap.
The fixed-header inspector validates only the bounded header and never admits cell, string, style,
row, or extension data. A capability-scoped local client may use that inspector before full decode
only to acknowledge a sequence it has already committed. This is sound because the Host delivery
owner makes the complete encoded record immutable when it hands off that scoped sequence. Every
higher OBTF format-`0.0.1` sequence, and every frame arriving outside that trusted local delivery path,
still requires the full decoder before any payload data is exposed or committed. OBPB format `0.0.1`
projection buffers provide separately named strict-decode and local structural-view boundaries
below. A repeated sequence with different header commit metadata is a protocol-state conflict and
requires recovery; consumers do not hash or rescan the duplicate payload to defend against a
producer violation of the immutable-sequence contract.
For every pair of adjacent rows present in one frame, the prior row's wrap flag equals the next
row's wrap-continuation flag. A spacer-head is valid only in the final column of a wrapped row; if
the following row is present, it begins with the corresponding wide cell. These relationships are
validated before a frame is exposed or committed.

## Current projection buffer

OBPB format `0.0.1` is the packed Host-to-render data plane. `decodeTerminalProjectionBuffer` is the
default boundary and strictly scans row, cell, color, string-range, UTF-8, and wide-cell invariants
before exposing typed views. A gateway, network peer, plugin, or persisted artifact must use that
strict boundary. `viewTerminalProjectionBuffer` is reserved for the immutable, same-user,
capability-scoped local Host path. It checks the fixed header, identities, counts, canonical section
directory, alignment and zero padding, total length, and the two small metadata strings, then creates
typed-array views over the same bytes. It deliberately does not walk every cell to revalidate values
already produced from canonical Ghostty state; cross-language conformance and producer tests
establish those column invariants.

The fixed header is 320 bytes. Bytes `0..159` contain `OBPB`, version `0.0.1`, snapshot/delta kind,
total/header lengths, columns, grid rows, included rows/cells, Host and attachment identities,
sequence/base/revision/generation, scrollback origin, cursor and pixel geometry, interaction flags,
default colors, nullable title/current-directory blob ranges, and total string bytes. Bytes
`160..287` contain sixteen fixed directory entries of `{ u32 offset, u32 byteLength }`; bytes
`288..319` are reserved zero. Nullable metadata uses offset `0xffffffff`; all other string offsets
are relative to the final string-byte section.

All integers are little-endian. The fixed header is:

| Offset | Bytes | Field                                            |
| -----: | ----: | ------------------------------------------------ |
|      0 |     4 | Magic `OBPB`                                     |
|      4 |     1 | Format major `0`                                 |
|      5 |     1 | Format minor `0`                                 |
|      6 |     1 | Kind: snapshot `1`, delta `2`                    |
|      7 |     1 | Optional flags; bit `7` is required/reserved     |
|      8 |     4 | Total buffer bytes                               |
|     12 |     2 | Header bytes `320`                               |
|     14 |     2 | Grid columns                                     |
|     16 |     2 | Grid rows                                        |
|     18 |     2 | Included row count                               |
|     20 |     4 | Included cell count                              |
|     24 |    16 | Host epoch                                       |
|     40 |    16 | Attachment ID                                    |
|     56 |     8 | Sequence                                         |
|     64 |     8 | Base revision; `0xffffffffffffffff` means absent |
|     72 |     8 | Revision                                         |
|     80 |     8 | Generation                                       |
|     88 |     8 | Scrollback-row origin                            |
|     96 |     2 | Cursor column                                    |
|     98 |     2 | Cursor row                                       |
|    100 |     2 | Cell pixel width                                 |
|    102 |     2 | Cell pixel height                                |
|    104 |     1 | Cursor shape                                     |
|    105 |     1 | Cursor flags                                     |
|    106 |     1 | Active screen                                    |
|    107 |     1 | Reserved zero                                    |
|    108 |     4 | Interaction flags                                |
|    112 |     4 | Default foreground color                         |
|    116 |     4 | Default background color                         |
|    120 |     4 | Default cursor color                             |
|    124 |     4 | Nullable title byte offset                       |
|    128 |     4 | Title byte length                                |
|    132 |     4 | Nullable current-directory byte offset           |
|    136 |     4 | Current-directory byte length                    |
|    140 |     4 | String-section byte length                       |
|    144 |     1 | Format patch `1`                                 |
|    145 |    15 | Reserved zero                                    |
|    160 |   128 | Sixteen directory entries                        |
|    288 |    32 | Reserved zero                                    |

The directory slots and element types are fixed:

| Slot | Column                 | Element |
| ---: | ---------------------- | ------: |
|    0 | included row indices   |   `u16` |
|    1 | row flags              |    `u8` |
|    2 | row prompt classes     |    `u8` |
|    3 | grapheme byte offsets  |   `u32` |
|    4 | grapheme byte lengths  |   `u16` |
|    5 | hyperlink byte offsets |   `u32` |
|    6 | hyperlink byte lengths |   `u16` |
|    7 | foreground colors      |   `u32` |
|    8 | background colors      |   `u32` |
|    9 | underline colors       |   `u32` |
|   10 | style flags            |   `u16` |
|   11 | underline kinds        |    `u8` |
|   12 | cell widths            |    `u8` |
|   13 | semantic classes       |    `u8` |
|   14 | protected-cell flags   |    `u8` |
|   15 | UTF-8 string bytes     |    `u8` |

Sections use their natural alignment and canonical slot order, with zero alignment padding. A
snapshot contains every row in canonical order. A delta contains complete dirty rows in increasing
order and names the exact acknowledged base. Snapshot consumers retain the immutable buffer;
delta consumers retain only buffers still referenced by dirty row ranges. Shared memory is not part
of this version.

## Current packed terminal row-window

`ReadRows` is scoped by attachment, revision, generation, positive request
sequence, requested row count, negotiated byte maximum, detail (`render` or `text`), purpose
(`viewport`, `search`, or `buffer`), and one origin:

- `absolute` names a retained absolute row.
- `oldest` names an unsigned offset from the oldest retained row.
- `tail` names how many rows remain below the requested viewport.
- `committed` applies a bounded signed delta to the attachment's last applied anchor.

The Host resolves the origin once and returns a contiguous `RowsSnapshot`. At most one row request
is in flight per attachment. Request sequences strictly increase for the attachment lifetime; a
request that fails before installation does not consume its sequence. A response is only a pending
anchor; the matching `RowsApplied` promotes it to the committed anchor, so a delayed acknowledgement
from an earlier row request cannot promote a later anchor. Resize invalidates pending generation-pinned row captures,
but a committed tracked anchor may follow a valid Ghostty reflow pin. Screen generation change, detach, or Ghostty page
eviction invalidates affected anchors. `HistoryChanged` coalesces while one event awaits the exact
`HistoryObserved` revision/generation acknowledgment.

Every row window begins with this fixed 112-byte little-endian header:

| Offset | Bytes | Field                                                                 |
| ------ | ----: | --------------------------------------------------------------------- |
| `0`    |     4 | ASCII `OBRW`                                                          |
| `4`    |     1 | Format major `0`                                                      |
| `5`    |     1 | Format minor `0`                                                      |
| `6`    |     1 | Detail: render `1` or text `2`                                        |
| `7`    |     1 | Optional flags; required bit `7` is currently zero                    |
| `8`    |     4 | Total row-window bytes                                                |
| `12`   |     2 | Header length, exactly `112`                                          |
| `14`   |     2 | Columns                                                               |
| `16`   |    16 | Host epoch                                                            |
| `32`   |    16 | Attachment identity                                                   |
| `48`   |     8 | Positive attachment-local request sequence                            |
| `56`   |     8 | Captured canonical revision                                           |
| `64`   |     8 | Captured screen generation                                            |
| `72`   |     8 | Total retained rows                                                   |
| `80`   |     8 | Absolute first row                                                    |
| `88`   |     2 | Contiguous row count                                                  |
| `90`   |     2 | Reserved zero                                                         |
| `92`   |     4 | Cell count for render detail or logical mapping count for text detail |
| `96`   |     4 | Body bytes                                                            |
| `100`  |     1 | Format patch `1`                                                      |
| `101`  |    11 | Reserved zero                                                         |
| `112`  |     N | OBPB format `0.0.1` projection buffer or text rows                    |

Render detail nests one OBPB format `0.0.1` snapshot projection buffer whose identity, sequence, revision, generation,
columns, rows, and scrollback-row origin agree with the outer window. Text detail stores each row as a 12-byte header followed by
strict UTF-8 text and, when required, ordered 12-byte UTF-8/UTF-16/column/width mappings. Row flag
bit `2` (`0x04`) requires zero serialized mappings and declares that
every text byte is narrow ASCII whose UTF-8 byte offset, UTF-16 code-unit offset, and terminal
column all equal its character index. The header entry count still includes these logical mappings,
so the 262,144-entry bound is unchanged. Unicode, wide-cell, combining-grapheme, and non-identity
rows retain explicit mappings. A non-empty explicit row has at least one mapping beginning at byte
and UTF-16 offset zero. Every mapping begins on one UTF-8 codepoint boundary, names the matching
UTF-16 position, fits inside the column count, and does not overlap the preceding mapped cell.
Each mapping delimits one logical cell, so the one UTF-8 string bound (16 KiB) applies per mapped
cell: every byte span from a mapping start to the next mapping start, and the final span to row
text end, must not exceed 16,384 bytes. A row whose mapped-cell spans are each bounded may still
aggregate above 16 KiB. Adjacent row wrap flags agree. Text row, logical-mapping, byte, UTF-8,
ordering, identity, and per-cell span relationships are strictly validated. The exported row-window
decoder uses the strict projection buffer decoder. The separately named local row-window view uses
the projection-buffer structural view and does not perform a second per-cell semantic scan.

## Host-owned selection and export

Selection gestures and commands always carry the attachment's current revision and
generation. Endpoints are absolute row/column pairs; active state includes only endpoints,
rectangle mode, and an optional 4 KiB preview. `none` and `evicted` are distinct so a client cannot
silently copy replacement content after scrollback loss or a screen change.

`SelectionCommand.set-range` carries explicit inclusive `start` and `end` cell endpoints plus
rectangle mode. It is the non-pointer path for assistive technology and other programmatic
selection; it does not synthesize click counts, drag thresholds, or pointer gesture state.

`SelectionExport` selects plain, HTML, or VT output and an attachment-local export sequence that
strictly increases for the attachment lifetime; an export that fails before installation does not
consume its sequence. The Host first returns `SelectionExportStarted`, then emits at most one raw
`SelectionExportChunk` in flight. Each
chunk contains the attachment identity, export sequence, monotonic `u32` chunk sequence, final bit,
and at most 1 MiB of bytes. `SelectionExportChunkApplied` advances only the matching export, so a
delayed acknowledgement from an earlier export cannot advance a later stream. Completion or explicit
cancellation releases all export storage. Total formatted output is capped at 16 MiB. Clipboard
write and other platform effects remain outside this protocol package and the Host.

## Hard bounds

The handshake may select smaller transport payload bounds. It never raises these package hard limits.

| Input or retained unit                |       Hard maximum |
| ------------------------------------- | -----------------: |
| Control JSON                          |             64 KiB |
| Terminals in one inventory page       |                128 |
| One raw, text, or paste input body    |              1 MiB |
| One key text                          |              4 KiB |
| `WriteInput` raw payload              |       1 MiB + 16 B |
| Largest semantic input payload        |       1 MiB + 32 B |
| Pending ordered input per terminal    |              4 MiB |
| Scrollback configuration per terminal |    1,000,000,000 B |
| PTY read batch                        |            256 KiB |
| Packed frame                          |              8 MiB |
| Transport record                      |       8 MiB + 64 B |
| Row-window payload                    |              2 MiB |
| Search query at a client boundary     |             16 KiB |
| Selection preview                     |              4 KiB |
| Selection export / one chunk          |     16 MiB / 1 MiB |
| One UTF-8 string                      |             16 KiB |
| Strings / styles / row records        | 4096 / 1024 / 4096 |

`One UTF-8 string` bounds each serialized string and, for text row windows, each logical cell's
byte span: the bytes from a mapping start to the next mapping start, and the final span to row
text end, must fit in 16,384 bytes. Aggregate row text is not capped by this per-cell bound.
| Cells / frame extensions              |        262144 / 64 |
| Total extension bytes                 |              1 MiB |
| Grid columns / rows                   |        4096 / 4096 |
| Cell-pixel width / height             |        4096 / 4096 |
| Total terminal pixel width / height   |      65535 / 65535 |

Counts and lengths are checked before dependent allocation. Integer overflow, inconsistent lengths,
truncation, invalid UTF-8, bad table references, duplicate or out-of-grid rows, and unknown required flags are
typed failures.

Terminal, attachment, and connection totals have no protocol or product hard maximum. Allocation,
process, PTY, identifier-space, or operating-system exhaustion rejects the current operation; it
does not authorize truncating inventory or deleting retained state.

## Consumer-owned behavior

A valid protocol value is not by itself a legal domain transition. The state owner decides whether
the value is admissible, performs effects, commits state, and determines retry behavior:

| Consumer      | Authoritative behavior and evidence                                                                                                                                                                                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Terminal Host | Terminal membership and lifecycle, process/PTY ownership, runtime-failure containment, attachment capability state, ordered input, projection/history/selection delivery, batch commit, and Host shutdown. See [the Host runtime model](../../../apps/terminal-host/docs/runtime-model.md). |
| Desktop       | Product binding, durable intent, reconnection, terminal-close escalation, outcome publication, update activation, and renderer/gateway coordination. See Desktop-owned guides, models, and tests.                                                                                           |
| Renderer      | Presentation and application of already validated projection/history values; it does not acquire Host lifecycle authority.                                                                                                                                                                  |

Consumer models may use protocol message names and schemas as refinement points, but they live with
the consumer that owns the modeled state. This package's evidence is intentionally limited to wire
shape, exact-version handling, safety bounds, deterministic codecs, malformed-input rejection,
generated fixture integrity, and cross-language conformance. It does not claim process, delivery,
recovery, scheduling, or product-policy correctness.
