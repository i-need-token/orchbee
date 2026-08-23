# Desktop Multi-Client Session Coordinator

## Problem and scope

One Orchbee application runtime may be observed and controlled by several independent pages. An
Electron renderer page and a Browser page have the same Workbench-client meaning: each owns a
private Client View and terminal viewport state while every page observes one recoverable shared
Workbench document and one set of terminal processes. Electron Main, preload, the local Browser
Gateway, and the Terminal Host are adapters or authorities, not clients.

Desktop Main owns one `DesktopClientSessionCoordinator`. It serializes shared-document proposals,
persists one canonical revision chain, reconciles durable terminal intents once for the application,
and publishes ordered document and terminal-observation events to every live `ClientSession`.
This foundation provides the Electron `MessagePort` adapter and a transport-neutral coordinator
core. It does not open a network listener, authenticate a remote peer, persist Client View, merge
concurrent semantic actions, add an application package dependency, or make application-lifetime
control available to an ordinary client.

Transport-neutral page payloads belong to `@orchbee/workbench-client-protocol`: session bootstrap,
document proposals and publications, projection-control requests, attachment bootstrap, and the
projected-terminal command/event stream. `@orchbee/desktop-client-protocol` owns only Electron's
atomic renderer bootstrap and application-lifetime messages. There is no compatibility alias for
the former unshipped Desktop-prefixed page-client contract.

`apps/web` is the repository's runnable Web product entry and the canonical product renderer for
all client environments. An absent runtime marker selects the in-memory Browser Preview; the local
Gateway marker authenticates an ordinary Browser page as a peer of Electron. An Electron
`BrowserWindow` loads the same application with one atomic three-port capability bundle injected by
preload. The platform-neutral
Workbench client runtime under `apps/web` consumes narrow client-session and projection ports and
owns document coordination, projected-terminal presentation, search, and actions without depending
on Electron, `MessagePort`, or a network transport. Desktop adapters supply the Electron ports and
layer the application-lifetime UI beside that common runtime. Desktop owns no parallel Workspace,
Tab, Panel, terminal UI implementation, or renderer build. Shared implementation belongs to the
owning frontend packages, while runnable entry composition remains under `apps/web` rather than a
new `packages/web` workspace.

## Inputs and boundaries

| Input                            | Trust and absence semantics                                                                                                   |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Stored `WorkbenchSharedDocument` | Untrusted file input parsed by `@orchbee/workbench/document`; missing is a distinct initial state and invalid is unavailable. |
| Client document proposal         | Untrusted capability input containing one request identity, expected revision, and complete strict document.                  |
| Terminal-runtime result          | Trusted only after matching the exact requested presentation and operation identity.                                          |
| Host exit                        | Untrusted process input validated by the Host protocol and resolved through the Main binding registry before publication.     |
| Client disconnect                | An attachment/session lifetime event; it is never terminal or product lifecycle intent.                                       |
| Application exit policy          | A separate Electron-only capability that may place the coordinator in application-wide quiescing.                             |

The coordinator depends on narrow ports for document load/commit, terminal intent execution,
session event publication, identifier allocation, and attachment-session disposal. The deterministic
core has no Electron, filesystem, Host, clock, randomness, network, React, or DOM dependency.

## Data and ownership

```text
DesktopClientSessionCoordinator
├── phase: accepting | quiescing | unavailable | disposed
├── canonical document: missing | WorkbenchSharedDocument
├── event sequence: process-local canonical unsigned decimal
├── proposal queue and at most one active store operation
├── terminal operations keyed by durable operation identity
├── transient TerminalPresentation observations
└── ClientSession[ClientSessionId]
    ├── connection generation and last published sequence
    ├── pending correlated proposal responses
    └── attachment-session disposal capability
```

The surrounding Desktop Main composition owns the cross-boundary cleanup and exit-delivery owners:

```text
Desktop Main lifecycle/projection owners
├── bounded TerminalExited retry queue (identity + newest revision)
├── durable pending-journal/rejection owner (queue-full, association, shutdown, unknown)
├── separate durable publication ledger (identity + newest published revision)
├── pending Host detach set for post-attach failures
├── active projection leases and resize ownership
└── retiring ClientSession records with retryable attachment disposal
```

