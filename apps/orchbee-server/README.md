# Rust Orchbee Server

Experimental, unshipped private Rust Server application. It owns the in-memory Workbench
projection kernel, the first POSIX local Server-Worker process spine, and an additive local Human
Client/Client Session admission spine. The emitted executable is named `orchbee-server`; it
participates in the root Cargo verification gate but is not consumed by public `orchbee`, Desktop,
packaging, an installer, or a service manager.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Workbench projection store

`WorkbenchProjectionStore` loads one encoded version-1 `workbench.snapshot` through
`orchbee-workbench-protocol`. A read parses one encoded `workbench.snapshot.read` intent and returns
an in-memory `SnapshotReadResult` containing the exact request ID and a clone of the complete
validated snapshot.

Invalid, oversized, semantically inconsistent, or wrong-kind inputs fail through the existing
`ContractError` boundary. The read boundary applies the 16 KiB intent cap before JSON decoding;
the snapshot parser likewise applies the 1 MiB projection cap first. Reads never change retained
state. The result is an unpublished application type, not a new wire response or stable
cross-application API.

## In-memory advancement and publication

`advance_projection` accepts an internally supplied complete encoded snapshot. It requires the
same Site and a strictly greater revision, then derives a deterministic aggregate diff: an optional
navigation replacement first, followed by changed Terminal Contexts in canonical identity order.
It returns a typed `WorkbenchPublication::Delta` when both the 1,024-change and 1 MiB encoded limits
hold. If either delta limit would be exceeded, it returns the complete validated snapshot instead.
An empty delta may advance a revision with no visible projection change.

Before changing retained state, the store serializes the selected publication within the contract
bound. A selected delta must also apply to the previous snapshot and reproduce the candidate
exactly. Parsing, Site, revision, refinement, or serialization failure leaves the previous snapshot
unchanged. The store does not accept Client deltas or semantic mutation intents and has no external
caller or publication adapter.

## Local Worker service

On macOS and Linux, the private development invocation
`orchbee-server --local-endpoint <absolute-worker-path>` binds one Unix-domain Worker socket without
deleting an existing path. The additive form appends
`--client-endpoint <absolute-client-path>` to bind the separate Client socket. Selected sockets are
restricted to mode `0600`; only after every selected listener is ready does Server write the
machine readiness marker `ready\n` to stdout. Any other invocation remains silent and exits with
status `1`. This is an internal additive process contract, not public CLI grammar or a production
service-installation interface.

The Server and Worker mutually require the peer's effective operating-system user through kernel
Unix-socket credentials. A credential mismatch or handshake transport-setup failure closes the
candidate without a protocol response. One listener admits at most one compatible Worker at a time
and assigns a strictly increasing process-local generation. A duplicate, malformed, timed-out,
wrong-message, protocol-incompatible, or cohort-incompatible attempt does not change the active
Worker. Disconnect removes only the matching generation, so delayed cleanup cannot retire a later
Worker. Handshakes are read synchronously under a finite timeout; only the accepted Worker receives
one monitor thread. There is no unbounded connection thread or message queue.

The endpoint path is supplied by the controlled development witness. Discovery, stale-endpoint
recovery, a default runtime directory, singleton activation across candidate paths, daemonization,
Windows named pipes, production service supervision, and shutdown signaling remain outside this
slice. Abrupt process termination or a post-bind setup failure may leave the supplied socket path
for its external owner to remove.

When selected, Client listener bind or listener-thread spawn failure occurs before readiness and
fails Server startup. An unexpected Client listener accept failure after readiness ends that
listener but is not yet coordinated with the Worker loop; production fault supervision and restart
remain reserved.

The private [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.md) crate owns
Worker framing, handshake, cohort rejection, and bounds;
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) owns kernel
same-user proof. Server owns listener lifetime, Worker admission state, generations, and cleanup. The
[`orchbee-worker`](../orchbee-worker/README.md) application owns the connecting process lifecycle.

## Local Client Session service

