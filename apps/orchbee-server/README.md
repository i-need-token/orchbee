# Rust Orchbee Server

Experimental, unshipped private Rust Server application. It owns the in-memory Workbench
projection kernel, the first POSIX local Server-Worker process spine and serialized post-admission
OBCN carrier, and an additive local Human Client/Client Session admission spine. The emitted
executable is named `orchbee-server`; it participates in the root Cargo verification gate but is
not consumed by public `orchbee`, Desktop, packaging, an installer, or a service manager.

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
Worker. Although OBLS decodes its Worker identity as an opaque 16-byte value, Server rejects the
all-zero value as `MalformedFrame` before registry admission or `WorkerAccepted`: that value cannot
map to OBCN's nonzero Worker identity, and the rejection consumes no generation. Handshakes are read
synchronously under a finite timeout. After Server completely writes `WorkerAccepted`, it gives
that same unbuffered stream to exactly one generation-scoped OBCN wire owner and exposes a cloneable
request handle through the Worker registry. The owner, not a caller or the registry, serializes every
write/read exchange; no registry lock is held across wire I/O.

The handle rejects a request for another Worker or generation before touching the wire. It admits
at most one submitted request behind the one request possibly in flight, and additional submissions
receive immediate backpressure. `Unavailable` and `Backpressure` both mean that the addressed
request did not cross the complete-write boundary. The owner writes one complete OBCN Launch or
Start request before reading exactly one result. It accepts only the matching result direction and
kind, operation ID, complete authorization echo, and, for Start, target identity. A malformed,
mismatched, timed-out, or disconnected exchange retires that Worker generation and makes queued
work unavailable.

An attached carrier remains the active registry entry for the wire owner's entire lifetime. The
only callable admission cancellation rejects attached carriers; neither the request handle nor the
registry exposes independent active-generation deactivation. Thus no replacement can be admitted
while the old owner can still write, read, validate, or publish an exact result. Owner settlement
gates retirement: on normal exit, completion-receiver loss, thread-spawn failure, or panic
unwinding, owner destruction first marks the handle inactive, conservatively settles any in-flight
request from its complete-write marker, and drains the submitted slot. Only then does its RAII
registration guard remove the matching generation; that removal is the retirement linearization
point at which replacement becomes admissible. A stale guard cannot remove a replacement.

Each complete request write and complete result read has its own two-second monotonic deadline;
partial progress does not restart either deadline. Failure before the request frame is completely
written is reported as `NotDelivered` and is known unchanged: strict Worker decoding cannot dispatch
an incomplete frame, and the owner retires that connection. After the complete write, any failure is
the distinct `PostDeliveryOutcomeUnknown` carrier error: Server neither fabricates the semantic OBCN
`OutcomeUnknown` value nor retries the request. An exact Worker result, including a Worker-reported
semantic unknown outcome, is preserved; the carrier does not branch on capability-version or
outcome variants. Bytes received while no request is outstanding are invalid and retire the
connection. Dropping the whole registry requests private shutdown but cannot admit a replacement;
the owner observes it within the 25-millisecond idle poll or the bounded current exchange, which has
at most its remaining write deadline plus one result-read deadline. There is no unbounded connection
thread or message queue.

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
same-user proof. The private
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) crate owns OBCN
semantic values and frame encoding. Server owns listener lifetime, Worker admission state,
generations, bounded submission, serialized carrier validation, and generation-matched cleanup. The
[`orchbee-worker`](../orchbee-worker/README.md) application owns the connecting process lifecycle;
its current runtime does not yet consume post-admission requests end to end.

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

## Local Worker OBCN carrier model and runtime refinement record

