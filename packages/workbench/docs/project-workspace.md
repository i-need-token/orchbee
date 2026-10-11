# Project and Workspace Model

The legacy Browser ingress and POSIX SSH Environment chain are retired. Current Desktop supplies local Folder and Worktree capabilities; Browser Preview uses in-memory state. Optional SSH interfaces below are retained library contracts without a current provider.

## Status and scope

Workbench owns independently usable Workspaces, optional Project organization, launch-resource namespaces, client-local selection, and terminal-context composition. Filesystem and Git effects, PTY ownership, persistence I/O and canonical commit ordering belong to application owners. The GUI document contract is [shared-workbench-document.md](shared-workbench-document.md).

## Problem

A person can create a Workspace immediately, then organize it later. A Project is an optional named group, with ordinary and Worktree-backed Workspaces as peers. Changing organization must not change execution, resource identity, or any client's selection.

## Inputs and validation

| Input                                  | Contract                                                                                                         |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Workspace and terminal-context IDs     | Stable identities independent of group membership; missing identities reject                                     |
| Project ID                             | Nullable Workspace membership; a non-null ID must resolve                                                        |
| Directory and launch-configuration IDs | Scoped to an immutable `resourceScopeId`; never flatten historical raw IDs globally                              |
| Local Directory                        | Available local Folder or Worktree descriptor; host adapters validate external paths and Git facts               |
| Browser path                           | Trimmed non-empty synthetic path; preview does not claim filesystem validation                                   |
| Worktree source                        | Exact Workspace, resource namespace, terminal context and source Directory; regrouping alone is not invalidation |
| Display name                           | User or localized text; duplicate names are allowed                                                              |
| System CWD                             | Host-supplied final fallback and initial CWD for independent Workspace creation                                  |

A Directory has `placement: local` or `placement: environment(EnvironmentId)`. Its discriminated kind must agree with Git metadata. Environment Directories are unavailable to local launch; ordinary local operations cannot silently turn a remote Workspace into a local shell. Applications own Environment authority.

## Data

```text
ProjectWorkspaceState
├── activeLocation: standalone | workspace(WorkspaceId)
├── standaloneTerminalContextId, expandedProjectIds, allocation sequences
├── projects[]: { id, name } (ordered optional groups)
├── resourceScopes[]: { id, name, directories[], primaryDirectoryId, launchConfigurations[] }
└── workspaces[] (ordered)
    └── { id, name, projectId: nullable, resourceScopeId,
          primaryDirectoryId, launchConfigurationId: nullable, terminalContextId }
```

`Directory` is the domain term; product copy uses Folder. `resourceScopeId` is the stable namespace for launch resources and the fallback Directory, not another user-visible group. Several Workspaces may share one namespace. A move or dissolution retains every resource, including unused Directories and launch configurations. Scope IDs may equal historical Project IDs after migration, but have separate authority and lifetime. Each resource key is `(resourceScopeId, id)`.

Projects neither define repository identity nor own terminal structure. One Project can contain several repositories; one repository can appear in several Projects. Each Workspace owns one independent terminal context containing Tabs, Panels and presentations. Launch configurations retain stable references and display names; executable payload remains host-owned.

## Invariants

- Project, resource-scope and Workspace IDs are unique in their respective collections; Directory and launch-configuration IDs are unique only within a resource scope.
- Every scope has at least one Directory and a resolving fallback primary Directory. Paths are unique within their placement and namespace.
- A Workspace's Directory and optional launch configuration resolve in its immutable resource scope, independently of its nullable Project membership.
- Standalone and Workspace terminal-context IDs are unique and resolve exactly once.
- Active location names only a Workspace, never its Project; moving, ungrouping and dissolving preserve selection in every client.
- Group operations preserve resource catalogs, CWD, context/Tab/Panel/presentation identities, attachments, focus and zoom, and emit no terminal or Git command.
- Resource catalogs remain after their last Workspace closes; closing or dissolving organization does not implicitly delete launch resources.
- Names are not identity. Rejected and unchanged transitions preserve the complete input state.
- First-shell structure and optional new group membership are staged together; allocation failure publishes neither.
- Canonical Git common-directory identity and validated Worktree paths, never Project labels or membership, decide physical equivalence.

