# Semantic Command Execution

## Boundary

Desktop owns one process-lifetime semantic-command coordinator generation, an explicit operation
catalog, a bounded in-memory execution ledger, and the private local endpoint that exposes that
service to an authenticated current-user client. The production catalog contains exactly thirteen
operations: `agent.session.list`, `agent.session.get`, `agent.session.explain`,
`agent.session.wait`, and `agent.session.changes`, plus `workbench.project.list`, `workbench.directory.list`,
`workbench.workspace.list`, `workbench.tab.list`, `workbench.panel.list`,
`workbench.terminal.list`, `workbench.terminal.output.read`, and `workbench.terminal.create`. It does
not contain an arbitrary Action, filesystem, shell, renderer, or Terminal Host escape hatch.

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
| Catalog                | Immutable `DesktopSemanticCommandCatalog`; only five Agent queries and eight Workbench entries exist          |
| Coordinator generation | Created once with the service, copied into the locator, and present in every public snapshot                  |
| Endpoint identity      | Random process-local identifier used only for the locator and socket or named-pipe name                       |
| Locator                | Native adapter publishes one private bounded file after listener creation and removes it during teardown      |
| Request receipt        | Ledger-owned request identity, canonical SHA-256 operation fingerprint, and execution identity while retained |
| Active execution       | Ledger-owned identity, commit marker, and cancellation-forward count; the service owns one `AbortController`  |
| Terminal execution     | Ledger-owned safe result and measured byte weight until oldest-first eviction                                 |
| HTTP response          | One strict JSON response no larger than 1 MiB; framing errors are payload-free and never serialize exceptions |
| Observation output     | Retained snapshot or an exact request/execution receipt-unavailable result                                    |
| Terminal text output   | One exact-frame plain-text tail of at most 1,000 rows and 128 KiB, retained only through the bounded ledger   |

The ledger stores no canonical input. An asynchronous handler closure may hold canonical input only
while its execution is active. Diagnostics receive no operation input or result payload. A query
result may explicitly contain the requested names and paths in its bounded response and receipt.

`workbench.directory.list` alone uses operation schema version `2` and output schema version `2`;
each result includes the Directory placement. The other seven Workbench operations remain version
`1`. A remote Directory result contains only its committed opaque Environment identity and
canonical path. The requested alias, failed observations, and raw operating-system diagnostics
never enter semantic output.

Tab and Panel lists project only canonical shared-document membership. Tab order is the shared Tab
collection order; exact-target Panel order is depth-first and left-to-right. Neither query reads or
returns Client View active, selected, focused, maximized, active-child, split-weight, or geometry
state. Signed cursors bind to the exact location, and Panel cursors additionally bind to the exact
Tab identity.

`workbench.terminal.output.read` pins one exact canonical Terminal presentation identity and opens
only a capture read attachment. It reads the current tail from a live local Terminal, an
already-connected SSH Terminal, or a retained local archive without starting or reconnecting a
runtime. The default is 80 rows, the owner limit is 1,000 rows, and the public text limit is 128 KiB.
The reader requests plain text against one exact Host generation and revision, joins soft-wrapped
rows, and exposes no ANSI styling, graphics, hyperlinks, raw VT bytes, or future output. Input,
paste, key, resize, focus, close, and stop authority remain absent. A changed frame, retired target,
unavailable projection, cancellation, cleanup failure, or expiration of the five-second deadline
for lease acquisition and frame/row collection selects an explicit result without old text or hidden
retry. Once a lease is acquired, the reader awaits one cleanup attempt before publication. Successful
cleanup precedes every ordinary result; a rejected disposal publishes `cleanup-failed`. Cleanup has
no separate deadline, so it may delay publication beyond five seconds. Text is not automatically
redacted and may contain secrets; it enters no diagnostic, telemetry, or durable history, but the
existing bounded volatile execution ledger can retain the safe result until eviction or restart.

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
   fingerprint. An execution identity is admitted at most once during the process lifetime, even
   after its receipt is evicted.
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
9. A successful Terminal output read names the requested live target and one exact Host generation
   and revision, returns only bounded plain text, and settles only after its capture lease is
   disposed. Every acquired read attempts cleanup before publication; rejected disposal publishes
   `cleanup-failed`. Every stale, retired, unavailable, cancelled, or cleanup-failed outcome contains
   no Terminal text.