This model-first record fixes the smallest Server-owned carrier contract after the accepted OBLS
response is completely written. The model is the accepted refinement target; the service
implementation above supplies its runtime evidence but specifies no OBCS business policy.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | After OBLS admission, Server must reuse that exact Worker generation's stream for bounded OBCN Launch and Start exchange without letting the registry, request submitters, or another task become a second wire owner.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Inputs       | One current Worker identity and process-local generation; three valid request candidates comprising two same-kind Launch operations with distinct operation IDs and one Start; stale-handle submission while active or inactive; complete-frame write or its absence; finite monotonic per-I/O deadline ticks; disconnect; and an actual incoming response value. The response domain carries direction, result kind, Terminal ID, operation ID, Worker ID/generation, fence, lease ID/expiry, capability, Start target, and the exact outcome payload, including two distinct definite payloads and the Host epoch in Launch `OutcomeUnknown`.                                                                                                                                                                                       |
| Data         | The model keeps one `activeRef`, one generation-scoped `requestHandleRef`, one scalar `wireOwnerRef`, one capacity-one `submittedSlot`, at most one `wireRequest`, its `wirePhase`, `now` and `wireExpiry`, an environment-owned `incomingResponse`, and attempt/completion/publication audit state. The checked configuration uses two Worker identities, two generations, two Launch operations, and one Start operation. The submitted slot may coexist with one wire request, but only the latter can be in flight.                                                                                                                                                                                                                                                                                                               |
| Outputs      | Rejection before a complete request-frame write is `known-unchanged`; an exact OBCN result retains the actual validated incoming value and payload, including Worker `OutcomeUnknown`; timeout, disconnect, or invalidity after a complete write becomes carrier-level `effect-unknown`. The latter is not a fabricated OBCN result. Retirement drains the queued slot as known unchanged/unavailable, inactive stale handles are unavailable without a write, and no class causes automatic retry.                                                                                                                                                                                                                                                                                                                                   |
| Invariants   | At most one current generation and one stream owner exist; the registry stores only the request handle and never the stream; the submitted and wire slots are independently bounded to one; each operation receives at most one complete write; the deadline clock is monotonic; exact publication equals a previously observed response whose direction, kind, operation, every authorization field, Start target, and outcome payload match; every mismatch class and invalid unknown payload cannot publish exact; unknown effects are non-retryable; replacement waits for old completion and publication settlement; stale activation and generation-matched cleanup cannot replace or remove the current generation.                                                                                                            |
| Decisions    | Wrong-Worker and wrong-generation routing reject before enqueue without retiring the owner. A stale handle, including while no generation is active, returns known unchanged/unavailable. Capacity is one submitted request plus one wire request; a fresh third valid request records backpressure without changing either occupied slot. `CompleteRequestWrite` is the ambiguity boundary and installs a fresh result-read deadline after the request-write deadline. Local time advancement and expiry settle a silent peer; response and disconnect arrival have no fairness assumption. `RetireCarrier` closes the generation gate: a later write or newly observed exact response is impossible, while an exact result observed first remains publishable. Replacement requires every old completion and publication to settle. |
| Model        | [`LocalWorkerObcnCarrier`](formal/tla/LocalWorkerObcnCarrier.tla) separates registry identity, cloneable request-handle authority, queue occupancy, sole wire ownership, write completion, actual response arrival, local validation, deadline expiry, publication, retirement, and cleanup. `ReceiveExactWorkerResult` validates and preserves `incomingResponse`; it does not construct a matching result. The model refines the post-OBLS boundary and OBCN semantic contract rather than reimplementing either byte codec.                                                                                                                                                                                                                                                                                                        |
| Transitions  | `ActivateCarrier` publishes one generation; `SubmitFirstLaunch`, `SubmitSecondLaunch`, and `SubmitStart` exercise distinct candidates; `DequeueRequest` moves one value to the owner; `CompleteRequestWrite` crosses the ambiguity boundary; `AdvanceMonotonicTime` enables pre- and post-write expiry; exact arrivals and one named arrival action per mismatch class populate `incomingResponse`; `ReceiveExactWorkerResult` or `RejectMismatchedWorkerResult` validates it; disconnect actions cover idle, before-write, after-write, queued-after-write, and after-result orderings; publication, late-result discard, replacement, stale rejection, and generation-matched cleanup settle retained state.                                                                                                                        |
| Dependencies | The target composes the existing Server Worker registry and post-`WorkerAccepted` stream with the private local-site and capability-node protocol contracts. The local-site protocol owns OBLS framing, the capability-node protocol owns OBCN values and strict framing, and the carrier composes them. `AllAttemptsSettled` is the carrier counterpart of the pending-result replacement guard in `TerminalLaunchAuthorization`. No OBCS mapping, authorization decision, Host dispatch, terminal retention, persistence, endpoint discovery, or remote ingress participates.                                                                                                                                                                                                                                                       |
| Ownership    | Server's registry owns current Worker generation and request-handle publication. One `WorkerWireOwner` equivalent exclusively owns stream reads and writes plus queue draining; request-handle clones can only submit. The codec owns byte validity, Worker owns the semantic OBCN result, and a later Server policy owner—not this carrier—owns authorization, reconciliation, or any permitted retry of a separately proven known-unchanged operation.                                                                                                                                                                                                                                                                                                                                                                              |
| Verification | The registered `local-worker-obcn-carrier` model requires nonzero TLC coverage for 47 semantic actions across `response-validation`, `disconnect-order`, `stale-routing`, and `capacity` variants. The variants cover both same-kind correlations, every response field/payload mismatch, deadline expiry on each write side, every disconnect ordering, active and inactive stale handles, replacement gating, cleanup, and a fresh third request against an occupied submitted slot plus in-flight wire request. Safety invariants and eventual settlement of admitted attempts, pending publications, and cleanup are checked; weak fairness applies only to local dequeue, time, expiry, validation, publication, and cleanup actions.                                                                                            |
| Limits       | This is bounded formal evidence, not runtime-refinement evidence. It abstracts the complete-frame write as one boundary and uses one-tick finite write and read deadlines rather than a concrete duration; it does not prove partial-I/O code, system clock implementation, thread scheduling, panic cleanup, OS behavior, codec correctness, OBCS-to-OBCN policy, Host effects, durable recovery, or whether another authorized owner should retry a known-unchanged request. The two sequential phase deadlines correspond to at most twice the configured per-I/O timeout.                                                                                                                                                                                                                                                         |