`ClientSessionId` and event sequence are Main-lifetime identities. They are never stored in the
shared document or recovery files. Client View and terminal viewport state remain page-local.
`TerminalId`, Binding identity, PTY/VT state, history, and process state remain Terminal Host or
Desktop terminal-runtime authority. Main's transient presentation observation is only the
presentation-scoped projection required to hydrate a client.

There is no product count ceiling for sessions, proposals, terminals, or attachments. Allocation,
structured-clone, storage, Host, or memory exhaustion is a typed failure; it never authorizes
truncation, eviction, identity reuse, or termination of another session.

## Document proposal decision

The principal pure decision is:

```ts
decideDesktopClientDocumentProposal(canonical, proposal, phase, clientAuthority)
  -> commit | converged | stale | rejected
```

| Condition                                                                 | Result                              |
| ------------------------------------------------------------------------- | ----------------------------------- |
| Coordinator is unavailable/disposed                                       | `rejected/control-unavailable`      |
| Application is quiescing and the client is not the exit owner             | `rejected/application-quiescing`    |
| Missing canonical, expected revision is null, candidate revision is `0`   | `commit`                            |
| Expected revision equals canonical and candidate is exact-next            | `commit`                            |
| Expected revision is stale but candidate content equals canonical content | `converged`                         |
| Expected revision is stale and candidate differs                          | `stale` with the canonical document |
| Candidate repeats/skips revision or violates the accepted exit transition | `rejected`                          |

Proposals enter one FIFO Main queue. The acceptance order, not transport, platform, focus, or wall
clock, determines the winner. No generic merge or automatic action replay is attempted. A complete
document does not carry enough semantic intent to safely merge Panel/Split hierarchy and terminal
removal lifecycles. The losing client applies the returned canonical document, reconciles its local
Client View, discards unaccepted shared effects, and remains connected.

One accepted proposal advances through its matching durable-effect publication before the queue
accepts another commit that could supersede that intent. Stale, malformed, and rejected proposals
may settle without waiting because they cannot change canonical state. This ordering prevents a
later document-only revision from eclipsing an unpublished terminal effect.

## Durable intent reconciliation

Clients create presentation and operation identities through Workbench but never invoke process
effects directly. A committed document is published before Main derives and executes its terminal
commands:

```text
validate proposal
  -> persist exact-next document
  -> publish canonical document
  -> reconcile provision / close / stop intent
  -> publish matching terminal result
```

Within one Main lifetime, one exact durable operation is dispatched at most once. An equivalent
duplicate joins the existing operation or receives its cached result. Reusing an operation identity
with different content retires only the offending ClientSession. Main restart may replay a still
durable operation through the existing idempotent binding and CreationId rules; cross-crash
exactly-once execution is not claimed.

`retain` has no durable operation identity. Main performs one restore-only reconciliation per
presentation after recovery, allocates a transient operation identity, and publishes the resulting
observation. Retain never creates a replacement terminal. Current operation results remain cached
only while their matching durable intent is current; removed or superseded intents retire the
cache without retaining an unbounded tombstone set.

## Publication and catch-up

Every published change receives one process-local sequence. Publication order is:

1. canonical document;
2. reconciliation-started observation when required;
3. matching lifecycle result;
4. a later canonical cleanup document produced by a client applying that result.

Joining a session is serialized with publication. It receives one atomic bootstrap containing the
current document state, common initialization seed when the document is missing, current terminal
observations, and the sequence after that snapshot. Live events begin strictly after that sequence,
so there is no join gap. A malformed event, wrong identity, transferred port, or sequence gap
retires the affected session. Reconnect creates a fresh session and full snapshot; session identity
and Client View are not resumed.

Document and lifecycle traffic is low-rate control data. Terminal frames, input, history, search,
selection, and copy continue on attachment-scoped ports and never pass through the coordinator.
Network transport backpressure and resynchronization are Gateway responsibilities.