10. Discovery and description expose exactly the thirteen composed operations. Catalog registration or
    UI Action availability alone grants no process authority.
11. The native adapter follows neither a POSIX symlink nor a Windows reparse point and accepts only
    a current-user private runtime directory, locator, endpoint, and mutually authenticated peer.
    Windows creation supplies the current process `TokenUser` as owner and a protected private
    DACL; it does not depend on the token's default owner. An existing runtime directory is opened
    without following reparse points and checked for current-user ownership before its DACL is
    tightened through that same handle. Existing same-user broad or null DACLs remain repairable;
    foreign ownership or a reparse point is rejected without changing ownership or permissions.
12. One connection carries one bounded request and one bounded response. Unknown framing, method,
    path, content type, trailing bytes, and capacity fail closed.
13. Locator publication follows listener creation. Shutdown retires the locator and listener before
    disposing the service. A client disconnect has no service transition and causes no replay.

## Transitions

| State            | Event                       | Guard                                                                                             | Result                                                                              |
| ---------------- | --------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Endpoint absent  | Start                       | Trusted runtime directory and native listener validate                                            | Create listener, publish matching locator, then report ready                        |
| Listening        | Accept                      | Peer identity matches the current effective OS user                                               | Read exactly one bounded request                                                    |
| Listening        | Reject peer                 | Peer identity is absent or mismatched                                                             | Close that connection; allocate no execution                                        |
| Connected        | Reject request              | Framing, route, content type, timeout, or capacity guard fails                                    | Return one bounded local error when possible; service and ledger unchanged          |
| Absent request   | Admit                       | Catalog entry is visible, version/input/authorization/confirmation pass, active capacity remains  | Allocate one execution and start one handler                                        |
| Retained request | Repeat or conflict          | Fingerprint matches or differs                                                                    | Return existing snapshot, or conflict without a second handler                      |
| Active execution | Cancel                      | Execution is uncommitted and no cancellation was forwarded                                        | Forward one cooperative abort                                                       |
| Active execution | Mark committed              | The canonical Workbench revision completed durable storage                                        | Record commit before post-commit reconciliation; reject later cancellation          |
| Terminal read    | Observe exact frame         | Exact target opens a capture-only lease and generation/revision remain current                    | Read a bounded plain-text tail, attempt lease cleanup, then publish                 |
| Terminal read    | Abort or invalidate         | Cancellation, deadline, stale frame, target retirement, unavailability, or cleanup failure occurs | Select a text-free result; disposal rejection replaces it with `cleanup-failed`     |
| Active execution | Settle                      | Settlement and commit-marker consistency guards hold                                              | Retain one validated terminal result                                                |
| Connected        | Client disconnects          | Request may already have reached the service                                                      | Drop only the response transport; do not cancel or replay accepted execution        |
| Any              | Process disposal or restart | Coordinator lifetime ends                                                                         | Retire locator/listener, abort active signals, and discard the in-memory generation |

## Dependencies and ownership

`DesktopSemanticCommandService` orchestrates catalog admission, fingerprinting, handler execution,
safe-result capacity, and cancellation. `DesktopSemanticCommandExecutionLedger` is the sole mutable
receipt writer and has no network, filesystem, clock, Workbench, Electron, or process dependency.
`createDesktopWorkbenchReadOperations` adapts the canonical shared document through the Workbench
owner's deterministic projections and binds page cursors to operation, target, generation, and
revision. `createDesktopWorkbenchTerminalCreateOperation` validates the Workbench-owned strict
input, applies the owner operation to an isolated hydrated runtime, commits its projected next
revision only through `DesktopClientSessionCoordinator`, and marks the execution committed in the
coordinator's immediate post-storage callback. The coordinator contains callback failure so it
cannot suppress canonical publication or reconciliation; the service reasserts the marker from the
handler's committed settlement before validation. The isolated runtime never dispatches a provision
command; normal canonical reconciliation starts the PTY only after persistence succeeds.

