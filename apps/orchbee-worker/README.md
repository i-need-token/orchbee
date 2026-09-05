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

Only after both peers accept does Worker write `ready\n` to stdout. Its main thread then runs one
serialized OBCN Launch/Start, viewer, and controller request loop over the two owned sockets. It has no
connection thread or operation queue; each viewer owns one bounded delivery slot. Server loss closes the Host broker socket. Host loss stops admission and allows only a
bounded delivery attempt for an already classified result before closing the Server attachment.
Either peer loss exits silently with status `1`; retained terminals remain owned by Host. Invalid
invocation, connection, authentication, handshake rejection, framing, output, or transport failure
has the same silent status-`1` result. A valid operation-level rejection returns its typed OBCN result
and leaves the loop available for the next request.

Worker consumes authorization only from the admitted same-user Server and uses the OBLS generation
for its authorization kernel. It sends Create/Start only after local admission and a fresh handoff
check. It performs no retry, discovery, activation, daemonization, child-process supervision,
terminal inventory, PTY operation, Agent behavior, or durable write. Server Client
Create/Start authorization is integrated. Endpoint selection, service supervision, Host activation,
bare-entry TUI activation and reconciliation remain later boundaries;
bare `orchbee` is not yet a usable terminal Client.

## Live viewer relay refinement

The accepted local-terminal milestone now refines `LocalTerminalRelay` into the Worker's single
Host broker. Server remains the only issuer of viewer attachments. Each generation-scoped viewer
has read capability only, an independent bounded delivery slot, and at most one frame awaiting exact
Client feedback. Worker validates envelope and fixed routing metadata while retaining the complete
opaque Host record; it never decodes terminal rows, VT, or history. Attach, poll, feedback, and detach
are serialized with Launch/Start on the admitted Server connection. Poll transfers a ready record
without waiting for terminal output; downstream delivery cannot hold the Worker operation slot.

The OBCN viewer carrier and Worker attachment transitions now serve Server's admitted Client
Session relay, with actual Host process traces. Server owns OBCS viewer messages and Session cleanup;
bare-entry TUI activation remains a later integration boundary.
Eight live viewers and one retained frame per viewer are implementation bounds, not release
performance budgets. A missing or contradictory feedback never acknowledges a Host frame.
`NeedSnapshot` preserves its reason and sequence; `ProjectionBlocked` remains an attachment event.
A full delivery slot closes only that viewer and releases its queued bytes. Its Host detach runs
before the next Server operation; late events for that closing viewer are drained without retaining
another frame. No blocked event or frame is silently replaced. The broker's `host_connection`
module owns event routing, viewer slots, controller grants, and the shared deadline exchange; `terminal_viewer_operation`
maps admitted read-only commands and validates their exact Host responses. No viewer-state borrow
survives Host I/O. Attach performs two sequential bounded exchanges (capabilities, then active
projection demand); cleanup may perform up to sixteen bounded viewer/control detach exchanges before a request.
Server now budgets twenty seconds for those exchanges and OBNR record delivery. Lifecycle/metadata notifications are validated and drained; bell, clipboard, and
application-signal effects are not exposed through this read-only projection path.
Detach and broker loss release transient attachments and preserve retained terminals. Reattachment
requires a fresh Server-issued attachment identity; no input or resize authority is granted here.

Evidence maps the existing authority/relay model to deterministic routing, exact-feedback, stale
scope, and capacity tests, plus a real Worker/Host trace with two viewers, a stalled viewer, snapshot
feedback, detach, reattach, and retained terminal lifetime. Wire tests check bounded framing and
opaque byte preservation. The current-context challenge pass is not independent review evidence.

## Controller grant and effects

`terminal_control_operation` consumes Server-issued OBCN acquisition, input, resize, and internal
release requests. `host_connection::controllers` owns bounded pure grant state: one controller per
Host terminal, the exact viewer and controller identities, pending or accepted Host capability
revision, and the highest consumed sequence. No grant-state borrow spans Host I/O. Both viewer and
controller adapters use the correlated, deadline-bounded exchange owned by `host_connection`.

Acquisition requires a live read-only viewer and creates a separate Host attachment with read
disabled, input and resize enabled, and the resize lease requested. It preserves that viewer's
projection baseline. Only an exact capability acknowledgement commits the grant. Input preserves
1 through 4,096 raw bytes and uses the accepted capability revision; the Host must acknowledge the
entire byte count. Resize must acknowledge the complete geometry. Both actions reject a wrong
viewer/grant or non-increasing sequence before Host I/O. Known Host rejection consumes the sequence;
ambiguous or contradictory effects close the broker without replay.