When the private Client endpoint is selected, every same-user compatible Human Client connection
creates one independent process-local Client Session generation. TUI, Web, Desktop, mobile, tab,
window, and renderer identity are intentionally absent: the endpoint admits one peer Client model.
Server accepts up to eight active Sessions and at most sixteen total candidate/session handler
threads. Each handshake has a two-second read/write timeout. One silent candidate therefore cannot
block another connection while a handler slot remains, and saturation closes excess candidates
without allocating more threads. Handler slots and admitted-Session registrations are
lifetime-owned guards, so normal completion, faults, and unwinding release both bounds.

An authenticated candidate receives a typed rejection for protocol/cohort incompatibility,
malformed or wrong messages, timeout, or active-Session capacity. After Server completely writes
`ClientAccepted`, it removes the handshake timeouts and reads strict OBCS on that same unbuffered
stream. It reads one request, constructs and attempts to write its complete correlated denial, and
only a completed write permits the next read. For every successfully decoded Create-immediate,
Create-deferred, and Start request, the constructed result has its exact result kind and `requestId`
with `rejected/not-authorized`; request fields and Worker presence do not change that decision. EOF,
malformed, wrong-direction, oversized, truncated, read, or write failure closes only that Session.
Disconnect cleanup matches the exact generation and cannot remove another or replacement Session.
It does not stop Server, disturb the independent Worker registry, or imply any terminal action.
Server process loss closes every Client liveness attachment.

