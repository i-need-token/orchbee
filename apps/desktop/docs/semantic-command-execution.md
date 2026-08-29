# Semantic Command Execution

## Boundary

Desktop owns one process-lifetime semantic-command coordinator generation, an explicit operation
catalog, a bounded in-memory execution ledger, and the private local endpoint that exposes that
service to an authenticated current-user client. The production catalog contains exactly twelve
operations: `agent.session.list`, `agent.session.get`, `agent.session.explain`,
`agent.session.wait`, and `agent.session.changes`, plus `workbench.project.list`, `workbench.directory.list`,
`workbench.workspace.list`, `workbench.tab.list`, `workbench.panel.list`,
`workbench.terminal.list`, and `workbench.terminal.create`. It does not contain an arbitrary Action,
filesystem, shell, renderer, or Terminal Host escape hatch.

Feature owners retain input and output meaning. The protocol package owns strict envelopes and
local HTTP mapping. Desktop owns catalog admission, execution, locator publication, endpoint
lifecycle, and OS peer authentication. It does not own Workbench resource semantics, CLI
presentation, remote authentication, or durable semantic-execution recovery.

| Input              | Validation and absence                                                                                                                                                  |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Locator            | Strict version, generation, and 32-character endpoint identity only; no credential, path, address, or product data                                                      |
| Local peer         | POSIX effective UID or Windows current-user process SID in both directions; identity absence or mismatch rejects the connection                                         |
| HTTP request       | One close-delimited HTTP/1.1 request, strict method/path, 16 KiB headers, 64 KiB body, JSON-only `POST`; malformed or over-capacity input never reaches the service     |
| Invocation request | `SemanticCommandInvocationRequestSchema`; malformed values allocate no execution                                                                                        |
| Catalog entry      | Constructor-validated unique identity and version, public descriptor, canonical input parser, authorization and confirmation policy, safe-output validator, and handler |
| Handler settlement | Strict `SemanticCommandTerminalResult`; throws, invalid output, and results above the individual safe-output bound become payload-free owned failures                   |

## Data and outputs

| State or output        | Owner and lifecycle                                                                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| Catalog                | Immutable `DesktopSemanticCommandCatalog`; only five Agent queries and seven Workbench entries exist          |
| Coordinator generation | Created once with the service, copied into the locator, and present in every public snapshot                  |
| Endpoint identity      | Random process-local identifier used only for the locator and socket or named-pipe name                       |
| Locator                | Native adapter publishes one private bounded file after listener creation and removes it during teardown      |
| Request receipt        | Ledger-owned request identity, canonical SHA-256 operation fingerprint, and execution identity while retained |
| Active execution       | Ledger-owned identity, commit marker, and cancellation-forward count; the service owns one `AbortController`  |
| Terminal execution     | Ledger-owned safe result and measured byte weight until oldest-first eviction                                 |
| HTTP response          | One strict JSON response no larger than 1 MiB; framing errors are payload-free and never serialize exceptions |
| Observation output     | Retained snapshot or an exact request/execution receipt-unavailable result                                    |

The ledger stores no canonical input. An asynchronous handler closure may hold canonical input only
while its execution is active. Diagnostics receive no operation input or result payload. A query
result may explicitly contain the requested names and paths in its bounded response and receipt.

`workbench.directory.list` alone uses operation schema version `2` and output schema version `2`;
each result includes the Directory placement. The other six Workbench operations remain version
`1`. A remote Directory result contains only its committed opaque Environment identity and
canonical path. The requested alias, failed observations, and raw operating-system diagnostics
never enter semantic output.

Tab and Panel lists project only canonical shared-document membership. Tab order is the shared Tab
collection order; exact-target Panel order is depth-first and left-to-right. Neither query reads or
returns Client View active, selected, focused, maximized, active-child, split-weight, or geometry
state. Signed cursors bind to the exact location, and Panel cursors additionally bind to the exact
Tab identity.

All five Agent operations use schema version `1`. List, get, and explain take one canonical snapshot
from the application-lifetime `AgentSnapshotSource`. List pages are signed and revision-fenced;
get and explain target one exact retained Session. Wait is a server-owned asynchronous query that
subscribes before its decisive recheck, reevaluates the canonical source, and settles on one exact
match, missing Session, future revision baseline, timeout, cooperative cancellation, or coordinator
retirement. Changes takes an immediate empty baseline or follows a signed cursor over the latest 512
applied runtime transitions. It returns at most 100 ordered content-free records, subscribes before
rechecking a current cursor, completes timeout as `current`, and returns explicit `reset-required`
when retention was crossed. Semantic-status conditions require fresh observation. No Agent query
reads a renderer, terminal screen, provider transport, prompt, transcript, tool content, filesystem
path, environment value, provider error, or adapter-private correlation data.