Viewer detach or closure first releases its controller, then detaches the viewer. Internal release
is idempotent only when that viewer has no controller; it never clears a different current grant.
Unexpected resize-lease revocation retires the broker. Neither release nor loss promotes another
viewer or terminates retained execution. Eight viewers allow at most eight extra control
attachments; cleanup and normal operations share the single Host writer.

Deterministic grant tests cover exclusivity, stale scope, pending acquisition, sequence exhaustion,
and explicit re-acquisition. Fault process tests cover silent and contradictory Host input
acknowledgements with no replay. Server's real two-Client process witness verifies shell input,
actual PTY geometry, non-controller rejection, detach, and a fresh grant for the other Client.
These runtime witnesses refine the existing authority/relay models; they do not establish public
TUI behavior, independent review, or measured release performance.

## Default interactive launch resolution

The application-private `default_interactive_launch` module supplies the running request loop's
launch-fact composition. Its transport-neutral `DefaultInteractiveLaunch` output owns only one
resolved executable, an exactly empty literal argument vector, and three fixed environment entries.
The loop combines these with Worker-local cwd, Terminal Protocol profile, geometry, and independent
creation identity. The three entries map to the Terminal Protocol launch environment; the
invocation-scoped `CreateTerminal.environmentOverlay` remains empty in the accepted first milestone.

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

The application-private `terminal_authorization` module is the running request loop's
carrier-neutral deterministic kernel for the private Host launch adapter. It consumes the semantic
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) Launch and
Start values. The loop constructs one kernel per admitted Server attachment.

