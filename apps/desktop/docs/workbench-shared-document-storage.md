# Desktop Shared Workbench Document Storage

## Scope and ownership

Desktop Main owns the privileged persistence effect for `WorkbenchSharedDocument` from
`@orchbee/workbench/document`. Workbench owns its Project, Workspace, Tab, Group, Panel, Split,
terminal-presentation, lifecycle-intent, validation, and Client View reconciliation semantics.
Desktop neither redefines those fields nor exposes filesystem authority to preload or renderer.

This contract covers one private shared-document file, exact-next compare-and-swap revisions,
serialized file operations, one renderer-document-scoped capability port, and write-ahead ordering
before terminal lifecycle effects. The store itself does not merge concurrent edits, persist Client
View, expose a network service, infer missing bindings, adopt unreferenced bindings, or define
recovery UI. Desktop's session coordinator separately owns the canonical multi-client proposal queue
and broadcasts.

The same coordinator exposes a process-local Main-only canonical mutation queue. Managed local
[Worktree creation](project-worktree-creation.md) and [adoption](project-worktree-adoption.md) use it
to commit their Directory, Workspace, and Terminal Context changes beside renderer proposals.
Current Desktop terminal effects require local placement. Legacy Environment placement can remain
in a valid document, but it supplies no SSH admission, remote provisioning, or remote projection
capability.

## Storage state and revision policy

The store returns exactly one state:

| State         | Meaning                                                                       |
| ------------- | ----------------------------------------------------------------------------- |
| `missing`     | The new shared-document path does not exist.                                  |
| `available`   | The complete file parses as one canonical version-3 shared document.          |
| `unavailable` | The path is unreadable, non-file, invalid UTF-8/JSON, or an invalid document. |

The current filename is `workbench-shared-document-v2.json`. `load()` first reads only that path.
If it exists, valid or invalid, it is the sole authority and Desktop never falls back to v1. If it
is missing, Desktop may read exact `workbench-shared-document-v1.json` once through Workbench's
historical v1 parser, add local placement to every Directory, and atomically write the validated v3
document. The v1 file remains byte-for-byte untouched. Missing v1 remains `missing`; invalid or
unreadable v1 is `unavailable` and is never repaired. There is no reverse migration.

The established filename stays unchanged. A valid v2 payload is strictly migrated to v3 in memory, retaining its revision, terminal graph and scoped resources; the next successful canonical commit writes v3. The v1 fallback also completes the strict v2-to-v3 migration before writing. Malformed old graphs remain unavailable and are never reset. Project grouping does not define Directory or remote terminal authority.

`commit(expectedRevision, document)` is serialized with every operation on the store instance and
applies these rules:

1. A missing file accepts only `expectedRevision = null` and candidate revision `0`.
2. An available file accepts only an expectation equal to its current revision.
3. The candidate revision must be exactly one greater than the expectation.
4. A stale expectation returns `conflict` with the actual revision and preserves the file.
5. A skipped, repeated, or otherwise non-next candidate returns `rejected/revision-sequence`.
6. Invalid or unreadable current data is `unavailable` and is never overwritten.
7. A failure known to precede rename is `unavailable/write-failed`. Failure after rename is
   `unavailable/write-indeterminate`; callers restart or load again before retrying.

Revisions are canonical decimal text and advance through `BigInt`; they have no JavaScript-number
ceiling. Only a changed shared fingerprint is committed. Client-local navigation, disclosure,
focus, active child, and Split-weight transitions do not change that fingerprint and therefore do
not advance the revision.

## File effect

The path is absolute and private to Desktop Main. Commit validates the complete candidate, creates
the parent directory owner-only, writes a new exclusive random temporary file, synchronizes and
closes it, renames it over the target, and synchronizes the parent directory where supported. The
result is newline-terminated UTF-8 JSON with owner-only permissions. A pre-rename failure removes
the temporary file. Parent-directory synchronization failure after rename cannot prove whether the
new file is durable and is therefore indeterminate rather than a safe negative result.

