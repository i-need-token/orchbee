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

The same coordinator exposes a process-local Main-only canonical mutation queue. Slice 3.2A uses it
after SSH Environment authorization and canonical Project-root inspection to commit a remote
Project, its Environment Directory, exactly one inactive Workspace, and one empty Terminal Context
as a single next revision. That mutation creates no terminal presentation and releases no Host
effect; retry upgrades a legacy remote Project missing the Workspace and otherwise converges to
unchanged.

Slice 3.2B adds one explicit Main-only mutation over that admitted empty context. It creates the
first remote terminal presentation, commits its `provision` intent through the same queue, and only
then reconciles the target Environment runtime. A retry that finds the exact singleton presentation
reconciles it without another commit; a different or non-empty shape rejects. Runtime availability
is observed outside the document, so disconnect or retryable/unknown execution leaves the durable
intent pending instead of publishing a false terminal outcome.

## Storage state and revision policy

The store returns exactly one state:

| State         | Meaning                                                                       |
| ------------- | ----------------------------------------------------------------------------- |
| `missing`     | The new shared-document path does not exist.                                  |
| `available`   | The complete file parses as one canonical version-2 shared document.          |
| `unavailable` | The path is unreadable, non-file, invalid UTF-8/JSON, or an invalid document. |

The current filename is `workbench-shared-document-v2.json`. `load()` first reads only that path.
If it exists, valid or invalid, it is the sole authority and Desktop never falls back to v1. If it
is missing, Desktop may read exact `workbench-shared-document-v1.json` once through Workbench's
historical v1 parser, add local placement to every Directory, and atomically write the validated v2
document. The v1 file remains byte-for-byte untouched. Missing v1 remains `missing`; invalid or
unreadable v1 is `unavailable` and is never repaired. There is no reverse migration.

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

The live renderer document receives one allowlisted `workbench-document-control` `MessagePort`
through sandboxed preload. Commands contain only a bounded request identity and either load or
commit intent, expected revision, and a complete validated shared document. Results contain only
the correlated typed outcome. File paths, Node errors, binding records, Host identities, generic
IPC, and filesystem operations never cross the boundary.

Main rejects malformed commands, transferred ports, wrong request shapes, and messages after
disposal. Preload accepts exactly one validated bootstrap for the Electron document and transfers
exactly one port through `window.postMessage`; it exposes no `ipcRenderer` object or generic send.
Renderer responses are correlated by request ID, duplicate IDs are unavailable, and late events
after disposal are ignored.

Client proposals may retain or delete exact Main-admitted remote placement, and may converge only
the exact admitted remote presentation from `provision` to `retain`. The coordinator rejects and
retires a Client that introduces, relabels, reparents, duplicates, converts, retargets, or otherwise
mutates a remote Directory, Workspace, or terminal structure, or reuses its protected identity.
This proposal-origin check is separate from structural document parsing.

## Renderer and write-ahead workflow

Renderer loads the shared document before constructing its managed Workbench runtime. An available
document hydrates with a deterministic default Client View and emits no process command. Missing
storage creates the normal initial runtime and commits revision `0` before releasing its initial
provision command. Unavailable storage fails the renderer generation closed without rendering,
creating replacement shared structure, or dispatching a terminal effect.

The renderer coordinator observes only shared-document projection changes and projects the complete
document. It coalesces changes while one commit is pending. Client View transitions use a separate
observer and cannot schedule a write; fingerprint comparison remains a defense at the coordinator.
Provision, restore, close, and application-stop
commands wait behind the shared-document barrier that contains their corresponding intent. Only a
successful exact-next commit releases the prepared commands. Conflict, rejection, or unavailable
storage permanently fails that renderer generation; no divergent document is overwritten.

An available `retain` presentation starts restore-only reconciliation after coordinator startup.
Main accepts restore only for a binding already retained by its ledger or established during the
same Main lifetime. Restore never allocates a new terminal. Renderer disposal suppresses pending
responses and new dispatch. A document proposal still waiting in the session queue is suppressed
before it reaches storage when its renderer generation is disposed; a proposal or terminal effect
already accepted by Main may finish. Main retains the settled result for a durable terminal
operation identity, so a new renderer generation can apply that result without executing the same
effect again. An unsettled durable intent remains available for reconciliation.

A Main-only remote-Workspace admission enters the same serialized canonical queue but has no
renderer write-ahead command. Its Project, Directory, Workspace, and empty context either share one
successful file commit or remain absent; a post-rename indeterminate result is reconciled by loading
and retrying the same `(EnvironmentId, canonicalPath)` key.

The explicit first-remote-terminal operation also enters that queue, but prepares no Host work
before admission. It rechecks cancellation and Environment connectivity at queue admission, applies
the exact Workbench mutation, and commits the next canonical revision. Only a successful commit may
release reconciliation to the Environment-specific runtime. `reconcileCanonicalDocument()` lets an
exact retry reconcile an unchanged canonical presentation without fabricating a second shared
change. The reconciler derives its execution target from canonical Project/Workspace containment;
missing or ambiguous ownership fails closed instead of falling back to the local runtime.

## Client View

Desktop does not write `WorkbenchClientView` into this file or browser key-value storage. The
renderer owns one in-memory view and derives defaults after a new page starts. This prevents the
shared document from carrying active Workspace/Tab, disclosure, focus, maximization, active child,
or Split weights and avoids accidentally sharing page-local state with a future Browser client.

## Invariants and evidence

| Claim                                                               | Evidence                                                                             | Limits                                                                   |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| Only exact-next CAS changes the stored revision                     | `DesktopWorkbenchDocumentStore.tla` and deterministic core tests                     | TLC checks two submitters and revisions missing through `2`              |
| Disposed queued work is suppressed before storage                   | `DesktopWorkbenchDocumentStore.tla` and session queue regression                     | Two renderer generations with one request each; accepted work may finish |
| Invalid/current unavailable state is never overwritten              | File-store corruption and injected failure tests                                     | Selected filesystem failures                                             |
| v1 migration is one-way and v2-present is authoritative             | owner implementation tests and migration tests                                       | Exact v1; TLA+ covers successful exact post-rename reload only           |
| Client View-only transitions do not write shared state              | Workbench projection plus coordinator fingerprint tests                              | Each Client View remains client-local                                    |
| Renderer receives no generic IPC or filesystem authority            | Contract, broker, preload, and renderer-client tests                                 | Electron structured clone is trusted                                     |
| Remote Workspace admission is atomic and Main-owned                 | Remote-admission TLA+, persistence, concurrency, and forgery tests                   | No terminal presentation or Host effect in Slice 3.2A                    |
| First remote terminal commits before its target effect              | Remote-provision TLA+, queue/commit traces, target-routing, retry, and forgery tests | One exact singleton presentation; projection/input remain reserved       |
| Durable intent precedes effects; settled retries do not repeat them | owner implementation tests and reconciler traces                                     | One abstract presentation and one reusable operation identity            |
| Restore cannot create a replacement terminal                        | Restore-only adapter and renderer recovery traces                                    | Binding ledger and Host reconciliation remain trusted                    |
| Cleanup failure retains replayable intent                           | Write-ahead model and managed-removal integration traces                             | Product recovery UI remains out of scope                                 |

The renamed CAS model retains the existing transition semantics; this slice changes the persisted
payload contract but not its asynchronous queue or revision algorithm. Broader multi-client
arbitration, broadcast ordering, reconnect behavior, and quiescing progress remain owned and modeled
by the separate Desktop session coordinator.
