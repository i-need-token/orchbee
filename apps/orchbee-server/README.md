# Rust Orchbee Server

Experimental, unshipped private Rust Server application. It owns the in-memory Workbench
navigation authority and private projection kernel, the first POSIX local Server-Worker process spine and serialized post-admission
OBCN carrier, local Human Client/Client Session admission, and an in-memory terminal coordinator. The emitted
executable is named `orchbee-server`; it participates in the root Cargo verification gate and is
consumed by bare Linux `orchbee` development cohorts, but not Desktop, packaging, an installer, or a service manager.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Workbench projection store

`WorkbenchAuthority` owns one in-memory Site navigation document and a bounded request/result
ledger. Admitted local Clients can read a correlated snapshot and create or rename Projects and
Workspaces through OBWP. Workspace creation commits one empty Terminal Context with its reference.
The first standalone Open also commits one shared Tab/Panel/Presentation and logical terminal binding
before launch. The local `tab.create` command appends another simple standalone Tab, jointly commits
membership and retained terminal intent, and dispatches one launch. Exact replay returns the same
pending or final result. A later Close may remove that membership without rewriting the historical
Create result. A Client can attach a viewer using a current snapshot's exact Site/Worker/Terminal
binding through OBCS. The [authority model](docs/workbench-authority.md) defines authorization, revision conflicts,
semantic replay, allocation, retirement and evidence. Reads clone the committed snapshot under the
same gate as writes; response I/O occurs after the gate is released.

`tab.rename` changes the shared Tab label. `tab.move` permutes complete standalone Tabs without
changing terminal bindings or taking the Worker registry lock. Both can complete while a launch is
pending; exact Move replay retains its original order-change result and revision. The native Move
interaction remains a separate Client integration boundary.

Local `tab.close`, status Read and Resume now use the retained lifecycle authority and one
process-lifetime maintenance owner. Known Host disposal and exact Worker/Server cleanup precede
canonical removal; Client exit does not cancel accepted Close. Linux carrier/process checks cover
eight live Client connections, idle control-slot release and same-connection reuse while an
unrelated shell survives. The [Close contract](docs/terminal-tab-close.md) owns the bounds and
remaining native TUI, recovery and release obligations.

Invalid, oversized, semantically inconsistent, or wrong-kind inputs fail through the protocol
boundary. Requests are bounded to 16 KiB before decoding; the snapshot bound remains 1 MiB.
OBWP owns the request/result envelope. The application library remains unpublished and is consumed
only by its own executable. Complete TUI/Desktop/CLI Workbench integration remains unfinished.

## In-memory advancement and publication

The authority's private `WorkbenchProjectionStore::advance_projection` accepts one complete
internally supplied snapshot. It requires the same Site and a strictly increasing revision, then
derives navigation replacement followed by changed contexts in canonical identity order. It returns
`WorkbenchPublication::Delta` within the 1,024-change and 1 MiB encoded bounds, or a complete
snapshot otherwise. An empty delta may advance revision without visible projection changes.

Before changing state, the store validates and serializes the publication. An applied delta must
reproduce the candidate exactly. Parsing, Site, revision, refinement or serialization failure leaves
state unchanged. Only the authority advances this private component. Clients cannot replace
snapshots or submit deltas. The carrier provides read-after-write; active publication/subscription
remains unfinished.

## Managed Linux Site startup

The private `--local-site <absolute-runtime-directory>` entry consumes the
[shared runtime contract](../../crates/local-site-runtime/README.md).
Server verifies the four-artifact cohort, runtime, and immutable activation marker before taking
the exclusive lifetime guard and detaching from the Client terminal session with `setsid`.
It binds the 0600 Worker socket and starts one exact cohort Worker. After registration it permanently
closes Worker admission, so no replacement generation can enroll, then atomically publishes the
permission-complete Client listener. Server startup has its own 20-second budget; the Client retains
its independent total startup deadline. This readiness means registered Server + Worker, not an
existing Host or terminal. Managed standard streams are null; this mode emits no explicit-endpoint
stdout readiness marker.

Client exit does not end the Site. Worker loss makes its capabilities unavailable without starting
a replacement; Server loss leaves the attempt marker and retained Host terminals for explicit
recovery. This entry implements no recovery, automatic cleanup, service-manager installation, or
restart. The ordinary explicit-endpoint test entry retains the behavior below.

## Local Worker service

On macOS and Linux, the private development invocation
`orchbee-server --local-endpoint <absolute-worker-path>` binds one Unix-domain Worker socket without
deleting an existing path. The additive form appends
`--client-endpoint <absolute-client-path>` to bind the separate Client socket. Selected sockets are
restricted to mode `0600`; only after every selected listener is ready does Server write the
machine readiness marker `ready\n` to stdout. Malformed explicit-endpoint invocation remains silent and exits with
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
request did not cross the complete-write boundary. The owner writes one complete OBCN Launch,
Start, or viewer request before reading exactly one result. It accepts only the matching result direction and
kind, operation ID, complete authorization echo, and, for Start, target identity. A malformed,
mismatched, timed-out, or disconnected exchange retires that Worker generation and makes queued
work unavailable.

