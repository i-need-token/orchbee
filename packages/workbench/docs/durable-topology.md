# Durable Workbench Topology

## Scope

`@orchbee/workbench/topology` owns the canonical version-1 representation of recoverable Workbench
product topology. It validates and serializes no storage, IPC, process, network, React, or DOM
effect. Applications own those adapters and must parse untrusted documents before constructing a
runtime.

## Model

```text
WorkbenchTopologyDocument
├── version: 1
├── revision: canonical unsigned decimal text
├── projectWorkspace
│   ├── active location, identity sequences, disclosure, and ordered Projects
│   └── each Workspace references exactly one Terminal Context
└── terminalContexts[] (canonical identity order)
    ├── Tab collection: order, groups, active Tab, and allocation sequence
    └── Panel workspace
        ├── ordered per-Tab Panel/Split trees, focus, maximization, and weights
        ├── terminal presentations: identity, title, launch CWD, and durable intent
        └── pending removals and allocation sequences
```

Presentation intent has exactly three forms:

| Intent      | Durable meaning                                                                   |
| ----------- | --------------------------------------------------------------------------------- |
| `provision` | The presentation is placed and its named operation still requires reconciliation. |
| `retain`    | A technical binding is expected; absence must not create a replacement shell.     |
| `close`     | The named operation and removal remain pending until lifecycle reconciliation.    |

Projection maps provisioning failures back to `provision`; restoring, reconciling, restore-failed,
running, or exited observations to `retain`; and close failures or incomplete close results to
`close`. Hydration maps `retain` to the non-projecting `restoring` lifecycle and maps the other
intents to their corresponding pending lifecycle without dispatching a command. A runtime adapter
must reconcile each intent before reporting an authoritative process observation. Restore uses a
distinct command so an application can require an existing technical binding instead of treating
absence as permission to provision a replacement.

## Persisted and excluded data

| Persisted                                                                       | Excluded                                                                                   |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Project, Directory, Workspace, and Terminal Context identity and membership     | `TerminalId`, `BindingId`, `CreationId`, `HostEpoch`, and `AttachmentId`                   |
| Tab and Group identity, order, title, pinning, collapse, and active selection   | Multi-selection anchors, drag state, loading, alerts, memory, network, and crash telemetry |
| Panel/Split identity, hierarchy, weights, focus, maximization, and active child | DOM geometry, animation, pointer state, GPU resources, and renderer caches                 |
| `TerminalPresentationId`, title, launch CWD, and durable lifecycle intent       | Current CWD, PTY bytes, terminal output, history, selection text, and environment          |

The model imposes no product count ceiling on Projects, Workspaces, Tabs, Panels, terminal
presentations, or pending operations. Allocation and application storage failures remain typed
adapter outcomes; they never authorize truncation, eviction, or identity reuse.

## Invariants

- Project, Workspace, Terminal Context, Tab, Group, Panel, Split, Panel-slot, removal, and
  `TerminalPresentationId` identities are unique in their declared scope.
- The Terminal Context collection is sorted by identity and equals the exact set referenced by the
  standalone context and all Workspaces.
- Project and Workspace Directory, primary-Directory, launch-configuration, active-location, and
  disclosure references satisfy the Project/Workspace model invariants.
- Tab order and Panel-workspace Tab order are identical; the active Tab resolves exactly when the
  collection is non-empty; pinned Tabs form one prefix; each Group is contiguous.
- Every Panel tree has valid focus and maximization references. Every Split has at least two
  children, unique slot identity, non-negative finite weights totaling `100`, and no child Split
  repeating its orientation.
- Every active child resolves in its Panel, and every terminal presentation appears exactly once
  across the document.
- Every pending removal names one exact child, Panel, or Tab presentation set. Its presentations
  have matching `close` intent, and close intents never exist outside a pending removal.
- Strict parsing rejects unknown fields. Missing or invalid relationships never receive inferred
  replacements or repairs.

## Operations

`createWorkbenchTopologyDocument(...)` validates a trusted candidate and canonicalizes Terminal
Context order. `parseWorkbenchTopologyDocument(...)` validates an untrusted value.
`WorkbenchProjectWorkspaceRuntime#getTopologyDocument(...)` projects managed runtime state and
rejects preview presentations because preview state has no process-lifecycle intent. Passing a
validated document as `initialTopology` reconstructs Project/Workspace, Tab, and Panel state while
setting current CWD to absent and emitting no process command. `subscribeTopology(...)` observes
Project/Workspace transitions and every nested Tab/Panel transition through one application-facing
projection subscription. `reconcileDurableTerminalPresentations()` is single-start: it replays
persisted provision and close operation identities and starts one restore-only operation for each
retain presentation. Restore success becomes running, restore failure remains retain intent, and a
second reconciliation call is an intentional no-op.

Schema version `1` has no migration reader. A composing application must treat another version or
an invalid document as unavailable without overwriting or reinterpreting it.

## Correctness evidence

| Claim                                                      | Evidence                                                                  | Limits                                             |
| ---------------------------------------------------------- | ------------------------------------------------------------------------- | -------------------------------------------------- |
| Strict documents preserve every identity and relationship  | Valid and malformed schema examples plus JSON round trips                 | Selected malformed classes                         |
| Projection and hydration preserve durable managed topology | Managed running and pending-close runtime round trips                     | Process reconciliation remains application work    |
| Retain reconciliation never projects provision intent      | Restore success/failure and replay command traces                         | Binding existence is enforced by the application   |
| One subscription observes nested durable topology          | Project, Tab, and Panel observer traces                                   | Application commit scheduling remains out of scope |
| Hierarchical topology remains canonical                    | Fixed-seed `fast-check` generation of 200 ordered multi-Tab/Panel layouts | Up to eight Tabs and four Panels per generated Tab |
| Preview state cannot claim durability                      | Deterministic rejection test                                              | Preview remains an in-memory product composition   |

TLA+ is not selected for this pure document projection because it has no asynchronous transition,
completion ordering, or cancellation. The application-owned commit and lifecycle-reconciliation
workflow requires its own model before integration.
