# Rust Orchbee Server

## Windows semantic events

The Site journal records actual accepted topology, metadata, worktree, Agent and Client changes.
An authenticated connection owns one bounded subscription; read/wait retain its cursor and probes,
while close/EOF releases the owner. Terminal output and exit use admitted Host observations, and
Client scroll/visible output use the exact read-only Client viewport. History gaps, unavailable
sources and uncertain observer cleanup remain explicit. [Event ownership and acceptance](docs/windows-events.md)
defines the lifecycle, bounds and Windows evidence; Desktop receives no new consumer here.

## Windows runtime metadata

The same-user OBLC metadata request reads this process's compiled version, retained expected
cohort, PID, terminal protocol, implementation feature bits and actual managed/stopping mode.
It runs before exact-cohort Client admission, creates no Session generation or Workbench/viewer
state, acquires no shutdown record lease, and closes after one response. Existing candidate and
handshake limits still apply, including while ordinary Client admission is stopping. These are
diagnostic facts, never an operation grant or proof of future availability. The
[runtime status contract](../orchbee/docs/native-runtime-status.md) owns CLI behavior and evidence;
the [local Client protocol](../../crates/local-client-protocol/README.md) owns framing. Only Windows
dispatches this new read; old admission and Linux/macOS runtime behavior remain unchanged.

## Durable Windows session lifecycle

For native default/named scopes, the CLI activation owner publishes an exact private Active catalog record before the managed
Server starts. Before Worker effects, the Server admits and retains this record alongside its
checkpoint store and lifetime owner. After confirmed child exit and listener cleanup, shutdown
publishes durable Stopped before completing the runtime activation record. A publication failure
leaves recovery uncertainty and never grants a successful clean-stop receipt. The runtime crate
owns catalog encoding, path admission and replacement; the Server owns when shutdown is proven.

## Managed Windows Site shutdown

The managed Server automatically saves committed Workbench changes after a 500ms debounce and
refreshes cached directories and provider resume references every 30 seconds. It retains three
valid snapshots and excludes terminal screens. Startup restores the newest valid snapshot;
unreadable copies are retained and reported to the Client, including an explicit empty-workspace
notice if every copy is invalid. The [checkpoint contract](docs/workbench-checkpoint.md) owns
capture, physical-directory fences and restoration; Site-owner admission remains a runtime decision.

One stop identity owns the remaining managed Server lifetime. Accepting it atomically closes new
application-record admission; existing admitted operations and result writes settle before cleanup.
Duplicate start and status requests observe the retained attempt; a different identity is busy.
Only explicit retry of the same failed identity starts another attempt, up to 64. Failure never
reopens ordinary admission or authorizes an uncertain Host mutation replay. Each attempt has one
60-second cooperative deadline, excluding uninterruptible operating-system calls.

The owner retains the exact Worker child, drains and joins its single checkpoint saver, joins the
periodic process sampler and the retained Worktree/Workspace-group owners, then stops Close and
graphics maintenance. It takes the fresh final capture/publication only after those writers and
samplers settle, freezes the Worker carrier to shutdown-only traffic (settling queued unsent work as unavailable), and sends one bounded Worker
shutdown request. Uncertain sampler viewer cleanup remains owned until the matching Worker/Host stop and actual Worker exit. A known no-Host/Host-stopped result and exact Worker exit must precede stopping
Agent observation and Client listeners, waking and joining retained Client handlers, and publishing
the stopped activation record. An activation guard excludes concurrent restart during publication
and lifetime-lock release. Pipe absence or lost replies cannot prove completion. Failed attempts
remain queryable while the control listener survives; after listener closure, the strict activation
record remains a positive receipt. No process discovery, adoption, path removal or platform stop
implementation for Linux/macOS is part of this owner.

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
ledger. Navigation stores global, identity-sorted `directories`, `projects` and `workspaces` arrays
and an explicit `workspaceOrder` permutation. A Project contains only its identity and label;
each Workspace has a required nullable `projectId`, so ungrouped Workspaces are ordinary members
of the same document. Directories are global resources independent of Project membership.
The graph admits at most 65,536 Directories, 1,023 Workspaces and 256 Workspaces per Project,
subject to the snapshot byte bound.

Admitted local Clients can create or rename Projects and Workspaces through OBWP.
`workspace.create` accepts a nullable Project and commits one empty Terminal Context with its
reference. `workspace.set-project` groups or ungroups an existing Workspace;
`workspace.create-project` atomically creates a Project and moves that Workspace into it;
`project.dissolve` removes the Project metadata and ungroups its members. These operations preserve
Workspace identities, directory references, contexts, Tabs, Panels, terminal bindings and working
directories. They perform no filesystem access or terminal creation. Workspace rename, Close and
primary-directory acceptance target the stable Workspace identity without a Project parameter.
The independently authorized `workspace.primary-directory.accept` validates a local Worker directory
and atomically adds a global Directory selected by that empty Workspace. The
[directory contract](docs/workspace-terminal-directory.md) owns validation, replay and failures.
The `worktree-list` read re-verifies one stable Directory identity against the committed
snapshot, then dispatches one fenced OBCN discovery job to that directory's owning Worker over
the existing carrier; the answer is the Worker's bounded raw `git worktree list --porcelain -z`
bytes parsed at the service boundary — membership and generation checks run under the locks,
request I/O never does.
The first standalone Open also commits one shared Tab/Panel/Presentation and logical terminal binding
before launch. The local `tab.create` command appends a simple standalone or configured Workspace Tab, jointly commits
membership and retained terminal intent, and dispatches one launch. Exact replay returns the same
pending or final result. A later Close may remove that membership without rewriting the historical
Create result. A Client can attach a viewer using a current snapshot's exact Site/Worker/Terminal
binding through OBCS. The [authority model](docs/workbench-authority.md) defines authorization, revision conflicts,
semantic replay, allocation, retirement and evidence. Reads clone the committed snapshot under the
same gate as writes; response I/O occurs after the gate is released.

