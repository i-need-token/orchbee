# Project and Workspace Model

The legacy Browser ingress and POSIX SSH Environment chain are retired. The retained Environment data model and optional SSH capabilities below are library contracts. Current Desktop supplies only local Folder and Worktree capabilities; Browser Preview uses in-memory state. Neither supplies SSH enrollment, remote terminals, or connection controls.

## Status and scope

This is the implementation contract for Workbench-owned Project, Directory, Workspace,
client-local active location, terminal-context composition, and browser Project onboarding.
Workbench maintains a per-client live projection over the canonical shared structure and exposes
their ownership split through [`shared-workbench-document.md`](shared-workbench-document.md).
Filesystem mutation, Git effects, PTY ownership, storage I/O, and cross-device synchronization
remain outside this contract.

## Problem

A person must be able to keep standalone terminal work available while organizing other terminal
contexts by Project and Workspace. The current flat Workbench projection cannot preserve several
independent Tab and Panel contexts or determine their default CWD from a Project Directory.

## Inputs and validation

| Input                                                   | Trust and absence                                                                  |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `ProjectWorkspaceState`                                 | Trusted only after construction or transition invariant checks                     |
| Project, Directory, Workspace, and terminal-context IDs | Stable opaque strings; missing or stale IDs reject                                 |
| Directory descriptor                                    | One canonical Folder or Worktree kind plus explicit local or Environment placement |
| Managed Worktree commit                                 | One host-validated created Worktree and one current source Directory               |
| Existing Worktree adoption                              | One host-revalidated discovered Worktree and one current source Directory          |
| Browser-entered Folder path                             | Trimmed, non-empty user input; otherwise preserved and not filesystem-verified     |
| Display name                                            | User or localized product input; duplicate names are allowed                       |
| Launch-configuration reference                          | Stable Project-local ID and display name; executable payload remains host-owned    |
| Explicit terminal Directory                             | Must be an available member of the target Project                                  |
| Split CWD                                               | A host-reported local CWD; absence continues through the fallback chain            |
| System CWD                                              | Required final fallback supplied by the composition host                           |

The first delivery receives normalized Directory descriptors and performs no filesystem lookup.
The browser preview additionally accepts a manually entered path and constructs a synthetic Folder
Directory for its simulated terminal projection. Its `available: true` means the current preview
can use the path as launch CWD; it is not evidence that the path exists on disk. Later platform
adapters must validate external filesystem and Git data before constructing authoritative local
descriptors.

Each Directory is the current Checkout-bearing entity and has exactly one placement:

```text
placement = local | environment(EnvironmentId)
```

The Environment reference is an opaque non-empty string to Workbench. The composing application
owns registry validation. Path uniqueness is scoped by placement, so the same spelling may exist
locally and in distinct Environments. Directory refresh cannot change placement. Ordinary Folder
acquisition, Git Worktree, Workspace creation/edit/duplicate, and terminal-CWD flows are local-only
and require `placement: local`. Creating or editing an ordinary Workspace or resolving its terminal
CWD additionally requires that local Directory to be available. An existing local Workspace remains
structurally valid if its Directory later refreshes unavailable, but it cannot launch until that
Directory is available again. The authoritative remote-registration exception is
`environment-workspace-registered`: it may reference a forced-unavailable Environment Directory
and creates an empty terminal context without a terminal presentation or Host effect. After that
placement is durable, the composing Main process may use the dedicated remote-terminal operation
described below; ordinary local creation and CWD resolution remain unchanged.

## Data

```text
ProjectWorkspaceState
├── activeLocation: standalone | workspace(ProjectId, WorkspaceId)
├── standaloneTerminalContextId
├── expandedProjectIds
└── projects[] (ordered)
    └── Project
        ├── id, name
        ├── directories[] (ordered, one or more)
        ├── primaryDirectoryId
        ├── launchConfigurations[] (ordered stable references)
        └── workspaces[] (ordered, zero or more)
            └── Workspace
                ├── id, name
                ├── primaryDirectoryId
                ├── launchConfigurationId (optional)
                └── terminalContextId
```

`Directory` is the internal domain term. Product presentation uses Folder. A Git Worktree is a
Directory with `kind: worktree` and required Worktree Git metadata. `source` records how the
Directory entered the Project and never determines its semantic kind. Folder Directories have
either repository metadata with `worktree: false` or no Git metadata.

The first delivery models launch configurations as Project-local stable references containing an
ID and display name. A Workspace selects one reference or `null`. Command, arguments, environment,
and other executable payload remain outside this delivery and require a later accepted platform
contract.

Each terminal-context ID owns one independent Tab model and Panel workspace. Project and Workspace
state references that context; it does not duplicate Tab, Panel, or Terminal Session state.

## Invariants

- Project, Directory, Workspace, and terminal-context IDs are unique in their owning state.
- Project order is a permutation of the current Project identities; reordering never changes
  membership, active location, or disclosure intent.
- Every Project owns at least one ordered Directory.
- A Project references at most one Environment identity across all of its Directories; local
  Directories may coexist with any number of distinct paths from that Environment.
- Every Directory's discriminated kind agrees with its Git metadata.
- Every Project `primaryDirectoryId` names one of its Directories.
- Launch-configuration IDs are unique within their Project.
- Every Workspace belongs to exactly one Project and its `primaryDirectoryId` names one Directory
  in that Project. Ordinary Workspace transitions require local placement; only the authoritative
  remote-registration transition may introduce an Environment-placed primary Directory.
- Every non-null Workspace `launchConfigurationId` names one Project launch configuration.
- Each Workspace has one terminal-context ID, and no two Workspaces or the standalone terminal
  share a terminal context.
