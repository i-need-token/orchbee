# Desktop Shared Workbench Document Storage

## Scope and ownership

Desktop Main owns the privileged persistence effect for `WorkbenchSharedDocument` from
`@orchbee/workbench/document`. Workbench owns its Project, Workspace, Tab, Group, Panel, Split,
terminal-presentation, lifecycle-intent, validation, and Client View reconciliation semantics.
Desktop neither redefines those fields nor exposes filesystem authority to preload or renderer.

This contract covers one private shared-document file, exact-next compare-and-swap revisions,
serialized file operations, one renderer-document-scoped capability port, and write-ahead ordering
before terminal lifecycle effects. It remains a single live renderer client in this foundation. It
does not merge concurrent edits, broadcast another client's document, persist Client View, expose a
network service, infer missing bindings, adopt unreferenced bindings, or define recovery UI.

## Storage state and revision policy

The store returns exactly one state:

| State         | Meaning                                                                       |
| ------------- | ----------------------------------------------------------------------------- |
| `missing`     | The new shared-document path does not exist.                                  |
| `available`   | The complete file parses as one canonical version-1 shared document.          |
| `unavailable` | The path is unreadable, non-file, invalid UTF-8/JSON, or an invalid document. |

`load()` never creates, repairs, migrates, truncates, or rewrites a file. The configured filename
is `workbench-shared-document-v1.json`. The product has not shipped, so the former development
topology filename and schema have no compatibility reader or alias; the old file is ignored and
not automatically deleted.

`commit(expectedRevision, document)` is serialized with every operation on the store instance and
applies these rules:

1. A missing file accepts only `expectedRevision = null` and candidate revision `0`.
2. An available file accepts only an expectation equal to its current revision.
3. The candidate revision must be exactly one greater than the expectation.
4. A stale expectation returns `conflict` with the actual revision and preserves the file.
5. A skipped, repeated, or otherwise non-next candidate returns `rejected/revision-sequence`.
6. Invalid or unreadable current data is `unavailable` and is never overwritten.
7. A write failure is `unavailable/write-failed`; callers load again before deciding to retry.

Revisions are canonical decimal text and advance through `BigInt`; they have no JavaScript-number
ceiling. Only a changed shared fingerprint is committed. Client-local navigation, disclosure,
focus, active child, and Split-weight transitions do not change that fingerprint and therefore do
not advance the revision.

## File effect

The path is absolute and private to Desktop Main. Commit validates the complete candidate, creates
the parent directory owner-only, writes a new exclusive random temporary file, synchronizes and
closes it, renames it over the target, and synchronizes the parent directory where supported. The
result is newline-terminated UTF-8 JSON with owner-only permissions. A pre-rename failure removes
the temporary file.

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
responses and new dispatch; already accepted Main work may finish, and durable intent permits a new
generation to reconcile it.

## Client View

Desktop does not write `WorkbenchClientView` into this file or browser key-value storage. The
renderer owns one in-memory view and derives defaults after a new page starts. This prevents the
shared document from carrying active Workspace/Tab, disclosure, focus, maximization, active child,
or Split weights and avoids accidentally sharing page-local state with a future Browser client.

## Invariants and evidence

| Claim                                                       | Evidence                                                         | Limits                                                      |
| ----------------------------------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------- |
| Only exact-next CAS changes the stored revision             | `DesktopWorkbenchDocumentStore.tla` and deterministic core tests | TLC checks two submitters and revisions missing through `2` |
| Invalid/current unavailable state is never overwritten      | File-store corruption and injected failure tests                 | Selected filesystem failures                                |
| Client View-only transitions do not write shared state      | Workbench projection plus coordinator fingerprint tests          | Single renderer client                                      |
| Renderer receives no generic IPC or filesystem authority    | Contract, broker, preload, and renderer-client tests             | Electron structured clone is trusted                        |
| Terminal effects require previously durable matching intent | `DesktopWorkbenchWriteAhead.tla` and coordinator traces          | One abstract presentation in the TLA+ model                 |
| Restore cannot create a replacement terminal                | Restore-only adapter and renderer recovery traces                | Binding ledger and Host reconciliation remain trusted       |
| Cleanup failure retains replayable intent                   | Write-ahead model and managed-removal integration traces         | Product recovery UI remains out of scope                    |

The renamed CAS model retains the existing transition semantics; this slice changes the persisted
payload contract but not its asynchronous queue or revision algorithm. Multi-client arbitration,
broadcast ordering, disconnect behavior, and progress require a new coordinator model in the next
slice rather than being inferred from this single-client evidence.
