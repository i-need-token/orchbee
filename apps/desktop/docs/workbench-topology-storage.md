# Desktop Workbench Topology Storage

## Scope and ownership

Desktop Main owns the privileged persistence effect for the canonical
`@orchbee/workbench/topology` document. Workbench continues to own Project, Workspace, Tab, Group,
Panel, Split, terminal-presentation placement, and document validation semantics. Desktop neither
redefines those fields nor exposes filesystem authority to preload or the renderer.

This contract covers one private file store, compare-and-swap revision policy, serialized commit
execution, one renderer-document-scoped capability port, and the renderer workflow that orders
durable Workbench state before terminal lifecycle effects. It does not merge concurrent documents,
synchronize another device, infer missing technical bindings, adopt unreferenced bindings, or
define product recovery UI.

## State and operations

The store has one of three observable load states:

| State         | Meaning                                                                 |
| ------------- | ----------------------------------------------------------------------- |
| `missing`     | No topology file exists.                                                |
| `available`   | The complete file parsed as one canonical version-1 Workbench document. |
| `unavailable` | The path was unreadable or its complete contents were invalid.          |

`load()` never creates, repairs, migrates, truncates, or rewrites a file. There is no compatibility
reader because the product has not shipped.

`commit(expectedRevision, document)` applies these rules atomically with respect to every call on
the one Main-owned store instance:

1. A missing file accepts only `expectedRevision = null` and document revision `0`.
2. An available file accepts only an expected revision equal to its current revision.
3. The candidate revision must be exactly one greater than the expected revision.
4. A stale expectation returns `conflict` with the actual revision and preserves the file.
5. A skipped, repeated, or otherwise non-next candidate revision returns `rejected` and preserves
   the file.
6. Invalid or unreadable current data returns `unavailable` and is never overwritten.
7. A write failure returns `unavailable/write-failed`. Because a platform failure may occur after
   rename but before directory synchronization completes, callers must load again before deciding
   whether to retry.

Canonical decimal revisions are converted with `BigInt`; the policy imposes no numeric product
ceiling and never converts a revision through JavaScript `number`.

## File effect

The configured path is absolute and private to Desktop Main. A commit validates the complete
candidate, creates its parent directory with owner-only permissions, writes one exclusive random
temporary file, synchronizes and closes that file, renames it over the target, and synchronizes the
parent directory where the platform supports that operation. The file is newline-terminated UTF-8
JSON with owner-only permissions. Temporary files are removed after a pre-rename failure.

The topology store is separate from the terminal-binding store. The two documents have different
semantic owners, schemas, failure recovery, and update timing; neither write transaction includes
the other file.

## Capability boundary

Each live renderer document receives one allowlisted topology-control `MessagePort` through the
sandboxed preload. Commands contain only a request identity, `load` or `commit` intent, expected
revision, and a canonical Workbench topology document. Results contain only the correlated load or
commit outcome. File paths, Node errors, binding records, Host identities, generic IPC, and
filesystem operations never cross the boundary.

Main validates every command and Renderer validates every event. Malformed data, an unexpected
transferred port, or reuse of a request identity for different content retires the capability. Main
serializes accepted commits across all renderer generations through the one store instance. Port
disposal suppresses a late response but does not cancel a commit already accepted by Main; its CAS
precondition still decides whether the file changes. A new renderer generation must load before it
commits.

## Renderer write-ahead workflow

The renderer loads topology before constructing its managed Workbench runtime. An available
document is hydrated exactly. A missing document constructs the current flat recovery/bootstrap
composition and must commit revision `0` before any initial provision command crosses the document
control port. An unavailable load, failed initial commit, conflict, or rejected revision stops the
generation without creating a replacement topology or dispatching a terminal lifecycle effect.

After startup, the renderer observes the complete Project/Workspace and nested Tab/Panel topology.
It coalesces ordinary in-memory changes while one commit is in flight. Each terminal command is
captured with the complete topology produced by its synchronous Workbench transition. Provision
and close commands remain queued until a successful CAS has stored their exact durable intent.
Commands whose intent was already loaded may proceed without a redundant commit. A later CAS
failure retires the writer for that renderer generation and leaves every undispatched command
blocked.

Applying a lifecycle result may remove a managed child, Panel, or Tab in memory before the cleanup
document commits. This is safe because the previously stored provision or close intent remains the
recovery authority: if cleanup does not commit, the next renderer replays that idempotent intent.
No result handler erases the durable intent before its external effect completes.

`retain` reconciliation uses a distinct restore-only command. Main accepts it only for a matching
Binding that was already durably retained or exited, or that became restorable after a successful
provision in the same Main lifetime. It never allocates a Binding or creates a replacement shell.
Restore completion projects to running or exited presentation state; restore failure remains
durable `retain` state and therefore cannot silently become provision intent.

Renderer disposal suppresses pending responses and stops new dispatch. A topology commit or
terminal operation already accepted by Main may still finish; its stored intent makes replay by a
new generation safe.

## Invariants and evidence

| Claim                                                           | Evidence                                                           | Limits                                                          |
| --------------------------------------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------------- |
| Only an exact-next CAS changes the stored revision              | `DesktopWorkbenchTopologyStore.tla` and deterministic core tests   | TLC checks two clients and revisions from missing through `2`   |
| Conflict, rejection, load, and unavailable paths preserve state | TLA+ invariants and file-store tests                               | Filesystem faults are selected injected cases                   |
| Calls from different generations share one serialized writer    | Desktop application composition and concurrent store tests         | One Electron Main process, enforced by the single-instance lock |
| Invalid current data is never repaired or overwritten           | Corrupt-file regression                                            | No recovery UI in this slice                                    |
| Renderer receives no generic IPC or filesystem authority        | Contract, broker, preload, and renderer-client tests               | Electron structured-clone implementation is trusted             |
| Terminal effects require a previously stored matching intent    | `DesktopWorkbenchWriteAhead.tla` and coordinator traces            | One abstract presentation; deterministic tests cover batching   |
| Restore never creates a replacement terminal                    | Write-ahead model, restore-only adapter tests, and recovery traces | Main Binding ledger and Host reconciliation remain trusted      |
| Cleanup failure preserves a replayable pre-effect intent        | Write-ahead model and managed-removal integration traces           | Product recovery UI remains outside this slice                  |

The TLA+ model abstracts document contents to revisions, the filesystem to writable/unavailable
state, and renderer documents to two clients. It explores load, submission, serialized start,
completion, disposal, stale expectations, invalid revision sequences, and suppressed late
responses. It does not prove Node filesystem atomicity, storage durability after hardware failure,
Zod correctness, Electron transport correctness, or correspondence to TypeScript; deterministic
adapter tests own those links.

The write-ahead model abstracts topology to one presentation intent and separates provision,
restore, and close effects. It explores initial replay, dirty intent, commit success or failure,
dispatch, completion, result cleanup, renderer disposal, and reload. It checks that dispatch always
has matching durable intent, failed commits preserve storage, only provision can create a terminal,
and a queued command progresses while its generation remains connected and storage remains
writable. It does not model hierarchical topology, command batching, binding-ledger internals, or
Host transport; Workbench invariant tests and Desktop adapter traces own those refinement links.