## Disconnect and projection policy

Closing either required session-control capability retires the page session, stops later requests,
and awaits disposal of that session's attachment leases. A failed disposal leaves the session in
Main's retiring set and is retried by a later retirement or coordinator disposal; it is not converted
to a completed release. A proposal already validated and accepted
by Main may finish after its initiator disconnects; its durable result is published to remaining
sessions. A queued but unaccepted client request is discarded.

Session retirement never changes the shared document, Binding registry, PTY, process, or terminal
lifecycle. Explicitly closing a Panel or Tab remains a shared product transition and reconciles its
durable close intent.

For a running terminal, the first successfully opened writable attachment receives the Host's
initial exclusive resize lease. Later writable attachments receive read/input/resize capability but
remain resize observers. An admitted focus-in, key press/repeat, committed text or paste,
application mouse press, or terminal-selection start makes that attachment the latest desired
owner; attachment, background state, viewport measurement, pointer motion, release/repair input,
and focus-out do not. Desktop coalesces concurrent intent per presentation and serializes one
explicit `ClaimResizeLease` at a time, with no fixed Electron/Browser priority. Host publishes the
old owner's revocation before the new owner's grant, after which the promoted surface immediately
submits its measured geometry. Owner detach promotes the remaining attachment with the newest
successful claim, with earliest admission breaking ties among never-granted observers; Desktop
reclaims for any newer remaining activity it recorded.

### Host-connection attachment cleanup

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Terminal Host disconnect already destroys every attachment owned by that connection, but retrying `DetachTerminal` through the ended Desktop transport cannot succeed and can retain a local lease or pending-detach owner indefinitely.                                                                                                                                                                                                             |
| Inputs       | Request-transport state, active and provisional attachment ownership, local release or retirement, matching or mismatched detach acknowledgement, detach failure, and connection end.                                                                                                                                                                                                                                                                |
| Data         | `ProjectedTerminalBroker` cleanup state plus the projection service's active-lease, provisional-attachment, in-flight-detach, and pending-detach ownership.                                                                                                                                                                                                                                                                                          |
| Outputs      | A live surface receives `host-connection-unavailable` before its broker requests cleanup; every affected local owner then settles through either a matching detach acknowledgement or confirmed connection end.                                                                                                                                                                                                                                      |
| Invariants   | While the request transport remains available, only a matching `DetachTerminal` acknowledgement clears an attachment owner. After it ends, Desktop emits no further Host request for that connection and clears each connection-scoped local owner exactly once. Neither path changes the shared document, Binding registry, PTY, process, or retained terminal. Duplicate and reordered close, release, retirement, and disposal remain idempotent. |
| Decisions    | Confirmed connection end is cleanup evidence equivalent to per-attachment detach only for attachments scoped to that ended connection. A detach failure while the transport remains available retains its exact owner for retry.                                                                                                                                                                                                                     |
| Model        | `ProjectedTerminalIpcReliability` extends the release-race state machine with request-transport end and connection-cleanup transitions; it assumes Terminal Host's connection teardown but does not assume eventual reconnect.                                                                                                                                                                                                                       |
| Transitions  | Matching detach acknowledgement removes one owner; available-transport failure retains it; connection end seals further requests and settles all owners from that connection; later duplicate cleanup is a no-op.                                                                                                                                                                                                                                    |
| Dependencies | Terminal Host connection teardown remains authoritative for remote attachment removal. The terminal protocol and Host terminal lifecycle do not change.                                                                                                                                                                                                                                                                                              |
| Ownership    | Terminal Host owns remote connection teardown, the Desktop projection service owns lease and pending-detach convergence, and each broker owns its renderer failure and port retirement.                                                                                                                                                                                                                                                              |
| Verification | TLA+ action coverage and invariants check detach success, available-transport failure, disconnect-before-detach, disconnect-during-detach, retry-after-failure, and duplicate cleanup. Deterministic HostConnection, broker, and projection-service tests cover the implementation boundary; the composed-runtime and application suites cover integration and disposal.                                                                             |
| Correctness  | Safety evidence is bounded to the registered finite model. Progress requires delivery of an already-observed connection-end continuation; no network recovery or latency bound is assumed.                                                                                                                                                                                                                                                           |
| Exclusions   | Live Host reconnect, startup deadlines, Windows server authentication, lifecycle-operation replay, and protocol changes are separate decisions.                                                                                                                                                                                                                                                                                                      |

