# Workbench restart checkpoint

## Scope and ownership

`workbench_authority/checkpoint.rs` owns the private versioned projection of retained Workbench
configuration and its reconstruction into a fresh authority. The managed Windows Site is the
storage/startup consumer. The domain codec remains separate from Windows filesystem admission,
checkpoint publication and terminal relaunch. Named-Site routing and the additional recovery owners
remain separate obligations. No other platform adapter is introduced.

The checkpoint retains optional Project membership, Project labels, global local-directory paths,
Workspaces and their display order, terminal contexts, ordered
Tabs, groups, pinned state, split structure and ratios, Panel input policy and Presentation labels. It also
retains Launch Profile definitions and the allocation high-water counter, including deleted IDs.
All primary and child Presentations survive structurally but carry no terminal binding or launch failure.
It also retains validated external provider references; see [Agent restart references](agent-restart-reference.md).

It does not retain running processes, Site/Worker identities, controller grants, leases, Client
views/focus/zoom, request results, publication cursors, unfinished operations, Agent state
or terminal history. Their recovery belongs to the corresponding runtime owners. A checkpoint
never authorizes replay of a terminal command or Git operation.

## Model

| Operation              | Preconditions                                                                                                                         | Result and invariant                                                                                                                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture                | Authority is not retired; committed graph is valid; every Directory and bound Presentation belongs to the supplied local Worker.      | One bounded checkpoint; source authority is unchanged; process bindings and Worker identities are removed.                                                     |
| Decode                 | Strict current version 1 document, bounded bytes, normalized navigation, valid configuration graph, counters and provider references. | Complete retained configuration or a typed rejection; never partial or empty fallback.                                                                         |
| Reconstruct            | Caller supplies a fresh boot Site identity and its admitted local Worker.                                                             | Retained structural IDs and local paths, fresh directory ownership, unbound Presentations, fresh profile Site prefixes, revision 0 and empty transient owners. |
| Allocate after restore | Restored high-water counter permits advancement.                                                                                      | IDs remain above all previously allocated counters; exhaustion keeps the ordinary typed rejection.                                                             |

The caller owns freshness of the new Site identity. Profile IDs are `<siteHex>:<counterHex>`:
the stored form retains only the counter, name and launch definition, and reconstruction uses
the new Site prefix. Old Site grants and profile IDs do not acquire authority over the new boot.
Names need not be unique; their existing ambiguous-selector semantics remain unchanged.

## Private format and validation

### Current normalized navigation

The writer and reader use only `orchbee.workbench.checkpoint` with numeric `version: 1`.
All retained fields are required, including `agentSessions`, `terminalDirectories`, `managedNames`,
`projectOrder` and `workspaceOrder`. `checkpoint/navigation.rs` has one DTO: identity-sorted global
`directories`, `projects` and `workspaces` arrays. Project rows contain `projectId` and `label`;
Workspace `projectId` is required-nullable. Directory rows contain `directoryId` and `path`, with
fresh Worker ownership supplied during reconstruction. Workspace order is a complete permutation;
empty Project order uses canonical ID order, otherwise it is a complete permutation.
Ungrouped Workspaces, empty Projects, shared or unused Directories and both display orders survive.
The graph permits at most 65,536 Directories, 1,024 Workspaces and 256 Workspaces per Project,
subject to the unchanged encoded bounds.