Close uses the lifecycle-only `submit_lifecycle` boundary to distinguish successful queue admission
from later settlement. Validation, frame encoding and completion storage preparation happen before
the existing `try_send`; backpressure returns no receipt. Success returns one owned
`PendingLifecycleResult`, even if the carrier retires immediately afterward. Its
`admitted_observed_at()` is sampled after successful submission, not the exact enqueue instant.
The continuation owner may use that later observation for conservative query spacing and must
charge the admitted step even if owner-gate reacquisition happens after the attempt deadline.
`wait(self)` consumes the receipt and returns the exact typed result or original carrier error;
lost completion still distinguishes pre-write Unavailable from post-write OutcomeUnknown using the
existing complete-write marker. Dropping a receipt does not cancel the submitted operation.
This boundary adds no queue, thread, timeout, retry, step counter or scheduling policy, and does not
change the existing blocking `request()` path. Held-owner queue, timestamp bounds, backpressure,
route rejection, completion-loss and real carrier result tests cover the admission/settlement split;
the Close owner models and integrated runtime own deadline and continuation behavior.

An attached carrier remains the active registry entry for the wire owner's entire lifetime. The
only callable admission cancellation rejects attached carriers; neither the request handle nor the
registry exposes independent active-generation deactivation. Thus no replacement can be admitted
while the old owner can still write, read, validate, or publish an exact result. Owner settlement
gates retirement: on normal exit, completion-receiver loss, thread-spawn failure, or panic
unwinding, owner destruction first marks the handle inactive, conservatively settles any in-flight
request from its complete-write marker, and drains the submitted slot. Only then does its RAII
registration guard remove the matching generation; that removal is the retirement linearization
point at which replacement becomes admissible. A stale guard cannot remove a replacement.

Each complete request write has a two-second and each complete result read a twenty-second monotonic deadline;
partial progress does not restart either deadline. Failure before the request frame is completely
written is reported as `NotDelivered` and is known unchanged: strict Worker decoding cannot dispatch
an incomplete frame, and the owner retires that connection. After the complete write, any failure is
the distinct `PostDeliveryOutcomeUnknown` carrier error: Server neither fabricates the semantic OBCN
`OutcomeUnknown` value nor retries the request. An exact Worker result, including a Worker-reported
semantic unknown outcome, is preserved; the carrier does not interpret launch capability-version or
lifecycle outcome variants. Viewer results additionally match the attachment identity and action/outcome kind. Bytes received while no request is outstanding are invalid and retire the
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
its current runtime consumes authorized Launch/Start and read-only viewer requests through the Host broker.

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
malformed or wrong messages, timeout, or active-Session capacity. After completely writing
`ClientAccepted`, Server removes handshake timeouts and reads strict OBCS or OBWP on that same unbuffered
stream. The private service grants its single local effective-user subject default-shell Create and
Start, plus viewing and explicit acquisition of an exclusive input/resize controller for those terminals. It reads one request, invokes the Server-owned service, and writes the complete
correlated result before reading another request from that Session. The result is committed before
Client output; a stalled writer holds only its Session handler, never a coordinator or Worker lock.
Idle reads have no deadline until the first byte; thereafter the whole record has two seconds. Each
complete Client write also has two seconds, including large OBSR records. Expiry retires that Session.

Create selects the current local Worker and validates any exact Worker constraint. Start resolves
a successful deferred Create by its request ID across Sessions of this same subject. Server retains
the exact Worker generation and complete Host identity. Worker loss or replacement never authorizes
retargeting, re-creation, or replay. EOF, malformed, wrong-direction, oversized, truncated, read, or
write failure closes only the affected Session; generation-scoped cleanup preserves terminal state.

The private [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.md) owns
Client framing and its connecting adapter. The independent
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.md) binding owns OBCS
validation and frame I/O. Server owns subject policy, placement, operation/lease/fence issuance,
terminal binding state, bounded submission and correlated semantic results. Bare Linux `orchbee` consumes this path; persistence and production ingress remain outside this slice.

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

Within the three-generation exploration bound, `AdmissionUsesPreviousCursor` and
`GenerationCursorChangesOnlyOnAdmission` independently check that each Worker admission copies and
advances the process-local cursor. Generation exhaustion and implementation refinement remain
deterministic Rust evidence. Named rejection actions retain the abstract classification and
authority-preservation proof; exact typed or silent wire outcomes remain protocol and process-test
evidence, so the model keeps no accumulated rejection history.

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
| Limits       | This is bounded formal evidence, not runtime-refinement evidence. It abstracts the complete-frame write as one boundary and uses one-tick finite write and read deadlines rather than a concrete duration; it does not prove partial-I/O code, system clock implementation, thread scheduling, panic cleanup, OS behavior, codec correctness, OBCS-to-OBCN policy, Host effects, durable recovery, or whether another authorized owner should retry a known-unchanged request. The two sequential phase deadlines correspond to the configured write timeout plus response timeout.                                                                                                                                                                                                                                                   |

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

