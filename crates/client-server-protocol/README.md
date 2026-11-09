# Client-Server Protocol Rust Binding

## Transient terminal clear and reset

The additive `reset` control action has the same shape and admission as `clear` below. Its `reset-accepted` outcome carries the same three fields as `cleared`, and the Worker sends the Host's full reset in place of the prompt-preserving clear.

The additive `clear` control action carries only an existing viewer `attachmentId`. It does not acquire or transfer a controller. A `cleared` outcome carries positive decimal `generation` and `revision`, and `cleanupConfirmed`; unknown delivery uses the existing `outcome-unknown` class. Other outcomes retain their closed meanings. The Server owns Session-exposure, physical-binding and Worker-generation fencing and fresh transient attachment identity; the Worker owns input-only attach → prompt-preserving Host clear → detach. [Native clear](../../apps/orchbee/docs/native-cli-query.md#clear-terminal-output) owns public command, deadlines and receipts. Matching development cohorts are required.

## Scope

`orchbee-client-server-protocol` owns the native OBCS v1 Rust binding. The language-neutral
v1 schema and JSON/binary corpus live under [`schemas/client-server`](../../schemas/client-server/README.md). The crate owns
validated Rust values, strict direction-specific JSON decoding, deterministic encoding, and bounded
frame I/O. It has no dependency on the OBLC local admission binding, the OBCN Server-Worker
authorization binding, Terminal Host types, or Workbench types.

The crate does not connect a transport, authenticate a Client, authorize a request, choose a Worker,
store request identity, execute a terminal effect, cancel work, reconcile an unknown outcome, or
project Workbench state.

## Terminal control result pairing

`TerminalControlAction::admits` owns control action/result pairing. Each action admits its own
success plus rejection, unavailability, or an unknown outcome; Resize also requires an exact
geometry echo. The action match is exhaustive, so adding an action requires declaring its
admissible outcomes. Callers validate request correlation and attachment/controller identities
separately.

## Public API

| API                                                                                                                                                    | Contract                                                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `RequestId`, `SessionId`, `WorkerId`, `TerminalId`, `ViewerId`, `HostEpoch`, `ControllerId`, `PresentationId`, `PanelId`, `TabId`, `TerminalContextId` | Parse separately typed nonzero opaque 128-bit lowercase hexadecimal identities.                                                                                                                        |
| `WorkingDirectory`                                                                                                                                     | Validate an absolute local-platform UTF-8 path of at most 4,096 encoded bytes with no NUL.                                                                                                             |
| `Geometry`                                                                                                                                             | Construct complete geometry only when all scalar and product bounds hold. Cell-pixel width and height are either both positive (pixels known) or both zero (pixels unknown); a mixed pair is rejected. |
| `TerminalCreateRequest`, `TerminalStartRequest`                                                                                                        | Construct validated Client requests; start rejects equal start/create request identities.                                                                                                              |
| `CreateOutcome`, `StartOutcome` and result types                                                                                                       | Represent only the stable Client-visible result combinations admitted by OBCS v1.                                                                                                                      |
| `encode_client_frame`, `decode_client_frame`                                                                                                           | Encode or decode exactly one complete in-memory Client-to-Server frame.                                                                                                                                |
| `encode_server_frame`, `decode_server_frame`                                                                                                           | Encode or decode exactly one complete in-memory Server-to-Client frame.                                                                                                                                |
| `read_client_message`, `read_server_message`                                                                                                           | Read exactly one frame from a partial-read-capable ordered stream and leave a following frame unread.                                                                                                  |
| `write_client_message`, `write_server_message`                                                                                                         | Write a complete frame through partial writes without flushing the caller.                                                                                                                             |

All decoded objects and enums are closed. Missing explicit nullable create fields, unknown fields or
tags, duplicate keys, BOM, invalid UTF-8, trailing JSON values, direction mismatches, invalid
identities, paths, or geometry, and JSON frames outside the exact length or 63 KiB payload bound are
rejected. Protocol and geometry numbers use the schema-owned finite binary64 safe-integer
refinement, so equivalent integral decimal and exponent spellings decode to the same value.
Canonical encoding uses compact deterministic JSON with ordinary integer tokens.

`TerminalControlAction::HostColors` carries effective default foreground/background, a required
nullable cursor, and exactly sixteen nullable ANSI RGB entries. Null records an unobserved outer
slot; the Worker keeps its fallback for that slot. Confirmed named palettes report every slot even
when the outer terminal never answers. The language-neutral
[controller query contract](../../schemas/client-server/README.md#controller-query-colors) owns
validation, authorization and the in-place version-1 shape.

## Viewer binding

`TerminalViewerAction::admits` exhaustively pairs each viewer action with its response class.
Record payload and target validation remain with the consumer that owns their interpretation.

`TerminalViewerRequest`, `TerminalViewerAction`, `TerminalViewerResult`, and `TerminalViewerOutcome`
provide session-scoped read-only Attach, AttachBinding, Poll, FrameApplied, NeedSnapshot, and Detach
values. AttachBinding carries separately typed OBCS `SessionId`, `WorkerId`, and `TerminalId` values;
equal encoded bytes across those domains remain valid. The crate validates their lexical contract
but neither imports Workbench types nor decides whether a tuple is currently exposed. Server owns
that semantic resolution. `ViewerUnavailableReason::WorkbenchUnavailable` is available only in a
viewer result.

Poll, FrameApplied, NeedSnapshot, and Detach require a typed `attachment_id: ViewerId`; Attach and
AttachBinding cannot carry it. `TerminalViewerAction::attachment_id()` returns the existing-viewer
selector, or `None` for the two attachment actions. Server validates ownership within the submitting
Session; the binding only validates identity syntax.

`ViewerId` and `HostEpoch` represent Server-issued attachment scope. Attached also requires the
actual `HostTerminalId`; native projections use epoch plus terminal identity to reject paint proofs
for another terminal in the same Host. A matched development cohort is required. Feedback counters use canonical
u64 decimal strings; the JSON corpus tests every action, outcome, reason, closed field set, and
identity boundary. Record results retain the schema-owner's OBSR binary framing and
`MAXIMUM_VIEWER_RECORD_BYTES` bound; the same Server frame functions dispatch either JSON control or
an opaque binary record. Client-direction readers reject OBSR. AttachBinding adds no OBCN or OBSR
type. This crate does not decode OBTP or terminal presentation; Server and the eventual renderer own
their respective routing and presentation validation. Large-record tests cover byte equality,
partial reads, following-frame alignment, direction, and invalid header/length/identity rejection.

### History viewport actions (unpublished)

Five additive action kinds extend the viewer surface for the native history viewport:
`set-projection-demand`, `set-history-demand`, `read-viewport-rows`, `rows-applied`, and
`history-observed`. They remain an unpublished contract requiring an exact development cohort; no
fallback shape is defined. Each carries `attachmentId` inside its own variant, matching the
existing attachment-scoped actions.

`set-projection-demand` carries `demand` (`"active"` or `"paused"`); `set-history-demand` carries
`demand` (`"following-bottom"` or `"viewing-history"`). Their Host acknowledgements echo only the
attachment and demand pair, so callers match them by correlation plus echoed demand — there is no
generation field to compare.

`read-viewport-rows` carries an `origin` object (`{"kind":"tail","rowsBefore":u32}`,
`{"kind":"committed","delta":i32}` or `{"kind":"absolute","row":"u64"}`), `rowCount`
(`1..=4096`), `revision`, `generation`, `requestSequence` (positive u64 decimal strings), and
`maximumBytes` (`112..=8,388,608`, the same OBRW payload bound `read-rows` uses). For a tail or
committed origin the Host serves the current view: the returned revision becomes the window's
captured identity, so the binding does not require revision equality with the request. An absolute
origin, which a GUI sends to reveal a find match in history, names a row of the request's revision,
so the Host answers `rows-rejected` with `stale-revision` once the history has moved. `rowsBefore` and `delta`
are bounded JSON integers (`u32`/`i32` domains, both inside the JavaScript safe-integer range);
`delta` is signed natively and admits negative scroll deltas. `row` is a canonical decimal string
like the other u64 fields.
The optional `detail` is `text` (default, omitted in canonical encoding) or `render`. Server
forwards that exact detail and rejects a response of the other kind. Interactive history requests
render; existing text requests preserve their bytes. Upgrade or roll back complete cohorts.

`rows-applied` carries `requestSequence`, `generation`, and `revision` — the exact tuple the Host
promotes from a pending viewport anchor to committed. `history-observed` carries `sequence`,
`generation`, and `revision` — the exact acknowledgement tuple for one delivered history-changed
event. Both use positive u64 decimal strings for sequence and generation and a canonical u64
decimal string for revision.

Five additive viewer action kinds carry Host-owned selection and plain-text export without adding
Client-side screen authority: `selection-gesture`, `selection-command`, `selection-export`,
`selection-export-chunk-applied`, and `selection-export-cancel`. Every action names the exact
`attachmentId`. Gesture and command generation and export sequence are positive canonical u64 text;
chunk sequence is `u32`. Gesture actions are the closed flat set `start`, `update`, `end`, and
`cancel`. Viewport points use columns and rows `0..=4095`, click count is `1..=3`, pointer axes are
`-65535..=65535`, and update geometry requires cell width and columns `1..=4096` plus a positive
`u16` screen height. Commands are the closed flat set `clear`, `select-all`, `set-range`, and
`select-output`; each carries a canonical u64 `revision`, and their points name a column `0..=4095`
and an absolute retained row as canonical u64 text. `plain` is the only export format; cancel
reasons are `clipboard-failed`, `disposed`, `replaced`, and `user`. Unknown fields and open
discriminants fail at the OBCS boundary.

The binding carries controls and opaque records only. A gesture's or command's `SelectionChanged`,
an export's `SelectionExportStarted`, and a cancel's `SelectionExportFinished` remain unchanged
record outcomes; the exact chunk-applied echo maps to `FeedbackAccepted`. Poll-delivered selection
changes, raw export chunks, and export-finished events continue through OBSR without interpreting
chunk bytes. Clipboard writes, gesture scheduling, export assembly, and retry policy remain Client
responsibilities. Existing action JSON stays byte-for-byte canonical, and the schema corpus covers
all four gesture branches, all four commands, the three export controls, hard bounds, closed enums,
and extra-field rejection. Matching Client/Server/Worker cohorts ship and roll back together.

`ViewerRejectedReason::HostRejected` (`"host-rejected"`) is a closed viewer-level reason shared by
the four history control-acknowledgement actions and the five selection/export controls above when
Host returns an explicit typed failure. It is distinct from the controller-plane
`ControlRejectedReason::HostRejected` and does not apply to older actions. `read-viewport-rows`
failures ride the existing `RowsRejected` nine-reason vocabulary; `read-rows` is unchanged
byte-for-byte.

## Controller binding

`TerminalControlRequest`, `TerminalControlAction`, `TerminalControlResult`, and `TerminalControlOutcome`
carry explicit Acquire and grant-scoped Input/Resize. `ControllerId` is a Server-issued identity;
all three actions require `attachment_id: ViewerId`, exposed by
`TerminalControlAction::attachment_id()`. This is a required unpublished OBCS v1 shape change;
Client/Server/Worker must use an exact development cohort with no implicit-viewer fallback.
`ControlInput` validates 1 through `MAXIMUM_CONTROL_INPUT_BYTES` (4,096) bytes and serializes canonical
lowercase hex. Shared number adapters validate positive decimal u64 sequences. Acquire, Input, and
Resize use the existing 63 KiB OBCS JSON framing.

`Paste` is the semantic auto-mode paste action. It carries `PasteContent`, an opaque 1 through
`MAXIMUM_PASTE_CONTENT_BYTES` (262,144) byte body, and never appears in OBCS JSON: the client frame
codec encodes it as the dedicated `OBCQ` record — four-byte magic, big-endian version 1 (`u16`),
zero reserved field (`u16`), and big-endian body length (`u32`), then request ID (16), attachment
ID (16), controller ID (16), sequence (8), and the content. A complete `OBCQ` record is 68 through
262,212 bytes. The codec validates the complete 12-byte header and bound before allocating payload,
consumes exactly one record, and preserves the following frame boundary; server-direction readers
dispatch either OBCS JSON or `OBCQ`, while client-direction readers reject `OBCQ`. A `Pasted`
outcome echoes the positive `accepted_bytes` count the Host reported. In addition to the shared
rejection reasons, paste maps the Host typed failures `host-input-invalid`,
`host-input-limit-exceeded`, `host-input-backpressure`, and `host-input-unavailable`.

The schema-owner's corpus checks each action/outcome/reason and malformed, maximum, and excessive
inputs. This binding grants no controller authority and never retries an unknown effect.

## Send binding

`TerminalSendRequest`, `SendRecords`, `TerminalSendResult`, and `TerminalSendOutcome`
carry one Server-authorized non-interactive input submission to an exposed Workbench terminal.
The request is a flat `requestId` + `sessionId`/`workerId`/`terminalId` binding triple + closed
record sequence; the triple is the declared target the Server resolves at authorize time, not an
incarnation lock. The binding grants no controller, viewer, resize, or lease authority, consumes
no attachment slot, and never rebinds across a restart: a target that changed worker or Host
incarnation after admission fails or reports an unknown outcome instead of redirecting.

`SendRecords` contains 1–64 ordered `SendRecord` values: text, semantic stroke, opaque bytes or Host-bracketed paste. Text and its optional folded CR together fit 4,096 bytes; opaque bytes fit 1 MiB and paste content fits 256 KiB. The sequence's total decoded content fits 1 MiB; keys contribute no decoded bytes. Every key uses `SendKeyStroke`, including Control-modified characters. Named identities are 1–13; Insert, F1..F25, non-control Unicode scalars and Control/Alt/Shift/Super modifiers use the same stroke encoding. The [native send contract](../../schemas/client-server/README.md#current-native-contract) owns canonical binary layouts. A sequence is not atomic.

Every `TerminalSendOutcome` carries confirmed prefix `progress` and orthogonal `cleanup`. Accepted records never exceed the first stopped index; accepted input requires an equal nonzero count/index and cleanup other than not-needed. Any nonzero prefix excludes not-needed cleanup; unknown input excludes confirmed cleanup. `acceptedEncodedBytes` sums the Host-encoded queue counts (zero is legal for mode-ignored keys). Acceptance means queue admission, not child execution. Rejection and unavailability reasons are closed sets; a partial or unknown result never authorizes automatic replay.

`TerminalSendResult` also carries `target`: the `SendTarget` physical incarnation — worker
generation, Host epoch, and Host terminal identity — the Server actually admitted for the send.
The `target` field is always present in the wire shape and may be `null`, but `null` is only
legal for `rejected`/`unavailable` outcomes whose cleanup is `not-needed` — the pre-admission
refusals decided before any binding was resolved. Every `accepted`, `outcome-unknown`, or
`confirmed`/`unconfirmed`-cleanup result must record the actual admitted target; construction
and both codec boundaries enforce the rule identically, and a `rejected`/`unavailable` result
with `not-needed` cleanup may still carry the pinned target. Consumers read the recorded target
fields verbatim and never re-resolve them.

## Launch read binding

`TerminalLaunchReadRequest` (`terminal.launch.read`) and
`TerminalLaunchReadResult` (`terminal.launch.read.result`) carry one read-only launch-fact
lookup for a single terminal. The request is `requestId` + `terminalId` plus the mandatory
`protocolVersion`; it dispatches no Worker/Host RPC, attaches no viewer, holds no grant, and
never mutates — the Server resolves the target against its current committed Workbench
membership inside the coordinator lock and answers in one pass.

The `found` outcome carries the current routing row (`terminalId`, `workerId`, `boundTo`
presentation/panel/tab/context identities), the `sessionId`, the decimal-string
`workbenchRevision` the join observed, and a `launch` fact object. When the terminal was
launched through a named Launch Profile, `found` also carries an optional `launchProfile`
member `{profileId, catalogRevision}` — the provenance frozen when the launch was prepared;
the member is absent entirely for terminals launched another way, and it stays accurate even
if the profile was deleted after its selection was captured. `not-found` and
`unavailable{reason:"server-busy"}` are the only other outcomes; a session without a Workbench
answers `not-found`, including for orphaned retained records.

An explicit program also contributes optional `launchSelection:{exec,args}` to `found`.
It is the immutable, validated selection captured at creation, including a resolved named profile;
profile deletion and Panel movement do not rewrite it. Default-shell and `not-tracked` facts omit
the member. Present null, malformed selection and `not-tracked` with a selection are invalid.
The existing 128-argument/30,720-byte bounds apply. This read exposes no environment or authorization
data, performs no RPC and introduces no durable storage. Older messages may omit the member.

`launch.state` is the retained launch phase verbatim — `creating`, `awaiting-start`,
`starting`, `started`, `unknown`, or `unavailable` — plus `not-tracked` when no uniquely
attributable consistent coordinator record exists for the visible binding (a missing,
ambiguous, or inconsistent record is reported, never repaired). `not-tracked` is an
observation about retained facts, not the `unknown` phase and never process liveness;
`creating` likewise records a phase, not proof of dispatch. `workerGeneration` is a
positive decimal string and is `null` exactly for `not-tracked`; `geometry` is non-null
exactly for `started`; `createOutcome` echoes the retained `CreateOutcome` encoding and is
`null` for `creating` and `not-tracked`. All three members are always present so a missing
key is a malformed frame, while `null` carries the absence. The decoder enforces those
conditional-null rules together with the closed-dictionary and identity checks. No lease,
fence, key, or authorization internals appear in either direction.

## Request boundary

The crate models no replay lookup, historical duplicate detection, or runtime idempotency ledger.
`RequestId` is request/result correlation only; a Client generates a fresh value for every submitted
request and does not reuse it within one active Server authority and authenticated subject.
`terminalCreateRequestId` identifies a deferred create within that same scope, independently of the
particular Client Session transport. A Client submits a mutating intent at most once and must not
automatically resend it after timeout, EOF, Session loss, or caller abandonment. None of those
events encodes cancellation or proves that an effect did not commit; ambiguous completion is left
to a future read-only projection or observation boundary.

## Verification

Run from the repository root:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo fmt --all -- --check
CARGO_TARGET_DIR="$PWD/target" cargo check -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

The conformance tests consume the canonical cross-language corpus and cover frame boundaries,
strict payload decoding, construction invariants, partial I/O, and no-flush behavior. They do not
prove a transport, Server state machine, authorization policy, Worker or Host mapping, terminal
lifecycle, or end-to-end execution.

## Workspace creation boundary

Terminal creation is a Workspace-owned Workbench command. OBCS contains no `terminal.open` request/result or singleton Open identity. Clients select exact bindings from the canonical Workbench snapshot and use `attach-binding`; normal viewer/controller admission remains required. The closed schema and independent Rust decoder reject removed message kinds.

AttachBinding is an additive, unpublished OBCS v1 source contract. It retains the existing
frame version and limits. The installed exact runtime cohort is Client, Server, Worker, and Host;
this Rust source binding is a contract input rather than an installed cohort artifact. Before
publication, its source contract and the runtime cohort roll back together, with no persisted state
or migration.

## Native launch environment extension

The native OBCS binding accepts optional `environment` on `terminal.create`, encoded as ordered
`[name, value]` pairs. Empty environments omit this optional member.
`LaunchEnvironment` validates at construction and JSON decode: at most 32 pairs and 1,024 combined
UTF-8 name/value bytes, ASCII identifier names of 1..128 bytes, no NUL, no case-insensitive
duplicates, and no `ORCHBEE_*`, `TERM`, `COLORTERM` or `TERM_PROGRAM`. Empty values and embedded
`=` remain literal. Values are omitted from Debug output. The complete request, including this
overlay, participates in request identity. The Server converts it to the independent OBCN value.

The v1 schema/corpus covers launch selection, environment, profile provenance, send sequence progress, all four OBSN input record kinds (including semantic strokes), the idle-prompt precondition with its prompt-line shape, and the prompt refusals with their `promptState`. `TerminalSendRequest::new` builds an ungated send; `TerminalSendRequest::idle_prompt` builds the gated one and refuses records that are not one prompt line. Conformance verifies independently authored canonical bytes and negative cases. This establishes native OBCS agreement, not Desktop or Workbench schema parity.

Native v1 frames are bounded at 63 KiB before allocation. A combined maximum escaped cwd, executable/argv and environment test covers the request budget; the older 32 KiB bound cannot contain that valid combination.

## Terminal process reads (unpublished)

`TerminalViewerAction::ReadProcessInfo` (`read-process-info`) carries only the existing `attachmentId`. It uses the current viewer grant and returns an unchanged, correlated Host kind-79 process sample or kind-5 typed failure in a record outcome. The sample carries the Host's `launchKind` when that Host confirms prompts (`prompt-gated-input`). Server validates Host epoch, request correlation, attachment and physical terminal identity; it never supplies launch parameters as current process facts. The native `panel process-info` consumer interprets the typed Host result. This read creates no controller, input or resize grant, does not advance frame feedback, and never converts missing or changed process observations to empty success. Existing session deadlines and transient-viewer cleanup apply. Same-development-cohort deployment is required.

## Explicit controller takeover

`TerminalControlAction::Takeover` is the closed OBCS v1 `takeover` action with only `attachmentId`.
It shares Acquire's Acquired result and rejection/unavailability/unknown classes. The
[language-neutral contract](../../schemas/client-server/README.md#session-controller) owns semantics;
[Server's transfer model](../../apps/orchbee-server/docs/terminal-controller-takeover.md) owns
reservation, revocation and cleanup. This additive action requires a same-development cohort.

`Observe` and `Activate` publish current Viewer ownership and recent focus order. Activate records
focus and content geometry without acquiring control. The observing owner alone receives its private
grant, consumed sequence and acknowledged geometry. The language-neutral Session-controller
contract above owns the closed shape and bounds; Server owns promotion and cleanup.

Launch selections additionally cap worst-case JSON string-content bytes at 30 KiB: ASCII controls count as six, quotes and backslashes as two, and other UTF-8 bytes as one. This preserves the previous 2 KiB input domain while bounding the larger resume commands. Matching cohorts are required for the enlarged limits.

## Plugin terminal display environment

An explicit terminal launch may carry a separate `PluginTerminalEnvironment` (`pluginEnvironment` in OBCS). It admits only plugin ID/root/entrypoint/config/state/context, native binary path and optional Workspace/Tab/Pane display IDs: ten unique named entries, seven required, at most 24 KiB combined UTF-8. These are display data, not authority or plugin registration. The ordinary caller environment retains its existing limits and reserved-name refusal. Runtime base/scope and Worker/terminal identities remain owned by the Worker adapter. A plugin environment requires an explicit executable. OBCN launch kind 3 transports the explicit selection, ordinary environment and plugin environment under the 40 KiB frame limit. Server checks the complete encoded plugin launch before reserving a physical operation. Worker removes inherited known plugin display fields before applying this launch's values. Workbench plugin resolution, Pane association and Client placement remain separate integration responsibilities. Focused boundary and launch-fact tests cover this carrier; this is not a complete plugin Pane receipt.
