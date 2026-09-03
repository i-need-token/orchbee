# Capability Node Protocol

Private, experimental Rust semantic contract for authorized terminal operations between Orchbee
Server and a Capability Node. The initial Capability Node is the Worker, but this crate does not
bind the messages to a local socket or any other carrier. No runtime consumes this protocol yet,
and it is not a public API or a compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

One canonical frame contains the fixed `OBCN` magic, an unsigned big-endian frame version, an
unsigned big-endian payload length, and at most 8 KiB of payload. The frame codec rejects unknown
magic, version, message or enum tags, malformed or incomplete values, excess bytes, and oversized
payloads. Frame version 1 and terminal-launch capability version 1 are separate version domains.
Bounded streaming helpers read the complete eight-byte header before validating it and allocating
the declared payload, then read exactly that payload; writes tolerate partial progress. I/O failure
is distinct from a syntactically invalid protocol frame. Writing one frame does not flush a
caller-owned buffer; that carrier policy remains with the adapter.

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

## Authorization outcomes

Pre-admission rejection distinguishes wrong Worker, stale generation, expired lease, stale fence,
conflicting same-fence replay, and unsupported capability. Host-side rejection and failure retain
the observed Host epoch. `HostUnavailableBeforeHandoff` is a definite pre-effect failure;
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
own protocols. Server owns product authorization and reconciliation; a future Capability Node
adapter owns explicit mapping to its Worker admission identity and Terminal Host protocol. This
crate depends on neither protocol.

## Checked authorization model

The registered
[`TerminalLaunchAuthorization`](../../apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.tla)
model is the Server-owned behavioral authority for this unconsumed contract. It explores current
and stale Worker generations, lease expiry before admission, between admission and Host handoff,
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

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models terminal-launch-authorization
pnpm test:tla:inventory
pnpm lint:documentation
pnpm lint:projects
```