The composition root constructs this intrinsically version 2 kernel with only the current
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
| Problem      | The OBCN carrier may deliver a Server-authorized Launch or Start more than once, late, for the wrong Worker generation, or after authority has advanced. Worker must decide locally whether one Host operation may begin and must preserve the exact terminal result without acquiring Server placement authority or performing the Host effect itself.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Inputs       | Canonical OBCN `AuthorizedTerminalLaunch` or `AuthorizedTerminalStart` from an authenticated authoritative Server that obeys the globally fresh `OperationId` producer invariant, with its authorization capability checked against the kernel's intrinsic version 2 policy; the caller-supplied current local-Site Worker ID, nonzero OBLS generation, and current Unix-millisecond time; the observed Host epoch at Start admission and again at the handoff boundary; and a typed Host completion. A missing result or occupied slot remains explicit state rather than inferred success.                                                                                                                                                                                                                                                                                                                                                                                                              |
| Data         | `AuthorizedHostOperationKernel` owns at most 1,024 complete highest-fence bindings per Worker generation, keyed by product `TerminalId` and including operation kind, full authorization, Launch or Start payload, and an optional exact cached result. It separately owns one process-local slot in `empty`, `awaiting-handoff`, `handed-off`, or `result-ready` state and a maximum-observed Unix-millisecond high-water mark. A private `AuthorizedHostDispatch` retains the complete admitted operation and therefore its generation and authority binding.                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Outputs      | Admission returns `Dispatch`, `ResultReady`, or `SlotOccupied`; handoff returns `HandedOff` or `ResultReady`; `take_result` returns one typed OBCN Launch or Start result, including `CapacityExceeded` for a valid unseen terminal after the retained-binding limit is reached; invalid completion observations return a typed internal error without changing the slot. The kernel opens no carrier, performs no Host call, emits no product event, and writes no durable state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Invariants   | Wrong Worker, stale generation, unsupported capability, expired new lease, lower fence, conflicting same-fence binding, and a valid unseen terminal beyond capacity never dispatch. An exact binding dispatches at most once and replays only its cached exact result, even after expiry or at capacity. Once the kernel observes a lease as expired, a lower later time cannot revive it. A valid higher fence replaces the prior binding and clears its cache even at capacity. Capacity rejection never inserts a tombstone or changes any binding. At most one operation or undelivered result occupies the global slot. Only `handed-off` can become a Host outcome.                                                                                                                                                                                                                                                                                                                                 |
| Decisions    | Classification order is occupied slot, Worker identity, Worker generation, exact cached replay, capability, lease expiry, per-terminal fence, unseen-terminal capacity, then Start target epoch. Consequently unsupported or expired new requests retain their capability or lease classification at capacity; existing lower and same-fence requests retain their fence classifications; and only a valid unseen terminal receives `CapacityExceeded`. At Start admission and again at handoff, a target epoch mismatch becomes `HostIdentityChanged`; capacity precedes that check for an unseen terminal. Handoff expiry prevents an effect, and accepted Start identity, epoch, and geometry must remain exact.                                                                                                                                                                                                                                                                                       |
| Model        | One deterministic state owner combines a generation-local, 1,024-entry per-terminal monotonic binding map with one globally bounded operation/result slot. `admit_launch` and `admit_start` are the principal entry points; `begin_host_handoff` is the named external-effect boundary; typed observation methods close the accepted binding and queue its result. [`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla) composes the cross-terminal capacity refinement at a finite threshold plus one unseen identity with a minimal empty, operation-pending, and result-ready slot abstraction.                                                                                                                                                                                                                                                                                                                                                                             |
| Transitions  | The transition table below defines admission, handoff, completion, exact replay, result delivery, conflict, and stale-token behavior. Rejection before a binding is accepted queues an uncached result; every terminal outcome after a binding is accepted is cached before it is exposed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Dependencies | The Rust standard library, canonical local-Site Worker identity, and OBCN semantic types and constructors only. Time, socket liveness, decoded framing, current Host epoch, and Host observations are supplied as values by the operation adapters; the kernel monotonizes supplied time but never reads a clock. It does not read an environment, filesystem, socket, process, profile, executable, shell, or working directory, and it does not parse or encode Host framing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
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

The application-private `terminal_host_launch_adapter` module is consumed by the running single-operation
Host exchange. It is a pure value mapper between one admitted `AuthorizedHostDispatch`,
existing Terminal Protocol request/response values, and the Host observations already accepted by
`terminal_authorization`; it neither opens a Host socket nor changes kernel state.

### Launch-adapter modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | An admitted OBCN Launch or Start must cross the existing Terminal Protocol without giving the carrier authority to invent a process, directory, environment, identity, geometry, or outcome.                                                                                                                                                                                                                                                                                                                                |
| Inputs       | One private admitted dispatch; for Launch only, an injected `ResolvedTerminalLaunchFacts` containing a stable Host `CreationId`, previously resolved executable, literal arguments, durable launch environment, omitted-cwd default, terminal profile, and optional terminal configuration; or one decoded Host success/`TypedFailure` plus the response envelope's exact Host epoch.                                                                                                                                       |
| Data         | The mapper retains no state. `ResolvedTerminalLaunchFacts::new` is the narrow composition seam for the node-local default-interactive-launch resolver and separately supplied creation-identity/cwd/profile/configuration facts; operating-system, account, environment, and filesystem lookup remain outside this module.                                                                                                                                                                                                  |
| Outputs      | Launch becomes `CreateTerminal`; Start becomes `StartTerminal`; Host success or `TypedFailure` becomes the existing `LaunchHostCompletion` or `StartHostCompletion`. The caller remains responsible for framing, I/O, correlation, handoff, and applying an observation to the kernel.                                                                                                                                                                                                                                      |
| Invariants   | Launch copies the independently resolved stable Host `CreationId`; it is not derived from product `TerminalId`. Start sends the OBCN target's Host `TerminalId` and preserves its full identity without imposing a product/Host byte-equality policy. Explicit cwd is copied exactly; omission selects the injected Worker-user-home default. Immediate Launch and Start preserve all four geometry dimensions; deferred Launch carries no geometry. Caller overlay is always empty. Other launch facts are copied exactly. |
| Decisions    | The mapper reuses the Terminal Protocol encoders as pure preflight validation, so public protocol values cannot bypass its identity, text, count, palette, geometry, or control-payload bounds. Terminal Protocol `TypedFailure.retryable` controls retry guidance, not failure kind; every generic `TypedFailure` therefore becomes `HostReportedFailure`. `HostRejected` requires a future explicit stable rejection contract. Response kind, identity, epoch, and geometry contradictions remain typed adapter errors.   |
| Model        | `Launch(dispatch, facts) -> CreateTerminal`; `Start(dispatch) -> StartTerminal`; `(Launch dispatch, expected CreationId, response epoch, exact response) -> Launch completion`; `(Start dispatch, response epoch, exact response) -> Start completion`. The operation exchange owns the transport handoff distinction. Each function is deterministic and side-effect free.                                                                                                                                                 |
| Dependencies | Application-private OBCN authorization types and the existing Rust Terminal Protocol semantic values/encoders only. The module has no socket, envelope codec, carrier, clock, process, filesystem, account-database, environment-reader, shell resolver, or kernel-orchestration dependency.                                                                                                                                                                                                                                |
| Ownership    | The default-interactive-launch resolver owns node-local executable and durable default environment selection. The composition root owns stable Host creation identity plus omitted-cwd, profile, and optional configuration facts. This adapter owns only deterministic translation and cross-contract consistency checks. Terminal Host still owns PTY/process effects; the carrier owns exact envelope/correlation transport; the authorization kernel owns replay and outcome state.                                     |
| Verification | Unit tests cover deliberately different product `TerminalId` and Host `CreationId` values, immediate and deferred Launch, explicit and omitted cwd, stable Host creation identity, empty overlay and accepted default environment, Start, both transitions, both `retryable` values mapping to reported failure for each operation, wrong response kinds, identity/epoch/geometry contradictions, and exact argument/environment plus malformed-value bounds.                                                               |
| Limits       | The mapper performs no default resolution itself and does not own Host framing, socket I/O, activation, retry, reconciliation, or a Terminal Protocol change.                                                                                                                                                                                                                                                                                                                                                               |

| Authorized input | Terminal Protocol request | Successful Host response                           | Kernel completion                                                                 |
| ---------------- | ------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------- |
| Immediate Launch | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity; kernel derives `Started` from the authorized complete geometry |
| Deferred Launch  | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity; kernel derives `AwaitingStart`                                 |
| Start            | `StartTerminal`           | `TerminalStartAccepted` with exact target/geometry | Accepted identity, geometry, and `started` or `already-started` transition        |

## Local Host broker modeling record

The checked model governs the complete broker lifecycle. The single-operation exchange and
integrated request loop below refine its Host effect and bounded result-delivery boundaries.
The executable now invokes both after its two handshakes; restart recovery remains unimplemented.

| Field        | Modeled broker contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | After Server-first dual admission, the Worker carrier must turn one dispatch already admitted by the authorization kernel into at most one Terminal Host effect and return its exact OBCN result. It must preserve kernel `ResultReady` decisions made during `begin_host_handoff`, validate an actual Host response rather than a diagnostic label, and never invent a Host-rejection classification that the wire does not carry.                                                                                                                                                                                                                                                                                                                                       |
| Inputs       | Accepted Server and Host links and the handshake Host epoch; one OBCN Launch or Start `Dispatch` already admitted through the kernel's identity, generation, intrinsic capability, lease, fence, 1,024-binding capacity, and global-slot gates; the Start target Host epoch; the current Host epoch supplied to `begin_host_handoff`; one nonzero command correlation; the actual Host response role, kind, correlation, and epoch; timeout or link loss; and one bounded OBCN result-delivery opportunity. A retained Host terminal may also be observed after Worker loss.                                                                                                                                                                                              |
| Data         | One process-local broker phase; one operation phase projected onto the kernel's single global `empty`/`awaiting-handoff`/`handed-off`/`result-ready` slot; the Launch/Start kind; handoff-attempt and effect-boundary counts; Start target and observed handoff Host epochs; mapped Host command and correlation; retained response role, kind, correlation, and epoch; whether Host applied the command; the exact completion source and cached-result fact; one bounded delivery count; an unknown retry barrier; and retained-terminal disposition. No adapter queue or durable state is introduced.                                                                                                                                                                   |
| Outputs      | Launch maps only to Host `CreateTerminal`; Start maps only to Host `StartTerminal`. Lease expiry during `begin_host_handoff` returns cached OBCN `Rejected(LeaseExpired)`; a changed Start Host epoch returns cached `Rejected(HostIdentityChanged)` with the observed epoch; neither crosses the effect boundary. Definite transport failure before the call remains cached `Failed(HostUnavailableBeforeHandoff)`. Exact success becomes `HostAccepted`; an exact generic Terminal Protocol `TypedFailure` becomes `Failed(HostReportedFailure)`. Unknown and retained-state outcomes follow the prior bounded lifecycle.                                                                                                                                               |
| Invariants   | Server admission precedes Host contact. Readiness requires both accepted links and a nonzero Host epoch. At most one kernel-global operation/result slot and one Host command exist. A successful `begin_host_handoff` precedes the first possible Host effect; its `ResultReady` branches retain an attempted call but zero crossed boundaries, commands, or effects. A known Host completion requires actual response role `Response`, the request-specific expected kind, exact command correlation, and exact retained handoff Host epoch. An exact generic `TypedFailure` can produce only `HostReportedFailure`; a mismatched one fails closed as cached unknown, and neither can produce `HostRejected`. Unknown never retries.                                    |
| Decisions    | Kernel results before dispatch, including `CapacityExceeded` and `SlotOccupied`, remain distinct and never enter this adapter model. During handoff, lease expiry and Start identity replacement are exact cached rejections, not transport failures. An authoritative Host-rejection discriminator does not exist in the current Terminal Protocol, so identical exact generic `TypedFailure` input is never nondeterministically split: it maps conservatively to OBCN `Failed(HostReportedFailure)`. OBCN `HostRejected` is reserved for a future explicitly classified owner contract. Post-handoff loss, timeout, or invalid response is `OutcomeUnknown`.                                                                                                           |
| Model        | [`LocalHostBroker`](formal/tla/LocalHostBroker.tla) retains the Server-first handshake and coupled lifecycle, takes one admitted dispatch from Server-owned [`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla), and projects its detailed `awaiting-handoff`/`handed-off` phases onto `WorkerTerminalBindingCapacity`'s single `operation-pending` global kernel slot. Its Host-side effect boundary and lost-reply classification refine [`TerminalHostRequestOutcome`](../terminal-host/formal/tla/TerminalHostRequestOutcome.tla) and the [Terminal Host runtime model](../terminal-host/docs/runtime-model.md); it does not reimplement any authority or change a wire contract.                                           |
| Transitions  | The table below covers admission, both `begin_host_handoff -> ResultReady` branches, definite pre-call transport failure, successful handoff, command emission and possible apply, concrete response validation, conservative generic failure, independently ordered Server/Host loss, bounded exact-result delivery, the unknown retry barrier, and retained-terminal recovery.                                                                                                                                                                                                                                                                                                                                                                                          |
| Dependencies | The adapter depends on the authorization kernel and separately checked `WorkerTerminalBindingCapacity` bound and global slot, OBCN semantic values and codec, local-Site carrier, Terminal Protocol bindings, Terminal Host runtime outcome contract, authenticated sockets, and bounded timers. The model treats the kernel's post-capacity `Dispatch`, decoded Host records, and liveness as inputs. It does not select carrier bytes, deadline values, queue implementation, activation, shell, working directory, PTY behavior, or reconciliation policy.                                                                                                                                                                                                             |
| Ownership    | Server owns placement, issuance, generation, and reconciliation. The Worker authorization kernel owns binding capacity, admission/replay, exact cached results, and the single global operation/result slot. The adapter may consume only `Dispatch` and must not add a parallel slot or capacity counter. `WorkerTerminalBindingCapacity` checks the bound and collapses this model's pre-result adapter phases into its `operation-pending` phase. OBCN owns result variants. Terminal Protocol owns response roles, kinds, correlation, epoch, and generic `TypedFailure`, but no generic rejected/failed discriminator. Terminal Host owns request commit/effect/reply ordering. Worker owns only mapping, strict observation, loss classification, and link cleanup. |
| Verification | The TLA inventory requires every admission, both handoff-result branches, command/effect, concrete response mismatch, accepted/generic-failure outcome, delivery, ordered loss, retry-block, and recovery action. Triggers cover the kernel and carrier surfaces, both Terminal Protocol bindings, Terminal Host source and runtime model, `TerminalHostRequestOutcome`, and accepted topology/capacity authority. TLC checks safety and finite progress under weak fairness. The Host exchange and integrated request loop have socket and real-process refinement witnesses described below.                                                                                                                                                                            |
| Correctness  | Checked invariants map operation phases to the kernel-global slot; distinguish attempted handoff from the crossed effect boundary; require exact cached lease/identity rejection and pre-call failure classifications; preserve the full handoff-result projection through bounded delivery or abandonment; validate retained response fields; classify exact generic `TypedFailure` as `HostReportedFailure`, mismatched generic failure as unknown, and neither as rejected; and retain the prior one-command, unknown/no-retry, link-cleanup, result-delivery, and recovery properties. These are bounded model claims, not proof of the Rust kernel, carrier, Host, codecs, or a capacity implementation.                                                             |
| Limits       | The state space contains one Worker, one admitted operation, two symbolic nonzero Host epochs, one operation correlation plus one mismatched correlation, one kernel-global result slot, and at most one delivery. The model starts after dispatch, so pre-admission rejection/replay and enumeration of the 1,024-binding bound stay in the kernel and `WorkerTerminalBindingCapacity`; the explicit slot projection prevents the adapter from inventing another outstanding operation. Byte framing, numeric timeout duration, failure-code taxonomy, carrier integration, inventory, and recovery remain outside this slice. No current runtime behavior or production adoption follows from the abstraction.                                                          |