Removing a Folder retains referential integrity: the scope fallback becomes its circular next surviving Directory; Workspace primaries use that order while skipping remote Directories. Removing the final Directory or leaving no local fallback for an affected Workspace rejects. Removing a Worktree Directory remains outside ordinary Folder removal.

## Outputs

`transitionProjectWorkspace` returns state, `applied`/`unchanged`/`rejected`, a named rejection and terminal-context commands. It performs no React, DOM, storage, process, filesystem, Git, clock or network effect. The runtime prepares context commands before the Action commit callback and authoritative replacement, then publishes synchronously while isolating observer failures.

New Workspace uses an available localized default name (Workspace, Workspace (2), and so on), host CWD and default shell with one Tab and Panel. It requires no Project or form and never starts an Agent automatically. The explicit Folder onboarding flow can still create a group and its initial Workspace together.

```text
explicit Directory in the Workspace resource scope
→ split terminal current CWD
→ available Workspace primary Directory
→ available resource-scope fallback Directory
→ system CWD
```

Invalid explicit Directories reject. Unavailable local defaults are skipped. Remote Workspace primaries reject local launch before any fallback. Existing terminals retain their current and immutable launch CWD when defaults or grouping change. Splits and child terminals inherit current CWD; preview duplication uses recorded launch CWD and fresh identities. Managed context cloning remains unavailable.

### Semantic command projections and terminal creation

`@orchbee/workbench/semantic-commands` reads a validated shared document and mutates through the authoritative runtime. Directory listing accepts `resourceScopeId`; Workspace listing accepts a nullable Project filter and returns both membership and resource scope. Exact terminal locations contain only Workspace identity. Read snapshots preserve canonical order and input revision, never consult Client View, and leave pagination to the application.

`executeWorkbenchTerminalCreate` changes one exact context, preserving current Workspace and existing Tab selections. The sole first Tab is the empty-context validity fallback. Results contain exact context/Tab/Panel/presentation and preview/provision identities. Application owners assign the revision, durably commit before process reconciliation and handle pagination, effects and navigation. Missing identities or invalid launch targets reject without mutation.

### Projected terminal metadata and Tab titles

The application forwards one complete committed terminal metadata observation with the trusted
hostname of the machine that owns the terminal. Workbench normalizes the natural OSC 0/2 title by
trimming outer whitespace and then removing NUL characters. `null` clears the natural title. The
durable fallback title and optional durable custom Tab title remain separate. Display resolution is
custom title, then the focused terminal's latest natural title, then the final non-empty path
segment of its current CWD or launch CWD, then fallback. Product-created fallbacks use the localized
unnumbered Terminal label, so identity allocation never leaks into the visible title. Setting a
custom title trims it; blank or already-displayed input is unchanged. A later natural title remains
hidden beneath the custom title, and clearing the custom title reveals that latest natural title
immediately.

An OSC 7 current-directory value is accepted only as
`file://<host>/<percent-encoded-absolute-path>` when `<host>` matches the trusted terminal-machine
hostname under ASCII case-insensitive comparison. Empty hosts, `localhost`, other hosts, non-file
schemes, malformed percent escapes, and non-UTF-8 decoded bytes are ignored. Accepted paths are
percent-decoded before replacing `currentCwd`; an ignored value preserves the prior current CWD,
while a reported `null` clears it. Metadata never changes `launchCwd`.

This policy is a narrow behavioral parity projection from Warp revision
`086dade63ff7327426d6276d57f85b3f2f313af0`. Title state, override, reset, and persistence were
reviewed in `app/src/terminal/model/terminal_model.rs`, `app/src/pane_group/mod.rs`, and
`app/src/workspace/view.rs`; OSC title and CWD normalization were reviewed in
`app/src/terminal/model/ansi/mod.rs`. Deterministic scenarios cover natural-title normalization,
custom override, hidden natural updates, reset, persistence, local hostname matching, URI decoding,
and rejection classes. Warp's in-band-command title suppression and SSH-block CWD suppression are
outside the parity scope because Orchbee's projected metadata currently exposes no equivalent
trusted command or SSH fact; this policy does not guess either state.

