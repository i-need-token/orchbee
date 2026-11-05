# Project and Workspace Model

The Server holds every Project, Workspace, Directory, and terminal. `projectServerSnapshot` mirrors each publication into the model below, every structural change goes to the Server through `WorkbenchAuthority`, and this Client changes only its own view. Desktop supplies the local Folder picker.

## Status and scope

Workbench owns the read model of independently usable Workspaces and optional Project organization, client-local selection and disclosure, and terminal-context composition. Structure, filesystem and Git effects, PTY ownership, persistence, and commit ordering belong to the Server. The publication and Client View contracts are in [publication-and-client-view.md](publication-and-client-view.md).

## Problem

A person can create a Workspace immediately, then organize it later. A Project is an optional named group, with ordinary and Worktree-backed Workspaces as peers. Changing organization must not change execution, a Workspace's identity or directory, or any client's selection.

## Inputs and validation

| Input                              | Contract                                                                     |
| ---------------------------------- | ---------------------------------------------------------------------------- |
| Workspace and terminal-context IDs | Server identities independent of group membership; missing identities reject |
| Project ID                         | Nullable Workspace membership; a non-null ID must resolve                    |
| Directory                          | The Server's identity and path for the Workspace's directory, or none        |
| Display name                       | The Server's label; duplicate names are allowed                              |

## Data

```text
ProjectWorkspaceState
├── activeLocation: no-workspace | workspace(WorkspaceId)
├── expandedProjectIds, recentsExpanded
├── projects[]: { id, name } (ordered optional groups)
└── workspaces[] (ordered)
    └── { id, name, projectId: nullable, directory: { id, path } | null, terminalContextId }
```

`Directory` is the domain term; product copy uses Folder. A Workspace records at most one Directory, where its new terminals start; a Workspace without one starts no Tab. As in the TUI, the no-Workspace location holds no terminal context, and this Client is there only while no Workspace exists.

Projects neither define repository identity nor own terminal structure. One Project can contain several repositories; one repository can appear in several Projects. Each Workspace owns one independent terminal context containing Tabs, Panels and presentations.

## Invariants

- Project and Workspace IDs are unique in their respective collections.
- A Workspace's Directory is independent of its nullable Project membership.
- Every Workspace names exactly one terminal context, and every terminal context belongs to exactly one Workspace.
- Active location names only a Workspace, never its Project, or names no Workspace while none exists. A publication keeps this Client's view, so moving, ungrouping, and dissolving preserve selection in every Client.
- A publication that leaves a terminal context unchanged keeps that context's Tab and Panel objects, focus, child selection, and zoom.
- Names are not identity. Rejected and unchanged transitions preserve the complete input state.

## Outputs

`transitionProjectWorkspace` applies only this Client's events and returns state, `applied`/`unchanged`/`rejected`, and a named rejection. It performs no React, DOM, storage, process, filesystem, Git, clock or network effect. `WorkbenchProjectWorkspaceRuntime.applyPublishedWorkbench` replaces the shared part with each publication and publishes synchronously.