`tab.rename` changes the shared Tab label. `tab.move` permutes complete simple Tabs within a standalone or Workspace context without
changing terminal bindings or taking the Worker registry lock. Both can complete while a launch is
pending; exact Move replay retains its original order-change result and revision. The native Move
interaction remains a separate Client integration boundary.

`tabs.configure` atomically changes group/pin metadata and order through the same authority gate.
It allocates only a new group identity, rejects unfinished context Close operations, and has no
Worker effects. The [native group contract](../orchbee/docs/native-tab-groups.md) defines its exact
transition and receipt checks; the shared conformance corpus compares Rust and GUI ordering.

`panel.split` is the first tree mutation: it jointly commits the source leaf, a new leaf, split
orientation and both slot identities before dispatching the second immediate terminal launch. The
Server retains exact tree membership, and aggregate Close now disposes every leaf in a captured Tab
before publishing one removal. The Linux process witness creates two real terminals, observes the
Split projection, closes the Tab and verifies that the Host has no remaining terminal or exits after
the Worker disconnects. The Linux native Client now projects that canonical split tree and moves
local focus across its validated leaves; it still attaches one viewer at a time and does not submit
`panel.split` from a TUI shortcut.

Local standalone and Workspace `tab.close`, leaf-scoped `panel.close`, status Read and Resume use
the retained lifecycle authority and one process-lifetime maintenance owner. A Panel close captures
one exact leaf, disposes and cleans it through the same lifecycle, and publishes the surviving Tab
with the leaf's parent Split collapsed into the sibling subtree; a Tab's only leaf closes as an
aggregate Close. Known Host disposal and exact Worker/Server cleanup precede
canonical removal; Client exit does not cancel accepted Close. Linux carrier/process checks cover
thirty-two live Client connections, idle control-slot release and same-connection reuse while an
unrelated shell survives. The [Close contract](docs/terminal-tab-close.md) owns the bounds and
separately evidenced native TUI integration and remaining production recovery and release obligations.

`panel.move` is the lifecycle-free tree operation: it relocates one leaf beside another leaf in the
same Tab, collapsing the source parent Split and committing one fresh Split with two fresh Slots in
a single revision while the moved leaf keeps its Panel, Presentation and terminal binding exact.

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
snapshots or submit deltas. The carrier provides read-after-write; every committed publication is
retained in a 64-entry journal and served to readers through `workbench.publication.read` under the
same read grant, so a Client holding a baseline revision fast-forwards with ordered publications
instead of full snapshots. A journal that cannot connect the caller's revision answers with one
complete snapshot boundary publication. Push subscription remains unfinished.

## Managed Linux and macOS Site startup