`createDesktopTerminalOutputReadOperation` delegates to `DesktopTerminalOutputReader`, which opens
an exact presentation through `DesktopTerminalProjectionRouter`. Local live and archived targets
come from the Desktop Terminal runtime; connected SSH targets come from the existing environment
runtime owner without preparation or reconnection. The reader owns the five-second deadline,
which spans lease acquisition and frame/row collection, plus exact-frame row paging, public text
projection, cancellation, and lease cleanup. The deadline is cleared before the reader awaits
cleanup. Workbench owns the strict operation input and output contracts; Desktop owns routing and
execution, while Terminal Host remains authoritative for captured screen rows.

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

Windows accept uses one original timeout for connection and identity establishment. The native
server reads one byte to establish the pipe client's Windows security context, keeps it private
until exact current-user SID validation and successful `RevertToSelf`, and then returns it exactly
once before later stream bytes. No request parsing or service effect precedes authentication.
Accept timeout, peer refusal, disconnect, and the next accept discard any buffered byte; a terminal
stream failure also clears it. An authenticated read polling timeout remains retryable on the same
connection under the Worker's existing request deadline and never replays an already returned byte.

## Correctness evidence

| Claim                                                 | Contract          | Formal or generated evidence                                                                                                                                                         | Deterministic or real refinement                                                                                                                                      | Limits                                                                                                                       |
| ----------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Explicit authorized catalog only                      | Invariants 3,10   | N/A: the execution-ledger abstraction begins after catalog and service admission                                                                                                     | Catalog/service tests cover negative admission and exact thirteen-entry composition; HTTP integration invokes all Workbench reads plus exact-target Terminal creation | Thirteen accepted operations only                                                                                            |
| Request/execution mappings and identities stay unique | Invariants 1-4    | `RequestAndExecutionMappingsAgree`, `RetainedRequestHasOneHandlerStart`, `ExecutionIdentityIsProcessUnique`, and `ActiveExecutionCapacity`                                           | Ledger boundary tests, duplicate execution identity after eviction, and the fixed-seed stateful command property                                                      | Request replay ends at eviction; execution identity uniqueness ends at process restart                                       |
| Cancellation and retention are safe                   | Invariants 5-8    | `CancellationForwardedAtMostOnce`, `CancelCommittedDoesNotForward`, `CommittedExecutionNeverCancels`, and count/weight retention invariants                                          | Deferred-handler, pre-/post-commit cancellation, duplicate request, receipt eviction, and exact capacity tests                                                        | Handler liveness and rollback after successful durable storage are not claimed                                               |
| Agent wait matches canonical state                    | Invariants 3,10   | `AgentSemanticWait.tla`: exact subscription, eligibility, freshness, output, and revision invariants                                                                                 | Agent domain and Desktop operation tests cover pagination, exact reads, status explanation, race-safe recheck, timeout, cancellation, and future baselines            | Bounded volatile runtime only; no restart continuity or provider-truth claim                                                 |
| Agent changes expose every retained revision or reset | Invariants 3,10   | `AgentSemanticChanges.tla`: ordered suffix, batch, subscription, `WaitingCursorIsCurrent`, gap, reset, timeout, cancellation, and retirement invariants                              | Agent journal and Desktop operation tests cover applied-only records, page order, signed cursor, synchronous recheck, overflow/reset, timeout, and cancellation       | Retention is 512 in memory; no durable replay, cryptographic proof, or restart continuity                                    |
| Terminal output is exact, bounded, and capture-only   | Invariants 3,9-10 | `TerminalSemanticRead.tla`: capture-only lease, exact success fence, text-free failures, cleanup-attempt/disposal distinction, cleanup-failure preservation, and one terminal result | Reader and operation tests cover live and archived routing, HTTP invocation, UTF-8 capacity, wrapping, staleness, retirement, deadline, cancellation, and cleanup     | Safety only: no per-read settlement progress, cleanup deadline, retry, redaction, raw VT, wait, remote reconnect, or history |
| Locator and peer stay current-user                    | Invariants 11,13  | Native source is compiled for maintained POSIX and Windows families                                                                                                                  | Native Node and Electron tests create, authenticate, read, reject unsafe POSIX ownership, and retire the endpoint; maintained Windows CI exercises DACL/SID paths     | A compromised same-user process is trusted; cross-user negative execution needs a platform account fixture                   |
| Framing and capacity fail closed                      | Invariants 8,12   | Strict protocol framing and route schemas                                                                                                                                            | Unit rejection matrix plus real Node/Electron local-transport tests cover trailing input, 64 KiB request, 1 MiB response, malformed input, and recovery               | Kernel queue scheduling and process kill are outside the claim                                                               |
| Disconnect cannot cancel or replay                    | Invariant 13      | N/A: encoded transport and client connection state are outside the execution-ledger abstraction                                                                                      | `verify-semantic-command-local-http-runtime.mjs` disconnect proof; `SemanticCommandClient` read-only polling and generation-mismatch tests                            | Durable recovery across machine or coordinator restart is excluded                                                           |