- `activeLocation` is standalone or names one current Workspace in one current Project.
- Project expansion changes only disclosure state and never changes `activeLocation`.
- Display names need not be unique and never serve as identity.
- A transition that rejects or reports unchanged preserves the complete input state.

Removing Directories preserves referential integrity. For every removed Project primary Directory,
choose the first surviving Directory after it in the Project's previous stable order, wrapping to
the first surviving Directory when necessary. A Workspace primary uses the same circular order but
skips every remote Directory; removal rejects if no local fallback survives. Removing the final
Project Directory also rejects.

## Outputs

`transitionProjectWorkspace` returns the complete next state, an `applied`, `unchanged`, or
`rejected` outcome, a named rejection when applicable, and terminal-context commands. Commands
create an empty or populated context, clone a context, or dispose an in-memory context; the pure
transition performs no React, DOM, storage, process, filesystem, Git, time, random, or network
effect.

Each terminal presentation records immutable `launchCwd` separately from host-reported `currentCwd`.
Workbench owns its opaque presentation ID and discriminated lifecycle, while the application owns
non-reusing identity allocation and command dispatch. Host terminal identity never enters this
model.
`resolveTerminalCwd` returns one launch path and its source:

```text
explicit Project Directory
→ split terminal CWD
→ available Workspace primary Directory
→ available Project primary Directory
→ system CWD
```

An invalid explicit Directory rejects in the Workbench runtime before terminal creation.
Unavailable local default Directories are skipped. An Environment-placed Workspace primary rejects
as `workspace-directory-remote` before Project or system fallback, so the local runtime cannot turn
a remote Workspace into a local shell accidentally. The host may report a terminal's current CWD
without changing its launch CWD. Panel split and child-terminal creation inherit current CWD;
Workspace duplication starts fresh shells from recorded launch CWD. Changing a Project or Workspace
primary Directory never changes an existing terminal CWD.

`createDesktopSshEnvironmentRemoteTerminal` accepts a correlated request identity and derives one
deterministic Presentation identity. An exact replay returns that Presentation; a distinct request
appends one default top-level Tab, one primary Panel, and one `provision` Presentation to the same
compatible ungrouped remote Terminal Context. It preserves active location, Project disclosure, and
other Client View state, rejects Environment mismatch,
Group/split/duplicate/explicit-directory/insertion-index placement, and never falls back to a local
CWD. The durable canonical commit precedes every Host effect. `resolveActiveTerminalCreationRoute`
keeps the Action available for a compatible remote context whenever the Electron capability and
default placement are present; Main still revalidates exact authority and the Action waits for
canonical publication before selecting and focusing the created Tab.

### Semantic command projections and terminal creation

`@orchbee/workbench/semantic-commands` is the DOM-, React-, transport-, and storage-independent
entry point for approved Workbench inspection and exact-target terminal creation. It consumes a
validated `WorkbenchSharedDocument` for reads and the authoritative
`WorkbenchProjectWorkspaceRuntime` for mutation; it never edits a persisted document or dispatches
an application commit itself.

| Modeling field | Contract                                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | A nonvisual caller needs stable Project, Directory, Workspace, and terminal identities, plus one terminal-create operation that cannot drift to a focused client object.                                                                                                                                                                                                                                            |
| Inputs         | Read projections accept a validated shared document. The public terminal-create contract accepts only an exact `WorkbenchLocationRef` and optional Project Directory. The owner operation additionally receives the owner-supplied fallback title, fixed top-level placement, and current Workbench runtime. Missing or stale identities reject.                                                                    |
| Data           | Project, Directory, Workspace, terminal-context, Tab, Panel, presentation, and durable intent identities remain owned by the existing Workbench models. The operation creates no parallel Store.                                                                                                                                                                                                                    |
| Outputs        | Reads return complete canonically ordered snapshots with their input document revision. The owner operation returns exact terminal-context, Tab, Panel, presentation, and preview/provision intent identities, or one typed rejection. The public committed output adds the composing application's stored revision.                                                                                                |
| Invariants     | Reads never consult Client View. Creation changes exactly one resolved terminal context, preserves active location and every existing active Tab selection, creates one top-level Tab with one primary Panel presentation, and leaves every other context unchanged. An empty target context adopts the sole created Tab as its model-required validity fallback. Rejection preserves the complete Workbench state. |
| Decisions      | Project, Directory, and Workspace order follows the shared document. Terminal order is Tab order, depth-first Panel order, then primary presentation before child presentations. Exact workspace resolution checks Project membership before Workspace identity.                                                                                                                                                    |
| Model          | Read operations are deterministic projectors. `executeWorkbenchTerminalCreate` is the principal synchronous operation and delegates the one mutation to `WorkbenchProjectWorkspaceRuntime#createTerminalAtLocation`.                                                                                                                                                                                                |
| Transition     | Resolve exact location → resolve optional/default CWD → validate Tab placement and allocate identities → invoke the commit callback → add one non-active Tab when another Tab exists, otherwise install the first-Tab validity fallback → reconcile its Panel presentation. The product Action may then select only the returned exact Tab as a separate Client View projection.                                    |
| Dependencies   | `@orchbee/tab-strip` owns Tab insertion invariants; Workbench owns Project/Workspace resolution, Panel construction, presentation lifecycle, and command projection; the composing application owns identity allocation, revision assignment, durable commit, and process reconciliation.                                                                                                                           |
| Verification   | Deterministic examples cover every read projection, rejection class, and empty-context first-Tab fallback. Fixed-seed stateful properties cover valid, stale, unavailable-Directory, and cross-Project traces, exact-context mutation, identity uniqueness, shared-document validity, and unchanged existing active Client View selection.                                                                          |

