# Client-Server Terminal Control Contract

## Authority and boundary

[`v1/client-server.schema.json`](v1/client-server.schema.json) and this document are the
language-neutral authority for version 1 of the Orchbee Client-Server terminal-control protocol
(`OBCS`). OBCS carries terminal semantic control between an authorized Client Session and Server
over any ordered byte stream. The Rust crate `orchbee-client-server-protocol` is an independent
binding and may not add values, omit invariants, or redefine field meaning.

OBCS has these adjacent boundaries:

| Boundary             | Responsibility                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| `OBLC`               | Same-user local Client connection admission and liveness only; it grants no terminal operation. |
| `OBCS`               | Carrier-neutral Client-to-Server terminal semantic requests and stable Client-visible results.  |
| `OBCN`               | Server-to-Worker placement authorization, operation, generation, lease, and fence facts.        |
| Workbench projection | Read-only authorized product projection; it is not a terminal-control writer.                   |

Version 1 admits `terminal.create`, `terminal.create.result`, `terminal.start`,
`terminal.start.result`, `terminal.viewer`, `terminal.viewer.result`, `terminal.control`, and
`terminal.control.result`, plus the unshipped `terminal.open` / `terminal.open.result` and viewer
`attach-binding` extensions. It defines no close, cancel, retry transport, authentication,
authorization policy, Server state store, Worker operation, Host launch, or Workbench mutation.

## Frame

| Field          | Encoding                                               |
| -------------- | ------------------------------------------------------ |
| Magic          | Four bytes `OBCS`                                      |
| Frame version  | Unsigned 16-bit big-endian integer `1`                 |
| Payload length | Unsigned 16-bit big-endian byte length, at most 32 KiB |
| Payload        | Strict UTF-8 JSON object with `protocolVersion: 1`     |

A decoder validates the complete eight-byte header before allocating payload storage, reads exactly
the declared payload, and rejects an invalid magic, unsupported frame version, payload over 32 KiB,
truncation, or extra bytes when decoding one complete in-memory frame. A stream reader consumes one
frame at a time so another complete frame may follow. Readers and writers must support partial I/O;
a frame writer does not flush its caller's stream.

The payload rejects a UTF-8 BOM, invalid UTF-8, duplicate object keys at any depth, a trailing second
JSON value, non-object roots, unknown fields, unknown message or enum tags, and a message in the
wrong direction. Whitespace, object-key order, and equivalent JSON string escapes are not semantic.
Numeric `protocolVersion` and geometry values refine through finite IEEE 754 binary64 safe integers
before their exact bounds are applied. Integral decimal and exponent spellings such as `1.0`, `1e0`,
and `8e1` are therefore admitted when their refined value is in range; values that remain
fractional after refinement, non-finite values, unsafe integers, and out-of-range values are
rejected. Canonical encoding is deterministic compact JSON and emits ordinary integer tokens.

## Identity, path, and geometry

`requestId`, `terminalCreateRequestId`, `siteId`, `workerId`, and `terminalId` are opaque, nonzero
128-bit values encoded as exactly 32 lowercase hexadecimal characters. Site, Worker, Terminal, and
request identities are distinct semantic domains, but their encoded bytes need not differ; each
field is validated independently. `requestId` correlates one submitted request with its result; it
is not a replay or Server-ledger identity. A Client generates a fresh `requestId` for every submitted
request and does not reuse it within one active Server authority and authenticated subject. This is
a Client producer obligation, not a promise that Server retains or searches request history. The
separate `terminal.start` invariant requires distinct `requestId` and `terminalCreateRequestId` values.
Its `terminalCreateRequestId` identifies the deferred create within that same authority and subject,
independent of the particular Client Session transport.

An explicit `cwd` is an absolute POSIX UTF-8 path whose encoded path value contains 1 through 4,096
bytes and no NUL. `/` is valid. OBCS does not normalize the path, resolve `.` or `..`, inspect the
filesystem, infer Git meaning, or establish that the selected Worker accepts it. `cwd: null` asks
the selected Worker boundary to resolve its authorized default; omission is invalid.

Every geometry contains all four integer fields and satisfies every invariant:

| Field or product                                       | Range                     |
| ------------------------------------------------------ | ------------------------- |
| `columns`, `rows`, `cellPixelWidth`, `cellPixelHeight` | 1 through 4,096 inclusive |
| `columns * rows`                                       | At most 262,144           |
| `columns * cellPixelWidth`                             | At most 65,535            |
| `rows * cellPixelHeight`                               | At most 65,535            |

## Client requests

`terminal.create` contains `protocolVersion`, `requestId`, explicit `workerConstraint`, explicit
`cwd`, and `startMode`.

