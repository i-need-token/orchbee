# Desktop Multi-Client Session Coordinator

## Problem and scope

One Orchbee application runtime may be observed and controlled by several independent pages. An
Electron renderer page and a Browser page have the same Workbench-client meaning: each owns a
private Client View and terminal viewport state while every page observes one recoverable shared
Workbench document and one set of terminal processes. Electron Main, preload, a future Gateway,
and the Terminal Host are adapters or authorities, not clients.

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
both environments. The current normal browser mode is an in-memory preview; a future Gateway will
provide its shared runtime capability adapter. An Electron `BrowserWindow` loads the same
application with one atomic three-port capability bundle injected by preload. The platform-neutral
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
and awaits disposal of that session's attachment leases. A proposal already validated and accepted
by Main may finish after its initiator disconnects; its durable result is published to remaining
sessions. A queued but unaccepted client request is discarded.

Session retirement never changes the shared document, Binding registry, PTY, process, or terminal
lifecycle. Explicitly closing a Panel or Tab remains a shared product transition and reconciles its
durable close intent.

For a running terminal, the first successfully opened attachment owns the Host's exclusive resize
lease. Later authorized attachments receive read/input without resize and render the canonical grid
with client-local padding, scrolling, or scaling. Concurrent opens are serialized per presentation
and have no Electron/Browser priority. When the owner detaches, existing observers remain attached
without automatic promotion; the next new attachment may acquire the lease. Explicit control
transfer is reserved for the Gateway design.

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
- A terminal-effect failure becomes one matching presentation observation; it does not relabel the
  durable intent or retry without an explicit new operation/restart reconciliation.
- No compatibility reader, alias, or migration is provided for the removed unshipped single-client
  IPC contract. The version-1 shared-document file format and path do not change.

## Correctness claims and evidence

| Claim                                                                         | Deterministic evidence                           | Formal/system evidence                                    | Limits                                                    |
| ----------------------------------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------------- | --------------------------------------------------------- |
| One exact revision chain has one Main writer                                  | Proposal decision and queue tests                | `DesktopMultiClientSession` canonical/revision invariants | Complete-document first-writer-wins policy                |
| A terminal effect follows matching durable intent                             | Intent reconciler traces                         | TLA+ `EffectRequiresPublishedIntent`                      | Host/ledger idempotency trusted                           |
| Equivalent races converge and divergent stale writes preserve canonical state | Two-client and stateful property tests           | TLA+ stale/converged actions                              | No semantic merge                                         |
| Every connected fair session can catch up in publication order                | Session bootstrap/broadcast tests                | TLA+ publication progress                                 | No progress through suspended/broken transport            |
| Page disconnect releases only its attachments                                 | Session disposal and concurrent projection tests | TLA+ attachment/session invariant                         | OS/process crash cleanup delegated to existing boundaries |
| Quiescing rejects non-owner mutation before application drain                 | Exit-owner transition tests                      | TLA+ quiescing invariant                                  | Existing irreversible stop recovery policy                |
| Multiple viewers preserve one resize authority                                | Serialized projection-open tests                 | Existing Terminal Host exclusive-lease model              | No promotion of an already-open observer                  |

The TLA+ configuration checks two clients, two document values, one attachment, both missing and
empty available initialization, equivalent and divergent proposals, reordered disconnect/commit/effect
completion, quiescing and disconnected-owner handoff, publication, and two bounded revisions (`0`
and `1`). `provision` represents
an effect that leaves a terminal alive, `stop` represents terminal-ending effects including close,
and `none` represents a document-only change. Each abstract client identity connects and proposes at
most once; a reconnect is a fresh identity, as required by the production contract. Stateful
property tests use fixed seeds and generated client actions. The real Electron-plus-Browser trace,
Gateway
authentication, network backpressure, and explicit resize-control transfer belong to later slices.
No throughput acceptance gate is added; the terminal frame hot path is unchanged.
