# Project and Workspace Model

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

| Input                                                   | Trust and absence                                                               |
| ------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `ProjectWorkspaceState`                                 | Trusted only after construction or transition invariant checks                  |
| Project, Directory, Workspace, and terminal-context IDs | Stable opaque strings; missing or stale IDs reject                              |
| Directory descriptor                                    | A host-normalized local location with one canonical Folder or Worktree kind     |
| Managed Worktree commit                                 | One host-validated created Worktree and one current source Directory            |
| Existing Worktree adoption                              | One host-revalidated discovered Worktree and one current source Directory       |
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
| Add Directory                | Known Project; location not already present                    | Directory appends to stable order                                     | None                                                                  |
| Register created Worktree    | Known Project and available source; unique valid Worktree      | Directory appends; optional Workspace appends, expands, and activates | Create one context only when the Workspace option is enabled          |
| Adopt existing Worktree      | Known Project and available source; unique discovered Worktree | Directory and Workspace append, expand, and activate                  | Create one context rooted at the adopted Worktree                     |
| Refresh Directory            | Known identity; unchanged kind and source; unique path         | Host observations update in place; order and references remain        | None                                                                  |
| Remove Directories           | At least one Directory survives; only Folder kinds requested   | Referencing primaries use circular next-survivor selection            | None                                                                  |
| Reconcile launch references  | Known Project; unique non-empty reference IDs                  | Catalog replaces; removed Workspace selections become `null`          | None                                                                  |
| Set Project primary          | Available Project Directory                                    | Future Project defaults change                                        | None                                                                  |
| Create Workspace             | Available Project Directory                                    | Workspace appends, Project expands, Workspace becomes active          | Create one context with one Tab, Panel, and shell projection          |
| Edit Workspace               | Known Workspace and available Project Directory                | Name or future default changes                                        | None                                                                  |
| Duplicate Workspace          | Known Workspace                                                | Copy appends and becomes active                                       | Clone layout and names into fresh Tab, Panel, and Terminal identities |
| Delete Workspace             | Known Workspace                                                | Workspace is removed; active deletion falls back to standalone        | Dispose its context                                                   |
| Remove Project               | Known Project                                                  | Project and its Workspace metadata are removed; disk locations remain | Dispose every Project Workspace context                               |

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

Desktop injects a narrow existing-Folder acquisition capability at the same adaptation point. Its
Electron Main owner opens one native single-directory picker, normalizes the selected absolute path,
and checks that the path still names a readable, traversable Directory before returning it. The
check follows a symbolic link while retaining the selected path spelling; it performs no directory
creation, enumeration, Git discovery, or other filesystem mutation. Browser and Gateway clients
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

The New Workspace submenu first presents Project Folder choices. When the host supplies Worktree
discovery, each represented Git repository with existing Worktrees contributes a later group labeled
by its source Project Directory; multiple repositories remain separate groups. Worktrees already in
the Project and newly discovered external Worktrees share their repository group. Reopening a
Project menu refreshes this point-in-time projection. Selecting a current Directory invokes ordinary
Workspace creation; selecting an external Worktree invokes host adoption and creates the Workspace
only after revalidation and canonical commit.

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

| Responsibility                                                                             | Owner                                |
| ------------------------------------------------------------------------------------------ | ------------------------------------ |
| Project, Directory, Workspace, active-location model, invariants, and product presentation | `@orchbee/workbench`                 |
| Tab ordering, grouping, selection, and Tab interaction                                     | `@orchbee/tab-strip`                 |
| Panel layout plus terminal presentation identity, lifecycle, and command projection        | `@orchbee/workbench` Panel workspace |
| Product descriptors and `en-US` / `zh-CN` catalogs                                         | `@orchbee/i18n`                      |
| Browser and Gateway path-entry presentation and synthetic Folder projection                | `@orchbee/workbench`                 |
| Existing-Folder native picker, path normalization, and point-in-time validation            | `apps/desktop`                       |
| Typed platform Folder-acquisition capability                                               | `@orchbee/workbench`                 |
| Typed platform Worktree discovery, creation, and adoption capability                       | `@orchbee/workbench`                 |
| Electron Worktree discovery, creation, and adoption data contract                          | `@orchbee/desktop-client-protocol`   |
| Managed Worktree Git observation, creation, compensation, recovery, and fixed path         | `apps/desktop`                       |
| Existing Worktree discovery, revalidation, and canonical registration                      | `apps/desktop`                       |
| Launch-configuration executable payload                                                    | Future application or runtime owner  |
| Other filesystem mutation, Git effects, PTY, synchronization, and recovery                 | Future application or runtime owner  |
| Locale and platform adapter selection                                                      | Composition application              |

