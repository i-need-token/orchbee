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
All primary and child Presentations survive structurally but carry no terminal binding.
Version 2 also retains validated external provider references; see [Agent restart references](agent-restart-reference.md).

It does not retain running processes, Site/Worker identities, controller grants, leases, Client
views/focus/zoom, request results, publication cursors, unfinished operations, Agent state
or terminal history. Their recovery belongs to the corresponding runtime owners. A checkpoint
never authorizes replay of a terminal command or Git operation.

## Model

| Operation              | Preconditions                                                                                                                        | Result and invariant                                                                                                                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture                | Authority is not retired; committed graph is valid; every Directory and bound Presentation belongs to the supplied local Worker.     | One bounded checkpoint; source authority is unchanged; process bindings and Worker identities are removed.                                                     |
| Decode                 | Strict version 1–7 document, bounded bytes, version-matched navigation, valid configuration graph, counters and provider references. | Complete retained configuration or a typed rejection; never partial or empty fallback.                                                                         |
| Reconstruct            | Caller supplies a fresh boot Site identity and its admitted local Worker.                                                            | Retained structural IDs and local paths, fresh directory ownership, unbound Presentations, fresh profile Site prefixes, revision 0 and empty transient owners. |
| Allocate after restore | Restored high-water counter permits advancement.                                                                                     | IDs remain above all previously allocated counters; exhaustion keeps the ordinary typed rejection.                                                             |

The caller owns freshness of the new Site identity. Profile IDs are `<siteHex>:<counterHex>`:
the stored form retains only the counter, name and launch definition, and reconstruction uses
the new Site prefix. Old Site grants and profile IDs do not acquire authority over the new boot.
Names need not be unique; their existing ambiguous-selector semantics remain unchanged.

## Private format and validation

### Normalized navigation and historical migration

The writer emits `orchbee.workbench.checkpoint` with numeric `version: 7`.
`checkpoint/navigation.rs` owns separate current and historical navigation DTOs. Versions 6 and 7 store
global, identity-sorted `directories`, `projects` and `workspaces` arrays. Project rows contain only
`projectId` and `label`; every Workspace requires a nullable `projectId`. Local Directory rows
contain only `directoryId` and `path`, with fresh Worker ownership supplied during reconstruction.
The explicit `workspaceOrder` is a permutation of every Workspace identity. Ungrouped Workspaces,
empty Projects, shared Directory references and unused Directories survive capture and restart.
The normalized graph permits at most 65,536 Directories, 1,023 Workspaces and 256 Workspaces in each
Project, also subject to the encoded bounds below.

Version 7 adds independent `projectOrder`: a nonempty value must be an exact permutation of every
Project identity. Omission or empty uses canonical ID order. Version 6 remains readable without
Project order; a nonempty Project order labeled version 6 rejects. Older cohorts reject version 7
explicitly and retain committed files; no automatic downgrade or order-dropping conversion exists.
Workspace and Project order survive capture/restart while the identity registries stay sorted.

Versions 1–5 must use the historical shape: each Project owns its `directories` and `workspaces`,
and historical Workspace rows have no `projectId`. Migration first validates the original
per-Project array ordering, 256-Directory and 256-Workspace limits, and primary-directory ownership
within that same Project. Flattening cannot make an invalid cross-Project reference valid.
It then assigns each Workspace its enclosing Project, preserves every identity and Directory,
sorts the global arrays, and runs complete graph and identity/high-water validation. An explicit
historical display order is retained; omitted or empty order becomes the historical Project/Workspace
traversal order, which may differ from global Workspace identity order.

Navigation shape must match its declared version. Injected current fields into historical rows,
historical navigation labeled version 6 or 7, unknown or duplicate fields, duplicate identities, missing
references and a high-water counter below a retained identity reject the entire document. The
codec never converts malformed input into an empty Workbench. The storage consumer's explicit
all-copies-invalid notice below is a separate recovery decision.

