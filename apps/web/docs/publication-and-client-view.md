# Workbench Publication and Client View

## Scope and ownership

A window holds two shapes with different authority:

- `WorkbenchPublication` is everything the Server published, in the shapes the window's stores
  hold. `projectServerSnapshot` builds one from each Server snapshot that
  `@orchbee/workbench-client-protocol` has parsed, and `WorkbenchProjectWorkspaceRuntime` takes it
  as the whole shared state. The window never edits it: every change is a `WorkbenchAuthority`
  command that settles once a later publication carries it.
- `WorkbenchClientView` is one window's disposable navigation and layout choices, defined in
  `src/workbench/document`. It never authorizes a product or terminal lifecycle change.

Neither shape performs a storage, IPC, process, network, React, or DOM effect or carries a platform
discriminator. The composing application owns those adapters and parses untrusted values at its
boundary.

## Publication

```text
WorkbenchPublication
├── projects[]: { id, name }                                  (display order)
├── workspaces[]: { id, name, projectId: nullable,
│                   directory: { id, path } | null, terminalContextId }   (display order)
└── terminalContexts[]                    (canonical identity order, one for each Workspace)
    ├── fingerprint
    ├── groups[]: { id, title, color }
    ├── tabs[]: { id, title, customTitle: nullable, group: nullable, pinned, root }
    │   └── root: Panel/Split tree whose presentations carry lifecycle and names
    └── withheldTabIds[]: Tabs the Server holds that the window leaves out for now
```

The projection keeps the Server's identities, order, and structure and adds only what a window needs
to show them. Names follow the TUI's: an automatically named Tab falls back to its place, such as `Tab 2`,
and a terminal that shows no title takes the program the Server launched for it, else its Tab's
place, both in the window's language. A Split weight is the Server's ratio, or an even split when
the Server holds none, so every Client shows the same sizes.

| Server fact                      | Projected lifecycle               |
| -------------------------------- | --------------------------------- |
| A launch failure                 | `launch-failed`, with its reason  |
| No binding                       | `unbound`                         |
| A binding this window saw settle | `exited` or `failed`, as it ended |
| Any other binding                | `running`                         |

A Presentation a restore held because its saved directory was unavailable has no binding, so it
projects as `unbound`; the terminal surface reads the hold from the Server's `launchFailure` and
shows the directory with Retry, Start in home folder and Close, as the TUI does. Retry and Start in
home folder submit `panel.restore-terminal` through the authority's held restores, which report
"Directory still unavailable", "Agent conversation not resumed" or "Terminal not started" on the
status line; nothing retries by itself.