`LocalClientSessions` owns bounded admission, handler accounting, generation cleanup and Server
loss. `LocalClientObcsSessions` checks the same-stream framing and sequential result publication
with an injected denial handler. That handler is an explicit test instance, not the service's
terminal authorization policy. Its no-terminal-effect and not-authorized invariants apply only to
that instance. Runtime `application_session` now delegates semantic work through a callback; its unit tests
retain this instance to verify correlation, duplicate carrier records, partial I/O, failure and
read-after-complete-write ordering independently of terminal effects.

The Session model explores three Clients, two active Sessions, three handlers and bounded
generations. OBCS stream-fault and correlation configurations include coalesced handshake/request,
partial frames, stalled output, malformed direction, duplicate and distinct requests. Neither model
proves terminal authorization, Worker dispatch, cancellation, replay, persistence or live relay.
The coordinator model and process tests below own the added effect boundary.

`LocalClientSessions` records the pre-transition cursor and checks exact assignment and advance on
every admission; `ActiveGenerationsPrecedeCursor` states only the retained current-state relation.
`LocalClientObcsSessions` keeps that current-state relation as scaffold shape and does not claim a
second allocator proof. Both claims are finite and process-local; checked `NonZeroU64` exhaustion
remains runtime evidence. Named admission-rejection actions retain authority branch coverage without
an unread result label; exact wire reasons remain deterministic protocol and service evidence.

The carrier model configurations instantiate Launch/Start only. Viewer/controller scope/outcome checks and
cleanup with an internally consumed completion reuse its bounded queue and owner lifetime, with
socket tests as their concrete refinement evidence. Viewer cleanup has no external completion receiver; validated settlement is its publication, so
success keeps the shared carrier alive. Controller cleanup retains an internal completion receiver
until the exact grant is released or its Worker retires. The read-only
projection path is separately mapped to `LocalTerminalRelay`; neither model proves presentation
decoding or controller runtime behavior.

## Local Client viewer relay refinement

The accepted local-terminal milestone now connects the Server's Client Sessions to the Worker's
existing read-only viewer loop. The staged native LocalClient adapter consumes the viewer protocol. A process witness proves
two independently admitted Clients receiving real Host projections through Server and Worker. The controller refinement below adds input/resize; bare Linux startup now consumes this relay.

One admitted Session owns at most one transient viewer. Attach references an accepted terminal
Create in the same local subject; Server resolves its retained Host identity and exact current
Worker/generation, allocates fresh request, attachment, and cleanup identities, and pins that Worker
handle. Clients never supply authoritative Worker, Host, or attachment identities. Each poll moves
one complete opaque Host record; Server validates its envelope, route, and exact feedback scope,
without decoding terminal presentation. Frame delivery and feedback cannot cross Session slots.
Attach commits before Client publication. Disconnect, failed publication, or explicit detach release
the viewer and any owned controller; they never terminate or retarget the retained terminal. Unknown exchanges are not
replayed. Reattachment receives a new attachment identity and requires a snapshot.

No coordinator or registry lock spans Worker or Client I/O. Drop cleanup uses blocking admission to
the existing one-entry Worker request queue, without waiting for a Client response or adding an
unbounded queue. Retiring handlers retain their existing bounded Client Session/connection slots
until cleanup admission completes. The wire owner observes completion internally. A stopped carrier
closes the broker and all its transient attachments. Worker responses allow twenty seconds for up
to sixteen pending Host viewer/control detaches, two Attach exchanges, and result delivery; control
writes retain their two-second limit. The one queued plus one in-flight bound requires a forty-six-second launch
lease. Client record reads are idle until their first byte, then use a total two-second deadline;
Client writes also have a total two-second deadline and retire only that Session on expiry.

The existing `LocalTerminalRelay` remains the authority for attachment scope, one frame flight,
exact feedback, queue isolation, and detach lifetime. This slice maps those transitions to OBCS
viewer controls, OBSR opaque records, the existing OBCN/OBNR carrier, and Session-owned cleanup.
Evidence includes strict protocol fixtures, deterministic stale-scope/feedback tests, carrier
cleanup/backpressure tests, and real two-Client/Server/Worker/Host process traces. These establish
the private read-only path, not TUI rendering, controller authority, release performance budgets,
or independent review evidence.

An exact viewer/control ServerBusy response establishes known-unchanged terminal, viewer and
controller effects. Carrier queue rejection precedes dispatch; an Attach Worker capacity rejection
may follow an OBCN exchange but neither inserts a viewer nor dispatches a Host action. Input/Resize
may already have consumed the Client sequence, so admitted Client backpressure handling must use
both a fresh request ID and fresh effect sequence under the original logical-operation deadline.
Other unavailable, rejected or unknown results grant no authority to attempt again.

## Local terminal controller refinement

The accepted milestone extends the Session viewer with explicit acquisition of one combined input
and resize controller per retained Host terminal. Server reserves the grant before Worker I/O;
other Sessions cannot acquire while that grant is pending, active, or being cleaned up. A fresh
Server-issued controller identity scopes a strictly increasing sequence shared by input and resize.
Client-supplied identities must equal that Session's grant. Rejected non-controllers and stale
sequences never dispatch; valid submitted sequences are consumed even if delivery fails. Unknown
effects are reported without replay, and authority loss never retargets a retained terminal.

