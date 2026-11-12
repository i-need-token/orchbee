# Terminal Protocol Current Contract and Correctness Record

| Field           | Current authority                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status          | Current language-neutral wire-contract authority; the bindings implement no lifecycle state machine, PTY, IPC listener, renderer, Electron, recovery, or network behavior                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Wire owner      | `schemas/terminal`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Native consumer | `apps/terminal-host` consumes the language-neutral reference, registry and fixtures for bounded native dispatch and Zig conformance                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Rust binding    | The unpublished independent `crates/terminal-protocol` binding mirrors the complete message registry, fixed OBTP envelope, connection-control payloads, kind-10 terminal-inventory controls, kinds 12-19 non-creation lifecycle controls, kinds 20-25 attachment controls, the kind-30/31 raw-input exchange, the kind-32/33 resize-control exchange, kinds 34-39 semantic-input requests, the kind-43/44 projection-feedback controls, kind-45/50 row-access controls, kinds 47-49 projection demand, kinds 51-54 history controls, kinds 55-58 terminal-effect event representations, kinds 60-62 selection controls, kinds 63-68 selection-export representations, and kinds 70-73 terminal-configuration controls; the unshipped Rust Worker consumes only the envelope and connection-control subset for the existing Host handshake |
| Reference scope | Protocol generation 1; OBTF, OBPB, OBRW, and OBKG format generation 1; input/selection codecs; failure vocabulary; bounds; and pinned fixtures                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Change rule     | A changed field, message meaning, wire byte, bound, or fixture updates this record, Rust, Zig and TypeScript implementations, every producer/consumer, and evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

## Modeling record

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A native Host and TypeScript clients need one bounded, versioned, inspectable wire contract and allocation-conscious binary codecs without sharing business state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Inputs       | Raw and external records are untrusted until their exported strict decoder or schema accepts them. The same-user capability-scoped local Host data plane may use the projection-buffer structural view: fixed metadata and section bounds are checked once, while Host-produced per-cell columns are trusted by contract and are not rescanned. A local client may also inspect only a frame header to recognize an already committed immutable duplicate. Absence is explicit.                                                                                                                                                                                                                                  |
| Data         | Protocol versions and identities; transport envelopes; control records; terminal frames; row windows; input, projection, history, bell, selection, configuration, and runtime-failure wire vocabulary.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Outputs      | Successfully decoded or encoded values, typed `ProtocolFailure`, bounded binary payloads, and strict control values.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Invariants   | Accepted records use the exact current version, canonical identifiers and integers, registered message roles and encodings, valid discriminants, and declared hard bounds. Once one encoded frame is handed off for `(HostEpoch, AttachmentId, sequence)`, every byte and all header metadata for that scoped sequence are immutable and the sequence is never reused.                                                                                                                                                                                                                                                                                                                                           |
| Decisions    | Exact-version handshake, message requiredness and encoding, binary layouts, safety limits, strict control shapes, and mutually exclusive outcome representation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Model        | Strict normalization at external boundaries; deterministic JSON and binary codecs; and a separately named, capability-scoped structural-view path for immutable local Host projection buffers.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Dependencies | Zod validation and standard typed-array/TextEncoder APIs in TypeScript; the Rust envelope binding uses only the standard library, while its JSON control bindings use pinned `serde` and `serde_json`. Neither path gains clock, randomness, storage, process, network, DOM, Electron, GPU, PTY, or domain transition capability.                                                                                                                                                                                                                                                                                                                                                                                |
| Ownership    | `schemas/terminal` owns wire meaning only. Terminal Host owns lifecycle, delivery, retry, and concurrency; Worker owns Host lifetime, its Server-first Host admission order, coupled connection liveness, and terminal-close escalation; Server owns product binding, publication, and durable intent.                                                                                                                                                                                                                                                                                                                                                                                                           |
| Verification | Named examples, stable-seed codec properties, strict-prefix exploration, generated malformed corpora, generated-file comparison, export checks, Zig decoding of canonical binary inputs, Rust registry/envelope/connection/inventory/lifecycle/attachment/raw-input/resize/semantic-input/projection-feedback/projection-demand/history/row-access-control/terminal-effect-event/selection-control/selection-export/terminal-configuration-control conformance against the same language-neutral fixtures and the current metadata example, Worker real-process Host-handshake and peer-loss tests, and consumer tests that distinguish immutable duplicate recognition from strict admission of a new sequence. |
| Correctness  | Codec evidence validates wire representation and bounds. Consumer-owned state models and integration tests validate whether a well-formed message is legal and correctly acted on in current state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

## Identity and versioning

Every protocol identity is exactly 16 non-zero bytes. JSON renders it as exactly 32 lowercase
hexadecimal characters without separators. All-zero bytes are reserved for an absent envelope
identity and are rejected where an identity is required.

| Identity                   | Owner and lifetime                                                                                                               |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `HostEpoch`                | Random per Host process; scopes every Host-owned reference                                                                       |
| `CreationId`               | Application-created durable identity; an idempotency key while its terminal remains retained by one Host epoch                   |
| `TerminalId`               | Random Host identity retained until explicit disposal                                                                            |
| `ClientId`                 | Wire-supplied diagnostic broker identity; it does not grant authority independently of the active physical session               |
| `AttachmentId`             | One physical-connection-owned attachment carrying negotiated capabilities                                                        |
| `TerminalThemeOperationId` | Application-created idempotency identity for one terminal-theme compare-and-set operation                                        |
| `CorrelationId`            | Required on every request and response after `Hello`; all-zero envelope bytes mean absence only for handshake messages or events |