The shared-document store is separate from the terminal-binding ledger. The schemas, owners,
failure recovery, and update timing differ; no transaction pretends to atomically write both.

## Capability boundary

The live renderer receives the initial document state in the atomic Desktop bootstrap and submits
complete-document proposals through its `client-session` `MessagePort`. Each proposal carries a
bounded request identity, expected revision, and strict shared document; Main owns storage load and
commit. Correlated results and ordered canonical publications use that same session capability.
Storage paths, Node errors, binding records, Host identities, generic IPC, and filesystem operations
are not document-control capabilities.

Sandboxed preload validates one bootstrap and the complete capability bundle declared by
[`DESKTOP_RENDERER_PORT_ROLES`](../../../packages/desktop-client-protocol/src/desktop-renderer-bootstrap-contract.ts)
before forwarding it through `window.postMessage`; it exposes no `ipcRenderer` object or generic
send. Main rejects malformed commands and transferred ports. Exact repeated request content is
deduplicated; identity reuse with different content retires that session. Disposal suppresses late
responses and further proposal admission.

The legacy remote-placement guard permits exact retention or deletion, admitted `provision` to
`retain` convergence, exact whole-Tab close intent, and Tab display metadata changes. It rejects
forged Directory, Workspace, Presentation, launch-CWD, placement, or topology authority. This
proposal-origin check is separate from structural parsing and does not authorize a remote effect.

## Renderer and write-ahead workflow

Main loads the shared document before publishing the renderer bootstrap. An available document
hydrates the managed Workbench runtime without dispatching a process command. Missing storage
provides one common initialization document; its revision `0` must commit before Main releases
the initial provision effect. Unavailable storage cannot publish a replacement shared structure
or dispatch a new terminal effect.

The renderer coordinator observes only shared-document projection changes and projects the complete
document. It coalesces changes while one proposal is pending. Client View transitions use a separate
observer and cannot schedule a write; fingerprint comparison remains a defense at the coordinator.
Main publishes a committed document before reconciling its provision, close, or application-stop
intent. An equivalent race converges without a second write. A divergent stale proposal returns
the canonical document, which the renderer applies without replaying its losing local change.
Rejected, unavailable, or ambiguous proposal outcomes close that renderer coordinator's admission;
they do not authorize overwriting a divergent document.

An available `retain` presentation starts restore-only reconciliation after coordinator startup.
Main accepts restore only for a binding already retained by its ledger or established during the
same Main lifetime. Restore never allocates a new terminal. Renderer disposal suppresses pending
responses and new dispatch. A document proposal still waiting in the session queue is suppressed
before it reaches storage when its renderer generation is disposed; a proposal or terminal effect
already accepted by Main may finish. Main retains the settled result for a durable terminal
operation identity, so a new renderer generation can apply that result without executing the same
effect again. An unsettled durable intent remains available for reconciliation.

Main-owned mutations use the same revision queue. After canonical initialization, a rejected or
unchanged mutation allocates no further revision and writes nothing. A successful durable commit precedes its notification, archive
reconciliation, client publication, and terminal reconciliation; failure in one post-commit observer
cannot undo the commit or suppress the others. An indeterminate store outcome freezes canonical
control until recovery. Explicit reconciliation can retry the unchanged current document without
fabricating another revision. Canonical containment must resolve each presentation uniquely to the
local runtime; missing, ambiguous, and Environment targets fail before a terminal effect.

## Client View

Desktop does not serialize `WorkbenchClientView` into this file or browser key-value storage. Each
renderer owns its in-memory view. The Web composition may restore one bounded session-local focus
hint against the current document, as specified by [navigation routing](../../web/docs/navigation-routing.md).
Active Workspace/Tab, disclosure, focus, maximization, active child, and Split weights remain outside
the shared document and cannot become another page's navigation authority.

## Invariants and evidence