## Application lifetime

Application lifetime is an Electron-only capability layered beside ordinary ClientSession control.
Before preserve or stop preparation, Main enters quiescing. Already accepted work may finish. New
ordinary proposals from every other session are rejected. Preserve admits no new document change;
stop admits only the exit owner's structure-preserving `stop` to `stopped` progression. This keeps
another page from creating or structurally changing terminals while the application drains.

Lifetime choice and shared-document proposals use independent ports, so transport arrival order is
not authority order. The renderer waits for a correlated Main mode-accepted event before it flushes
preserve state or generates stop intent. Main emits that event only after the coordinator has
entered quiescing for the exact ClientSession. If the exit owner disconnects, a fresh session may
adopt only the same quiescing mode; an existing connected session or a different mode cannot steal
the workflow.

Cancel before choosing a mode does not quiesce. Once stop effects begin, failure remains in the
existing irreversible recovery workflow; quiescing is not rolled back into an apparently normal
application. An ordinary page close is never application quit.

## Failure policy

- Invalid stored state makes the coordinator unavailable and is never overwritten or repaired.
- Store failure freezes new shared changes and terminal effects but preserves the last canonical
  document, existing Host processes, and already-open projections.
- One malformed, stale, disconnected, or blocked client never retires another client.
- A publication failure retires only its target session.
- A terminal-exit publication that reaches some sessions and fails on another retains one sequence
  and retries only sessions that have not accepted that exact event; successful sessions are not
  replayed and a retry does not allocate a sequence gap.
- An exit durability or association failure is a typed retryable outcome. Before the Host listener
  returns, Desktop Main synchronously admits every accepted `TerminalExited` to the owner-only durable
  pending journal. The bounded 64-entry in-memory retry queue is only a scheduler/cache; it is not the
  crash-recovery owner, and the current Host event envelope still has no acknowledgement/redelivery
  field. The pending journal, publication ledger, and durable rejection journal have independent
  capacities. Queue-full and shutdown entries can be promoted for retry; an association-pending or
  unknown identity is parked until the exact identity is observed and then promoted. Unknown entries
  do not receive an unbounded timer retry loop. Pending ownership is removed only after the runtime
  result, adapter commit, publication-ledger admission, reconciler visibility, and live publication
  have succeeded. Pending-journal full/write failure records a durable rejection when possible and
  otherwise fails closed; it is never represented only by a trace message. A bootstrap reads only the
  reconciler's committed visible observations, not a pending reservation or an uncommitted ledger
  attempt.
- The Workbench terminal-presentation publication is Main's only lifecycle publication. An attachment's
  `ProjectedTerminalBroker` may forward a separate identity-checked raw `terminal-exited` or
  `terminal-failed` transport notification with monotonic per-attachment revision filtering. That
  notification is not a coordinator sequence event and is not an exactly-once claim across the two
  channels. Main records `terminal-failed` as the retained presentation outcome before broadcasting
  it; a failed session is read-only and remains explicitly closable. Failure recovery comes from the
  surviving Host's inventory rather than the exit-specific spill and publication ledgers.
- Projection cleanup failures are retained by the service-level lease/detach owner. Renderer port-close,
  malformed-control, postMessage, session-retirement, and post-attach factory/start failures converge
  through later release, retirement, or dispose retry while the Host request transport remains
  available; only a matching detach acknowledgement clears the request on that path. Once connection
  end is confirmed, Host connection teardown is cleanup evidence for that connection's attachments:
  Desktop sends no further Host request and settles the connection-scoped local owners.
- No compatibility reader, alias, or migration is provided for the removed unshipped single-client
  IPC contract. The version-1 shared-document file format and path do not change.