The implemented runtime refinement maps `WorkerRequestHandle` to `requestHandleRef` plus `submittedSlot`
and maps the sole `WorkerWireOwner` to `wireOwnerRef`, `wireRequest`, `wirePhase`, `now`, and
`wireExpiry`. The scalar expiry is phase-indexed: `DequeueRequest` starts the total write deadline,
and `CompleteRequestWrite` replaces it with the total read deadline. `WrongWorker` and
`WrongGeneration` never enter `submittedSlot`; `Backpressure`
leaves both occupied slots unchanged; `Unavailable`, `NotDelivered`, and queued-owner shutdown map
to `known-unchanged`; and `PostDeliveryOutcomeUnknown` maps only to carrier `effect-unknown` after
`CompleteRequestWrite`. Deactivation must linearize with the owner: before complete write it is
known unchanged, after complete write it is unknown, and `ReceiveExactWorkerResult` can win only
before `RetireCarrier`. Checking an active flag only before an exchange is insufficient because it
would permit a write or newly accepted exact result after retirement. An already observed exact
result may still publish before replacement; replacement activation must wait for that publication,
matching the authorization model's pending-result guard. The carrier may not convert any transport
classification into a successful or failed OBCN result.

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

## Accepted terminal Client authority and relay modeling record

The accepted model-only slice makes the Rust Server the owner of product-level Client Sessions and
terminal authority on the relayed `Client -> Server -> Worker -> Terminal Host` path, and now checks
the reverse live-output path as `Host -> Worker -> Server -> Client`. These models describe the
accepted refinement target; they do not claim that the current runtime, wire contracts, or public
UI implement it. A viewer attachment associates one Client Session generation with a retained
terminal, while the controller is that terminal's one combined input-and-resize grant.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Several peer TUI, Web, Desktop, or mobile Client Sessions must be able to read one retained terminal concurrently without creating several input or geometry writers. A slow Client must not block another Client or Worker progress, and Client exit must detach transient authority without ending the terminal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Inputs       | Server-owned Client Session identity and generation; Worker, Host, and terminal availability; viewer attach, explicit controller acquire, input, resize, detach, disconnect, reconnect, and cleanup; opaque Host-produced OBTF snapshot/delta bytes; and the existing attachment-scoped `FrameApplied`, `NeedSnapshot`, and `ProjectionBlocked` meanings. The relay model adds abstract request identity and strictly increasing attachment-local request sequence, without selecting their wire fields or encoding.                                                                                                                                                                                                                                                                                    |
| Data         | The imported Server authority state plus one attachment generation and projection phase per modeled Session, three bounded per-attachment downstream queues for Host-to-Worker, Worker-to-Server, and Server-to-Client delivery, opaque frame values, exact feedback scope, one control request flight, used and effect-unknown request identities, and an independent Worker progress counter. One attachment-scoped projection flight occupies at most one downstream queue until Client feedback starts its return path. No Server or Worker VT, screen, row, scrollback, or history state exists in the model.                                                                                                                                                                                      |
| Outputs      | Every live viewer can receive attachment-scoped snapshots, deltas, or `ProjectionBlocked`; applied-frame acknowledgements and `NeedSnapshot` return on the same attachment scope. Input and resize reach Host effect only for the one imported controller; non-controller, out-of-order, and reused-identity requests are rejected. A full downstream queue can only pause or close that attachment, without changing another attachment's queues or disabling independent Worker progress.                                                                                                                                                                                                                                                                                                             |
| Invariants   | `LocalTerminalClientAuthority` remains the sole authority: viewers are its imported viewer set, and effective input and resize authority are the same derived set with cardinality at most one. Acquire never replaces an owner and no loss, detach, or cleanup auto-promotes a viewer. Every downstream queue is bounded and attachment-scoped; relay hops preserve the complete opaque record; feedback is attachment-scoped; applied sequence never exceeds emitted sequence; an effect-unknown request is retired and can never re-enter a send phase; detach, disconnect, and stale cleanup preserve a retained terminal.                                                                                                                                                                          |
| Decisions    | A controller owns input and resize together. Another viewer may acquire only after explicit release leaves the grant unowned; there is no automatic promotion, implicit claim, or modeled forced takeover. Server selects product authority, Worker remains the sole Host broker, and Host remains the PTY, VT, projection, and history owner. Unknown input or resize outcome consumes its abstract request identity and is never automatically replayed. `NeedSnapshot` abandons queued delivery for that attachment and requests a fresh snapshot; `ProjectionBlocked` blocks only that attachment until Host emits a later-revision snapshot rather than moving projection ownership into the relay.                                                                                                |
| Model        | [`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) explicitly `EXTENDS LocalTerminalClientAuthority` and derives read, input, and resize attachments from the imported `viewers` and `controller`; it does not create a parallel authority machine. Its control-outcome, projection-feedback, backpressure-isolation, reattach-feedback, attachment-lifecycle, and failure-interleavings configurations split unrelated finite histories while exercising one state machine. The registered source digest includes the imported authority module.                                                                                                                                                                                                                                                |
| Transitions  | Session open and attachment establish `needs-snapshot`; exact opaque records traverse the three queues without decoding; Client apply creates a `FrameApplied` acknowledgement; `NeedSnapshot` clears that attachment's in-flight records; `ProjectionBlocked` can arise before the first snapshot or during streaming, enters a blocked phase, and recovers only through a later-revision snapshot. Queue saturation can pause and then close only the addressed attachment. Detach clears delivery and the controller grant if owned while retaining the terminal; re-attach advances the attachment generation, resets attachment-local relay state, and requires a new snapshot. Disconnect/reconnect and stale cleanup remain generation-scoped.                                                   |
| Dependencies | The model maps the package-owned Terminal Protocol attachment, input, resize, OBTF, projection-demand, and projection-feedback semantics onto the existing Server-Worker spine and Worker-owned Host broker. This slice adds no codec, wire message, runtime dependency, direct Client-Worker path, or UI behavior. A future implementation must refine these actions to concrete carriers and Host commands without giving Server or Worker VT/history authority.                                                                                                                                                                                                                                                                                                                                      |
| Ownership    | Server owns Client Session generations, viewer membership, combined controller selection, relay queue admission, rejection, and stale cleanup. Worker owns Host connection brokering but not terminal presentation state. Host owns retained PTY lifetime, Ghostty VT, immutable attachment-scoped OBTF production, projection/history state, and low-level capability enforcement. The existing Host resize-owner promotion mechanism is not adopted as Server product policy.                                                                                                                                                                                                                                                                                                                         |
| Verification | The authority configuration checks three Client slots, two Session generations, bounded input/geometry revisions, all semantic actions, safety, and cleanup progress. Relay configurations check two Clients and attachment generations, two request identities and request/frame sequences, two distinct opaque records, depth-one queues, exact source-record equality across every relay hop, blocked-projection recovery, stale-feedback rejection, and control/output interleavings with Client, Server, Worker, and Host loss. Independent Worker progress remains queue-independent and completes under weak fairness unless Worker is lost. Every relay action requires nonzero coverage. These are exhaustive finite-model bounds, not release budgets; no runtime refinement test exists yet. |
| Limits       | The models cover one retained terminal, one attachment per modeled Client Session, one abstract control flight, and record-count queues. They do not define or prove wire schema/encoding, concrete request fields, production queue count or byte budgets, slow-Client thresholds or pause-versus-close policy, production scheduler/fairness guarantees, timeout values, runtime/PTY retention, restart/durable recovery, direct attachment, forced takeover, collaborative input, controller fairness, or public UX.                                                                                                                                                                                                                                                                                 |

The checked
[`LocalTerminalClientAuthority`](formal/tla/LocalTerminalClientAuthority.tla) model remains the
authority and cleanup proof, including explicit detach actions that preserve an open Session and
the terminal. [`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) composes that state with
transport-only queues and feedback. Server or Worker loss can make authority ineffective but cannot
transfer it or change the retained terminal; Host loss alone can change the terminal to `lost`.