| Field              | Admitted value                                                                  |
| ------------------ | ------------------------------------------------------------------------------- |
| `workerConstraint` | `null`, or `{ "kind": "exact", "workerId": ... }`                               |
| `cwd`              | `null`, or one valid absolute POSIX path                                        |
| `startMode`        | `{ "kind": "immediate", "geometry": ... }`, or exactly `{ "kind": "deferred" }` |

Deferred creation carries no geometry or fallback. Immediate creation carries one complete valid
geometry. A `terminal.create` request never supplies a Terminal identity, Worker generation,
operation, lease, fence, Host identity or epoch, executable, argument vector, environment or
overlay, Project, Workspace, Panel, Worktree, Client generation, profile, or configuration fact.

`terminal.start` contains `protocolVersion`, a new `requestId`, the distinct
`terminalCreateRequestId` that identifies the deferred create request, and one complete valid
`geometry`. It carries no terminal identity or fallback geometry.

## Server results

Every JSON result contains `protocolVersion`, the exact request's `requestId`, and one closed `outcome`.
Create/Start results expose no internal Terminal, Worker, operation, lease, fence, Host, or placement fact.

| Create outcome    | Closed detail                                                                                      |
| ----------------- | -------------------------------------------------------------------------------------------------- |
| `accepted`        | `state` is `awaiting-start`, or `started` with the accepted complete `geometry`.                   |
| `rejected`        | `not-authorized`, `worker-constraint-unsatisfied`, or `cwd-not-accepted`.                          |
| `unavailable`     | `no-compatible-worker`, `terminal-runtime-unavailable`, or `server-busy`.                          |
| `failed`          | `terminal-create-failed`.                                                                          |
| `outcome-unknown` | No additional field. The Client must reconcile rather than infer success or retry as a new effect. |

| Start outcome     | Closed detail                                                                                           |
| ----------------- | ------------------------------------------------------------------------------------------------------- |
| `accepted`        | The accepted complete `geometry` and `transition` of `started` or `already-started`.                    |
| `rejected`        | `not-authorized`, `terminal-create-unavailable`, `terminal-not-awaiting-start`, or `geometry-conflict`. |
| `unavailable`     | `terminal-runtime-unavailable` or `server-busy`.                                                        |
| `failed`          | `terminal-start-failed`.                                                                                |
| `outcome-unknown` | No additional field. The Client must reconcile rather than infer a transition.                          |

## Session viewer control and binary records

`terminal.viewer` contains `protocolVersion`, a fresh `requestId`, and one closed `action`. Each
admitted Client Session can own multiple read-only viewers on its existing ordered carrier. `attach` supplies only
`terminalCreateRequestId`; Server resolves an accepted terminal in the same authority and subject,
pins its existing Worker generation and Host epoch, and issues a fresh attachment identity.
`attach-binding` instead supplies exactly `siteId`, `workerId`, and logical `terminalId` obtained
from a canonical Workbench snapshot. It carries no Workbench revision, Worker generation, Host
identity, operation, lease, or Create reference.

The binding triple selects an existing terminal and grants no authorization. Server requires the
current active Site and exact Worker/Terminal pair to remain exposed by a canonical Workbench
presentation before resolving the exact retained terminal and its pinned Worker generation. Selection performs no Open,
Create, Start, launch, membership, rebind, navigation, input, resize, or controller effect. Target
validation has this precedence after the Server-owned Session capacity guard:

1. poisoned registry or Site ownership returns `server-busy`;
2. missing or retired Workbench returns `workbench-unavailable`;
3. wrong Site or an unexposed exact pair returns `terminal-unavailable`;
4. missing current Worker or a different logical Worker returns `terminal-runtime-unavailable`;
5. a missing exact retained record returns `terminal-unavailable`;
6. a different retained Worker generation returns `terminal-runtime-unavailable`;
7. a retained phase that cannot attach returns `terminal-unavailable`; and
8. attachment identity exhaustion or known unchanged capacity returns `server-busy`.

Workbench exposure and retained-target resolution linearize together. Later Workbench retirement
does not cancel the pinned operation. Attachment success linearizes at the existing post-IPC
current-Worker check; Worker loss after that check can race response delivery and is fenced by the
next operation. `poll`, `frame-applied`, `need-snapshot`, and `detach` require an in-action
`attachmentId`, using the nonzero 32-character lowercase hexadecimal identity format. It selects
only a Server-issued viewer owned by the submitting Session; it cannot create or transfer ownership.
`attach` and `attach-binding` forbid this field. Session loss detaches transient viewing state and never terminates the retained
terminal. The action changes only OBCS JSON; the existing OBCN relay, OBSR framing, and byte limits
remain unchanged.

