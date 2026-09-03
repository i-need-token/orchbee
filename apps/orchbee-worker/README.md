# Rust Orchbee Worker

Experimental, unshipped private Rust Worker application for the first POSIX local terminal
vertical. The emitted executable is `orchbee-worker`. It participates in the root Cargo gate but
is not packaged, installed, activated by `orchbee`, or consumed by a current product path.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Process boundary

On macOS and Linux, the exact private development invocation is:

```text
orchbee-worker \
  --local-endpoint <absolute-server-socket> \
  --terminal-host-runtime-directory <absolute-directory> \
  --terminal-host-endpoint-id <32-lowercase-hex>
```

Worker first connects to the supplied Server Unix-domain socket, requires the same effective
operating-system user, submits its bounded cohort handshake, and verifies typed admission. It does
not contact Terminal Host before Server accepts that Worker connection.

After Server admission, Worker resolves only `<runtime-directory>/<endpoint-id>.s`, connects to the
existing Terminal Host, independently requires the same effective user, and performs the existing
exact-version Terminal Protocol `Hello`/`HelloAccepted` exchange. The request identifies one fixed
diagnostic Worker broker client, advertises the protocol hard limits and no optional feature, and
is accepted only through the registered request/response roles, exact correlation, exact protocol
version, matching negotiated limits, an empty feature intersection, and a nonzero Host epoch.

Only after both peers accept does Worker write `ready\n` to stdout. Its main thread then polls the
two owned sockets without a connection thread or queue. Server loss closes the Host broker socket;
Host loss closes the Server attachment. Either outcome is silent and exits status `1`, allowing
Server to observe Worker loss and allowing a Host with no retained terminal to apply its own idle
shutdown rule. Invalid invocation, connection, authentication, rejection, protocol, output, or
transport failure has the same silent status-`1` result.

This slice sends no post-handshake Server message and no Terminal Host message other than `Hello`.
It performs no retry, discovery, activation, daemonization, child-process supervision, terminal
inventory, launch, attachment, input, resize, filesystem inspection, shell or working-directory
resolution, PTY operation, Agent behavior, or durable write. It uses no current-directory,
Project, Environment, Checkout, Worktree, launch-profile, Files, or Git fact. Endpoint selection,
service supervision, Server-issued launch authorization, Host activation, and reconciliation
belong to later slices.

## Local Host broker modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | The local Server can admit one Worker and the Rust Terminal Protocol binding can encode Host connection messages, but no real Worker process owns the normal node-local Host broker connection. This slice must establish that connection without inventing terminal authority or bypassing Server launch authorization.                                                                                                                 |
| Inputs       | An untrusted private argument vector; one absolute Server socket; one absolute Host runtime directory; one canonical 32-lowercase-hex Host endpoint ID; kernel peer credentials for both sockets; one bounded local-Site handshake; and one bounded Terminal Protocol handshake. Missing, malformed, incompatible, wrong-role, rejected, timed-out, and disconnected inputs are explicit failures.                                       |
| Data         | One Worker-owned Server socket plus its accepted nonzero Worker generation and, only after Server admission, one Worker-owned Host socket plus the accepted nonzero Host epoch and negotiated protocol limits. These process-local values are released together and never become Server, PTY, VT, attachment, or terminal state.                                                                                                         |
| Outputs      | Successful dual admission emits exactly one `ready\n` marker. Every other result is a silent status-`1` process outcome. Host and Server each observe connection close when the other side fails. No terminal request, semantic event, or durable mutation is produced.                                                                                                                                                                  |
| Invariants   | Host contact requires prior Server admission; only the existing Host `Hello` request crosses the Host boundary; readiness requires both accepted connections; one Worker owns at most one socket to each peer; stopping releases both; neither peer loss can leave the other connection or a connection thread behind; terminal-command count remains zero.                                                                              |
| Decisions    | Invocation validity, Server admission, Host endpoint syntax, peer-user equality, envelope length, registered message role, correlation identity, exact protocol version, negotiated limits, optional-feature intersection, Host response kind, timeout, and peer liveness completely determine the result.                                                                                                                               |
| Model        | A sequential admission workflow followed by one two-descriptor liveness poll. [`LocalHostBroker`](formal/tla/LocalHostBroker.tla) models independently ordered Server rejection/loss, Host peer rejection, wrong role, incompatibility, malformed response, timeout, Host loss, accepted readiness, and coupled link cleanup.                                                                                                            |
| Transitions  | `absent -> connecting-server -> connecting-host -> host-handshake -> host-admitted -> ready`; any rejection or loss moves directly to `stopped` with both links closed. No transition creates, attaches, mutates, or disposes a terminal.                                                                                                                                                                                                |
| Dependencies | Rust standard-library argument, path, Unix-socket, bounded I/O, and process APIs; safe `nix` polling; the independent local peer-credential adapter; the private local-Site protocol; and the independent Rust binding of the existing Terminal Protocol. There is no async runtime, network listener, child process, persistent storage, shell, PTY, VT, filesystem-content, Git, or Agent capability.                                  |
| Verification | Registered TLA+ safety and progress checking; real Worker-process tests against controlled fake Server and Host peers for ordering, malformed/rejected handshakes, exact readiness, Host-loss and Server-loss cleanup; and a native real Worker-to-Zig-Host process witness that performs the production handshake and observes Host idle shutdown after Worker disconnect.                                                              |
| Limits       | Runtime evidence covers macOS and Linux under one effective user. Same-user process separation is not hostile-tenant isolation. The witness establishes connection, handshake, liveness, and cleanup only; it does not establish Host activation, terminal launch, retained-terminal continuity, data relay, reconnect, Windows named pipes, remote enrollment, production service lifecycle, packaging, latency, or throughput budgets. |

The checked TLA+ model has no terminal command transition. Its `serverLink` and `hostLink` map to
the two owned Unix streams, `hostRequest = "hello"` maps to the only Terminal Protocol record sent,
and `readinessEmitted` maps to the stdout marker. Process tests own implementation refinement and
native boundary evidence; the model proves only its finite state space and declared fairness.
Fairness applies only while admission, Host handshake, or readiness publication is pending. Once
ready, peer loss remains an ordinary nondeterministic external action, so an intact Worker may poll
indefinitely.

## Ownership

The [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.md) crate owns the
Server-Worker framing. The
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) crate owns the
kernel same-user proof used for both local connections. The
[`orchbee-terminal-protocol`](../../packages/terminal-protocol/references/protocol.md) package owns
the Host wire; its independent Rust binding supplies the consumed envelope and connection codecs.
Worker owns admission order, the Host broker connection, and coupled liveness. Server remains the
only placement and launch-authorization owner. Terminal Host remains the sole PTY, process-tree,
VT, history, and attachment authority.

The [`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model continues
to own Server admission and Worker-generation behavior. `LocalHostBroker` starts only after that
admission boundary and adds no second Server or Host model.

## Verification

Run from the repository root. The real Host process witness consumes the normal Terminal Host build
artifact, so build it before a focused Cargo test; the root merge gate already builds before tests.

```sh
pnpm --filter @orchbee/terminal-host build
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-site-process-spine,local-host-broker
pnpm verify
```