| Current state                   | Observation or action                                                   | Next state               | Exact result and lifecycle rule                                                                                                 |
| ------------------------------- | ----------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Before Server admission         | Host connect attempt                                                    | Forbidden                | Host contact has no enabled transition.                                                                                         |
| Ready, kernel slot empty        | Consume one kernel `Dispatch`                                           | `admitted`               | Project the global slot to `awaiting-handoff`; capacity, replay, rejection, and occupied-slot results do not dispatch.          |
| `admitted`                      | `begin_host_handoff` observes expired lease                             | `result-ready`           | Cache and queue exact `Rejected(LeaseExpired)`; record one attempted call and no crossed effect boundary or Host command.       |
| `admitted` Start                | `begin_host_handoff` observes a Host epoch different from its target    | `result-ready`           | Cache and queue exact `Rejected(HostIdentityChanged)` with both target and observed epochs; perform no Host effect.             |
| `admitted`                      | Definite transport failure or Host loss before the handoff call         | `result-ready`           | Cache and queue `Failed(HostUnavailableBeforeHandoff)` distinctly from both handoff rejections.                                 |
| `admitted`                      | Successful `begin_host_handoff` with the retained current Host epoch    | `handed-off`             | Cross the named external-effect boundary exactly once and project the kernel slot to `handed-off`.                              |
| `handed-off`                    | Emit mapped command with one operation correlation                      | `awaiting-host`          | Send at most one Host command; Host may apply it before a response is observable.                                               |
| `awaiting-host`                 | `Response` + expected success kind + exact correlation + handoff epoch  | `result-ready`           | Queue cached OBCN `HostAccepted`.                                                                                               |
| `awaiting-host`                 | `Response` + generic `TypedFailure` + exact correlation + handoff epoch | `result-ready`           | Queue cached `Failed(HostReportedFailure)`; never derive `HostRejected` from the same exact record.                             |
| `awaiting-host`                 | Wrong role, kind, correlation, or Host epoch                            | `draining-result`        | Retain the concrete mismatched observation, fail closed as cached outcome unknown, close Host, and block retry.                 |
| `handed-off` or `awaiting-host` | Host loss or timeout                                                    | `draining-result`        | Queue cached outcome unknown, close Host, block retry, and retain Server only for bounded result delivery.                      |
| Any live operation/result       | Server loss                                                             | `stopped`                | Close both links immediately; an unobserved in-flight dispatch is unknown and any undelivered exact result is abandoned intact. |
| `result-ready`                  | Result write succeeds or its bounded opportunity expires                | `completed` or `stopped` | Empty the global slot; deliver the cached result exactly once, or abandon it without changing its classification.               |
| `stopped`, retained terminal    | Recovery classification                                                 | `recovery-required`      | Preserve the unmatched disposition for explicit reconciliation; no Worker-adoption transition exists.                           |