Workbench returns complete read snapshots rather than pages. The Desktop semantic-command
coordinator paginates a snapshot and binds its opaque HMAC cursor to operation, target, coordinator
generation, and revision without moving cursor policy into Workbench. A managed creation result
means that one durable `provision`
intent exists in the Workbench projection; only the application coordinator may relabel that result
as durably committed after its canonical revision is stored. Preview creation remains explicitly
distinguished and cannot enter a shared document. The operation performs no terminal process,
network, filesystem, or Client View navigation effect.

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

| Intent                       | Guard                                                          | Result                                                                | Terminal-context command                                              |
| ---------------------------- | -------------------------------------------------------------- | --------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Activate standalone terminal | None                                                           | Standalone becomes active                                             | None                                                                  |
| Toggle Project expansion     | Known Project                                                  | Only disclosure changes                                               | None                                                                  |
| Reorder Project              | Known Project and optional known following Project             | Project moves before that Project, or to the end when absent          | None                                                                  |
| Reorder Workspace            | Known Project, source Workspace, and optional sibling target   | Workspace moves within that Project, or to the end when absent        | None                                                                  |
| Activate Workspace           | Known Project and Workspace membership                         | Workspace becomes active and Project expands                          | None                                                                  |
| Create Project               | One normalized available Directory                             | Project, Primary Workspace, and active location are created           | Create one context with one Tab, Panel, and shell projection          |
| Register remote Project      | One unavailable Environment Folder; unique placement and path  | Project with no Workspace is appended; active location is unchanged   | None                                                                  |
| Register remote Directory    | Known Project; unavailable Folder in its sole Environment      | Directory appends; Project and Workspace defaults remain unchanged    | None                                                                  |
| Register remote Workspace    | Known Environment Directory without its admitted Workspace     | One inactive Workspace appends; Project disclosure remains unchanged  | Create one empty context; no Tab, Panel, presentation, or Host effect |
| Add Directory                | Known Project; location not already present                    | Directory appends to stable order                                     | None                                                                  |
| Register created Worktree    | Known Project and available source; unique valid Worktree      | Directory appends; optional Workspace appends, expands, and activates | Create one context only when the Workspace option is enabled          |
| Adopt existing Worktree      | Known Project and available source; unique discovered Worktree | Directory and Workspace append, expand, and activate                  | Create one context rooted at the adopted Worktree                     |
| Refresh Directory            | Known identity; unchanged kind and source; unique path         | Host observations update in place; order and references remain        | None                                                                  |
| Remove Directories           | At least one Directory survives; only Folder kinds requested   | Referencing primaries use circular next-survivor selection            | None                                                                  |
| Reconcile launch references  | Known Project; unique non-empty reference IDs                  | Catalog replaces; removed Workspace selections become `null`          | None                                                                  |
| Set Project primary          | Available Project Directory                                    | Future Project defaults change                                        | None                                                                  |
| Create Workspace             | Available Project Directory                                    | Workspace appends, Project expands, Workspace becomes active          | Create one context with one Tab, Panel, and shell projection          |
| Edit Workspace               | Known Workspace and available Project Directory                | Name or future default changes                                        | None                                                                  |
| Duplicate Workspace          | Known Workspace with a local primary Directory                 | Copy appends and becomes active                                       | Clone layout and names into fresh Tab, Panel, and Terminal identities |
| Delete Workspace             | Known Workspace                                                | Workspace is removed; active deletion falls back to standalone        | Dispose its context                                                   |
| Remove Project               | Known Project                                                  | Project and its Workspace metadata are removed; disk locations remain | Dispose every Project Workspace context                               |

Remote Project, Directory, and Workspace registration are pure Workbench transitions intended only
for an authoritative application owner. Project registration allocates through the existing
sequence, accepts the owner-supplied Directory identity, and emits no command. Directory
registration appends one forced-unavailable Environment Folder to an exact existing Project only
when that Project has no other Environment identity; an exact duplicate is unchanged, while a
cross-Project location or second Environment rejects. It changes no Project name, primary Directory,
Workspace, active location, identity sequence, or terminal context. Workspace registration allocates
one Workspace and terminal-context identity for that Environment Directory but leaves the active
location and Project disclosure unchanged; its only command materializes an empty context. Desktop
owns remote inspection, registry authorization, random Directory identity, durable idempotency, and
proposal-origin policy.

Remote terminal provision is a separate Main-owned operation over that already admitted placement.
It is not a general Project transition and is not exposed through the ordinary local terminal-create
semantic command. Desktop owns Environment connectivity, target-specific Host selection,
commit-before-effect ordering, retry reconciliation, and process execution; Workbench owns only the
exact structural mutation and its rejection rules.

The runtime's optional transition commit callback runs only for an `applied` result, immediately
before any terminal-context command or authoritative state replacement. Action adapters use it to
record their commit point. After replacement, the runtime synchronously publishes the new snapshot;
an observer failure is isolated and cannot relabel the transition or prevent later observers from
receiving it.

Delete Workspace and Remove Project remain Action-owned destructive transitions. When invoked
from a dropdown, context menu, shortcut, or another Action source, their confirmation uses the shared
modal Alert Dialog centered in the viewport with a page-wide backdrop. `Escape` cancels and `Enter`
confirms; no destructive transition commits before confirmation is accepted.

Preview Workspace duplication copies the primary Directory, launch-configuration selection, Tab groups,
Panel layout, presentation names, and each terminal's recorded launch CWD. It uses a whitelist:
Tab and Group attention, loading, network, crash, permission, discard, capture, memory, and other
runtime observations do not copy; presentations reset to preview lifecycle with fresh identities.
It never copies
process identity, output, command history, Worktrees, or source files. The in-memory preview creates
ordinary shell projections with fresh identities. Managed composition rejects terminal-context
cloning; Desktop does not expose Project or Workspace duplication in this integration stage.