## Invariants

1. One retained request identity maps to exactly one retained execution and one canonical
   fingerprint.
2. Repeating that request with the same fingerprint returns the existing snapshot; a different
   fingerprint returns `request-identity-conflict`; neither starts another handler.
3. Invalid, unavailable, unauthorized, confirmation-requiring, version-incompatible, and
   over-capacity requests allocate no execution.
4. At most 32 executions are active, and active records are never evicted.
5. At most one cooperative cancellation is forwarded for one active uncommitted execution. Once
   durable commit is marked, cancellation is not forwarded. Cancellation never manufactures a
   terminal result or overwrites a handler settlement.
6. A handler settlement marked `committed` can contain only `completed`; an uncommitted settlement
   may contain any terminal result kind.
7. Terminal retention contains at most 256 records and at most 32 MiB of encoded safe results.
   Completion evicts the oldest terminal records until both limits hold.
8. One execution result is bounded below the local response capacity. Oversize becomes
   `failed/output-capacity-exceeded` with no owner payload.
9. Discovery and description expose exactly the twelve composed operations. Catalog registration or
   UI Action availability alone grants no process authority.
10. The native adapter follows neither a POSIX symlink nor a Windows reparse point and accepts only
    a current-user private runtime directory, locator, endpoint, and mutually authenticated peer.
11. One connection carries one bounded request and one bounded response. Unknown framing, method,
    path, content type, trailing bytes, and capacity fail closed.
12. Locator publication follows listener creation. Shutdown retires the locator and listener before
    disposing the service. A client disconnect has no service transition and causes no replay.

## Transitions

| State            | Event                       | Guard                                                                                            | Result                                                                              |
| ---------------- | --------------------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Endpoint absent  | Start                       | Trusted runtime directory and native listener validate                                           | Create listener, publish matching locator, then report ready                        |
| Listening        | Accept                      | Peer identity matches the current effective OS user                                              | Read exactly one bounded request                                                    |
| Listening        | Reject peer                 | Peer identity is absent or mismatched                                                            | Close that connection; allocate no execution                                        |
| Connected        | Reject request              | Framing, route, content type, timeout, or capacity guard fails                                   | Return one bounded local error when possible; service and ledger unchanged          |
| Absent request   | Admit                       | Catalog entry is visible, version/input/authorization/confirmation pass, active capacity remains | Allocate one execution and start one handler                                        |
| Retained request | Repeat or conflict          | Fingerprint matches or differs                                                                   | Return existing snapshot, or conflict without a second handler                      |
| Active execution | Cancel                      | Execution is uncommitted and no cancellation was forwarded                                       | Forward one cooperative abort                                                       |
| Active execution | Mark committed              | The canonical Workbench revision completed durable storage                                       | Record commit before post-commit reconciliation; reject later cancellation          |
| Active execution | Settle                      | Settlement and commit-marker consistency guards hold                                             | Retain one validated terminal result                                                |
| Connected        | Client disconnects          | Request may already have reached the service                                                     | Drop only the response transport; do not cancel or replay accepted execution        |
| Any              | Process disposal or restart | Coordinator lifetime ends                                                                        | Retire locator/listener, abort active signals, and discard the in-memory generation |

## Dependencies and ownership

`DesktopSemanticCommandService` orchestrates catalog admission, fingerprinting, handler execution,
safe-result capacity, and cancellation. `DesktopSemanticCommandExecutionLedger` is the sole mutable
receipt writer and has no network, filesystem, clock, Workbench, Electron, or process dependency.
`createDesktopWorkbenchReadOperations` adapts the canonical shared document through the Workbench
owner's deterministic projections and binds page cursors to operation, target, generation, and
revision. `createDesktopWorkbenchTerminalCreateOperation` validates the Workbench-owned strict
input, applies the owner operation to an isolated hydrated runtime, commits its projected next
revision only through `DesktopClientSessionCoordinator`, and marks the execution committed in the
coordinator's immediate post-storage callback. The isolated runtime never dispatches a provision
command; normal canonical reconciliation starts the PTY only after persistence succeeds.

`createDesktopAgentReadOperations` adapts the application-lifetime `AgentSnapshotSource` and
`AgentChangeSource` through Agent-owned projections and contracts. Desktop owns signed cursor
binding, asynchronous wait and change subscriptions, timeout scheduling, cooperative abort, and the
coordinator-lifetime failure boundary; it does not reinterpret Agent status or become another Agent
state owner.