## Decisions and transitions

| Intent                      | Guard                                               | Atomic result                                                             | Terminal effect              |
| --------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------- | ---------------------------- |
| New Workspace               | Valid host CWD                                      | Ungrouped Workspace, resource scope, first shell and initiating selection | Create one context           |
| Move / ungroup              | Workspace and optional destination exist            | Only `projectId` changes                                                  | None                         |
| New Project and move        | Workspace exists; non-empty group name              | Group plus membership in one transition                                   | None                         |
| Dissolve Project            | Project exists                                      | Remove group and ungroup its current Workspaces                           | None                         |
| Close Workspace             | Workspace exists                                    | Remove only that Workspace; selected closure falls back to standalone     | Dispose only its context     |
| Close All Workspaces        | Project exists, including an empty group            | Close its current members; retain group and resources                     | Dispose member contexts      |
| Folder onboarding           | Available local Folder                              | Group, resource scope and initial Workspace                               | Create one context           |
| Worktree create / adopt     | Exact source and validated Git descriptor           | Directory, optional Workspace, first shell and current grouping together  | Create one context if needed |
| Reopen associated Worktree  | Same physical Worktree already has a Workspace      | Return that stable Workspace; no duplicate                                | None                         |
| Resource catalog edit       | Valid source namespace and resolving references     | Update only that namespace and affected defaults                          | None                         |
| Activate Workspace          | Stable Workspace exists                             | Select it and disclose current group if any                               | None                         |
| Reorder                     | Source and target share current nullable membership | Reorder peers without changing membership                                 | None                         |
| Duplicate preview Workspace | Local source and preview composition                | Fresh context identities, same resource namespace and membership          | Fresh shell projections      |
| Remote registration         | Authoritative Environment descriptor                | Scope / inactive Workspace with an empty context                          | No shell or Host effect      |

Close Workspace and Close All Workspaces are separate destructive Actions with the shared confirmation dialog. Dissolve Project is a grouping operation without a destructive confirmation. None of these operations deletes or relocates a checkout. Resource edits use scope identity; group removal cannot invalidate a pending resource operation by itself.

## Project Folder acquisition

The sidebar's New Workspace control is immediate. The optional Create Project menu retains Select Folder and New Folder onboarding. Browser Select Folder asks for one absolute path and creates a synthetic local descriptor; native Desktop selection validates one directory without changing disk state. Folder-derived group names use the final path segment, falling back to the trimmed path for a root.

Manage Project edits only the group name. Manage Workspace owns the name, default Folder and launch-resource Folder list. Add Folder replaces its last row with a path and local Cancel/Add controls in preview, or invokes the host picker. Folder addition, fallback selection and removal commit independently of the Workspace-name Save action. Duplicate paths in that namespace are rejected; removal never alters existing terminal CWD. A sole Folder omits redundant selection controls. Names expose complete paths through Tooltips; unavailable remote Folders carry the localized SSH marker.

One mounted Workbench permits one native selection. Before adding the selected Folder it rechecks the same resource namespace and duplicate path, independent of any grouping change. Cancellation, unmount, port retirement and replacement revoke the request. The optional SSH Folder callback carries `resourceScopeId`, never a mutable Project identity or credential.

New Folder selects a parent then explicitly submits one child name. Filesystem success and product success remain separate. The retained creation receipt identifies Workspace, resource scope, Directory, context and first presentation; its nullable Project is informational and regrouping does not invalidate it. The workflow awaits canonical publication and exact route acceptance before acknowledgement. A created Folder survives failed or uncertain opening, with path-based Select Folder recovery; unknown effects never trigger another mkdir. Main's lifecycle and filesystem rules are owned by [project-folder-acquisition.md](../../../apps/desktop/docs/project-folder-acquisition.md).