The registered TLC model uses two request identities, two fingerprints, three execution identities,
one active slot, two coordinator generations, and separate count- and weight-limited retention
variants. Those bounds expose replay, conflict, cancellation, completion-order eviction, and
restart interleavings, including post-commit cancellation and execution-identity reuse after
eviction; they do not prove unbounded state. Native tests run under both Node and the pinned
Electron runtime on the current platform, while the maintained macOS and Windows CI matrix is the
authority for platform behavior. No manual native-window test participates.

`verify-semantic-command-endpoint-windows-security.mjs` compiles the production Windows adapter
with warnings as errors. Its finite case matrix covers creation, existing private/broad/null DACLs,
foreign ownership, and reparse refusal. Real NTFS operations verify current-user directory and
locator creation, DACL repair, and rejection of a real directory junction without changing its
target. API-boundary fixtures separately exercise a default owner distinct from `TokenUser` and a
foreign-owner security query; they verify explicit creation ownership and refusal without a
security write. Those fixtures do not change tokens or accounts and do not claim actual elevated
or cross-user execution. Such execution still requires an appropriate Windows account fixture.

The same verifier runs `tests/windows_accept.c` against real named pipes. Its controlled
connect-before-first-write cases cover default client flags and the bundled client's identification
flags, exact binary prefix/tail delivery, delayed fragments after an authenticated read polling
timeout, peer EOF, disconnect, and successor reuse. Clock fixtures cover remaining accept budget,
expiration before reading or publishing admission, and the existing infinite-timeout sentinel;
identity failure fixtures cover missing/mismatched tokens and failed impersonation. The byte is
unobservable on failed admission, and the accept thread returns without impersonation. These finite
cuts refine invariants 11-13; they do not add trusted identities or replace real cross-user testing.

The Agent-wait model adds two Sessions, two semantic statuses, three health values, two Agent
mutations (revisions 0 through 2), and two coordinator generations. Its action coverage includes begin, missing, ahead,
update, removal, timeout, cancellation, and retirement paths. These bounds expose subscription and
decisive-recheck races without claiming unbounded waiter counts, time, revisions, or restart
recovery.

The Agent-changes model uses revisions 0 through 4, retention 2, page bound 2, and two coordinator
generations. Required action coverage includes baseline, current follow, immediate batch, overflow
reset, invalid cursor, update, timeout, cancellation, and retirement. These bounds expose silent-gap,
ordering, batching, and subscribe-before-recheck errors; `WaitingCursorIsCurrent` independently
rejects a waiting reader left behind a newer revision. The model does not claim unbounded retention,
readers, revisions, cryptographic cursor security, or restart recovery.

The Terminal-read model uses one target, two target epochs, two frame generations and revisions,
and one capture lease. Required action coverage includes opening, unavailable and cleanup-failed
open, deadline expiry, frame observation, exact row match, stale rows, cancellation, target
retirement, successful and rejected cleanup, and result publication. These bounds expose
interactive-lease leakage, stale-success, text-bearing or overwritten cleanup failure,
settlement-before-cleanup-attempt, and duplicate-result errors. The model is safety-only: it has no
fairness or temporal settlement property and does not claim bounded cleanup, unbounded rows, time,
targets, Host behavior, or process recovery.