The private `--local-site <absolute-runtime-directory>` entry consumes the
[shared runtime contract](../../crates/local-site-runtime/README.md).
Server verifies the four-artifact cohort, runtime, and immutable activation marker before taking
the exclusive lifetime guard and detaching from the Client terminal session with `setsid`.
On Linux it binds the 0600 Worker socket with no-replace publication; on macOS it binds the final
socket path inside the private runtime directory, applies mode `0600`, and verifies the path is a
single-link socket owned by the effective user. It starts one exact cohort Worker. After
registration it permanently closes Worker admission, so no replacement generation can enroll, then
atomically publishes the permission-complete Client listener. Server startup has its own 20-second budget; the Client retains
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
submissions into two bounded FIFO lanes under one queue lock — high for interactive and
cleanup work, low for viewer observation — each capped at eight requests behind the one request
possibly in flight. Ordinary submissions try-admit and receive immediate `Backpressure` when
their lane is full — high-lane submissions also back off while a cleanup waiter is queued;
only the two session-cleanup producers wait FIFO with priority over later try-admits. The owner dispatches at most four consecutive
high-lane requests while the low lane is non-empty, then serves one low-lane request.
`Unavailable` and `Backpressure` both mean that the addressed request did not cross the
complete-write boundary. The [carrier scheduling model](docs/worker-carrier-model.md) owns
lane classification, admission, dispatch bounds, drain and failure settlement. The owner writes one complete OBCN Launch,
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
Server accepts up to thirty-two active Sessions and at most forty-eight total
candidate/session handler threads. Each handshake has a two-second read/write timeout. One silent candidate therefore cannot
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
Idle reads have no deadline until the first byte; thereafter the whole record has two seconds,
including large `OBCQ` paste records. Each
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

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | The verified native artifacts had no real process relationship. This Worker sub-spine must prove that a separate Server can admit and retain one separate same-user compatible Worker without creating a terminal, activation policy, or durable state.                                                                                                                              |
| Inputs       | An untrusted private argument vector, an absolute endpoint path supplied by the development owner, kernel peer credentials, and one bounded framed Worker handshake. Absence, malformed bytes, timeout, wrong message, wrong user, version mismatch, cohort mismatch, duplicate Worker, and disconnect are explicit.                                                                 |
| Data         | Server phase, optional active Worker identity plus nonzero process-local generation, next generation, and one candidate connection. Server is the sole writer. No value survives process exit.                                                                                                                                                                                       |
| Outputs      | Successful bind emits one `ready\n` marker. Worker admission returns its generation; an authenticated protocol-level rejection returns a typed bounded reason when the stream permits it. Peer-authentication and transport-setup failures close silently. Process setup or listener failure exits status `1` without a human diagnostic.                                            |
| Invariants   | One endpoint has at most one listener; one Server has at most one active Worker; active generations are nonzero and strictly increase; rejection cannot replace active authority; generation-scoped cleanup cannot remove a replacement; Worker disconnect does not stop Server; Server loss closes the Worker connection.                                                           |
| Decisions    | Invocation validity, peer-user equality, frame validity, protocol and cohort equality, expected Worker message kind, active-Worker absence, and matching cleanup generation completely determine the result. No current directory, Project, Environment, Worktree, Agent, or Terminal fact participates.                                                                             |
| Model        | A deterministic Worker registry owns admission and generation-scoped disconnect. A thin synchronous Unix-socket workflow authenticates, decodes, admits or rejects, acknowledges, and monitors the accepted connection.                                                                                                                                                              |
| Transitions  | `absent -> listening`; candidate `none -> pending -> admitted/rejected/timed-out`; Worker `none -> active(generation) -> none`; Server `listening -> stopped`. A second bind is rejected, and stale cleanup is an idempotent no-op against another generation.                                                                                                                       |
| Dependencies | Rust standard-library process, filesystem, Unix socket, timeouts, threads, and I/O plus the private local-site protocol and kernel peer-credential crates. No terminal, child process, network socket, async runtime, persistence, environment lookup, or randomness.                                                                                                                |
| Verification | Pure codec and registry tests; native same-user socket-pair tests on macOS and Linux; real Server and Worker process tests for bind, readiness, admission, duplicate and incompatible rejection, replacement after disconnect, Server loss, malformed peers, exact output, and cleanup bounds. These implementation traces do not constitute an exhaustive admission protocol proof. |
| Limits       | This Worker evidence covers macOS and Linux under the same effective user. It does not establish Windows behavior, cross-user negative execution, production endpoint discovery, crash cleanup, service activation, Client Session semantics, terminal retention, or remote security.                                                                                                |

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
| Model        | The carrier state owner separates registry identity, cloneable request-handle authority, queue occupancy, sole wire ownership, write completion, actual response arrival, local validation, deadline expiry, publication, retirement, and cleanup. `ReceiveExactWorkerResult` validates and preserves `incomingResponse`; it does not construct a matching result. The model refines the post-OBLS boundary and OBCN semantic contract rather than reimplementing either byte codec.                                                                                                                                                                                                                                                                                                                                                  |
| Transitions  | `ActivateCarrier` publishes one generation; `SubmitFirstLaunch`, `SubmitSecondLaunch`, and `SubmitStart` exercise distinct candidates; `DequeueRequest` moves one value to the owner; `CompleteRequestWrite` crosses the ambiguity boundary; `AdvanceMonotonicTime` enables pre- and post-write expiry; exact arrivals and one named arrival action per mismatch class populate `incomingResponse`; `ReceiveExactWorkerResult` or `RejectMismatchedWorkerResult` validates it; disconnect actions cover idle, before-write, after-write, queued-after-write, and after-result orderings; publication, late-result discard, replacement, stale rejection, and generation-matched cleanup settle retained state.                                                                                                                        |
| Dependencies | The target composes the existing Server Worker registry and post-`WorkerAccepted` stream with the private local-site and capability-node protocol contracts. The local-site protocol owns OBLS framing, the capability-node protocol owns OBCN values and strict framing, and the carrier composes them. No OBCS mapping, authorization decision, Host dispatch, terminal retention, persistence, endpoint discovery, or remote ingress participates.                                                                                                                                                                                                                                                                                                                                                                                 |
| Ownership    | Server's registry owns current Worker generation and request-handle publication. One `WorkerWireOwner` equivalent exclusively owns stream reads and writes plus queue draining; request-handle clones can only submit. The codec owns byte validity, Worker owns the semantic OBCN result, and a later Server policy owner—not this carrier—owns authorization, reconciliation, or any permitted retry of a separately proven known-unchanged operation.                                                                                                                                                                                                                                                                                                                                                                              |
| Verification | Carrier tests cover exact correlation, response-field/payload mismatch, expiry on each write side, disconnect ordering, stale handles, replacement gating, cleanup and occupied queue capacity. Deterministic clocks check local settlement; no separate carrier TLA proof is retained.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
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
classification into a successful or failed OBCN result. The single submitted-slot occupancy this
record explored has since been superseded by the dual bounded lanes in the
[carrier scheduling model](docs/worker-carrier-model.md); the wire-phase, ambiguity-boundary and
settlement obligations are unchanged.

## Local Client Session and OBCS modeling record

The Session model explores three Clients, two active Sessions, three handlers and bounded
generations. OBCS stream-fault and correlation configurations include coalesced handshake/request,
partial frames, stalled output, malformed direction, duplicate and distinct requests. Neither model
proves terminal authorization, Worker dispatch, cancellation, replay, persistence or live relay.
The coordinator model and process tests below own the added effect boundary.

## Local Client viewer relay refinement

The accepted local-terminal milestone now connects the Server's Client Sessions to the Worker's
existing read-only viewer loop. The staged native LocalClient adapter consumes the viewer protocol. A process witness proves
two independently admitted Clients receiving real Host projections through Server and Worker. The controller refinement below adds input/resize; bare Linux startup now consumes this relay.

One admitted Session owns at most eight transient viewers on its existing ordered carrier. Attach
references an accepted terminal Create in the same local subject; AttachBinding supplies an exact
canonical binding selector. Server resolves the retained Host identity and current Worker/generation,
allocates fresh request, attachment and cleanup identities, and pins that Worker handle. Clients
cannot allocate authoritative runtime identities; every subsequent viewer/control action must echo
one Server-issued attachment ID owned by the issuing Session. Each poll moves one complete opaque
Host record; Server validates its envelope, selected attachment and exact feedback scope without
decoding terminal presentation. Every viewer owns its own projection flight. Equal counters on two
flights do not authenticate content: the explicit selector determines feedback ownership. A Poll also
drains the viewer's kind-53 `HistoryChanged` event lane: the Worker keeps a bounded per-viewer event
FIFO beside the frame record slot and alternates the two sources through an explicit
next-preference, so a queued event is deliverable while a frame flight is unsettled and a
continuously ready source overtakes the other at most once. A kind-53 record is a valid result only
for `Poll`; surfacing one to any other action is a strict route error that never reaches
`projection.receive`. Event delivery never marks the frame flight delivered, and `HistoryObserved`
rides its own acknowledgement flight rather than settling `FrameApplied`. Event FIFO or byte-budget
saturation closes only that viewer, and frame plus event bytes are released exactly once on every
teardown path; a valid kind-53 event for an already-retired viewer is swallowed like a late frame.
Attach commits before Client publication. Explicit detach releases only the selected viewer and its
controller; Session disconnect or failed publication cleans all its viewers. These operations never
terminate or retarget retained terminals. Unknown exchanges are not replayed. Reattachment receives
a fresh attachment identity and requires a snapshot.