`@orchbee/workbench` exposes one product-capability boundary. The first delivery creates no new
headless package and keeps unavailable platform actions hidden or disabled through their owned
Action availability.

## Correctness and verification

| Claim                                                                                                      | Deterministic evidence                                    | Additional evidence                           | Limits                                                  |
| ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------- |
| Membership and primary references remain valid                                                             | Transition examples and invariant assertions              | Fixed-seed stateful property tests            | Generated finite traces                                 |
| Directory fallback selects the circular next survivor                                                      | Boundary and removal-set examples                         | Property oracle over generated orders         | Host path identity is trusted                           |
| Directory refresh preserves identity, order, and references                                                | Transition and runtime tests                              | N/A                                           | Host observations are trusted                           |
| Active location always resolves                                                                            | Transition and deletion tests                             | Stateful property tests                       | In-memory state only                                    |
| CWD follows the accepted precedence                                                                        | Decision-table unit tests                                 | N/A                                           | Local path projection only                              |
| Projected title and CWD follow the declared Warp scope                                                     | Normalization and runtime projection scenarios            | Web bridge and bootstrap adapter tests        | No in-band-command or SSH-state parity                  |
| Workspace copies reset runtime state and preserve layout projection                                        | Context-runtime and integration tests                     | Component scenario                            | No process or output parity claim                       |
| Project disclosure does not switch terminal context                                                        | Reducer and component tests                               | Keyboard and pointer interaction              | Controlled browser environment                          |
| Project reorder preserves identity, active location, and disclosure                                        | Transition boundaries and permutation tests               | Deterministic pointer timeline tests          | Primary mouse and pen pointers                          |
| Workspace reorder stays inside one Project and preserves identity, active location, and terminal ownership | Transition boundaries and fixed-seed permutation tests    | Deterministic scoped-pointer timeline tests   | Primary mouse and pen pointers; no cross-Project move   |
| Drag completion and cancellation restore disclosure and activation intent                                  | Gesture-machine phase tests                               | Deterministic pointer timeline tests          | Controlled pointer and scroll geometry                  |
| Product behavior ships in both locales                                                                     | Catalog parity and component tests                        | Storybook review                              | `en-US` and `zh-CN` only                                |
| Browser path entry creates one atomic Project or no state                                                  | Action, component, and web integration tests              | N/A                                           | Synthetic in-memory preview only                        |
| Browser path entry appends one unique Folder or leaves state intact                                        | Action, component, and web integration tests              | N/A                                           | Exact path comparison; no FS call                       |
| Desktop selection commits only one validated selected result; cancel and failure never mutate              | Main runtime, broker, client, Hook, and integration tests | `DesktopLifecycle` Folder scenario            | Injected dialog adapter; real picker is manual evidence |
| A retired renderer or stale/duplicate Add target cannot accept a late native completion                    | Broker lifecycle and Workbench state-machine tests        | `DesktopLifecycle` safety and progress checks | Bounded model and controlled event loop                 |
| A managed Worktree commit adds its Directory and optional Workspace atomically                             | Transition and managed-runtime tests                      | Desktop creation transaction model            | Git and filesystem refinement remain Desktop-owned      |
| Existing Worktree adoption groups repository inventories and atomically creates one Workspace              | Transition, runtime, protocol, Hook, and component tests  | Real-repository Git adapter test              | Point-in-time local inventory; no Git mutation          |
| Ellipsis and row context gestures expose one action-menu model                                             | Component parity and invocation tests                     | Pointer interaction review                    | Project and Workspace rows only                         |

TLA+ is not required for the synchronous in-memory transition and browser path-entry set. The
asynchronous existing-Folder adapter is covered by the `project-folder-acquisition` configuration
of `DesktopLifecycle`, including selection, cancellation, failure, stale targets, renderer
retirement, and late native completion. Managed Worktree Git/product parity and recovery are covered
by Desktop's separately registered `ProjectWorktreeCreation` model. Existing Worktree discovery and
adoption use the bounded transition and adversarial scenarios documented by Desktop; they do not add
a Git mutation or recovery obligation. Real operating-system picker behavior, Git command refinement,
other filesystem mutation, and cross-device synchronization remain outside those model claims.

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