## Project Folder acquisition

The browser application starts with only the standalone terminal context. The Project create
control first opens the same Project creation menu used by platform applications. Selecting
`Select Folder…` is the narrow browser adaptation point: instead of a native Folder picker, the
Workbench preview opens a dialog that accepts one Folder path. `New Folder…` remains visible but
disabled because this delivery performs no filesystem mutation. Input processing trims outer
whitespace, rejects an empty result, and otherwise preserves the path. The default Project and
Folder display name is the final non-empty segment separated by `/` or `\\`; a root-like path with
no ordinary final segment falls back to the complete trimmed path.

A valid submission invokes one Project-create Action. The Action constructs a synthetic Folder
Directory with `git: null`, `kind: folder`, and `source: selected`, then uses the existing atomic
Project transition. Cancellation and invalid input leave state unchanged. The flow performs no
filesystem or Git call and provides no persistence or real terminal-process claim.

The Project management Dialog uses the same narrow browser adaptation for adding an existing
Folder. Its `Add Folder` control owns the final row of the Folder list. Activating it replaces only
that row with a path field and compact Cancel and Add icon actions; their Tooltips expose the action
labels and keyboard hints while the Folder rows, Project-name field, and outer Cancel and Save
actions remain stable. `Enter` or the Add icon action trims and submits a non-empty path; `Escape` or
the local Cancel icon action discards only that inline draft and keeps the Dialog open. An exact path
already in the Project is not submitted and receives an inline explanation. A valid path invokes
one Project-directory-add Action, which constructs the same
synthetic Folder Directory and appends it through the existing `directory-added` transition. The
Project name draft, primary Directory, Workspace defaults, active location, and existing terminal
contexts do not change. Folder addition commits independently from the Project-name Save action,
matching the existing immediate Set Primary and Remove Folder operations.

The Folder list omits primary designation and Set Primary controls while the Project owns only one
Folder because the choice is implicit. With multiple Folders, the primary row carries a quiet
designation while each available non-primary row reveals its raised Set Primary action only when
the row is hovered or keyboard focus is within it. Hovering or focusing a Folder name exposes the
complete path through the shared Tooltip; the displayed name remains compact and may truncate.

When a future host supplies both local and SSH acquisition, `Add Folder` opens an explicit choice between
**Add Local Folder…** and **Add SSH Folder…**. The SSH callback carries only the exact current Project
identity and leaves the manager open while the application-owned overlay connects and browses the
remote host. Every canonically published Environment Folder has a visible localized **SSH** marker
with an accessible remote-folder label; local Folders remain unmarked, and the existing path Tooltip
continues to expose the complete canonical path.

Desktop injects a narrow existing-Folder acquisition capability at the same adaptation point. Its
Electron Main owner opens one native single-directory picker, normalizes the selected absolute path,
and checks that the path still names a readable, traversable Directory before returning it. The
check follows a symbolic link while retaining the selected path spelling; it performs no directory
creation, enumeration, Git discovery, or other filesystem mutation. Browser Preview clients
receive no native acquisition port and continue to use the manual path-entry presentation above.

One mounted Workbench admits at most one native selection. A successful selection invokes the same
Project-create or Project-directory-add Action as browser entry. Cancellation is a no-op. Bounded
picker or validation failures open one localized diagnostic and perform no Project mutation. Before
adding a selected Folder, Workbench rechecks the latest Project snapshot: a removed Project or an
exact path that another operation already added rejects without mutating another target. Unmount,
renderer-document replacement, port retirement, window close, and application disposal revoke the
request; a later native completion cannot commit. The maintained Desktop policy and evidence limits
live in
[`apps/desktop/docs/project-folder-acquisition.md`](../../../apps/desktop/docs/project-folder-acquisition.md).

## SSH Project enrollment and connection controls

Workbench accepts one optional platform capability for Environment-placed Projects. When the host
supplies it, the Project-create menu adds **Connect via SSH…**; absence removes that entry completely.
The callback opens an application-owned workflow and carries no Project, Environment, path, SSH, or
credential payload through Workbench. Browser Preview therefore keep the shared Project
menu without claiming this effect.

A separate optional callback extends only the Project manager's `Add Folder` row. It carries the
canonical target Project identity to the application-owned SSH workflow but never accepts an
Environment identity, Directory identity, path, credential, or remote listing through Workbench.
The resulting Folder appears only through ordinary canonical document publication.

Each admitted Environment Project may have one process-local connection status: `blocked`,
`connected`, `connecting`, `disconnected`, `interaction-required`, `retrying`, or `unavailable`.
Workbench projects that status as a quiet indicator on the Project row and includes its localized
meaning in the row's accessible name. The Project action menu offers Disconnect only while
connected; Connect while disconnected; Retry while blocked or unavailable; and a disabled status
item while connecting, retrying, or waiting for interaction. Invocations carry only the canonical
Project identity to the injected capability. Workbench does not start SSH on mount, persist status,
answer prompts, mutate the Project document, navigate, or create a terminal from these controls.

These optional capabilities have no current application provider, so they perform no SSH enrollment or connection effect. A future provider requires a separately reviewed design and admission.

## Managed Worktree product commit

Workbench keeps Git Worktrees and Workspaces as separate domain objects. A platform owner may
submit `project-worktree-created` only after it has validated and created one Git Worktree. The pure
transition verifies the current Project, the available source Directory, repository consistency,
the created Worktree descriptor, and Directory identity and path uniqueness. It then appends the
Worktree Directory and, when requested, appends and activates one Workspace whose primary Directory
is that Worktree. The optional Workspace and its terminal context are part of the same transition;
when the option is off, active location and identity sequences remain unchanged.