The private
[`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.md) crate owns Client
framing and the real Rust connecting adapter. The independent
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.md) binding owns OBCS
validation and frame I/O. Server owns listener, handler-slot, admission, sequential denial,
generation, and cleanup state. Public `orchbee` does not yet call the adapter, so bare, help, and
version behavior remains unchanged. This slice adds no Workbench command, successful terminal
result, terminal relay, TUI, activation, endpoint discovery, persistence, or remote ingress.

## Local Worker process-spine modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | The verified native artifacts had no real process relationship. This Worker sub-spine must prove that a separate Server can admit and retain one separate same-user compatible Worker without creating a terminal, activation policy, or durable state.                                                                                    |
| Inputs       | An untrusted private argument vector, an absolute endpoint path supplied by the development owner, kernel peer credentials, and one bounded framed Worker handshake. Absence, malformed bytes, timeout, wrong message, wrong user, version mismatch, cohort mismatch, duplicate Worker, and disconnect are explicit.                       |
| Data         | Server phase, optional active Worker identity plus nonzero process-local generation, next generation, and one candidate connection. Server is the sole writer. No value survives process exit.                                                                                                                                             |
| Outputs      | Successful bind emits one `ready\n` marker. Worker admission returns its generation; an authenticated protocol-level rejection returns a typed bounded reason when the stream permits it. Peer-authentication and transport-setup failures close silently. Process setup or listener failure exits status `1` without a human diagnostic.  |
| Invariants   | One endpoint has at most one listener; one Server has at most one active Worker; active generations are nonzero and strictly increase; rejection cannot replace active authority; generation-scoped cleanup cannot remove a replacement; Worker disconnect does not stop Server; Server loss closes the Worker connection.                 |
| Decisions    | Invocation validity, peer-user equality, frame validity, protocol and cohort equality, expected Worker message kind, active-Worker absence, and matching cleanup generation completely determine the result. No current directory, Project, Environment, Worktree, Agent, or Terminal fact participates.                                   |
| Model        | A deterministic Worker registry owns admission and generation-scoped disconnect. A thin synchronous Unix-socket workflow authenticates, decodes, admits or rejects, acknowledges, and monitors the accepted connection.                                                                                                                    |
| Transitions  | `absent -> listening`; candidate `none -> pending -> admitted/rejected/timed-out`; Worker `none -> active(generation) -> none`; Server `listening -> stopped`. A second bind is rejected, and stale cleanup is an idempotent no-op against another generation.                                                                             |
| Dependencies | Rust standard-library process, filesystem, Unix socket, timeouts, threads, and I/O plus the private local-site protocol and kernel peer-credential crates. No terminal, child process, network socket, async runtime, persistence, environment lookup, or randomness.                                                                      |
| Verification | Pure codec and registry tests; native same-user socket-pair tests on macOS and Linux; real Server and Worker process tests for bind, readiness, admission, duplicate and incompatible rejection, replacement after disconnect, Server loss, malformed peers, exact output, and cleanup bounds; registered TLA+ safety and progress checks. |
| Limits       | This Worker evidence covers macOS and Linux under the same effective user. It does not establish Windows behavior, cross-user negative execution, production endpoint discovery, crash cleanup, service activation, Client Session semantics, terminal retention, or remote security.                                                      |

The checked [`LocalSiteProcessSpine`](formal/tla/LocalSiteProcessSpine.tla) model explores two
Worker identities and three generations. Its abstraction maps candidate actions to handshake
admission, `activeWorker` and `activeGeneration` to the registry, stale cleanup to
generation-scoped disconnect, and Server stop to transport loss. It proves only the bounded model;
deterministic and process tests own implementation refinement.

## Local Client Session and OBCS modeling record

The Rust handler now refines the checked deny-only phase below. It consumes strict OBCS after the
accepted OBLC response is completely written, but it gains no terminal capability and produces no
terminal or Worker effect.

| Field        | Current runtime and checked refinement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Keep the implemented same-user OBLC admission boundary and prove the smallest next same-stream phase: only after the OBLC accepted frame has been queued and written may that exact generation read OBCS. OBLC admission is not terminal capability.                                                                                                                                                                                                                                                                                                                                              |
| Inputs       | Runtime and model consume the optional absolute endpoint, kernel peer credentials, a bounded OBLC frame decoded under the current per-I/O two-second handshake timeout, and strictly decoded complete Create-immediate, Create-deferred, and Start request classes. The model additionally abstracts one or two typed frames already buffered by the carrier; idle, partial-header, partial-payload, and result-write stalls; strict malformed, wrong-direction, oversized, truncated, clean EOF, I/O failure, and Server loss. The OBCS schema, corpus, and binding own byte and field validity. |
| Data         | The runtime owns at most eight active Session generations and sixteen candidate/session handlers. The bounded models use three Client slots, at most two Sessions, three handlers, four globally fresh generations, and two request-ID equivalence classes. An active Session and a slow candidate each occupy one handler. The OBCS refinement holds at most one outstanding request per Session; up to two carrier-buffered frames are an environment fact, not a Server request queue.                                                                                                         |
| Outputs      | Runtime and model map valid Create to `terminal.create.result` and valid Start to `terminal.start.result`, always `rejected/not-authorized` with the exact request `requestId`; an actual write still depends on carrier writability. No output exposes or creates Terminal, Worker, Host, generation, operation, lease, fence, shell, executable, environment, or Workbench authority.                                                                                                                                                                                                           |
| Invariants   | OBLC accepted is written before OBCS decode; each active generation owns exactly one handler; Session and handler bounds are independent; at most one request/result is outstanding per Session; malformed input or transport loss closes only the addressed current generation and releases its handler; stale EOF or cleanup cannot remove a replacement; Server stop clears all Sessions; stalled Client work preserves another Client and the Worker registry; every modeled terminal- and Worker-request effect counter remains zero.                                                        |
| Decisions    | Valid Create and Start inputs are denied without consulting Worker presence, exact Worker constraint, `cwd`, Create start mode, geometry, or the Start `terminalCreateRequestId`. The Start correlation `requestId` is distinct from that reference, and the reference is not looked up. A repeated request ID is decided again from current state; no replay lookup, historical conflict detection, cancellation, retry, or idempotency ledger exists.                                                                                                                                           |
| Model        | [`LocalClientSessions`](formal/tla/LocalClientSessions.tla) owns OBLC admission, generation, and the separate Session/handler bounds. [`LocalClientObcsSessions`](formal/tla/LocalClientObcsSessions.tla) refines each non-closed phase to one active generation and handler, derives the deny result directly from its one outstanding correlation record, and retains generation-scoped cleanup. Its `stream-faults` and `correlation` configurations share the same state machine but avoid multiplying unrelated carrier-pair histories by every fault/stall ordering.                        |
| Transitions  | Candidate `none -> slow -> released` or Session `closed -> OBLC-accepted-queued -> OBCS-idle`; read may stall or advance through partial header/payload; one complete request becomes one ready or write-stalled result; a successful result write clears that request before read re-entry. Fault, EOF, I/O failure, or result-write failure closes the current generation. A later admission receives a fresh generation, and stale events preserve it. Server loss closes all generations.                                                                                                     |
| Dependencies | Runtime composes the Server-owned local Client admission workflow with the OBLC and independent OBCS Rust bindings; the model composes their contracts. Neither performs Worker lookup, OBCN mapping, Host dispatch, PTY operation, terminal retention, shell selection, product projection, persistence, or network ingress.                                                                                                                                                                                                                                                                     |
| Ownership    | Server owns Client Session generation, handler accounting, same-stream ordering, deny result construction, and generation-scoped cleanup. The carrier owns byte arrival, buffering, EOF, and writability. OBCS owns strict framing and message validity. Worker/Host authority and future OBCS-to-OBCN mapping remain outside this slice.                                                                                                                                                                                                                                                         |
| Verification | Deterministic session tests cover every request class, exact correlation, partial I/O, two-frame ordering, duplicate IDs, strict failure classes, deterministic write failure, and RAII cleanup during unwinding. Real process tests cover coalesced OBLC plus OBCS, sequential Create/Start, capacity cleanup, post-admission stall recovery, and Worker non-interference. Both registered models require nonzero TLC coverage for their semantic actions and check the bounded admission and OBCS state spaces described below.                                                                 |
| Limits       | This is bounded safety and native process evidence, not a successful terminal effect or codec reimplementation. Runtime adds no OBCS timeout, queue, retry, cancellation, ledger, or fairness promise; a stalled Session may remain stalled forever. It proves neither eventual response nor terminal creation. The Client protocol still requires a fresh `requestId` for each submitted request under the active Server authority and authenticated subject; repeated IDs are processed statelessly and do not create a Server history obligation.                                              |

The admission model uses `MaxSessions = 2` and `MaxHandlers = 3`, so a slow candidate can consume
the spare handler independently of two active Sessions. The OBCS model uses two distinct request
IDs; Start always uses one as its result-correlation ID and the other as its distinct Create
reference. Carrier-pair annotations exist only to prove duplicate-versus-distinct ordering and are
never consulted by the Server denial decision. A buffered Start retains its distinct Create
reference until decode, while denial and result correlation ignore that reference. Neither model
has a temporal fairness clause.

## Accepted terminal Client authority modeling record

The next accepted implementation slice makes the Rust Server the owner of product-level Client
Sessions and terminal authority on the relayed `Client -> Server -> Worker -> Terminal Host` path.
This record and its formal model describe the accepted refinement target; they do not claim that
the current runtime or wire contracts implement it. A Client Session is a Server transport/session
identity, a viewer attachment associates one generation of that session with a retained terminal,
and the controller is the terminal's one combined input-and-resize grant.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Several peer TUI, Web, Desktop, or mobile Client Sessions must be able to view one retained terminal concurrently without creating several input or geometry writers. Closing a Client must release its temporary authority without ending the terminal.                                                                                                                                                                                                                     |
| Inputs       | Server availability; independently generated Client Session identities; Worker and Host availability; retained or lost terminal state; viewer attach, controller acquire, input, resize, Client disconnect, and generation-scoped cleanup events. Client Session establishment depends only on a live Server. Worker, Host, or terminal unavailability blocks terminal attach and control, not Server session establishment.                                                 |
| Data         | The Server-owned last-known session registry and per-Client generation, generation-scoped viewer attachments, one optional combined controller reference per terminal, and pending cleanup references. After Server loss, registry entries are only the final Server snapshot, not evidence of a live Client transport. Effective Client transport additionally requires the Server to be live.                                                                              |
| Outputs      | Viewer attachment succeeds only across a live retained-terminal path. Input and resize advance only for the same current controller reference; attempts from another attached viewer are rejected without changing terminal input or geometry. A disconnected Client is removed from its current attachment, and a disconnected controller releases the grant immediately.                                                                                                   |
| Invariants   | Any number of modeled current sessions may be viewers, but effective input and resize authority are the same set and contain at most one generation-scoped controller. The controller is a current viewer. An acquire operation can fill only an unowned grant and never replaces another controller. Disconnect and stale cleanup cannot terminate a retained terminal, and cleanup for an old generation cannot revoke a reconnected generation's newer grant.             |
| Decisions    | Client Session creation and reconnection require only Server availability. Viewer attach and controller use require Server, Worker, Host, and a retained terminal. Controller disconnect releases authority; another viewer may acquire only after the grant is unowned. The exact acquire/takeover interaction is intentionally undecided. Server or Worker loss may release a temporary grant or freeze its registry snapshot, but cannot transfer it or end the terminal. |
| Model        | The Server projects each Client generation into zero or one viewer reference for the modeled terminal and stores zero or one combined controller reference. Effective input and resize controllers are derived from that reference plus path availability. The existing Host resize lease remains a lower-level mechanism and is not the Server product policy.                                                                                                              |
| Transitions  | Session `closed -> open(generation)` independently of terminal availability; viewer `detached -> attached`; controller `unowned -> owned(current viewer) -> unowned`; Client `open -> disconnected -> reconnected(next generation)`, with old-generation cleanup before or after reconnect; process `up -> lost`; terminal `retained -> lost` only on Host loss. No restart or automatic controller transfer transition exists.                                              |
| Dependencies | Future runtime refinement will depend on the Server-Worker spine, Worker-owned Host broker, Terminal Host attachment/input/resize protocols, and Client transport. It must project read-only Host attachments for viewers and exact combined input-plus-resize authority for the controller, or prove an equivalent refinement. No new runtime or wire dependency is introduced by this modeling slice.                                                                      |
| Ownership    | Server owns Client Session generations, viewer membership, product-level controller selection, rejection, and stale cleanup. Worker remains the sole Terminal Host broker. Host owns retained PTY lifetime and enforces its low-level attachment capabilities and resize lease. A Host-side resize-owner promotion after detach is an existing mechanism only; this model does not elevate it into Server-level automatic takeover policy.                                   |
| Verification | The registered TLA+ model checks three Client slots, two generations per slot, bounded input and geometry revisions, nonzero coverage for every semantic transition, safety invariants, and cleanup progress while Server remains available. Runtime refinement tests do not yet exist because this slice changes no runtime.                                                                                                                                                |
| Limits       | The model covers one terminal and at most one attachment per Client slot. It does not prove runtime or protocol refinement, byte/queue/backpressure behavior, PTY retention implementation, direct Client-Worker paths, restart, durable recovery, automatic or forced takeover, collaborative input, controller fairness, or takeover UX.                                                                                                                                   |

The checked
[`LocalTerminalClientAuthority`](formal/tla/LocalTerminalClientAuthority.tla) model keeps Client
transport liveness separate from terminal path availability. Server or Worker loss makes terminal
authority ineffective and can either release the temporary controller reference or leave a frozen
registry snapshot; neither event changes the retained terminal. Host loss alone changes the
terminal to `lost`. Its progress property guarantees that pending generation cleanup drains while
the Server survives, not that authority is automatically reassigned.

## Ownership

This application owns projection mechanics for its injected in-memory snapshot copy plus the
experimental local Worker and Client Session registries and deny-only OBCS phase. It is not a
second current product writer: every successfully decoded terminal intent is assigned a denial
decision without an effect, no Client or current runtime can submit a projection candidate, and no
publication leaves the application. The current Desktop/Workbench composition remains the
delivered Workbench writer, and the Node
[`apps/server`](../server/README.md) remains authoritative for its existing Browser ingress
behavior.

Semantic business transitions, authorized or successful terminal processing, terminal relay,
product authorization, persistence, recovery, external publication, endpoint discovery and
activation, production service lifecycle, Terminal Host authority/runtime integration, packaging,
migration, and cutover require later implementation slices.

The native source tree has two roots: `src/lib.rs` is the importable projection root, while
`src/main.rs` is the private binary composition root. The binary's exhaustive lateral import
directions are `main -> local_site_service`, `local_site_service -> worker_registry` plus
`local_client_service::start`, and `local_client_service -> client_session_registry` plus its
private `obcs_session`. Reverse imports, cousin imports, and every other lateral direction are
forbidden. Tests may access owner-private state without expanding a production direction.

`obcs_session` is not re-exported. Its only entry point is a `pub(super)` runner that receives the
already admitted stream, and its only non-standard-library dependency is
`orchbee-client-server-protocol`. It must not import the Worker registry, Host, projection,
effect, or dispatch authority. Any wider visibility or dependency, including OBCS-to-OBCN
mapping, requires a later accepted slice.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-client-sessions-obcs,local-site-process-spine,local-terminal-client-authority
pnpm verify
```