Each of the five version domains carries a generation in the major component of its existing
version field, with minor and patch zero. The current protocol is generation 1: `Hello`,
`HelloAccepted`, and every transport envelope carry major `1`, minor `0`, and patch `0`. The
envelope codec reports the carried version unchanged so that a receiver can answer with it; every
receiver refuses any other version, and the Host answers such a `Hello` with `ProtocolRejected`.
This pre-release contract contains no negotiated range, compatibility mode, or historical message
shape. The packed OBTF frame, OBPB projection-buffer, OBRW row-window, and OBKG graphics-extension
formats each carry their own generation in the format major byte and are at generation 1. Their
decoders accept only that exact encoding, reject any other version as `unsupported-version`, and
follow the repository [pre-release version rule](../../../docs/ARCHITECTURE.md#data-boundaries).
Generation 1 replaced the pre-release `0.1.0` encoding of all five domains. After the first public
release, an incompatible change introduces the next generation of its domain and is recorded in
[`../docs/version-history.md`](../docs/version-history.md). The Host build version is a
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

`TerminalProfile` carries explicit `defaultCursorShape` (`block`, `underline`, or `bar`) and
`defaultCursorBlinking` values. Both fields have compatibility defaults (`block` and `false`) when
decoding an older persisted profile shape; current encoders materialize them. These are launch
defaults, not immutable cursor state: terminal escape sequences may override both until RIS or an
application reset restores the profile values.

`TerminalTheme` is a complete runtime color value containing `colorScheme`, unsigned 24-bit
`defaultForegroundRgb` and `defaultBackgroundRgb`, nullable unsigned 24-bit `defaultCursorRgb`,
and exactly 256 unsigned 24-bit `palette` entries. `TerminalThemeState` adds canonical decimal-text
uint64 `revision` and a nullable `operationId`. Revision `0` is the launch baseline and is the only
legal state with a null operation identity; every positive revision carries the identity that
committed it. Runtime theme does not include cursor shape/blink, overlays, application tokens,
content revision, or generation.

The message registry also owns whether each kind may be a request, response, event, or a declared
combination; its payload encoding in each legal role; and the successful response kind for every
request. TypeScript connection code derives those facts instead of accepting them as call-site
arguments. Native production mirrors the registry and rejects a wrong role or encoding before a
record is handed off. The three registry tests compare every mirrored message field against `v1/constants.json`, not only the
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

`CreateTerminal` also carries required `environmentOverlay`, separate from `TerminalLaunch`. It
contains at most 16 entries; every value is non-null and at most 4 KiB of UTF-8. Names use the same
syntax and target-operating-system identity rule as launch environment names, must be unique within
the overlay, and must not overlap any launch environment name. The overlay is not inventory data
and is not part of the durable launch fingerprint. The Host nevertheless derives a
target-normalized, order-independent digest for its retained creation generation: a non-empty replay
must match that digest, while an empty replay may validate an already retained creation without
reconstructing its capability. The Host runtime contract owns those replay and lifetime semantics;
this contract owns only the required bounded wire distinction.

The four launch geometry fields form one closed choice. `columns`, `rows`, `cellPixelWidth`, and
`cellPixelHeight` are either all valid integers or all `null`; mixed presence is malformed.
`columns` and `rows` are always positive; `cellPixelWidth` and `cellPixelHeight` are either both
positive (pixels known) or both zero (pixels unknown) — a mixed cell-pixel pair is malformed.
Numeric geometry requests immediate startup and is required for headless and test callers. Null
geometry reserves a deferred terminal identity and prepared launch without supplying an implicit
grid. A later `StartTerminal` carries one complete `TerminalGeometry`; the wire contract defines no
timeout or 80x24 visual fallback.

An optional non-null `CreateTerminal.captureSource` selects Windows scrollback
editor creation with deferred geometry only. Its closed object carries
`attachmentId`, `terminalId`, positive canonical uint64 text `generation`, and
canonical uint64 text `revision` (zero permitted). Ordinary omission retains the
legacy request shape and encoded bytes; explicit null, unknown fields, malformed
identifiers/fences and immediate geometry are refused. The Host owns read
authorization, one-lock complete capture, source-discriminated replay identity,
literal document-path append, deferred Start and document cleanup. The request
does not accept a filesystem path or transcript-selected executable. The selected
editor's launch remains under the upstream trusted launch authority. Rust projects
this optional workflow as `CreateScrollbackEditor` within the same creation kind;
ordinary `CreateTerminal` values and acknowledgement shapes stay unchanged.

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
Inventory summaries report the effective configuration and its independent configuration revision,
plus the complete current runtime-theme state and its independent theme revision.

Canonical unsigned 64-bit integers inside JSON use decimal text with no sign, whitespace, exponent,
or leading zero except the value `0`. Packed binary uses little-endian unsigned integers.

### Process samples

`ReadProcessInfo` (kind 79, control JSON request and response) is scoped to one read-capable `attachmentId`. The response echoes `attachmentId` and its exact `terminalId`, with `kind: "windows-job"`, positive uint32 `rootPid`, and a PID-sorted `processes` array of at most 256 unique rows. Each row is a compact seven-element array `[pid, parentPid, creationTime, console, image, commandLine, currentDirectory]`: positive uint32 `pid`, nullable positive uint32 `parentPid`, positive canonical decimal uint64 `creationTime` (Windows FILETIME) string, `console` one of `attached`, `detached`, `none`, or `unknown`, and nullable `image`, `commandLine`, and `currentDirectory`; an array of any other length is invalid. A row with a null `image` is opaque: the member is live but denied limited query rights, so only its identity and parent are known. Every row is a member that was live when read, so a consumer's anchor is live exactly when a row matches its PID and creation time. Each text is strict Unicode without NUL, bounded to 4096 UTF-16 code units; the entire reply remains within the 64 KiB control limit. A root that has exited can be absent from the process rows while its retained identity still supplies `rootPid`. The request may carry the optional member `includeLaunchKind`; only when it is `true` does the response add `launchKind` after `kind`. It names how the Host launched the root: `managed-powershell` for PowerShell given the managed startup, `cmd` for an interactive cmd given prompt marks, and `other` for every other launch and for every launch off Windows. Without the member, or with `false`, the response is unchanged, so decoders that predate it keep working.

The Host owns sampling and attachment authorization. Unknown or non-readable attachments fail `missing-resource`; unsupported platforms fail `process-sample-unsupported`; membership change is never a failure; OS/budget failure fails `process-sample-unavailable`; capacity or encoded-size excess fails `limit-exceeded`. No failure becomes empty success. The sample describes sequential observations of one retained Windows Job, or on macOS and Linux of the session the root leads, not an atomic tree, foreground process group, input permit, or readiness guarantee. Optional fields may be absent when remote inspection is unavailable. Consumers must correlate both the actual response and attachment/terminal identities before using the facts.

On macOS and Linux, `kind` stays `windows-job` and every field keeps its shape. `creationTime` stays on the FILETIME scale (100 ns units since 1601 UTC). macOS start times are exact to the microsecond. Linux start times have clock-tick resolution, usually 10 ms, and are rounded down: they are never later than the real start, can be up to about 11 ms earlier, and stay equal across samples of one process. `parentPid` is the current parent, so an orphan reports the process that adopted it. `console` compares a member's controlling terminal with the live root's terminal, `none` means the member has none, and every row is `unknown` once the root has exited. `commandLine` is the argument vector with each argument quoted as Python `shlex.quote` quotes it, joined by single spaces, so a POSIX word splitter recovers the exact arguments; a trailing empty argument is not preserved. On macOS, `commandLine` and `currentDirectory` are absent for a member that runs as another user.

## Runtime binding

Each connection sends exactly one `Hello` before any other request. `Hello` and `HelloAccepted`
negotiate `frameBytes`, `inputBytes`, and `rowWindowBytes`. Every post-handshake request carries a
non-zero `CorrelationId` and the current `HostEpoch`; every response preserves the correlation.
Events carry no correlation. Stale epochs, a repeated `Hello`, and pre-handshake operations fail
without state mutation.

Named response kinds are authoritative: `HelloAccepted`, `ProtocolRejected`,
`TerminalStartAccepted`, `AttachmentCapabilities`, `InputAccepted`, `ResizeAccepted`,
`ProjectionDemandAccepted`, `TerminalDefaultsAccepted`, `TerminalConfigurationAccepted`, and
`TerminalControlAccepted` and `TerminalThemeResult` answer their corresponding request kinds.
Other successful operations respond with their request kind. `TypedFailure` may answer any request
and preserves its correlation.

| Operation                         | Successful payload                                                                                                                                                                                                                                                                                   |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ListTerminals`                   | Generation-scoped page of effective terminal summaries                                                                                                                                                                                                                                               |
| `CreateTerminal`                  | `TerminalIdentity`                                                                                                                                                                                                                                                                                   |
| `StartTerminal`                   | Exact terminal identity, accepted complete geometry, and `started` or `already-started` transition                                                                                                                                                                                                   |
| `SetTerminalDefaults`             | Effective default configuration and default revision                                                                                                                                                                                                                                                 |
| `UpdateTerminalConfiguration`     | Terminal identity, effective configuration and revision, content revision/generation, and evicted-row count                                                                                                                                                                                          |
| `ApplyTerminalTheme`              | Terminal and request-operation identities, `applied`, `already-applied`, or `stale` transition, and the complete current operation/revision/theme state                                                                                                                                              |
| `TerminateTerminal`               | Target `TerminalId`                                                                                                                                                                                                                                                                                  |
| `ForceTerminateTerminal`          | Target `TerminalId`                                                                                                                                                                                                                                                                                  |
| `DisposeTerminal`                 | Target `TerminalId`                                                                                                                                                                                                                                                                                  |
| `AttachTerminal`                  | Attachment and terminal identities, granted input/read/resize capabilities, initial capability revision `1`, and resize-lease fact                                                                                                                                                                   |
| `DetachTerminal`                  | Target `AttachmentId`                                                                                                                                                                                                                                                                                |
| `UpdateAttachmentInputCapability` | Attachment and terminal identities, complete current capabilities, current positive capability revision, and resize-lease fact                                                                                                                                                                       |
| `ClaimResizeLease`                | Attachment and terminal identities, complete unchanged capabilities and capability revision, and the committed resize-lease fact                                                                                                                                                                     |
| raw or semantic input             | Attachment identity, exact capability revision, and encoded terminal-byte count committed to ordered ingress                                                                                                                                                                                         |
| `ResizeTerminal`                  | Attachment identity, accepted canonical cell/pixel geometry, and resulting revision                                                                                                                                                                                                                  |
| `ClearTerminal` / `ResetTerminal` | Attachment and terminal identities, exact operation, and resulting revision/generation                                                                                                                                                                                                               |
| projection demand                 | Attachment identity plus accepted `active` or `paused` demand; an input-capable attachment without read capability holds it only for focus-claim eligibility                                                                                                                                         |
| frame/recovery                    | OBTF snapshot/delta event carrying the committed Host theme revision, exact `FrameApplied`, typed `NeedSnapshot`, or typed `ProjectionBlocked`                                                                                                                                                       |
| history/read rows                 | Accepted history demand, bounded render/text `RowsSnapshot`, exact `RowsApplied`, `HistoryChanged`, and `HistoryObserved`                                                                                                                                                                            |
| selection/export                  | Current selection state plus bounded started/chunk/acknowledgment/finished or cancellation flow                                                                                                                                                                                                      |
| terminal start / outcome          | `TerminalStarted` carries terminal identity after the child exists. Normal exit carries final revision and an exit-code, signal, or unknown result. Runtime failure carries final revision, typed failure, and process observation. Inventory uses mutually exclusive phase-specific summary fields. |

`RuntimeFailure.stage` is the discriminant for a primary terminal failure; `kind` is its required
coarser category, not an independent field. Its legal pairs are
`admit-terminal-reply/terminal-reply`, `apply-resize/resize`,
`dispatch-termination/process-control`,
`observe-process/process-observation`, `read-output/pty-read`,
`synchronize-worker/worker-integrity`, and `write-input/pty-write`. Every other pair is malformed.
`ProcessObservation.kind = unavailable` always carries the `observe-process/process-observation`
pair. `FailureContainment.kind = blocked` instead carries one contextual cleanup pair:
`contain-process/process-control`, `release-output/pty-read`, or
`release-process-identity/process-observation`, or
`release-editor-document/editor-document`. The document-release pair identifies
retained Windows scrollback editor cleanup after containment, joined workers and
process-identity release, or cleanup of a failed deferred Start. It never claims a
child process outcome. Cleanup pairs are malformed as primary failures,
and primary pairs are malformed in either contextual position.

`ListTerminals` accepts `{ cursor, maximumTerminals }`. `cursor` is `null` for the
first page or contains canonical decimal `inventoryGeneration` and `offset` values from the prior
response. `maximumTerminals` is `1-128`. The response repeats `inventoryGeneration`, returns at most
the requested number of summaries, and supplies either the next generation-scoped cursor or
`null`. Create and dispose advance membership generation; lifecycle, geometry, revision, and exit
changes do not. A generation mismatch returns `stale-inventory-cursor`, so one traversal never
combines two memberships. Every page remains within the 64 KiB control bound, and the protocol sets
no total terminal count limit.

An `awaiting-start` inventory summary carries null `columns` and `rows`, no exit, failure,
containment, or process observation, and `terminateRequested: false`. Started phases carry positive
grid geometry. An exact `StartTerminal` replay after startup returns the original identity and
geometry with `transition: already-started`; a different geometry is a non-retryable typed
`start-conflict`. Spawn failure is a typed retryable failure and publishes neither successful start
response nor `TerminalStarted` event; lifecycle ownership of those effects remains with Terminal
Host.

`TerminateTerminal` and `ForceTerminateTerminal` are distinct request kinds. The wire permits
Desktop to send force directly; the Host-owned lifecycle decides whether the current phase accepts
the request and guarantees that force subsumes graceful intent. Neither request disposes retained
identity. Desktop owns escalation timing and waits for either `TerminalExited` or `TerminalFailed`
before requesting disposal. Missing identities and invalid current-state requests are reported as
typed failures without being reinterpreted by the codec.

Successful `TerminateTerminal`, `ForceTerminateTerminal`, and `DisposeTerminal` responses retain
the request kind (`12`, `18`, and `13`) and carry exactly `{ terminalId }`, validated by
`TerminalAcknowledgedSchema`. The Rust lifecycle acceptance variants retain that typed
`TerminalTarget`; they reject absent, malformed, duplicate, or additional fields. Decoding an
acknowledgement preserves its target without deciding whether it matches a pending request. The
consumer owns envelope correlation and target matching; termination acceptance does not establish
process exit or authorize disposal. The existing `outputs.terminal_acknowledged` conformance
fixture defines the canonical bytes for all three responses. Rust lifecycle fixture round trips,
target-retention and malformed-input cases verify this bounded codec claim against the same fixture
used by Host Zig encoder conformance. They do not establish Host runtime transitions, asynchronous
ordering, or a consumer's close workflow.

The wire does not expose the Host's physical broker-session identity. The Host binds attachments to
its one active broker session and resolves each `AttachmentId` through that session. `ClientId` is
diagnostic metadata and does not merge, transfer, or independently grant authority.

Every attachment requests at least one of input, read, or resize authority. A resize lease is legal
only when resize authority is also requested and granted. Empty requested capability bindings and a
resize lease without resize authority are malformed at attach admission. A complete current
capability value may be all false after an input-only attachment is downgraded.

`ClaimResizeLease` kind `25` carries exactly one attachment identity. A running attachment with
resize capability may claim the terminal's sole lease; replay by the current owner is idempotent.
An applied transfer returns complete unchanged `AttachmentCapabilities`, first publishes the prior
owner's `ResizeLeaseChanged` fact with `resizeLease: false`, and then publishes the claimant's fact
with `resizeLease: true`. The successful response follows both events. A rejected claim preserves
the prior owner, geometry, capability revision, and admitted capability value. A `ResizeTerminal`
request from a non-owner is rejected and never acts as an implicit claim.

`ResizeLeaseChanged` is an event-only complete ownership fact containing the exact attachment and
terminal identities plus `resizeLease`; it is never a toggle. When detaching a resize owner promotes
an existing observer, the Host commits that observer's event before the successful detach response.
Consumers validate both identities against their attachment scope before changing who may issue
`ResizeTerminal`; presentation measurement itself remains client-local and does not require a lease.

`ResizeTerminal` requires complete geometry within the per-dimension, cell-pixel-pair, grid-cell,
and unsigned-16 pixel-extent bounds. `ResizeAccepted` echoes the committed geometry and adds its resulting revision.
For compatibility, the current response schema enforces the four per-dimension bounds plus the
cell-pixel-pair invariant — a mixed pair was never decodable under the pre-change minimum and
carries no valid meaning — but does not repeat the request schema's grid-cell and pixel-extent
cross-field refinements. This decoder boundary does not authorize or
describe invalid runtime geometry: Terminal Host accepts and emits only complete valid geometry.
Tightening response admission requires an explicit compatibility decision rather than an incidental
binding change.

`ApplyTerminalTheme` is terminal-scoped and requires no attachment. It carries `terminalId`,
`operationId`, `expectedThemeRevision`, and one complete `terminal` theme. The Host checks an exact
current-operation replay before lifecycle and compare-and-set: the same operation ID and theme
returns `already-applied` without mutation, while the same ID with different theme data returns
non-retryable `theme-operation-conflict`. A different operation whose expected revision is not
current returns `stale` without mutation. An accepted operation applies the complete deferred or
started runtime state and then advances the independent theme revision once; its result transition
is `applied`.

Every `TerminalThemeResult` echoes the request operation and terminal identities and includes the
complete authoritative `current` state. For `applied` and `already-applied`, `current.operationId`
equals the echoed request operation; for `stale`, it differs. Runtime application replaces the
Host-owned dynamic-color defaults and clears prior OSC color overrides; later terminal OSC actions
may override those defaults, while their reset forms, RIS, and application reset return to the
latest runtime theme. A lost result is unknown outcome: the client first reconciles through
inventory before it rebases and never infers authority from a client-local override.

`ClearTerminal` and `ResetTerminal` carry the input-capable `AttachmentId`; Clear additionally requires Boolean `preservePrompt`. With false, Clear erases and homes the active Ghostty screen without resetting modes; true preserves the complete primary-screen prompt line under the [Host runtime contract](../../../apps/terminal-host/docs/runtime-model.md#prompt-preserving-clear). These are semantic Host requests. Reset accepts no prompt policy, performs Ghostty's full reset and returns to the primary screen. `TerminalControlAccepted` returns attachment, resolved terminal, exact operation and resulting generation/revision. A lost response is unknown and does not authorize replay.

Every attachment starts with capability revision `1`. `AttachmentCapabilities` reports that
positive canonical uint64 revision beside the complete current capability value, including an
all-false value after an input-only attachment is downgraded.
`UpdateAttachmentInputCapability` kind `24` carries the attachment, a positive revision, and the
next input bit. The Host applies only an exact `current + 1` revision that changes the input bit;
the exact current revision and value is an idempotent replay. A same-revision different value or a
next-revision unchanged value is `capability-revision-conflict`; an older revision or gap is
`stale-capability-revision`. Read and resize capability remain the immutable admitted values in
this protocol version.

`WriteInput` is a raw-byte payload containing exactly 16 non-zero `AttachmentId` bytes, one
positive little-endian uint64 capability revision, and then one to the negotiated `inputBytes`
terminal bytes. The hard terminal-byte maximum is 1 MiB, so the largest raw payload is 1 MiB plus
24 bytes. The attachment and exact revision resolve current input authority; `TerminalId` never
appears in this payload. `InputAccepted` echoes the revision and means the complete terminal byte
suffix was committed once to the ordered bounded Host queue. It is queue-admission-only and does
not promise the bytes have reached the terminal writer, so terminal-generated replies may be
delivered ahead of accepted client bytes under the Host's bounded reply-priority policy. A broken
connection before that response leaves the outcome unknown and never authorizes replay.

TypeScript producers use `encodeInputRequest`; the message kind selects the raw or semantic binary
codec. Native consumers route the same registered kinds through one semantic-input decoder, so
neither side maintains a business-layer kind-to-format table.

`KeyInput`, `TextInput`, `MouseInput`, `FocusInput`, `PasteInput`, and `PasteAutoInput` are bounded packed payloads
prefixed by the same 24-byte authority header: the attachment identity followed by its positive
little-endian uint64 capability revision. They carry browser-normalized semantic intent, not DOM
objects or preselected escape sequences. `KeyInput` byte `24` carries the key action, while byte
`25` carries composing in bit `0` and the per-event Option-as-Alt intent in bit `1`. The client
resolves any left/right macOS preference before publication; Terminal Host applies the intent to
the authoritative Ghostty encoder modes instead of accepting a client-selected escape sequence.
`PasteInput` uses a 32-byte header: byte `24` carries the reviewed bracketed-paste expectation in
bit `0`, bytes `25-27` are reserved zero, and bytes `28-31` carry the little-endian text byte
length. Unknown flag
bits or nonzero reserved bytes are rejected. A mouse press or release always identifies its button.
Mouse motion with no pressed buttons carries a null button; motion with pressed buttons identifies
one active button that is present in `pressedButtons`, whose bit set describes the complete current
pressed-button state. Wheel reports remain button presses and need not describe a held button.
Host dispatch reserves ingress order and snapshots authoritative Ghostty encoder modes before
encoding, so output coalescing or a frame in flight cannot make key, mouse, or focus encoding stale.
For paste, the Host compares authoritative bracketed-paste mode with the reviewed expectation while
holding the same terminal lock used for encoding. A mismatch returns non-retryable
`input-mode-changed` before queue admission; the client must obtain current projected state and ask
for a new user decision rather than replaying an ambiguous paste. The negotiated `inputBytes` limit applies
to the encoded terminal-byte output, not the packed semantic payload; `acceptedBytes` reports that
output length and may be zero when the current terminal mode intentionally emits nothing. The Host
must preflight zero-output and paste bounds before queue admission, then charge only the encoded
bytes to the ordered ingress budget.

`PasteAutoInput` carries the same 24-byte authority header and a 32-byte header layout with every
flag and reserved byte required zero; bytes `28-31` carry the little-endian content byte length and
the remaining body is opaque paste content. Unlike `PasteInput`, the content is raw bytes with no
UTF-8 requirement and no client-declared bracketed expectation: the Host snapshots the authoritative
bracketed-paste mode under the same terminal lock used for encoding and wraps the content itself.
Opaque paste content is bounded at 256 KiB, below the general semantic-input body bound.

`PromptLineInput` (kind 84, raw-bytes request answered by `InputAccepted`) types one shell command
line only while the Host confirms that the terminal's root shell waits at a fresh, empty prompt.
It carries the same 24-byte authority header and a 32-byte header: byte `24` carries Right in bit
`0`, bytes `25-27` are reserved zero, and bytes `28-31` carry the little-endian content byte length.
Unknown flag bits or nonzero reserved bytes are rejected. The content is 1 byte to 256 KiB of valid
UTF-8 with no C0 control or DEL, so Enter is the line's only submission. The Host encodes the
content as one paste under the authoritative bracketed-paste mode, then Right when requested, then
Enter, each key as a press and its release, and queues the result as one client entry;
`acceptedBytes` reports its length. The Worker is its only intended sender; no TypeScript producer
exists.

The Host admits the line only when all of these hold:

1. Its own launch record names the root an interactive shell.
2. A synchronous process sample finds no other console program; a failed sample refuses.
3. The primary screen is active.
4. The latest primary prompt went from its start (`A`, `N`, or `P` with no kind or `k=i`) to its
   input start (`B` or `I`). No command start or end (`C`, `D`), newer prompt start, or
   continuation or secondary prompt (`k=c`, `k=s`) followed. A right prompt (`k=r`) drawn before
   the input start belongs to that prompt.
5. The cursor is still at the position anchored at the input start, on the same screen
   generation, and the text from that anchor to the end of its logical line is unchanged.
6. No client input other than a focus report was published after the input start, no client
   queue entry was pending at the input start, and none is pending now. Host terminal replies do
   not count.
7. The input start is at least the quiet period old: `prompt_line_quiet_milliseconds` in the
   constant registry's `protocol_limits`, 100 ms. The Host never waits for a younger prompt; it
   applies requests one at a time, so a wait would stall every terminal it serves.

Publication repeats every rule except the process sample and also requires that no prompt mark
arrived after preparation. Marks printed by any program other than a root shell the Host started
fail rules 1 or 2.

A refusal types nothing and reserves no queue position. It is a `TypedFailure` whose code is
`prompt-unconfirmed` and which adds one member, `reason`, after `retryable`. Reasons in check order
are `not-shell` (rule 1), `busy` (rule 2), `full-screen` (rule 3), `unmarked` (no primary prompt
has started since launch), `not-at-prompt` (rule 4), `input-pending` (rule 6), `changed` (rule 5,
a mark after preparation, or a prompt without a mark time), and `settling` (rule 7, reported only
when every other rule holds). Only `settling` is retryable: its sender waits out the quiet period
and sends the line once more, and a second `settling` is a refusal like the others. Every other
reason is non-retryable. A root that has exited fails `invalid-phase` like other input. No other
typed failure carries `reason`.

A Host that serves `PromptLineInput` lists the capability `prompt-gated-input` in its `worker-host`
handshake answer. A Worker whose Host answer lacks it refuses an idle-prompt send before any
attachment; it never types the line as ordinary input.

## Message registry

| Range   | Group                                      | Kinds                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ------- | ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `1-5`   | Connection                                 | `Hello=1`, `HelloAccepted=2`, `ProtocolRejected=3`, `Ping=4`, `TypedFailure=5`                                                                                                                                                                                                                                                                                                                                                    |
| `10-19` | Inventory and lifecycle                    | `ListTerminals=10`, `CreateTerminal=11`, `TerminateTerminal=12`, `DisposeTerminal=13`, `TerminalStarted=14`, `TerminalExited=15`, `StartTerminal=16`, `TerminalStartAccepted=17`, `ForceTerminateTerminal=18`, `TerminalFailed=19`                                                                                                                                                                                                |
| `20-25` | Attachment                                 | `AttachTerminal=20`, `DetachTerminal=21`, `AttachmentCapabilities=22`, `ResizeLeaseChanged=23`, `UpdateAttachmentInputCapability=24`, `ClaimResizeLease=25`                                                                                                                                                                                                                                                                       |
| `30-39` | Input and size                             | `WriteInput=30`, `InputAccepted=31`, `ResizeTerminal=32`, `ResizeAccepted=33`, `KeyInput=34`, `TextInput=35`, `MouseInput=36`, `FocusInput=37`, `PasteInput=38`, `PasteAutoInput=39`                                                                                                                                                                                                                                              |
| `41-58` | Projection and effects                     | `SnapshotFrame=41`, `DeltaFrame=42`, `FrameApplied=43`, `NeedSnapshot=44`, `ReadRows=45`, `RowsSnapshot=46`, `SetProjectionDemand=47`, `ProjectionDemandAccepted=48`, `ProjectionBlocked=49`, `RowsApplied=50`, `SetHistoryDemand=51`, `HistoryDemandAccepted=52`, `HistoryChanged=53`, `HistoryObserved=54`, `TerminalBell=55`, `TerminalClipboardWriteRequest=56`, `TerminalMetadataChanged=57`, `TerminalApplicationSignal=58` |
| `60-68` | Selection and export                       | `SelectionGesture=60`, `SelectionCommand=61`, `SelectionChanged=62`, `SelectionExport=63`, `SelectionExportStarted=64`, `SelectionExportChunk=65`, `SelectionExportChunkApplied=66`, `SelectionExportFinished=67`, `SelectionExportCancel=68`                                                                                                                                                                                     |
| `70-78` | Terminal configuration, control, and theme | `SetTerminalDefaults=70`, `TerminalDefaultsAccepted=71`, `UpdateTerminalConfiguration=72`, `TerminalConfigurationAccepted=73`, `ClearTerminal=74`, `ResetTerminal=75`, `TerminalControlAccepted=76`, `ApplyTerminalTheme=77`, `TerminalThemeResult=78`                                                                                                                                                                            |
| `79-84` | Host services                              | `ReadProcessInfo=79`, `DecodeGraphicsPng=80`, `GraphicsPngDecoded=81`, `StageTerminalOutput=82`, `TerminalOutputStaged=83`, `PromptLineInput=84`                                                                                                                                                                                                                                                                                  |

Unlisted numbers are unassigned. Title and current-directory changes travel in terminal frames and
the identity-scoped metadata event. Initial resize-lease authority is returned by attachment
admission; later ownership transfers use the attachment-scoped claim and complete events.

`../v1/constants.json` owns this table's language-neutral message contracts. Rust, Zig and
TypeScript compare their actual implementations, including the private TypeScript
`MESSAGE_CONTRACTS`, against it. Adding or changing a kind requires one atomic protocol change
to the registry, bindings, codecs, fixtures and consumers.

`StageTerminalOutput` (kind 82, raw-bytes request) stages earlier output for the next
`CreateTerminal` on the same connection. Its payload is the four bytes `OBSO` and the bytes `00 01 00 00`, the
16-byte creation id, a big-endian u32 output length, four zero bytes, then exactly that many output
bytes, at most 1 MiB. The output admits only UTF-8 text, CR, LF and SGR sequences (`ESC [` then
digits, `;` or `:`, ending in `m`); any other control, escape, DEL, C1 or invalid UTF-8 is an
`invalid-field` typed failure. The Host keeps one staged output per connection, replacing any earlier
one, and answers `TerminalOutputStaged` (kind 83, raw bytes): `OBSS` and the bytes `00 01 00 00`, the creation id,
the staged length and four zero bytes. The next `CreateTerminal` takes the slot whether or not it
matches; only a creation with the same creation id draws the output into the new screen before the
child starts, then resets style, moves it into history and homes the cursor, so the console host
starts from a blank screen. A replayed creation never draws it again.

`TerminalBell` is an event-only control record containing `attachmentId` and a positive canonical
terminal bell `sequence`. The Host baselines a new read-capable attachment at the terminal's current
sequence, so earlier bells are never replayed on attach or reconnect. Each later sequence is emitted
once per live read-capable attachment when its exact outgoing record is committed. The event has no
client acknowledgement and carries semantic intent only; sound, visual flash, notification, and
accessibility policy remain application effects rather than wire behavior.

`TerminalClipboardWriteRequest` is an event-only control record containing `attachmentId`, a
positive terminal-owned canonical `sequence`, and at most 16 KiB of plain UTF-8 text. NUL,
unpaired surrogates, a zero sequence, and oversized text are malformed. It represents one Host-
admitted OSC 52 clipboard write assigned to one read-capable attachment; it does not represent a
clipboard read or grant an effect. Terminal Host owns bounded pending state, one-recipient delivery,
outgoing-batch rollback, and removal after handoff. The receiving application owns the explicit
per-request commit policy and platform clipboard effect. The record is not acknowledged or replayed
to another attachment, and it carries no trust duration, selection-kind choice, rich format, or
clipboard contents from the client.

`TerminalApplicationSignal` is an event-only control record containing one exact terminal identity,
a positive terminal-owned `sequence`, and at most 4 KiB of UTF-8 `payload`. Terminal Host admits
the decoded payload from the exact private APC prefix `orchbee.agent.v1;`; the record's literal
channel `orchbee.agent.v1` is only a historical Host-to-Desktop discriminator and is not emitted by
the child. The record does not authenticate the emitting process or grant Agent, lifecycle,
attention, outcome, notification, or provider authority. Terminal Host retains up to 64 admitted
hints in terminal-sequence FIFO order and removes the queue head only after its exact outgoing
record commits. Product adapters must validate a narrower payload schema before interpreting it.

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
| `4`    |     1 | Format major `1`                                                                                   |
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
| `144`  |     1 | Format patch `0`                                                                                   |
| `145`  |     8 | Committed Host terminal-theme revision                                                             |
| `153`  |     7 | Reserved zero                                                                                      |

The interaction word uses bits 0–17 for the existing keyboard/mouse modes and bit 18 for
Ghostty `mouse_alternate_scroll` (DEC 1007). The Host captures the complete word with each
revision, including mode-only changes; deltas replace it. OBPB carries the same word at offset
108, including inside render row windows. Bits 19–31 remain invalid. `alternateScroll` is a
required typed Boolean; false is an observed mode, never a missing-field default. Update and
roll back the complete development cohort under the current pre-release version policy.

Variable sections follow in this exact order:

1. Each string is `u32 byteLength` followed by strict UTF-8 bytes.
2. Each 20-byte style contains foreground, background, and underline color codes (`u32`), hyperlink
   string index `u32`, explicit style flags `u16`, underline kind `u8`, and reserved zero `u8`.
   RGB occupies the low 24 bits. Cell colors in OBTF, OBPB and render-detail OBRW use these exact codes:

   | Code                      | Meaning                                              |
   | ------------------------- | ---------------------------------------------------- |
   | `0x00000000`–`0x00ffffff` | Explicit RGB, including black                        |
   | `0x01000000`              | Client-effective channel default                     |
   | `0x02000000`              | No explicit color; underline inherits displayed text |
   | `0x03000000`–`0x0300000f` | Client-effective ANSI palette index 0–15             |

   Other codes and nonzero reserved bits are rejected. Header defaults and cursor colors remain
   RGB with their existing field-specific cursor sentinel. The current generation 1 contracts change
   in place under the pre-release version policy; same-cohort admission prevents mixed bindings.
   Clients resolve ANSI tags from their active palette before applying inverse. Bold stays a font
   attribute: the Host never adds a bright offset; SGR 90–97 already identifies indexes 8–15.
   Faint keeps its existing Client intensity treatment. Explicit underline colors follow the same
   palette rule, and an absent underline color inherits the displayed text after inverse. Client
   selection/search foregrounds and fills take precedence over the resolved cell colors. Cursor
   metadata remains Host RGB; Client cursor presentation and contrast policies remain local.

   **Vetoable decision: child palette overrides.** A slot explicitly overridden by a program with
   OSC 4 is projected as RGB, even for indexes 0–15 and even when equal to its configured value.
   OSC 104 for that slot or all slots restores the ANSI tag. This preserves program-selected colors
   without an override map on the wire. Indexes 16–255 always remain Host-resolved RGB.

3. Each eight-byte row header contains row index `u16`, wrap flags `u8`, OSC 133 prompt class `u8`,
   cell count `u16`, and reserved zero `u16`. Exactly `columns` eight-byte cells follow: grapheme
   string index `u32`, style index `u16`, packed width/OSC 133 semantic/protected flags `u8`, and
   reserved zero `u8`.
4. Each extension has kind `u16`, flags `u16`, byte length `u32`, and bytes. Flag bit `15` means
   required; an unknown required extension is rejected. Unknown optional extensions remain bounded
   and are returned as opaque data.

The current optional extension registry is:

| Kind |        Bytes | Meaning                                                                                                                                              |
| ---: | -----------: | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
|  `2` | `40..33 MiB` | One strictly decoded OBKG Kitty graphics resource snapshot or placement-only update.                                                                 |
|  `3` |          `2` | Current terminal progress: state byte plus percentage `0..100` or null sentinel `255`.                                                               |
|  `4` |         `25` | Latest semantic command completion: positive sequence, duration milliseconds, positive wall-clock completion milliseconds, and normalized exit code. |
|  `5` | `10` or `66` | Row-origin epoch and the latest correlated Shell invocation; see the extension contract below.                                                       |
|  `6` |          `8` | Real-time start of the invocation in the same frame's kind `5`; see the extension contract below.                                                    |
|  `7` |          `9` | The program's current OSC 7501 root-record state and the real time the Host received it; see the program status extension below.                     |

Kind `1` is unassigned.

Progress state byte `0` removes the indicator, `1` is active, `2` is error, `3` is indeterminate,
and `4` is paused. `remove` and `indeterminate` require the null sentinel. `set`, `error`, and
`pause` accept either the null sentinel or a percentage. Duplicate progress extensions, unknown
states, percentages above `100`, and illegal state/value combinations reject the complete frame.
An omitted progress extension resolves to `remove`. Progress is current presentation metadata, not
lifecycle, process, notification, or completion authority.

Command-completion kind `4` stores three little-endian `u64` values — positive terminal-local
sequence, duration milliseconds, and positive Unix-epoch completion milliseconds — followed by one
`u8` exit code. Duplicate records, zero sequence, zero completion time, or a non-25-byte payload
reject the complete frame. The payload intentionally carries no command text, arguments, directory,
environment, or output. It is a recent presentation fact; each client owns freshness, focus,
permission, deduplication, rate limiting, and notification policy.

OBKG begins with this fixed 40-byte header:

| Offset | Bytes | Field                                                |
| ------ | ----: | ---------------------------------------------------- |
| `0`    |     4 | ASCII `OBKG`                                         |
| `4`    |     3 | Format major, minor, and patch `1.0.0`               |
| `7`    |     1 | Resource snapshot `1` or placement-only update `2`   |
| `8`    |     4 | Total OBKG bytes                                     |
| `12`   |     8 | Resource revision reused by placement-only updates   |
| `20`   |     8 | Monotonic Ghostty animation tick                     |
| `28`   |     2 | Image-record count; zero for a placement-only update |
| `30`   |     2 | Placement-record count                               |
| `32`   |     4 | Aggregate decoded pixel bytes                        |
| `36`   |     4 | Reserved zero                                        |

Each snapshot image is a 32-byte record followed immediately by its raw pixel bytes: image ID
`u32`, gray/gray-alpha/RGB/RGBA format `u8`, three reserved zero bytes, generation `u64`, width and
height `u32`, byte length `u32`, and reserved zero `u32`. Pixel length must exactly equal dimensions
times channels, image IDs are unique and nonzero, and all current placements must reference a
snapshot image. Each 56-byte placement records image and placement IDs, signed grid column and row,
pixel offsets, source crop, destination pixel size, signed `z`, and flags. Only the internal-ID flag
bit is defined. Every signed 32-bit z value is valid; composition uses exactly three ordered layers:
`z < -1073741824` below cell backgrounds, `-1073741824 <= z < 0` between cell backgrounds and text,
and `z >= 0` above text. Snapshot crop rectangles are bounded by their image.

The client retains at most 32 MiB across 256 visible decoded images and 1,024 visible placements.
A placement-only update carries no image bytes and must name the exact retained resource revision;
the consumer additionally validates every reference and crop against its retained image map before
committing any text row. A terminal snapshot with placement-only extension data is invalid. The
packaged TypeScript store and the native Rust projection apply the same consumer rules: either
rejects the whole frame for a malformed record, and each replaces its retained set atomically, so
an image that leaves the Host's set is released with the frame that omits it. Resource-set
or displayed-animation-generation change, retransmission, deletion, reset, screen change, or an
unconfirmed base promotes the Host's whole OBTF record to a snapshot. Direct, file, temporary-file,
and supported POSIX shared-memory inputs, raw/PNG/zlib/chunk transport, animation, virtual and
relative placements, cropping, signed-z composition, and historical intersection are resolved at
the native boundary. The pinned loader admits at most 400 MiB of encoded acquisition data, while
decoded storage and every projected resource set remain independently capped at 32 MiB. OBKG
carries only owned decoded pixels and resolved placements; it never
carries a path, URL, handle, shared-memory name or mapping, encoded container, compressed stream,
or unresolved Ghostty pin.

A snapshot has no base and contains every visible row in canonical order. A delta names the exact
acknowledged base and replaces complete, unique dirty rows in increasing row order; it never carries
partial cells. A delta may contain zero rows when no grid row is dirty while still carrying its
metadata and revision transition. Column count, grid-row count, cell-pixel width, cell-pixel height,
active-screen, or generation changes require a snapshot. The TypeScript decoder first validates and
scans all tables and rows, then allocates exact typed columns and reads
cells without constructing a JavaScript object per cell. The projected store stages a full snapshot
or every delta row before mutating the committed grid. Frame sequences are monotonically emitted,
but an abandoned frame may leave a gap in the committed client sequence. A delta must be the exact
next sequence and name the exact committed base. A complete snapshot may use any sequence greater
than the last committed sequence and atomically re-establish the client base across such a gap.
The fixed-header inspector validates only the bounded header and never admits cell, string, style,
row, or extension data. A capability-scoped local client may use that inspector before full decode
only to acknowledge a sequence it has already committed. This is sound because the Host delivery
owner makes the complete encoded record immutable when it hands off that scoped sequence. Every
higher OBTF sequence, and every frame arriving outside that trusted local delivery path,
still requires the full decoder before any payload data is exposed or committed. OBPB
projection buffers provide separately named strict-decode and local structural-view boundaries
below. A repeated sequence with different header commit metadata is a protocol-state conflict and
requires recovery. The explicitly trusted local duplicate path may rely on immutable commit metadata
without hashing the body; external and relayed consumers may instead fully decode a duplicate before
acknowledging it. If a full body decode fails after fixed-header validation and exact scope association,
`NeedSnapshot(decoder-rejected)` names that header's exact sequence. If the fixed header itself cannot
be validated or associated, the consumer fails the connection closed and never guesses a sequence.
For every pair of adjacent rows present in one frame, the prior row's wrap flag equals the next
row's wrap-continuation flag. A spacer-head is valid only in the final column of a wrapped row; if
the following row is present, it begins with the corresponding wide cell. These relationships are
validated before a frame is exposed or committed. Before a sparse delta mutates retained state, its
consumer additionally validates the complete candidate's wrap and spacer-head/wide topology across
each replaced row's retained predecessor and successor.

The native surface evidence set contains exported `ghostty-surface.bin`,
`ghostty-alternate-surface.bin`, `ghostty-render-row-window.bin`, and the Kitty graphics sequence
`ghostty-kitty-graphics-placed.bin`, `ghostty-kitty-graphics-scrolled.bin`, and
`ghostty-kitty-graphics-deleted.bin`. Terminal Host
generates the OBTF pair in sequence from one pinned native Ghostty VT and the real OBTF encoder. A
separate pinned terminal replays the primary corpus, adds DECSCA protected and unprotected cells,
and generates the render-detail OBRW only through `row_encoding.encode` and
`projection_buffer.encodeRenderSnapshot`; no TypeScript encoder constructs those bytes. A third
pinned 8x4 terminal at 8x16-pixel cells generates the graphics sequence from a direct Kitty
transmission: a resource snapshot, the placement-only delta after the image scrolls one row, and
the empty resource snapshot after deletion. The native Rust client consumes that sequence end to
end through its projection and TUI renderers. Producer conformance regenerates and byte-compares
all six. The TypeScript OBRW consumer checks strict and
trusted views against one contract-authored complete 6x8 matrix, commits the trusted frame through
`replaceProjectionBufferSnapshot`, and verifies wire-valid mutations for color section order, style,
semantic, protection, hyperlink, row-wrap, and width topology. This evidence detects native column
misassignment or omission, but does not change the format, validate PTY/history behavior, or grant
this package VT, renderer, lifecycle, or trusted-local-path authority.

The corpus's OSC 10/11 overrides appear as explicit foreground/background RGB in its cell colors,
including blank cells; the same effective values remain in the headers. Host projection retains
default sentinels only for channels without a program override, except that active DECSCNM emits
both swapped defaults explicitly. Explicit cell and palette RGB keep their exact values even when
equal to a default. Unmodified ANSI 0–15 use palette tags; child OSC 4 overrides remain RGB until
OSC 104 resets them. The current format versions remain unchanged.

## Current projection buffer

OBPB is the packed Host-to-render data plane. `decodeTerminalProjectionBuffer` is the
default boundary and strictly scans row, cell, color, string-range, UTF-8, and wide-cell invariants
before exposing typed views. A network peer, plugin, or persisted artifact must use that
strict boundary. `viewTerminalProjectionBuffer` is reserved for the immutable, same-user,
capability-scoped local Host path. It checks the fixed header, identities, counts, canonical section
directory, alignment and zero padding, total length, and the two small metadata strings, then creates
typed-array views over the same bytes. It deliberately does not walk every cell to revalidate values
already produced from canonical Ghostty state; cross-language conformance and producer tests
establish those column invariants.

`TerminalFrameSchema` validates equal string content once per parse and preserves an issue at every
invalid occurrence path.

The fixed header is 320 bytes. Bytes `0..159` contain `OBPB`, version `1.0.0`, snapshot/delta kind,
total/header lengths, columns, grid rows, included rows/cells, Host and attachment identities,
sequence/base/revision/generation, scrollback origin, cursor and pixel geometry, interaction flags,
default colors, nullable title/current-directory blob ranges, total string bytes, and the committed
Host terminal-theme revision. Bytes
`160..287` contain sixteen fixed directory entries of `{ u32 offset, u32 byteLength }`; bytes
`288..319` are reserved zero. Nullable metadata uses offset `0xffffffff`; all other string offsets
are relative to the final string-byte section.

All integers are little-endian. The fixed header is:

| Offset | Bytes | Field                                            |
| -----: | ----: | ------------------------------------------------ |
|      0 |     4 | Magic `OBPB`                                     |
|      4 |     1 | Format major `1`                                 |
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
|    144 |     1 | Format patch `0`                                 |
|    145 |     8 | Committed Host terminal-theme revision           |
|    153 |     7 | Reserved zero                                    |
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
snapshot contains every row in canonical order. A delta contains zero or more complete dirty rows in increasing
order and names the exact acknowledged base. Snapshot consumers retain the immutable buffer;
delta consumers retain only buffers still referenced by dirty row ranges. Shared memory is not part
of this version.

The language-neutral `v1/fixtures/packed-projection-conformance.json` is the generated, tracked evidence authority for shared OBPB and OBRW strict-admission cases. Its 23 OBPB cases contain the 17 canonical mutation classes, accepted sparse delta row indices `[0, 2]` with duplicate and descending rejections, and optional/required header-flag partitions; its eleven OBRW cases cover the equivalent header-flag partitions plus projection-section length, graphics-section length, and graphics-snapshot-kind failures, and five text-detail success witnesses: an identity-ASCII row with implicit cell mappings, one aggregate row whose total text exceeds 16 KiB while every per-cell span stays within the per-cell bound, an empty row, a three-row wrap chain, and a row whose text opens with U+FEFF. The fixture generator owns every complete canonical byte string and hard-codes each contract-authored outcome and success observation without asking either decoder for the oracle. `test:fixtures` rejects missing or byte-stale output. `decodeTerminalProjectionBuffer` and `decodeTerminalRowWindow` consume every matching case; the Host runner reads the language-neutral fixture path, supplies bounded corpus bytes, and makes both Zig strict validators consume the same cases while asserting only observations exposed by their public summaries.

The language-neutral `v1/fixtures/other-generation-conformance.json` is the generated evidence that
every boundary refuses another generation. It holds one canonical record per domain with only its
generation replaced by `2`. The OBTF, OBPB, OBRW, and OBKG cases expect `unsupported-version` from
the strict decoder; the OBTP case expects the envelope codec to report the peer version unchanged
for the receiver to refuse. The Rust, TypeScript, and Zig conformance suites consume every case.

## Current packed terminal row-window

`ReadRows` is scoped by attachment, observed revision, exact generation, positive request sequence,
requested row count, negotiated byte maximum, detail (`render` or `text`), purpose (`viewport`,
`search`, or `buffer`), revision policy (`exact` or `current-view`), and one origin:

- `absolute` names a retained absolute row.
- `oldest` names an unsigned offset from the oldest retained row.
- `tail` names how many rows remain below the requested viewport.
- `committed` applies a bounded signed delta to the attachment's last applied anchor.

`exact` requires the requested revision to equal the current Host revision and is valid for every
purpose and origin. `current-view` is valid only for a `viewport` request with `tail` or `committed`
origin. Its revision is an observation cursor rather than request identity: under the terminal mutex,
the Host resolves the origin against the then-current view and returns that captured current revision,
generation, and total-row count. Both policies require the exact current generation, so a resize or
screen-generation transition cannot reinterpret an old viewport request.

The Host resolves the origin once and returns a contiguous `RowsSnapshot`. At most one row request
is in flight per attachment and purpose lane; `viewport`, `search`, and `buffer` lanes progress
independently. Request sequences strictly increase for the attachment lifetime; a
request that fails before installation does not consume its sequence. A response is only a pending
anchor; the matching `RowsApplied` promotes it to the committed anchor, so a delayed acknowledgement
from an earlier row request cannot promote a later anchor. Resize invalidates pending generation-pinned row captures,
but a committed tracked anchor may follow a valid Ghostty reflow pin. Screen generation change, detach, or Ghostty page
eviction invalidates affected anchors. `HistoryChanged` coalesces while one event awaits its exact
acknowledgement. Every `HistoryChanged` event and its `HistoryObserved` acknowledgement carry a
positive attachment-local event `sequence` that strictly increases for the attachment lifetime and
is never reused; a `HistoryObserved` echoes the exact `sequence` of the event it acknowledges. A
stale or duplicate acknowledgement whose `sequence` is older than the currently in-flight event is
an idempotent no-op and never clears a newer flight, while the revision/generation identity remains
validated for the exactly matching flight.

Every row window begins with this fixed 112-byte little-endian header:

| Offset | Bytes | Field                                                                 |
| ------ | ----: | --------------------------------------------------------------------- |
| `0`    |     4 | ASCII `OBRW`                                                          |
| `4`    |     1 | Format major `1`                                                      |
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
| `100`  |     1 | Format patch `0`                                                      |
| `101`  |     4 | Render-detail OBPB projection length; zero for text detail            |
| `105`  |     4 | Render-detail OBKG graphics length; zero for text detail              |
| `109`  |     3 | Reserved zero                                                         |
| `112`  |     N | Projection then graphics for render detail, or text rows              |

Render detail concatenates one OBPB snapshot and one complete OBKG graphics snapshot. The OBPB
identity, sequence, revision, generation, columns, rows, and scrollback-row origin agree with the
outer window. Every graphics placement intersects the selected
row interval and is rebased to the window; every referenced image is included. Text detail requires
both section lengths to be zero and stores each row as a 12-byte header followed by
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

Pointer selection gestures carry the attachment's exact generation and bounded viewport-relative
numeric row/column endpoints; they do not carry revision identity. Gesture `start` carries the
browser-classified click count, integer surface position, and whether a third click selects semantic
output; `update` carries the current surface position, rectangle intent, and bounded cell geometry;
`end` carries only its viewport point; and `cancel` carries no point or action-specific fields. Under
the terminal mutex, the Host resolves each point at processing time against the attachment's
committed tracked history anchor while it is viewing history, or against the current active viewport
otherwise. Ordinary output revision changes therefore do not cancel a drag; generation mismatch,
screen replacement, or anchor eviction does.

The Host uses those facts with pinned Ghostty selection behavior to own word, line, output-block,
rectangle, and within-cell endpoint semantics. `cancel` idempotently releases the pinned gesture
while preserving the latest selection, whereas only `end` represents an ordinary pointer release.
Published selection endpoints are absolute row/column pairs; active state includes only endpoints,
rectangle mode, and an optional 4 KiB preview. `none` and `evicted` are distinct so a client cannot
silently copy replacement content after scrollback loss or a screen change.

`SelectionCommand.set-range` and `select-output` name rows, so they retain exact revision and
generation identity. `clear` and `select-all` name none: like a pointer action they outlive ordinary
output, so only a generation mismatch rejects them and the Host does not compare the revision they
still carry.

`SelectionCommand.set-range` carries explicit inclusive absolute `start` and `end` cell endpoints
plus rectangle mode. It is the non-pointer path for assistive technology and other programmatic
selection; it does not synthesize click counts, drag thresholds, or pointer gesture state.

`SelectionExport` carries the literal `plain` format and an attachment-local export sequence that
strictly increases for the attachment lifetime; HTML and raw VT are not admitted request or response
values. An export that fails before installation does not consume its sequence. The Host first
returns `SelectionExportStarted`, then emits at most one raw `SelectionExportChunk` in flight. Each
chunk contains the attachment identity, export sequence, monotonic `u32` chunk sequence, final bit,
and at most 1 MiB of bytes. Its 32-byte header stores the raw 16-byte attachment identity, positive
little-endian `u64` export sequence, little-endian `u32` chunk sequence, final bit in flags byte
zero, and three zero reserved bytes; the remaining opaque body may be empty and is not implicitly
UTF-8. `SelectionExportChunkApplied` advances only the matching export, so a
delayed acknowledgement from an earlier export cannot advance a later stream. Completion or explicit
cancellation releases all export storage. Total formatted output is capped at 16 MiB. Selection-
export clipboard writes and other platform effects remain outside this protocol package and the
Host; the separately defined OSC 52 request carries intent only and still grants no platform effect.

## Hard bounds

The handshake may select smaller transport payload bounds. It never raises these package hard limits.

| Input or retained unit                 |       Hard maximum |
| -------------------------------------- | -----------------: |
| Control JSON                           |             64 KiB |
| One OSC 52 clipboard text              |             16 KiB |
| Launch-overlay entries / one value     |         16 / 4 KiB |
| One terminal application signal        |              4 KiB |
| Terminals in one inventory page        |                128 |
| One raw, text, or paste input body     |              1 MiB |
| One `PasteAutoInput` content body      |            256 KiB |
| One `PromptLineInput` content body     |            256 KiB |
| One key text                           |              4 KiB |
| `WriteInput` raw payload               |       1 MiB + 24 B |
| `PasteAutoInput` raw payload           |     256 KiB + 32 B |
| `PromptLineInput` raw payload          |     256 KiB + 32 B |
| Largest semantic input payload         |       1 MiB + 32 B |
| Pending ordered input per terminal     |              4 MiB |
| Scrollback configuration per terminal  |    1,000,000,000 B |
| PTY read batch                         |            256 KiB |
| Packed frame                           |             36 MiB |
| All frame extensions / one OBKG record |             33 MiB |
| Decoded Kitty resources                |             32 MiB |
| Kitty images / placements              |         256 / 1024 |
| Transport record                       |      44 MiB + 64 B |
| Row-window record                      |             44 MiB |
| Search query at a client boundary      |             16 KiB |
| Selection preview                      |              4 KiB |
| Selection export / one chunk           |     16 MiB / 1 MiB |
| `SelectionExportChunk` raw payload     |       1 MiB + 32 B |
| One UTF-8 string                       |             16 KiB |
| Strings / styles / row records         | 4096 / 1024 / 4096 |
| Cells / frame extensions               |        262144 / 64 |
| Total extension bytes                  |             33 MiB |
| Grid columns / rows                    |        4096 / 4096 |
| Cell-pixel width / height              |        4096 / 4096 |
| Total terminal pixel width / height    |      65535 / 65535 |

`One UTF-8 string` bounds each serialized string and, for text row windows, each logical cell's
byte span: the bytes from a mapping start to the next mapping start, and the final span to row
text end, must fit in 16,384 bytes. Aggregate row text is not capped by this per-cell bound.

Counts and lengths are checked before dependent allocation. Integer overflow, inconsistent lengths,
truncation, invalid UTF-8, bad table references, duplicate or out-of-grid rows, and unknown required flags are
typed failures.

Terminal, attachment, and connection totals have no protocol or product hard maximum. Allocation,
process, PTY, identifier-space, or operating-system exhaustion rejects the current operation; it
does not authorize truncating inventory or deleting retained state.

## Consumer-owned behavior

A valid protocol value is not by itself a legal domain transition. The state owner decides whether
the value is admissible, performs effects, commits state, and determines retry behavior:

| Consumer      | Authoritative behavior and evidence                                                                                                                                                                                                                                                           |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Terminal Host | Terminal membership and lifecycle, process/PTY ownership, runtime-failure containment, attachment capability state, ordered input, projection/history/selection delivery, batch commit, and Host shutdown. See [the Host runtime model](../../../apps/terminal-host/docs/runtime-model.md).   |
| Worker        | The Host's local broker for one Session: Host lifetime, coupled peer liveness, and terminal-close escalation. The Server above it owns product binding and durable intent. See the [Worker](../../../apps/orchbee-worker/README.md) and the [Server](../../../apps/orchbee-server/README.md). |
| Renderer      | Presentation and application of already validated projection/history values; it does not acquire Host lifecycle authority.                                                                                                                                                                    |

Consumer models may use protocol message names and schemas as refinement points, but they live with
the consumer that owns the modeled state. This package's evidence is intentionally limited to wire
shape, exact-version handling, safety bounds, deterministic codecs, malformed-input rejection,
generated fixture integrity, and cross-language conformance. It does not claim process, delivery,
recovery, scheduling, or product-policy correctness.

Packed OBTF and OBPB geometry preserves unknown cell pixels as `(0, 0)`, matching the Host and
control geometry contract. Otherwise both dimensions are positive; mixed zero/positive pairs are
invalid. Grid and pixel-extent bounds remain unchanged. A terminal snapshot never invents pixel
dimensions merely to export text or styles.

## Correlated command observation extension

Optional OBTF extension kind `5` contains a positive output-origin epoch (`u64`), reader-ready (`u8`, 0/1), and invocation phase (`u8`, 0 absent, 1 running, 2 completed). An absent invocation is exactly 10 bytes. Otherwise its 66-byte payload appends: nonzero 16-byte invocation ID; start row `u64`, start column `u16`, end row `u64`, end column `u16`, positive start generation `u64`, positive start output epoch `u64`, and signed `i32` status, all integers little-endian. A running invocation has zero end coordinates and status. Duplicates, illegal phases/booleans, zero identities/epochs/generations and incorrect lengths reject the frame in consumers that understand this extension.

The observation is the latest cooperative Shell invocation, with no command text. It does not replace kind 4's presentation metadata. Coordinates belong to retained physical rows only while generation and output epoch still agree. The Host changes output epoch when its oldest retained-row pin loses continuity; clients must never infer continuity from a saturated row count. Older consumers skip this optional extension. The Windows CLI combines it with terminal identity and Host epoch to fence opaque output cursors.

Optional OBTF extension kind `6` is exactly 8 bytes: a nonzero little-endian `u64` of real-time Unix nanoseconds read after the Host received that invocation's start marker. It annotates the invocation in the same frame's kind `5` and appears only when the Host read its clock. Because the Shell writes the marker before running the command, a process created later than this time started after the marker. Duplicates, zero, incorrect lengths, or a frame whose kind `5` carries no invocation reject the frame in consumers that understand this extension; older consumers skip it.

## Program status extension

Optional OBTF extension kind `7` is exactly 9 bytes: a state byte (`1` idle, `2` working, `3`
blocked, `4` done, `5` error) followed by a nonzero little-endian `u64` of real-time Unix
nanoseconds read after the Host received the report. It carries the root record of the
[OSC 7501 Program Status protocol](https://www.superlogical.com/rex/docs/build/program-status), a
program's report about itself, and is present only while that record exists. Records addressed by
an `id` and the display fields `app`, `msg`, and `title` are not carried. The Host removes the
record on `state=clear`, RIS, and its reset control; a new shell prompt (OSC 133 `A`, `N`, or a
primary `P`) removes `idle`, `working`, and `blocked` but keeps `done` and `error`. Switching
between primary and alternate screens has no effect. Because the program writes the report before
the Host reads the clock, a process created later than this time did not send it. Duplicates,
unknown states, zero, or incorrect lengths reject the frame in consumers that understand this
extension; older consumers skip it. The record is a program's claim, not process, lifecycle, or
Agent identity authority.
