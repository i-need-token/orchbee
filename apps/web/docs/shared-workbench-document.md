# Shared Workbench Document and Client View

## Scope and ownership

Two versioned contracts have different authority:

- `WorkbenchSharedDocument` is the recoverable, revisioned product structure shared by every
  Browser page and Electron renderer page connected to one application runtime.
  `@orchbee/workbench-client-protocol` owns its strict schema, revision, and parser.
- `WorkbenchClientView` is one page's disposable navigation and layout projection, owned by
  `src/workbench/document`. It never authorizes a product or terminal lifecycle change.

Workbench produces, hydrates, and transforms both contracts; this reference remains their semantic
description. Neither package performs a storage, IPC, process, network, React, or DOM effect.
Applications own those adapters and parse untrusted values at the boundary. A Browser page and an
Electron renderer page have equal client semantics; neither contract contains a platform
discriminator.

## Shared document

```text
WorkbenchSharedDocument
├── version: 1
├── revision: canonical unsigned decimal text
├── projectWorkspace
│   ├── identity sequences, ordered optional Projects and independent launch-resource scopes
│   └── ordered Workspaces reference a nullable Project, stable resource scope and one Terminal Context
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

The public stop retry is one batch operation over the current document: it restarts every
retryable `stop-failed` or `close-failed` presentation with a fresh operation identity while
preserving already-stopped and still-in-flight peers. A failed durability commit rolls the staged
stop back before any close command, and starting a stopped placement allocates a fresh Presentation
identity rather than reviving its retired binding.

A Terminal Context may be structurally empty: allocation sequences start at `1`, while its Tab,
Group, Panel, presentation, and pending-removal collections are empty. This is a document shape,
not evidence that a consuming application provides remote Workspace admission or provisioning.

The schema retains Environment placement for compatible documents. Placement identifies a resource
namespace but grants no transport or process capability. Applications own effect admission and
proposal-origin checks.

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
├── active Workspace-only location and expanded Projects
└── terminalContexts[]
    ├── active/selected Tabs, selection anchor, recent Group, and collapsed Groups
    └── per-Tab focused/maximized Panel, active child presentation, and Split weights
```

Client View has no shared revision. Switching a Workspace or Tab, focusing or maximizing a Panel,
showing one child terminal, collapsing a Group, or resizing a Split does not change the shared
document. Split hierarchy and orientation are shared; weights are local so clients with different
viewport sizes can render the same structure independently.

The live per-Tab MRU Panel focus history is runtime-only state initialized from each Client View
focused Panel. It is not serialized in either contract, and initial hydration therefore does not
restore earlier focus visits.

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
- Every Directory has exactly one placement: `local`, or `environment` with one non-empty opaque
  Environment reference. An environment-placed Directory is always unavailable.
- Every Workspace primary Directory resolves and may have local or Environment placement. Ordinary
  Workbench execution mutations require local placement; organization changes preserve either
  placement. Structural parsing does not authorize creation or retargeting of remote effects;
  consuming applications enforce their own proposal and runtime boundaries.
- Tab and Panel-workspace Tab membership and order match; pinned Tabs form one prefix and every
  Group is contiguous.
- Every Split has exactly two children and unique slots. A child Split may repeat its parent's
  orientation because that ancestry owns local close and resize behavior.
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
  repaired; malformed Client View input is rejected. Only valid stale references use deterministic reconciliation.

## Compatibility and application integration

`WorkbenchSharedDocument` version `1` separates ordered Project groups from ordered Workspaces and immutable launch-resource namespaces. Strict parsing validates the complete current graph, including scoped Directory/configuration references, duplicate IDs, context ownership and terminal topology. Resource IDs may repeat across namespaces; allocation skips every occupied ID.

Only the current complete shape is accepted; Tab `customTitle` is required and may be `null`. Partial, malformed, unknown-version and historical development topologies are rejected without migration or empty replacement state.

Client View version `1` uses `workspaceId` without Project identity and validates only the current view graph. Reconciliation preserves the same Workspace after move or dissolution. A grouping-only shared publication retains live context, Tab model and Panel store objects, local focus/zoom/child selection and terminal attachments.

Only shared changes advance the application-owned revision and enter its durability barrier.
`subscribeDocument(...)` and `subscribeClientView(...)` compare their owned projections and notify
independently, so a client-only change cannot schedule a shared commit. Workbench does not arbitrate
clients; Desktop's session coordinator owns the single canonical writer, proposal ordering, and
broadcasts.

## Correctness evidence

| Claim                                                        | Evidence                                                                                                                                                                                           | Limits                                                           |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Strict shared documents preserve every binary relationship   | Valid/malformed examples, JSON round trips, and generated same-axis hierarchies                                                                                                                    | Generated bounds: eight Tabs, four Panels each                   |
| Only the complete current shape is accepted                  | Strict parsing, invalid-graph rejection and Desktop file-authority tests                                                                                                                           | No historical-record migration                                   |
| Client Views are isolated and reconciliation is idempotent   | Two-view examples, stale-reference regression, and fixed-seed properties                                                                                                                           | In-memory view contract only                                     |
| Hydration preserves durable terminal intent                  | Provision/retain/close/stop/stopped runtime round trips                                                                                                                                            | Host binding existence remains application work                  |
| Custom title survives while natural metadata stays transient | Title override/reset and shared-document hydration scenarios                                                                                                                                       | Rename presentation is adapter-owned                             |
| Applying a shared document emits no terminal effect          | Runtime trace with structural removal and retained running observation                                                                                                                             | One renderer runtime                                             |
| An empty Terminal Context carries no presentation intent     | [Document schema tests](../../../packages/workbench-client-protocol/src/__tests__/workbench-shared-document-contract.unit.test.ts)                                                                 | Pure structure; no remote runtime admission                      |
| View-only changes never require a shared write               | Projection-specific observers and Desktop coordinator fingerprint tests                                                                                                                            | Shared ordering is Desktop session authority                     |
| Preview state cannot claim shared durability                 | Deterministic rejection test                                                                                                                                                                       | Preview remains an in-memory composition                         |
| Application stop is durable before close and batch-retryable | [Stop lifecycle tests](../src/workbench/model/__tests__/terminal-session-stop.unit.test.ts) and [document runtime tests](../src/workbench/model/__tests__/workbench-document-runtime.unit.test.ts) | Controlled runtime fixtures; OS termination is outside the claim |

Pure document/view projection and strict current-shape admission use deterministic/property tests. Controlled lifecycle tests cover commit-before-close, retryable failure, stopped-without-binding, fresh replacement identity and preservation of placement, title and launch-CWD. Desktop document-store CAS retains its separately bounded protocol model; that model does not establish the whole multi-client or lifecycle composition.
