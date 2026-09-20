# Client-Server Protocol Rust Binding

## Scope

`orchbee-client-server-protocol` is the independent Rust binding for the language-neutral OBCS
version-1 contract under [`schemas/client-server`](../../schemas/client-server/README.md). It owns
validated Rust values, strict direction-specific JSON decoding, deterministic encoding, and bounded
frame I/O. It has no dependency on the OBLC local admission binding, the OBCN Server-Worker
authorization binding, Terminal Host types, or Workbench types.

The crate does not connect a transport, authenticate a Client, authorize a request, choose a Worker,
store request identity, execute a terminal effect, cancel work, reconcile an unknown outcome, or
project Workbench state.

## Public API

| API                                                                                                                                                 | Contract                                                                                                                                                                                               |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `RequestId`, `SiteId`, `WorkerId`, `TerminalId`, `ViewerId`, `HostEpoch`, `ControllerId`, `PresentationId`, `PanelId`, `TabId`, `TerminalContextId` | Parse separately typed nonzero opaque 128-bit lowercase hexadecimal identities.                                                                                                                        |
| `WorkingDirectory`                                                                                                                                  | Validate an absolute POSIX UTF-8 path of at most 4,096 encoded bytes with no NUL.                                                                                                                      |
| `Geometry`                                                                                                                                          | Construct complete geometry only when all scalar and product bounds hold. Cell-pixel width and height are either both positive (pixels known) or both zero (pixels unknown); a mixed pair is rejected. |
| `LaunchSelection`                                                                                                                                   | Construct or decode a validated explicit launch selection: a nonempty absolute UTF-8 executable path and at most 128 NUL-free argument strings totaling at most 2,048 UTF-8 bytes.                     |
| `TerminalCreateRequest`, `TerminalStartRequest`                                                                                                     | Construct validated Client requests; start rejects equal start/create request identities.                                                                                                              |
| `CreateOutcome`, `StartOutcome` and result types                                                                                                    | Represent only the stable Client-visible result combinations admitted by OBCS v1.                                                                                                                      |
| `encode_client_frame`, `decode_client_frame`                                                                                                        | Encode or decode exactly one complete in-memory Client-to-Server frame.                                                                                                                                |
| `encode_server_frame`, `decode_server_frame`                                                                                                        | Encode or decode exactly one complete in-memory Server-to-Client frame.                                                                                                                                |
| `read_client_message`, `read_server_message`                                                                                                        | Read exactly one frame from a partial-read-capable ordered stream and leave a following frame unread.                                                                                                  |
| `write_client_message`, `write_server_message`                                                                                                      | Write a complete frame through partial writes without flushing the caller.                                                                                                                             |

All decoded objects and enums are closed. Missing explicit nullable create fields, unknown fields or
tags, duplicate keys, BOM, invalid UTF-8, trailing JSON values, direction mismatches, invalid
identities, paths, or geometry, and JSON frames outside the exact length or 32 KiB payload bound are
rejected. Protocol and geometry numbers use the schema-owned finite binary64 safe-integer
refinement, so equivalent integral decimal and exponent spellings decode to the same value.
Canonical encoding uses compact deterministic JSON with ordinary integer tokens.

## Viewer binding

`TerminalViewerRequest`, `TerminalViewerAction`, `TerminalViewerResult`, and `TerminalViewerOutcome`
provide session-scoped read-only Attach, AttachBinding, Poll, FrameApplied, NeedSnapshot, and Detach
values. AttachBinding carries separately typed OBCS `SiteId`, `WorkerId`, and `TerminalId` values;
equal encoded bytes across those domains remain valid. The crate validates their lexical contract
but neither imports Workbench types nor decides whether a tuple is currently exposed. Server owns
that semantic resolution. `ViewerUnavailableReason::WorkbenchUnavailable` is available only in a
viewer result.

Poll, FrameApplied, NeedSnapshot, and Detach require a typed `attachment_id: ViewerId`; Attach and
AttachBinding cannot carry it. `TerminalViewerAction::attachment_id()` returns the existing-viewer
selector, or `None` for the two attachment actions. Server validates ownership within the submitting
Session; the binding only validates identity syntax.

`ViewerId` and `HostEpoch` represent Server-issued attachment scope. Feedback counters use canonical
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

`read-viewport-rows` carries an `origin` object (`{"kind":"tail","rowsBefore":u32}` or
`{"kind":"committed","delta":i32}`), `rowCount` (`1..=4096`), `revision`, `generation`,
`requestSequence` (positive u64 decimal strings), and `maximumBytes` (`112..=8,388,608`, the same
OBRW payload bound `read-rows` uses). The Host serves the current view: the returned revision
becomes the window's captured identity, so the binding does not require revision equality with the
request. `rowsBefore` and `delta` are bounded JSON integers (`u32`/`i32` domains, both inside the
JavaScript safe-integer range); `delta` is signed natively and admits negative scroll deltas.