`frame-applied` also carries `sequence`, `generation`, and `revision`; `need-snapshot` also carries
`sequence` and `reason`. These counters are canonical decimal strings in the unsigned 64-bit range, with no
leading zero or sign; sequence and generation are positive. Reasons are `activity-stale`,
`base-mismatch`, `decoder-rejected`, `dimensions-mismatch`, `generation-mismatch`, `identity-mismatch`,
`sequence-gap`, and `store-reset`. Only exact feedback for the selected viewer's outstanding frame
may reach its Worker attachment. Different viewers may have equal counters; the selector and that
viewer's frame scope determine validity. Poll while that viewer awaits feedback returns `empty`
without another frame and does not block a sibling viewer.
A new attachment requires a new baseline; viewer attachment grants neither input nor resize authority.

An unknown or unowned selector returns `terminal-unavailable` for Poll or feedback and effect-free
`detached` for Detach. Control returns `viewer-unavailable`. A same-Session attachment to the same
resolved live runtime target returns `already-attached`; different targets can coexist. The owning
[Server guide](../../apps/orchbee-server/README.md#accepted-session-viewer-routing-modeling-record)
defines capacity precedence, disposal, generation fencing, and cleanup. The codec validates syntax,
not runtime ownership.

This required selector changes the unpublished OBCS v1 request shape. Build, deploy, and roll back
the exact Client/Server/Worker development cohort together; mixed cohorts and an implicit-current
viewer fallback are unsupported. OBCN and OBSR wire formats do not change.

JSON viewer outcomes are `attached` with `attachmentId` and `hostEpoch` (nonzero 32-character
lowercase hex), fieldless `empty`, `feedback-accepted`, or `detached`, `rejected` with reason
`already-attached`, `terminal-unavailable`, or `feedback-mismatch`, and `unavailable` with reason
`server-busy`, `terminal-runtime-unavailable`, or viewer-only `workbench-unavailable`. Attached
answers Attach or AttachBinding; Empty or a binary record answers Poll; FeedbackAccepted answers
either feedback action; Detached answers Detach, including an already absent viewer. Rejected and
Unavailable can answer any action. Wrong kind or correlation retires the Client carrier. Only a
correlated `server-busy` result known to have committed no effect may use the existing bounded retry
policy: the Client retains identical action values, uses a fresh `requestId`, and stays within the
same logical deadline. Ambiguous failure never authorizes replay, fallback, or retargeting.

A record result uses binary `OBSR`, not JSON: four magic bytes, big-endian u16 version 1, two zero
reserved bytes, then big-endian u32 payload length. The payload is the exact request ID as 16
big-endian bytes followed by one opaque OBTP event record of 64 through `64 + 36 * 1024 * 1024`
bytes. The 12-byte header and bound are checked before payload allocation; stream readers consume
one frame, and in-memory decoders reject truncation or trailing bytes. Only Server-to-Client readers
accept OBSR. Server validates Host epoch, attachment, role, frame routing metadata and feedback;
presentation decoding remains the Client's responsibility. OBSR has no JSON `record` outcome and
does not enlarge the 32 KiB OBCS JSON limit. Its byte contract is this section; the JSON corpus owns
control messages, while binding tests cover binary boundaries and opaque byte preservation.

## Session controller

`terminal.control` contains `protocolVersion`, a fresh `requestId`, and one closed `action`.
Every control action requires in-action `attachmentId`. `acquire` has no other field and requires
that exact attached viewer in the submitting Session. Server explicitly grants one
combined input-and-resize controller per terminal, without takeover or automatic promotion. The
`acquired` result carries a fresh `controllerId`, using the nonzero 32-character lowercase hex
identity format. The grant is bound to that Session, viewer, Worker generation, and Host terminal.

`input` additionally contains `controllerId`, `sequence`, and `bytes`; `resize` additionally contains `controllerId`,
`sequence`, and complete `geometry`. Input bytes are 1 through 4,096 raw bytes encoded as an even,
lowercase hexadecimal string, with no prefix; they are not UTF-8 text, Base64, or executable metadata.
Both actions share a strictly increasing positive u64 sequence encoded as a canonical decimal
string. Gaps are allowed; an accepted submission consumes the sequence even if downstream delivery
fails or is rejected. Grant mismatch or stale sequence has no terminal effect. A new grant starts
with no consumed sequence; an old grant never regains authority by resetting its sequence.

`terminal.control.result` uses the ordinary correlated JSON envelope. Outcomes are `acquired`
with `controllerId`, fieldless `input-accepted`, `resized` with exact accepted `geometry`, fieldless
`outcome-unknown`, `rejected` with reason `viewer-unavailable`, `controller-busy`, `not-controller`,
`sequence-mismatch`, or `host-rejected`, and `unavailable` with reason `server-busy` or
`terminal-runtime-unavailable`. InputAccepted means complete admission to Host's bounded input queue,
not that the shell has executed it. Rejected, Unavailable and OutcomeUnknown may answer any action;
other outcomes must match the submitted action, and Resize must match its geometry.

Detach or Session loss releases the combined grant before retiring the viewer. There is no separate
Client release action in this slice. Pending acquisition and cleanup exclude competing grants;
cleanup can outlive the Client submission deadline. Unknown input/resize is never automatically
replayed, including after reconnect or a new grant. Definite rejection grants no retry policy.
The conformance corpus covers all control actions, outcomes, reasons, byte bounds and invalid scope
fields. Runtime ownership and Host admission remain with their owning Server and Worker guides.

## Submission, reference, and cancellation obligations

A Client submits each mutating intent at most once. OBCS version 1 defines no replay lookup,
historical duplicate detection, or idempotency ledger. A consumer may reject a request when its
current accepted state exposes an ID collision or otherwise makes the request invalid, but that
state validation is not a promise to retain, recognize, or replay a historical request.

Version 1 has no cancel message. A timeout, transport loss, Client Session loss, or caller
abandonment neither encodes cancellation nor proves that a terminal effect did not commit. A Client
must not automatically resend the mutating intent after any of those events; an ambiguous result is
reconciled through a future read-only projection or observation boundary instead.

## Conformance and limits

[`v1/client-server-conformance-v1.json`](v1/client-server-conformance-v1.json) is the deterministic
cross-language oracle. It includes every message, result category, reason, accepted-state variant,
and accepted-start transition, plus malformed and semantically invalid payloads. Repeated-value
recipes compactly materialize exact byte-boundary, multibyte, and worst-case JSON path inputs.
Bindings consume the corpus without deriving expected results from their own codec. AttachBinding
fixtures cover exact field names, required and additional fields, lexical identity failures, zero
sentinels, and wrong message/result kinds. Equal valid bytes across its separately typed identity
domains are canonical. Schema validation cannot determine whether a well-formed tuple names the
current exposed terminal; Server tests own that semantic refusal.

Run from the repository root:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

These checks establish the schema-to-binding corpus agreement and deterministic frame behavior.
They do not establish authentication, authorization decisions, replay lookup, duplicate-history or
idempotency storage, cancellation, terminal execution, persistence, recovery, or any live transport
integration.

## Standalone terminal opening

The unshipped version-1 extension `terminal.open` carries `protocolVersion`, a fresh
Client-generated `requestId`, and complete `geometry`. It opens the local single-subject
Server's standalone terminal reference: first prepare creates an immediate terminal using
Worker-default cwd; later opens return the same reference. Open itself neither dispatches nor commits Resize; a Client may subsequently attach, acquire controller authority, and submit a separate validated Resize.
This terminal wire contract adds no Workbench payload. The native Server owns the first shared
standalone membership commit before launch, as defined in its
[Workbench authority](../../apps/orchbee-server/docs/workbench-authority.md#standalone-terminal-membership-model);
Clients observe that structure through the separate Workbench protocol.

`terminal.open.result` correlates the exact calling request and returns one closed outcome:
`opened` with `terminalCreateRequestId`; `unavailable` with a Create unavailable reason;
`failed` with `terminal-create-failed`; or exactly `outcome-unknown`.
The returned reference may identify a Server-generated internal create. It is independent of the
caller's correlation ID and grants no viewer or controller authority: normal Session attachment
and controller admission still apply. AttachBinding does not change this result shape; the native
standalone flow observes the separate Workbench snapshot rather than treating this Create reference
as canonical selection state. Entropy failure or reference collision returns server-busy before
preparation and leaves the standalone slot empty. Once dispatched, failure or uncertainty does not
authorize another create. No Client retries an unknown effect; another open only observes the
already reserved standalone result.

The schema, Rust binding, and canonical corpus define the additive Open and AttachBinding contracts
together. They remain unshipped and change no frame version, byte limit, existing action encoding,
admission, or unknown-effect replay rule; legacy Attach remains available. The installed exact
runtime cohort is Client, Server, Worker, and Host. Schema and Rust source are contract inputs, not
installed cohort artifacts. Before publication, the unpublished source contract and runtime cohort
roll back together. This extension adds no persisted state or migration.