## SSH Project enrollment and connection controls

Optional SSH enrollment and group-status interfaces have no current application provider. They neither start SSH on mount nor define execution authority. Environment placement remains a Directory property in its resource namespace. Any future connection provider must resolve that stable placement; a movable Project cannot stand in for Environment, repository or terminal identity.

## Managed Worktree product commit

`project-worktree-created` takes an exact source Workspace and Directory. The host revalidates its immutable scope, context and path after asynchronous preparation. A moved source uses its current group. An ungrouped source and the new Workspace gain one newly created Project in the same transition as the Directory and first shell. If no Workspace is requested, only the resource catalog changes. A closed source is never recreated.

Publication preserves local navigation. Git effects and known partial-result retention are owned by [project-worktree-creation.md](../../../apps/desktop/docs/project-worktree-creation.md). A known created checkout is retained if the product commit fails; regrouping never authorizes compensation or deletion.

## Existing Worktree adoption

`project-worktree-adopted` accepts host-revalidated `source: discovered` metadata, including detached `branch: null` and exact `HEAD`. It shares creation's current-membership and first-shell rules. Physical Git identity is checked across all groups and resource scopes. An already-associated Worktree returns its existing Workspace; an equivalent unassociated Folder descriptor may be upgraded in place without changing its ID. The host performs no Git mutation. See [project-worktree-adoption.md](../../../apps/desktop/docs/project-worktree-adoption.md).

## Project navigation action menus

Each Project row and Workspace row owns one ordered action-menu model. The row's ellipsis button
opens that model as an anchored Dropdown Menu; right-click or long press on the row opens the same
model as a pointer-positioned Context Menu. Both projections preserve item order, grouping, icons,
labels, destructive presentation, disabled state, and callbacks. They are render adapters, not
independent action definitions.

Opening either menu does not activate a Workspace, toggle Project disclosure, or otherwise mutate
Project and Workspace state. Dismissal invokes no action. The product suppresses the native browser
context menu only inside a row that owns this product Context Menu. Selecting an entry through
either projection invokes the same callback exactly once.

Row options and the Project section's create control remain in the normal Tab order while visually
hidden at rest. Hover, focus within the owning row or section, and an open menu reveal them
immediately; hidden controls receive no pointer input. Empty sections keep their create control
visible. This preserves both forward and reverse keyboard discovery without changing layout.

The row-local menu adapter uses the following decision model. It owns no Project, Workspace, or
menu state: stable object IDs remain with the existing callbacks, while shared menu primitives own
opening, traversal, dismissal, and focus handoff.
Captured trigger and pre-open focus references are scoped to the exact Project or stable Workspace
ID (`null` identifies the Project row), consumed by `finalFocus`, and released with the owner.
Unchanged pre-open focus is not a later handoff.

| Input or transition                                                                             | Result and invariant                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact `Shift+F10` or unmodified `ContextMenu` from a focused Project or Workspace row control   | Focus and invoke that row's live, enabled, closed ellipsis trigger, opening its existing Dropdown Menu once. The row advertises these keys through `aria-keyshortcuts`.                                                                                                                  |
| Unexpected modifiers, composing or repeated input, a consumed event, or no live enabled trigger | No invocation and no event suppression. Terminal input has no row listener.                                                                                                                                                                                                              |
| A portaled menu `keydown` event bubbles through the React row                                   | Actual DOM containment rejects it; the open menu keeps keyboard ownership.                                                                                                                                                                                                               |
| A native Menu-key release synthesizes `contextmenu` after the Dropdown Menu opens               | Suppress only keyboard context events from the row or the exact open dropdown named by its live enabled ellipsis's `aria-controls`. Require that trigger to remain expanded; this also covers portaled autofocus targets. Keep pointer behavior and shared menu state unchanged.         |
| Right-click or long press on the row                                                            | Capture its explicit ellipsis for the Context Menu's existing `finalFocus` callback, retaining the same object target and action entries.                                                                                                                                                |
| Escape or outside dismissal                                                                     | The primitive closes the menu without invoking an action; Escape returns to the valid explicit trigger, while outside interaction retains the primitive's focus policy.                                                                                                                  |
| An action opens a dialog or removes its row                                                     | Return is rejected when the trigger disappears, a dialog owns focus, or another live control has taken focus. Outside, focus-out, and sibling-menu dismissal also suppress explicit return. The adapter schedules no late return and never substitutes a sibling object's action target. |

