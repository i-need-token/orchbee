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
experimental local Worker and Client Session registries. It is not a second current product writer:
the admitted Client transport carries no product message, no Client or current runtime can submit a
projection candidate, and no publication leaves the application. The current
Desktop/Workbench composition remains the delivered Workbench writer, and the Node
[`apps/server`](../server/README.md) remains authoritative for its existing Browser ingress
behavior.

Semantic business transitions, post-handshake product Client transport, product authorization,
persistence, recovery, external publication, endpoint discovery and activation, production
service lifecycle, Terminal Host authority/runtime integration, packaging, migration, and cutover
require later implementation slices.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-site-process-spine,local-terminal-client-authority
pnpm verify
```