Historical normalization can add enough fields to exceed the unchanged 1 MiB Snapshot wire limit,
even when the original version 1–5 Snapshot fitted that limit and its checkpoint fits 2 MiB.
`HistoricalMigrationTooLarge { version, actual, maximum }` explicitly refuses that migration;
it does not admit an oversized graph, discard retained content or increase either bound. Source
DTOs still reject unknown and duplicate fields before normalization. There is no automatic
downgrade or conversion.

Versions 2–7 require `agentSessions`, versions 3–7 require `terminalDirectories`, and versions 4–7
require `managedNames`; earlier versions reject those fields. Versions 5–7 retain canonical split
ratios; versions 1–4 restore equal ratios and reject injected nondefault weights. See
[shared ratios](shared-split-ratios.md). Each of at most 512 unique name/Presentation claims carries
only `presentationId`, `name` and `providerId`, and requires a matching admitted official provider
reference. See [managed names](agent-canonical-model.md#managed-name-restart).
`lastIdentity` and each profile `counter` are canonical nonzero 32-character lowercase hex.
Terminal contexts require every Presentation binding to be `null`. Unsupported versions reject.

### Per-terminal directory restoration

Version 3 requires a `terminalDirectories` list of stable `presentationId` and validated `path` pairs. Primary and child Presentations remain independent. Versions 1 and 2 remain readable only without this field; null, duplicate/foreign members and invalid paths reject the entire document. The existing 2 MiB limit remains; older cohorts reject version 3.

After request/background quiescence, clean stop takes a fresh bounded observation through the existing private viewer owner before Worker disposal. A valid terminal-reported directory takes precedence over the root process directory; child process directories never substitute for the root. The final physical membership and Worker generation must still match. Unknown detach remains owned by the stopped process observer, fails this stop attempt and preserves the preceding checkpoint; a later stop may settle that exact attachment.

During normal operation the existing process sampler caches only its already observed validated directory, with the exact Worker generation, Host terminal identity and sample start time. Cached capture rechecks current physical membership under the Site gate. Stale, removed or replaced bindings contribute no directory. An unavailable observation does not fabricate a directory or erase retained restart intent. Saving never opens a viewer, samples processes or reads screen contents.

Capture maps admitted observations to current Presentation identities. If observation is unavailable, retained restart intent remains; otherwise ordinary workspace/home defaults apply. Removed members disappear. Fresh launch prefers the retained directory, then workspace default; if that selected path no longer names a directory, Windows restore uses the Worker's home default. This restoration policy does not alter explicit user launch validation. The shared PowerShell prompt synchronization also runs in restored encoded-command shells, so directory changes after a provider exits can be observed.

Evidence must cover exact identity/revision fences, primary/child separation, old-format compatibility, malformed rows, capture/reconstruct without live authority, unresolved viewer cleanup, actual Windows directory changes and a deleted saved directory. A process launch witness alone does not prove this directory lifecycle.

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
exact context/Tab/Panel/Presentation membership and separate retained/Workspace directory intent.
Read-only physical registration visits every primary and child in every context; destructive tree
guards remain unchanged. Standalone/unconfigured targets without retained cwd use Worker home.

`prepare_restored_terminal` rechecks Site, local Worker, membership, unbound status and the cwd
default, then validates and prepares the entire resulting projection/publication before mutation.
Dropping preparation changes nothing. Commit sets exactly one binding and advances revision;
structural identities, allocator high-water, labels, order, groups, input policy and profiles remain
unchanged. Already-bound targets and duplicate physical terminal bindings are refused.

This preparation does not create a process or replay a command. The managed coordinator must
prepare a fresh terminal reservation and this binding together, commit both under the same lock,
then dispatch and settle the exact physical request. It must also reconstruct standalone Open
selection before exposing Clients. Planning and binding tests do not establish that runtime integration.

## Managed Windows startup and clean stop

Managed startup opens the exclusive `WindowsSessionStore` below `LOCALAPPDATA` and parses its
committed copies before spawning a Worker. The storage owner selects the latest completely validated
payload from three slots. Invalid copies are retained and produce a visible restore notice; if all
copies are invalid, startup explicitly reports an empty restoration. Missing data means a new Workbench.
Private-path or filesystem admission failure still aborts startup. See the
[Windows checkpoint storage contract](../../../crates/local-site-runtime/docs/windows-checkpoint-storage.md).
Default and named activation publish catalog v5 before checkpoint-v7 storage use; the catalog reader
accepts v1–v5. Earlier readers limited to v4 refuse the new record, including after clean stop.
The Server retains the admitted durable generation before opening its store, so a direct Server
entry also rejects an unsupported catalog before pending-file cleanup.

`WindowsSessionStore::try_read_validated` separates ordinary invalid copies (`Ok(None)`) from
fatal domain admission (`Err`); `read_validated` retains its infallible-callback behavior.
A historical migration-limit refusal in any complete storage-admitted slot aborts Server loading,
even when another older or newer valid slot exists. It propagates the typed checkpoint error as the
I/O error source, preserves every committed copy and publishes no recovered/empty notice. No Worker,
restored authority or background saver starts. Ordinary corruption still uses the existing
fallback/empty policy. Store-open cleanup of uncommitted pending files remains unchanged.
`WorkbenchCheckpoint` is an admitted
configuration value: private graph-validation identities never become a runnable authority. Only
reconstruction with the fresh Site and admitted Worker creates the new authority.

Before the first launch IPC, the coordinator prepares all fresh shell/provider reservations and
Workbench bindings. Failure at this point leaves no Host or terminal; the creation-owned Worker
is terminated and its exit confirmed. Each prepared request is then dispatched and settled once;
launch failure or unknown outcome stays explicit and is never retried automatically. Retained terminal directories, workspace/home
defaults and initial geometry of 80x24 at 8x16 cells apply until the Client resizes.
The first restored standalone primary supplies the retained Open result instead of creating an
extra Tab. A missing saved/configured directory falls back to Worker home during restoration.
Open checks complete primary/child membership, including grouped and pinned Tabs; this read does
not relax the separate guards on destructive operations. Requests not sent before the startup
deadline settle as unavailable. A later startup failure stops maintenance and requests Worker-owned
Host/terminal shutdown, then confirms Worker/carrier exit and stops ingress. Unconfirmed cleanup
retains recovery-required activation state; terminating a Worker alone is not Host cleanup.

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
Historical fixtures remain in versions 1–5's nested shape. Migration witnesses include interleaved
global identities, explicit and implicit historical display order, unused Directories, cross-Project
primary-directory rejection before flattening, per-Project limits, duplicate identities, high-water
failures and version/field mismatches. Version 6/7 capture/restart preserves ungrouped membership,
directory references and available Agent, cwd and managed-name restart intent.
Constructed version 1–5 fixtures compare a historical Snapshot at exactly 1 MiB with its normalized
over-limit form and an exact-limit fitting control. Native storage and Server-load witnesses cover
fatal refusal with older/newer usable copies or no fallback, typed error propagation, unchanged
committed bytes and ordinary corruption recovery. Direct Server admission also preserves pending
files when the durable catalog version is unsupported.
Coordinator tests cover joint physical/graph preparation, grouped/pinned/child membership, Open
reuse and explicit unavailable outcomes without replay. Windows storage/process tests cover private
replacement, locking and exact-handle termination. Packaged Windows clean-stop/restart acceptance
compares retained layouts/profiles, fresh runtime identities, shell effects and TUI Open reuse;
blocked replacement must preserve old bytes and live terminals until an explicit successful retry,
and invalid copies must retain their exact original bytes and produce the declared fallback/empty
notice without bypassing Site-owner admission. Cached-capture tests cover physical replacement,
out-of-order samples and removed members. Scheduling tests use explicit times and delayed writes to
prove debounce, periodic refresh, bounded pending work and final-stop ordering. Native crash/restart
acceptance must compare multiple Tabs, nested splits, labels, directories and provider resume references;
real provider continuity, machine reboot and interactive load remain separate evidence obligations.