Worker maps the grant to a distinct Host attachment with input and resize enabled, read disabled,
and the resize lease requested. Existing viewer attachments remain read-only and keep their frame
baseline. Worker also enforces one grant per Host terminal, its exact viewer/grant scope, and
increasing sequences. Only the Host's exact capability revision authorizes raw input. Input contains
1 through 4,096 bytes; acceptance means admission to Host's bounded input queue, not shell execution.
Resize acknowledges the exact complete geometry. No new Host wire contract or VT owner is added.

Detach and Session loss release the control attachment before the viewer. The Server reservation
remains occupied until generation-pinned cleanup completes or that Worker becomes unavailable;
cleanup admission uses the existing bounded request queue, with no lock held across I/O. A stale
cleanup cannot clear a new grant. Failed or contradictory Host exchanges end the Worker connection;
the Server reports an unknown control outcome after complete delivery. No other viewer is promoted.
Independent release while retaining the viewing attachment and forced takeover remain later work.

At most eight viewers imply at most eight control attachments. Cleanup can therefore perform sixteen
one-second Host exchanges before the next request. The Worker response budget is twenty seconds,
launch leases last forty-six seconds, and native Client submissions allow fifty seconds. Request
writes and Client record reads/writes retain their existing two-second bounds. These are local
implementation limits, not release performance budgets.

`LocalTerminalClientAuthority` owns acquisition, controller input/resize, detach, loss, and stale
cleanup; `LocalTerminalRelay` owns increasing requests and unknown-effect retirement. Concrete
reservation, cleanup, protocol and process tests refine these claims, including competing Sessions,
wrong grant/sequence, input queue acknowledgement, observable shell output, accepted resize,
disconnect and re-acquisition. The challenge pass uses the current context and is not independent
review evidence. Public cutover, durable recovery and full Workbench acceptance
remain separate boundaries.

## Accepted terminal Client authority and relay modeling record