| Claim                                                               | Evidence                                                                                 | Limits                                                                 |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Only exact-next CAS changes the stored revision                     | [Revision decision tests][decision-tests] and [file-store tests][store-tests]            | Missing, stale, non-next and large revisions; concurrent writers       |
| Disposed queued work is suppressed before storage                   | [Session queue tests][queue-tests]                                                       | Controlled disconnect before/after admission; accepted work may finish |
| Invalid/current unavailable state is never overwritten              | [File-store tests][store-tests]                                                          | Selected corruption and filesystem failures                            |
| v1 migration is one-way and the current filename is authoritative   | [File-store tests][store-tests] and [document migration tests][document-tests]           | Strict v1/v2 inputs and selected rename failures                       |
| Client View-only transitions do not write shared state              | [Document runtime tests][runtime-tests] and [renderer coordinator tests][renderer-tests] | Each Client View remains client-local                                  |
| Renderer receives only the declared document capability             | [Session broker tests][broker-tests] and [preload tests][preload-tests]                  | Electron structured clone is trusted                                   |
| Main-owned mutations share the renderer proposal queue              | [Session queue tests][queue-tests] and [Worktree runtime tests][worktree-tests]          | Controlled in-process commit and failure traces                        |
| Legacy placement cannot grant remote effect authority               | [Proposal guard tests][proposal-tests] and [target resolver tests][target-tests]         | Current Desktop effects are local-only                                 |
| Durable intent precedes effects; settled retries do not repeat them | [Session queue tests][queue-tests] and [reconciler tests][reconciler-tests]              | Current operation identity; Host/ledger idempotency remains trusted    |
| Restore cannot create a replacement terminal                        | [Lifecycle adapter tests][adapter-tests]                                                 | Binding ledger and Host reconciliation remain trusted                  |
| A pre-rename cleanup write failure preserves stored intent          | [File-store failure tests][store-tests] and [document runtime tests][runtime-tests]      | Selected write failures and hydration traces, not a workflow model     |

The store and session queue each have one in-process serialization owner. Revision decisions,
competing commits, disconnect before/after admission, and queue draining are checked directly by
the implementation tests above. This boundary has no exhaustive TLA+ safety or progress claim;
completion traces assume awaited operations settle. File tests cover selected migration and write
failures, not general filesystem durability. Multi-client arbitration, publication, disconnect,
and quiescing evidence belongs to the [session coordinator](multi-client-session-coordinator.md).

[decision-tests]: ../src/main/workbench-document/__tests__/decideWorkbenchDocumentCommit.unit.test.ts
[store-tests]: ../src/main/workbench-document/__tests__/FileWorkbenchDocumentStore.unit.test.ts
[document-tests]: ../../../packages/workbench/src/workbench/document/__tests__/workbench-document.unit.test.ts
[runtime-tests]: ../../../packages/workbench/src/workbench/model/__tests__/workbench-document-runtime.unit.test.ts
[renderer-tests]: ../../web/src/workbench-client/workbench-document-control/__tests__/WorkbenchDocumentCoordinator.unit.test.ts
[queue-tests]: ../src/main/client-session/__tests__/DesktopClientSessionCoordinator.unit.test.ts
[broker-tests]: ../src/main/client-session/__tests__/DesktopClientSessionBroker.unit.test.ts
[preload-tests]: ../src/preload/__tests__/installDesktopRendererBootstrapBridge.unit.test.ts
[worktree-tests]: ../src/main/project-worktree/__tests__/DesktopProjectWorktreeRuntime.unit.test.ts
[proposal-tests]: ../src/main/client-session/__tests__/validateDesktopClientRemotePlacementProposal.unit.test.ts
[target-tests]: ../src/main/client-session/__tests__/desktop-terminal-effect-target.unit.test.ts
[reconciler-tests]: ../src/main/client-session/__tests__/DesktopTerminalIntentReconciler.unit.test.ts
[adapter-tests]: ../src/main/terminal-document-control/__tests__/DesktopTerminalLifecycleAdapter.unit.test.ts
