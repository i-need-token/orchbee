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
inventory, launch, attachment, input, resize, runtime filesystem inspection, shell or
working-directory resolution, PTY operation, Agent behavior, or durable write. It uses no
current-directory, Project, Environment, Checkout, Worktree, launch-profile, Files, or Git fact.
Endpoint selection, service supervision, carrier delivery and runtime consumption of Server-issued
launch authorization, Host activation, and reconciliation belong to later slices.

## Staged default interactive launch resolution

The application-private `default_interactive_launch` module is compiled into Worker but is not
called by the current process path. Its transport-neutral `DefaultInteractiveLaunch` output owns
only one resolved executable, an exactly empty literal argument vector, and three fixed environment
entries. A later authorized Host adapter may combine those facts with separately resolved cwd,
Terminal Protocol terminal profile, configuration, geometry, and creation identity. The three
entries map to the Terminal Protocol launch environment; the invocation-scoped
`CreateTerminal.environmentOverlay` remains empty in the accepted first milestone.

Resolution considers an inherited `SHELL`, then the effective operating-system account's shell,
then `/bin/sh`. A candidate is valid only when its path is nonempty, absolute, valid UTF-8, free of
U+0000, no longer than 4,096 bytes, resolves to a regular file, and is executable by the process's
effective user at resolution time. An invalid or unusable candidate falls through without exposing
its value. If even `/bin/sh` is unusable, resolution returns the typed `NoUsableShell` failure.

The native adapter reads only `SHELL`, the effective-user account record, and executable metadata
and access state. It neither canonicalizes a selected path nor starts a process. The result always
sets exactly `TERM=xterm-256color`, `COLORTERM=truecolor`, and `TERM_PROGRAM=ghostty`; it never
copies caller-transient or product variables, reads a cwd or launch profile, emits diagnostics,
opens a Server/OBCN/Host transport, or constructs a Host command. Pure table tests own candidate
admission, all selection combinations, output facts, and typed failure; native tests cover the
executable probe and the current-machine resolution boundary. A file can change after this
observation, so the eventual Host execution boundary still owns typed launch failure.

## Worker-local authorized Host-operation kernel

The application-private `terminal_authorization` module is compiled into Worker but is deliberately
not constructed or called by the current process path. It is a carrier-neutral deterministic
kernel for the private Host launch adapter and consumes the semantic
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) Launch and
Start values. Its presence does not add a post-handshake Server message or a Terminal Host command
to the runtime described above.

A future composition root must construct this intrinsically version 2 kernel with only the current
local-Site `WorkerId` and the nonzero Worker generation accepted by OBLS.
Its carrier must authenticate the active authoritative Server and supply only well-formed issuance:
within that Server authority, every `OperationId` is globally fresh across Terminals, Workers, and
Worker generations. The kernel deliberately keeps no unbounded cross-terminal collision ledger and
does not claim to contain a Server that violates that producer invariant.
A successor Server authority may begin a new producer scope only after its generation, lease, and
fence transition makes every earlier authorization non-current.

One kernel represents one accepted Worker generation and retains at most 1,024 distinct terminal
bindings for that generation. It has no release, TTL, LRU, or eviction transition. Capacity rejects
only a previously unseen `TerminalId`; exact cached replay and a valid higher-fence replacement of
an existing terminal remain eligible when the map is full. This behavior is terminal-launch
capability version 2; a version 1 or otherwise mismatched request fails closed as
`UnsupportedCapability` before capacity is considered. The current kernel is intrinsically a
version 2 implementation; its constructor cannot be configured to admit a legacy or future
capability while retaining version 2 capacity semantics.

Every admission or pre-handoff decision also receives an explicit Unix-millisecond time, and Start
admission receives the currently observed Host epoch. A returned dispatch is permission to prepare
the matching operation, not evidence that a Host effect occurred. The adapter must call
`begin_host_handoff` with a fresh observed Host epoch immediately before the first possible Host
effect, then report one typed Host completion, definite pre-handoff failure, or post-handoff unknown
outcome through the same private dispatch token. The kernel retains the maximum time it has
observed, so a caller-visible wall-clock rollback cannot revive any expired lease.

