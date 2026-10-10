# Native Worktree launch

## Interaction and scope

On Windows, New Worktree and `Ctrl+B W` create a checkout with a generated branch and default
base after read-only source discovery. No Project chooser or branch/name form blocks this intent.
Use the CLI for an explicit branch, base or destination. `Ctrl+B O` opens an exact checkout
selected from the repository listing. F1 and `[keys].new_worktree` / `open_worktree` use the
existing binding registry.

Default creation gives the Workspace a short localized name (Worktree 1, Worktree 2, and so on),
choosing the first unused name across the current snapshot. The generated Git branch retains its
unique operation identity. Renaming the Workspace later does not rename the branch or move files.

The directory follows the [CLI Worktree policy](windows-worktrees.md). A worktree-backed Workspace
is a peer of ordinary Workspaces. Creating from an ungrouped source Workspace atomically creates
a Project containing source and new Workspace; a grouped source uses its current Project at
the authoritative commit. An existing linked source creates another peer. Opening a checkout
already associated with one Workspace reuses it instead of duplicating it. Ambiguous associations
remain explicit failures.

The Open list shows main and linked checkouts and resolves exact canonical paths, never duplicate
branch or Workspace labels. Keyboard arrows, Enter and mouse selection operate the same choices;
Esc cancels before submission. Bounded scrolling exposes long lists. Bare entries cannot be opened.
A surface that cannot fit releases capture with a localized notice.

After submission, Esc hides observation without cancelling Server work. W or O reopens the retained
operation; `r` only reads that operation. The operation ID and known checkout/Workspace facts survive
failed reads and entry failure. Unknown Start is never replayed. Resume, deletion, trust override
and rollback are never implicit. A terminal result can be dismissed before a new explicit operation.
[Workspace Close](native-workspace-close.md) and checkout deletion are separate intents.

## Model and ownership

| Record       | Contract                                                                                                                                |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs       | Exact origin context/member, primary directory or primary root cwd, canonical snapshot, bounded Git listing, geometry and configuration |
| Data         | Opening token, immutable Start plan, retained operation ID and known checkout/Workspace result; local focus permission is separate      |
| Outputs      | Read-only preparation, one Worktree Start, bounded reads and optional exact terminal entry                                              |
| Shared owner | Server coordinates Git facts, current Project membership, Workspace launch and retained operation status                                |
| Client owner | The initiating Client owns capture, observation and revocable entry permission                                                          |

The app-private lane uses its own authenticated Workbench Client and a wakeable bounded worker.
It never holds the UI lock during IPC. Shutdown stops admission, wakes I/O and joins its thread.
Slow Git work remains Server-owned and does not occupy the ordinary terminal Viewer FIFO.
Only exact root-cwd preparation uses that FIFO and its existing deadline and cleanup rules.
A borrowed primary Viewer is never detached; a temporary Viewer is released before preparation settles.

Configured destinations use one pure policy shared with CLI; unconfigured destinations remain
Worker-owned. The generated branch is also used when deriving a configured default destination.
The TUI reads admitted settings at preparation. Create validates the Worker-observed canonical
receipt rather than comparing its literal operand spelling. Open matches the exact path selected
from the listing.

Project labels and grouping are metadata, not physical repository identity. Source revalidation
uses stable Workspace/context/directory/Worker identity and the exact terminal binding.
A regroup during Git preparation does not invalidate that source. At attachment, Server resolves
the source's current group under the Site gate and commits a new group, source membership,
new Workspace and first terminal structure together when needed. Closing the source refuses
attachment without resurrecting it; already known Git effects remain in the partial result.
There is no preliminary Client-owned Default Project creation or empty Project left by that step.

## Entry and failure

| Transition                           | Guard and result                                                                                |
| ------------------------------------ | ----------------------------------------------------------------------------------------------- |
| Hidden → prepare                     | Capture current semantic origin; read source and listing without mutation                       |
| Prepared Create → Start              | Still-current visible intent authorizes one plan using defaults; no second confirmation         |
| Prepared Open → list                 | Exact source, Site, non-stale revision and stable checkout paths                                |
| Choose Open                          | A complete current draw token and decoded packet authorize that exact choice                    |
| Cancel preparation or list           | Drop unsubmitted intent; late discovery cannot start an operation                               |
| Start → observe                      | Retain one operation identity before fallible result consumption; uncertainty permits only Read |
| Pending → Read                       | One bounded observation request; ordinary terminal output continues                             |
| Opened → navigate                    | Prove existing reuse or first-terminal start, exact fresh membership and unchanged origin       |
| Navigate → Ready                     | Existing cleanup, attach, control, geometry and first-frame sequence                            |
| Newer input, hide or source change   | Revoke entry permission permanently for this operation; retain facts                            |
| Failed/unknown terminal or lost read | Preserve checkout/Workspace evidence; never recreate or compensate                              |

Entry waits for an in-flight publication refresh and then rechecks the origin. Missing membership
allows at most one extra read-only refresh within the original entry deadline. Hiding an
unpresentable surface cancels entry and preserves the receipt. Directory comparison uses the
Server association policy without inventing filesystem identity or dropping the Worker guard.

The receipt snapshot fixes the target's first Tab, Panel, primary Presentation, terminal binding
and directory ID. Every accepted source refresh must still prove them. The context-selection
snapshot is checked before old Viewer cleanup or installing selection; legal Tab reordering
resolves the same member. A peer deletion or replacement refuses entry and preserves the current
terminal. Exact primary presentation takes precedence over remembered child terminals.
The guard applies only to its admitted refresh generation and never changes unrelated navigation.

Canonical commit and Client entry are separate outcomes. A post-commit read, attachment or draw
failure does not undo Git, remove a Workspace or resend Start. Other Clients keep their focus.
Use `orchbee worktree status <id> --format json` while Server retains the operation; Server restart
has the [existing retention limit](windows-worktrees.md#retained-continuation).

## Correctness and verification

Maintain witnesses for default one-step creation, checkout selection, current-group attachment,
closed/closing source refusal, cancellation, exact draw authority, ambiguous handoff, known partial
facts, Start/Read separation, target replacement and bounded Viewer release.

Owning suites include `state/worktree_launch/tests.rs`,
`connection/native/worktree_launch/tests.rs`, `ui/worktree_launch_tests.rs` and adjacent
state, transport, input and rendering suites. Run
`cargo test --locked -p orchbee --all-targets --all-features` and the applicable root gate.
This workflow has no separate TLA model. Named implementation traces do not establish exhaustive
interleavings or physical terminal usability; real mouse/keyboard experience is separate evidence.
Shared ownership and commit protocols retain their registered models.
