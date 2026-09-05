# Capability Node Protocol

Private, experimental Rust semantic contract for authorized terminal operations between Orchbee
Server and a Capability Node. The initial Capability Node is the Worker, but this crate does not
bind the messages to a local socket or any other carrier. The local Server adapter now consumes its
codec in an application-private post-OBLS carrier. The local Server coordinator now submits
authorized Create/Start through that carrier to the Worker Host-execution loop. It is not a public API or a compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

One canonical frame contains the fixed `OBCN` magic, an unsigned big-endian frame version, an
unsigned big-endian payload length, and at most 8 KiB of payload. The frame codec rejects unknown
magic, version, message or enum tags, malformed or incomplete values, excess bytes, and oversized
payloads. Frame version 1 and terminal-launch capability version 2 are separate version domains.
Bounded streaming helpers read the complete eight-byte header before validating it and allocating
the declared payload, then read exactly that payload; writes tolerate partial progress. I/O failure
is distinct from a syntactically invalid protocol frame. Writing one frame does not flush a
caller-owned buffer; that carrier policy remains with the adapter.

Capability version 2 adds the generation-local Worker binding bound and its typed capacity result
without changing the frame envelope. A Worker admits only its exact supported capability version;
version 1 and version 2 requests therefore reject each other as `UnsupportedCapability`. Both
versions use the same rejection tag for that outcome, while a version 1 decoder rejects the new
capacity tag as unknown. Mixed versions consequently fail closed instead of silently using version
1's unbounded binding behavior. The current Worker authorization kernel is fixed to version 2
rather than accepting a configurable version with version 2 capacity behavior. Construction and
decoding also reject `CapacityExceeded` when its echoed authorization is not version 2.