The transition performs no Git or filesystem effect and does not select a physical path. Desktop
owns the current effectful adapter, fixed managed location, canonical-document transaction,
compensation, and recovery. Its maintained contract is
[`apps/desktop/docs/project-worktree-creation.md`](../../../apps/desktop/docs/project-worktree-creation.md).
Existing Folder or Worktree choices remain available to the ordinary Workspace-create transition,
so several Workspaces may reference one Directory.

## Existing Worktree adoption

Workbench accepts a host-revalidated existing Worktree through `project-worktree-adopted`. The pure
transition requires an available source Directory, a valid available Worktree descriptor with
`source: discovered`, repository consistency, and unique Directory identity and path. It atomically
appends the Directory and one Workspace, expands the Project, activates that Workspace, and creates
one terminal context rooted at the Worktree. A detached Worktree keeps `branch: null` and its exact
`HEAD`.

Desktop owns read-only Git discovery, opaque Worktree selection, point-in-time revalidation, and the
canonical-document commit. If the selected path became represented after discovery, Desktop uses
the ordinary `workspace-created` transition against that equivalent Directory instead of adding a
duplicate. Git is never changed. The maintained platform contract and evidence limits live in
[`apps/desktop/docs/project-worktree-adoption.md`](../../../apps/desktop/docs/project-worktree-adoption.md).

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
Captured trigger and pre-open focus references are scoped to one Project and its exact Workspace
IDs (`null` identifies the Project row), consumed by `finalFocus`, and released with the owner.
Unchanged pre-open focus is not a later handoff.

| Input or transition                                                                             | Result and invariant                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact `Shift+F10` or unmodified `ContextMenu` from a focused Project or Workspace row control   | Focus and invoke that row's live, enabled, closed ellipsis trigger, opening its existing Dropdown Menu once. The row advertises these keys through `aria-keyshortcuts`.                                                                                                                  |
| Unexpected modifiers, composing or repeated input, a consumed event, or no live enabled trigger | No invocation and no event suppression. Terminal input has no row listener.                                                                                                                                                                                                              |
| A portaled menu event bubbles through the React row                                             | Actual DOM containment rejects it; the open menu keeps keyboard ownership.                                                                                                                                                                                                               |
| Right-click or long press on the row                                                            | Capture its explicit ellipsis for the Context Menu's existing `finalFocus` callback, retaining the same object target and action entries.                                                                                                                                                |
| Escape or outside dismissal                                                                     | The primitive closes the menu without invoking an action; Escape returns to the valid explicit trigger, while outside interaction retains the primitive's focus policy.                                                                                                                  |
| An action opens a dialog or removes its row                                                     | Return is rejected when the trigger disappears, a dialog owns focus, or another live control has taken focus. Outside, focus-out, and sibling-menu dismissal also suppress explicit return. The adapter schedules no late return and never substitutes a sibling object's action target. |

`WorkbenchProjectWorkspace.unit.test.tsx` covers target identity, key guards, cancellation, modal
handoff, removal, and unchanged active location and disclosure. The navigation Stories are fixtures
for real-browser Tab/Shift+Tab, computed visibility, single-menu, and focus-return traces; DOM-only
tests do not establish CSS reachability. Capability-dependent entries and localized reasons remain
the existing action model's responsibility. This synchronous adapter introduces no async owner or
additional TLA+ model.

The New Workspace submenu first presents Project Folder choices. When the host supplies Worktree
discovery, each represented Git repository with existing Worktrees contributes a later group labeled
by its source Project Directory; multiple repositories remain separate groups. Worktrees already in
the Project and newly discovered external Worktrees share their repository group. Reopening a
Project menu refreshes this point-in-time projection. Selecting a current Directory invokes ordinary
Workspace creation; selecting an external Worktree invokes host adoption and creates the Workspace
only after revalidation and canonical commit.

### Created Workspace activation

The client that creates a Workspace selects it after the creation is committed. Ordinary Folder or
already-registered Worktree choices use the synchronous `workspace-created` transition, which
appends and activates the Workspace in one Workbench commit. Managed Worktree creation and external
Worktree adoption commit through the application owner, so their result identifies the Workspace
but the initiating renderer may receive that result before it observes the newer shared document.
Workbench therefore waits for that exact Project and Workspace identity in its current projection
before invoking the existing Workspace-activation Action. It never infers intent from an unrelated
Workspace appearing in the shared document, so another client cannot steal the initiating client's
active location.