### Authorization-kernel modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A future OBCN carrier may deliver a Server-authorized Launch or Start more than once, late, for the wrong Worker generation, or after authority has advanced. Worker must decide locally whether one Host operation may begin and must preserve the exact terminal result without acquiring Server placement authority or performing the Host effect itself.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Inputs       | Canonical OBCN `AuthorizedTerminalLaunch` or `AuthorizedTerminalStart` from an authenticated authoritative Server that obeys the globally fresh `OperationId` producer invariant, with its authorization capability checked against the kernel's intrinsic version 2 policy; the caller-supplied current local-Site Worker ID, nonzero OBLS generation, and current Unix-millisecond time; the observed Host epoch at Start admission and again at the handoff boundary; and a typed Host completion. A missing result or occupied slot remains explicit state rather than inferred success.                                                                                                                                                                                                                                                                                                                                                                                                              |
| Data         | `AuthorizedHostOperationKernel` owns at most 1,024 complete highest-fence bindings per Worker generation, keyed by product `TerminalId` and including operation kind, full authorization, Launch or Start payload, and an optional exact cached result. It separately owns one process-local slot in `empty`, `awaiting-handoff`, `handed-off`, or `result-ready` state and a maximum-observed Unix-millisecond high-water mark. A private `AuthorizedHostDispatch` retains the complete admitted operation and therefore its generation and authority binding.                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Outputs      | Admission returns `Dispatch`, `ResultReady`, or `SlotOccupied`; handoff returns `HandedOff` or `ResultReady`; `take_result` returns one typed OBCN Launch or Start result, including `CapacityExceeded` for a valid unseen terminal after the retained-binding limit is reached; invalid completion observations return a typed internal error without changing the slot. The kernel opens no carrier, performs no Host call, emits no product event, and writes no durable state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Invariants   | Wrong Worker, stale generation, unsupported capability, expired new lease, lower fence, conflicting same-fence binding, and a valid unseen terminal beyond capacity never dispatch. An exact binding dispatches at most once and replays only its cached exact result, even after expiry or at capacity. Once the kernel observes a lease as expired, a lower later time cannot revive it. A valid higher fence replaces the prior binding and clears its cache even at capacity. Capacity rejection never inserts a tombstone or changes any binding. At most one operation or undelivered result occupies the global slot. Only `handed-off` can become a Host outcome.                                                                                                                                                                                                                                                                                                                                 |
| Decisions    | Classification order is occupied slot, Worker identity, Worker generation, exact cached replay, capability, lease expiry, per-terminal fence, unseen-terminal capacity, then Start target epoch. Consequently unsupported or expired new requests retain their capability or lease classification at capacity; existing lower and same-fence requests retain their fence classifications; and only a valid unseen terminal receives `CapacityExceeded`. At Start admission and again at handoff, a target epoch mismatch becomes `HostIdentityChanged`; capacity precedes that check for an unseen terminal. Handoff expiry prevents an effect, and accepted Start identity, epoch, and geometry must remain exact.                                                                                                                                                                                                                                                                                       |
| Model        | One deterministic state owner combines a generation-local, 1,024-entry per-terminal monotonic binding map with one globally bounded operation/result slot. `admit_launch` and `admit_start` are the principal entry points; `begin_host_handoff` is the named external-effect boundary; typed observation methods close the accepted binding and queue its result. [`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla) composes the cross-terminal capacity refinement at a finite threshold plus one unseen identity with a minimal empty, operation-pending, and result-ready slot abstraction.                                                                                                                                                                                                                                                                                                                                                                             |
| Transitions  | The transition table below defines admission, handoff, completion, exact replay, result delivery, conflict, and stale-token behavior. Rejection before a binding is accepted queues an uncached result; every terminal outcome after a binding is accepted is cached before it is exposed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Dependencies | The Rust standard library, canonical local-Site Worker identity, and OBCN semantic types and constructors only. Time, socket liveness, decoded framing, current Host epoch, and Host observations are supplied as values by a future adapter; the kernel monotonizes supplied time but never reads a clock. It does not read an environment, filesystem, socket, process, profile, executable, shell, or working directory, and it does not parse or encode Host framing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Ownership    | Server continues to own placement, authorization issuance, and reconciliation. OBCN owns the carrier-independent request/result types and wire codec. Worker owns only local validation, monotonic replay memory, the single dispatch gate, and classification around the Host handoff boundary. Terminal Host remains the sole owner of terminal and process effects.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Verification | Table-driven unit tests exercise both operation kinds across wrong identity, generation, legacy and future capability mismatch, expiry and clock rollback, overlapping rejection priorities, the 1,023/1,024/1,025 capacity boundary, no-tombstone rejection, exact replay and higher-fence replacement at capacity, lower and conflicting fences, exact replay of every accepted/rejected/failed/unknown outcome, expiry after admission, stale and wrong completions, slot backpressure, initial and raced Start identity changes, mismatched Start completion geometry, both Launch modes, and both Start success transitions. OBCN tests own value/codec validity; the registered TLA+ models own their bounded abstractions.                                                                                                                                                                                                                                                                         |
| Correctness  | The Server-owned [`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) variables `acceptedBinding`, `requestPhase`, cached outcome/epoch/transition, and pending result map respectively to one kernel binding, slot phases, cached typed result, and result-ready slot. Its `FreshOperation` guard is the single-terminal, single-Worker projection of the Server producer premise. Worker-owned [`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla) maps its bounded fence/cache functions, retained set, and abstract global slot to the Rust binding map and operation/result slot. It checks capacity, occupied-slot priority, at most one uncached operation/result, delivery clearing, rejection preservation, and full-capacity exact replay and higher-fence replacement only after the slot is empty. Unit tests are implementation-refinement evidence; TLC remains bounded model evidence, not a proof of this Rust code. |
| Limits       | No current carrier or Host adapter invokes this kernel, so tests establish deterministic state behavior only. Bindings and cached results are process-local, are not crash-persistent, and remain until the kernel is dropped or a higher fence replaces them. The 1,024 limit has no release, TTL, LRU, or eviction path; an explicit retirement contract remains required before production. The kernel cannot detect a Server's cross-terminal `OperationId` reuse without an unbounded ledger.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

| Current slot       | Input or observation                               | Guard                                                                          | Next slot          | Binding/result effect                                                                                      |
| ------------------ | -------------------------------------------------- | ------------------------------------------------------------------------------ | ------------------ | ---------------------------------------------------------------------------------------------------------- |
| `empty`            | New or higher-fence Launch/Start                   | Worker, generation, capability, lease, and fence are current                   | `awaiting-handoff` | Store the complete binding with an empty cache and return its private dispatch.                            |
| `empty`            | Invalid new, stale, or conflicting request         | A named rejection applies                                                      | `result-ready`     | Queue the exact-echo typed rejection without accepting or replacing a binding.                             |
| `empty`            | Valid unseen Launch/Start at 1,024 bindings        | Worker, generation, capability, and lease are current                          | `result-ready`     | Queue exact-echo `CapacityExceeded`; do not insert a tombstone or change any binding.                      |
| `empty`            | Exact accepted binding                             | Its exact terminal result is cached                                            | `result-ready`     | Queue the cached result semantically unchanged; do not dispatch or re-evaluate expiry.                     |
| `awaiting-handoff` | `begin_host_handoff`                               | Lease remains live and any Start target epoch equals the observed one          | `handed-off`       | Preserve the complete dispatch and observed Host epoch; only now may the adapter perform its first effect. |
| `awaiting-handoff` | Expiry, Start epoch replacement, or definite loss  | No Host effect has begun                                                       | `result-ready`     | Cache and queue lease/identity rejection or `HostUnavailableBeforeHandoff`.                                |
| `handed-off`       | Accepted, rejected, failed, or unknown Host result | Dispatch and Host observation are consistent; accepted Start geometry is exact | `result-ready`     | Cache and queue the exact typed result; Launch state derives from its admitted mode.                       |
| `result-ready`     | `take_result`                                      | One result is present                                                          | `empty`            | Deliver the queued result once while retaining any accepted binding cache for replay.                      |
| Any occupied slot  | Another admission                                  | An operation or undelivered result already owns the slot                       | Unchanged          | Return `SlotOccupied`; do not compare or mutate terminal authority.                                        |
| Any phase          | Wrong, stale, or out-of-phase dispatch observation | Dispatch or phase does not match                                               | Unchanged          | Return a typed internal error and preserve the current operation/result.                                   |

| Decision partition                       | Launch evidence                                                                     | Start evidence                                                             | Required result                                                                                       |
| ---------------------------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Worker, generation, capability, or lease | Table case for every rejection                                                      | Same table cases                                                           | Exact request echo, no dispatch.                                                                      |
| Overlapping invalid conditions           | Lower plus unsupported, lower plus expired, same-fence changed capability or expiry | Shared classification path                                                 | Capability precedes expiry, expiry precedes fence, and only an exact cached binding bypasses expiry.  |
| Per-terminal fence                       | Lower, same-operation, lease, and payload conflicts; higher replacement             | Lower, cross-kind, target, and geometry conflicts; higher replacement      | Stale or conflicting rejection; valid higher fence clears the old cache.                              |
| Retained-binding capacity                | 1,023 admits, 1,024 is full, 1,025th identity rejects; replay/rebind while full     | Same boundary plus identity-mismatched unseen Start                        | Only an unseen identity receives `CapacityExceeded`; no tombstone, binding mutation, or eviction.     |
| Exact replay after expiry                | Accepted in both launch modes, rejected, failed, and unknown                        | `Started`, `AlreadyStarted`, rejected, failed, and unknown                 | The original complete cached result; no new Host effect.                                              |
| Handoff boundary                         | Expiry, definite failure, and Host epoch-bound completion                           | Admission mismatch, expiry, target epoch replacement, and definite failure | No accepted Host observation before handoff; post-handoff loss is unknown.                            |
| Completion identity and ordering         | Wrong token, stale token, wrong phase, and wrong epoch                              | Wrong operation kind, target identity, epoch, or accepted geometry         | Typed error with unchanged state; a later valid completion or unknown observation can still close it. |
| Global result slot                       | Awaiting, handed-off, and result-ready backpressure                                 | Shared slot behavior                                                       | Exactly one outstanding operation or undelivered result across both kinds.                            |
| Time rollback                            | Expired unaccepted request and admission-to-handoff expiry                          | Expired unaccepted request                                                 | Maximum-observed time prevents an expired lease from becoming dispatchable again.                     |

## Worker-local Terminal Host launch adapter

The application-private `terminal_host_launch_adapter` module is also compiled but not called by
the current process path. It is a pure value mapper between one admitted `AuthorizedHostDispatch`,
existing Terminal Protocol request/response values, and the Host observations already accepted by
`terminal_authorization`; it neither opens a Host socket nor changes kernel state.

### Launch-adapter modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | An admitted OBCN Launch or Start must cross the existing Terminal Protocol without giving the carrier authority to invent a process, directory, environment, identity, geometry, or outcome.                                                                                                                                                                                                                                                                                                                                |
| Inputs       | One private admitted dispatch; for Launch only, an injected `ResolvedTerminalLaunchFacts` containing a stable Host `CreationId`, previously resolved executable, literal arguments, durable launch environment, omitted-cwd default, terminal profile, and optional terminal configuration; or one decoded Host success/`TypedFailure` plus the response envelope's exact Host epoch. Transport supplies only the definite-before-handoff or unknown-after-handoff distinction.                                             |
| Data         | The mapper retains no state. `ResolvedTerminalLaunchFacts::new` is the narrow composition seam for the node-local default-interactive-launch resolver and separately supplied creation-identity/cwd/profile/configuration facts; operating-system, account, environment, and filesystem lookup remain outside this module.                                                                                                                                                                                                  |
| Outputs      | Launch becomes `CreateTerminal`; Start becomes `StartTerminal`; Host success or `TypedFailure` becomes the existing `LaunchHostCompletion` or `StartHostCompletion`; transport loss becomes the kernel-facing pre-handoff-failure or post-handoff-unknown observation. The caller remains responsible for framing, I/O, correlation, handoff, and applying an observation to the kernel.                                                                                                                                    |
| Invariants   | Launch copies the independently resolved stable Host `CreationId`; it is not derived from product `TerminalId`. Start sends the OBCN target's Host `TerminalId` and preserves its full identity without imposing a product/Host byte-equality policy. Explicit cwd is copied exactly; omission selects the injected Worker-user-home default. Immediate Launch and Start preserve all four geometry dimensions; deferred Launch carries no geometry. Caller overlay is always empty. Other launch facts are copied exactly. |
| Decisions    | The mapper reuses the Terminal Protocol encoders as pure preflight validation, so public protocol values cannot bypass its identity, text, count, palette, geometry, or control-payload bounds. Terminal Protocol `TypedFailure.retryable` controls retry guidance, not failure kind; every generic `TypedFailure` therefore becomes `HostReportedFailure`. `HostRejected` requires a future explicit stable rejection contract. Response kind, identity, epoch, and geometry contradictions remain typed adapter errors.   |
| Model        | `Launch(dispatch, facts) -> CreateTerminal`; `Start(dispatch) -> StartTerminal`; `(Launch dispatch, expected CreationId, response epoch, exact response) -> Launch completion`; `(Start dispatch, response epoch, exact response) -> Start completion`; transport loss before handoff maps to definite failure, while loss after handoff maps to unknown. Each function is deterministic and side-effect free.                                                                                                              |
| Dependencies | Application-private OBCN authorization types and the existing Rust Terminal Protocol semantic values/encoders only. The module has no socket, envelope codec, carrier, clock, process, filesystem, account-database, environment-reader, shell resolver, or kernel-orchestration dependency.                                                                                                                                                                                                                                |
| Ownership    | The default-interactive-launch resolver owns node-local executable and durable default environment selection. The composition root owns stable Host creation identity plus omitted-cwd, profile, and optional configuration facts. This adapter owns only deterministic translation and cross-contract consistency checks. Terminal Host still owns PTY/process effects; the carrier owns exact envelope/correlation transport; the authorization kernel owns replay and outcome state.                                     |
| Verification | Unit tests cover deliberately different product `TerminalId` and Host `CreationId` values, immediate and deferred Launch, explicit and omitted cwd, stable Host creation identity, empty overlay and accepted default environment, Start, both transitions, both `retryable` values mapping to reported failure for each operation, both transport outcomes, wrong response kinds, identity/epoch/geometry contradictions, and exact argument/environment plus malformed-value bounds.                                      |
| Limits       | The current runtime still sends no terminal command because no carrier invokes this mapper or the authorization kernel. This slice performs no default resolution itself and does not add Host framing, socket I/O, activation, retry, reconciliation, or a Terminal Protocol change.                                                                                                                                                                                                                                       |

| Authorized input | Terminal Protocol request | Successful Host response                           | Kernel completion                                                                 |
| ---------------- | ------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------- |
| Immediate Launch | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity; kernel derives `Started` from the authorized complete geometry |
| Deferred Launch  | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity; kernel derives `AwaitingStart`                                 |
| Start            | `StartTerminal`           | `TerminalStartAccepted` with exact target/geometry | Accepted identity, geometry, and `started` or `already-started` transition        |

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
The capability-node protocol crate owns OBCN semantic values and framing without importing either
local-Site or Terminal Host types. Worker owns admission order, the Host broker connection, coupled
liveness, the unintegrated deterministic validation/replay/dispatch kernel, bounded default
interactive launch resolution, and the pure Terminal Host launch adapter. Server remains the only
placement, authorization-issuance, and reconciliation owner. Terminal Host remains the sole PTY,
process-tree, VT, history, and attachment authority.

The [`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model continues
to own Server admission and Worker-generation behavior. `LocalHostBroker` starts only after that
admission boundary and adds no second Server or Host model. The Server-owned
[`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) model
remains the abstract authorization authority; this Worker module is a concrete deterministic
refinement of its local admission, handoff, cache, and result actions, not a competing model.
That model now uses capability version 2 for admitted authority and version 1 only as an
unsupported request witness.
The Worker-owned [`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla)
composes the generation-local cross-terminal binding bound and rejection priority with the global
operation/result slot. Its TLC configuration uses capacity two plus one unseen identity rather than
claiming exhaustive checking of the production value 1,024. The model authority and its Rust
enforcement are one additive, unshipped delivery cohort; the model-only commit is not an
independently shippable boundary.

## Verification

Run from the repository root. The real Host process witness consumes the normal Terminal Host build
artifact, so build it before a focused Cargo test; the root merge gate already builds before tests.

```sh
pnpm --filter @orchbee/terminal-host build
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-site-process-spine,local-host-broker,terminal-launch-authorization,worker-terminal-binding-capacity
pnpm verify
```
