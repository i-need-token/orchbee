# Windows checkpoint storage

## Scope and ownership

The private Windows session store transports opaque Workbench documents. The Server validates
their domain meaning and owns capture, restoration and scheduling. This contract implements the
accepted retention and recovery policy: keep three valid snapshots, fall back to a valid copy,
retain unreadable originals, and report an empty restoration when every copy is invalid. It does
not authorize starting beside an unknown or living Site owner. Windows crash admission remains
the local Site runtime's responsibility.

The Workbench payload remains its existing bounded versioned JSON format. Filesystem framing is
private to this storage owner; it does not add fields to the Workbench document or a public wire
protocol. Old plain `checkpoint.json` files remain readable. Earlier executable cohorts must not
open the upgraded Site; keeping Workbench JSON at version 4 does not make the framed multi-slot
layout backward compatible. Catalog admission must preserve every byte and refuse an earlier
cohort rather than flattening or deleting upgraded state. Rollback requires a separately authorized
offline conversion or a different Site. No automatic downgrade or live-owner adoption is provided.

The durable session catalog version is the admission boundary for this private layout. It is
separate from both the Workbench document version and the frame version:

| Catalog record | Process identity and recovery                                    | Checkpoint evidence for a stopped Site                                                       |
| -------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| v1             | No process registry; an Active record is never inferred dead     | Only the legacy plain `checkpoint.json` primary                                              |
| v2             | Exact PID and creation-FILETIME registry                         | Only the legacy plain `checkpoint.json` primary                                              |
| v3             | The same exact process registry and current cold-recovery guards | Any fixed slot or an exact owned corrupt-raw archive; the Server still validates all content |

Reading v1 or v2 does not migrate it. A valid stopped legacy record can begin a new activation
only while its plain primary exists. Before committing that activation marker, spawning Server or
opening the multi-slot store, the activator atomically publishes the new Active record as catalog
v3. Fresh activations also publish v3 first. An earlier cohort whose parser admits only v1 and v2
therefore rejects an upgraded Site before changing the catalog, runtime marker or checkpoint
directory. A stopped v3 record may use backup or raw evidence because a prior all-invalid restore
can intentionally have no valid slot; content validation and the recovered/empty decision remain
with the current Server.

## State and publication

The existing private directory and exclusive `state.lock` remain the sole writer authority.
Three fixed slots are `checkpoint.json`, `checkpoint.previous-1.json` and
`checkpoint.previous-2.json`. Their names do not establish recency. A framed file contains the
five-byte magic consisting of `OBCK` followed by byte `1`, an eight-byte little-endian nonzero
generation, a 32-byte SHA-256 of the preceding magic/generation bytes followed by the payload,
and the unchanged payload bytes. The payload limit remains
2 MiB; the framing adds exactly 45 bytes. A legacy unframed file has generation zero.
Each new frame advances from the greatest generation among completely admitted valid slots. A
generation read from an invalid digest or domain-invalid payload is diagnostic only and never
advances publication. With no valid slot, ordering restarts at one; a completely valid slot at
`u64::MAX` still exhausts the counter and refuses publication.

Loading admits every slot through the private regular-file boundary, bounds its bytes, checks
its framing/digest and asks the Server to validate the complete payload. Missing files are
distinct from invalid files. Permission, identity, reparse and filesystem errors fail admission;
they never become an empty workspace. Pending bytes are not a committed slot.

The newest valid generation wins. Conflicting payloads with the same generation are invalid;
identical duplicate copies do not invent a different generation. If invalid copies are present,
the result carries a recovery notice. If no valid copy remains, existing invalid or archived
originals require an explicit empty-restoration notice; complete absence means a new workspace.

Publication follows these transitions under the retained state lock:

| Transition        | Guard                                                                | Effect                                                                                                                                               |
| ----------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Prepare           | Payload is validated and bounded; generation can advance             | Choose a missing slot, otherwise an invalid slot, otherwise the oldest valid slot. Write and flush one private pending file.                         |
| Preserve invalid  | Chosen existing slot is invalid                                      | Move its complete original file to a fresh private `checkpoint.corrupt-<32 lowercase hex>.raw` name; never overwrite or truncate the original bytes. |
| Commit            | Pending file is complete; any displaced invalid original is retained | Atomically replace the chosen slot. This replacement is the sole publication point.                                                                  |
| Fail or interrupt | Commit has not occurred                                              | Keep the preceding valid slots. Any already moved invalid original remains retained. Discard uncommitted pending bytes on the next store open.       |

A successful replacement retains at most three valid slots. Replacing the oldest valid slot
cannot destroy the newest preceding valid snapshot. Failure to preserve invalid bytes prevents
replacement. Generation exhaustion refuses publication and leaves data intact. An unchanged
newest framed payload needs no new publication; periodic sampling does not consume history with
identical copies.

Corrupt originals are user recovery data, outside the three-valid-copy budget. They stay in this
Site's private state directory. For a stopped named Site,
`orchbee session delete <name> --confirm` removes its owned corrupt raw archives with its other
owned checkpoint files. The default Site has no delete command; its raw archives remain until a
separately authorized repair removes them. They are not execution logs and must not be silently
garbage-collected. Deletion recognizes only the fixed store files and the exact owned corrupt-name
grammar; unknown files, linked entries and living owners retain the existing refusal boundary.

## Restore notices

The Server publishes a bounded private restore notice before making its managed Client endpoint
ready. The notice is tied to the exact durable activation generation and contains only a closed
outcome, never payload bytes or localized prose. A normal restoration clears the current notice.
Client activation reads the matching notice through the existing private runtime owner.
Notice publication failure prevents endpoint readiness. `WindowsCheckpointRestoreNotice` models
loading, publication/failure, readiness, Client connection and stale-generation reads as separate events.

Headless `site start` renders a localized warning on stderr while preserving its existing success
document and exit code. The initial TUI displays the same warning through its existing notice
surface. Reattachment can display the notice for the same activation. Stale generations do not
warn a new boot. This provides visible recovery feedback without reading terminal screens or
adding an external notification destination.

## Evidence and limits

The storage model starts from a v1, v2 or v3 catalog. Its `legacyPrimary` input means one admitted
plain primary; backup and raw names never satisfy that input. `PublishCatalogV3` maps to successful
atomic durable-record publication by the activator. Multi-slot prepare and load are disabled until
that publication. `RefuseOldCohort` changes only the trace observation while retaining catalog and
storage state. Runtime-marker preservation is outside this storage state vector and requires the
native old-cohort no-effect witness.

The storage model separates an abstract monotonic `publicationId` from the private frame generation.
The model uses `publicationId` only to distinguish each committed original across corruption and
archival; it is not serialized. The adapter maps that identity to the exact pinned slot bytes or
fresh raw archive name. Frame generation models only ordering among currently admitted valid slots,
including restart at one after all valid ordering evidence is lost. The model checks three slots,
invalid-original preservation, atomic commit and fallback; the Server's autosave model checks
final-stop ordering. Its adapter mapping treats a successful same-directory atomic replacement as
commit and claims process-crash recovery only after that result. Deterministic storage tests cover legacy bytes,
digest/format/domain rejection, ordering independent of filenames, generation conflicts,
preservation failure, pending interruption, exhaustion and unchanged publication. Native Windows
tests cover exclusive ownership, private paths, denied replacement and retained original bytes.

The adapter flushes the pending file before replacement but does not claim directory-entry
write-through. These checks do not prove storage hardware survives power loss, and a native machine
restart witness has not yet been run. Native process-crash/restart acceptance and visible notice
evidence are required separately. The Server's [Workbench checkpoint contract](../../../apps/orchbee-server/docs/workbench-checkpoint.md)
owns the payload, scheduling and exact physical-directory fences.