| Modeling field | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | A person who creates a Workspace from the current Workspace otherwise has to find and select the result manually, while application-owned Worktree commits may publish after their success result reaches the initiating client.                                                                                                                                                                                                                                |
| Inputs         | A trusted successful managed-Worktree creation result with a non-null `workspaceId`, a trusted successful external-Worktree adoption result, and the current validated `ProjectWorkspaceState`. Failure, cancellation, a null creation `workspaceId`, and unmount are explicit absence cases.                                                                                                                                                                   |
| Data           | `WorkspaceCreationActivationState` is `idle`, `waiting(ProjectId, WorkspaceId)`, or `disposed`. The waiting target is client-local intent and never enters the shared document.                                                                                                                                                                                                                                                                                 |
| Outputs        | Once the exact target is present, one `activate-workspace(ProjectId, WorkspaceId)` command delegates to the existing Action and navigation port. Every other transition emits no command.                                                                                                                                                                                                                                                                       |
| Invariants     | Activation is never requested before exact Project membership and Workspace identity are observed; one waiting target owns at most one command; repeated identical results are unchanged; failure, null results, and disposal never activate; shared Project/Workspace membership is never mutated by this adapter.                                                                                                                                             |
| Decisions      | A current Folder or represented Worktree uses the synchronous creation transition. A successful application-owned result waits if publication is late and activates immediately if publication is already visible. A newer successful creation intent supersedes an older unpublished intent, so the person's latest completed request wins.                                                                                                                    |
| Model          | `transitionWorkspaceCreationActivation` is the deterministic state machine. The sidebar Hook supplies normalized successful identities and each complete current published-identity set, then executes returned activation commands. Each model Target atom is an injective normalized `(projectId, workspaceId)` pair; exact pair construction and cross-Project alias rejection remain deterministic resolver claims.                                         |
| Transitions    | `mount` begins one active lifetime in `idle`; `request(target)` records or replaces the waiting target; `observe(publishedTargets)` replaces the prior current set, including with a removal or empty set, and activates and returns to `idle` only when that current set contains the exact target; `dispose` suppresses later requests and observations for that lifetime.                                                                                    |
| Dependencies   | The platform Worktree capability owns Git, filesystem, canonical commit, and typed results. `ProjectWorkspaceState` owns membership. The existing Workspace Action and application navigation port own client-local activation and routing.                                                                                                                                                                                                                     |
| Ownership      | `@orchbee/workbench` owns initiating-client intent and the deterministic wait; the application owns canonical publication ordering; no new public capability, persistent state, or document writer is introduced.                                                                                                                                                                                                                                               |
| Verification   | State-machine tests cover publication-before-result, result-before-publication, exact identity, replacement, repetition, failure absence, and disposal. Hook and Workbench integration tests cover managed creation, adoption, and ordinary Folder creation. `WorkspaceCreationActivation` checks current-projection safety, including addition, removal, and empty replacement, latest-intent safety, disposal, and progress while membership remains current. |
| Correctness    | The refinement maps a typed successful capability result to `request`, each complete rendered canonical Project/Workspace projection to one replacing `observe`, and the sole model command to the existing activation callback. Withdrawal of the target resolves the formal progress antecedent. The application commit and document transport remain trusted; operating-system and Git effects remain outside this claim.                                    |

## Project and Workspace reorder gestures

The sidebar supports primary mouse or pen pointer reordering for two or more top-level Projects and
for two or more Workspace rows within one expanded Project. Project and Workspace ellipsis controls,
cross-Project Workspace movement, cross-section movement, Project nesting, touch long-press, and
keyboard sorting remain outside this delivery. The initial Workspace created with a Project has no
fixed ordering role; every Workspace in the owning Project participates in the same ordering model.

The semantic gesture state is `idle`, `armed`, or `dragging`. `armed` records the pointer identity,
source identity, owning reorder scope, and activation origin without changing authoritative order or
Project disclosure. Movement of at least four CSS pixels activates `dragging`. The DOM adapter
projects stable direct sibling-row layout offsets into viewport coordinates and reports the first
non-source sibling whose vertical midpoint is below the dragged source row's vertical midpoint as
the nullable `beforeId`; no such sibling means the end of that list. Offset parents outside the list
must not be counted twice, and the pointer's grab position cannot delay or advance a squeeze crossing.
Project hit testing uses only top-level Project rows. Workspace hit testing uses only the Workspace
rows in the source Workspace's owning Project.

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
`workspaces` array of the identified Project; it preserves Workspace identities, active location,
terminal contexts, Project order, Project disclosure, and all Workspace fields. Missing Project,
source Workspace, or target Workspace identities reject atomically. Self-targeted and
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

The Project and Workspace navigation surface projects live Agent Sessions from every Workspace
terminal context without becoming an Agent-state owner. The canonical hierarchy remains Project →
Workspace → Tab → Panel → primary or child terminal presentation → zero or one Agent Session. The
top Tab Strip remains the only persistent Tab manager; the sidebar never repeats Tabs, terminal-only
rows, Panel thumbnails, prompts, commands, tool activity, provider errors, or internal identities.

