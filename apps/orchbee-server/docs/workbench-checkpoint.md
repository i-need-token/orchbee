# Workbench restart checkpoint

## Scope and ownership

`workbench_authority/checkpoint.rs` owns the private versioned projection of retained Workbench
configuration and its reconstruction into a fresh authority. The managed Windows Site is the
intended storage/startup consumer. The codec is implemented separately from filesystem admission,
checkpoint publication, terminal relaunch and named-Site CLI routing; those runtime integrations
remain required before claiming session persistence. No other platform adapter is introduced.

The checkpoint retains Projects, local directory paths, Workspaces, terminal contexts, ordered
Tabs, groups, pinned state, split structure, Panel input policy and Presentation labels. It also
retains Launch Profile definitions and the allocation high-water counter, including deleted IDs.
All primary and child Presentations survive structurally but carry no terminal binding.

It does not retain running processes, Site/Worker identities, controller grants, leases, Client
views/layout overrides, request results, publication cursors, unfinished operations, Agent state
or terminal history. Their recovery belongs to the corresponding runtime owners. A checkpoint
never authorizes replay of a terminal command or Git operation.

## Model

| Operation              | Preconditions                                                                                                                    | Result and invariant                                                                                                                                           |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture                | Authority is not retired; committed graph is valid; every Directory and bound Presentation belongs to the supplied local Worker. | One bounded checkpoint; source authority is unchanged; process bindings and Worker identities are removed.                                                     |
| Decode                 | Strict version 1 document, bounded bytes, valid configuration graph and counters.                                                | Complete retained configuration or a typed rejection; never partial or empty fallback.                                                                         |
| Reconstruct            | Caller supplies a fresh boot Site identity and its admitted local Worker.                                                        | Retained structural IDs and local paths, fresh directory ownership, unbound Presentations, fresh profile Site prefixes, revision 0 and empty transient owners. |
| Allocate after restore | Restored high-water counter permits advancement.                                                                                 | IDs remain above all previously allocated counters; exhaustion keeps the ordinary typed rejection.                                                             |

The caller owns freshness of the new Site identity. Profile IDs are `<siteHex>:<counterHex>`:
the stored form retains only the counter, name and launch definition, and reconstruction uses
the new Site prefix. Old Site grants and profile IDs do not acquire authority over the new boot.
Names need not be unique; their existing ambiguous-selector semantics remain unchanged.

## Private format and validation

The schema discriminator is `orchbee.workbench.checkpoint`, with numeric `version: 1`.
`lastIdentity` and each profile `counter` are canonical nonzero 32-character lowercase hex.
Navigation uses local directories containing only `directoryId` and `path`; its other retained
objects use their shared typed contracts. Terminal contexts require every Presentation binding
to be `null`. Unknown or duplicate fields and unsupported versions are errors.

The encoded checkpoint limit is 2 MiB. The reconstructed Snapshot must pass the existing complete
graph validator and 1 MiB wire bound; the catalog keeps its 64-entry and 256 KiB bounds. All owned
structural/profile counters must be distinct and at most `lastIdentity`. References are validated
by the Snapshot owner. A forged bound Presentation, foreign Worker during capture, invalid path,
missing membership, malformed launch definition or high-water counter below a retained ID rejects the whole
operation. JSON encoding alone does not establish graph validity.

The domain performs no filesystem access or process launch. Storage must admit private Windows
paths, publish complete records, distinguish absence from corruption, and retain failure evidence.
Startup restoration must establish new physical bindings and account for interrupted operations;
returning an unbound authority from this codec is not proof that a usable session has resumed.
Format changes require an explicit version/migration decision; there is no permissive fallback.

## Terminal rebind preparation

`checkpoint/restore.rs` enumerates every unbound primary and child Presentation in context, Tab
and depth-first tree order. Each private restore target carries its fresh Site/local Worker and
exact context/Tab/Panel/Presentation membership. Its cwd default is the owning Workspace's primary
directory; standalone and unconfigured Workspace targets have no explicit directory. This is not
a saved observation of each terminal's last cwd; that belongs to runtime checkpoint capture.

`prepare_restored_terminal` rechecks Site, local Worker, membership, unbound status and the cwd
default, then validates and prepares the entire resulting projection/publication before mutation.
Dropping preparation changes nothing. Commit sets exactly one binding and advances revision;
structural identities, allocator high-water, labels, order, groups, input policy and profiles remain
unchanged. Already-bound targets and duplicate physical terminal bindings are refused.

This preparation does not create a process or replay a command. The managed coordinator must
prepare a fresh terminal reservation and this binding together, commit both under the same lock,
then dispatch and settle the exact physical request. It must also reconstruct standalone Open
selection before exposing Clients. Planning and binding tests do not establish that runtime integration.

## Evidence

Deterministic tests exercise capture/reconstruct with independent graph expectations, nested
splits and child Presentations, profile rebasing, allocation after deleted identities, foreign
ownership, corruption, duplicate/unknown fields, binding injection, graph failures and size limits.
They establish the codec/reconstruction boundary only. Windows filesystem, clean restart, crash
recovery, terminal/Agent relaunch and named-Site lifecycle require their own integration evidence.