New Workspace asks the authority for an ungrouped Workspace; the [Web README](../README.md#server-client-path) gives its name and directory rules. It requires no Project or form and never starts an Agent automatically. Existing terminals keep their current and launch CWD when grouping changes.

### Projected terminal metadata and Tab titles

The application forwards one complete committed terminal metadata observation with the trusted
hostname of the machine that owns the terminal. Workbench normalizes the natural OSC 0/2 title by
trimming outer whitespace and then removing NUL characters. `null` clears the natural title. The
durable fallback title and optional durable custom Tab title remain separate. Display resolution is
custom title, then the focused terminal's latest natural title, then the final non-empty path
segment of its current CWD, then fallback. The Server holds the custom title; setting one trims it,
and blank input asks for nothing. A later natural title remains hidden beneath the custom title.

An OSC 7 current-directory value is accepted only as
`file://<host>/<percent-encoded-absolute-path>` when `<host>` matches the trusted terminal-machine
hostname under ASCII case-insensitive comparison. Empty hosts, `localhost`, other hosts, non-file
schemes, malformed percent escapes, and non-UTF-8 decoded bytes are ignored. Accepted paths are
percent-decoded before replacing `currentCwd`; an ignored value preserves the prior current CWD,
while a reported `null` clears it.

Deterministic scenarios cover natural-title normalization,
custom override, hidden natural updates, local hostname matching, URI decoding,
and rejection classes. Projected metadata supplies no trusted command-execution or SSH state, so the policy does not suppress title or CWD updates on that basis.

## Decisions and transitions

| Event                       | Guard            | Result                                        |
| --------------------------- | ---------------- | --------------------------------------------- |
| `workspace-activated`       | Workspace exists | Select it and disclose its Project or Recents |
| `project-expansion-toggled` | Project exists   | Fold or unfold that Project                   |
| `recents-expansion-toggled` | None             | Fold or unfold Recents                        |

New Workspace, move, ungroup, new Project and move, dissolve, rename, reorder, close, and Worktree commands go to the authority in Workbench's own event vocabulary, as [Workbench](workbench.md#scope) describes, and settle once a publication carries them.

Close Workspace is a destructive Action with the shared confirmation dialog. Dissolve Project is a grouping operation without a destructive confirmation. Neither deletes or relocates a checkout.

## Project Folder acquisition

The sidebar's New Workspace control is immediate. The optional Create Project menu retains Select Folder and New Folder onboarding. Native Desktop selection validates one directory without changing disk state; a composition without that capability asks for one path instead and does not validate it. Folder-derived group names use the final path segment, falling back to the trimmed path for a root.

Rename asks for a Project's or Workspace's name in one field that the Dialog's title labels, prefilled with the current name. The name is kept exactly as typed, as the protocol and the TUI keep it: a name that is blank by Unicode White_Space or longer than 512 characters is refused on submit or blur with a localized reason. Renaming a Workspace keeps the directory it records. New Project… in a Workspace's Move to Project submenu asks the same way for the new Project's name, prefilled with the Workspace's, and commits the Project and that Workspace's membership together. A Dialog whose Project or Workspace leaves closes with it. A Project gains a folder only through a Workspace that records it.

One mounted Workbench permits one native selection, which only creates a Project. Cancellation, unmount, port retirement and replacement revoke the request.

New Folder selects a parent then explicitly submits one child name. Filesystem success and product success remain separate. The retained creation receipt identifies Workspace, Directory, path, context and first presentation; its nullable Project is informational and regrouping does not invalidate it. The workflow awaits canonical publication and exact route acceptance before acknowledgement. A created Folder survives failed or uncertain opening, with path-based Select Folder recovery; unknown effects never trigger another mkdir. Main's lifecycle and filesystem rules are owned by [project-folder-acquisition.md](../../../apps/desktop/docs/project-folder-acquisition.md).

## Server Worktree commands

Where the authority reaches Git through `WorkbenchAuthority.worktrees`, each Workspace menu carries the TUI's Worktree commands. Opening the menu discovers the Workspace's repository again. Until Git answers, New Worktree is disabled and Open Worktree holds one disabled row saying what is known: reading, not a Git repository, checkouts unavailable, or no other checkouts. No earlier answer stands in.

| Entry                    | Action                              | Result                                                                                       |
| ------------------------ | ----------------------------------- | -------------------------------------------------------------------------------------------- |
| New Worktree             | `workbench.worktree.create`         | A checkout with the TUI's defaults opens as a Workspace named with the first unused ordinal  |
| Open Worktree            | `workbench.worktree.open`           | The Workspace standing in the chosen checkout activates; with none, a new one opens there    |
| Remove Worktree…         | `workbench.worktree.remove`         | After one confirmation, the Workspace closes and its checkout is deleted; the branch stays   |
| Resume Worktree Removal… | `workbench.worktree.resume-removal` | After its own confirmation, a removal of this Workspace that stopped continues the operation |

Remove Worktree… follows Delete Workspace and appears only for a checkout the Workspace may remove, which the authority decides. Its confirmation is the shared Action confirmation, naming the Workspace, its terminals, the path, and the kept branch. A removal that stops where it may continue asks at once whether to resume; declining leaves the failure notice, and while the authority keeps that removal, Resume Worktree Removal… takes Remove Worktree…'s place whatever Git reports. Rows wait while a command runs. Progress and outcomes are authority notices: an opened Worktree shows as its activated Workspace, a removal reports for four seconds, and a failure or unknown outcome stays until pressed. A created or opened Workspace activates by the created-Workspace rule below.

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

A Workspace row shows them as the TUI does, in a fixed two-line row with no leading icon: the name
and its Agent fold chevron on the first line, which holds the row tools, and a second line below.
The end area keeps its room at all times, so names truncate at one width: at rest it holds
`↑N`, commits to push in the success role, `↓N`, commits to pull in the warning role, each only when
positive, and the unread dot; under the pointer or focus, Options and Close take its place. The
arrows tell the counts apart without color. The second line names the branch, otherwise the folder,
`~` for home, unless the Workspace's Agents are folded, when it lists their states instead (see
[Workspace Agent overview](#workspace-agent-overview)); a row with neither keeps the line empty. Names
and lines truncate at their end. A truncated name shows whole in a tooltip, and the second line's
tooltip holds the full directory. The arrows are hidden from assistive technology, and the row's
accessible name adds the location and the counts in words. The current Workspace carries
`aria-current="page"` and the current fill.
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

Row tools are hidden at rest. Hover, focus within the owning row, and an open menu reveal them
immediately; hidden controls receive no pointer input. They are never tab stops: the list is one tab
stop, as the TUI's sidebar is one focus region, and the keyboard reaches a row's menu from the row.

The list's keys follow the TUI. Tab enters at the current Workspace, else the heading folded around
it, else the first row. ↑ ↓ Home End move between headings, Workspaces and Agent rows; → unfolds a
row and then moves into it; ← folds it and then moves to its parent. Enter or Space folds a heading
and opens a Workspace or Agent, then hands the keyboard to its terminal, as a pointer click does.
Shift+N starts a Workspace, `/` opens Search and F1 opens the shortcut reference; Ctrl+↑ and Ctrl+↓,
⌘ on macOS, move a Project or Workspace among its siblings, as a drag does; Shift+← and Shift+→
change the width by `10px` and `0` returns it to `250px`; Esc returns to the terminal. The rules
live in the pure `resolveSidebarNavigationKey` and `resolveSiblingReorder`, which unit tests cover.
The global New Workspace and shortcut-reference keys wait for shortcut catalog entries.

The row-local menu adapter uses the following decision model. It owns no Project, Workspace, or
menu state: stable object IDs remain with the existing callbacks, while shared menu primitives own
opening, traversal, dismissal, and focus handoff.
Captured trigger and pre-open focus references are scoped to the exact Project or stable Workspace
ID (`null` identifies the Project row), consumed by `finalFocus`, and released with the owner.
Unchanged pre-open focus is not a later handoff.

| Input or transition                                                                                 | Result and invariant                                                                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact `F2`, `Shift+F10` or unmodified `ContextMenu` from a focused Project or Workspace row control | Focus and invoke that row's live, enabled, closed ellipsis trigger, opening its existing Dropdown Menu once, beside the row. The row advertises these keys through `aria-keyshortcuts`. A pointer opens the menu below its trigger.                                                      |
| Unexpected modifiers, composing or repeated input, a consumed event, or no live enabled trigger     | No invocation and no event suppression. Terminal input has no row listener.                                                                                                                                                                                                              |
| A portaled menu `keydown` event bubbles through the React row                                       | Actual DOM containment rejects it; the open menu keeps keyboard ownership.                                                                                                                                                                                                               |
| A native Menu-key release synthesizes `contextmenu` after the Dropdown Menu opens                   | Suppress only keyboard context events from the row or the exact open dropdown named by its live enabled ellipsis's `aria-controls`. Require that trigger to remain expanded; this also covers portaled autofocus targets. Keep pointer behavior and shared menu state unchanged.         |
| Right-click or long press on the row                                                                | Capture its explicit ellipsis for the Context Menu's existing `finalFocus` callback, retaining the same object target and action entries.                                                                                                                                                |
| Escape or outside dismissal                                                                         | The primitive closes the menu without invoking an action; Escape returns to the valid explicit trigger, while outside interaction retains the primitive's focus policy.                                                                                                                  |
| An action opens a dialog or removes its row                                                         | Return is rejected when the trigger disappears, a dialog owns focus, or another live control has taken focus. Outside, focus-out, and sibling-menu dismissal also suppress explicit return. The adapter schedules no late return and never substitutes a sibling object's action target. |

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

As in the TUI, a held Workspace becomes current the moment the press turns into a drag, without
moving keyboard focus, and stays current whether the drag commits or is cancelled; a Project is never
current. The semantic gesture state is `idle`, `armed`, or `dragging`. `armed` records the pointer identity,
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

A Workspace with no Agent has no Agent disclosure. Each Agent row is one line, as in the TUI: its state glyph, what names the Agent and then its provider in quieter text, and the unread dot; the glyph names its state in a tooltip, and Agent rows have no menu. What names the Agent follows the TUI's rule: the Panel name a person chose, then what the terminal's own title names once this window has read it, then the label the Server gave the terminal at creation. Blank text, an automatic number, a console's launch path, and text that only names the provider, as an Agent's idle title does, name nothing, and the provider then stands alone. The TUI first takes a name a person gave the Agent and an Agent report's title; a Client does not read those from the Server, and it reads a terminal's title only from a terminal it has attached. The glyph and color mapping keeps a Done slot, which waits for the Server's Done state. Status priority is blocked, error, done, working, idle, unknown, with no color-only or animated status.

As in the TUI, every Workspace lists its Agent rows until this Client folds them, and each Workspace folds on its own. A fold is client-local and lasts while its Workspace exists: through regrouping, ungrouping, dissolution, a collapsed Project, a hidden sidebar, and Agents that end and start again. A folded Workspace replaces its location line with its Agents' state glyphs, most urgent first and otherwise in row order, one gap apart; more than five show four and a muted `+N`. The line's tooltip names each distinct state in that order, and while the Agent rows are hidden the Workspace row carries their unread marker and its accessible name says those states and unread. A dragged Workspace hides its own Agent rows until the drag ends, as a dragged Project folds; its siblings keep theirs.

`workbench.agent.navigate` targets only Workspace and terminal Presentation identity, confirms that the terminal still holds an Agent in that Workspace, re-resolves current topology and then selects the exact Tab, Panel and child, removing conflicting zoom. Its existing Action/projection coordinator rejects stale or superseded focus. Agent listing and navigation never start an Agent.

Navigation origin and cancellation use the last committed React location; suspended or discarded renders cannot redirect that intent. Target readiness still comes from the latest Agent observation and runtime topology. Hook tests cover a newer Agent observation between navigation and completion, preserving focus after a discarded location render while canceling it after committed unrelated navigation.

## Dependencies and ownership

Workbench owns the read model, action adapters, dialogs and client-local views. Tab Strip owns Tab semantics, UI owns menu/dialog primitives, and i18n owns copy. The Server owns structure, terminal Agent observations, filesystem validation, Git physical identity, revisions, storage and process reconciliation.

## Correctness and verification

Deterministic tests cover the client-local events and rejected transitions, and snapshot tests cover how publications project into this model. Client View tests cover reconciliation of stale references and its idempotence. Runtime tests preserve context object identity, focus, child selection and zoom across publications.

Hook and component tests cover menus, stable management targets, Agent rows and late activation suppression. The Server owns Worktree Git effects, races and retained partial results; Web owns stable routes and unavailable identities. DOM tests do not establish native-window focus or actual shell process behavior. The repository integration gate remains the integration owner's responsibility.

## Delivery and recovery

This model stores nothing: each window projects it again from the Server's snapshots, which the protocol parser validates before use and never repairs. The Client View accepts only its current version-1 shape and rejects a malformed one without migration. See [publication-and-client-view.md](publication-and-client-view.md).
