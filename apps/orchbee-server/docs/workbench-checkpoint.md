# Workbench restart checkpoint

## Scope and ownership

`workbench_authority/checkpoint.rs` owns the private versioned projection of retained Workbench
configuration and its reconstruction into a fresh authority. The managed Windows Session is the
storage/startup consumer. The domain codec remains separate from Windows filesystem admission,
checkpoint publication and terminal relaunch. Named-Session routing and the additional recovery owners
remain separate obligations. No other platform adapter is introduced.

The checkpoint retains optional Project membership, Project labels, global local-directory paths,
Workspaces and their display order, terminal contexts, ordered
Tabs, groups, pinned state, split structure and ratios, Panel input policy and Presentation labels. It also
retains Launch Profile definitions and the allocation high-water counter, including deleted IDs.
All primary and child Presentations survive structurally but carry no terminal binding, launched program or launch failure.

It does not retain running processes, Session/Worker identities, controller grants, leases, Client
views/focus/zoom, request results, publication cursors, unfinished operations or terminal history.
Of Agent state it retains only per-Presentation Agent session references: a provider from a closed
table and that provider's native session id. Their recovery belongs to the corresponding runtime
owners. A checkpoint never authorizes replay of a terminal command or Git operation; resuming an
Agent types the provider's own resume command from the closed table and never replays retained
text.

## Model

| Operation              | Preconditions                                                                                                                    | Result and invariant                                                                                                                                              |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture                | Authority is not retired; committed graph is valid; every Directory and bound Presentation belongs to the supplied local Worker. | One bounded checkpoint; source authority is unchanged; process bindings and Worker identities are removed.                                                        |
| Decode                 | Strict current version 1 document, bounded bytes, normalized navigation, valid configuration graph, counters and directories.    | Complete retained configuration or a typed rejection; never partial or empty fallback.                                                                            |
| Reconstruct            | Caller supplies a fresh boot Session identity and its admitted local Worker.                                                     | Retained structural IDs and local paths, fresh directory ownership, unbound Presentations, fresh profile Session prefixes, revision 0 and empty transient owners. |
| Allocate after restore | Restored high-water counter permits advancement.                                                                                 | IDs remain above all previously allocated counters; exhaustion keeps the ordinary typed rejection.                                                                |

The caller owns freshness of the new Session identity. Profile IDs are `<sessionHex>:<counterHex>`:
the stored form retains only the counter, name and launch definition, and reconstruction uses
the new Session prefix. Old Session grants and profile IDs do not acquire authority over the new boot.
Names need not be unique; their existing ambiguous-selector semantics remain unchanged.

## Private format and validation

### Current normalized navigation