No coordinator or registry lock spans Worker or Client I/O. Drop cleanup uses blocking admission to
the existing one-entry Worker request queue, without waiting for a Client response or adding an
unbounded queue. Retiring handlers retain their existing bounded Client Session/connection slots
until cleanup admission completes. The wire owner observes completion internally. A stopped carrier
closes the broker and all its transient attachments. Worker responses allow twenty seconds for up
to sixteen pending Host viewer/control detaches, two Attach exchanges, and result delivery; control
writes retain their two-second limit. The one queued plus one in-flight bound requires a forty-six-second launch
lease. Client record reads are idle until their first byte, then use a total two-second deadline;
Client writes also have a total two-second deadline and retire only that Session on expiry.

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
Client-supplied identities must equal the selected viewer's grant in the issuing Session. Rejected non-controllers and stale
sequences never dispatch; valid submitted sequences are consumed even if delivery fails. Unknown
effects are reported without replay, and authority loss never retargets a retained terminal.

Worker maps the grant to a distinct Host attachment with input and resize enabled, read disabled,
and the resize lease requested. Existing viewer attachments remain read-only and keep their frame
baseline. Worker also enforces one grant per Host terminal, its exact viewer/grant scope, and
increasing sequences. Only the Host's exact capability revision authorizes raw input. Input contains
1 through 4,096 bytes; acceptance means admission to Host's bounded input queue, not shell execution.
Resize acknowledges the exact complete geometry. No new Host wire contract or VT owner is added.

Host cleanup releases the control attachment before the viewer. Explicit OBCN Detach performs that order inside Worker; the later Server lease Drop sends an idempotent Release to settle its registry. Session Drop submits OBCN Release before Detach. The Server reservation
remains occupied until generation-pinned cleanup completes or that Worker becomes unavailable;
cleanup admission uses the existing bounded request queue, with no lock held across I/O. A stale
cleanup cannot clear a new grant. Failed or contradictory Host exchanges end the Worker connection;
the Server reports an unknown control outcome after complete delivery. No other viewer is promoted.
Explicit takeover now follows the [controller takeover model](docs/terminal-controller-takeover.md):
Pending reservations fence both competing takeover and stale effects; the candidate owns old/new
cleanup until acknowledged transfer. Independent release while retaining the Viewer remains later work.

At most eight viewers imply at most eight control attachments. Cleanup can therefore perform sixteen
one-second Host exchanges before the next request. The Worker response budget is twenty seconds,
launch leases last forty-six seconds, and native Client submissions allow fifty seconds. Request
writes and Client record reads/writes retain their existing two-second bounds. These are local
implementation limits, not release performance budgets.

## Local terminal send

An admitted Client may submit one non-interactive input send against an exposed Workbench terminal
it could already close. The selector names the exact site, worker, and terminal binding the Server
validates at that admission; it is not an implicit incarnation lock taken from an earlier list
result. Send is a new explicit orchestration-input authority, not a read-only query and not a
zero-permission change; it adds no remote exposure and bypasses no cohort, peer, or Host capability
check.

Send admission resolves Workbench binding exposure, a unique retained binding, the current Worker
placement and the binding's recorded Worker generation, a Started terminal phase, an Unfenced
lifecycle, and no accepted close. A not-yet-started, closed, fenced, or stale-generation target
rejects before dispatch and consumes no Server-issued identity; resolution never rebinds a stale
incarnation. On admission the Server mints a fresh operation identity and a fresh input-only
attachment identity and pins the exact worker id, generation, Host epoch, and Host terminal id into
exactly one OBCN request; the Worker never re-resolves a logical identifier. If the pinned Worker
exits or changes generation before dispatch, the request fails unavailable or unknown and is never
resent.

The Worker runs one ordered attach, input, detach sequence on its existing Host broker; no viewer,
controller, or resize grant is borrowed. Acceptance means the input entered the Host's bounded
input queue; it says nothing about the child process reading or executing it. A typed Host
rejection still runs the one detach. After a transport failure, a wrong acknowledgement, or an
uncertain detach, the Worker closes that Worker-Host connection — whose teardown reclaims the
temporary attachment — and never continues a new exchange on a possibly desynchronized stream.
Every result keeps two independent facts: input status (not-submitted, rejected, accepted, or
unknown) and cleanup status (not-needed, confirmed, or unconfirmed). A post-acceptance cleanup
failure preserves the accepted byte count and never reports the input as unsent.

Each send submits one OBCN request whose identity correlates exactly one request and response; no
persistent dedupe ledger is added, and a manually retried send is a new request that may deliver
input twice. Sends serialize on the single Worker carrier alongside controller exchanges, so input
order equals arrival order. The existing twenty-second Worker response budget, two-second write
bounds, and fifty-second Client submission window all hold; three one-second Host exchanges fit
inside them without enlargement.

## Accepted terminal Client authority and relay modeling record

`RelayDeliveryIsolation.tla/.cfg` owns delivery isolation. Two viewers each have independent frame/history queues and one in-flight record per lane. Two source records and a one-slot queue expose both successful draining and overflow. An overflowing viewer closes without clearing its peer's work. Feedback must name the exact delivered record, history delivery preserves the frame flight, and a stalled viewer cannot disable Worker progress. This is a safety model; no network-delivery fairness is assumed.

`Frame`/`History` map to relay enqueue, `DeliverFrame`/`DeliverHistory` to viewer delivery, feedback actions to acknowledgement, and `Close` to viewer-local cleanup. Exact bytes, production capacity, request IDs, attachment generations and codecs are checked by `local_terminal_service/viewer_session` and Worker carrier tests. Authority and cleanup ordering are owned by `SessionViewerOwnership`; these separate models do not prove their full product composition.

The mutation suite accepts wrong-flight feedback and requires a counterexample. Run `node scripts/tla/run-models.mjs --models local-terminal-relay` and Server/Worker implementation tests.

## Accepted Session Viewer routing modeling record

