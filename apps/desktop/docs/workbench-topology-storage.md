# Desktop Workbench Topology Storage

## Scope and ownership

Desktop Main owns the privileged persistence effect for the canonical
`@orchbee/workbench/topology` document. Workbench continues to own Project, Workspace, Tab, Group,
Panel, Split, terminal-presentation placement, and document validation semantics. Desktop neither
redefines those fields nor exposes filesystem authority to preload or the renderer.

This contract covers one private file store, compare-and-swap revision policy, serialized commit
execution, and one renderer-document-scoped capability port. It does not yet connect persistence to
Workbench transitions, dispatch terminal lifecycle effects, reconcile restored presentations with
technical bindings, merge concurrent documents, synchronize another device, or define product
recovery UI.

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

This foundation exposes the scoped client but does not yet make the Workbench renderer load or
commit topology. That integration belongs to the write-ahead lifecycle slice.

## Invariants and evidence

| Claim                                                           | Evidence                                                         | Limits                                                          |
| --------------------------------------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------- |
| Only an exact-next CAS changes the stored revision              | `DesktopWorkbenchTopologyStore.tla` and deterministic core tests | TLC checks two clients and revisions from missing through `2`   |
| Conflict, rejection, load, and unavailable paths preserve state | TLA+ invariants and file-store tests                             | Filesystem faults are selected injected cases                   |
| Calls from different generations share one serialized writer    | Desktop application composition and concurrent store tests       | One Electron Main process, enforced by the single-instance lock |
| Invalid current data is never repaired or overwritten           | Corrupt-file regression                                          | No recovery UI in this slice                                    |
| Renderer receives no generic IPC or filesystem authority        | Contract, broker, preload, and renderer-client tests             | Electron structured-clone implementation is trusted             |

The TLA+ model abstracts document contents to revisions, the filesystem to writable/unavailable
state, and renderer documents to two clients. It explores load, submission, serialized start,
completion, disposal, stale expectations, invalid revision sequences, and suppressed late
responses. It does not prove Node filesystem atomicity, storage durability after hardware failure,
Zod correctness, Electron transport correctness, or correspondence to TypeScript; deterministic
adapter tests own those links.
