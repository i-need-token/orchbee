# Project and Workspace Model

## Status and scope

This is the implementation contract for Workbench-owned Project, Directory, Workspace,
active-location, terminal-context composition, and browser Project onboarding. Workbench maintains
the live product model and exposes its canonical topology through
[`durable-topology.md`](durable-topology.md). Filesystem mutation, Git effects, PTY ownership,
storage I/O, and cross-device synchronization remain outside this contract.

## Problem

A person must be able to keep standalone terminal work available while organizing other terminal
contexts by Project and Workspace. The current flat Workbench projection cannot preserve several
independent Tab and Panel contexts or determine their default CWD from a Project Directory.

## Inputs and validation

| Input                                                   | Trust and absence                                                               |
| ------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `ProjectWorkspaceState`                                 | Trusted only after construction or transition invariant checks                  |
| Project, Directory, Workspace, and terminal-context IDs | Stable opaque strings; missing or stale IDs reject                              |
| Directory descriptor                                    | A host-normalized local location with one canonical Folder or Worktree kind     |
| Browser-entered Folder path                             | Trimmed, non-empty user input; otherwise preserved and not filesystem-verified  |
| Display name                                            | User or localized product input; duplicate names are allowed                    |
| Launch-configuration reference                          | Stable Project-local ID and display name; executable payload remains host-owned |
| Explicit terminal Directory                             | Must be an available member of the target Project                               |
| Split CWD                                               | A host-reported local CWD; absence continues through the fallback chain         |
| System CWD                                              | Required final fallback supplied by the composition host                        |

The first delivery receives normalized Directory descriptors and performs no filesystem lookup.
The browser preview additionally accepts a manually entered path and constructs a synthetic Folder
Directory for its simulated terminal projection. Its `available: true` means the current preview
can use the path as launch CWD; it is not evidence that the path exists on disk. Later platform
adapters must validate external filesystem and Git data before constructing authoritative local
descriptors.

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
- Every Directory's discriminated kind agrees with its Git metadata.
- Every Project `primaryDirectoryId` names one of its Directories.
- Launch-configuration IDs are unique within their Project.
- Every Workspace belongs to exactly one Project and its `primaryDirectoryId` names one Directory
  in that Project.
- Every non-null Workspace `launchConfigurationId` names one Project launch configuration.
- Each Workspace has one terminal-context ID, and no two Workspaces or the standalone terminal
  share a terminal context.
- `activeLocation` is standalone or names one current Workspace in one current Project.
- Project expansion changes only disclosure state and never changes `activeLocation`.
- Display names need not be unique and never serve as identity.
- A transition that rejects or reports unchanged preserves the complete input state.

Removing Directories preserves referential integrity. For every removed primary Directory, choose
the first surviving Directory after it in the Project's previous stable order, wrapping to the
first surviving Directory when necessary. Removing the final Directory rejects.

## Outputs

`transitionProjectWorkspace` returns the complete next state, an `applied`, `unchanged`, or
`rejected` outcome, a named rejection when applicable, and terminal-context commands. Commands
create, clone, or dispose in-memory terminal contexts; the pure transition performs no React, DOM,
storage, process, filesystem, Git, time, random, or network effect.

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
Unavailable default Directories are skipped. The host may report a terminal's current CWD without
changing its launch CWD. Panel split and child-terminal creation inherit current CWD; Workspace
duplication starts fresh shells from recorded launch CWD. Changing a Project or Workspace primary
Directory never changes an existing terminal CWD.

## Decisions and transitions

| Intent                       | Guard                                                        | Result                                                                | Terminal-context command                                              |
| ---------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Activate standalone terminal | None                                                         | Standalone becomes active                                             | None                                                                  |
| Toggle Project expansion     | Known Project                                                | Only disclosure changes                                               | None                                                                  |
| Reorder Project              | Known Project and optional known following Project           | Project moves before that Project, or to the end when absent          | None                                                                  |
| Reorder Workspace            | Known Project, source Workspace, and optional sibling target | Workspace moves within that Project, or to the end when absent        | None                                                                  |
| Activate Workspace           | Known Project and Workspace membership                       | Workspace becomes active and Project expands                          | None                                                                  |
| Create Project               | One normalized available Directory                           | Project, Primary Workspace, and active location are created           | Create one context with one Tab, Panel, and shell projection          |
| Add Directory                | Known Project; location not already present                  | Directory appends to stable order                                     | None                                                                  |
| Refresh Directory            | Known identity; unchanged kind and source; unique path       | Host observations update in place; order and references remain        | None                                                                  |
| Remove Directories           | At least one Directory survives; only Folder kinds requested | Referencing primaries use circular next-survivor selection            | None                                                                  |
| Reconcile launch references  | Known Project; unique non-empty reference IDs                | Catalog replaces; removed Workspace selections become `null`          | None                                                                  |
| Set Project primary          | Available Project Directory                                  | Future Project defaults change                                        | None                                                                  |
| Create Workspace             | Available Project Directory                                  | Workspace appends, Project expands, Workspace becomes active          | Create one context with one Tab, Panel, and shell projection          |
| Edit Workspace               | Known Workspace and available Project Directory              | Name or future default changes                                        | None                                                                  |
| Duplicate Workspace          | Known Workspace                                              | Copy appends and becomes active                                       | Clone layout and names into fresh Tab, Panel, and Terminal identities |
| Delete Workspace             | Known Workspace                                              | Workspace is removed; active deletion falls back to standalone        | Dispose its context                                                   |
| Remove Project               | Known Project                                                | Project and its Workspace metadata are removed; disk locations remain | Dispose every Project Workspace context                               |