`SessionViewerOwnership.tla/.cfg` checks two immutable viewer identities, two Sessions, two targets and two Worker epochs. Attach explores every Session/target assignment, including same-target contention. A controller grant remains reserved while its owner closes; controller release precedes viewer retirement, and stale cleanup cannot release a successor's grant. Control admission validates Session, target, viewer and Worker epoch together. Target disposal or Worker replacement closes affected bindings. Weak fairness of controller/viewer release establishes eventual retirement.

`Attach`, `Acquire`, `Control`, `Close` and the release actions map to `local_terminal_service/viewer_session`, `controller_session` and Worker carrier ownership. Viewer identities are never reused within the model. Capacity arithmetic, candidate attach failures, frame/history routing, wire validation and compound operation results belong to the existing service unit tests and `tests/client_process` traces. They are not exhaustive claims of this identity core. The relay model independently checks lane isolation.

The maintained late-cleanup mutant must violate `StaleCleanupPreservesSuccessor`. Run `node scripts/tla/run-models.mjs --models session-viewer-routing` plus the owning Server tests.

## Accepted Session Viewer history read modeling record

This accepted slice adds one one-shot retained-history read to an existing admitted viewer. It
extends the same Session/Server/Worker/Host routing spine with a single correlated exchange and no
new long-lived state. The [OBCS contract](../../schemas/client-server/README.md) owns the wire
representation; the OBCN viewer section of the capability-node README owns the tag-6 encoding.

| Field          | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | A one-shot consumer (the native CLI) needs bounded retained scrollback plus visible rows without keeping a viewer, consuming the live feed, or inventing a second read path.                                                                                                                                                                                                                                                                                                  |
| Inputs         | `read-rows` action: owned `attachmentId`, absolute `startRow`, `rowCount` 1–4096, exact pinned `revision`/`generation`, positive `requestSequence`, `maximumBytes` 112–8,388,608 bounding the OBRW payload. The Client computes `startRow`/`rowCount` from one validated frame (`K = min(N, S, 262144 / columns)`, `start = S - K`); Server/Worker treat the action as untrusted and re-validate every field.                                                                 |
| Data and owner | `LocalTerminalSession` still owns the viewer entry and its pinned Worker handle. Server signs one fresh Worker `OperationId` per dispatch; that identity alone is the inner Host correlation and is never equated with the outer OBCS `requestId`. No per-request history state survives the exchange.                                                                                                                                                                        |
| Outputs        | Exactly one of: complete correlated OBTP response record through OBNR/OBSR, `RowsRejected` with one of the nine closed reasons, or the existing explicit `Rejected`/`Unavailable` outcomes. A byte-budget overrun is a typed failure, never a partial success. `Empty` never answers `read-rows`.                                                                                                                                                                             |
| Routing        | Server resolves the selector within the issuing Session (unowned → `terminal-unavailable`), revalidates the pinned Worker/generation/current-worker, then dispatches OBCN tag 6. The response record's Host correlation must equal this dispatch's Worker `OperationId`; role, kind, encoding, Host epoch, and attachment must match the request scope. The record bypasses frame-only `projection.receive`. Interleaved Host events still route to their owning attachments. |
| Settlement     | Host `TypedFailure` maps to the closed `RowsRejected` reason set; a legal `RowsRejected` does not retire a healthy viewer. `RowsRejected` on a non-`ReadRows` operation, or a response `Record` in an invalid phase, is a protocol violation. No acknowledgement (`RowsApplied`) is sent for buffer reads, and no implicit retry, resend, or retargeting follows any failure.                                                                                                 |
| Lifetime       | The exchange is bounded by the existing one-second Worker/Host budget inside the caller's deadline. Viewer slot release and detach/drop stay with the existing Session cleanup; nothing here extends viewer lifetime.                                                                                                                                                                                                                                                         |

The action is additive and matched-cohort only: an older Worker rejects tag 6, and a same-cohort
violation (wrong inner correlation, response record in the wrong phase, `RowsRejected` on another
action) fails closed rather than degrading to a guessed interpretation. Worker additionally rejects
a successful response whose actual OBRW payload length exceeds this request's effective budget
(`min(requested, negotiated row-window, record carrier)`); the 64-byte OBTP header counts against
the carrier bound, while `maximumBytes` bounds only the OBRW payload.

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

## Ownership

This application owns local native Workbench authority, standalone membership, private projection, Worker and Client registries, the OBCN carrier, terminal-intent coordination, and Session viewer/controller relay. The retained local Desktop composition has its own Workbench writer. Production Browser ingress remains a future capability with no replacement service introduced by legacy retirement.

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
The `workspace_directory` children own directory admission and canonical commit in WorkbenchAuthority,
its Site grant composition, and the service's lock-free Worker observation followed by revalidation.
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
`WorkbenchReadOrNavigationRequest` excludes `tab.create`, `tab.move`, `tab.close`,
`panel.split`, `panel.close`, `panel.move` and
`workspace.primary-directory.accept`, which need their explicit Site composition paths.
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

Viewer routing tests execute all 64 generated seeds and require their declared boundary coverage.
Every trace round-trips through the replay codec; one seed also replays against a fresh IPC fixture.
Successful traces are otherwise executed once. A failure retains its original seed and trace and
shrinks against the same failure fingerprint.

The Server model command supports the shared runner’s bounded concurrency. Each retained configuration keeps its declared bounds, properties and action coverage. Implementation tests remain required when product behavior changes.

The relay core has one configuration with a 20-second deadline. It must enumerate the complete reachable graph and satisfy every declared property and action witness. The complete uncached `pnpm test:tla` gate, including tooling witnesses, has a 60-second budget.

## Local standalone terminal opening

