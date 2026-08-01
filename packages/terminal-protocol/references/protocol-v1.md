# Terminal Protocol v1 Contract and Correctness Record

| Field           | Current authority                                                                                                                                                           |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status          | Current protocol and correctness authority through Slice 4; this package implements no PTY, IPC listener, renderer, Electron, packaging, or network behavior                |
| Semantic owner  | `@orchbee/terminal-protocol`                                                                                                                                                |
| Native consumer | `apps/terminal-host` consumes declared fixture exports and protocol semantics for Zig conformance and bounded native dispatch                                               |
| Reference scope | Compatible protocol `1.0`/`1.1` semantics, packed frame format `2.0`, semantic-input codecs, and pinned canonical fixtures; no Ghostty GUI or performance-parity claim      |
| Change rule     | A changed field, message meaning, wire byte, bound, transition, invariant, fairness assumption, or fixture updates this record, both language implementations, and evidence |

## Modeling record

| Field        | Contract                                                                                                                                                                                                                                                                                     |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A native Host and TypeScript clients need one bounded, versioned, inspectable control contract and one allocation-conscious projection codec without sharing product topology or transport implementation.                                                                                   |
| Inputs       | Every peer record, control JSON value, identifier, length, count, flag, UTF-8 string, lifecycle state, and lifecycle event is untrusted until the exported decoder or schema accepts it. Absence uses explicit `null` or a named wire sentinel.                                              |
| Data         | Protocol versions, five principal 128-bit identities, correlation identity, transport envelope, terminal frame tables, Host state, terminal state, connection state, attachment state, revision, sequence, capability, resize lease, pending input, and update intent.                       |
| Outputs      | Successful decoded/encoded values; typed `ProtocolFailure`; lifecycle outcome `applied`, `idempotent`, or `rejected`; and serializable commands for spawn, terminate, input write, PTY resize, frame emission, connection close, or Host stop.                                               |
| Invariants   | I1-I15 in [Traceability](#traceability); every accepted lifecycle transition revalidates its complete state schema before returning.                                                                                                                                                         |
| Decisions    | Protocol overlap; message requiredness and encoding; attachment-prefixed raw and semantic input; full-row snapshot/delta projection; demand, capture, generation, acknowledgment, input capacity, completion reorder, and retained-terminal upgrade gates.                                   |
| Model        | Strict boundary normalization, deterministic binary codecs, and `transitionProtocolLifecycle(state, event)` as the principal abstract operation.                                                                                                                                             |
| Transitions  | Named serializable events with explicit guards, outcomes, state, and commands; rejected, stale, repeated, and reordered cases preserve state unless their documented idempotent transition applies.                                                                                          |
| Dependencies | Zod validation and standard typed-array/TextEncoder APIs in TypeScript; no clock, randomness, storage, process, network, DOM, Electron, GPU, or PTY capability. TLA+ and Zig are pinned repository evidence toolchains.                                                                      |
| Ownership    | The package owns protocol meaning and abstract transition semantics. Host owns terminal processes and emulation; Desktop owns transport adapters, capabilities, recovery binding, and updates; renderer owns projection and interaction; future gateway owns networking and remote security. |
| Verification | Named example tests, stable-seed properties, strict-prefix exploration, generated format-2 malformed corpus, generated-file comparison, two TLC configurations, export checks, and Zig decoding of every canonical binary input.                                                             |
| Correctness  | TLA+ checks the declared finite abstraction; property tests sample configured generators; Zig checks a native decoder against current fixtures. Platform PTY and IPC behavior requires Host-owned evidence; this package does not prove renderer, installer, Host crash, or remote security. |

## Identity and versioning

Every protocol identity is exactly 16 non-zero bytes. JSON renders it as exactly 32 lowercase
hexadecimal characters without separators. All-zero bytes are reserved for an absent envelope
identity and are rejected where an identity is required.

| Identity        | Owner and lifetime                                                                                                               |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `HostEpoch`     | Random per Host process; scopes every Host-owned reference                                                                       |
| `CreationId`    | Application-created durable idempotency key; opaque to Host product placement                                                    |
| `TerminalId`    | Random Host identity retained until explicit disposal                                                                            |
| `ClientId`      | One trusted local protocol connection                                                                                            |
| `AttachmentId`  | One client attachment carrying negotiated capabilities                                                                           |
| `CorrelationId` | Required on every request and response after `Hello`; all-zero envelope bytes mean absence only for handshake messages or events |

The current protocol version is `1.1`. A `ProtocolVersionRange` supports one major and an inclusive
minor interval. Negotiation rejects different majors or disjoint minor intervals and otherwise
selects the highest shared minor. Protocol `1.0` lifecycle, raw input, and non-readable attachment
behavior remains usable. A readable attachment, pixel-aware launch/resize, projection demand, or
semantic input requires negotiated `1.1`. Packed frame format has its own version: the current and
only shipped format is `2.0`; the unshipped fixture-only format 1 has no compatibility decoder.

## Control contract

Low-rate control payloads are UTF-8 JSON bounded by the envelope and parsed with the message's Zod
schema. Unknown keys are rejected by strict objects. Invalid UTF-8, invalid JSON, duplicate
environment names, invalid identities, and out-of-range sizes fail before state mutation.

`TerminalLaunch` contains a literal executable plus an argument array; it never contains a
shell-joined command string. `currentDirectory: null` means inherit the Host launch context.
Environment entries are a delta: `value: null` removes or leaves the variable absent, while `""`
sets an explicitly empty value. Names are unique and cannot contain `=`. No launch string may contain
U+0000 because executable, argument, directory, name, and value fields cross an operating-system
process boundary. Product secrets and values remain payload data and are excluded from normal
diagnostics.

Canonical unsigned 64-bit integers inside JSON use decimal text with no sign, whitespace, exponent,
or leading zero except the value `0`. Packed binary uses little-endian unsigned integers.

## Runtime binding and protocol 1.1 additions

Each connection sends exactly one `Hello` before any other request. `Hello` and `HelloAccepted`
negotiate `frameBytes`, `inputBytes`, and `rowWindowBytes`. Every post-handshake request carries a
non-zero `CorrelationId` and the current `HostEpoch`; every response preserves the correlation.
Events carry no correlation. Stale epochs, a repeated `Hello`, and pre-handshake operations fail
without state mutation.

Named response kinds are authoritative: `HelloAccepted`, `ProtocolRejected`,
`AttachmentCapabilities`, `InputAccepted`, `ResizeAccepted`, and `ProjectionDemandAccepted` answer
their corresponding request kinds. Other successful operations respond with their request kind.
`TypedFailure` may answer any request and preserves its correlation.

| Operation             | Successful payload                                                                                                                  |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `ListTerminals`       | At most one terminal summary: identity, phase, dimensions, canonical revision, termination-requested fact, and optional exit result |
| `CreateTerminal`      | `TerminalIdentity`                                                                                                                  |
| `TerminateTerminal`   | Target `TerminalId`                                                                                                                 |
| `DisposeTerminal`     | Target `TerminalId`                                                                                                                 |
| `AttachTerminal`      | Attachment and terminal identities, granted input/read/resize capabilities, and resize-lease fact                                   |
| `DetachTerminal`      | Target `AttachmentId`                                                                                                               |
| raw or semantic input | Attachment identity and encoded terminal-byte count committed to ordered ingress                                                    |
| `ResizeTerminal`      | Attachment identity, accepted canonical cell/pixel geometry, and resulting revision                                                 |
| projection demand     | Attachment identity plus accepted `active` or `paused` demand                                                                       |
| frame/recovery        | Format-2 snapshot/delta event, exact `FrameApplied`, typed `NeedSnapshot`, or typed `ProjectionBlocked`                             |
| terminal start / exit | Terminal identity; exit additionally carries the final revision and exit-code, signal, or unknown platform result                   |

`WriteInput` is a raw-byte payload containing exactly 16 non-zero `AttachmentId` bytes followed by
one to the negotiated `inputBytes` terminal bytes. The hard terminal-byte maximum is 1 MiB, so the
largest raw payload is 1 MiB plus 16 bytes. The attachment resolves the terminal and input
capability; `TerminalId` never appears in this payload. `InputAccepted` means the complete terminal
byte suffix was committed once to the ordered bounded Host queue. A broken connection before that
response leaves the outcome unknown and never authorizes replay.

`KeyInput`, `TextInput`, `MouseInput`, `FocusInput`, and `PasteInput` are bounded packed payloads
prefixed by the same attachment identity. They carry browser-normalized semantic intent, not DOM
objects or preselected escape sequences. Host dispatch reserves ingress order and snapshots
authoritative Ghostty encoder modes before encoding, so output coalescing or a frame in flight
cannot make key, mouse, paste, or focus encoding stale.

## Message registry

| Range   | Group                   | Kinds                                                                                                                                                                                        |
| ------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `1-5`   | Connection              | `Hello=1`, `HelloAccepted=2`, `ProtocolRejected=3`, `Ping=4`, `TypedFailure=5`                                                                                                               |
| `10-17` | Inventory and lifecycle | `ListTerminals=10`, `CreateTerminal=11`, `TerminateTerminal=12`, `DisposeTerminal=13`, `TerminalStarted=14`, `TerminalExited=15`, `TitleChanged=16`, `CurrentDirectoryChanged=17`            |
| `20-23` | Attachment              | `AttachTerminal=20`, `DetachTerminal=21`, `AttachmentCapabilities=22`, `ResizeLeaseChanged=23`                                                                                               |
| `30-38` | Input and size          | `WriteInput=30`, `InputAccepted=31`, `ResizeTerminal=32`, `ResizeAccepted=33`, `KeyInput=34`, `TextInput=35`, `MouseInput=36`, `FocusInput=37`, `PasteInput=38`                              |
| `41-49` | Projection              | `SnapshotFrame=41`, `DeltaFrame=42`, `FrameApplied=43`, `NeedSnapshot=44`, `ReadRows=45`, `RowsSnapshot=46`, `SetProjectionDemand=47`, `ProjectionDemandAccepted=48`, `ProjectionBlocked=49` |

Requests and responses use the response flag and `CorrelationId`; the named pairs in
[runtime binding](#runtime-binding-and-protocol-11-additions) use their registered response kind and other
responses reuse the request kind. Events use the event flag. An unknown kind is rejected unless
the optional-message flag is present, in which case the bounded payload may be skipped without
state change.

## Transport envelope v1

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
| `22`   |     1 | Encoding: control JSON `1`, raw bytes `2`, terminal frame `3`                   |
| `23`   |     1 | Reserved zero                                                                   |
| `24`   |    16 | Correlation identity; all zero means absent                                     |
| `40`   |    16 | Host epoch; all zero is allowed only before an epoch-bearing semantic operation |
| `56`   |     8 | Reserved zero                                                                   |
| `64`   |     N | Payload                                                                         |

Flag bit `0` marks an optional message, bit `1` a response, and bit `2` an event. Bits `3-7` are
the optional flag partition and unknown values are reported but ignored. Bits `8-15` are the
required partition and any unknown value closes decoding with `unknown-required-flag`. Event and
response cannot both be set.

## Packed terminal frame format 2

The frame begins with a fixed 160-byte header:

| Offset | Bytes | Field                                                                                              |
| ------ | ----: | -------------------------------------------------------------------------------------------------- |
| `0`    |     4 | ASCII `OBTF`                                                                                       |
| `4`    |     1 | Format major `2`                                                                                   |
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
| `144`  |    16 | Reserved zero                                                                                      |

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
or every delta row before mutating the committed grid.

## Hard bounds

Negotiation may select a smaller value. It never raises these package hard limits.

| Input or retained unit              |       Hard maximum |
| ----------------------------------- | -----------------: |
| Control JSON                        |             64 KiB |
| One raw, text, or paste input body  |              1 MiB |
| One key text                        |              4 KiB |
| `WriteInput` raw payload            |       1 MiB + 16 B |
| Largest semantic input payload      |       1 MiB + 32 B |
| Pending ordered input per terminal  |              4 MiB |
| PTY read batch                      |            256 KiB |
| Packed frame                        |              8 MiB |
| Transport record                    |       8 MiB + 64 B |
| Row-window payload                  |              2 MiB |
| One UTF-8 string                    |             16 KiB |
| Strings / styles / row records      | 4096 / 1024 / 4096 |
| Cells / frame extensions            |        262144 / 64 |
| Total extension bytes               |              1 MiB |
| Grid columns / rows                 |        4096 / 4096 |
| Cell-pixel width / height           |        4096 / 4096 |
| Total terminal pixel width / height |      65535 / 65535 |

Counts and lengths are checked before dependent allocation. Integer overflow, inconsistent lengths,
truncation, invalid UTF-8, bad table references, duplicate or out-of-grid rows, and unknown required flags are
typed failures.

## Abstract lifecycle transitions

| Event group                     | Guard and abstract result                                                                                                                                                                         |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Launch / endpoint ready         | Only absent launches; starting becomes serving for the same epoch. Concurrent launch is idempotent and cannot publish a second authority.                                                         |
| Connect / disconnect            | Only serving accepts a client. Disconnect removes that client's attachments and preserves all terminal facts.                                                                                     |
| Create / spawn                  | Desktop authority plus connected client and new `CreationId` creates one starting terminal. Equivalent retry is idempotent; changed launch digest conflicts; spawn failure removes partial state. |
| Terminate / complete / dispose  | Desktop may request termination once. Child exit and PTY drain may arrive in either order; exited requires both. Running terminals cannot be disposed.                                            |
| Attach / detach                 | Desktop creates an attachment with bounded capabilities. Detach changes no terminal fact. One attachment per terminal may hold the resize lease.                                                  |
| Output / capture                | Parse mutation advances canonical revision under terminal authority; display-cadenced capture samples revision/generation independently and never creates a revision.                             |
| Demand / frame / acknowledgment | A readable attachment starts paused. Active, unblocked demand may emit at most one frame; later mutation coalesces into one dirty marker. Exact acknowledgment advances its base.                 |
| Recovery / generation / final   | Bad sequence/base/generation requests a snapshot. Resize or screen change forces a new-generation snapshot. Finalization may wait for active demand to reach `finalRevision`.                     |
| Resize                          | Only the lease attachment mutates canonical rows, columns, and cell-pixel geometry; every attachment observes the resulting revision.                                                             |
| Input                           | A capable attachment reserves bounded ordered ingress before raw or semantic encoding; rejection/cancellation frees capacity without an ordering gap.                                             |
| Upgrade / stop                  | Retained terminals keep a serving Host with `upgradePending`. The final explicit disposal enters draining; only an empty draining Host stops.                                                     |

All stale-epoch events, missing identities, wrong authority, wrong phase, wrong frame sequence/revision,
and denied resize attempts are rejected without commands or state mutation. Repeated operations explicitly
classified as idempotent also return no commands.

## Abstract-to-TypeScript refinement

| TLA+ variable or action                           | Concrete contract                                                                          |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `hostPhase`, `hostEpoch`                          | `ProtocolLifecycleState.phase`, `.epoch`                                                   |
| `creationOf`, `terminalPhase`                     | `terminals[].creationId`, `.terminalId`, `.phase`                                          |
| `childExited`, `ptyDrained`                       | `terminals[].childExited`, `.ptyDrained`                                                   |
| `connections`                                     | `connections[]`                                                                            |
| attachment owner/terminal/epoch/lease             | lifecycle attachment fields and Host `AttachmentState` authority                           |
| `revision`, `generation`, `finalized`             | Host canonical geometry/runtime revision, projection generation, and sealed final revision |
| `capturedRevision`, `capturedGeneration`          | one terminal-owned Ghostty render-state capture plus sampled metadata                      |
| `demand`, `blocked`                               | `SetProjectionDemand` and typed Host `ProjectionBlocked` state                             |
| acknowledgment, frame base/sequence/generation    | `FrameApplied`, `NeedSnapshot`, Host in-flight metadata, and format-2 header               |
| `pendingDirty`, `forceSnapshot`                   | bounded Host attachment delivery fields                                                    |
| `inputBuffer`                                     | lifecycle pending bytes and Host ordered-ingress charged bytes                             |
| `AttemptCreate`, `Disconnect`, `Detach`           | matching discriminated lifecycle events and `LifecycleOutcome`                             |
| `Capture`, `Emit`, `FrameApplied`, `NeedSnapshot` | Host capture scheduler/core transitions plus protocol events                               |
| `SetDemand`, `CaptureFailure`, `EncodeFailure`    | demand controls and Host blocked-recovery transitions                                      |
| `RequestUpgrade`, `StopHost`                      | matching events plus `close-connections` and `stop-host` commands                          |

TLA+ abstracts away concrete identifiers, launch payloads, bytes, error text, process handles,
transport, render-state allocation, and frame contents. TypeScript schemas/codecs and the native
Host state machine refine those omitted values through strict validation, hard bounds, and
deterministic tests. Desktop transport must later show that executing commands and transferring
buffers preserves this mapping.

## Traceability

| Claim                                           | Owned contract                      | TLA+ property or invariant                                        | Deterministic/property evidence                                          | Integrated evidence                              | Limits                                                         |
| ----------------------------------------------- | ----------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------ | -------------------------------------------------------------- |
| I1 renderer/client loss preserves terminals     | Lifecycle transitions               | `DisconnectPreservesTerminalState`                                | `disconnects attachments without changing any terminal lifecycle fact`   | Native PTY retry survives local disconnect       | Desktop restart and renderer broker remain later               |
| I2 graceful replacement requires no terminal    | Upgrade/stop transition             | `GracefulStopRequiresNoTerminal`, `DrainingEventuallyStops`       | `defers upgrade until the final retained terminal exits and is disposed` | N/A until updater slice                          | Weak fairness for enabled stop                                 |
| I3 one `CreationId`, no duplicate spawn         | Identity/create transition          | `TerminalIdentityIntegrity`                                       | `creates once by CreationId and rejects a conflicting retry`             | Lost-response retry retains one native PTY       | Current Host retains at most one terminal                      |
| I4 stale epoch mutates nothing                  | Identity and rejection rule         | `HostEpochIntegrity`, `AttachmentIntegrity`                       | `rejects stale epochs and attachment authority without mutating state`   | Native dispatch rejects a correlated stale epoch | One active native Host epoch                                   |
| I5 one in-flight frame plus dirty marker        | Frame backpressure                  | `OneFrameAndDirtyMarker`                                          | demand/coalescing/duplicate Host-core traces                             | Real PTY service and projected-store tests       | Two attachments, revision 0-2 in TLC                           |
| I6 delta base equals acknowledgment             | Format-2 frame and frame transition | `DeltaBaseMatchesAcknowledgement`                                 | replacement-snapshot and frame round-trip properties                     | Native producer plus packed-store recovery tests | Bounded revision model                                         |
| I7 detach has no terminal side effect           | Detach transition                   | `DetachPreservesTerminalState`                                    | disconnect/detach state tests                                            | Native connection cleanup retains PTY state      | Desktop broker remains later                                   |
| I8 one resize authority                         | Attachment/resize transition        | `ExclusiveResizeLease`                                            | `allows only the exclusive resize lease to mutate canonical dimensions`  | Lease-owned native PTY resize integration        | Two attachments in deterministic core                          |
| I9 scoped attachment lacks privileged authority | Control/lifecycle authority         | `ScopedAuthorityCannotCreate`                                     | authority rejection test                                                 | N/A until Desktop broker slice                   | Create modeled formally; other privileged events deterministic |
| I10 sharing-off starts no network state         | Package non-goal and state literal  | `NoNetworkListener`                                               | every parsed lifecycle state requires `networkListener: false`           | Host builds only UDS or named-pipe endpoints     | Static/runtime evidence does not cover a future gateway        |
| I11 one endpoint-owning epoch                   | Host launch model                   | `HostEpochIntegrity` plus one authoritative Host state            | repeated launch/stale epoch traces                                       | Native lifetime-lock race and reconnect test     | One user/channel endpoint                                      |
| I12 byte and queue memory remain bounded        | Hard bounds and codecs              | `TypeOK`, `InputMemoryIsBounded`, `AcceptedInputEventuallyWrites` | malformed corpus, strict-prefix property, input-backpressure test        | Native oversized-record and bounded-queue tests  | OS exhaustion remains outside progress claims                  |
| I13 frames name captured canonical state        | Capture/revision contract           | `FrameNamesCapturedCanonicalState`                                | mutation/capture and generation traces                                   | Ghostty capture/encode tests                     | Capture allocation failure is modeled abstractly               |
| I14 paused or blocked demand performs no work   | Projection demand                   | `PausedOrBlockedCannotBecomeReady`                                | pause/resume and explicit blocked-recovery traces                        | no-demand counters and real PTY pause test       | Scheduler/platform suspension is outside progress claims       |
| I15 frame sequence commits only with a frame    | Encode/enqueue atomicity            | `SequenceCommittedOnlyWithFrame`                                  | encode/enqueue failure and stale recovery traces                         | native frame conformance and attachment tests    | Transport transfer is deferred to Desktop                      |

Progress checks additionally include `ReadyDemandEventuallyEmits` and
`InFlightEventuallyClears`. They assume a healthy connected peer and weak fairness for an enabled
emit, acknowledgment, write, or stop action. No progress is claimed through process failure,
resource exhaustion, broken transport, a client that stops acknowledging, OS suspension, or a child
that ignores termination.

The merge TLC configurations contain two epochs, clients, creation identities, terminal identities,
and attachments in the lifecycle model; the frame model contains two attachments, revisions `0-2`,
generations `1-2`, attachment-local sequences `1-2`, and input capacity `2`. They use no state
constraint to remove reachable behavior. Larger identity, byte, platform, and timing spaces require
property or integrated evidence rather than being implied by these finite checks.
