# Project and Workspace Model

The legacy Browser ingress and POSIX SSH Environment chain are retired. Current Desktop supplies the local Folder picker and the Server authority supplies Worktree commands; Browser Preview uses in-memory state. Optional SSH interfaces below are retained library contracts without a current provider.

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
├── standaloneTerminalContextId, expandedProjectIds, recentsExpanded, allocation sequences
├── projects[]: { id, name } (ordered optional groups)
├── resourceScopes[]: { id, name, directories[], primaryDirectoryId, launchConfigurations[] }
└── workspaces[] (ordered)
    └── { id, name, projectId: nullable, resourceScopeId,
          primaryDirectoryId, launchConfigurationId: nullable, terminalContextId }
```

`Directory` is the domain term; product copy uses Folder. `resourceScopeId` is the stable namespace for launch resources and the fallback Directory, not another user-visible group. Several Workspaces may share one namespace. A move or dissolution retains every resource, including unused Directories and launch configurations. Scope and Project identities have separate authority and lifetime. Each resource key is `(resourceScopeId, id)`.

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

The internal `semantic-commands` subsystem owns the current terminal creation operation and its location contract; it has no package subpath. Exact terminal locations contain only Workspace identity; the retired TypeScript CLI list projections are not part of this package.

`executeWorkbenchTerminalCreate` changes one exact context, preserving current Workspace and existing Tab selections. The sole first Tab is the empty-context validity fallback. Results contain exact context/Tab/Panel/presentation and preview/provision identities. Application owners assign the revision, durably commit before process reconciliation and handle effects and navigation. Missing identities or invalid launch targets reject without mutation.

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

Deterministic scenarios cover natural-title normalization,
custom override, hidden natural updates, reset, persistence, local hostname matching, URI decoding,
and rejection classes. Projected metadata supplies no trusted command-execution or SSH state, so the policy does not suppress title or CWD updates on that basis.

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
| Activate Workspace          | Stable Workspace exists                             | Select it and disclose its Project or Recents                             | None                         |
| Reorder                     | Source and target share current nullable membership | Reorder peers without changing membership                                 | None                         |
| Duplicate preview Workspace | Local source and preview composition                | Fresh context identities, same resource namespace and membership          | Fresh shell projections      |
| Remote registration         | Authoritative Environment descriptor                | Scope / inactive Workspace with an empty context                          | No shell or Host effect      |

Close Workspace is a destructive Action with the shared confirmation dialog. Dissolve Project is a grouping operation without a destructive confirmation. Neither deletes or relocates a checkout. Resource edits use scope identity; group removal cannot invalidate a pending resource operation by itself.

## Project Folder acquisition

The sidebar's New Workspace control is immediate. The optional Create Project menu retains Select Folder and New Folder onboarding. Browser Select Folder asks for one absolute path and creates a synthetic local descriptor; native Desktop selection validates one directory without changing disk state. Folder-derived group names use the final path segment, falling back to the trimmed path for a root.

Rename asks for a Project's or Workspace's name in one field that the Dialog's title labels, prefilled with the current name. The name is kept exactly as typed, as the protocol and the TUI keep it: a name that is blank by Unicode White_Space or longer than 512 characters is refused on submit or blur with a localized reason. Renaming a Workspace keeps the directory it records. New Project… in a Workspace's Move to Project submenu asks the same way for the new Project's name, prefilled with the Workspace's, and commits the Project and that Workspace's membership together. A Dialog whose Project or Workspace leaves closes with it. A Project gains a folder only through a Workspace that records it; the sidebar no longer edits a Workspace's launch-resource Folders.

One mounted Workbench permits one native selection, which only creates a Project. Cancellation, unmount, port retirement and replacement revoke the request.

New Folder selects a parent then explicitly submits one child name. Filesystem success and product success remain separate. The retained creation receipt identifies Workspace, resource scope, Directory, context and first presentation; its nullable Project is informational and regrouping does not invalidate it. The workflow awaits canonical publication and exact route acceptance before acknowledgement. A created Folder survives failed or uncertain opening, with path-based Select Folder recovery; unknown effects never trigger another mkdir. Main's lifecycle and filesystem rules are owned by [project-folder-acquisition.md](../../../apps/desktop/docs/project-folder-acquisition.md).

## SSH Project enrollment and connection controls

Optional SSH enrollment and group-status interfaces have no current application provider. They neither start SSH on mount nor define execution authority. Environment placement remains a Directory property in its resource namespace. Any future connection provider must resolve that stable placement; a movable Project cannot stand in for Environment, repository or terminal identity.

## Server Worktree commands

Where the authority reaches Git through `WorkbenchAuthority.worktrees`, each Workspace menu carries the TUI's Worktree commands. Opening the menu discovers the Workspace's repository again. Until Git answers, New Worktree is disabled and Open Worktree holds one disabled row saying what is known: reading, not a Git repository, checkouts unavailable, or no other checkouts. No earlier answer stands in.

| Entry                    | Action                              | Result                                                                                       |
| ------------------------ | ----------------------------------- | -------------------------------------------------------------------------------------------- |
| New Worktree             | `workbench.worktree.create`         | A checkout with the TUI's defaults opens as a Workspace named with the first unused ordinal  |
| Open Worktree            | `workbench.worktree.open`           | The Workspace standing in the chosen checkout activates; with none, a new one opens there    |
| Remove Worktree…         | `workbench.worktree.remove`         | After one confirmation, the Workspace closes and its checkout is deleted; the branch stays   |
| Resume Worktree Removal… | `workbench.worktree.resume-removal` | After its own confirmation, a removal of this Workspace that stopped continues the operation |

Remove Worktree… follows Delete Workspace and appears only for a checkout the Workspace may remove, which the authority decides. Its confirmation is the shared Action confirmation, naming the Workspace, its terminals, the path, and the kept branch. A removal that stops where it may continue asks at once whether to resume; declining leaves the failure notice, and while the authority keeps that removal, Resume Worktree Removal… takes Remove Worktree…'s place whatever Git reports. Rows wait while a command runs. Progress and outcomes are authority notices: an opened Worktree shows as its activated Workspace, a removal reports for four seconds, and a failure or unknown outcome stays until pressed. A created or opened Workspace activates by the created-Workspace rule below.

## In-memory Worktree transitions

No current surface dispatches `project-worktree-created` or `project-worktree-adopted`; the Server Worktree commands replaced the Desktop capability that did. Their contracts hold while the in-memory runtime keeps them.

`project-worktree-created` takes an exact source Workspace and Directory. The host revalidates its immutable scope, context and path after asynchronous preparation. A moved source uses its current group. An ungrouped source and the new Workspace gain one newly created Project in the same transition as the Directory and first shell. If no Workspace is requested, only the resource catalog changes. A closed source is never recreated. Publication preserves local navigation. A known created checkout is retained if the product commit fails; regrouping never authorizes compensation or deletion.

`project-worktree-adopted` accepts host-revalidated `source: discovered` metadata, including detached `branch: null` and exact `HEAD`. It shares creation's current-membership and first-shell rules. Physical Git identity is checked across all groups and resource scopes. An already-associated Worktree returns its existing Workspace; an equivalent unassociated Folder descriptor may be upgraded in place without changing its ID.

## Sidebar groups

The sidebar groups Workspaces as the TUI does. Projects come first in their order, each heading its
Workspaces; the Workspaces outside any Project follow under a Recents heading, always last and absent
while it would be empty. The list has no heading of its own, and no group heading shows a member
count. A heading is never current, even folded around the active Workspace, although a sidebar focus
request lands on that heading while the active Workspace is hidden. Recents folds like a Project
through `recentsExpanded`, which this client's view keeps beside `expandedProjectIds` and opens by
default; it has no menu and neither starts nor receives a drag. Activating a Workspace, or creating one
here, opens its Project or Recents.

## Workspace locations

A host may supply a `WorkbenchWorkspaceLocationSource`: where each Workspace stands, keyed by
Workspace ID, as its folder, its Git branch and divergence, and the name a moved row still holds.
The sidebar subscribes once per source while it is mounted, so a source that reads only while
watched stops when the sidebar goes. Workbench words these facts and decides nothing about them.

A Workspace row shows them as the TUI does. Its identity line keeps a lone row's height, so the icon,
row options, and Agent disclosure stay centred on it. After the name, flush with the trailing
markers, `↑N` counts commits to push in the success role and `↓N` commits to pull in the warning
role, each only when positive; the arrows tell them apart without color. A second line names the
branch, otherwise the folder, unless the Workspace's Agents are folded, when it lists their states
instead (see [Workspace Agent overview](#workspace-agent-overview)). A row with neither stays one
line, so pending facts reserve no blank row. A branch truncates at its end. A folder keeps its final folder whole and truncates its start,
where the TUI cuts symmetric halves. The line's title holds the whole text, the arrows are hidden
from assistive technology, and the row's accessible name adds the location and the counts in words.
While a source holds a moved row's earlier facts, their held name stands in for the Workspace name,
so the name, folder, and facts change together; the row menu keeps naming the Workspace itself.

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

Row menus take the TUI's compositions, which [native-action-menus.md](../../../apps/orchbee/docs/native-action-menus.md) owns. The Project menu holds New Workspace ▸ and Rename, then a separated Dissolve. New Workspace ▸ holds New Worktree… from the repository of the Project's first Workspace, a Folders group with each available directory the Project's Workspaces record, and a Git Worktrees group once Git answers. Folders follow navigation order, compare ignoring ASCII case, and show their folder name, or their full path where folder names repeat; each creates a Workspace in that directory inside the Project. While the Project records no folder, New Workspace leads and creates a Workspace in the Project at the default directory. Opening the Project menu starts Worktree discovery.

The Workspace menu holds Move to Project ▸ and Rename, then New Worktree… and Open Worktree ▸ where the authority provides them, then a separated destructive Close and, once discovery allows it, Remove Worktree…. Move to Project ▸ lists the other Projects, then New Project… and, for a grouped Workspace, Remove from Project. Set Directory… is absent because the Server offers Clients no command to set a Workspace's directory.

### Created Workspace activation

The initiating client captures an activation intent before an async Folder creation or a Worktree creation or opening begins. Completion names an exact Workspace and waits for its publication, in either response/publication order. Only the newest live intent may activate it once. Workspace, Tab or Panel navigation, explicit same-row navigation, a newer creation or unmount retires the old intent. Grouping changes and publication of another context do not count as navigation. No Project identity participates in readiness, and a closed source cannot be resurrected by completion. Readiness and focus use committed React projections; a suspended or discarded render cannot publish membership or redirect an operation. A late completion still observes the latest committed projection.

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
displacement uses the source row's own footprint, its height plus the gap to its neighbor, measured
once when the drag activates, rather than a live transformed rectangle, so one-line and two-line
Workspace rows squeeze by what they occupy. If the source Project was expanded, only its dragged presentation is compact: authoritative
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

The sidebar renders zero or more Agent rows directly beneath each Workspace, grouped or ungrouped: one for each terminal in which the Server observes an Agent. It does not repeat Tab or Panel managers. `projectWorkbenchAgentOverview` resolves each observed Presentation against each Workspace's exact context and retains canonical Tab/Panel/presentation order; a Presentation this Workbench does not hold has no row. The Server owns detection, Agent labels and states; Workbench only projects them.

A Workspace with no Agent has no Agent count or disclosure. One or two providers use their marks; more use the neutral multi-Agent mark. Counts cap visually at `99+` while accessible text retains the exact count. Status priority is blocked, error, done, working, idle, unknown, with no color-only or animated status.

As in the TUI, every Workspace lists its Agent rows until this Client folds them, and each Workspace folds on its own. A fold is client-local and lasts while its Workspace exists: through regrouping, ungrouping, dissolution, a collapsed Project, a hidden sidebar, and Agents that end and start again. A folded Workspace replaces its location line with its Agents' state glyphs, most urgent first and otherwise in row order, one gap apart; more than five show four and a muted `+N`. The line's title names each distinct state in that order, and while the Agent rows are hidden the Workspace row carries their unread marker and its accessible name says those states and unread. A dragged Workspace hides its own Agent rows until the drag ends, as a dragged Project folds; its siblings keep theirs.

`workbench.agent.navigate` targets only Workspace and terminal Presentation identity, confirms that the terminal still holds an Agent in that Workspace, re-resolves current topology and then selects the exact Tab, Panel and child, removing conflicting zoom. Its existing Action/projection coordinator rejects stale or superseded focus. Agent listing and navigation never start an Agent.

Navigation origin and cancellation use the last committed React location; suspended or discarded renders cannot redirect that intent. Target readiness still comes from the latest Agent observation and runtime topology. Hook tests cover a newer Agent observation between navigation and completion, preserving focus after a discarded location render while canceling it after committed unrelated navigation.

## Dependencies and ownership

Workbench owns the pure organization model, action adapters, resource dialogs and client-local views. Tab Strip owns Tab semantics, the Server owns terminal Agent observations, UI owns menu/dialog primitives, and i18n owns copy. Applications own filesystem validation, Git physical identity, canonical revisions, storage and process reconciliation. The GUI resource namespace is intentionally independent from the native protocol's globally unique Directory IDs.

## Correctness and verification

Deterministic and fixed-seed generated tests cover independent creation, scoped ID collisions, move/new-group/ungroup/dissolve, peer ordering, close versus close-all, resource and selection retention, unavailable fallbacks, physical Worktree deduplication and rejected transitions. Current-shape tests cover graph validation and malformed Client Views. Two-client runtime tests preserve context object identity, focus, child selection and zoom; first-shell allocation failure leaves no published structure.

Hook and component tests cover immediate creation, menus, stable management targets, Agent rows and late activation suppression. The Server owns Worktree Git effects, races and retained partial results; Web owns stable routes and unavailable identities. DOM tests do not establish native-window focus or actual shell process behavior. The repository integration gate remains the integration owner's responsibility.

## Delivery and recovery

GUI shared documents and Client Views accept only their current version-1 shapes. Complete graphs validate before use; invalid graphs are never migrated, reset or repaired. Historical development shapes have no compatibility promise. See [shared-workbench-document.md](shared-workbench-document.md).