| Modeling field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem               | A person working in one Workspace must be able to notice and reach live Agents in another Workspace without mounting its inactive terminal surfaces.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Inputs                | One validated `AgentRuntimeSnapshot`, the current `ProjectWorkspaceState`, and the latest Tab and Panel topology of every referenced terminal context. Only `lifecycle: live` Sessions with a matching local presentation attachment are represented.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Projection            | `projectWorkbenchAgentOverview` counts and lists Agent Sessions, not attachments or provider marks. One Session attached to several presentations in one Workspace appears once; the same Session attached to several Workspaces appears once in each. Parent and child Agent Sessions remain independent flat rows. Preferred destinations are navigable before non-navigable, then current Tab order, canonical Panel leaf order, primary presentation, child-presentation order, and stable Session identity. Status changes never reorder rows.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Workspace summary     | A Workspace with no represented live Session keeps the existing layout icon and has no Agent count, status badge, or Agent disclosure. One distinct provider uses its curated mark, two use two complete marks in fixed provider-registry order with one group status badge, and three or more use the provider-neutral multi-Agent mark. The visible count is the exact live Session count, capped visually at `99+` while accessibility retains the exact value.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Status                | The aggregate priority is needs-attention, failed/canceled/interrupted, succeeded, working, idle, then unknown. Ended Sessions are excluded before aggregation. Freshness is an independent overlay: stale or disconnected state uses degraded warning treatment and includes the last semantic status plus health in text. Attention is orange, failure is red, work is blue, live success is green, idle or unknown is neutral, and degraded health is warning-colored; every status also has a glyph and localized text. Provider marks retain brand color. No status uses continuous animation or color alone.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Disclosure            | Every Workspace Agent disclosure starts collapsed, is process-local, and is never persisted or opened by Workspace activation or Agent arrival. At most one Workspace disclosure is expanded across the sidebar. Collapsing its Project or losing its last represented Session clears it. An expanded disclosure contains every represented Agent as a flat row and uses the sidebar's existing scroll container. Workspace reordering hides the detail projection during the gesture so Agent rows never initiate or distort sorting.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Agent row             | The first line shows one provider identity and localized semantic status. The second line shows localized `Tab title · Panel position · primary/child terminal` context. User-derived text truncates visually while retaining a complete accessible name. A non-navigable matched attachment may remain visible as a disabled row; no row owns persistent selection.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Navigation            | `workbench.agent.navigate` accepts only exact Project, Workspace, and Agent Session identity. At execution it re-resolves that Session against the latest topology in the same Workspace, rejects absence or a non-navigable destination, selects the exact Tab, reveals and focuses the exact Panel and primary or child presentation, removes a conflicting maximization, and requests the host Workspace route. The sidebar invokes this one Action and never writes Tab or Panel Stores directly.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Focus and concurrency | A client-local navigation coordinator waits for both accepted Action settlement and an exact active Project/Workspace/Tab/Panel/presentation projection before issuing one terminal-focus request. Every current projection observation replaces the prior readiness class: an origin-Workspace or target-Workspace-but-not-exact observation can move `ready` back to `pending`, so stale readiness cannot authorize focus. A newer Agent click replaces the older intent. Rejected execution, target disappearance, disposal, or navigation to an unrelated location cancels without fallback or late focus theft. If routing has already committed when the target disappears, the routed Workspace remains in its ordinary state and no other terminal is focused.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Ownership             | `@orchbee/agent` remains authoritative for Session lifecycle, provider, status, attachment, and freshness. Workbench owns the read-only cross-Workspace projection, disclosure interaction, semantic navigation Action, and terminal-focus intent. The application navigation port owns route publication; the terminal-surface adapter owns the final DOM focus effect. No Agent state, disclosure, history, prompt content, timer, background terminal mount, or notification policy is added.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Verification          | Deterministic projection tests cover live-only filtering, attachment deduplication, cross-Workspace fan-out, stable topology ordering, status-stable ordering, status priority, and 512-Session bounds. Action tests cover exact revalidation, primary and child destinations, conflicting maximization, and unavailable targets. Coordinator tests cover result/projection reordering, replacement, disappearance, disposal, and exactly-once focus. Workbench integration tests cover default collapse, flat disclosure, Project-collapse cleanup, exact navigation and focus, and ended-Session cleanup. Provider component tests cover curated single-provider marks, fixed two-provider composition, and provider-neutral composition for three or more providers. Presenter tests cover localized exact counts and the `99+` visual cap in both locales. The dedicated Storybook story and browser inspection cover collapsed and expanded states, one-, two-, three-provider and no-Agent summaries, attention, failure, working and success states, exclusive disclosure, exact navigation, and narrow-sidebar clipping; CSS supplies the reduced-motion fallback. `WorkbenchAgentNavigation` additionally checks ready-to-pending current-projection regression and supplies bounded ordering and latest-intent evidence for Action settlement, the latest normalized route-projection class, disappearance, replacement, disposal, and focus. Structured identity resolution remains deterministic-test-owned, and formal `Focus` is a separately schedulable command-admission abstraction. `AcceptedReadyIntervalEventuallyFocusesOrEnds` applies only while accepted settlement and the ready projection remain current; a later origin or target-pending observation ends that interval and the request continues waiting. The model assumes no fairness for Action settlement or for the projection becoming or remaining ready. |

The v1 scope deliberately omits Project-level rollups, a global Agent page, search or filtering,
priority-based row sorting, topology thumbnails, inline Agent responses or approvals, Agent launch or
termination, durable history, persisted disclosure, automatic expansion, and notification-policy
changes. A collapsed Project can therefore hide its Workspace summaries until the Project is
expanded.

## Dependencies and ownership

| Responsibility                                                                             | Owner                                |
| ------------------------------------------------------------------------------------------ | ------------------------------------ |
| Project, Directory, Workspace, active-location model, invariants, and product presentation | `@orchbee/workbench`                 |
| Tab ordering, grouping, selection, and Tab interaction                                     | `@orchbee/tab-strip`                 |
| Panel layout plus terminal presentation identity, lifecycle, and command projection        | `@orchbee/workbench` Panel workspace |
| Product descriptors and `en-US` / `zh-CN` catalogs                                         | `@orchbee/i18n`                      |
| Browser Preview path-entry presentation and synthetic Folder projection                    | `@orchbee/workbench`                 |
| Existing-Folder native picker, path normalization, and point-in-time validation            | `apps/desktop`                       |
| Typed platform Folder-acquisition capability                                               | `@orchbee/workbench`                 |
| Typed platform Worktree discovery, creation, and adoption capability                       | `@orchbee/workbench`                 |
| Electron Worktree discovery, creation, and adoption data contract                          | `@orchbee/desktop-client-protocol`   |
| Managed Worktree Git observation, creation, compensation, recovery, and fixed path         | `apps/desktop`                       |
| Existing Worktree discovery, revalidation, and canonical registration                      | `apps/desktop`                       |
| SSH Project-create entry, status indicator, and connection action presentation             | `@orchbee/workbench`                 |
| Launch-configuration executable payload                                                    | Future application or runtime owner  |
| Other filesystem mutation, Git effects, PTY, synchronization, and recovery                 | Future application or runtime owner  |
| Locale and platform adapter selection                                                      | Composition application              |

`@orchbee/workbench` exposes one product-capability boundary. The first delivery creates no new
headless package and keeps unavailable platform actions hidden or disabled through their owned
Action availability.

## Correctness and verification