`WorkbenchProjectWorkspace.unit.test.tsx` covers target identity, key guards, cancellation, modal
handoff, removal, and unchanged active location and disclosure. The navigation Stories are fixtures
for real-browser Tab/Shift+Tab, computed visibility, single-menu, and focus-return traces; DOM-only
tests do not establish CSS reachability. Capability-dependent entries and localized reasons remain
the existing action model's responsibility. This synchronous adapter introduces no async owner or
additional TLA+ model.

The Workspace menu offers New Workspace from its resource Folders and per-repository Worktree choices. Reopening the menu refreshes discovery. Repository labels describe source Folders, never the Project group. Move to Project offers existing groups, New Project and Move, and Remove from Project. The Project menu separates Manage Project, Dissolve Project and Close All Workspaces.

### Created Workspace activation

The initiating client captures an activation intent before async create/adopt begins. Completion names an exact Workspace and waits for its publication, in either response/publication order. Only the newest live intent may activate it once. Workspace, Tab or Panel navigation, explicit same-row navigation, a newer creation or unmount retires the old intent. Grouping changes and publication of another context do not count as navigation. No Project identity participates in readiness, and a closed source cannot be resurrected by completion. Readiness and focus use committed React projections; a suspended or discarded render cannot publish membership or redirect an operation. A late completion still observes the latest committed projection.

## Project and Workspace reorder gestures

The sidebar supports primary mouse or pen pointer reordering for two or more top-level Projects and
for two or more Workspace rows within one current group, including ungrouped Workspaces. Project and Workspace ellipsis controls,
cross-Project drag movement, cross-section movement, Project nesting, touch long-press, and
keyboard sorting remain outside this delivery. The initial Workspace created with a Project has no
fixed ordering role; every Workspace in the same group participates in the same ordering model.

The semantic gesture state is `idle`, `armed`, or `dragging`. `armed` records the pointer identity,
source identity, owning reorder scope, and activation origin without changing authoritative order or
Project disclosure. Movement of at least four CSS pixels activates `dragging`. The DOM adapter
projects stable direct sibling-row layout offsets into viewport coordinates and reports the first
non-source sibling whose vertical midpoint is below the dragged source row's vertical midpoint as
the nullable `beforeId`; no such sibling means the end of that list. Offset parents outside the list
must not be counted twice, and the pointer's grab position cannot delay or advance a squeeze crossing.
Project hit testing uses only top-level Project rows. Workspace hit testing uses only the Workspace
rows with the source Workspace's current nullable Project membership.

While dragging, authoritative order and DOM sibling order remain unchanged. The source row follows
the pointer vertically as a translated, elevated surface, while its original compact row slot keeps
list geometry stable. Each time the dragged row's midpoint crosses a sibling row's vertical midpoint,
the siblings between the source and target animate into the vacated slot, including the final sibling
when moving to the end. This squeeze motion is the same ordering language used by horizontal and
vertical Tabs; neither Project nor Workspace reorder uses a separate insertion marker. Sibling
displacement uses the tokenized compact navigation-row footprint rather than a live transformed
rectangle. If the source Project was expanded, only its dragged presentation is compact: authoritative
`expandedProjectIds` is never changed. Workspace dragging never changes Project disclosure. Near the
top or bottom of the shared Project viewport, proportional automatic scrolling recomputes the scoped
target without detaching the source row from the pointer.