| Message                    | Direction                 | Closed semantic payload                                                                                                                                               |
| -------------------------- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AuthorizedTerminalLaunch` | Server -> Capability Node | Complete operation authorization, optional bounded node-local current directory, and either `Immediate` with complete pixel geometry or `Deferred`.                   |
| `TerminalLaunchResult`     | Capability Node -> Server | The exact authorization plus `HostAccepted`, `Rejected`, `Failed`, or `OutcomeUnknown`; every post-handoff outcome preserves the observed Host epoch.                 |
| `AuthorizedTerminalStart`  | Server -> Capability Node | A separately fenced and leased authorization, complete target Host terminal identity, and complete pixel geometry for a previously deferred terminal.                 |
| `TerminalStartResult`      | Capability Node -> Server | The exact authorization and target Host identity plus `HostAccepted`, `Rejected`, `Failed`, or `OutcomeUnknown`; success is explicitly `Started` or `AlreadyStarted`. |

Every authorization binds the product terminal ID, operation ID, protocol-owned Worker ID,
nonzero Worker generation, nonzero per-terminal monotonic fence, stateful lease ID and nonzero
absolute Unix-millisecond expiry, and explicit capability version. All protocol-owned IDs are
nonzero opaque 128-bit values. Results echo the complete authorization so Server can discard stale
generation, fence, operation, lease, or capability observations.

Operation identity also has a producer invariant that cannot be established by decoding one frame:
the authenticated authoritative Server allocates every `OperationId` globally fresh within its
active authority, across all Terminal IDs, Workers, and Worker generations, and never assigns one
identity to two different authorizations. A successor Server authority may begin a new producer
scope only after its authority transition makes all earlier authorizations non-current through
generation, lease, and fence validation. This contract does not specify that transition or require
a Capability Node to retain an unbounded cross-terminal tombstone ledger; Worker refinement assumes
well-formed issuance from the authenticated authority and does not claim to contain a violating
issuer.

The optional current directory is an absolute POSIX UTF-8 path of 1 through 4,096 encoded bytes and
cannot contain NUL. Its absence is distinct from an empty value. Geometry contains columns, rows,
cell-pixel width, and cell-pixel height. Each value is in `1..=4096`, the grid is at most 262,144
cells, and each total pixel extent fits `u16`. These types are Capability Node refinements, not
imports or re-exports of Terminal Host wire payloads; a future Worker adapter must convert them
explicitly and preserve validation.

## Read-only viewer messages

The Worker now consumes OBCN kind 5 `TerminalViewerRequest` and emits kind 6
`TerminalViewerResult`. Server now submits these messages for its admitted Client Sessions alongside Launch/Start. These additive messages remain private and unshipped.
An older decoder rejects the new tags or magic; no fallback or mixed-cohort compatibility is claimed.

Every request contains a fresh request `OperationId`, exact Worker ID and nonzero generation, and a
Server-issued attachment `OperationId`. Server allocates attachment identities fresh across its
active authority and never reuses them after detach. They name independent transient viewers, not
product Terminal identities or Host creation identities. The authenticated Server alone grants
read access; Worker checks the admitted Worker/generation and exact Host epoch before attaching.
This path grants neither input nor resize, and never claims the Host resize lease.

The closed actions are `Attach` with Host epoch and Host terminal ID, `Poll`, `FrameApplied` with
positive sequence/generation and revision, `NeedSnapshot` with positive sequence and the existing
eight snapshot reasons, and `Detach`. Kind 5 lays out request ID (16 bytes), Worker ID (16),
generation (8), attachment ID (16), then action tag (1). Tags 1 through 5 select those actions in
that order. Attach adds two 16-byte identities; FrameApplied adds three 8-byte integers;
NeedSnapshot adds an 8-byte sequence and reason tag 1 through 8: `ActivityStale`, `BaseMismatch`,
`DecoderRejected`, `DimensionsMismatch`, `GenerationMismatch`, `IdentityMismatch`, `SequenceGap`,
and `StoreReset`.
All integers are unsigned big-endian. Kind 6 contains request and attachment IDs followed by
`Attached`, `Empty`, `FeedbackAccepted`, `Detached`, or `Rejected` tags 1 through 5. Rejection tags
1 through 8 are wrong Worker, stale Worker generation, stale Host epoch, capacity exceeded,
attachment unavailable, attachment already exists, feedback mismatch, and Host rejected.
Results always echo both request and attachment IDs. A poll never waits for future Host output.

A record result uses the separate `OBNR` envelope: four-byte magic, big-endian version 1 (`u16`),
zero reserved field (`u16`), and big-endian payload length (`u32`), then request ID (16), attachment
ID (16), and one unchanged OBTP record. Its record is 64 through `64 + 36 MiB` bytes, matching the
Host envelope plus maximum OBTF payload. The codec validates the complete 12-byte header and bound
before allocating payload; consumers validate the opaque record's Host envelope, route, role,
identity, and negotiated size. This does not enlarge the 8 KiB OBCN control bound or decode frame
content. Strict in-memory decoding rejects trailing bytes; streaming decoding consumes exactly one
record and preserves the following frame boundary. No acknowledgement, retry, or queue policy is
hidden in these codecs. Viewer carrier failure retires transient state; callers never replay an
ambiguous attachment or feedback exchange automatically.

## Controller messages

OBCN kind 7 `TerminalControlRequest` and kind 8 `TerminalControlResult` extend the same serialized
Server-Worker stream. Server is the actual producer and Worker maps the granted controller to a
separate non-reading Host attachment. Server allocates fresh request and controller identities;
Viewer and control identities never alias, and all requests pin the admitted Worker generation.

A request contains request ID (16 bytes), Worker ID (16), Worker generation (u64), viewer ID (16),
controller ID (16), then action: 1 Acquire, 2 Input, 3 Resize, or 4 Release. Input adds a positive u64
sequence, u16 length, and 1 through 4,096 raw bytes. Resize adds a positive u64 sequence and the four
u16 geometry fields in the existing columns/rows/cell-width/cell-height order. Other actions add no
field. Integers use big-endian encoding; all IDs are nonzero. Input and resize share a strictly
increasing per-grant sequence; admission consumes it even on a definite Host rejection.

A result contains request, viewer, and controller IDs (16 bytes each), then outcome: 1 Acquired,
2 Released, 3 InputAccepted, 4 Resized with the same complete geometry, or 5 Rejected with a reason
byte: 1 WrongWorker, 2 StaleWorkerGeneration, 3 ViewerUnavailable, 4 ControllerBusy, 5 NotController,
6 SequenceMismatch, 7 HostRejected. Exact IDs and action/outcome compatibility are required; Resize
must echo geometry. Release is internal cleanup and is idempotent if that viewer has no controller;
a different current grant rejects. An ambiguous Host exchange closes the Worker carrier instead
of fabricating acceptance, and Server classifies complete delivery without a result as unknown.

The 8 KiB OBCN limit, exact-length decoding, and direction rules remain unchanged. Codec tests cover
all actions/outcomes/reasons, raw byte preservation, maximum input and invalid scope bounds. The
Server and Worker guides own single-controller reservation, cleanup and real PTY evidence.

## Authorization outcomes

Pre-admission rejection distinguishes wrong Worker, stale generation, expired lease, stale fence,
conflicting same-fence replay, unsupported capability, and retained-binding capacity. A
`CapacityExceeded` result is valid only for a previously unseen `TerminalId` after one Worker
generation has retained 1,024 bindings; it inserts no tombstone and changes no binding. Host-side
rejection and failure retain the observed Host epoch. `HostUnavailableBeforeHandoff` is a definite
pre-effect failure;
`OutcomeUnknown` is the conservative result when a dispatched request has no observable result,
because the Server cannot infer the Worker's internal handoff stage from carrier write completion.
Launch success returns complete Host creation, epoch, and terminal identity plus either
awaiting-start or started geometry. Start results always retain their complete target Host identity;
accepted results additionally return the observed identity, geometry, and started/already-started
transition.

Launch and Start use distinct rejection and failure types: Launch cannot encode
`HostIdentityChanged` because it has no target Host terminal. A Start `HostAccepted` observation
must exactly equal the frame's target identity. Its `HostRejected` and `HostReportedFailure` epochs
must equal the target epoch; a different observed epoch is represented only by
`HostIdentityChanged`, whose epoch must actually differ. Validated result construction and decoding
reject these identity and epoch contradictions, so canonical encoding cannot emit one through those
constructors. The Worker refinement additionally requires a successful Start observation to retain
the exact authorized geometry before it constructs a result.

The launch payload has no general environment overlay, executable, argument vector, Project,
Checkout, Worktree, Agent, or Launch Profile field. Unknown trailing bytes are rejected, so a
nonempty environment is not representable by this contract.

## Carrier and ownership boundaries

This crate owns only the closed semantic values and canonical `OBCN` frame codec. It does not open,
authenticate, admit, retain, retry, reconnect, multiplex, or apply backpressure to a connection.
Its streaming helpers borrow a caller-owned `Read` or `Write` only for one frame and retain no
socket, buffer, connection, or phase state.
Unix-domain sockets, same-user peer credentials, executable cohort, the `OBLS` admission handshake,
remote transport, encryption, and connection state machines belong to carrier adapters and their
own protocols. The current local Server adapter owns a capacity-one submitted slot behind at most
one wire request, serializes complete request/result exchanges, validates exact correlation and
echoed authorization, and distinguishes failures before and after a complete request write. It
does not map OBCS, synthesize semantic outcomes, retry, or reconcile. Server owns local terminal authorization and
reconciliation; a Capability Node adapter owns explicit mapping to its Worker
admission identity and Terminal Host protocol. This crate depends on neither protocol.

For the current adapter, an attached generation remains registered until its sole wire owner has
settled or conservatively abandoned the current exchange and drained the submitted slot. Only then
does a lifetime guard remove that exact generation, so no replacement overlaps an old write or
exact-result publication. Normal return, completion-receiver loss, thread-spawn failure, and panic
unwinding follow the same ordering. Abandonment after a complete write remains a typed carrier-level
post-delivery unknown; the adapter does not manufacture an OBCN result.

## Checked authorization model

The registered
[`TerminalLaunchAuthorization`](../../apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.tla)
model is the Server-owned behavioral authority for this experimental contract. It
explores current and stale Worker generations, lease expiry before admission, between admission
and Host handoff,
and after handoff, lower/exact/conflicting fences, definite pre-handoff failure, post-handoff
unknown outcomes, Host epoch replacement—including an admitted Start whose target epoch changes
before handoff—deferred launch followed by separate authorized start, stale result suppression,
every connection loss, and retained-terminal lifetime. That raced identity change is cached as the
exact `HostIdentityChanged` result without a Host effect. The model's invariants require at most one
Host effect per operation and prevent old generation, fence, operation, or Host epoch results from
overwriting current authority. Because this protocol has no progress acknowledgement, a lost Worker
link after dispatch produces a conservative unknown observation at every unobserved Worker stage;
only a returned Worker result can report definite pre-handoff failure.

The model permits one outstanding request and one pending-result slot. Worker loss before
admission, after admission, and after Host handoff is checked separately and always occupies that
empty slot with an unknown result. A replacement Worker generation is not activated until that
pending loss observation is settled, so reconnection cannot make the current unknown result appear
stale. Start acceptance separately explores `Started` and `AlreadyStarted`. Once an admitted
operation has a terminal result, the model caches its exact outcome, Host epoch, and transition. An
exact replay returns that cache without a new Host effect—including after lease expiry—so a
rejection, failure, or unknown result can never be manufactured into success.

The only fairness condition settles a pending result while Server remains connected. It does not
require any healthy connection to fail and does not promise automatic reconnection, authorization,
or terminal recovery. The model is bounded to two operation IDs, two distinct lease identities, and
two Worker generations, fences, and Host epochs. Its deterministic fence-to-operation-and-lease
mapping is a state-space symmetry reduction, not a runtime derivation rule; a checked no-revival
property prevents an expired accepted lease from becoming valid again without replacing that
binding. `FreshOperation` is the one-retained-terminal projection of the global Server producer
invariant and remains in force across modeled Worker reconnection because current authority and Host
effects are retained. The model does not enumerate the cross-terminal and cross-Worker namespace or
a successor Server-authority transition. Future runtime adapters require separate refinement
evidence.

The registered `TerminalLaunchAuthorization` model uses capability version 2 as its supported
authority and version 1 as its unsupported-capability witness.

The Worker-owned registered
[`WorkerTerminalBindingCapacity`](../../apps/orchbee-worker/formal/tla/WorkerTerminalBindingCapacity.tla)
refinement composes the cross-terminal bound with the Worker's abstract global operation/result
slot while the single-terminal Server authority model retains the detailed handoff phases. Its TLC
configuration uses capacity two and three terminal identities to exhaustively cover below-limit
admission, the full threshold, rejection of one new identity, occupied-slot priority, result
delivery clearing, exact cached replay and higher-fence replacement at capacity only after the slot
is empty, rejection preservation, and classification priority. The production constant is exactly
1,024 retained bindings per Worker generation. There is no release, TTL, LRU, or eviction path;
explicit retirement remains required before production. This bounded model is not a proof over
1,024 concrete entries or of the Rust implementation, and its authority ships only in the same
unshipped delivery cohort as the corresponding Rust enforcement.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models terminal-launch-authorization,worker-terminal-binding-capacity
pnpm test:tla:inventory
pnpm lint:documentation
pnpm lint:projects
```