The writer and reader use only `orchbee.workbench.checkpoint` with numeric `version: 1`.
All retained fields are required, including `terminalDirectories`,
`projectOrder` and `workspaceOrder`; the one exception is `agentSessions`, described below. `checkpoint/navigation.rs` has one DTO: identity-sorted global
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
`lastIdentity` and each profile `counter` are canonical nonzero 32-character lowercase hex.
Every Presentation binding is `null`; `customLabel` is required-nullable.
`checkpoint/contexts.rs` owns the retained graph shape: Tab `automaticLabel` and `groupId`,
Panel `rightClick`, Split `ratio` and Presentation `binding` are explicit even at their default or
null values. The runtime-only `launchProgram` and `launchFailure` fields are not admitted into retained configuration.
Retained navigation stores the protocol Workspace shape, so Workspace `automaticLabel` keeps its
protocol omission rule: it appears only when true, and its absence restores a fixed name.
Optional `terminalDirectory` is retained the same way. The directory Server last sampled for each
first terminal is process-local and is not retained; see
[automatic Workspace names](../../orchbee/docs/native-location-labels.md#automatic-workspace-names).

### Per-terminal directory restoration

The required `terminalDirectories` list stores stable `presentationId` and validated `path` pairs.
Primary and child Presentations remain independent. Null lists, duplicate/foreign members and
invalid paths reject the entire document; the 2 MiB limit remains.

After request/background quiescence, clean stop takes a fresh bounded observation through the existing private viewer owner before Worker disposal. A valid terminal-reported directory takes precedence over the root process directory; child process directories never substitute for the root. The final physical membership and Worker generation must still match. Unknown detach remains owned by the stopped process observer, fails this stop attempt and preserves the preceding checkpoint; a later stop may settle that exact attachment.

During normal operation the existing process sampler caches only its already observed validated directory, with the exact Worker generation, Host terminal identity and sample start time. Cached capture rechecks current physical membership under the Session gate. Stale, removed or replaced bindings contribute no directory. An unavailable observation does not fabricate a directory or erase retained restart intent. Saving never opens a viewer, samples processes or reads screen contents.

Capture maps admitted observations to current Presentation identities. If observation is unavailable, retained restart intent remains; otherwise ordinary workspace/home defaults apply. Removed members disappear. Fresh launch prefers the retained directory, then workspace default; if that selected path no longer names a directory, Windows restore holds the Presentation instead of launching (see [Held restore](#held-restore)). This restoration policy does not alter explicit user launch validation. The shared PowerShell prompt synchronization also runs in restored encoded-command shells, so directory changes after a provider exits can be observed.

Evidence must cover exact identity/revision fences, primary/child separation, incompatible-format rejection, malformed rows, capture/reconstruct without live authority, unresolved viewer cleanup, actual Windows directory changes and a deleted saved directory. A process launch witness alone does not prove this directory lifecycle.

### Per-Presentation Agent session references

`checkpoint/agent_sessions.rs` owns the `agentSessions` list of `presentationId`, `provider` and
`sessionId` rows. `provider` is one id of the closed `AgentResumeProvider` table, and `sessionId`
must be valid in that provider's own id grammar; the
[provider table](agent-session-resume.md#provider-table) lists both. The id is stored verbatim,
never case-folded, as `RetainedAgentSession`. Each row names a distinct member Presentation that also has a
`terminalDirectories` row, and `(provider, sessionId)` pairs are distinct. A null list, a
non-member or duplicate Presentation, an unknown provider, an id invalid for its provider, a
duplicate pair, a row without a directory row or an unknown field rejects the entire document. An
older build therefore rejects a document with a provider it does not know and loses that layout
once.

The writer always emits the list. Unlike every other retained field it may be omitted on read,
where it means no references: an omitted list cannot name anything to resume, so a document written
before the list existed keeps its layout. A present list is validated strictly.

Reconstruction keeps the rows as pending restart intent on `WorkbenchAuthority`, keyed by
Presentation, the same way it keeps restart directories. `take_pending_agent_session` consumes an
entry without changing the Workbench revision. Capture writes, for each member in capture order, its
live reference if the runtime supplies one, else its still pending reference, and only when the
same checkpoint has that Presentation's directory row. The first member keeps a shared
`(provider, sessionId)`; later duplicates are dropped. Observed bindings must belong to the local
Worker and to current members, one per terminal. When recording is off the runtime supplies no
observation set and capture writes no rows, pending ones included. Removed members disappear.
[Agent session resume](agent-session-resume.md) owns which live reference the runtime supplies.

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

## Held restore

When the directory a target would start in (its retained directory, else the Workspace default)
does not name an accessible directory, Windows restore does not launch it. `hold_restored_terminal`
commits the Presentation unbound in its place with a `directory-unavailable` launch failure that
names that directory and whether a retained Agent reference still waits (only while resume is
enabled; otherwise the reference is consumed as before). The restart directory and Agent reference
stay pending on `WorkbenchAuthority`, so every capture while the Panel is held writes the same
directory row and Agent row, and launch facts stay unchanged.

Nothing retries automatically. `panel.restore-terminal` names the exact Context, Tab, Panel and
Presentation and is decided against the current hold, not the expected revision; it is not
journaled. `directory: saved` probes the held directory outside every gate. A directory that is
still unavailable answers `unavailable/directory-unavailable` and keeps the hold. An available one
binds and launches through the same reservation and 80x24 geometry as restore, then hands a still
pending Agent reference to the resume driver once the shell started. `directory: home` drops the
restart directory and Agent reference in the same critical section and launches with no cwd, so the
Worker home applies and the next capture records what the new shell reports. A request that no
longer names a held Presentation is rejected with `panel-unavailable`. Closing a held Panel is an
ordinary Panel close.

The resume driver runs once per Server lifetime: a Retry after it already ran leaves the reference
pending, and the next checkpoint and restart resume it. See
[Agent session resume](agent-session-resume.md).

## Terminal rebind preparation

`checkpoint/restore.rs` enumerates every unbound primary and child Presentation in Workspace Context, Tab and depth-first tree order. Each target carries the fresh Session/local Worker, exact member identities and separate retained/Workspace directory intent. Read-only registration visits every primary and child; destructive operations retain their own guards. An unconfigured Workspace without retained cwd uses Worker home.

`prepare_restored_terminal` rechecks Session, local Worker, membership, unbound status and the cwd
default, then validates and prepares the entire resulting projection/publication before mutation.
Dropping preparation changes nothing. Commit sets exactly one binding, with the launched program
its fresh reservation planned, and advances revision;
structural identities, allocator high-water, labels, order, groups, input policy and profiles remain
unchanged. Already-bound targets and duplicate physical terminal bindings are refused.

Preparation creates no process and replays no command. The managed coordinator prepares a fresh terminal reservation with the canonical binding, commits both under one lock, then dispatches and settles that exact physical request before exposing Clients. Restored snapshot bindings are the only attachment selection source; there is no reconstructed Open reference. Planning tests alone do not establish runtime integration.

## Managed Windows startup and clean stop

Managed startup opens the exclusive `SessionStore` below `LOCALAPPDATA` and parses its
committed copies before spawning a Worker. The storage owner selects the latest completely validated
payload from three slots. Invalid copies are retained and produce a visible restore notice; if all
copies are invalid, startup explicitly reports an empty restoration. Missing data means a new Workbench.
Private-path or filesystem admission failure still aborts startup. See the
[Windows checkpoint storage contract](../../../crates/local-runtime/docs/windows-checkpoint-storage.md).
Default and named activation publish the current version 1 durable record before storage use.
The Server admits the exact durable shape before opening the store, including pending-file cleanup.
`SessionStore::read_validated` treats every invalid payload as an unreadable copy: select a
valid slot or report an empty restoration, preserving invalid bytes until they are archived before
replacement. No domain parsing failure is a special fatal migration error; filesystem admission
errors still abort. Uncommitted pending-file cleanup remains unchanged.

`WorkbenchCheckpoint` is an admitted
configuration value: private graph-validation identities never become a runnable authority. Only
reconstruction with the fresh Session and admitted Worker creates the new authority.

Restoration deliberately launches the current default program from the Session's shared preferences,
not the program or Launch Profile previously used by each Panel. Checkpoints retain Launch Profile
definitions but no per-Presentation launch selection. An unreadable preference record therefore
restores terminals with the built-in system shell default; retained custom profiles do not select
themselves. This policy never replays a prior command.

Before launch IPC, the coordinator prepares all fresh shell reservations and Workbench bindings. Failure before dispatch leaves no Host/terminal; it terminates the creation-owned Worker and confirms exit. Each prepared request dispatches and settles once; failure/unknown outcomes remain explicit and are never retried automatically. Restored directories precede Workspace/home defaults; a target whose selected directory is missing or inaccessible is held, not launched. Geometry starts at 80x24 with 8x16 cells until resized. Primary and child members restore without an extra Tab. Undispatched requests settle unavailable at the deadline. Later boot failure stops maintenance, requests owned Host/terminal shutdown, confirms Worker/transport exit and stops observation. Unconfirmed cleanup retains recovery state; Worker termination alone does not prove Host cleanup.

Clean stop saves after request/background quiescence and before Worker disposal. Encoding or
publication failure prevents disposal and remains a failed stop; preceding valid snapshots remain,
and any displaced invalid original remains in its raw archive. Clean stop first joins the Agent
resume driver, which starts after restoration settles and the process observer starts, so a
reference not yet attempted stays pending for the final capture
([Agent session resume](agent-session-resume.md#delivery)). It then drains the single
background saver and performs fresh directory capture
and final publication before Worker disposal. A late background write cannot replace this final snapshot.
A stop after the Worker exited has nothing to sample. Its final capture keeps each terminal's last
cached directory, fenced to its exact physical generation as a background save fences it, else its
restart directory, and it publishes only once that Worker's Host is observed absent.
Fresh physical creation establishes terminal bindings. Client and history recovery remain separate. Activation markers retain their existing recovery semantics on failed boot.

## Running save model

The managed Windows Server owns one background saver after terminal restoration has settled. It watches committed Workbench revision; each change restarts a 500ms debounce.
Every 30 seconds it also captures cached directories. Agent session rows come from the same
capture. No terminal screen, history, Agent conversation or live controller state enters the payload.
The pending work is one latest snapshot, not an unbounded queue.
When every retained copy was invalid, the saver retains the exact initial empty checkpoint in
memory and does not publish that baseline. A later layout or cached directory
change must alter the complete checkpoint before automatic publication can replace an invalid slot.
This suppression applies only to unattended background saves. An explicit `server stop --confirm`
confirms the current workspace, so its fresh final capture may publish the unchanged empty state;
the storage owner archives the displaced invalid bytes before that commit.

Capture takes the existing Session gate only to build a bounded configuration value and then releases
it. Filesystem validation, preservation, flush and replacement happen outside the gate, on the single
writer. Revision changes during I/O remain dirty and schedule another capture. A failed save keeps
the previous valid copies and a bounded error state; it does not terminate a terminal or authorize a
fresh Session. The saver must settle before final clean-stop capture. Failure to join it prevents stop
completion rather than allowing two storage writers.

## Earlier layouts

The Server keeps earlier layouts so a person can reopen Workspaces and Tabs they closed. The
history policy lives in `layout_history.rs`; it sees only committed saves and a narrow store port,
so the save path above is unchanged. After each committed save (background or clean stop) it
records the saved checkpoint as an entry when the layout differs from the newest entry and at least
15 minutes have passed since that entry, or the history is empty. Layout means Workspaces, Tabs
and Panels by identity, label and order; focus, scroll, split ratios and directories are not part
of it. The index of kept layouts is read once at startup, so a save never reads entries back. A
clock behind the newest entry records nothing. A failed history write is one
`operation=layout-history` diagnostic per failure kind and never fails the save. The newest 48
entries are kept; the interval is a constant, not a setting. The
[storage owner](../../../crates/local-runtime/docs/windows-checkpoint-storage.md#earlier-layouts)
defines files and atomicity.

`workbench.layout-history` lists entries newest first, each with its save time, Workspace and Tab
counts and how many of them are not open now (`read` permission). `reopen` names an entry by save
time (navigation and terminal-create permission). In one commit it adds only the Workspaces and
Tabs that are not open now, with their saved split layout and labels; nothing open is closed, moved
or relabeled. Product identities are never reused, because Close keeps tombstones that fence a
closed identity, so reopened structure takes fresh identities. The Server records the lineage of
each reopened Workspace and Tab, the saved identity it stands in for, and both the listing and
`reopen` count an item as open when it or a reopened item of the same lineage is open. Reopening
the same entry again, or any entry that names the same Workspaces or Tabs, therefore answers
`nothing-to-reopen`. The lineage lives with the running Server and keeps the newest 4096 reopened
identities; after a Server restart an entry saved before a reopen counts those items as not open
again.

- a missing Workspace comes back whole at the end of the Workspace order, reusing an existing
  Directory with the same path, inside its Project when that Project still admits it, otherwise
  ungrouped; its Tab groups and pins come back with it;
- a missing Tab of an open Workspace returns at its saved index, clamped to the current strip after
  the pinned prefix and outside groups, unpinned and ungrouped; a Close that holds that Workspace
  answers `unavailable/busy`;
- a result that would exceed the Workbench bounds is `rejected/resource-limit` and opens nothing;
  an entry with nothing missing answers `nothing-to-reopen`.

Each reopened Presentation keeps its saved directory as restart intent. After the commit the Server
probes those directories outside every gate; a missing one gets the [held restore](#held-restore)
state, and the rest bind and launch through restore's reservation path. No process, Agent or
screen comes back. The result names the first reopened Tab; interactive Clients select it, and the
CLI does not move focus.

## Evidence

Deterministic tests exercise capture/reconstruct with independent graph expectations, nested
splits and child Presentations, profile rebasing, allocation after deleted identities, foreign
ownership, corruption, duplicate/unknown fields, binding injection, graph failures and size limits.
Current-shape fixtures preserve ungrouped membership, display order, unused/shared Directories,
cwd. Strict fixtures cover missing fields and incompatible
versions. Agent session fixtures cover every row fault, each provider's id grammar, the omitted list, round trips into pending
intent, carry-over, take, live override, the directory-row requirement, first-member dedupe and
recording off. Native storage and Server-load witnesses verify fallback/empty notices and exact-byte
preservation for invalid copies, including a preceding version 7 document. Direct Server admission
also preserves pending files when the durable catalog is unsupported.

Layout history tests drive the record rule under an injected clock (change and interval, the
48-entry bound, oldest first, reload) and reopen against a Session missing a Tab or a Workspace,
including the hold of a missing directory; the storage test covers pruning and a renamed frame.

Coordinator tests cover joint physical/graph preparation, grouped/pinned/child membership, binding reattachment and explicit unavailable outcomes without replay. Windows storage/process tests cover private
replacement, locking and exact-handle termination. Packaged Windows clean-stop/restart acceptance
compares retained layouts/profiles, fresh runtime identities, shell effects and TUI binding reattachment;
blocked replacement must preserve old bytes and live terminals until an explicit successful retry,
and invalid copies must retain their exact original bytes and produce the declared fallback/empty
notice without bypassing Session-owner admission. Cached-capture tests cover physical replacement,
out-of-order samples and removed members. Scheduling tests use explicit times and delayed writes to
prove debounce, periodic refresh, bounded pending work and final-stop ordering. Native crash/restart
acceptance must compare multiple Tabs, nested splits, labels, directories;
machine reboot and interactive load remain separate evidence obligations.
