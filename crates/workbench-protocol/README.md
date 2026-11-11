# Workbench protocol

## Result admission

Action/result pairing belongs to this crate. The result-family `answers` methods enumerate
their actions and retain the required identity, order, cursor and payload echoes.
`ClientViewAction::admits`, `WorkbenchWorktreeAction::admits`,
`WorkspaceGroupCloseAction::admits` and `TabSetCloseAction::admits` share the response rules with
Clients, including resume attempts, frozen set scope and intent-specific progress. `WorkbenchCommand::admits_creation_outcome` admits only
layout/terminal creation receipts with their revision and target checks. Clients separately
validate connection generation and operation-specific state postconditions.

## OBWP v1 multiplexed streams

The [native Client/Server delivery contract](../../apps/orchbee-server/README.md#multiplexed-client-delivery)
uses the same 12-byte OBWP header: magic, big-endian u16 version 1, u16 kind and u32 payload length.
Kinds 64–71 carry complete bounded inner OBCS/OBWP/OBAP records without changing their semantic
version or codec. This namespace is separate from graphics producer stream identity.

| Kind                 | Payload                                                                                                                                                           |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 64 StreamOpen        | Nonzero u64 stream ID, nonzero u64 logical lane ID, u8 mode, inner request. Modes: Request 0, State 1, Events 2, Invalidations 3.                                 |
| 65 StreamFrame       | Nonzero u64 stream ID and sequence, u8 frame kind, inner response. Kinds: Result 0, Snapshot 1, Update 2. Invalidation updates contain an eight-byte commit hint. |
| 66 StreamClose       | Nonzero u64 stream ID, u8 reason: Complete 0, Cancelled 1, Unsupported 2, Capacity 3, InvalidRequest 4, Stopping 5 (a drained Server refused the intent).         |
| 67 Gap               | Nonzero u64 stream ID and last Server sequence through the loss boundary. A replacement Snapshot follows.                                                         |
| 68 Disconnect        | u8 reason: SlowConsumer 0, Capacity 1, InvalidStream 2, Unavailable 3.                                                                                            |
| 69 Resnapshot        | Nonzero u64 stream ID; Client requests Gap then fresh Snapshot after local event inbox overflow.                                                                  |
| 70 LaneOpen          | Nonzero u64 lane ID, monotonically increasing within the connection. Root lane 1 exists implicitly.                                                               |
| 71 LaneClose         | Nonzero u64 lane ID; release only that lane's logical resources. Root lane cannot be closed independently.                                                        |
| 72 ViewerRecordReady | Nonzero u128 viewer ID, Server to Client only: that viewer on this connection holds something to Poll. A hint, never a record.                                    |

A `ViewerRecordReady` belongs to no stream or lane, and a Client may discard one it cannot place.
Poll remains the only way to take a frame, so a lost hint delays that Poll and loses nothing.

Stream IDs increase across all lanes and are never reused within one physical connection. Each
subscription generation begins with sequence 1 and Snapshot; subsequent sequences increase.
Request streams accept one Result. Explicit lanes and streams are bounded to 64 each, with root lane 1 additional; the shared retained
byte budget is 32 MiB and each inner record is at most 24 MiB plus a 12-byte header. Unknown versions,
kinds, lengths, tags and zero identities are rejected. Incremental decoding bounds partial input
and permits cancellation/command wakeups without blocking on an incomplete frame.

Decoding is independent of read boundaries. For the same bytes, the blocking reader, one
whole-buffer push and any partition of pushes yield the same messages and the same clean,
incomplete or invalid ending, provided each push keeps retained input within the 32 MiB budget;
a push beyond it is refused whatever the framing. Every accepted record re-encodes to exactly the
bytes it consumed, so malformed input is rejected rather than normalized. The tests in
`src/multiplex/tests/` check this differentially with every single-byte mutation and seeded
multi-edit streams under random partitions, printing the minimal failing edits and cuts;
`ORCHBEE_PROTOCOL_MUTATION_SEED` explores another seed. They do not prove Server resource
ownership or delivery ordering.

`schemas/workbench/v1/workbench-multiplex-conformance-v1.json` fixes the exact bytes of valid
records and rejected inputs for both implementations: `tests/multiplex_conformance.rs` checks this
crate and the `apps/web` Client codec test checks TypeScript against the same file.

The Client implementation reserves one of those 64 streams and 256 KiB within the total budget
for its bounded idle progress read; this changes no wire version or record kind. Decoder input
and transient parsed frames have separate limits from retained stream queues. Incremental input
capacity grows geometrically only up to 32 MiB. After a complete large record leaves, empty or
at-most-16-KiB suffixes replace capacity above 64 KiB, preserving a partial adjacent header without
keeping the old large scratch allocation. The large-record trace in `src/multiplex/tests.rs`
covers this release.

## Panel Agent completion

Panel Agent report views and physical snapshots carry the Server-owned completion fact alongside
raw provider state. `AgentCompletion::display` shares its presentation rule with native consumers;
an accepted edit acknowledges completion for all Clients while leaving raw idle and semantic
sequence unchanged. The [shared Panel Agent contract](../../schemas/workbench/README.md#workspace-metadata)
owns wire compatibility and the [attention owner](../../apps/orchbee/docs/native-client-attention.md)
owns physical identity, transitions and Client seen state.

## Common setting values

`UserSettingsValues` is the typed view of the common settings both Clients present, with the value
types its fields use and their bounds, including the launch catalog. This crate owns only the
types: `orchbee-local-runtime` builds them from `config.toml` keys or a `workbench.settings`
snapshot, and the [settings registry](../local-runtime/README.md#user-settings) owns keys,
defaults and effects. The [Native Settings wire contract](../../schemas/workbench/README.md#native-settings-wire-contract)
owns the launch catalog's relational rules.

## Project organization and stable Workspace identity

Navigation owns global identity-sorted `directories`, `projects` and `workspaces`, plus independent `projectOrder` and `workspaceOrder`. Each nonempty
order is an exact permutation of its registry; omission or an empty order uses canonical ID order. Project contains only
`projectId` and `label`. Workspace carries required-nullable `projectId` and `primaryDirectoryId`;
its ID, Terminal Context and physical Directory are independent of grouping. References must
resolve, and each Terminal Context has exactly one Workspace owner, including the valid zero-Workspace/zero-Context state. Capacity is 256 Projects,
65,536 global Directories, 256 Workspaces in one Project and 1,024 Workspaces overall. The existing
1 MiB projection byte budget is an additional bound, independent of collection capacity.

`workspace.create` requires `projectId`, allowing null. Pure `workspace.set-project` changes one
existing Workspace's membership; `workspace.create-project` creates a Project and assigns that
Workspace atomically; `project.dissolve` removes only the group and leaves its Workspaces ungrouped.
Results use `workspace-project-changed` with required-nullable `projectId`, or `project-dissolved`.
Regrouping preserves Workspace IDs, directories, terminal identities and display order. Authoritative
membership admission, revision CAS and atomic publication remain Server responsibilities.

Workspace create, launch and rename commands require a label containing at least one non-whitespace
Unicode scalar. Accepted text is preserved exactly within the existing 512-scalar bound. Rust,
TypeScript and JSON Schema share this boundary and the command conformance cases; historical
snapshot labels stay decodable so Clients can show a fallback and rename them. This does not
change the independently owned Project, Tab or Tab-group naming rules.

The additive `workspace.close-if-idle {workspaceId}`, `project.dissolve-if-idle {projectId}`,
`tab.close-if-idle {terminalContextId, tabId}` and `panel.close-if-idle {terminalContextId, tabId,
panelId}` actions keep the ordinary command envelope and existing accepted receipts, with distinct
full fingerprints. They accept no caller idle flag. Only Server-owned complete-scope admission can
accept them; definite `rejected` reason `confirmation-required` has no effect and grants no
automatic resubmission. Ordinary confirmed actions remain unchanged. The
[Server conditional contract](../../apps/orchbee-server/docs/terminal-tab-close.md#conditional-idle-admission)
owns pending reservations, exact never-created evidence, the Server's terminal evidence, fences
and organization preservation.

`panel.restore-terminal {terminalContextId, tabId, panelId, presentationId, directory}` with
`WorkbenchRestoreDirectory` `saved` or `home` starts a Presentation whose `PresentationLaunchFailure`
carries `DirectoryUnavailable`. That reason pairs only with an unbound Presentation, a `null`
executable and the held `directory`, plus an optional `agentResume` flag; `TerminalCreateFailed`
keeps requiring a binding and carries neither. The
[checkpoint owner](../../apps/orchbee-server/docs/workbench-checkpoint.md#held-restore) defines
admission against the current hold.

`navigation.configure` carries one closed `NavigationConfiguration`: `move-project` requires a
Project ID and nullable `beforeProjectId`; `move-workspace` requires a Workspace ID, nullable
`projectId` and nullable `beforeWorkspaceId`. A null anchor appends within that destination group.
A non-null Workspace anchor must already belong to the destination. `configure_navigation` shares
one deterministic transition between authority planning and native receipt verification. It preserves
identity-sorted registries and every physical resource, preserves other groups' relative order, and
changes membership and display order atomically. Missing identities, invalid anchors and full groups
refuse the whole candidate. No-op returns `unchanged` without a new revision; a change returns
`navigation-configured`. The native gesture contract belongs to [sidebar layout](../../apps/orchbee/docs/tui-shell-layout.md).

`workspace.launch` carries `placement`: `ungrouped`, `project { projectId }`, `new-project { label }`,
or `alongside { workspaceId, projectLabel }`. Alongside resolves the source's current grouping at
authoritative commit. An ungrouped source causes one atomic new Project plus membership of both
source and newly launched Workspace; an already grouped source reuses that group. Launch requires
an absolute `options.cwd`, excludes Plugin Pane options and conflicting launch/profile selectors,
and uses the 96 KiB launch command class. Optional `automaticLabel` requests a Workspace name that
follows its first terminal; `Workspace.automaticLabel` carries it, and both omit `false`.
Optional `Workspace.terminalDirectory` carries the first terminal's observed directory only while
it differs from the primary Directory. Pending and
completed receipts require nullable `projectId`. `panel.relocate`'s `new-workspace` destination also requires nullable `projectId`.

Rename, Close and primary Directory acceptance target stable Workspace identity without Project.
Close acceptance, reads, resumes and their results likewise carry no `projectId`; the exact maximum
Close read result is 1,000,007 encoded bytes. Workspace event data contains only `kind` and
`workspace`, whose required-nullable membership is the sole Project field.

Worktree Start requires nullable `projectId`: only a Directory source may supply a non-null
destination. Workspace and `workspace-directory` sources and Remove must supply null; their grouping
is resolved from the stable Workspace at final commit. `workspace-directory { workspaceId, path }`
runs Git in `path`, where the Workspace's first terminal stands, and carries no Directory identity. Available receipts retain the original requested nullable
destination as correlation metadata, not a current membership claim. Exact request equality,
retained outcomes and unknown-delivery rules remain unchanged. Worktree query associations have
nullable Project membership. Legacy `workbench.worktree-list.read` and its `found` result identify
only the global Directory, with no Project scope; its result retains the 32 KiB bound.

Project group Close Start contains only `operationId` and `projectId`. Available results contain
`projectId`, `attempt`, `status` and frozen `members`; each member contains only `workspaceId`,
`label` and required-nullable `close`. Admission captures all Workspaces in that Project regardless
of Git repository or directory, with at most 256 members. There is no source-last rule. Unique
Workspace/child request identities, ordered closed-child prefixes and explicit recovery truth are
still enforced. Empty Projects admit Pending and vacuously Closed results. Closed requires every
captured child to be observed Closed. The result frame budget is 1 MiB so 256 maximum escaped labels
fit; requests remain 16 KiB. The driver owns progress from admission to completion.

This is one unpublished OBWP v1 matched-cohort change: Rust, TypeScript, schemas and shared corpora
activate and roll back together. Protocol tests cover exact encoded limits, closed shapes, nullable
fields, global references/order/capacity, launch placements and retained child truth. Checkpoint
migration and native/frontend effects belong to their runtime owners.

## Tab set Close

`workbench.tab-set-close` (OBWP 67/68) closes several Tabs of one terminal context as one retained
Server operation. Start carries `operationId`, `terminalContextId`, 1–256 distinct `tabIds` in the
context's current order, `expectedRevision` and `condition` (`if-idle` or `confirmed`); Read carries
only `operationId`. Available results echo the context and condition with a status (`pending`,
`closed`, or `failed` with a reason) and frozen `members`: `tabId`, `label`, required-nullable
`close` (the member's own Close request ID and nullable observation) and `absent`, set when the Tab
was gone before its turn. An if-idle Start may answer `confirmation-required`; `rejected` carries the
current revision and a set-specific reason. Neither retains anything. `TabSetCloseAction::admits`
requires a Start receipt to name the requested context, members and condition. Results enforce
unique members and child IDs, no child or absence after the first unfinished member, and Closed
only when every member is closed or absent. There is no set-level Resume: a failed member's own
Close stays retained for Tab Resume. Requests keep the 16 KiB intent cap; results have a 1 MiB
pre-allocation cap so 256 maximum escaped labels fit.

## Tab duplication

## Earlier layouts

`workbench.layout-history` (OBWP 71/72) reads and reopens the earlier layouts the Server keeps.
`list` answers `listed` with at most 48 entries in strictly decreasing `savedAt` (Unix
milliseconds, at most 2^53 − 1), each with Workspace and Tab totals and how many are not open now.
`reopen` names one `savedAt` and answers `reopened` with the revision, the counts it opened and a
required-nullable `focus` (the first reopened Tab, present whenever Tabs were reopened),
`nothing-to-reopen`, `not-found`, `rejected` (`session-mismatch`, `resource-limit`), `denied` or
`unavailable` (`service-unavailable`, `busy`). `WorkbenchLayoutHistoryResult::answers` admits only
a listing for `list` and never one for `reopen`. Both frames keep the 16 KiB intent cap. The
[schema](../../schemas/workbench/v1/workbench-layout-history.schema.json) and
[shared corpus](../../schemas/workbench/v1/workbench-layout-history-conformance-v1.json) drive this
binding and TypeScript; the
[Server checkpoint contract](../../apps/orchbee-server/docs/workbench-checkpoint.md#earlier-layouts)
owns what is kept and reopened.

## Session stop

`workbench.session-stop` (OBWP 73/74) lets an admitted Client stop its Session as `server stop`
does. The request names the Client's `sessionId` and a `stopId` it chose, a nonzero 128-bit
lowercase hexadecimal `WorkbenchStopId`, which `server stop-status` and `server stop --retry` take
as the request ID; `WorkbenchStopId::bytes` gives its 16 bytes. Outcomes are `recorded`, `busy`
(another identity's stop holds the Session), `unmanaged`, `rejected` (`session-mismatch`) and
`denied`; only `recorded` holds a stop. There is no status or retry action: a recorded stop refuses
every later record, so its progress and retries go through the unadmitted stop exchange under the
same identity. Both frames keep the 16 KiB intent cap. The
[schema](../../schemas/workbench/v1/workbench-session-stop.schema.json) and
[shared corpus](../../schemas/workbench/v1/workbench-session-stop-conformance-v1.json) drive this
binding and TypeScript; the [Server](../../apps/orchbee-server/README.md#managed-windows-session-shutdown)
owns admission and the stop.

`workbench.tab-duplicate` (OBWP 69/70) duplicates several Tabs of one terminal context as one
retained Server operation. Start carries `operationId`, `terminalContextId`, 1–256 distinct
`tabIds` in the context's current order, `expectedRevision` and the terminal `geometry` every copy
starts at; Read carries only `operationId`. Available results echo the context with a status
(`pending`, `duplicated`, or `failed` with a reason) and `members`: `tabId` and a required-nullable
`copy` holding the copy's `tabId`, the `revision` that published its latest fact and a
required-nullable `terminal` outcome, null while the launch is still being delivered. `rejected`
carries the current revision and a reason, including `resource-limit` when the context cannot hold
one copy of every member; it retains nothing. `TabDuplicateAction::admits` requires a Start receipt
to name the requested context and members. Results enforce unique members, copy Tab IDs that are
unique and differ from every member, no copy after the first member whose copy has not started,
and Duplicated only when every copy started. There is no Cancel or Resume: a stopped operation
leaves the copies it made and never replays one. Requests keep the 16 KiB intent cap; results have
a 256 KiB pre-allocation cap.

## Tab configuration

`tabs.configure` carries an exact context and one closed `TabConfiguration`; `tabs-configured`
returns the committed revision and a required-null group ID except for creation. The pure
`configure_tabs` transition preserves terminal membership while enforcing pinned and contiguous
group order. Authority admission, revision CAS and publication remain Server-owned. Rust and GUI
`TabModel` consume the same `tab-configuration-conformance-v1.json` corpus; wire shapes also use
the shared command corpus. This extends the unpublished OBWP v1 matched cohort. Runtime and
receipt semantics are documented in the [native group contract](../../apps/orchbee/docs/native-tab-groups.md).

`WorkbenchTabIdSet` admits 1–256 unique IDs for `create-group-for-tabs`, `set-tabs-group` and
`set-tabs-pinned`; a missing member refuses the entire candidate. Canonical order determines
placement. `move-tab` matches GUI group boundaries and consumes an ephemeral adjacent-group
collapse hint. It does not introduce shared selection or collapse state.

`move-tabs-by-drag` atomically places an exact selected set at an original-order insertion
boundary (0–256) with a required-nullable destination group. It preserves pinned partitions,
complete selected groups, contiguous untouched groups and every terminal tree/binding.
Mixed pinned selections, missing targets and out-of-context boundaries are rejected.

## Terminal Tab placement

`TabCreationPlacement` adds optional `in-group`, `new-group`, `after-tab` and exact single-Tab `duplicate` to
`tab.create`; omission preserves append. `validate_tab_creation` checks targets and post-cleanup
capacity before effects. `after-tab` derives the exact target group and index + 1, refusing an unpinned
insertion inside the pinned prefix while admitting its final boundary. `create_terminal_tab` builds the complete candidate from fresh caller-owned
identities and binding. It preserves old trees, copies duplicate metadata and creates one fresh Panel;
Server admission/publication and terminal lifecycle remain their existing owners. Native exact receipt
validation shares this pure transition. GUI and Rust consume `tab-creation-conformance-v1.json`.
`WorkbenchTabGroupLabel` admits empty group names (0–512 Unicode scalar values), including `CreateGroup`, `CreateGroupForTabs`
and `UpdateGroup`; `WorkbenchLabel` remains nonempty. See the [native group contract](../../apps/orchbee/docs/native-tab-groups.md#terminal-creation-and-duplication).

## Semantic event contract

Closed `workbench.events` request/result kinds 55/56 have independent 256 KiB limits. Subscribe,
read, bounded wait and close carry an exact connection-owned subscription identity. Journal cursors
pin boot epoch and full-width decimal sequence; probe positions retain a separate subscription
sequence. Filter conditions, event/data kinds, source versions, ordered pages and explicit gaps are
validated before interpretation. `matches_semantic_filter` shares pure semantic matching with
runtime publishers and native CLI receipt checks. The [Windows event model](../../apps/orchbee-server/docs/windows-events.md)
owns actual publication and observation effects. Optional Client `splitRevision` is the full-width
canonical Workbench revision of the projected cache; legacy omission supplies no ratio-change evidence.

This crate owns the typed requests, responses, validation, and framed transport shared by native Workbench clients and the Server. It defines contracts without opening sockets or executing operations; runtime owners interpret admitted requests.

`WorkbenchDirectoryPath` preserves admitted operand spelling. `same_directory_spelling`
shares the existing Worktree association comparison between Server and Client: POSIX-rooted
spellings compare byte for byte, and Windows drive spellings may differ only in separators,
trailing separators and ASCII case. It performs no filesystem identity check,
canonicalization, verbatim-prefix or dot-component resolution. `with_native_separators` gives a
drive path the `\` spelling Windows reports, for a writer recording a Git-reported path; POSIX-rooted
and `\\?\` verbatim spellings stay unchanged, and it is equally lexical. Wire syntax and runtime
execution ownership remain unchanged.

## Configured command invocation

Matched cohorts use closed `workbench.configured-command` request/result kinds
53/54 with a 256 KiB bound. The installed key profile owns catalog IDs and all
four actions. Invocation carries the expected canonical revision and an optional
exact Client/source fence, never an executable. Replies distinguish background
job admission, canonical Pane creation, plugin execution and retained Client
popup control; nested receipts preserve their original uncertainty.

Configured popup controls carry a separate typed selector, source, display
context and configured dimensions. Open/read/close are owned by the authenticated
Client connection and use the existing independent terminal lifecycle. A stale
catalog, selection, membership or source refuses before dispatch. No configured
command is represented as an installed plugin. [Native command behavior](../../apps/orchbee/docs/windows-custom-commands.md)
owns configuration, dispatch and Windows lifetime.

## Native plugin runtime contract

Closed plugin invocation/log/get/cancel requests and replies use matched-cohort OBWP kinds 45/46, capped at 32 KiB and 8 MiB respectively. Context carries display data only; admitted Server resolves executable intent and Worker owns processes. Log cursors pin Worker identity/generation and descending sequence; uncertain mutation delivery is explicit. Rust, TypeScript and JSON share the plugin conformance corpus. [Native plugin behavior](../../apps/orchbee/docs/windows-plugins.md) owns runtime lifecycle and remaining integration.

Plugin Pane launch options may carry `pluginPane: { pluginId, entrypointId }` on `tab.create` and `panel.split`. Both identities are bounded ASCII catalog names (120 bytes; entrypoint excludes dots). The selector is exclusive with executable/profile and rejected on Workspace or child-terminal creation. Empty and null optional selectors normalize away; legacy request serialization remains unchanged. Server resolution and protected execution environment belong to terminal admission, not this display-free selector.

Plugin PaneGet is read-only and returns PaneFound with a nonzero canonical revision, closed selector, exact context/Tab/Panel/Presentation and binding. Response correlation requires the requested Panel; the result is not an action Job receipt or caller display context.

## Agent integration jobs

`AgentIntegrationRequest`/`AgentIntegrationResult` add the closed `workbench.agent-integration`
and `workbench.agent-integration.result` directions, OBWP kinds 63/64, capped at 10 KiB and
36 KiB. `action` is `start` with one
[Agent integration request](../agent-integration/README.md#contract), or `poll`/`cancel` with a
job identity `{workerId, generation, jobId}` (nonzero generation, 32 lowercase hexadecimal job ID).
Outcomes are `accepted`, `pending`, `completed` with one Agent integration report,
`cancel-registered`, `already-terminal`, or `failed` with a closed reason: `denied`,
`session-mismatch`, `invalid-request`, `worker-unavailable`, `worker-changed`, `busy`,
`job-unknown`, `abandoned`, or `malformed`. A result answers only its request's protocol version,
request ID, Session, and action; poll and cancel outcomes answer only their own job. Starting
`enable` or `disable` is mutating. The
[direction schema](../../schemas/workbench/v1/workbench-agent-integration.schema.json) governs it
for every language, and Rust, TypeScript, and JSON Schema share its
[conformance corpus](../../schemas/workbench/v1/workbench-agent-integration-conformance-v1.json).

## Workspace Git status reads

`WorkspaceGitRequest`/`WorkspaceGitResult` add the closed `workbench.workspace-git`
and `workbench.workspace-git.result` directions, OBWP kinds 49/50, with independent
8 KiB pre-allocation limits. Required nullable branch/divergence fields distinguish
absence from zero; branch text is control-free UTF-8 at most 1024 bytes, counts are
uint32, and detached branches cannot carry counts. Exact request/Session/Workspace
correlation is mandatory. The [schema and shared corpus](../../schemas/workbench/README.md#workspace-git-status-reads)
cover both Rust and TypeScript. This read grants no mutation or Desktop command.
Configuration publication additionally carries separate required 32 KiB
`spacesSidebarProfile`; its matched-cohort frame budget is 256 KiB.

## Workspace current directory reads

`CurrentDirectoryRequest`/`CurrentDirectoryResult` add the closed `workbench.current-directory`
and `workbench.current-directory.result` directions, OBWP kinds 65/66. An outcome is `observed`
with a `WorkbenchDirectoryPath`, or `failed` with `denied`, `not-found` or `unavailable`. The
26 KiB pre-allocation limit holds the longest directory with every byte escaped. An optional
`CurrentDirectoryPanel` names a Panel and the Presentation the asking Client shows in it; absent, the
read is about the Workspace's first terminal, and present, it is never `null`. Exact
request/Session/Workspace/Panel correlation is mandatory, and the read changes no Workbench state. The
[schema and shared corpus](../../schemas/workbench/README.md#workspace-current-directory-reads)
cover both Rust and TypeScript; the [Server](../../apps/orchbee-server/README.md#workspace-current-directory-reads)
owns which directory answers.

## Plugin overlay observation and Client return identity

Read-only `PaneObserve` carries exact `PluginPaneInfo`; `PaneObserved` echoes it with a nonzero canonical revision and a boolean settlement fact. Correlation includes selector, context/Tab/Panel/Presentation and binding. It grants no lifecycle mutation. Private Client `Overlay` control carries the created bound member, exact nullable return member and previous zoom. Validation rejects self-return identities; Applied requires the requested focused member, zoom and multiple leaves. Required nullability and semantic failures share the Rust/TypeScript corpus. [Windows plugins](../../apps/orchbee/docs/windows-plugins.md#overlay-lifecycle-model) owns runtime records, observation cleanup and canonical Close. These extensions require the matched native cohort and add no Desktop command.

Pane launch options additionally carry optional bounded `pluginContext` only with `pluginPane` on Tab/split creation. The shared command corpus covers null normalization, unknown fields, UTF-8/total context bounds and unsupported creation kinds. Context is display data: the Server validates source membership, supplies canonical labels and fixes invocation source/correlation before freezing protected plugin environment. It never selects executable, launch directory or Worker authority. Omission preserves legacy admission. Rust redacts the context from launch Debug; TypeScript shares its context schema without a launch/plugin import cycle and preserves the existing plugin export. Runtime semantics and acceptance scope belong to [Pane launch source context](../../apps/orchbee/docs/windows-plugins.md#pane-launch-source-context).

## Independent plugin popup contract

Private plugin PopupOpen/PopupRead/PopupClose use the existing matched-cohort plugin transport and explicit correlated PopupOpened/PopupState/PopupClosing/PopupClosed/PopupUnknown results. PopupOpen carries a closed catalog selector, exact context/Tab/Panel and full Client Presentation selection, bounded display context, required nullable cwd/dimensions, and the shared protected-name launch-environment admission. Bound attachment generation/incarnation and nonzero unbound selection generation remain part of the source identity. These requests do not create canonical Pane membership. Client PopupOpen/PopupClose require explicit matching completion; generic Applied cannot complete them. A hadPopup=false close may preserve an unrelated Busy dialog. A Client refuses a popup open with `modal-open` while another popup, overlay, dialog, history view, or input mode such as copy or resize mode owns its input, with `insufficient-space` when its surface is absent or cannot fit the smallest popup frame, and with `busy` for any other not-ready state. So that the Client can name that cause, the Server delivers popup, command-popup and editor open intents to a Busy Client (`ClientViewIntent::refused_before_delivery`); a Starting Client and the single active command per Client still refuse them as `busy`. Rust, TypeScript and JSON share valid/invalid conformance vectors. [Independent popup behavior](../../apps/orchbee/docs/windows-plugins.md#independent-terminal-popup) owns physical lifecycle, UI input and acceptance. Peers deploy together; no Desktop operation or new Host capability is granted.

## Private scrollback editor popup contract

`workbench.terminal-editor` and its result use matched-cohort OBWP51/52 without
changing old kinds or public CLI definitions. Closed Open/Read/Close messages carry
Session/request/popup correlation. Open names a canonical bound Client popup source,
a same-connection read viewer and positive generation/full-u64 revision as decimal
strings, plus drawable geometry. No executable, environment, document path or
transcript is admitted. Results distinguish opened/state/closing/closed/unknown
and closed failures; action-specific responses and exact correlation are checked.
The 8KiB request/result limit is whole-message, with strict unknown-field and
nonzero identity checks. The [Server popup model](../../apps/orchbee-server/README.md#windows-scrollback-editor-popup-model)
owns source authorization, deferred creation/start and cleanup. This private
contract does not prove Client focus or completed public editor parity.

The matched Client view contract also admits private `editor-open` with only a
bound `ScrollbackEditorSpec.source`. It shares the exact-source `popup-opened`
completion and generic `popup-close` with typed plugin/editor actors. The
[Client popup model](../../apps/orchbee/docs/windows-client-control.md#scrollback-editor-popup-model)
owns same-connection read-only capture, full-content geometry, input, cancellation
and cleanup. Public editor operation receipts use the sole version-1 control contract. The public Windows `terminal edit-scrollback` path is defined in
[CLI workflows](../../apps/orchbee/docs/cli-workflows.md#edit-the-focused-terminals-scrollback).
Focused wire/actor evidence does not establish final GUI/cohort acceptance.

## Panel image contracts

`PaneGraphicsRequest` and `PaneGraphicsResult` use OBWP 57/58 with independent preallocation bounds (24 MiB request, 512 KiB result). They cover complete PNG/RGB/RGBA/BGRA uploads, static layers, metadata catalogs, revision-pinned pixel chunks and connection-owned producers. Shapes, canonical base64, channel lengths, damage bounds, full-width producer sequences and response correlations are validated without renderer or file effects. Runtime owners supply PNG decoding, source fences, file leases and actual Client capabilities. Shared conformance cases live under `schemas/workbench/v1/workbench-pane-graphics-conformance-v1.json`.

Canonical `PanelNode::Split.ratio` defaults to 0.5 and admits finite 0.1–0.9 values. The identified CAS command `layout.set-split-ratio` returns `split-ratio-configured`; [shared ratio ownership](../../apps/orchbee-server/docs/shared-split-ratios.md) defines its runtime effects.

The runtime JSON dependency explicitly enables `float_roundtrip` so numeric ratios
retain their exact `f64` identity across transport and checkpoints. For example,
0.54 minus 0.05 produces 0.49000000000000005; decoding it as 0.49 would invalidate
the exact CAS/publication comparison after a successful adjustment. This changes
neither the JSON shape nor the admitted range. `tests/split_ratios.rs` covers all
648 directional coarse/fine numeric roundtrips. Development dependencies can
also enable this feature, so test builds alone do not prove shipment coverage;
the [keyboard resize model](../../apps/orchbee/docs/native-keyboard-resize.md)
requires an ordinary Windows cohort witness as well.

## Native Settings contract

The closed `workbench.settings` kinds 59/60 read and edit `config.toml` on the Server's machine, keyed by dotted setting path. `read {}` answers `current` with the whole snapshot and is subscribable as state. `edit { changes }` carries 1 to 64 changes with unique keys, each a `value` to set or `restore: true` to delete the key and fall back to its default, and the Server's editor deletes a scalar key set to its registered default the same way; every change lands in one file replace or none does, and the answer is `edited` with a snapshot at or after the edit and whether the file changed. A snapshot's `revision` comes from the same counter as the configuration snapshot, so a Client drops an older one; it also carries the display `filePath`, the file `status` (`ok`, `missing`, `syntax-error` with its one-based line and column, or `unreadable`, both keeping the last valid values), the `platform`, every scalar key's effective value, the keys the file holds, the effective launch catalog and bounded diagnostics. A `refused` answer names a stable `reason` and only the members that reason admits; a refused edit wrote nothing. The contract checks key grammar, value types and bounds only: which keys exist and what each admits belongs to the [settings registry](../../crates/local-runtime/README.md#published-key-file), so a new setting needs no protocol change. Requests and results are each bounded at 2 MiB, and uncertain exchanges are not replayed. Rust, TypeScript and JSON Schema consume the [same conformance corpus](../../schemas/workbench/v1/workbench-settings-conformance-v1.json); the [native Settings owner](../../apps/orchbee/docs/native-settings.md) governs the TUI page.

## Retained close identity lookup

`CloseOperationRead` and `CloseOperationResult` use OBWP 61/62 within the existing intent size cap. They read a retained close by its original request ID, with exact Session/request/operation correlation and closed Workspace/Tab/Panel targets. This read creates no state and retains tombstones. Rust, TypeScript and JSON Schema share the command corpus; [CLI operation status](../../apps/orchbee/docs/native-operation-status.md) owns invocation and recovery.