## Correctness claims and evidence

| Claim                                                                                  | Deterministic evidence                                    | Formal/system evidence                                    | Limits                                                    |
| -------------------------------------------------------------------------------------- | --------------------------------------------------------- | --------------------------------------------------------- | --------------------------------------------------------- |
| One exact revision chain has one Main writer                                           | Proposal decision and queue tests                         | `DesktopMultiClientSession` canonical/revision invariants | Complete-document first-writer-wins policy                |
| A terminal effect follows matching durable intent                                      | Intent reconciler traces                                  | TLA+ `EffectRequiresPublishedIntent`                      | Host/ledger idempotency trusted                           |
| Equivalent races converge and divergent stale writes preserve canonical state          | Two-client and stateful property tests                    | TLA+ stale/converged actions                              | No semantic merge                                         |
| Every connected fair session can catch up in publication order                         | Session bootstrap/broadcast tests                         | TLA+ publication progress                                 | No progress through suspended/broken transport            |
| Page disconnect releases only its attachments                                          | Session disposal and concurrent projection tests          | TLA+ attachment/session invariant                         | OS/process crash cleanup delegated to existing boundaries |
| Quiescing rejects non-owner mutation before application drain                          | Exit-owner transition tests                               | TLA+ quiescing invariant                                  | Existing irreversible stop recovery policy                |
| Multiple viewers preserve one resize authority; an eligible active viewer may claim it | Lease handoff, eligibility, and claim-serialization tests | Desktop and Terminal Host resize-lease models             | No implicit priority by client type                       |
| Released projection requests never become active                                       | Client, Gateway, and Main release-race tests              | `ProjectedTerminalIpcReliability` release-race invariants | One ordered live control connection                       |
| A Host attach response installs its consumer before later inbound delivery             | Host response/frame/close ordering tests                  | TLA+ response-barrier safety and progress                 | JavaScript microtask-before-`setImmediate` refinement     |
| An opening frame is acknowledged only after renderer readiness admits its draw         | Slow-start frame/transport tests                          | TLA+ ready-frame acknowledgement invariant                | Client commit acknowledgement, not completed paint        |
| Recovery replaces an honest frozen frame only with a painted fresh attachment          | Projected-surface recovery tests                          | TLA+ recovery and operation-identity invariants           | Two attempts; transport progress is not assumed           |

The TLA+ configuration checks two clients, two document values, one attachment, both missing and
empty available initialization, equivalent and divergent proposals, reordered disconnect/commit/effect
completion, quiescing and disconnected-owner handoff, publication, and two bounded revisions (`0`
and `1`). `provision` represents
an effect that leaves a terminal alive, `stop` represents terminal-ending effects including close,
and `none` represents a document-only change. Each abstract client identity connects and proposes at
most once; a reconnect is a fresh identity, as required by the production contract. Stateful
property tests use fixed seeds and generated client actions. The real
Electron-plus-two-Browser trace covers Gateway authentication, independent page sessions, shared
canonical state, attachment disconnect/reconnect, Browser effects, and exact attachment
backpressure recovery. Explicit resize-control transfer remains outside this contract. No
throughput acceptance gate is added; the terminal frame hot path is unchanged.

`ProjectedTerminalIpcReliability` checks three orthogonal configurations rather than multiplying
their state spaces. The release-race configuration permits client release before service attach,
after attach, or after an open event has already been posted; cleanup may fail, and the failed Main
request and lease remain owned until retirement succeeds. A late open event fails the client
control capability closed and never activates its transferred attachment. The attach-order
configuration queues an attach response, optional first frame, and close in Host wire order. The
response barrier must let the promise continuation install the broker and attachment listeners
before either later item is delivered. Renderer initialization and first-frame delivery may occur
in either order, but `FrameApplied` requires both the delivered commit and renderer readiness.
Weak fairness applies only to already-queued, in-process continuation and Host delivery actions;
renderer initialization remains an environment action and has no assumed deadline.

