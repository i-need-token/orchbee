# Workbench protocol

## Shared user preferences

[The common preference contract](docs/user-preferences.md) owns the GUI/TUI inventory,
defaults, validation, scope, effect horizons and capability boundaries. Native Settings
uses the same typed record and source-checked patch as the matched Desktop helper.
The JSON schema and cross-language corpus live under `schemas/workbench/v1`.

## Project organization and stable Workspace identity

Navigation owns global identity-sorted `directories`, `projects` and `workspaces`, plus
`standaloneTerminalContextId` and independent `projectOrder` and `workspaceOrder`. Each nonempty
order is an exact permutation of its registry; omission or an empty order uses canonical ID order. Project contains only
`projectId` and `label`. Workspace carries required-nullable `projectId` and `primaryDirectoryId`;
its ID, Terminal Context and physical Directory are independent of grouping. References must
resolve, and each Terminal Context has exactly one navigation owner. Capacity is 256 Projects,
65,536 global Directories, 256 Workspaces in one Project and 1,023 Workspaces overall. The existing
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

The additive `workspace.close-if-idle {workspaceId}` and `project.dissolve-if-idle {projectId}`
actions keep the ordinary command envelope and existing accepted receipts, with distinct full
fingerprints. They accept no caller idle flag. Only Server-owned complete-scope admission can
accept them; definite `rejected` reason `confirmation-required` has no effect and grants no
automatic resubmission. Ordinary confirmed actions remain unchanged. The
[Server conditional contract](../../apps/orchbee-server/docs/terminal-tab-close.md#conditional-idle-admission)
owns pending reservations, exact never-created evidence, fences and organization preservation.

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
and uses the 96 KiB launch command class. Pending and completed receipts require nullable
`projectId`. `panel.relocate`'s `new-workspace` destination also requires nullable `projectId`.

Rename, Close and primary Directory acceptance target stable Workspace identity without Project.
Close acceptance, reads, resumes and their results likewise carry no `projectId`; the exact maximum
Close read result is 1,000,007 encoded bytes. Workspace event data contains only `kind` and
`workspace`, whose required-nullable membership is the sole Project field.

Worktree Start requires nullable `projectId`: only a Directory source may supply a non-null
destination. Workspace sources and Remove must supply null; their grouping is resolved from the
stable Workspace at final commit. Available receipts retain the original requested nullable
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

`WorkbenchDirectoryPath` preserves admitted operand spelling. `same_windows_path_spelling`
shares the existing Worktree association comparison between Server and Client: separators,
trailing separators and ASCII case may differ. It performs no filesystem identity check,
canonicalization, verbatim-prefix or dot-component resolution. Wire syntax and runtime
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

## Workspace Git status reads

`WorkspaceGitRequest`/`WorkspaceGitResult` add the closed `workbench.workspace-git`
and `workbench.workspace-git.result` directions, OBWP kinds 49/50, with independent
8 KiB pre-allocation limits. Required nullable branch/divergence fields distinguish
absence from zero; branch text is control-free UTF-8 at most 1024 bytes, counts are
uint32, and detached branches cannot carry counts. Exact request/Site/Workspace
correlation is mandatory. The [schema and shared corpus](../../schemas/workbench/README.md#workspace-git-status-reads)
cover both Rust and TypeScript. This read grants no mutation or Desktop command.
Configuration publication additionally carries separate required 32 KiB
`spacesSidebarProfile`; its matched-cohort frame budget is 256 KiB.

## Plugin overlay observation and Client return identity

Read-only `PaneObserve` carries exact `PluginPaneInfo`; `PaneObserved` echoes it with a nonzero canonical revision and a boolean settlement fact. Correlation includes selector, context/Tab/Panel/Presentation and binding. It grants no lifecycle mutation. Private Client `Overlay` control carries the created bound member, exact nullable return member and previous zoom. Validation rejects self-return identities; Applied requires the requested focused member, zoom and multiple leaves. Required nullability and semantic failures share the Rust/TypeScript corpus. [Windows plugins](../../apps/orchbee/docs/windows-plugins.md#overlay-lifecycle-model) owns runtime records, observation cleanup and canonical Close. These extensions require the matched native cohort and add no Desktop command.

Pane launch options additionally carry optional bounded `pluginContext` only with `pluginPane` on Tab/split creation. The shared command corpus covers null normalization, unknown fields, UTF-8/total context bounds and unsupported creation kinds. Context is display data: the Server validates source membership, supplies canonical labels and fixes invocation source/correlation before freezing protected plugin environment. It never selects executable, launch directory or Worker authority. Omission preserves legacy admission. Rust redacts the context from launch Debug; TypeScript shares its context schema without a launch/plugin import cycle and preserves the existing plugin export. Runtime semantics and acceptance scope belong to [Pane launch source context](../../apps/orchbee/docs/windows-plugins.md#pane-launch-source-context).

## Independent plugin popup contract

Private plugin PopupOpen/PopupRead/PopupClose use the existing matched-cohort plugin transport and explicit correlated PopupOpened/PopupState/PopupClosing/PopupClosed/PopupUnknown results. PopupOpen carries a closed catalog selector, exact context/Tab/Panel and full Client Presentation selection, bounded display context, required nullable cwd/dimensions, and the shared protected-name launch-environment admission. Bound attachment generation/incarnation and nonzero unbound selection generation remain part of the source identity. These requests do not create canonical Pane membership. Client PopupOpen/PopupClose require explicit matching completion; generic Applied cannot complete them. A hadPopup=false close may preserve an unrelated Busy dialog. Rust, TypeScript and JSON share valid/invalid conformance vectors. [Independent popup behavior](../../apps/orchbee/docs/windows-plugins.md#independent-terminal-popup) owns physical lifecycle, UI input and acceptance. Peers deploy together; no Desktop operation or new Host capability is granted.

## Private scrollback editor popup contract

`workbench.terminal-editor` and its result use matched-cohort OBWP51/52 without
changing old kinds or public CLI definitions. Closed Open/Read/Close messages carry
Site/request/popup correlation. Open names a canonical bound Client popup source,
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
and cleanup. Public editor operation receipts use control v10; v1-v9 reject this
intent. The public Windows `terminal edit-scrollback` path is defined in
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

Matched Windows cohorts use closed `workbench.settings` kinds 59/60 with independent 4 KiB request and 256 KiB response bounds. Open observes a complete configuration snapshot and an exact SHA-256 content/absence token. Save carries the expected publication revision and three required-nullable dirty fields; empty patches and extra operation fields are rejected. Correlated outcomes distinguish no change, saved/applied, saved/not-applied and pre-write refusal. No arbitrary path or TOML travels on this contract, and uncertain exchanges are not replayed. Rust, TypeScript and JSON Schema consume the [same conformance corpus](../../schemas/workbench/v1/workbench-settings-conformance-v1.json); the [native Settings owner](../../apps/orchbee/docs/native-settings.md) governs effects.