Pointer movement updates the visible squeeze target. Pointer release commits that last previewed
target through exactly one Project-reorder or Workspace-reorder Action and returns to `idle`; release
jitter cannot silently replace the visible target with an unpreviewed placement. A Workspace-reorder
transition changes only the
global `workspaces` order within the current membership; it preserves Workspace identities, active location,
terminal contexts, Project order, Project disclosure, and all Workspace fields. Missing source or target Workspace identities and a target in a different group reject atomically. Self-targeted and
already-satisfied placements are unchanged. The adapter synchronously removes the active
displacement transition before the Action changes DOM order, so the committed layout cannot replay
the old transform.
Pointer capture begins only after drag activation, so an armed press does not retarget an ordinary
Project disclosure or Workspace activation click. Release before activation is an ordinary click and
performs no reorder.
Escape, `pointercancel`, unexpected lost pointer capture, unmount, a stale source, or a stale target
cancels without an Action or order mutation and restores the same disclosure intent. After either
commit, cancellation, or identity invalidation of an activated drag, the adapter suppresses the
browser's synthetic trailing click at the document capture boundary so disclosure or activation
cannot move to another row when pointer capture is unavailable. Repeated, self-targeted, and
already-satisfied moves are unchanged; missing source or target identities reject without partial
mutation.

## Workspace Agent overview

The sidebar renders zero or more live Agent rows directly beneath each Workspace, grouped or ungrouped. It does not repeat Tab or Panel managers. `projectWorkbenchAgentOverview` resolves Session attachments against each Workspace's exact context, deduplicates each Session within that Workspace, and retains canonical Tab/Panel/presentation order. Ended Sessions are excluded. Agent runtime owns lifecycle, provider, status and freshness; Workbench only projects them.

A Workspace with no Agent has no Agent count or disclosure. One or two providers use their marks; more use the neutral multi-Agent mark. Counts cap visually at `99+` while accessible text retains the exact count. Status priority is attention, failed/canceled/interrupted, succeeded, working, idle, unknown, with freshness rendered separately and no color-only or animated status.

At most one Workspace Agent disclosure is open, initially collapsed and client-local. Regrouping, ungrouping, dissolution and temporarily collapsing a Project preserve that Workspace's disclosure; removal or loss of all represented Sessions clears it. Dragging hides details temporarily. `workbench.agent.navigate` targets only Workspace and Agent Session identity, re-resolves current topology and then selects the exact Tab, Panel and child, removing conflicting zoom. Its existing Action/projection coordinator rejects stale or superseded focus. Agent listing and navigation never start an Agent.

## Dependencies and ownership

Workbench owns the pure organization model, action adapters, resource dialogs and client-local views. Tab Strip owns Tab semantics, Agent owns Sessions, UI owns menu/dialog primitives, and i18n owns copy. Applications own filesystem validation, Git physical identity, canonical revisions, storage and process reconciliation. The GUI resource namespace is intentionally independent from the native protocol's globally unique Directory IDs.

## Correctness and verification

Deterministic and fixed-seed generated tests cover independent creation, scoped ID collisions, move/new-group/ungroup/dissolve, peer ordering, close versus close-all, resource and selection retention, unavailable fallbacks, physical Worktree deduplication and rejected transitions. Strict historical migration tests cover graph validation, unused resources, sequence collisions and malformed Client Views. Two-client runtime tests preserve context object identity, focus, child selection and zoom; first-shell allocation failure leaves no published structure.

Hook and component tests cover immediate creation, menus, stable management targets, Agent rows and late activation suppression. Desktop owns async Git races and retained partial results; Web owns stable routes and legacy-link redirects. DOM tests do not establish native-window focus or actual shell process behavior. The repository integration gate remains the integration owner's responsibility.

## Delivery and recovery

GUI shared documents migrate from strict v2 to v3; Client View v1 migrates to v2. The complete old graph validates before migration, then the complete new graph validates. No malformed graph is reset. Existing IDs, resource order, unused configurations and context structure survive. See [shared-workbench-document.md](shared-workbench-document.md). Grouping rollback must not discard v3-only ungrouped Workspaces or resource namespaces; older builds cannot safely interpret this model.