| Claim                                                                                                          | Deterministic evidence                                               | Additional evidence                           | Limits                                                                       |
| -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------- |
| Membership and primary references remain valid                                                                 | Transition examples and invariant assertions                         | Fixed-seed stateful property tests            | Generated finite traces                                                      |
| Directory fallback selects the circular next survivor                                                          | Boundary and removal-set examples                                    | Property oracle over generated orders         | Host path identity is trusted                                                |
| Directory refresh preserves identity, order, and references                                                    | Transition and runtime tests                                         | N/A                                           | Host observations are trusted                                                |
| Library remote registration converges on one Project, Workspace, and empty context without a terminal effect   | Deterministic transition, document, and runtime tests                | No application remote witness                 | Bounded traces; no current remote application provider                       |
| Active location always resolves                                                                                | Transition and deletion tests                                        | Stateful property tests                       | In-memory state only                                                         |
| CWD follows the accepted precedence                                                                            | Decision-table unit tests                                            | N/A                                           | Local path projection only                                                   |
| Semantic reads resolve exact canonical resources without Client View fallback                                  | Ordered read-projection and stale/cross-Project tests                | Shared-document schema validation             | Complete in-memory snapshots; no pagination                                  |
| Exact terminal creation changes one target context and preserves existing selection or its sole valid fallback | Rejection matrix, empty-context, and product Action tests            | Fixed-seed stateful property traces           | Durable commit and revision remain application-owned                         |
| Projected title and CWD follow the declared Warp scope                                                         | Normalization and runtime projection scenarios                       | Web bridge and bootstrap adapter tests        | No in-band-command or SSH-state parity                                       |
| Workspace copies reset runtime state and preserve layout projection                                            | Context-runtime and integration tests                                | Component scenario                            | No process or output parity claim                                            |
| Project disclosure does not switch terminal context                                                            | Reducer and component tests                                          | Keyboard and pointer interaction              | Controlled browser environment                                               |
| Project reorder preserves identity, active location, and disclosure                                            | Transition boundaries and permutation tests                          | Deterministic pointer timeline tests          | Primary mouse and pen pointers                                               |
| Workspace reorder stays inside one Project and preserves identity, active location, and terminal ownership     | Transition boundaries and fixed-seed permutation tests               | Deterministic scoped-pointer timeline tests   | Primary mouse and pen pointers; no cross-Project move                        |
| Drag completion and cancellation restore disclosure and activation intent                                      | Gesture-machine phase tests                                          | Deterministic pointer timeline tests          | Controlled pointer and scroll geometry                                       |
| Product behavior ships in both locales                                                                         | Catalog parity and component tests                                   | Storybook review                              | `en-US` and `zh-CN` only                                                     |
| Browser path entry creates one atomic Project or no state                                                      | Action, component, and web integration tests                         | N/A                                           | Synthetic in-memory preview only                                             |
| Browser path entry appends one unique Folder or leaves state intact                                            | Action, component, and web integration tests                         | N/A                                           | Exact path comparison; no FS call                                            |
| Desktop selection commits only one validated selected result; cancel and failure never mutate                  | Main runtime, broker, client, Hook, and integration tests            | `DesktopLifecycle` Folder scenario            | Injected dialog adapter; real picker is manual evidence                      |
| A retired renderer or stale/duplicate Add target cannot accept a late native completion                        | Broker lifecycle and Workbench state-machine tests                   | `DesktopLifecycle` safety and progress checks | Bounded model and controlled event loop                                      |
| A managed Worktree commit adds its Directory and optional Workspace atomically                                 | Transition and managed-runtime tests                                 | Desktop creation transaction model            | Git and filesystem refinement remain Desktop-owned                           |
| Existing Worktree adoption groups repository inventories and atomically creates one Workspace                  | Transition, runtime, protocol, Hook, and component tests             | Real-repository Git adapter test              | Point-in-time local inventory; no Git mutation                               |
| The initiating client activates only its exact successfully created Workspace after publication                | Activation state-machine, Hook, Action, routing, and component tests | `WorkspaceCreationActivation` model           | One latest pending client intent; canonical commit and transport are trusted |
| Ellipsis and row context gestures expose one action-menu model                                                 | Component parity and invocation tests                                | Pointer interaction review                    | Project and Workspace rows only                                              |

TLA+ is not required for the synchronous in-memory transition, semantic projection and terminal
creation, or browser path-entry set. The legacy remote-admission models are retired; retained
one-way document placement migration is covered by Desktop's registered migration model. The
asynchronous existing-Folder adapter is covered by the `project-folder-acquisition` configuration
of `DesktopLifecycle`, including selection, cancellation, failure, stale targets, renderer
retirement, and late native completion. Managed Worktree Git/product parity and recovery are covered
by Desktop's separately registered `ProjectWorktreeCreation` model. Existing Worktree discovery and
adoption use the bounded transition and adversarial scenarios documented by Desktop; they do not add
a Git mutation or recovery obligation. The client-local success/publication interleaving is covered
by the registered `WorkspaceCreationActivation` model. Real operating-system picker behavior, Git
command refinement, other filesystem mutation, and cross-device synchronization remain outside
those model claims.

## Delivery and recovery

Integrate in dependency order: model and invariant tests, terminal-context lifetime, Actions,
presentation, then the web in-memory preview. Replace the experimental single-context Workbench
API and its only application consumer atomically; do not retain a compatibility path.

The browser delivery performs no disk or process effect. The Desktop existing-Folder slice writes no
filesystem data; only the selected path may enter the existing canonical Project document through
the normal Workbench Action and shared-document commit. Desktop managed-Worktree creation is the
only current Git mutation slice and enters product state through the atomic transition above.
Existing-Worktree discovery is read-only; adoption revalidates the opaque selection and atomically
registers the Directory and Workspace without changing Git. Rollback of any adapter does not
invalidate already committed Project records. New Folder, Worktree deletion or movement, and other
platform effects remain outside current authority until separately accepted, implemented, and
documented by their semantic owners.
