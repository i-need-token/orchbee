# Shared Workbench Document and Client View

## Scope and ownership

`@orchbee/workbench/document` owns two version-1 contracts with different authority:

- `WorkbenchSharedDocument` is the recoverable, revisioned product structure shared by every
  Browser page and Electron renderer page connected to one application runtime.
- `WorkbenchClientView` is one page's disposable navigation and layout projection. It never
  authorizes a product or terminal lifecycle change.

The package validates and transforms these contracts but performs no storage, IPC, process,
network, React, or DOM effect. Applications own those adapters and parse untrusted values at the
boundary. A Browser page and an Electron renderer page have equal client semantics; neither
contract contains a platform discriminator.

## Shared document

```text
WorkbenchSharedDocument
├── version: 1
├── revision: canonical unsigned decimal text
├── projectWorkspace
│   ├── identity sequences and ordered Projects
│   └── each Workspace references exactly one Terminal Context
└── terminalContexts[] (canonical identity order)
    ├── Tab and Group identity, metadata, membership, and order
    └── Panel workspace
        ├── per-Tab Panel/Split hierarchy, orientation, slot identity, and order
        ├── terminal presentations: identity, title, launch CWD, and durable intent
        └── pending removals and allocation sequences
```

Presentation intent has exactly five forms:

| Intent      | Shared durable meaning                                                          |
| ----------- | ------------------------------------------------------------------------------- |
| `provision` | The named presentation operation still requires reconciliation.                 |
| `retain`    | An existing technical binding is expected; absence cannot create a replacement. |
| `close`     | The named close operation and structural removal remain pending.                |
| `stop`      | Application exit is stopping the binding while retaining product placement.     |
| `stopped`   | Product placement remains, but no binding or process is expected.               |

Projection maps provisioning failures to `provision`; running, restoring, reconciling,
restore-failed, or exited observations to `retain`; close states to `close`; stop states to `stop`;
and a completed application stop to `stopped`. Hydration reconstructs pending lifecycle state but
dispatches no process command. Explicit reconciliation later replays provision, close, and stop and
uses restore-only for `retain`.

The effect command makes destructive disposition explicit: `terminal-close-requested` carries
`remove` for ordinary close and structural removal, or `stop` for application stop and its durable
retry. This command-only distinction lets the application archive a stopped terminal without
changing the five shared intent forms or treating ordinary close as retained product history.

Each shared Tab stores a durable fallback `title` and nullable durable `customTitle`. Product-created
fallbacks are the localized unnumbered Terminal label; a custom title is the explicit user override.
Host-derived natural terminal titles and current CWD are transient runtime observations and never
enter the shared document. Display may derive a compact directory label from the durable launch CWD,
so hydration can restore directory context before a new projection while preserving the stored
fallback and custom title unchanged.

## Client View

```text
WorkbenchClientView
├── version: 1
├── active Project/Workspace location and expanded Projects
└── terminalContexts[]
    ├── active/selected Tabs, selection anchor, recent Group, and collapsed Groups
    └── per-Tab focused/maximized Panel, active child presentation, and Split weights
```

Client View has no shared revision. Switching a Workspace or Tab, focusing or maximizing a Panel,
showing one child terminal, collapsing a Group, or resizing a Split does not change the shared
document. Split hierarchy and orientation are shared; weights are local so clients with different
viewport sizes can render the same structure independently.

Terminal viewport state is outside both contracts. Each projected terminal client owns its
`AttachmentId`, dimensions, scroll position, search input and results, selection, copy state, GPU
resources, and renderer caches. `TerminalId`, Binding identity, PTY bytes, VT state, output, and
history remain terminal-runtime authority.

## Reconciliation

`createDefaultWorkbenchClientView(document)` derives one complete deterministic view.
`reconcileWorkbenchClientView(document, candidate)` preserves every valid local reference, drops
unknown contexts and references, derives deterministic fallbacks, restores missing equal Split
weights, and orders the result by the shared document. It is idempotent and never mutates the
shared document or creates/deletes Projects, Tabs, Panels, presentations, or processes.

When a shared action also implies local navigation, the shared structural result and the initiating
client's view result are separate projections. For example, creating a Tab adds it to the shared
document and selects it only in the initiating Client View. Deleting a shared Tab causes every
client to reconcile independently.

