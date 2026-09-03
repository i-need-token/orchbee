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
without allocating more threads.

An authenticated candidate receives a typed rejection for protocol/cohort incompatibility,
malformed or wrong messages, timeout, or active-Session capacity. Any byte after admission,
including a repeated hello, closes only that Session. Disconnect cleanup matches the exact
generation and cannot remove another or replacement Session. It does not stop Server, disturb the
independent Worker registry, or imply any terminal action. Server process loss closes every Client
liveness attachment.

The private
[`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.md) crate owns Client
framing and the real Rust connecting adapter. Server owns listener, handler-slot, admission,
generation, and cleanup state. Public `orchbee` does not yet call the adapter, so bare, help, and
version behavior remains unchanged. This slice adds no Workbench command, terminal relay, TUI,
activation, endpoint discovery, persistence, or remote ingress.

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

## Local Client Session modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | The accepted peer Client model had no native Server connection boundary. This slice must prove several independent same-user local Client Sessions without changing public `orchbee`, adding terminal messages, or coupling Client disconnect to Server, Worker, another Client, or terminal lifetime.                                                                   |
| Inputs       | An optional absolute Client endpoint supplied by the development owner, kernel peer credentials, and bounded framed handshakes. Missing endpoint means no Client listener. Slow, malformed, wrong-message, wrong-user, incompatible protocol/cohort, capacity, repeated post-admission input, disconnect, stale cleanup, and Server loss are explicit.                   |
| Data         | Up to eight active nonzero process-local Client Session generations, one next generation, at most sixteen candidate/session handler slots, and each accepted liveness stream. Server is the sole writer. No value survives process exit.                                                                                                                                 |
| Outputs      | Admission returns one generation; authenticated protocol-level failures return a typed bounded rejection when the stream and handler bound permit it. Credential failure and handler saturation close silently. Client disconnect has no product mutation output.                                                                                                        |
| Invariants   | Every accepted connection has one unique generation; active Sessions never exceed eight; handler threads never exceed sixteen; cleanup removes only its matching generation; rejection changes no admitted Session; one Client loss preserves Server, Worker, and every other Session; Server stop releases every Session.                                               |
| Decisions    | Listener selection, handler-slot availability, peer-user equality, frame validity, protocol/cohort equality, expected Client message kind, active-Session capacity, next-generation availability, and matching cleanup generation completely determine the result. Renderer kind and local view state never participate.                                                 |
| Model        | A deterministic Client Session registry owns bounded admission and generation-scoped disconnect. A bounded synchronous Unix-socket handler authenticates, decodes, admits or rejects, acknowledges, and retains exactly one liveness attachment. A fixed listener thread admits work only through a sixteen-slot guard.                                                  |
| Transitions  | Server `absent -> listening -> stopped`; candidate `none -> slow/rejected/admitted`; Session `none -> active(generation) -> none`; Worker marker independently `absent -> registered`. Duplicate post-admission hello is scoped disconnect. A stale cleanup is a no-op for every current Session because generations are never reused.                                   |
| Dependencies | Rust standard-library process, filesystem, Unix socket, timeout, thread, lock, and I/O; private local Client framing/adapter; and the safe kernel peer-credential crate. No async runtime, network listener, terminal, Workbench command, child process, discovery, activation, persistence, environment lookup, or randomness.                                          |
| Verification | Pure codec and registry cases; native same-user socket-pair proof; real Server process tests using the concrete Rust Client adapter for concurrent admission, capacity, disconnect/replacement, Server loss, Worker preservation, slow-handshake isolation, duplicate input, malformed/version/cohort rejection, exact readiness, permissions, and cleanup; TLA+ checks. |
| Limits       | Runtime evidence covers macOS and Linux under the same effective user. It does not establish cross-user execution, Windows behavior, public Client integration, endpoint discovery, activation, terminal retention, renderer behavior, remote authentication, hostile isolation, graceful Server shutdown, or production service lifecycle.                              |

The checked [`LocalClientSessions`](formal/tla/LocalClientSessions.tla) model explores three
abstract Clients, at most two simultaneous Sessions, and four generations. It maps `sessions` to
the registry, a slow candidate to a bounded handshake thread, `workerRegistered` to the independent
Worker registry's presence, and stale cleanup to generation-scoped disconnect. It checks safety,
not handshake-time progress or implementation refinement; deterministic and process tests own
those mappings.

## Ownership

This application owns projection mechanics for its injected in-memory snapshot copy plus the
experimental local Worker and Client Session registries. It is not a second current product writer:
the admitted Client transport carries no product message, no Client or current runtime can submit a
projection candidate, and no publication leaves the application. The current
Desktop/Workbench composition remains the delivered Workbench writer, and the Node
[`apps/server`](../server/README.md) remains authoritative for its existing Browser ingress
behavior.

Semantic business transitions, post-handshake Client transport, product authorization,
persistence, recovery, external publication, endpoint discovery and activation, production
service lifecycle, Terminal Host integration, packaging, migration, and cutover require later
accepted slices.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-site-process-spine
pnpm verify
```