A Presentation's `agentNotResumed` fact says the Server gave up resuming its Agent conversation
after a restart. The authority's resume notices read it from each snapshot and the status line says
it once, for four seconds, only within 60 seconds of the fact's time and in the TUI's words; the
same words already showing keep their time. The
[resume owner](../../orchbee-server/docs/agent-session-resume.md#what-clients-show) defines the fact.

Members a progressing close the window follows are withheld from the projection until that close
settles, from the Server's acceptance of it. A Tab leaves the published Tabs, and a Group that holds
only withheld Tabs leaves with them; every other Tab keeps the place its automatic name carries. A
Panel of several leaves its Tab's split the way the Server removes it: its sibling takes the split's
place, and a Tab left with no Panel leaves too. A focused Panel that leaves hands focus to the
Panel at its place in the Tab's previous order, clamped to the Panels that remain, as the TUI picks
its stand-in. A Workspace leaves with its terminal context, and a
Project's close leaves each of its Workspaces once the Server accepts it, unless that first answer
already reports the close stopped. A context's fingerprint
covers the Server's context, the window's language, the ends the window saw, and the Tabs and Panels
it shows, so it changes exactly when the projected context does. A member whose close is refused,
stops, or is lost shows again with its report, as the Server still holds it; a completed close has
removed it by then.

## Applying a publication

`applyPublishedWorkbench(publication)` reconciles the current Client View against the publication,
then rebuilds only the contexts whose fingerprint changed. An unchanged context keeps its live
`TabModel` and `PanelWorkspaceStore`, so focus, zoom, child selection, and terminal attachments
survive a grouping-only change. A rebuilt context keeps the natural title and current CWD this
window saw each remaining terminal report, and each Tab's Panel focus history. The runtime keeps
the Client View and Panel focus history of each withheld Tab, and a Tab that returns takes them up
again, as the TUI keeps a leaving Tab's navigation memory; the selection stays where it moved. A Tab
that is neither withheld nor published is gone, and its kept view with it.

Applying sends no terminal command: the runtime has no command port, and nothing launches, retries,
or binds a terminal because of a publication. A publication the runtime cannot apply ends the
connection, as the [Web README](../README.md#server-client-path) describes.

Readers observe the runtime: `subscribe` for each publication and each location or expansion
change, `subscribeClientView` for this window's view, `subscribeNavigation` for its location, active
Tab, and focus, and `subscribeTerminalTopology` for the terminals it holds. The last three notify
only when their own projection changed.

## Client View

```text
WorkbenchClientView
├── version: 1
├── activeLocation: no-workspace | workspace(WorkspaceId)
├── expandedProjectIds, recentsExpanded
└── terminalContexts[]                                   (canonical identity order)
    ├── active and selected Tabs, selection anchor, most recent Group, collapsed Groups
    └── per Tab: focused and maximized Panel, each Panel's displayed child presentation
```

Switching a Workspace or Tab, focusing or maximizing a Panel, showing one child terminal, or
collapsing a Group changes only the Client View. Split hierarchy, orientation, and weights are
published. As in the TUI, the no-Workspace location holds no terminal context, and a window is
there only while no Workspace exists.

The live per-Tab Panel focus history is runtime state initialized from each Tab's focused Panel. It
is not serialized, so initial hydration does not restore earlier focus visits.

Terminal viewport state is outside both shapes. Each projected terminal client owns its
`AttachmentId`, dimensions, scroll position, search, selection, copy state, GPU resources, and
renderer caches. `TerminalId`, binding identity, PTY bytes, VT state, output, and history remain
terminal-runtime authority.

## Reconciliation

`createDefaultWorkbenchClientView(publication)` derives one complete deterministic view.
`reconcileWorkbenchClientView(publication, candidate)` keeps every choice that still names something
published and derives the rest: the first Tab, the first Panel, no maximized Panel, and the active
Tab's Group. A location whose Workspace is gone moves to the next Workspace after it that remains
in the sidebar order the window showed, Projects first and then Recents, else the one before it, as
the TUI lands when the current Workspace closes; without either, to the first Workspace the sidebar
lists, and to No Workspace once none is left. Since a closing Workspace leaves at the Server's
acceptance, the selection lands on that neighbour at once, and on No Workspace when no other
Workspace is shown. The TUI keeps such a last closing Workspace current until publication, because
its canonical projection admits the no-Workspace location only for a snapshot without Workspaces;
the window has no such limit. A Workspace whose close then stops or is lost shows again, and the
selection stays where it landed, unless it landed on No Workspace, which gives way to it.
Reconciliation is idempotent and never changes the publication.

An action that changes shared structure and implies navigation changes the two shapes separately:
creating a Tab adds it for every Client and selects it only in the window that asked. Every other
window reconciles on its own.

The Client View lives in memory. Closing a window destroys it, and a new one receives the default
unless the application supplies a candidate. The Desktop composition stores only one bounded,
session-scoped focused `TerminalPresentationId` and projects it into an otherwise default candidate,
as [renderer reload focus recovery](navigation-routing.md#renderer-reload-focus-recovery)
describes. Full or cross-restart Client View recovery requires a separate accepted owner and
contract.

## Invariants

- The protocol parser admits only a snapshot whose identities are unique in their scopes, whose
  Workspaces and Terminal Contexts name each other exactly once, whose pinned Tabs form one
  ungrouped prefix, and whose Groups are contiguous. The projection preserves these and repairs
  nothing.
- Every Split has exactly two children with unique slots, and every presentation appears exactly
  once. The runtime checks each rebuilt Panel workspace and fails rather than show a broken one.
- A reconciled Client View has exactly one entry per published Terminal Context and Tab and contains
  only resolving references.
- Strict parsing rejects a malformed Client View; only well-formed stale references are reconciled.

## Correctness evidence

| Claim                                                                                        | Evidence                                                                                                  | Limits                                         |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| The projection keeps the Server's structure and names terminals as the TUI does              | [Projection tests](../src/workbench-client/server-workbench/__tests__/projectServerSnapshot.unit.test.ts) | Snapshots come from the shared test fixture    |
| An unchanged context keeps its models; a changed one keeps this window's choices and reports | `applyPublishedWorkbench` cases in the projection and runtime tests                                       | One renderer runtime                           |
| Client Views are isolated and reconciliation is idempotent                                   | Two-view examples, stale-reference regression, and fixed-seed properties                                  | Generated bounds: eight Tabs, four Panels each |
| A Tab without a chosen name follows its terminal                                             | Runtime title scenario across current directory and natural title                                         | The Server owns chosen names                   |
| Client View observers hear this window's choices, not a Server rename                        | Runtime observer tests                                                                                    | Shared ordering is Server authority            |