`rows-applied` carries `requestSequence`, `generation`, and `revision` — the exact tuple the Host
promotes from a pending viewport anchor to committed. `history-observed` carries `sequence`,
`generation`, and `revision` — the exact acknowledgement tuple for one delivered history-changed
event. Both use positive u64 decimal strings for sequence and generation and a canonical u64
decimal string for revision.

`ViewerRejectedReason::HostRejected` (`"host-rejected"`) is a new closed viewer-level reason
reserved for the four control-acknowledgement actions' explicit Host typed failures. It is
distinct from the controller-plane `ControlRejectedReason::HostRejected` and does not apply to
older actions. `read-viewport-rows` failures ride the existing `RowsRejected` nine-reason
vocabulary; `read-rows` is unchanged byte-for-byte.

## Controller binding

`TerminalControlRequest`, `TerminalControlAction`, `TerminalControlResult`, and `TerminalControlOutcome`
carry explicit Acquire and grant-scoped Input/Resize. `ControllerId` is a Server-issued identity;
all three actions require `attachment_id: ViewerId`, exposed by
`TerminalControlAction::attachment_id()`. This is a required unpublished OBCS v1 shape change;
Client/Server/Worker must use an exact development cohort with no implicit-viewer fallback.
`ControlInput` validates 1 through `MAXIMUM_CONTROL_INPUT_BYTES` (4,096) bytes and serializes canonical
lowercase hex. Shared number adapters validate positive decimal u64 sequences. Acquire, Input, and
Resize use the existing 32 KiB OBCS JSON framing.

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

`TerminalSendRequest`, `TerminalSendPayload`, `TerminalSendResult`, and `TerminalSendOutcome`
carry one Server-authorized non-interactive input submission to an exposed Workbench terminal.
The request is a flat `requestId` + `siteId`/`workerId`/`terminalId` binding triple + closed
payload union; the triple is the declared target the Server resolves at authorize time, not an
incarnation lock. The binding grants no controller, viewer, resize, or lease authority, consumes
no attachment slot, and never rebinds across a restart: a target that changed worker or Host
incarnation after admission fails or reports an unknown outcome instead of redirecting.

`TerminalSendPayload` is exactly one of `text` or `key`. `text` carries a `SendTextInput`
pairing validated non-empty UTF-8 `SendText` with an `enter` flag; the carriage return folds into
the same payload, so one send is always exactly one Host input record. `SendText` accepts up to
`MAXIMUM_SEND_TEXT_BYTES` (4,096) bytes, and `SendTextInput::new` enforces the combined bound:
`text` + `enter` together must fit it — a 4,096-byte text is legal only without `enter`. The
fields are private so no public construction or decoding path can exceed the bound, and the wire
encoder re-checks it independently. `key` carries one `SendKey` from the closed 15-name set
(`enter`, `escape`, `tab`, `backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`,
`page-up`, `page-down`, `ctrl-c`, `ctrl-d`); modifiers are not expressible beyond the named
`ctrl-` variants, and arbitrary key sequences, multi-record atomicity, raw hex, and paste are
outside this contract.

Every `TerminalSendOutcome` variant carries the orthogonal `cleanup` axis:
`accepted`/`rejected`/`unavailable`/`outcome-unknown` each pair their status with
`not-needed`, `confirmed`, or `unconfirmed` attachment cleanup. Two pairs cannot arise from the
Attach → input → Detach sequence and are rejected at construction and at both codec boundaries:
`accepted` + `not-needed` (an accepted input implies a real attachment) and
`outcome-unknown` + `confirmed` (a confirmed release implies a completed exchange). `accepted`
echoes `acceptedBytes`, the Host-encoded input-queue count (`SendAcceptedBytes`, zero through
`Self::MAXIMUM`); zero is legal because a terminal mode may ignore a semantic key. Accepted means
the Host admitted the record to its input queue — never that a child process read, executed, or
completed it. Rejection and unavailability reasons are closed sets; `outcome-unknown` means the
record may have landed and must never be retried automatically.

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
presentation/panel/tab/context identities), the `siteId`, the decimal-string
`workbenchRevision` the join observed, and a `launch` fact object. `not-found` and
`unavailable{reason:"server-busy"}` are the only other outcomes; a site without a Workbench
answers `not-found`, including for orphaned retained records.

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

## Standalone opening binding

`TerminalOpenRequest`, `TerminalOpenResult` and `OpenOutcome` implement the unshipped
[standalone opening contract](../../schemas/client-server/README.md#standalone-terminal-opening).
The request carries only correlation and complete geometry; Opened returns the independent
Server-created reference retained by legacy viewer attachment. AttachBinding does not change the
Open result. The binding owns strict encoding and validation, while Server owns singleton selection,
creation and lifetime.
Canonical fixtures cover every open outcome and reject absent, extra, malformed and wrong-direction
messages through both the JSON schema and independent Rust decoder.

Open and AttachBinding are additive, unpublished OBCS v1 source contracts. They retain the existing
frame version and limits. The installed exact runtime cohort is Client, Server, Worker, and Host;
this Rust source binding is a contract input rather than an installed cohort artifact. Before
publication, its source contract and the runtime cohort roll back together, with no persisted state
or migration.