For runtime refinement, keep capacity, `SlotOccupied`, pre-admission rejection, cached replay, and
the one global slot in `AuthorizedHostOperationKernel`; consume only its post-capacity `Dispatch`
and never introduce an adapter-local operation queue. Call
`begin_host_handoff` once and inspect its return before any Host send: forward either exact
`ResultReady` rejection unchanged, or retain the returned handoff epoch for one command. Allocate
one nonzero correlation for that dispatch. A known completion requires actual `Response` role, the
request-specific success kind or generic `TypedFailure`, exact correlation, and exact handoff Host
epoch. Map generic `TypedFailure` only to `HostReportedFailure` until an authoritative classified
Host-rejection contract exists. Treat any post-boundary uncertainty as outcome unknown, cache it in
the kernel, and never resend. Keep delivery single-slot and bounded, and quarantine inventory after
Worker death for explicit recovery rather than startup adoption.

### Single-operation Host exchange refinement

The accepted local-terminal milestone delegates the `terminal_host_operation` adapter to the
Worker owner. Its consumer is the running Worker OBCN request loop: after kernel admission,
that loop supplies resolved launch facts and a fresh nonzero Host correlation, executes one
prepared command, then delivers the kernel's exact result before admitting another request.
Server request consumption, default cwd/profile/creation-identity composition, and result delivery
belong to that loop, not to this adapter.