## Ownership

This application owns projection mechanics for its injected in-memory snapshot copy plus the
experimental local Worker and Client Session registries, the application-private OBCN Worker
carrier, and the deny-only OBCS phase. It is not a second current product writer: every successfully
decoded Client terminal intent is assigned a denial decision without an effect, OBCS does not call
the Worker request handle, no Client or current runtime can submit a projection candidate, and no
publication leaves the application. The current Desktop/Workbench composition remains the delivered
Workbench writer, and the Node
[`apps/server`](../server/README.md) remains authoritative for its existing Browser ingress
behavior.

OBCS-to-OBCN mapping, semantic business transitions, authorized or successful terminal processing,
terminal relay, product authorization, persistence, recovery, external publication, endpoint
discovery and activation, production service lifecycle, Terminal Host authority/runtime
integration, retry or reconciliation policy, packaging, migration, and cutover require later
implementation slices.

The native source tree has two roots: `src/lib.rs` is the importable projection root, while
`src/main.rs` is the private binary composition root. The binary's exhaustive lateral import
directions are `main -> local_site_service`, `worker_registry`, and `worker_obcn_carrier`;
`local_site_service -> worker_registry`, `worker_obcn_carrier`, and
`local_client_service::start`; `worker_registry -> worker_obcn_carrier`; and
`local_client_service -> client_session_registry` plus its private `obcs_session`. Reverse imports,
cousin imports, and every other lateral direction are forbidden. Tests may access owner-private
state without expanding a production direction.

`worker_obcn_carrier` is not re-exported. Its cloneable request handle is available only through
the private Worker registry, and its registered wire owner receives the already admitted stream.
Only that owner's lifetime guard may retire an attached generation, after wire settlement and queue
drain; the registry may cancel only an admission that has not attached a carrier. It must not import
OBCS, Client Session, Host, projection, product authorization, effect, retry, or recovery policy.
Mapping an OBCS request into this handle requires a later accepted slice.

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
node scripts/tla/run-models.mjs --models local-client-sessions,local-client-sessions-obcs,local-site-process-spine,local-terminal-client-authority,local-terminal-relay,local-worker-obcn-carrier
pnpm verify
```