The recovery-operation configuration checks two fresh attachment identities, candidate loss both
before paint and after a frame is ready but before promotion, frozen-frame retention, promotion only
while the candidate remains live, and budget exhaustion. A framed candidate that becomes unavailable
is sealed and cannot replace the frozen projection.
Selection and Find are distinct atomic invocation identities that may occur in either order and at
any point between Host handoff, Host completion, client delivery, transport loss, and retry. Opening
Find snapshots only the latest Host selection already delivered to that attachment. A still-pending
selection therefore produces an unseeded Find operation instead of consulting a timeout or being
silently rebound later. An operation handed to a disconnected attachment has an unknown outcome,
seals that attachment, and is never copied to the retry identity; a user may explicitly create a
new invocation after recovery, but that is not replay of the old identity.

This cross-boundary model assumes only ordered delivery within one still-live MessagePort, stream,
or WebSocket connection. It does not assume a latency bound, eventual reconnect, delivery after
disconnect, exactly-once execution across a lost connection, browser scheduling time, Terminal Host
internals, or a refinement proof from every transport implementation. The existing deterministic
client, Gateway, broker, service, HostConnection, and recovery-controller tests own that refinement.
Passing TLC is bounded safety evidence plus in-process response-barrier progress, not a claim that a
failed network eventually recovers.

### Clock boundaries and failure detection

Cross-boundary correctness is derived from connection-local ordering, correlation identities,
attachment identities, generations, revisions, and explicit terminal outcomes. Elapsed time never
decides whether one IPC operation happened before another, whether an old result belongs to a new
attachment, or whether an unknown operation may be replayed.

- The 10-second Host request deadline is a failure detector after transport handoff. Expiry retires
  the whole Host connection, reports every still-pending outcome as unknown, and permits no automatic
  replay; it does not manufacture a failed response for just the slow correlation.
- The terminal search watchdog retires only the current client-side scan. Its search identity is
  advanced, the row read is aborted, and the Worker is terminated, so late progress cannot publish
  into the replacement search. It does not infer anything about Host mutation order.
- The 250/500/1,000-millisecond projection recovery delays schedule fresh request and attachment
  identities. They neither extend the authority of the disconnected attachment nor replay selection,
  input, clipboard, link, or search operations from it.
- Font/GPU initialization has no deadline. A real transport-unavailable event races startup and
  revokes the candidate, while an initialized candidate is rechecked immediately before promotion.
  Bell, completion-status, scrollbar, and animation timers affect presentation only.
- Native browser input settlement is a separate local adapter heuristic owned by
  `packages/terminal/formal/tla/TerminalNativeInputSettlement.tla`; its model treats time as event
  ordering and makes no exact browser-scheduling claim.

The Gateway control and attachment WebSocket bootstraps currently have no connection deadline. A
socket that neither authenticates nor emits `close`/`error` can therefore leave the initial surface
in `connecting` indefinitely; bounded automatic recovery begins only after an explicit unavailable
outcome. Choosing a deadline, user-cancellation behavior, and whether that deadline retires one
attachment or the whole control capability is a product/protocol decision, not part of the present
safety refinement.

Terminal recovery has two independently checked TLA+ state machines so one state space cannot hide
the other's actions. `TerminalRecoveryBinding` owns Main acquisition/release, same- and cross-Host
epoch creation recovery, renderer-document generations, attachment replacement, final-revision
durability, and exit publication. `TerminalExitRecovery` owns bounded volatile admission, durable
pending ownership, explicit full-journal rejection, unknown-identity observation/rejection,
publication reservation and adapter commit, bounded live-publication failure, deduplication, and
visibility recovery across Main release/reacquisition. Its merge configuration checks three exit
identities with a volatile limit of `1` and pending-journal limit of `2`, so volatile, durable-only,
and rejected admission branches all have reachable witnesses.

Both models are owner-retention abstractions narrower than the production coordinator. They do not
model session retirement, partial session delivery, sequence allocation, renderer transport, or
filesystem atomicity. Required action coverage proves only that the declared finite configurations
exercise each maintained semantic branch; passing TLC remains bounded model evidence rather than a
proof of the production implementation.