`PreparedHostOperation` binds one kernel dispatch to a validated Create or Start payload and its
correlation. Preparation is effect-free and cannot change the kernel. `execute` uses the admitted
Host connection, the already admitted Server stream, an injected lease clock, and a caller-supplied
positive I/O budget. A single monotonic deadline bounds the complete Host write and response read,
including fragmented records; it does not choose a release timeout. Kernel state is the only
operation/result slot. No queue, result cache, retry, identity allocator, shell resolver, terminal
inventory, or VT state is added. Valid lifecycle and metadata events are drained as described below,
including before handoff and between a command write and its exact response. They cannot complete
an operation and consume the same Host deadline.

| Boundary         | Runtime refinement and evidence obligation                                                                                                                                                                                                                                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Before handoff   | Check both peers without consuming Server bytes. Observed Server loss or unexpected Server data closes both links and abandons the pending operation. Host loss or invalid unsolicited Host data closes Host and records the kernel's exact pre-handoff failure (or expired-lease rejection). Preparation and invalid budgets send nothing. |
| Handoff          | Call `begin_host_handoff` once with the retained handshake epoch and freshly sampled lease clock. Its `ResultReady` branches send nothing and preserve the exact cached lease or Host-identity rejection. Only `HandedOff` permits the first write.                                                                                         |
| Response         | Require exact protocol, mandatory control encoding, Response role, command correlation, and handoff epoch before decoding the operation-specific success or generic `TypedFailure`. The existing mapper validates payload identity and geometry. Generic failure maps only to `HostReportedFailure`.                                        |
| Uncertainty      | Any write/read/timeout/invalid-response failure after handoff records the kernel's exact unknown result and shuts down Host. Observed Server loss closes both links; no command is resent.                                                                                                                                                  |
| Result ownership | Success and every classified failure remain in the kernel slot. The caller must deliver or abandon that result and honor `HostClosed`/`ServerClosed`; this adapter never drains the slot or claims OBCN delivery.                                                                                                                           |

