# Client-Server Terminal Control Contract

## Transient terminal clear

The same-cohort additive `terminal-control` action `{kind:"clear", attachmentId}` is a one-shot prompt-preserving terminal mutation through the admitted viewer binding, independent of session controller acquisition. `cleared` reports canonical positive decimal generation/revision and a Boolean cleanup confirmation. Server validates current Site exposure, exact physical binding and Worker identity; Worker opens a fresh input-only Host attachment without resizing or transferring a controller. Known clears survive cleanup uncertainty, while missing effect acknowledgements remain unknown without retry. [Native clear](../../apps/orchbee/docs/native-cli-query.md#clear-terminal-output) owns the command and resource deadlines.

## Authority and boundary

[`v2/client-server.schema.json`](v2/client-server.schema.json) and this document are the
language-neutral authority for version 2 of the Orchbee Client-Server terminal-control protocol
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

Version 2 admits `terminal.create`, `terminal.create.result`, `terminal.start`,
`terminal.start.result`, `terminal.viewer`, `terminal.viewer.result`, `terminal.control`, and
`terminal.control.result`, plus the unshipped `terminal.open` / `terminal.open.result` and viewer
`attach-binding` extensions, `terminal.launch.read` / result and `terminal.send.result`. Send requests use OBSN binary records. It defines no close, cancel, retry transport, authentication,
authorization policy, Server state store, Worker operation, Host launch, or Workbench mutation.

## Frame

| Field          | Encoding                                               |
| -------------- | ------------------------------------------------------ |
| Magic          | Four bytes `OBCS`                                      |
| Frame version  | Unsigned 16-bit big-endian integer `1`                 |
| Payload length | Unsigned 16-bit big-endian byte length, at most 63 KiB |
| Payload        | Strict UTF-8 JSON object with `protocolVersion: 2`     |

A decoder validates the complete eight-byte header before allocating payload storage, reads exactly
the declared payload, and rejects an invalid magic, unsupported frame version, payload over 63 KiB,
truncation, or extra bytes when decoding one complete in-memory frame. A stream reader consumes one
frame at a time so another complete frame may follow. Readers and writers must support partial I/O;
a frame writer does not flush its caller's stream.

The payload rejects a UTF-8 BOM, invalid UTF-8, duplicate object keys at any depth, a trailing second
JSON value, non-object roots, unknown fields, unknown message or enum tags, and a message in the
wrong direction. Whitespace, object-key order, and equivalent JSON string escapes are not semantic.
Numeric `protocolVersion` and geometry values refine through finite IEEE 754 binary64 safe integers
before their exact bounds are applied. Integral decimal and exponent spellings such as `2.0`, `2e0`,
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

An explicit `cwd` is an absolute local-platform UTF-8 path whose encoded path value contains 1 through 4,096
bytes and no NUL. `/` is valid. OBCS does not normalize the path, resolve `.` or `..`, inspect the
filesystem, infer Git meaning, or establish that the selected Worker accepts it. `cwd: null` asks
the selected Worker boundary to resolve its authorized default; omission is invalid.

Every geometry contains all four integer fields and satisfies every invariant:

| Field or product                    | Range                                                                                                     |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `columns`, `rows`                   | 1 through 4,096 inclusive                                                                                 |
| `cellPixelWidth`, `cellPixelHeight` | 0 through 4,096 inclusive; either both positive (pixels known) or both zero (pixels unknown), never mixed |
| `columns * rows`                    | At most 262,144                                                                                           |
| `columns * cellPixelWidth`          | At most 64,512                                                                                            |
| `rows * cellPixelHeight`            | At most 64,512                                                                                            |

## Client requests

`terminal.create` contains `protocolVersion`, `requestId`, explicit `workerConstraint`, explicit
`cwd`, and `startMode`.

| Field              | Admitted value                                                                  |
| ------------------ | ------------------------------------------------------------------------------- |
| `workerConstraint` | `null`, or `{ "kind": "exact", "workerId": ... }`                               |
| `cwd`              | `null`, or one valid absolute local-platform path                               |
| `startMode`        | `{ "kind": "immediate", "geometry": ... }`, or exactly `{ "kind": "deferred" }` |

Deferred creation carries no geometry or fallback. Immediate creation carries one complete valid
geometry. A `terminal.create` request never supplies a Terminal identity, Worker generation,
operation, lease, fence, Host identity or epoch, Project, Workspace, Panel, Worktree, Client generation, profile, or configuration fact. Optional literal launch/environment values follow the version-2 contract below.

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
next operation. `poll`, `frame-applied`, `need-snapshot`, `detach`, `read-rows`,
`set-projection-demand`, `set-history-demand`, `read-viewport-rows`, `rows-applied`, and
`history-observed` require an
in-action `attachmentId`, using the nonzero 32-character lowercase hexadecimal identity format. It selects
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

`read-rows` performs one buffer read of retained history plus visible rows against the exact pinned
`generation` and `revision`, anchored at the absolute `startRow`. Alongside `attachmentId` it carries
`startRow` and `revision` as canonical decimal strings, `generation` and `requestSequence` as positive
canonical decimal strings, `rowCount` as an integer 1 through 4096, and `maximumBytes` as an integer
112 through 8,388,608 bounding the OBRW payload (not the carrier record). It is a single correlated
Host exchange with no acknowledgement, replay, or retry: a byte budget overrun is a typed failure,
never a partial success. Interleaved Host events still route to their owning attachments; they are
never mistaken for the response. A successful answer is the complete correlated OBTP response
record through the existing OBSR binary carrier — the same `Record` shape that answers Poll, with
response (not event) content.

`set-projection-demand` and `set-history-demand` each carry `attachmentId` and one `demand`:
`active` or `paused` for projection, `following-bottom` or `viewing-history` for history.
`read-viewport-rows` performs one buffer read of retained history plus visible rows anchored
relative to the Client's history position rather than an absolute row: `origin` is either
`{"kind":"tail","rowsBefore":N}` with `rowsBefore` an integer 0 through 4,294,967,295, or
`{"kind":"committed","delta":N}` with `delta` an integer -2,147,483,648 through 2,147,483,647.
Alongside `attachmentId` and `origin` it carries `rowCount` as an integer 1 through 4096,
`maximumBytes` as an integer 112 through 8,388,608 bounding the OBRW payload (not the carrier
record), `revision` as a canonical decimal string that may be stale, and `generation` and
`requestSequence` as positive canonical decimal strings. It is a single correlated Host exchange
with no acknowledgement, replay, or retry: the served answer's revision identifies the captured
window and is not an equality echo of the request revision, and Host may serve a clamped window.
`rows-applied` acknowledges the Client's applied window with positive canonical `requestSequence`
and `generation` plus canonical `revision`; `history-observed` acknowledges one history event with
`sequence`, `generation`, and `revision` under the same counter shapes. Interleaved Host events
still route to their owning attachments; they are never mistaken for the response.

An unknown or unowned selector returns `terminal-unavailable` for Poll, feedback, or ReadRows and
effect-free `detached` for Detach. Control returns `viewer-unavailable`. A same-Session attachment to the same
resolved live runtime target returns `already-attached`; different targets can coexist. The owning
[Server guide](../../apps/orchbee-server/README.md#accepted-session-viewer-routing-modeling-record)
defines capacity precedence, disposal, generation fencing, and cleanup. The codec validates syntax,
not runtime ownership.

This required selector changes the unpublished OBCS v2 request shape. Build, deploy, and roll back
the exact Client/Server/Worker development cohort together; mixed cohorts and an implicit-current
viewer fallback are unsupported. The OBSR carrier format does not change; `read-rows` /
`rows-rejected` additively extend the matched OBCN cohort (action tag 6 and result tag 6), and the
five viewport and history actions add OBCN action tags 7 through 11 the same way — an
older Worker rejects the new tags rather than silently ignoring them.

JSON viewer outcomes are `attached` with `attachmentId`, `hostEpoch` and the actual `hostTerminalId` (nonzero 32-character
lowercase hex), fieldless `empty`, `feedback-accepted`, or `detached`, `rejected` with reason
`already-attached`, `terminal-unavailable`, `feedback-mismatch`, or `host-rejected`,
`rows-rejected` with reason
`stale-revision`, `stale-generation`, `history-evicted`, `missing-resource`, `invalid-field`,
`invalid-acknowledgement`, `limit-exceeded`, `resource-exhausted`, or `invalid-phase`, and
`unavailable` with reason `server-busy`, `terminal-runtime-unavailable`, or viewer-only
`workbench-unavailable`. Attached
answers Attach or AttachBinding; Empty or a binary record answers Poll; FeedbackAccepted answers
either feedback action or one of the four control acknowledgement actions (`set-projection-demand`,
`set-history-demand`, `rows-applied`, `history-observed`); Detached answers Detach, including an
already absent viewer; a binary
record or RowsRejected answers ReadRows or ReadViewportRows, which never answer Empty. RowsRejected
answers only ReadRows or ReadViewportRows — on any other action it is a protocol violation — and a
legal RowsRejected does not
retire the viewer. `host-rejected` reports the Host's typed failure for one of the four control
acknowledgement actions only; the healthy viewer stays attached and usable after it. The new reason
widens the closed enum — it does not widen the set of pairings a consumer may produce. Rejected and
Unavailable can answer any action. Wrong kind or correlation retires the Client carrier. Only a
correlated `server-busy` result known to have committed no effect may use the existing bounded retry
policy: the Client retains identical action values, uses a fresh `requestId`, and stays within the
same logical deadline. Ambiguous failure never authorizes replay, fallback, or retargeting.

A record result uses binary `OBSR`, not JSON: four magic bytes, big-endian u16 version 1, two zero
reserved bytes, then big-endian u32 payload length. The payload is the exact request ID as 16
big-endian bytes followed by one opaque OBTP record of 64 through `64 + 36 * 1024 * 1024`
bytes — a Host event record for Poll, or the correlated Host response record for ReadRows or
ReadViewportRows. The
inner Host correlation is the Server-issued Worker operation identity; the Client `requestId` lives
only in this outer carrier and is never equated with it. The 12-byte header and bound are checked
before payload allocation; stream readers consume
one frame, and in-memory decoders reject truncation or trailing bytes. Only Server-to-Client readers
accept OBSR. Server validates Host epoch, attachment, role, frame routing metadata and feedback;
presentation decoding remains the Client's responsibility. OBSR has no JSON `record` outcome and
does not enlarge the 63 KiB OBCS JSON limit. Its byte contract is this section; the JSON corpus owns
control messages, while binding tests cover binary boundaries and opaque byte preservation.

## Session controller

`terminal.control` contains `protocolVersion`, a fresh `requestId`, and one closed `action`.
Every control action requires in-action `attachmentId`. `acquire` has no other field and requires
that exact attached viewer in the submitting Session. Server explicitly grants one
combined input-and-resize controller per terminal, without automatic promotion. Ordinary Acquire
refuses an occupied terminal; explicit `takeover` carries the same sole `attachmentId` field and
replaces an active incumbent. It refuses pending acquisitions/transfers. Server owns fresh grant
identities and fences the old grant before transfer; Worker acknowledges exact old Host detach
before new acquisition. A failed takeover may revoke the old controller without granting a new one,
never silently restoring authority. Rejected, Unavailable and OutcomeUnknown remain distinct;
unknown cleanup retains a fence. See the [takeover model](../../apps/orchbee-server/docs/terminal-controller-takeover.md).
This additive v2 action requires the same development cohort; older peers reject it. The
`acquired` result carries a fresh `controllerId`, using the nonzero 32-character lowercase hex
identity format. The grant is bound to that Session, viewer, Worker generation, and Host terminal.

`input` additionally contains `controllerId`, `sequence`, and `bytes`; `resize` additionally contains `controllerId`,
`sequence`, and complete `geometry`. Input bytes are 1 through 4,096 raw bytes encoded as an even,
lowercase hexadecimal string, with no prefix; they are not UTF-8 text, Base64, or executable metadata.
Both actions share a strictly increasing positive u64 sequence encoded as a canonical decimal
string. Gaps are allowed; an accepted submission consumes the sequence even if downstream delivery
fails or is rejected. Grant mismatch or stale sequence has no terminal effect. A new grant starts
with no consumed sequence; an old grant never regains authority by resetting its sequence.

The `paste` action is a semantic whole-envelope operation: it submits 1 through 262,144 opaque
content bytes under the same attachment, controller, and strictly increasing sequence, and Host
encodes them once under its current bracketed-paste mode. It is never carried by OBCS JSON: a
`"kind": "paste"` action inside `terminal.control` is invalid in both directions. The sole request
encoding is a binary `OBCQ` record on the Client-to-Server stream: four magic bytes, big-endian u16
version 1, two zero reserved bytes, then big-endian u32 body length. The body is the exact request
ID as 16 big-endian bytes, the attachment ID as 16 bytes, the controller ID as 16 bytes, the
positive sequence as big-endian u64, then the raw content bytes — 57 through 262,200 bytes in total.
The 12-byte header and bound are checked before body allocation; stream readers consume one record,
and in-memory decoders reject truncation or trailing bytes. OBCQ has no JSON `paste` action and does
not enlarge the 63 KiB OBCS JSON limit; a small paste uses the same record encoding.

`terminal.control.result` uses the ordinary correlated JSON envelope. Outcomes are `acquired`
with `controllerId`, fieldless `input-accepted`, `resized` with exact accepted `geometry`, `pasted`
with `acceptedBytes` (an integer from 1 through 1,048,576 counting the encoded bytes Host admitted
to its bounded input queue), fieldless `outcome-unknown`, `rejected` with reason
`viewer-unavailable`, `controller-busy`, `not-controller`, `sequence-mismatch`, `host-rejected`,
`host-input-invalid`, `host-input-limit-exceeded`, `host-input-backpressure`, or
`host-input-unavailable`, and `unavailable` with reason `server-busy` or
`terminal-runtime-unavailable`. InputAccepted and Pasted mean complete admission to Host's bounded
input queue, not that the shell has executed it. Rejected, Unavailable and OutcomeUnknown may answer
any action; other outcomes must match the submitted action, and Resize must match its geometry. A
`pasted` result never answers a non-paste action, and `input-accepted` never answers `paste`.

Detach or Session loss releases the combined grant before retiring the viewer. There is no separate
Client release action in this slice. Pending acquisition and cleanup exclude competing grants;
cleanup can outlive the Client submission deadline. Unknown input/resize is never automatically
replayed, including after reconnect or a new grant. Definite rejection grants no retry policy.
The conformance corpus covers all control actions, outcomes, reasons, byte bounds and invalid scope
fields. Runtime ownership and Host admission remain with their owning Server and Worker guides.

## Submission, reference, and cancellation obligations

A Client submits each mutating intent at most once. OBCS version 2 defines no replay lookup,
historical duplicate detection, or idempotency ledger. A consumer may reject a request when its
current accepted state exposes an ID collision or otherwise makes the request invalid, but that
state validation is not a promise to retain, recognize, or replay a historical request.

Version 2 has no cancel message. A timeout, transport loss, Client Session loss, or caller
abandonment neither encodes cancellation nor proves that a terminal effect did not commit. A Client
must not automatically resend the mutating intent after any of those events; an ambiguous result is
reconciled through a future read-only projection or observation boundary instead.

## Conformance and limits

[`v2/client-server-conformance-v2.json`](v2/client-server-conformance-v2.json) is the deterministic
cross-language oracle. Its JSON fixtures and independently authored binary hex records cover every message, result category, reason, accepted-state variant,
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

The unshipped version-2 extension `terminal.open` carries `protocolVersion`, a fresh
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

## Native Windows path refinement

Absolute wire paths accept a POSIX leading slash or an ASCII drive letter followed by `:` and `/` or `\`, optionally prefixed by `\\?\` for a Windows verbatim drive path. This lexical rule is independent of the decoder platform; bytes are preserved without expansion or normalization. Backslash-rooted, drive-relative, UNC, and device paths are not admitted by this Windows increment. Existing byte bounds and NUL/Unicode validation remain unchanged. The execution owner still validates the path against its local filesystem before effects.

## Native version 2

The native cohort uses JSON `protocolVersion: 2`, OBCS frame version 1 and a 63 KiB JSON payload limit. The [version-1 schema](v1/client-server.schema.json) and [corpus](v1/client-server-conformance-v1.json) remain frozen references; the current [version-2 schema](v2/client-server.schema.json) and [corpus](v2/client-server-conformance-v2.json) govern the native binding. Version 2 rejects version-1 bodies and legacy JSON `terminal.send` requests. The dedicated OBSN binary request retains encoding version 1; its four nonzero 16-byte identities, ordered record sequence and progress-result semantics are native v2 contracts. Binary framing, aggregate content limits and lexical validation are separate from the JSON schema.

A v2 Create additionally accepts optional `launch` (null or literal `{exec,args}`) and `environment` (ordered `[name,value]` pairs, omitted when empty). The executable is absolute, argv excludes argv0, at most 128 arguments and 30,720 combined UTF-8 bytes, with no NUL. Environment admits at most 32 entries and 1,024 combined UTF-8 name/value bytes. ASCII identifier names have 1–128 bytes, are unique ignoring ASCII case and exclude `ORCHBEE_*`, `TERM`, `COLORTERM` and `TERM_PROGRAM` ignoring case. Values are literal, may be empty, and exclude NUL. Byte sums and case-folded uniqueness are binding refinements beyond JSON Schema. The optional `launchProfile` sibling in a found launch read carries opaque `profileId` (1–128 UTF-8 bytes) and canonical unsigned-64 decimal `catalogRevision`; it records captured provenance and grants no launch authority.

A found launch read may also carry non-null `launchSelection:{exec,args}` under the same program/argument bounds. It is the resolved selection frozen at creation; default-shell and `not-tracked` facts omit it. Null, malformed selections and `not-tracked` with a selection are rejected. Older messages may omit the member. It exports no environment, does not read the current profile catalog and does not imply persistence across restart. Deploy/roll back this unpublished additive field with the same cohort; older readers may reject it.

Send results replace `acceptedBytes` with `progress`: numeric `recordsAccepted`, `firstStoppedRecord` (0–64) and `acceptedEncodedBytes` (0–67,108,864). Accepted records never exceed the stopped index. A successful outcome has a nonzero equal count/index and cleanup other than not-needed; any nonzero accepted count excludes not-needed. An unknown outcome excludes confirmed cleanup. Accepted/unknown or any cleanup other than not-needed requires a physical target. Cross-field relations are independently tested binding refinements. The unknown frontier never authorizes resend. `input-budget-exhausted` is an additional unavailable reason. Exact cohort rollout/rollback remains required; no persisted migration, Desktop support or Workbench schema equivalence is implied.

The OBSN header is magic `OBSN`, big-endian u16 encoding version `1`, zero u16 reserved field, and u32 body length. The body is request/Site/Worker/Terminal identities (16 bytes each), u8 count (1–64), then ordered records. Its maximum body length is `64 + 1 + 64 * 7 + 1,048,576` bytes; reject a bad header before allocation. Each record starts with a u8 kind:

| Kind     | Remaining record bytes                                                                                                           |
| -------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 1 text   | u16 UTF-8 byte length, text, u8 enter (0 or 1); nonempty text plus optional CR <=4,096 bytes                                     |
| 2 key    | u8 code: enter, escape, tab, backspace, delete, up, down, left, right, home, end, page-up, page-down, ctrl-c, ctrl-d map to 1–15 |
| 3 combo  | u8 nonzero modifier flags (Control=1, Alt=2, Shift=4), u8 base kind, u8 base value                                               |
| 4 bytes  | u32 length, nonempty opaque bytes <=1,048,576                                                                                    |
| 5 paste  | u32 length, nonempty opaque content <=262,144; Host owns bracket encoding                                                        |
| 6 stroke | u8 modifiers (Control=1, Alt=2, Shift=4, Super=8; zero allowed), u8 key kind, u32 key value                                      |

Stroke identities are named key `(1,1..13)`, Insert `(2,0)`, F1..F25 `(3,1..25)` or one non-control Unicode scalar `(4,codepoint)`. Unknown bits, C0/C1 controls, surrogates and out-of-range values are rejected before dispatch. Legacy record bytes and combo bounds are unchanged. Both native protocol owners use the same field semantics and validate independently; this unpublished extension requires a matched Client/Server/Worker/Host cohort.

Combo bases are named key `(1,1..13)`, Insert `(2,0)`, F1..F12 `(3,1..12)`, Space `(4,0)`, ASCII lowercase letter `(5,97..122)` or digit `(6,48..57)`. Extra flag bits, invalid bases, unknown kinds, truncation and trailing bytes fail closed. Aggregate decoded text (including folded CR), bytes and paste content is at most 1,048,576 bytes; keys carry no decoded payload. The request is not atomic: its result preserves the confirmed prefix and first stopped record. The v2 corpus supplies canonical binary bytes, every key and base kind, mixed records and negative controls independently of the Rust encoder.

## Styled buffer reads

A `read-rows` action optionally carries `detail:"text"|"render"`. Omission means text, and native
text encoding omits the default member to preserve existing bytes. Unknown details fail closed.
Every existing bound, exact revision/generation/start-row pin, correlation and one-shot lifecycle
still applies. Server validates the returned OBRW detail against the request. Render carries the
Host's existing complete projection plus graphics snapshot; the 8 MiB payload budget is unchanged.
A render refusal never falls back to text or silently discards graphics. Viewport reads remain text.

This is an additive unpublished OBCS v2 contract with the existing frame version. Client, Server,
Worker and Host development artifacts roll out or roll back together. Old readers reject the new
member; no durable state or migration is added. Windows ANSI CLI output is the concrete consumer,
while the wire remains platform-neutral. JSON-schema and independent Rust cases cover default
text compatibility, render selection and invalid details; runtime peers verify exact response pins.

Launch selections additionally cap worst-case JSON string-content bytes at 30 KiB: ASCII controls count as six, quotes and backslashes as two, and other UTF-8 bytes as one. This preserves the previous 2 KiB input domain while bounding the larger resume commands. Matching cohorts are required for the enlarged limits.

## Plugin terminal display environment

An explicit terminal launch may carry a separate `PluginTerminalEnvironment` (`pluginEnvironment` in OBCS). It admits only plugin ID/root/entrypoint/config/state/context, native binary path and optional Workspace/Tab/Pane display IDs: ten unique named entries, seven required, at most 24 KiB combined UTF-8. These are display data, not authority or plugin registration. The ordinary caller environment retains its existing limits and reserved-name refusal. Runtime base/scope and Worker/terminal identities remain owned by the Worker adapter. A plugin environment requires an explicit executable. OBCN extension 3 transports the explicit selection, ordinary environment and plugin environment; legacy encodings and the 40 KiB frame limit stay unchanged. Server checks the complete encoded plugin launch before reserving a physical operation. Worker removes inherited known plugin display fields before applying this launch's values. Workbench plugin resolution, Pane association and Client placement remain separate integration responsibilities. Focused boundary and launch-fact tests cover this carrier; this is not a complete plugin Pane receipt.