The bounded model slice makes the Rust Server the owner of product-level Client Sessions and
terminal authority on the relayed `Client -> Server -> Worker -> Terminal Host` path, and checks the
reverse live-output path as `Host -> Worker -> Server -> Client`. The running private Server/Worker
path and process traces described above supply implementation evidence for viewer and controller
relay. The models remain finite semantic evidence and do not prove wire codecs or public UI. A
viewer attachment associates one Client Session generation with a retained terminal, while the
controller is that terminal's one combined input-and-resize grant.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Several peer TUI, Web, Desktop, or mobile Client Sessions must be able to read one retained terminal concurrently without creating several input or geometry writers. A slow Client must not block another Client or Worker progress, and Client exit must detach transient authority without ending the terminal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Inputs       | Server-owned Client Session identity and generation; Worker, Host, and terminal availability; viewer attach, explicit controller acquire, input, resize, detach, disconnect, reconnect, and cleanup; opaque Host-produced OBTF snapshot/delta bytes; and the existing attachment-scoped `FrameApplied`, `NeedSnapshot`, and `ProjectionBlocked` meanings. The relay model adds abstract request identity and strictly increasing attachment-local request sequence; projection records separately retain bounded delivery sequence, projection generation, and content revision, without selecting their wire fields or encoding.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Data         | The imported Server authority state plus one attachment generation and projection phase per modeled Session, three bounded per-attachment downstream queues for Host-to-Worker, Worker-to-Server, and Server-to-Client delivery, opaque frame and `ProjectionBlocked` payload tags, one exact source-record witness per projection flight, independent frame sequence/projection generation/content revision, exact feedback scope, one control request flight, used and effect-unknown request identities, and an independent Worker progress counter. One attachment-scoped projection flight occupies at most one downstream queue until Client feedback starts its return path. No Server or Worker VT, screen, row, scrollback, or history state exists in the model.                                                                                                                                                                                                                                                                                                                                                                  |
| Outputs      | Every live viewer can receive attachment-scoped snapshots, deltas, or `ProjectionBlocked`. `FrameApplied` must echo the exact attachment, delivery sequence, projection generation, and content revision; `NeedSnapshot` must echo the exact attachment and delivery sequence. Wrong same-flight candidates are rejected without changing the Host-applied baseline. Input and resize reach Host effect only for the one imported controller; non-controller, out-of-order, and reused-identity requests are rejected. A full downstream queue can only pause or close that attachment, without changing another attachment's queues or disabling independent Worker progress.                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Invariants   | `LocalTerminalClientAuthority` remains the sole authority: viewers are its imported viewer set, and effective input and resize authority are the same derived set with cardinality at most one. Acquire never replaces an owner and no loss, detach, or cleanup auto-promotes a viewer. Every downstream queue is bounded and attachment-scoped; relay hops preserve the exact source record, including an opaque `ProjectionBlocked` payload tag; feedback matches the retained flight identity; blocked recovery uses a content revision strictly newer than the blocked revision; applied sequence never exceeds emitted sequence; an effect-unknown request is retired and can never re-enter a send phase; detach, disconnect, and stale cleanup preserve a retained terminal.                                                                                                                                                                                                                                                                                                                                                         |
| Decisions    | A controller owns input and resize together. Another viewer may acquire only after explicit release leaves the grant unowned; there is no automatic promotion, implicit claim, or modeled forced takeover. Server selects product authority, Worker remains the sole Host broker, and Host remains the PTY, VT, projection, and history owner. Unknown input or resize outcome consumes its abstract request identity and is never automatically replayed. `NeedSnapshot` abandons queued delivery for that attachment and requests a fresh snapshot; `ProjectionBlocked` blocks only that attachment until Host emits a later-revision snapshot rather than moving projection ownership into the relay.                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Model        | [`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) explicitly `EXTENDS LocalTerminalClientAuthority` and derives read, input, and resize attachments from the imported `viewers` and `controller`; it does not create a parallel authority machine. Its control-outcome, projection-feedback, backpressure-isolation, reattach-feedback, attachment-lifecycle, and failure-interleavings configurations split unrelated finite histories while exercising one state machine. The registered source digest includes the imported authority module.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Transitions  | Session open and attachment establish `needs-snapshot`; exact opaque records traverse the three queues without decoding; Client apply creates a `FrameApplied` acknowledgement; `NeedSnapshot` clears that attachment's in-flight records; `ProjectionBlocked` can arise before the first snapshot or during streaming, enters a blocked phase, and recovers only through a later-revision snapshot. Queue saturation can pause and then close only the addressed attachment. Detach clears delivery and the controller grant if owned while retaining the terminal; re-attach advances the attachment generation, resets attachment-local relay state, and requires a new snapshot. Disconnect/reconnect and stale cleanup remain generation-scoped.                                                                                                                                                                                                                                                                                                                                                                                       |
| Dependencies | The model maps the package-owned Terminal Protocol attachment, input, resize, OBTF, projection-demand, and projection-feedback semantics onto the existing Server-Worker spine and Worker-owned Host broker. This slice adds no codec, wire message, runtime dependency, direct Client-Worker path, or UI behavior. A future implementation must refine these actions to concrete carriers and Host commands without giving Server or Worker VT/history authority.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Ownership    | Server owns Client Session generations, viewer membership, combined controller selection, relay queue admission, rejection, and stale cleanup. Worker owns Host connection brokering but not terminal presentation state. Host owns retained PTY lifetime, Ghostty VT, immutable attachment-scoped OBTF production, projection/history state, and low-level capability enforcement. The existing Host resize-owner promotion mechanism is not adopted as Server product policy.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Verification | The authority configuration checks three Client slots, two Session generations, bounded input/geometry revisions, all semantic actions, safety, and cleanup progress. Relay configurations check two Clients and attachment generations, two request identities, bounded delivery sequence/projection generation/content revision, two distinct opaque payload tags, depth-one queues, exact source-record equality across every relay hop, strictly newer blocked-projection recovery, same-or-older recovery rejection, wrong sequence/generation/revision feedback rejection, stale-feedback rejection, and control/output interleavings with Client, Server, Worker, and Host loss. Independent Worker progress remains queue-independent and completes under weak fairness unless Worker is lost. Every relay action requires nonzero coverage. These are exhaustive finite-model bounds, not release budgets; viewer and controller runtime evidence is recorded above. Both the authority configuration and all six relay configurations require completed TLC checks with action coverage; a timeout supplies no progress evidence. |
| Limits       | The models cover one retained terminal, one attachment per modeled Client Session, one abstract control flight, and record-count queues. They do not define or prove wire schema/encoding, concrete request fields, production queue count or byte budgets, slow-Client thresholds or pause-versus-close policy, production scheduler/fairness guarantees, timeout values, runtime/PTY retention, restart/durable recovery, direct attachment, forced takeover, collaborative input, controller fairness, or public UX.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |

`LocalTerminalRelay` gives a frame four independent identity fields: attachment incarnation,
delivery sequence, projection generation, and content revision. Exact `FrameApplied` matches all
four; exact `NeedSnapshot` matches attachment and sequence and uses zero generation/revision in this
quotient. `RejectWrongFrameAppliedSequence`, `RejectWrongFrameAppliedGeneration`,
`RejectWrongFrameAppliedRevision`, and `RejectWrongNeedSnapshotSequence` make the mismatch classes
explicit. `FeedbackHasExactAttachmentScope`, `FeedbackHasExactFlightIdentity`, and
`MismatchedFeedbackIsEffectFree` require those mismatches to preserve Host-applied projection state.

`RelaySessionRefs` is a representation quotient only. `attachment-lifecycle` is the only scenario
that enables Session reconnect and retains both configured Session generations, so reconnect and
stale cleanup remain live. Each of the other five scenarios opens only the initial Session
generation; non-sentinel Session references therefore stay at generation one, and the generation-two
entries in all seventeen relay-owned maps would remain at their `RelayInit` values. Those constant
entries are omitted from relay-owned maps and quantifiers while both Clients and every configured
attachment, delivery-sequence, projection-generation, content-revision, request, payload, queue, and
Worker-progress bound remain unchanged. `reattach-feedback` still reaches attachment generation two;
that independent attachment axis is not quotiented. The configured invariant
`CurrentRelaySessionsAreRepresented` checks that every open Session remains represented.

`ProjectionBlocked` keeps an independent revision and an uninterpreted opaque payload tag; reason
semantics remain outside the model. `RelayedRecordsPreserveSourceRecord` checks exact record
preservation across all three queues, and `SourceRecordIsUnique` ties each flight to one source
record. `RejectBlockedRecoveryWithoutNewRevision` rejects a recovery snapshot whose content revision
does not exceed the blocked revision. `BlockedRecoveryRejectionIsEffectFree` and
`BlockedRecoveryRequiresNewerRevision` independently check that boundary. The independent Worker
progress counter remains a relay-level noninterference abstraction; production scheduler fairness is
outside its scope.

The checked
[`LocalTerminalClientAuthority`](formal/tla/LocalTerminalClientAuthority.tla) model remains the
authority and cleanup proof, including explicit detach actions that preserve an open Session and
the terminal. [`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) composes that state with
transport-only queues and feedback. Server or Worker loss can make authority ineffective but cannot
transfer it or change the retained terminal; Host loss alone can change the terminal to `lost`.