`DesktopSemanticCommandLocalEndpoint` owns the Worker lifetime. The Worker performs bounded
blocking local I/O away from Electron Main and forwards only complete request bytes. The native
adapter owns private runtime, locator, Unix-domain socket or Windows named-pipe creation plus mutual
current-user validation. `@orchbee/semantic-command-protocol/local-http` owns framing and routes;
the CLI owns trusted enumeration and response validation. The endpoint is separate from Terminal
Host and Browser access and opens no TCP listener.

## Correctness evidence

| Claim                                                 | Contract         | Formal or generated evidence                                                                                                  | Deterministic or real refinement                                                                                                                                    | Limits                                                                                                     |
| ----------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Explicit authorized catalog only                      | Invariants 1-3,9 | `SemanticCommandExecutionLedger.tla`: `RequestAndExecutionMappingsAgree`                                                      | Catalog/service tests cover negative admission and exact twelve-entry composition; HTTP integration invokes all Workbench reads plus exact-target Terminal creation | Twelve accepted operations only                                                                            |
| At most one execution while retained                  | Invariants 1-4   | `RetainedRequestHasOneHandlerStart`, `ActiveExecutionCapacity`                                                                | Ledger boundary tests and the fixed-seed stateful command property                                                                                                  | No claim after eviction or restart                                                                         |
| Cancellation and retention are safe                   | Invariants 5-8   | `CommittedExecutionNeverCancels` plus cancellation, count, and weight invariants in both TLC variants                         | Deferred-handler, pre-/post-commit cancellation, duplicate request, receipt eviction, and exact capacity tests                                                      | Handler liveness and rollback after successful durable storage are not claimed                             |
| Agent wait matches canonical state                    | Invariants 3,9   | `AgentSemanticWait.tla`: exact subscription, eligibility, freshness, output, and revision invariants                          | Agent domain and Desktop operation tests cover pagination, exact reads, status explanation, race-safe recheck, timeout, cancellation, and future baselines          | Bounded volatile runtime only; no restart continuity or provider-truth claim                               |
| Agent changes expose every retained revision or reset | Invariants 3,9   | `AgentSemanticChanges.tla`: ordered suffix, batch, subscription, gap, reset, timeout, cancellation, and retirement invariants | Agent journal and Desktop operation tests cover applied-only records, page order, signed cursor, synchronous recheck, overflow/reset, timeout, and cancellation     | Retention is 512 in memory; no durable replay, cryptographic proof, or restart continuity                  |
| Locator and peer stay current-user                    | Invariants 10,12 | Native source is compiled for maintained POSIX and Windows families                                                           | Native Node and Electron tests create, authenticate, read, reject unsafe POSIX ownership, and retire the endpoint; maintained Windows CI exercises DACL/SID paths   | A compromised same-user process is trusted; cross-user negative execution needs a platform account fixture |
| Framing and capacity fail closed                      | Invariants 8,11  | Strict protocol framing and route schemas                                                                                     | Unit rejection matrix plus real Node/Electron local-transport tests cover trailing input, 64 KiB request, 1 MiB response, malformed input, and recovery             | Kernel queue scheduling and process kill are outside the claim                                             |
| Disconnect cannot cancel or replay                    | Invariant 12     | `Disconnect` and `Reconnect` leave ledger variables unchanged                                                                 | Real client timeout proves one delivery; CLI tests recover the same retained request and report unknown after generation change without reinvocation                | Durable recovery across machine or coordinator restart is excluded                                         |

The registered TLC model uses two request identities, two fingerprints, three execution identities,
one active slot, two coordinator generations, and separate count- and weight-limited retention
variants. Those bounds expose replay, conflict, cancellation, completion-order eviction, and
restart interleavings; they do not prove unbounded state. Native tests run under both Node and the
pinned Electron runtime on the current platform, while the maintained macOS and Windows CI matrix
is the authority for platform behavior. No manual native-window test participates.

The Agent-wait model adds two Sessions, two semantic statuses, three health values, two Agent
mutations (revisions 0 through 2), and two coordinator generations. Its action coverage includes begin, missing, ahead,
update, removal, timeout, cancellation, and retirement paths. These bounds expose subscription and
decisive-recheck races without claiming unbounded waiter counts, time, revisions, or restart
recovery.

The Agent-changes model uses revisions 0 through 4, retention 2, page bound 2, and two coordinator
generations. Required action coverage includes baseline, current follow, immediate batch, overflow
reset, invalid cursor, update, timeout, cancellation, and retirement. These bounds expose silent-gap,
ordering, batching, and subscribe-before-recheck errors without claiming unbounded retention,
readers, revisions, cryptographic cursor security, or restart recovery.