`WorkbenchProjectWorkspaceRuntime#applySharedDocument(...)` applies a different shared structure,
reconciles the current Client View, emits no terminal command, and retains natural title, current
CWD, and rich lifecycle observations for presentations whose complete durable record is unchanged.
The runtime returns `unchanged` when only the supplied revision differs because revision ownership
belongs to the application coordinator.

Client View is in-memory for this foundation. Closing a page destroys it; a new page receives the
deterministic default unless its application supplies a separately owned candidate for
reconciliation. The Client View itself is not written to browser key-value storage because its
reference sets have no product count ceiling and ordinary same-origin `localStorage` would
accidentally couple independent Browser pages. The current Desktop Web composition stores only one
bounded, session-scoped focused `TerminalPresentationId` hint and projects it into an otherwise
default candidate; it does not serialize the Client View. Full or cross-restart Client View
recovery requires a separate accepted owner and contract.

## Invariants

- Project, Workspace, Terminal Context, Tab, Group, Panel, Split, Panel-slot, removal, and
  `TerminalPresentationId` identities are unique in their declared scope.
- Terminal Contexts use canonical identity order and equal the exact set referenced by standalone
  and Workspace contexts.
- Directory, primary-Directory, launch-configuration, and Terminal Context references resolve.
- Tab and Panel-workspace Tab membership and order match; pinned Tabs form one prefix and every
  Group is contiguous.
- Every Split has at least two children, unique slots, and no child Split repeating its orientation.
- Every presentation appears exactly once. Pending removals resolve to an exact non-empty target,
  never overlap, and match only `close` intent with the same removal identity.
- A `stop` intent never belongs to a pending removal. `stopped` never provisions or restores
  implicitly.
- A reconciled Client View has exactly one entry per shared Terminal Context and Tab, contains only
  resolving references, and gives every Split exactly one non-negative weight per slot totaling
  `100`.
- Neither contract imposes a count ceiling. Allocation or adapter exhaustion is a typed failure and
  never authorizes truncation, eviction, or identity reuse.
- Strict parsing rejects unknown fields. Shared-document corruption is unavailable and never
  repaired; invalid disposable Client View input falls back through deterministic reconciliation.

## Compatibility and application integration

`WorkbenchSharedDocument` version `1` is a new contract, not a reader for the unshipped Workbench
topology schema. Within version `1`, an omitted Tab `customTitle` is canonicalized to `null` so
documents written before that optional override was introduced remain readable. There is no old
topology alias, migration, or field reinterpretation. Applications use a distinct storage path and
ignore any development-era topology file without deleting it.

Only shared changes advance the application-owned revision and enter its durability barrier.
`subscribeDocument(...)` and `subscribeClientView(...)` compare their owned projections and notify
independently, so a client-only change cannot schedule a shared commit. The current Desktop adapter
remains a single-client writer in this foundation; multi-client arbitration and broadcasts belong
to the Desktop session coordinator.

## Correctness evidence

| Claim                                                        | Evidence                                                                 | Limits                                          |
| ------------------------------------------------------------ | ------------------------------------------------------------------------ | ----------------------------------------------- |
| Strict shared documents preserve every relationship          | Valid/malformed examples, JSON round trips, and generated hierarchies    | Generated bounds: eight Tabs, four Panels each  |
| Client Views are isolated and reconciliation is idempotent   | Two-view examples, stale-reference regression, and fixed-seed properties | In-memory view contract only                    |
| Hydration preserves durable terminal intent                  | Provision/retain/close/stop/stopped runtime round trips                  | Host binding existence remains application work |
| Custom title survives while natural metadata stays transient | Title override/reset and shared-document hydration scenarios             | Rename presentation is adapter-owned            |
| Applying a shared document emits no terminal effect          | Runtime trace with structural removal and retained running observation   | One renderer runtime                            |
| View-only changes never require a shared write               | Projection-specific observers and Desktop coordinator fingerprint tests  | Multi-client ordering belongs to the next slice |
| Preview state cannot claim shared durability                 | Deterministic rejection test                                             | Preview remains an in-memory composition        |

TLA+ is not selected for the pure document/view projection because it contains no asynchronous
completion, cancellation, or independently ordered event. The Desktop commit/lifecycle workflow
owns its existing asynchronous model; the multi-client coordinator must replace or extend that
model when its ordering contract changes.