Unknown, duplicate or missing fields, nested historical navigation, unsupported versions, invalid
references and insufficient identity high-water reject the entire document. No migration or
historical normalization occurs. See the repository [data-boundary version rule](../../../docs/ARCHITECTURE.md#data-boundaries).
Existing files labeled versions 2–7, including the preceding writer's version 7, become unreadable;
relabeling a file is not a supported conversion. The managed recovery policy below preserves them.

Canonical split ratios remain retained; see [shared ratios](shared-split-ratios.md).
Each of at most 512 unique name/Presentation claims carries `presentationId`, `name` and `providerId`
and requires a matching admitted official provider reference; see [managed names](agent-canonical-model.md#managed-name-restart).
`lastIdentity` and each profile `counter` are canonical nonzero 32-character lowercase hex.
Every Presentation binding is `null`; `customLabel` is required-nullable.
`checkpoint/contexts.rs` owns the retained graph shape: Tab `automaticLabel` and `groupId`,
Panel `rightClick`, Split `ratio` and Presentation `binding` are explicit even at their default or
null values. The runtime-only `launchFailure` field is not admitted into retained configuration.

### Per-terminal directory restoration

The required `terminalDirectories` list stores stable `presentationId` and validated `path` pairs.
Primary and child Presentations remain independent. Null lists, duplicate/foreign members and
invalid paths reject the entire document; the 2 MiB limit remains.

After request/background quiescence, clean stop takes a fresh bounded observation through the existing private viewer owner before Worker disposal. A valid terminal-reported directory takes precedence over the root process directory; child process directories never substitute for the root. The final physical membership and Worker generation must still match. Unknown detach remains owned by the stopped process observer, fails this stop attempt and preserves the preceding checkpoint; a later stop may settle that exact attachment.

During normal operation the existing process sampler caches only its already observed validated directory, with the exact Worker generation, Host terminal identity and sample start time. Cached capture rechecks current physical membership under the Site gate. Stale, removed or replaced bindings contribute no directory. An unavailable observation does not fabricate a directory or erase retained restart intent. Saving never opens a viewer, samples processes or reads screen contents.

Capture maps admitted observations to current Presentation identities. If observation is unavailable, retained restart intent remains; otherwise ordinary workspace/home defaults apply. Removed members disappear. Fresh launch prefers the retained directory, then workspace default; if that selected path no longer names a directory, Windows restore uses the Worker's home default. This restoration policy does not alter explicit user launch validation. The shared PowerShell prompt synchronization also runs in restored encoded-command shells, so directory changes after a provider exits can be observed.

Evidence must cover exact identity/revision fences, primary/child separation, incompatible-format rejection, malformed rows, capture/reconstruct without live authority, unresolved viewer cleanup, actual Windows directory changes and a deleted saved directory. A process launch witness alone does not prove this directory lifecycle.

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
Format changes follow the repository data-boundary rule; there is no permissive fallback.

## Terminal rebind preparation

`checkpoint/restore.rs` enumerates every unbound primary and child Presentation in Workspace Context, Tab and depth-first tree order. Each target carries the fresh Site/local Worker, exact member identities and separate retained/Workspace directory intent. Read-only registration visits every primary and child; destructive operations retain their own guards. An unconfigured Workspace without retained cwd uses Worker home.

`prepare_restored_terminal` rechecks Site, local Worker, membership, unbound status and the cwd
default, then validates and prepares the entire resulting projection/publication before mutation.
Dropping preparation changes nothing. Commit sets exactly one binding and advances revision;
structural identities, allocator high-water, labels, order, groups, input policy and profiles remain
unchanged. Already-bound targets and duplicate physical terminal bindings are refused.

Preparation creates no process and replays no command. The managed coordinator prepares a fresh terminal reservation with the canonical binding, commits both under one lock, then dispatches and settles that exact physical request before exposing Clients. Restored snapshot bindings are the only attachment selection source; there is no reconstructed Open reference. Planning tests alone do not establish runtime integration.

## Managed Windows startup and clean stop

Managed startup opens the exclusive `WindowsSessionStore` below `LOCALAPPDATA` and parses its
committed copies before spawning a Worker. The storage owner selects the latest completely validated
payload from three slots. Invalid copies are retained and produce a visible restore notice; if all
copies are invalid, startup explicitly reports an empty restoration. Missing data means a new Workbench.
Private-path or filesystem admission failure still aborts startup. See the
[Windows checkpoint storage contract](../../../crates/local-site-runtime/docs/windows-checkpoint-storage.md).
Default and named activation publish the current version 1 durable record before storage use.
The Server admits the exact durable shape before opening the store, including pending-file cleanup.
`WindowsSessionStore::read_validated` treats every invalid payload as an unreadable copy: select a
valid slot or report an empty restoration, preserving invalid bytes until they are archived before
replacement. No domain parsing failure is a special fatal migration error; filesystem admission
errors still abort. Uncommitted pending-file cleanup remains unchanged.

`WorkbenchCheckpoint` is an admitted
configuration value: private graph-validation identities never become a runnable authority. Only
reconstruction with the fresh Site and admitted Worker creates the new authority.

Before launch IPC, the coordinator prepares all fresh shell/provider reservations and Workbench bindings. Failure before dispatch leaves no Host/terminal; it terminates the creation-owned Worker and confirms exit. Each prepared request dispatches and settles once; failure/unknown outcomes remain explicit and are never retried automatically. Restored directories precede Workspace/home defaults; missing saved/configured directories fall back to Worker home. Geometry starts at 80x24 with 8x16 cells until resized. Primary and child members restore without an extra Tab. Undispatched requests settle unavailable at the deadline. Later boot failure stops maintenance, requests owned Host/terminal shutdown, confirms Worker/transport exit and stops observation. Unconfirmed cleanup retains recovery state; Worker termination alone does not prove Host cleanup.

Clean stop saves after request/background quiescence and before Worker disposal. Encoding or
publication failure prevents disposal and remains a failed stop; preceding valid snapshots remain,
and any displaced invalid original remains in its raw archive. Clean stop first drains the single
background saver, then performs fresh directory capture
and final publication before Worker disposal. A late background write cannot replace this final snapshot.
Fresh physical creation seeds only the retained provider reference, never live Agent state or source
authority. Client and history recovery remain separate. Completed managed names restore
after fresh physical creation and reference seeding, before Client publication; ordinary names and
pending launches do not. Clean capture composes current Names claims with exact physical references. Activation markers retain their existing recovery semantics on failed boot.

## Running save model

The managed Windows Server owns one background saver after terminal restoration and reference/name
seeding have settled. It watches committed Workbench revision; each change restarts a 500ms debounce.
Every 30 seconds it also captures cached directories and current exact-generation provider resume
references. No terminal screen, history, Agent conversation or live controller state enters the payload.
The pending work is one latest snapshot, not an unbounded queue.
When every retained copy was invalid, the saver retains the exact initial empty checkpoint in
memory and does not publish that baseline. A later layout, cached directory or provider-reference
change must alter the complete checkpoint before automatic publication can replace an invalid slot.
This suppression applies only to unattended background saves. An explicit `server stop --confirm`
confirms the current workspace, so its fresh final capture may publish the unchanged empty state;
the storage owner archives the displaced invalid bytes before that commit.

Capture takes the existing Site gate only to build a bounded configuration value and then releases
it. Filesystem validation, preservation, flush and replacement happen outside the gate, on the single
writer. Revision changes during I/O remain dirty and schedule another capture. A failed save keeps
the previous valid copies and a bounded error state; it does not terminate a terminal or authorize a
fresh Site. The saver must settle before final clean-stop capture. Failure to join it prevents stop
completion rather than allowing two storage writers.

## Evidence

Deterministic tests exercise capture/reconstruct with independent graph expectations, nested
splits and child Presentations, profile rebasing, allocation after deleted identities, foreign
ownership, corruption, duplicate/unknown fields, binding injection, graph failures and size limits.
Current-shape fixtures preserve ungrouped membership, display order, unused/shared Directories,
Agent references, cwd and managed names. Strict fixtures cover missing fields and incompatible
versions. Native storage and Server-load witnesses verify fallback/empty notices and exact-byte
preservation for invalid copies, including a preceding version 7 document. Direct Server admission
also preserves pending files when the durable catalog is unsupported.

Coordinator tests cover joint physical/graph preparation, grouped/pinned/child membership, binding reattachment and explicit unavailable outcomes without replay. Windows storage/process tests cover private
replacement, locking and exact-handle termination. Packaged Windows clean-stop/restart acceptance
compares retained layouts/profiles, fresh runtime identities, shell effects and TUI binding reattachment;
blocked replacement must preserve old bytes and live terminals until an explicit successful retry,
and invalid copies must retain their exact original bytes and produce the declared fallback/empty
notice without bypassing Site-owner admission. Cached-capture tests cover physical replacement,
out-of-order samples and removed members. Scheduling tests use explicit times and delayed writes to
prove debounce, periodic refresh, bounded pending work and final-stop ordering. Native crash/restart
acceptance must compare multiple Tabs, nested splits, labels, directories and provider resume references;
real provider continuity, machine reboot and interactive load remain separate evidence obligations.