`ViewerTransitionsAreExact` independently checks every viewer transition against its predecessor:
attach adds only the addressed current Session; detach, disconnect, and cleanup remove only the
addressed Session; disconnect of an unattached Session preserves the set. Every unrelated viewer is
therefore preserved without adding the predecessor set to the model state.

## Local terminal coordinator modeling record

The accepted local milestone grants the Server's single effective-user subject default-shell
Create and Start through admitted local Human Client Sessions. OBLC alone remains only transport
admission; this private service explicitly selects that policy after peer and cohort validation.
No grant is inferred for another endpoint, remote identity, Worker peer, or renderer kind.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Connect admitted Client Create/Start intent to the existing Worker carrier without transferring placement authority, replaying effects, or losing retained bindings when a Client disconnects.                                                                                                                                                                                                                                                                                            |
| Inputs       | Strict OBCS intent, the current admitted Worker identity/generation, and an adapter-supplied absolute lease expiry. No Client field becomes an operation, terminal, lease, generation, or fence.                                                                                                                                                                                                                                                                                          |
| Data         | One Server-owned in-memory map of at most 1,024 create references for its single local subject; each retains exact placement, last authorization, launch mode, and pending, awaiting-start, started, unknown, or unavailable state. No eviction or durable recovery.                                                                                                                                                                                                                      |
| Outputs      | A correlated OBCS result or one exact OBCN command. Missing Worker and unsatisfied constraints are effect-free. Start resolves a successful deferred Create across Sessions; unknown/missing creates cannot dispatch.                                                                                                                                                                                                                                                                     |
| Invariants   | A checked monotonic 128-bit allocator never reuses operation identity across terminals or Worker generations. Each dispatched Start advances its terminal fence and uses a fresh lease. One terminal has at most one pending operation. No synchronous lock spans Worker I/O or Client output.                                                                                                                                                                                            |
| Decisions    | Known create-reference collisions reject; full retained capacity and concurrent operations return server-busy. Same-geometry Start after confirmed deferred Start returns already-started without another command; different geometry rejects. Immediate creates reject Start. Worker replacement never retargets a retained binding.                                                                                                                                                     |
| Model        | A pure prepare/complete state machine reserves a binding before effects and commits through exact results or explicit delivery failures. Independent Session output cannot block another Session’s next operation.                                                                                                                                                                                                                                                                        |
| Transitions  | Prepare reserves state before carrier submission. Exact current success commits Host identity and geometry before Client publication. Known pre-delivery Start failure restores awaiting-start; ambiguous delivery or contradictory success freezes the binding as unknown. Previously confirmed Host identity survives failed or unknown Start. Client loss cannot cancel the submitted operation or remove the binding.                                                                 |
| Dependencies | The pure coordinator consumes protocol values and explicit time/placement facts. A private service owns short mutex sections, wall-clock sampling, Worker-handle selection and one carrier call. The existing carrier owns wire serialization and complete-write classification.                                                                                                                                                                                                          |
| Ownership    | Server owns subject policy, create references, placement, operation/lease/fence issuance and result commitment. Worker owns launch defaults, admission and Host effects. Host owns PTY/VT and retained execution. No new public or cross-workspace contract.                                                                                                                                                                                                                              |
| Verification | Deterministic coordinator traces check allocation, pending exclusion, exact result commitment, unknown quarantine, capacity, geometry, and replacement. Socket/process traces check cross-Session Start, serialization, malformed peers and Client loss. The registered launch-authorization and coordinator models check their finite safety/progress claims.                                                                                                                            |
| Limits       | Additive unshipped POSIX local authority only. The 1,024-record bound follows the accepted Worker generation limit and requires explicit retirement before production. Lease lifetime is forty-six seconds, derived from the carrier's one queued plus one in-flight exchange with a two-second write and twenty-second result deadline each; expiry can reject without retry. No replay ledger, fine-grained permissions, UI activation, restart reconciliation, or production recovery. |

The coordinator's actual consumer is the post-admission OBCS handler. Its integrated witness is a
Client issuing Create and Start through the real Server, Worker, and Host processes. The next
missing user-visible boundary is TUI activation.