The module depends only on `host_connection`, `terminal_authorization`,
`terminal_host_launch_adapter`, and their protocol values. Its private `transport` child owns
nonblocking byte I/O and polls the Server alongside every Host wait; it imports no product policy.
These directions are one-way. The running request loop invokes this module.

Deterministic socket witnesses map the table to the existing `LocalHostBroker` handoff, exact
completion, mismatch, ordered-loss, and unknown/no-retry actions. A real built Zig Host witness
checks deferred Create and Start, stable Host identity, and retained inventory after broker loss.
This exchange leaves delivery to the request loop; recovery remains outside the runtime refinement. No Lean
claim is needed for a new pure transformation: mapping remains in the existing checked mapper;
finite protocol partitions and controlled socket timelines own this adapter's evidence. The real
Host witness proves only the native operating system on which it runs, not the full Client path.

| Claim                                     | TLA+ property                                                                       | Runtime witness (`terminal_host_operation::tests`)                                                                                                                                                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Handoff rejection sends nothing           | `HandoffResultReadyIsExact`, `BeginHandoffPrecedesEveryHostEffect`                  | `handoff_rejections_and_invalid_preparation_never_send_a_command`                                                                                                                                     |
| Exact completion and one result slot      | `HostCompletionIsStrictlyCorrelated`, `KernelGlobalSlotRefinesOperation`            | `exact_launch_success_remains_in_the_kernel_slot_and_replays_without_an_effect`, `start_responses_preserve_transitions_and_reject_contradictory_payloads`                                             |
| Generic Host failure stays failed         | `GenericTypedFailureIsNeverRejected`                                                | `exact_generic_failure_is_failed_even_when_its_diagnostics_say_rejected_and_retryable`                                                                                                                |
| Invalid response is unknown without retry | `MismatchedHostResponseCannotProduceKnownOutcome`, `UnknownOutcomeNeverRetriesHost` | `malformed_response_partitions_close_host_cache_unknown_and_never_resend`                                                                                                                             |
| Ordered loss closes the right links       | `StoppedWorkerOwnsNoLink`, `ObservableUnknownBeginsAfterHandoff`                    | `observed_host_loss_before_handoff_is_a_definite_failure`, `server_loss_before_handoff_abandons_without_sending_or_inventing_a_result`, `server_loss_interrupts_a_waiting_host_and_closes_both_links` |
| Deadline and retained execution           | `PendingOperationEventuallySettles`, `RetainedHostIsNeverSilentlyAdopted`           | `a_silent_host_is_bounded_by_the_operation_deadline`, `real_zig_host_creates_starts_and_retains_the_same_terminal_after_broker_loss`                                                                  |

## Integrated authorized request loop

`terminal_operations` composes the accepted local-terminal milestone inside the private Worker.
Its inputs are the admitted Server stream, admitted Host broker, local Worker identity, and OBLS
generation. It owns their serialized operation lifetime; `terminal_authorization` remains the only
launch binding/cache/operation authority. Viewer/control requests use the Host broker
attachment state and their respective operation adapters. Its private `server_transport` child adapts canonical OBCN
`Read`/`Write` codecs to nonblocking sockets and peer observation. Its private `launch_facts` child
resolves native launch inputs. Neither child gains Server placement authority or Host process state.

The refinement is `idle -> bounded request read -> kernel admission -> optional preparation and
Host exchange -> bounded exact result delivery -> idle`. A cached replay or rejection skips
preparation and Host I/O. No next request is admitted until result delivery completes. Malformed,
wrong-direction, oversized, partial-timeout, and observed pipelined requests close the attachment;
the codec bounds OBCN payload allocation to 8 KiB. Idle waits have no timeout; once the first record
is observable, all its fragments share a two-second monotonic budget. Result writes also share one
two-second budget. Both waits observe Host loss. Host loss during result delivery preserves that
bounded delivery attempt, then ends the Worker; Server loss abandons delivery and closes Host.