The unshipped `terminal.open` operation opens one Server-owned standalone terminal reference for
the local single-subject native path. The first request that passes all preflights atomically commits
shared Tab/Panel/Presentation membership and terminal intent, then dispatches one immediate launch
with Worker-default cwd and the supplied geometry. Subsequent Opens join or return the same retained
outcome and Create reference; the settled slot replays over the strict leaf-level tree membership,
so a committed `panel.split` leaf or a legal reorder never changes the returned reference or retires
the shared Workbench. OBCS local terminal admission authorizes this operation; the internal
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
| Model       | Empty -> Creating -> Settled. Pure terminal planning and Workbench validation precede one Site-gated commit. Dispatch and response publication occur outside the gate. Settled replay reads the strict leaf-level tree membership and requires the slot reference's retained binding exactly once across all members.              |
| Preflight   | Missing Worker/Workbench, entropy failure, reference collision, capacity, identity, revision or lease failure leaves Empty with neither terminal nor membership. Internal Workbench preflight/publication failure retires Workbench without dispatch.                                                                              |
| Completion  | Started maps to Opened; unavailable/failed preserve Create reasons; collision/entropy/Workbench preflight refusal maps to server-busy. Unexpected fixed-input rejection maps to terminal-create-failed. Unknown stays unknown. Post-commit failure retains membership and never permits another create.                            |
| Concurrency | Wait releases the Site mutex. One absolute 46-second condition deadline returns server-busy only to that waiter, without cancellation. Worker exchange is bounded by 44 seconds; ordinary Client submission has 50 seconds, clamped to the startup deadline until first frame.                                                     |
| Lifetime    | Client loss or failed publication preserves membership. Opened requires the exact current Worker generation; loss/replacement never adopts or relaunches. Later navigation retirement cannot cancel an already committed launch.                                                                                                   |
| Limits      | In-process successful-execution atomicity under one mutex; no crash/persistence atomicity. One standalone reference; no close/reset, recovery, full Workbench operation, remote authorization or activation policy. The wait deadline does not bound mutex acquisition, suspension or response writing. Runtime evidence is Linux. |

### Standalone opening correctness

| Claim                                                  | Refinement and evidence                                                                                                                                                                     |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SO-01: at most one prepared create                     | Reserve maps to the gated Site commit; Singleton/NoReset and core preflight/collision/concurrency tests.                                                                                    |
| SO-02: callers share a committed reference             | Complete/Observe, SameReference/OpenedWasCommitted, exact-placement tests and real two-Session/Host traces.                                                                                 |
| SO-03: Client loss never resets shared state           | Disconnect/Reopen, DisconnectPreservesSlot, unread-result and reopen process traces.                                                                                                        |
| SO-04: uncertainty never creates another effect        | Complete(unknown), SettledSlotIsFinal, failed/unknown/same-Worker-new-generation core and process tests.                                                                                    |
| SO-05: a waiter settles without Worker fairness        | Timeout/EveryWaitSettles and absolute-deadline condition tests; only local timeout observation is weakly fair.                                                                              |
| SO-06: launch requires committed membership            | Reserve/Dispatch, MembershipCommitted/DispatchHasMembership, logical authorization comparison before Worker completion.                                                                     |
| SO-07: reads preserve the binding across navigation    | Navigate/ReadWorkbench, ReadIsCommitted/MembershipIsRetained, stateful navigation traces and a stalled-Worker process witness.                                                              |
| SO-08: preflight consumes neither allocator            | PreflightPreservesState covers abstract membership/revision; exact identity, revision and encoded-capacity tests cover concrete allocation.                                                 |
| SO-09: settled replay survives a legal multi-leaf tree | `DerivedComposition` anchor-once over the ordered leaf member sequence, `MovePreservesBootstrap`, deterministic Site split/reorder/close tests and the Linux PTY split-exit-reopen witness. |

Deterministic and stateful implementation tests cover exact Client/Create/binding identity, navigation commit, appended membership, identity non-reuse and ignored duplicate completion. Process/PTY traces cover framing, logical-to-Host mapping, concurrent requests, real execution and retention. These concrete traces do not establish exhaustive interleavings or arbitrary scheduling progress.

## Windows native Site