[`LocalTerminalCoordinator`](formal/tla/LocalTerminalCoordinator.tla) checks one deferred binding,
two peer Sessions, three operation identities and two Worker generations. Prepare maps to
IssueCreate/IssueStart, complete maps to exact/not-delivered/rejected/unknown/stale settlement, and each
Session's independent publication can interleave another Session's operation. Progress assumes
bounded carrier completion and weak fairness for settlement and publication/abandonment; it does
not promise delivery to a disconnected Client. Geometry, capacity, type conversion and real PTY
behavior belong to deterministic tests and the process witness.

`RejectConcurrent` uses the other peer Session's one-outstanding-request slot as the bounded
correlation surrogate, queues an explicit `server-busy` result, and preserves the original
reservation. `PublicationCopiesExactPredecessor` requires publication to copy and clear that exact
Session's queued result while preserving every other Session. Exact OBCS request-ID echo remains
deterministic runtime and codec evidence.

`TerminalLaunchAuthorization` tracks each operation's modeled handoff-effect occurrences with an
independent counter whose domain includes two. `HostEffectAtMostOncePerOperation`,
`HostEffectCountMatchesObservations`, `HandoffRecordsExactlyOneEffectOccurrence`, and
`EffectCountAdvanceRequiresHandoff` check its bound, identity-set correspondence, exact increment,
and that only `HandoffToHost` may advance it. This proves at most one modeled handoff-effect occurrence per operation; Rust
duplicate-dispatch tests remain the evidence for concrete transport-write multiplicity.

## Ownership

This application owns the local Workbench authority, standalone membership workflow, private
projection mechanics, Worker and Client registries, OBCN carrier, terminal-intent coordinator, and
Session viewer/controller relay. Existing Desktop/Workbench remains the delivered Workbench writer;
[`apps/server`](../server/README.md) retains Browser ingress authority. Persistence, restart
reconciliation, production service lifecycle, packaging, migration and public cutover remain unfinished.

The importable authority boundary is `src/lib.rs`; `src/main.rs` composes the private binary.
The exhaustive lateral directions are `local_site_service -> worker_registry`,
`worker_obcn_carrier`, `local_client_service::start` and `local_terminal_service`;
`local_client_service -> client_session_registry`, `local_terminal_service`, `workbench_service`
and its private `application_session`; `workbench_service -> local_terminal_service`;
`local_terminal_service -> site_coordinator`, `WorkbenchAuthority`, `terminal_coordinator` value types, `worker_registry`
and `worker_obcn_carrier`; `site_coordinator -> WorkbenchAuthority` and `terminal_coordinator`;
`workbench_authority -> workbench_projection`; and `worker_registry -> worker_obcn_carrier`.
The service constructs the Workbench authority outside locks. SiteCoordinator owns the standalone
workflow, exact binding selection and shared commit boundary; it composes the two cores without
taking over their facts.

The terminal coordinator's private `outcomes` child owns Worker-observation projection.
SiteCoordinator's parent owns the first-terminal slot and its correspondence helpers. Its private
`standalone` child composes first membership and terminal intent, while `tab_create` composes
joint command preparation and settlement with origin-bound, consuming owner tokens;
`terminal_selection` composes canonical exposure, retained target resolution and attachment identity
preparation. WorkbenchAuthority's `terminal_binding_selection` child owns the read-only canonical exposure query;
`terminal_tab_create` owns command admission, prepared membership, reserved ledger bytes and settlement.
`terminal_tab_move` owns Move admission and prepared permutation; Site checks composition before
target/no-op evaluation. Both share command preflight; Move reuses the authority's prepared
settled-command publication.
LocalTerminalService's parent owns shared entropy acquisition; its `tab_create` child pins Worker
placement under registry-then-Site locks, dispatches outside both, and settles both owners before
notifying first-Open waiters. Shared helpers are defined in parents; children do not import siblings.
The service's `viewer_session` owns attachment lifetime; `controller_session` owns terminal
reservation, Session grants, effect sequences and exact release; `projection` owns routing and
feedback. Client `record_stream` owns post-admission I/O deadlines. Reverse and other lateral imports
remain forbidden; tests do not widen production visibility.