Each actual Host exchange has a one-second total write/read budget, leaving time for result delivery
inside the Server carrier's twenty-second result wait. This is an experimental local bound,
not a measured release performance claim. Local account/filesystem resolution is synchronous;
those OS calls are not covered by the socket deadline. A slow or failed resolution may therefore
make Server's outcome unknown. A preparation failure closes the attachment before any Host command,
without inventing a Host-reported failure. A missing or unrepresentable wall clock expires leases;
the kernel still prevents time rollback from reviving previously expired authority.

For an admitted Launch, explicit absolute cwd is preserved literally and requires no account-home
lookup. Omission reads the effective user's account home, ignoring inherited `HOME` and Worker
startup cwd. The existing shell resolver supplies executable, empty arguments, and the three
fixed launch environment entries. Configuration is absent and transient overlay is empty.
The baseline Terminal Protocol profile matches the existing Orchbee default: light scheme,
`0xfcfcfc` background, `0x141414` foreground, nonblinking block cursor with `0x57bef8` cursor color,
the 16 ANSI colors followed by the 216-color cube and 24 grays, conformance level 62 with feature 22,
and `Orchbee` version response. This private native default does not import Desktop source or add
a product Launch Profile or appearance preference.

Host also sends unsolicited `TerminalStarted`, `TerminalExited`, `TerminalFailed`, and
`TerminalMetadataChanged` events. The private `host_connection::events` adapter validates their
Event role, absent correlation, exact protocol/epoch, payload codec, and matching identity epoch.
It drains these notifications without a lifecycle/metadata projection. The broker routes viewer
frames and blocked events through their independent slots, including events interleaved with a
Launch/Start response. Events never acknowledge an operation or adopt retained terminals.
Response-like, unknown, malformed, or wrong-epoch events fail closed. An event has a one-second
read budget capped by an active OBCN deadline. Allocation is bounded by the OBTP header plus the
36 MiB frame limit; the envelope still enforces the 64 KiB control limit. Valid discarded lifecycle
events remain state-preserving steps in the symbolic operation model.

Creation and command correlation identities are independent 128-bit reads from the POSIX OS
random source. A zero value or read failure fails preparation closed. Product Terminal identity is
never used as a Host creation identity. Resolved facts live only in the one prepared operation;
replay uses the kernel's exact cached result and never resolves another launch or creates another
Host terminal. Start resolves neither shell nor cwd and preserves the exact authorized Host target
and complete geometry. Worker does not adopt retained Host inventory after reconnect.

Process witnesses in `tests/process/terminal_operations.rs` cover real Worker and Zig Host deferred
Create, Start, immediate Create, exact replay, and both retained terminals after Server/Worker loss;
a fake Host checks omitted/explicit cwd, launch environment, profile, and independent identities.
Further partitions cover authorization rejection with no command, wrong-direction/oversized/partial/
pipelined OBCN, a silent Host yielding one exact unknown result followed by disconnection, and exact result
delivery after Host closes immediately after its response.
These witnesses refine the existing `LocalHostBroker` admission, delivery, loss, and no-retry claims;
framing and OS facts remain concrete assumptions outside its symbolic state space. They do not
prove Server Client Session viewer routing, activation, restart reconciliation, or another OS platform.
The viewer process trace separately proves two read attachments, a stalled viewer, exact feedback,
snapshot recovery, detach/reattach, and terminal retention through the real Worker and Host.

## Ownership

The [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.md) crate owns the
Server-Worker framing. The
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) crate owns the
kernel same-user proof used for both local connections. The
[`orchbee-terminal-protocol`](../../packages/terminal-protocol/references/protocol.md) package owns
the Host wire; its independent Rust binding supplies the consumed control, envelope, and routing codecs.
The capability-node protocol crate owns OBCN semantic values and framing without importing either
local-Site or Terminal Host types. Worker owns admission order, the Host broker connection, coupled
liveness, integrated deterministic validation/replay/dispatch, bounded default interactive launch
resolution, Host operation mapping, transient viewer relay state, and grant-scoped control effects. Server remains the only
placement, authorization-issuance, and reconciliation owner. Terminal Host remains the sole PTY,
process-tree, VT, history, and attachment authority.

The [`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model continues
to own Server admission and Worker-generation behavior. `LocalHostBroker` preserves Server-first
admission as an ordering precondition for one broker; it does not redefine generation or add a
second Server or Host authority. The Server-owned
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
node scripts/tla/run-models.mjs --models local-site-process-spine,terminal-launch-authorization,worker-terminal-binding-capacity,local-host-broker,terminal-host-request-outcome
pnpm verify
```