The runtime's optional transition commit callback runs only for an `applied` result, immediately
before any terminal-context command or authoritative state replacement. Action adapters use it to
record their commit point. After replacement, the runtime synchronously publishes the new snapshot;
an observer failure is isolated and cannot relabel the transition or prevent later observers from
receiving it.

Delete Workspace and Remove Project remain Action-owned destructive transitions. When invoked
from a dropdown or context menu, their confirmation is a compact Popover anchored to the pointer or
selected menu item; it introduces no page-wide backdrop and supports `Escape` to cancel and `Enter`
to confirm. Invocations without a contextual anchor fall back to the shared modal Alert Dialog.

Preview Workspace duplication copies the primary Directory, launch-configuration selection, Tab groups,
Panel layout, presentation names, and each terminal's recorded launch CWD. It uses a whitelist:
Tab and Group attention, loading, network, crash, permission, discard, capture, memory, and other
runtime observations do not copy; presentations reset to preview lifecycle with fresh identities.
It never copies
process identity, output, command history, Worktrees, or source files. The in-memory preview creates
ordinary shell projections with fresh identities. Managed composition rejects terminal-context
cloning; Desktop does not expose Project or Workspace duplication in this integration stage.

## Browser Project folder entry

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

A future desktop composition replaces only the path-collection step with a native Folder picker and
passes its host-normalized Directory observation into the same Project-create or
Project-directory-add intent. Native picker cancellation, permission, validation, Git discovery,
and failure reporting remain reserved for that platform slice; the browser does not simulate them.

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

## Dependencies and ownership

| Responsibility                                                                             | Owner                                 |
| ------------------------------------------------------------------------------------------ | ------------------------------------- |
| Project, Directory, Workspace, active-location model, invariants, and product presentation | `@orchbee/workbench`                  |
| Tab ordering, grouping, selection, and Tab interaction                                     | `@orchbee/tab-strip`                  |
| Panel layout plus terminal presentation identity, lifecycle, and command projection        | `@orchbee/workbench` Panel workspace  |
| Product descriptors and `en-US` / `zh-CN` catalogs                                         | `@orchbee/i18n`                       |
| Browser path-entry presentation and synthetic Folder projection                            | `@orchbee/workbench`                  |
| Normalized filesystem and Git observations                                                 | Future application or runtime adapter |
| Launch-configuration executable payload                                                    | Future application or runtime owner   |
| Filesystem, Git, PTY, durable storage, synchronization, and recovery effects               | Future application or runtime owner   |
| Locale and platform adapter selection                                                      | Composition application               |

`@orchbee/workbench` exposes one product-capability boundary. The first delivery creates no new
headless package and keeps unavailable platform actions hidden or disabled through their owned
Action availability.

## Correctness and verification

| Claim                                                                                                      | Deterministic evidence                                 | Additional evidence                         | Limits                                                |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------- | ----------------------------------------------------- |
| Membership and primary references remain valid                                                             | Transition examples and invariant assertions           | Fixed-seed stateful property tests          | Generated finite traces                               |
| Directory fallback selects the circular next survivor                                                      | Boundary and removal-set examples                      | Property oracle over generated orders       | Host path identity is trusted                         |
| Directory refresh preserves identity, order, and references                                                | Transition and runtime tests                           | N/A                                         | Host observations are trusted                         |
| Active location always resolves                                                                            | Transition and deletion tests                          | Stateful property tests                     | In-memory state only                                  |
| CWD follows the accepted precedence                                                                        | Decision-table unit tests                              | N/A                                         | Local path projection only                            |
| Workspace copies reset runtime state and preserve layout projection                                        | Context-runtime and integration tests                  | Component scenario                          | No process or output parity claim                     |
| Project disclosure does not switch terminal context                                                        | Reducer and component tests                            | Keyboard and pointer interaction            | Controlled browser environment                        |
| Project reorder preserves identity, active location, and disclosure                                        | Transition boundaries and permutation tests            | Deterministic pointer timeline tests        | Primary mouse and pen pointers                        |
| Workspace reorder stays inside one Project and preserves identity, active location, and terminal ownership | Transition boundaries and fixed-seed permutation tests | Deterministic scoped-pointer timeline tests | Primary mouse and pen pointers; no cross-Project move |
| Drag completion and cancellation restore disclosure and activation intent                                  | Gesture-machine phase tests                            | Deterministic pointer timeline tests        | Controlled pointer and scroll geometry                |
| Product behavior ships in both locales                                                                     | Catalog parity and component tests                     | Storybook review                            | `en-US` and `zh-CN` only                              |
| Browser path entry creates one atomic Project or no state                                                  | Action, component, and web integration tests           | N/A                                         | Synthetic in-memory preview only                      |
| Browser path entry appends one unique Folder or leaves state intact                                        | Action, component, and web integration tests           | N/A                                         | Exact path comparison; no FS call                     |
| Ellipsis and row context gestures expose one action-menu model                                             | Component parity and invocation tests                  | Pointer interaction review                  | Project and Workspace rows only                       |

TLA+ is not required for the synchronous in-memory transition and browser path-entry set. A later
adapter that adds asynchronous filesystem, Git, persistence, cancellation, retry, or reordered
completion must add the applicable workflow model before production integration. Real filesystem
safety, Git recovery, PTY lifecycle, durable restoration, and cross-device synchronization are
explicit exclusions.

## Delivery and recovery

Integrate in dependency order: model and invariant tests, terminal-context lifetime, Actions,
presentation, then the web in-memory preview. Replace the experimental single-context Workbench
API and its only application consumer atomically; do not retain a compatibility path.

The first delivery writes no authoritative Project data and performs no disk or process effect.
Rollback removes the new model and preview composition and restores the prior flat in-memory
Workbench inputs. The product proposal remains until later platform effects are accepted,
implemented, and their stable facts converge into their owning authorities.