The carrier owns I/O, exact echo validation and generation retirement, not OBCS or product policy.
Deterministic cores import no transport, clock or process adapter. Terminal service operations take
registry then Site locks, or registry then controller-map locks for grants. Workbench access takes
only the Site gate after external OBWP policy checking for reads and pure navigation. The opaque
`WorkbenchReadOrNavigationRequest` excludes `tab.create` and `tab.move`, which need Site composition.
Move requires the navigation grant and only the Site gate; `tab.create` additionally requires
the terminal-create grant and uses the composed path above. Binding viewer selection holds registry
and Site locks, then samples the handle's active state; its internal canonical query uses OBCS
subject admission and preserves Workbench-first failure precedence. I/O releases every gate; no automatic retry is added.
The `application_session` runner dispatches exact OBCS/OBWP prefixes to separate semantic handlers
and imports only their contracts and standard-library I/O. A postcommit coordination failure closes
the mixed carrier without fabricating a command result. The unpublished application library
exposes the authority, Site grants and typed internal membership/exposure/preparation results; whole-projection writing
remains private.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-worker --locked
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --workspace apps/orchbee-server --jobs 1
pnpm verify
```

The Server model command runs one JVM at a time to avoid contention between its large state graphs.
Each configuration retains its declared TLC worker count, bounds, properties and action coverage.
Run this resource profile without other heavy builds or model checks; execution deadlines still apply.

The execution inventory gives each of the six `LocalTerminalRelay` configurations a 600-second
timeout with two TLC workers. This is an execution budget for full enumeration and final liveness
checking; completion still requires all configured properties and required action coverage.

## Local standalone terminal opening

The unshipped `terminal.open` operation opens one Server-owned standalone terminal reference for
the local single-subject native path. The first request that passes all preflights atomically commits
shared Tab/Panel/Presentation membership and terminal intent, then dispatches one immediate launch
with Worker-default cwd and the supplied geometry. Subsequent Opens join or return the same retained
outcome and Create reference. OBCS local terminal admission authorizes this operation; the internal
membership commit is separate from the OBWP navigation command ledger and its read/navigation grants.

The shared binding is the logical TerminalId and WorkerId from Server authorization, not a Host,
Client or request identity. It records product intent and survives failed or unknown launch; it
does not assert runtime availability. The [Workbench model](docs/workbench-authority.md#standalone-terminal-membership-model)
defines the canonical aggregate, preflight and shared gate. Open does not resize an existing terminal;
only an acquired controller may submit a separate validated Resize. Bare Linux startup consumes
Open followed by snapshot-based binding attachment; further Workbench operations and complete
Client Workbench rendering remain unfinished.

| Field       | Contract                                                                                                                                                                                                                                                                                                                           |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs      | Admitted OBCS Open, real geometry, current Worker placement, lease, fresh nonzero random Create reference and monotonic wait time. Request ID only correlates the response.                                                                                                                                                        |
| Owner       | SiteCoordinator owns the standalone slot and cross-owner workflow; WorkbenchAuthority owns projection/cursor, TerminalCoordinator owns terminal binding/allocation. The service owns IPC, entropy and deadlines; Client owns its request/viewer.                                                                                   |
| Model       | Empty -> Creating -> Settled. Pure terminal planning and Workbench validation precede one Site-gated commit. Dispatch and response publication occur outside the gate.                                                                                                                                                             |
| Preflight   | Missing Worker/Workbench, entropy failure, reference collision, capacity, identity, revision or lease failure leaves Empty with neither terminal nor membership. Internal Workbench preflight/publication failure retires Workbench without dispatch.                                                                              |
| Completion  | Started maps to Opened; unavailable/failed preserve Create reasons; collision/entropy/Workbench preflight refusal maps to server-busy. Unexpected fixed-input rejection maps to terminal-create-failed. Unknown stays unknown. Post-commit failure retains membership and never permits another create.                            |
| Concurrency | Wait releases the Site mutex. One absolute 46-second condition deadline returns server-busy only to that waiter, without cancellation. Worker exchange is bounded by 44 seconds; ordinary Client submission has 50 seconds, clamped to the startup deadline until first frame.                                                     |
| Lifetime    | Client loss or failed publication preserves membership. Opened requires the exact current Worker generation; loss/replacement never adopts or relaunches. Later navigation retirement cannot cancel an already committed launch.                                                                                                   |
| Limits      | In-process successful-execution atomicity under one mutex; no crash/persistence atomicity. One standalone reference; no close/reset, recovery, full Workbench operation, remote authorization or activation policy. The wait deadline does not bound mutex acquisition, suspension or response writing. Runtime evidence is Linux. |

### Standalone opening correctness

| Claim                                               | Refinement and evidence                                                                                                                     |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| SO-01: at most one prepared create                  | Reserve maps to the gated Site commit; Singleton/NoReset and core preflight/collision/concurrency tests.                                    |
| SO-02: callers share a committed reference          | Complete/Observe, SameReference/OpenedWasCommitted, exact-placement tests and real two-Session/Host traces.                                 |
| SO-03: Client loss never resets shared state        | Disconnect/Reopen, DisconnectPreservesSlot, unread-result and reopen process traces.                                                        |
| SO-04: uncertainty never creates another effect     | Complete(unknown), SettledSlotIsFinal, failed/unknown/same-Worker-new-generation core and process tests.                                    |
| SO-05: a waiter settles without Worker fairness     | Timeout/EveryWaitSettles and absolute-deadline condition tests; only local timeout observation is weakly fair.                              |
| SO-06: launch requires committed membership         | Reserve/Dispatch, MembershipCommitted/DispatchHasMembership, logical authorization comparison before Worker completion.                     |
| SO-07: reads preserve the binding across navigation | Navigate/ReadWorkbench, ReadIsCommitted/MembershipIsRetained, stateful navigation traces and a stalled-Worker process witness.              |
| SO-08: preflight consumes neither allocator         | PreflightPreservesState covers abstract membership/revision; exact identity, revision and encoded-capacity tests cover concrete allocation. |

The [model](formal/tla/LocalStandaloneTerminal.tla) separates two Clients, two Create references
and two logical bindings, with two navigation commits and one create attempt. It abstracts entropy,
encoding, allocation cursors, duplicate completion, time magnitude and scheduling. Deterministic
and stateful tests cover identity non-reuse and ignored duplicate completion; process/PTY traces
cover framing, logical-to-Host mapping, concurrency, real execution and retention.