The managed Windows Site keeps the existing in-memory authority and lifecycle guard. It accepts current-token-user named-pipe peers and starts one exact sibling `orchbee-worker.exe` with handle inheritance disabled, so CLI output capture finishes independently of the retained Site. Runtime path, ACL, lock, marker, and cohort validation remain in `orchbee-local-site-runtime`. The Windows observation ingress accepts one bounded complete JSON value without Unix half-close; malformed connections do not terminate other clients. This ingress adapter does not port the Unix provider hooks. See the [Client quick start](../orchbee/README.md#native-windows-quick-start) for the usable surface and remaining limits.

## Directory Workspace launch

The native unpublished `workspace.launch` command creates a Workspace, accepts its absolute directory and creates its first terminal under one request identity. Placement is `ungrouped`, an existing `project`, a `new-project`, or `alongside` a source Workspace. Alongside uses the source's current Project at final admission; if it is ungrouped, only that source and the new Workspace enter a newly created Project. A closed or closing source is refused. Navigation, directory association and terminal creation grants are all required. Explicit argv, environment and Launch Profiles use the existing bounded launch contracts. Client focus is outside this operation.

The Server checks admission, releases its gates for a read-only Worker directory probe, then rechecks the request, revision, source membership and exact Worker generation under registry-to-Site locking. It prepares six structural identities and any required Project identity in one candidate projection, reserves the retained result and terminal admission, and commits Project placement, Workspace, Directory, Context and first Tab/Panel/Presentation together at one revision. Failed admission publishes no partial Workspace or Project and cannot regroup the source. Terminal dispatch occurs only after commit; pending or failed launch retains all committed identities for inspection and explicit Close. The receipt carries the resulting nullable Project. Exact replay never dispatches again. A lost response is outcome unknown and must not be automatically retried.

This refinement reuses the Tab creation commit and settlement owner. Verification obligations cover grant separation, stale revision and Worker generation across the probe, capacity and identity bounds, exact replay, postcommit launch failure, and a Windows cwd/environment process witness. It does not claim durable restart recovery, Desktop support, focus changes or TLA execution.

## Retained Windows Worktree continuation

`workbench.worktree.operation` admits Start, pure Read and attempt-fenced Resume. The Workbench authority retains at most 64 operations, one Pending globally, and canonical Workspace association; a lazy maintenance owner continues after Client disconnect. Registry-to-Site admission and publication recheck the source and exact Worker incarnation; OBCN IPC runs outside both gates. Create/Open observe Worker checkout facts before reusing or launching a Workspace. Remove checks dirty state before Close, reserves new Workspace/directory admissions, holds canonical publication and waits for terminal disposal plus cleanup before Git removal. A known Git failure verifies the original checkout and restores fresh default shells into the retained presentation slots; independent bounded restoration counts preserve failed and unknown launches. Unknown deletion keeps the layout fenced without launching shells. Known success releases publication; final Close retries never repeat Git. Explicit ordinary Close after a failed Worktree can release the hold without Git. Explicit Site stop joins this owner before Close maintenance and Worker carrier freeze. See the [CLI/continuation contract](../orchbee/docs/windows-worktrees.md) and [shared protocol](../../schemas/workbench/v1/workbench-worktree.schema.json) for limits and recovery.

Worktree Start has a required nullable `projectId`: only a Directory source may request an
existing Project; Workspace sources and Remove require `null`. Retained outcomes preserve that
original request metadata. When Workspace-based Create/Open needs a new Workspace after
asynchronous preparation, it uses Alongside placement against the source's current membership and refuses a closed or closing
source. Verified Git associations name stable Workspace/Directory identities independently of
Project membership, so regrouping does not discard retained checkout facts.

## Read-only Worktree source queries

`workbench.worktree.query` resolves an explicit Workspace or literal directory without requiring a Project. It captures the source and Worker incarnation under registry-to-Site gates, runs one bounded discovery job outside the gates, then rechecks before joining current Workspace labels/identities. Multiple associations stay explicit; no arbitrary first match, focus, Workspace creation or mutation receipt is involved. The shared porcelain parser accepts absolute Windows drive paths as well as its existing path forms. Source changes, Worker loss, malformed metadata and expired budgets produce typed failures. See the [owning contract](../orchbee/docs/windows-worktrees.md#source-based-discovery).

## Retained Workspace group Close

Ordinary `workspace.close` removes only the selected Workspace and its terminal context, even when other Workspaces share its Project or Git repository. It does not cascade to physical Git peers or delete Project metadata.

`workbench.workspace-group-close` explicitly closes all members of one `projectId`. Admission captures at most 256 members in current Workspace display order, across any repositories, and composes their ordinary Workspace Close receipts. Workspaces in another Project are excluded even when they share a repository; later members are not added to the retained attempt. Empty Projects are valid targets. The result retains the Project identity, attempt, status and each captured Workspace's identity, label and Close result. Project metadata remains until a separate `project.dissolve`. Group and Worktree fresh mutations reserve each other while Pending. A shared maintenance lifetime adapter owns separate wake/thread/join instances; Windows Site stop joins both submitters before Close maintenance freezes. The [Project close-all contract](../orchbee/docs/windows-worktrees.md#project-close-all) defines limits, partial failure and recovery; [the wire schema](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json) owns public bytes. Group closure does not invoke Git removal.

## Windows Client view routing

Transport admission remains surface-neutral. An admitted Windows Session may additionally publish one ephemeral interactive view under its Server-assigned generation identity. `client_view_registry` owns bounded routing, freshness and retained delivery receipts; `WorkbenchClientSession` supplies authenticated ownership and disconnect cleanup. Canonical Workbench authority and terminal carrier ownership stay separate. The [owning Client control contract](../orchbee/docs/windows-client-control.md) defines the wire, post-draw confirmation, lifecycle and evidence; deterministic Client-view lifecycle tests cover the implementation.

## Workspace metadata reports

The [Workspace metadata model](../orchbee/docs/windows-workspace-metadata.md) owns transient token values, per-source sequence fences and monotonic TTL under the existing Site gate. The separate OBWP 31/32 RPC uses read permission for queries and navigation permission for reports, without creating topology ledger entries, changing canonical revision or requiring a terminal Worker. Reports are atomic across token/source/resource limits; stale sequences do not refresh expiry. State is absent after restart, and closed Workspace identities are pruned at the next metadata operation. Generic metadata events remain a separate integration.

Panel presentation reports use OBWP 33/34 under the Site gate, with read/navigation permissions and exact primary-binding guards. The [state model](docs/workbench-authority.md#panel-presentation-metadata) owns transient per-terminal sources, field priority, tokens and expiry.

## Native plugin action admission

Workbench plugin execution requires read, terminal-creation and lifecycle grants before source resolution. Local terminal service checks Site/Worker identity, enabled manifest availability, action ambiguity and Windows platform support, freezes argv/cwd/context and submits once to Worker. It does not own subprocesses. Read pages remain bound to Worker generation; lost mutation replies preserve uncertainty. [Native plugin behavior](../orchbee/docs/windows-plugins.md) owns the full contract and outstanding lifecycle integration.

Plugin Pane Tab/split admission resolves catalog selectors after grant, revision and replay guards, then freezes an explicit program, plugin-root fallback cwd and protected display environment. Both terminal planning and completion corroborate that environment with the frozen launch, cwd, ordinary environment and Worker generation. Resolution refusal precedes canonical reservation; retained requests replay without reading a changed manifest. Batch entrypoints are refused until a literal Windows shell-wrapper contract is available.

Plugin Pane membership is prepared against the candidate canonical snapshot and committed with its terminal reservation. The bounded ephemeral map follows exact Presentation/binding identity; read-only PaneGet prunes lost bindings and returns revision-correlated facts, including when the Worker is unavailable. A retired authority refuses the read.

## Workspace Git observations

`workbench.workspace-git` is a read-granted query of a canonical Workspace's
committed directory and Worker incarnation. Site, directory, canonical revision
and Worker generation are checked before and after the discovery job; no topology
or metadata report is mutated. Status headers supply branch and local upstream
counts, with explicit absence. The [Workspace sidebar model](../orchbee/docs/machines.md#workspace-sidebar-model)
owns the complete consumer and evidence limits.

Discovery exchanges use nonblocking high-lane admission and absolute-deadline
waits. WorktreeJob and AgentDrain replies remain checked on the sole wire owner,
but an abandoned read completion does not retire a healthy carrier. Effectful
operations retain required delivery. The status reader never resubmits a timed-out
start and queues one nonblocking cancel for its known job identity; physical TTL,
reaping and retained receipt remain Worker-owned if cleanup cannot be admitted.
The read budget does not preempt mutex acquisition or process suspension, and
cancel admission is not confirmed physical cleanup.

### Discovery admission and receipt ownership

`local_terminal_service::worktree_list::DiscoveryGate` coordinates all three
read-only starts: committed Directory lists (`Start`), Workspace/literal-directory
queries (`StartSource`) and sidebar Git observations (`StartStatus`). The Worker
still owns one physical discovery job and one retained receipt. A new accepted
start replaces that receipt, so Server admission covers the entire job lifetime,
including intervals between polls and terminal-result consumption.

The gate retains one active reader and at most one waiting foreground reader.
This represents a physical owner and its single successor, without increasing
Worker capacity or creating a background queue. Sidebar observations are
opportunistic and return their existing typed `Unavailable` when admission is
occupied. A registered foreground list/query owns the next turn; further
foreground contenders receive `ServerBusy` without dispatch.

| State/event                                              | Transition and ownership                                                                                         |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Idle; valid status/list/query                            | Acquire the sole RAII permit before any discovery Start.                                                         |
| Active; first foreground contender                       | Register one successor and wait on the condition variable under the original absolute deadline.                  |
| Active or foreground waiting; status contender           | Refuse immediately; no Worker request or retained status waiter.                                                 |
| Foreground already waiting; another foreground contender | Refuse without replacing the registered successor.                                                               |
| Waiting reader's deadline                                | Remove only that waiter; leave the active job and its permit intact.                                             |
| Active reader consumes a terminal result                 | Release its permit; the registered foreground successor has priority over new status reads.                      |
| Active reader abandons an admitted job                   | Queue one exact-job Cancel without waiting, then release the permit; Worker TTL owns cleanup if admission fails. |

No Site, registry or gate mutex is held across IPC, polling delays or admission
waits. Source, Site and Worker generation fences still surround observation;
organization or terminal resources are never changed by discovery. The gate is
ephemeral and does not survive a Server restart.

All readers keep their original five-second deadline, including admission and
reply waits. The sole wire owner consumes and validates late replies even after
the reader returns. An admitted Start keeps its exact operation/job identity for
Cancel even when acceptance arrives late. Foreground readers may retry only a
definite `Busy` Start refusal while an abandoned physical job settles, with fresh
operation IDs, at most 50 ms between attempts and no deadline renewal. Starts and
polls share a 101-request bound within the reserved 128-identity block, with room
for cancellation. Accepted starts, unknown outcomes and Git mutations are never
replayed. Physical cleanup and progress still depend on the existing Worker TTL,
carrier and process scheduling; this policy does not promise successful reads
after a Worker loss or a deadline exhausted by an earlier job.

`worktree_list::tests::discovery_*` checks bounded admission, foreground priority
and waiter expiry. The Windows real-LocalStream `worktree_query::tests` traces
cover six sidebar readers competing with both source forms between polls,
committed Directory list/query sharing, active-status handoff, admission timeout,
late Start/Poll replies, exact Cancel and definite-Busy recovery. These controlled
implementation traces cover this gate; the existing coordinator/relay/viewer TLA
models do not model discovery receipt ownership or claim this fix. Live terminal
load and deployed-cohort behavior require separate observation.

## Windows scrollback editor popup model

The private matched-cohort `workbench.terminal-editor` request/result (OBWP kinds51/52,
version1) admits Open, Read and Close for a connection-owned independent terminal.
Open carries an exact canonical context/Tab/Panel/Presentation/binding source, a
read-only viewer owned by that same connection, decimal full-u64 content generation
and revision, and drawable geometry. It carries no executable, environment,
document path or terminal transcript. Current-Client focus admission belongs to
the Client view; Server repeats canonical membership, connection viewer/frame,
Worker incarnation and physical target checks before creation and after read-only
Worker editor resolution. Read capability never grants controller or resize rights.

The independent popup owner is shared with plugin popups without installed-plugin
resolution or canonical Pane membership for editors. Admission reserves ownership
before handoff. Host deferred Create captures immutable text before Start writes
its exclusive document and spawns the editor. Confirmed creation precedes Start;
unknown creation/start is retained and never resubmitted. The states are Opening,
Active, Queued, Waiting, Dispose, Unknown and Closed. A known deferred target
abandoned before Start may enter Dispose directly; inventory AwaitingStart is
also disposable without claiming child exit. EOF transfers accepted cleanup to
existing Site maintenance. Source read attachments remain owned until their last
capture consumer, then detach; popup process/document cleanup remains Host-owned.

The serial application dispatcher shares its connection viewer session between
OBCS and editor OBWP requests through a local RefCell, with one borrow per request
and no reentrant callback or cross-thread access. Grant/site/source/editor refusal
precedes Create. Focused protocol and native Windows pipe traces judge source
fences, deferred ordering, literal argv, unknown results, existing plugin behavior,
canonical preservation and physical cleanup. They do not establish deployed GUI,
TUI/emulator or full-cohort acceptance. Client overlay and public CLI ingress must
be verified separately before reporting the complete editor capability.

Deferred Start admission renews the terminal authorization/fence. The popup owner
is locked before that admission and updated with the exact prepared Start
authorization before dispatch, so failure/unknown receipts and later lifecycle
cleanup retain the current owner rather than the obsolete Create authority.

## Windows Panel image ownership

The accepted model and current implementation boundary are maintained in [Windows Panel graphics](docs/windows-graphics.md). Canonical commits retire invalid layers immediately; authenticated connections own producers, and the shared temporary-viewer observer retains uncertain PNG-read cleanup. UI pixels, capability observations, CLI producers and final composed acceptance remain separately required.

Canonical split ratios are shared by native Clients and retained in checkpoint v6, guarded by catalog v5. The [checkpoint contract](docs/workbench-checkpoint.md) owns strict migration from versions 1–5; the [shared ratio contract](docs/shared-split-ratios.md) owns ratio mutation, projection and compatibility.
