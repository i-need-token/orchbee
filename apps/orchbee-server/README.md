# Rust Orchbee Server

Experimental, unshipped private Rust Server application. It owns the in-memory Workbench
projection kernel and the first POSIX local Server-Worker process spine. The emitted executable is
named `orchbee-server`; it participates in the root Cargo verification gate but is not consumed by
a Client, Desktop, packaging, an installer, or a service manager.

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

On macOS and Linux, the exact private development invocation
`orchbee-server --local-endpoint <absolute-path>` binds one Unix-domain socket without deleting an
existing path, restricts the socket to mode `0600`, writes the machine readiness marker `ready\n`
to stdout, and serves bounded Worker handshakes. Any other invocation remains silent and exits with
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

The private [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.md) crate owns
framing, the Worker handshake, cohort rejection, bounds, and mutual POSIX peer proof. Server owns
listener lifetime, Worker admission state, generations, and cleanup. The
[`orchbee-worker`](../orchbee-worker/README.md) application owns the connecting process lifecycle.

## Process-spine modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | The verified native artifacts had no real process relationship. This slice must prove that a separate Server can admit and retain one separate same-user compatible Worker without creating a terminal, Client, activation policy, or durable state.                                                                                       |
| Inputs       | An untrusted private argument vector, an absolute endpoint path supplied by the development owner, kernel peer credentials, and one bounded framed Worker handshake. Absence, malformed bytes, timeout, wrong message, wrong user, version mismatch, cohort mismatch, duplicate Worker, and disconnect are explicit.                       |
| Data         | Server phase, optional active Worker identity plus nonzero process-local generation, next generation, and one candidate connection. Server is the sole writer. No value survives process exit.                                                                                                                                             |
| Outputs      | Successful bind emits one `ready\n` marker. Worker admission returns its generation; an authenticated protocol-level rejection returns a typed bounded reason when the stream permits it. Peer-authentication and transport-setup failures close silently. Process setup or listener failure exits status `1` without a human diagnostic.  |
| Invariants   | One endpoint has at most one listener; one Server has at most one active Worker; active generations are nonzero and strictly increase; rejection cannot replace active authority; generation-scoped cleanup cannot remove a replacement; Worker disconnect does not stop Server; Server loss closes the Worker connection.                 |
| Decisions    | Invocation validity, peer-user equality, frame validity, protocol and cohort equality, expected Worker message kind, active-Worker absence, and matching cleanup generation completely determine the result. No current directory, Project, Environment, Worktree, Agent, or Terminal fact participates.                                   |
| Model        | A deterministic Worker registry owns admission and generation-scoped disconnect. A thin synchronous Unix-socket workflow authenticates, decodes, admits or rejects, acknowledges, and monitors the accepted connection.                                                                                                                    |
| Transitions  | `absent -> listening`; candidate `none -> pending -> admitted/rejected/timed-out`; Worker `none -> active(generation) -> none`; Server `listening -> stopped`. A second bind is rejected, and stale cleanup is an idempotent no-op against another generation.                                                                             |
| Dependencies | Rust standard-library process, filesystem, Unix socket, timeouts, threads, and I/O plus the safe `nix` peer-credential adapter and the private local-site protocol crate. No terminal, child process, network socket, async runtime, persistence, environment lookup, or randomness.                                                       |
| Verification | Pure codec and registry tests; native same-user socket-pair tests on macOS and Linux; real Server and Worker process tests for bind, readiness, admission, duplicate and incompatible rejection, replacement after disconnect, Server loss, malformed peers, exact output, and cleanup bounds; registered TLA+ safety and progress checks. |
| Limits       | Runtime evidence covers macOS and Linux under the same effective user. It does not establish Windows behavior, cross-user negative execution, production endpoint discovery, crash cleanup, service activation, Client sessions, terminal retention, or remote security.                                                                   |

The checked [`LocalSiteProcessSpine`](formal/tla/LocalSiteProcessSpine.tla) model explores two
Worker identities and three generations. Its abstraction maps candidate actions to handshake
admission, `activeWorker` and `activeGeneration` to the registry, stale cleanup to
generation-scoped disconnect, and Server stop to transport loss. It proves only the bounded model;
deterministic and process tests own implementation refinement.

## Ownership

This application owns projection mechanics for its injected in-memory snapshot copy and the
experimental local Worker registry. It is not a second current product writer: no Client or current
runtime can submit a projection candidate, and no publication leaves the application. The current
Desktop/Workbench composition remains the delivered Workbench writer, and the Node
[`apps/server`](../server/README.md) remains authoritative for its existing Browser ingress
behavior.

Semantic business transitions, Client transport, authorization, persistence, recovery, external
publication, endpoint discovery and activation, production service lifecycle, Terminal Host
integration, packaging, migration, and cutover require later accepted slices.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-site-process-spine
pnpm verify
```
