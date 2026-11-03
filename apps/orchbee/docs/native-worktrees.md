# Native Worktree launch

## Interaction and scope

On Windows, New Worktree and `Ctrl+B W` create a checkout with a generated branch and default
base after read-only source discovery. No Project chooser or branch/name form blocks this intent.
Use the CLI for an explicit branch, base or destination. `Ctrl+B O` opens an exact checkout
selected from the repository listing. F1 and `[keys].new_worktree` / `open_worktree` use the
existing binding registry. Both start only from a settled Workspace view. Without one, such as
in a Workspace whose terminals ended while its close waits on a failed removal, they start
nothing and the status line says the Workspace is not ready; while another local operation
owns the structure, it says the Client is busy. A retained result still reopens first.

Default creation gives the Workspace a short localized name (Worktree 1, Worktree 2, and so on),
choosing the first unused name across the current snapshot. The generated Git branch retains its
unique operation identity. Renaming the Workspace later does not rename the branch or move files.

The directory follows the [CLI Worktree policy](windows-worktrees.md). The native Client takes
`worktrees.directory` from the Session's applied configuration snapshot and never reads
`config.toml`; before the first snapshot is applied, Worktree launch reports configuration as
unavailable. A worktree-backed Workspace
is a peer of ordinary Workspaces. Creating from an ungrouped source Workspace atomically creates
a Project containing source and new Workspace; a grouped source uses its current Project at
the authoritative commit. An existing linked source creates another peer. Opening a checkout reuses
the one Workspace standing there, the one whose location line shows it, instead of duplicating it. A
Workspace stands where the Server last followed its first terminal, or in its primary directory while
nothing was followed away from it, compared by exact checkout path: a Workspace created in the
checkout that moved elsewhere is not picked, one that moved in is, and one in a subdirectory of the
checkout is not. With nothing standing there, Open starts a new Workspace. A reused ungrouped
Workspace lands where a new one would: the requested Project, the source's Project, or a new Project
the two found together; a Directory source without a Project leaves it ungrouped. A reused Workspace
that already has a Project stays in it. Several Workspaces standing at one checkout remain an explicit
failure.

The Open list shows main and linked checkouts and resolves exact canonical paths, never duplicate
branch or Workspace labels. Keyboard arrows, Enter and mouse selection operate the same choices;
Esc cancels before submission. Bounded scrolling exposes long lists. Bare entries cannot be opened.
Selecting an exact checkout in a sidebar submenu first opens the existing source form with that
checkout selected. Its first complete draw authorizes that one Open without a second Enter; a
blocked or cancelled presentation cannot submit it. The global Open picker still waits for an
explicit choice. Both routes revalidate the addressed source before Start and never replay it.
If the first complete shell cannot fit, that opening attempt is refused with a localized notice and
retains no hidden owner. A shell that has already completed one draw instead suspends in place,
preserving its operation, token, stage, fields, caret and checkout selection. Hidden edit, selection,
wheel and submit input is consumed without mutation; `Esc` remains the local recovery route. A
later complete draw resumes the same surface incarnation. Mere absence from a covered frame only
revokes its current draw proof. If the accepted operation completes while that proof is absent, its
result is retained without automatic terminal entry; uncovering the surface does not replay entry.

After submission, `[x]`, Esc and Close close the dialog without cancelling Server work, and the
result returns as a status notice. W or O reopens the retained operation. The operation ID and known checkout/Workspace facts survive
failed reads and entry failure. Unknown Start is never replayed. Resume, deletion, trust override
and rollback are never implicit. After successful entry completes, a hidden, correlated Opened result
with retained focus eligibility may yield to a new explicit operation. Its success notice is
transient: once the notice lifetime ends, that hidden result returns the launcher to Idle, which is
the state such a replacement would have produced. Both ordinary and addressed routes require no
remaining entry owner. Addressed replacement validates the current exact source,
mode, listed non-bare checkout and next token before discarding the old receipt; refusal preserves
that receipt and its reopen/read recovery. A new draft starts without old draw/input authority and
requires its own complete draw before Start. Visible, pending, uncertain, failed or focus-retired
results retain their recovery owner instead of silently becoming Idle.
[Workspace Close](native-workspace-close.md) and checkout deletion are separate intents.

Geometry suspension or a refused reopen revokes automatic terminal-entry focus but never cancels an
accepted Start or observation. Its completion remains bound to the original operation ID and may
update retained facts in the background; it cannot reopen the shell, submit another Start or force
focus. Reopening after a refused attempt creates a new presentation attempt for that retained
operation, and a successful draw restores its newest operation status.

`[x]`, Esc and Cancel share the current stage's cancellation. The New Worktree and Open Worktree
dialogs title their fields Branch and Base; an empty Base shows its HEAD placeholder. An invalid
field disables Create, shows its error as soon as it is edited, and Enter moves to the first invalid
field. Editable fields and their Open/Create button share the keyboard owner. Checkout rows act on matching plain-left release; list-only wheel moves three choices per notch, capped by visible two-line choices per pass without wrapping. Selection changes revoke submission until a fresh draw. A running operation keeps its primary in place, disabled, reads its own status, and offers only Close; a settled result offers Close plus explicit recovery for eligible Remove failures. These never replay Start or cancel accepted Server work.

## Safe removal

A Workspace's row menu appends **Remove Worktree…** after Close Workspace… when the
read-only listing proves that Workspace's exact primary directory is an associated linked,
non-bare checkout on the same Worker. Project and creation menus do not infer a removal target.
That directory proof compares spellings without touching the filesystem under
`same_directory_spelling`, the rule every Server checkout association also uses: POSIX-rooted paths
match byte for byte, because a Linux or macOS volume may be case sensitive; on Windows drive paths
only separators, trailing separators and ASCII case (including the drive letter) may differ. A
`\\?\` verbatim prefix, dot component, doubled separator, non-ASCII case difference or parent/child
path never matches, so an uncertain spelling hides the action rather than addressing another
directory.
The destructive action has no default shortcut and never falls back to the selected Client's cwd.
The confirmation names the Workspace, full checkout path, affected terminal count and retained
branch. It reuses the existing Worktree modal, action controls, draw authority and capture owner.
It opens with focus on Remove. Unmodified Enter or the Remove button submits once; Esc, `[x]` or
Cancel before submission changes nothing. If the complete confirmation cannot fit, no removal is submitted.

Windows keeps a process's working directory in use, and Git deletes the checkout only after the
Server has closed its Workspace. On Windows, confirming therefore first checks whether the TUI
itself stands in the checkout or below it, under the same spelling rule. If it does, nothing is
submitted, and the surface names the cause and the remedy: quit Orchbee, leave the folder in the
shell that started it, then remove the Worktree from elsewhere. Server, Worker and Host start in a
neutral directory rather than the activating Client's, so Orchbee's background processes never hold
a checkout. A POSIX directory can be removed while a process stands in it, so the check is
Windows-only.

Remove submits `projectId: null`, `force: false` and `trustRepository: false` through the existing
Server operation. Addressed Workspace, directory, terminal scope, Worker and checkout facts are
revalidated before Start. The Server retains dirty/untracked-file, main/bare-checkout and other
Workspace-association guards, terminal disposal, publication hold, Git deletion and shell recovery.
Successful removal closes the Workspace and preserves the branch. Ordinary Workspace Close keeps
its existing independent meaning and never deletes a checkout. This native Windows TUI addition
uses the same domain operation as the CLI; Desktop removal remains outside this delivery scope.

The surface shows readable failure reasons and retained Workspace/shell-restoration facts.
Each reason names its cause, such as a checkout Git no longer lists, a directory still in use or a
failed Git step, instead of presenting every Git-stage failure as a refusal. A nonresumable failure
whose Workspace Close is still held names `orchbee workspace close <workspace-id> --confirm`, which
releases that hold without Git.
For a known resumable failure, **Resume removal…** opens a new confirmation for the original
target; cancelling it restores the failure receipt. Confirming sends one Resume with the original
operation ID and exact failed attempt, followed only by reads. No ID transcription is needed.
A new draw is required before that confirmation can act. Attempt exhaustion, unknown results and
nonresumable failures never offer Resume. A lost Resume handoff retains the receipt and fences old
attempts; it never sends another Start or automatically retries Resume.

Pending progress publishes no canonical change, so within the 120-second observation window of a
Start, Resume or read, each Pending reply schedules another read after 250 ms, doubling to at most
2 s; a change notification can read sooner.
Closing running observation or a resumable/unknown result never cancels it; W, O or another
addressed removal entry reopens that retained task. **Stop Tracking** (`x`) on such a result opens
its own confirmation, which also needs a new draw. Confirming forgets only this Client's receipt: it never
cancels or rolls back Server work, and `orchbee worktree status` still reads the operation. Once the
target Workspace is absent from the canonical snapshot, Close, W, O and addressed entries discard a
settled removal result instead of reopening it.
Completion and shell recovery request the existing canonical publication refresh. They never
reopen the surface or select another Workspace. After the removed current Workspace disappears,
the ordinary membership owner chooses the surviving navigation context. Closing a completed result
releases its local receipt; the Server's existing retention limit is unchanged.

## Model and ownership

| Record       | Contract                                                                                                                                                  |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs       | Exact origin context/member, primary directory or primary root cwd, canonical snapshot, bounded Git listing, geometry and configuration                   |
| Data         | Opening token, immutable target/Start plan, retained operation ID, fenced attempt and known checkout/Workspace result; local focus permission is separate |
| Outputs      | Read-only preparation, one Worktree Start, explicitly confirmed Resume, bounded reads and optional exact terminal entry                                   |
| Shared owner | Server coordinates Git facts, current Project membership, Workspace launch/removal and retained operation status                                          |
| Client owner | The initiating Client owns capture, observation and revocable entry permission                                                                            |

The app-private lane uses its own authenticated Workbench Client and a wakeable bounded worker.
It never holds the UI lock during IPC. Shutdown stops admission, wakes I/O and joins its thread.
Slow Git work remains Server-owned and does not occupy the ordinary terminal Viewer FIFO.
Only exact root-cwd preparation uses that FIFO and its existing deadline and cleanup rules.
A borrowed primary Viewer is never detached; a temporary Viewer is released before preparation settles.

Configured destinations use one pure policy shared with CLI; unconfigured destinations remain
Worker-owned. The generated branch is also used when deriving a configured default destination.
The TUI reads admitted settings at preparation. Create validates the Worker-observed canonical
receipt rather than comparing its literal operand spelling. Open and Remove receipts name the
checkout in Git's spelling, which may differ from a Workspace directory taken from a launch
directory, so they match the path selected from the listing or the confirmed Workspace directory
under `same_directory_spelling`.

Project labels and grouping are metadata, not physical repository identity. Source revalidation
uses stable Workspace/context/directory/Worker identity and the exact terminal binding.
A regroup during Git preparation does not invalidate that source. At attachment, Server resolves
the source's current group under the Session gate and commits a new group, source membership,
new Workspace and first terminal structure together when needed. Closing the source refuses
attachment without resurrecting it; already known Git effects remain in the partial result.
There is no preliminary Client-owned Default Project creation or empty Project left by that step.

Sidebar Worktree choices derive an addressed source through `AddressedWorktreeSource`. A Workspace
uses its first canonical Tab's first leaf primary/root process, independent of focused Tabs, leaves
or child Agents. A Project uses the first Workspace in its canonical display order, then that same
first-terminal rule. Empty Projects have no source. Project has no canonical directory and may
contain unrelated repositories; a non-Git or unavailable first source never triggers a search of
later members or a borrow from the current Client context.

An addressed Workspace stands where its location line says: the first terminal's directory the
Server follows leads, because a shell that reports its directory, such as PowerShell, leaves its
process cwd in the launch directory. An admitted root cwd sample comes next, then the canonical
primary directory. A failed sample or an unbound first terminal costs nothing while one of these is
known, and an empty Workspace has no sample at all. A directory that names the canonical primary
directory on the same Worker (compared by `same_directory_spelling`) is the `workspace` source; any other is `workspace-directory`, which runs Git there while
the Workspace stays the grouping owner. Both keep required-nullable `projectId: null`, and the
existing Server Alongside owner resolves that Workspace's grouping, so an ungrouped Workspace and the
Worktree it creates or opens land in one new Project, wherever its first terminal stands. A
`workspace-directory` source says nothing about the Workspace's own checkout: Remove still addresses
the canonical directory, and no association is retained for the directory it stood in. The menu offers
Remove only while the Workspace stands in its own checkout, so the row never names a checkout the user
no longer sees it in. A Client must not copy the Project onto either Start, which the strict wire
contract rejects.

A Project source keeps the Directory form and may carry the frozen existing Project in Start. Its
followed directory leads an admitted sample, and it still needs that sample: an admitted explicit
absent root cwd permits only the member's canonical primary directory, failed, mismatched or
unavailable samples never mean absence, and an existing unbound first terminal is unavailable.

These pre-submit choices freeze exact Project identity and ordered members, addressed Workspace,
Context, first primary binding and directory proof. Membership, order, removal, rebind or source
drift retires them; unrelated canonical writes do not become source drift. A followed directory
change, and the automatic label it moves, keeps an open menu and starts its discovery over: the rows
keep a success only while its checkout contains the new directory, and otherwise stay disabled until
Git answers. A row chosen meanwhile waits for the new answer and runs only against it. A later menu
starts from the last success for the same source under the same rule, or omits the Worktree rows
after Git refused that exact directory, and a sidebar row that already
shows a branch for the directory it stands in opens its menu with New Worktree… enabled, under the
same wait for the first answer. A launch form already open reports the
source change at Start. Sidebar preparation uses
the existing background Worktree lane's own authenticated Session and a temporary read-only Viewer,
released after root samples before and after Git. It never borrows the interactive Viewer or its
controller. Every failure path detaches or retires that auxiliary Session within the original bounded
budget and cleanup reserve. Ordinary shortcut preparation retains its existing borrowed/temporary
Viewer policy. Discovery and creation must
revalidate the addressed proof, and the chosen source/directory appears in the existing launch form
before effect. The pure source model tests cover empty/noncurrent/multiple-repository grouping,
first terminal versus child/later terminal, explicit absence versus unavailable samples and source
drift. Git classification, actual cwd sampling and real menu interaction require their separate
integration evidence. `cli_windows_worktree_menu` runs the CLI, Server, Worker and Zig Host under
test against a real repository with a TUI Client driven by pointer: from an ungrouped Workspace's
row menu, Open Worktree ▸ opens a linked checkout into a new Project with the Workspace, and New
Worktree… then joins that Project. It also covers a first terminal in a subdirectory, a directory the
shell only reports, a directory change under the open menu with its rows staying enabled, a checkout another Workspace holds, a
Workspace that walked into the checkout beside one that walked out of it, a folder outside Git whose
next menu omits the Worktree rows, and a checkout every Workspace left. A direct Cargo run needs the Server and Worker built first.

## Entry and failure

| Transition                           | Guard and result                                                                                |
| ------------------------------------ | ----------------------------------------------------------------------------------------------- |
| Hidden → prepare                     | Capture current semantic origin; read source and listing without mutation                       |
| Prepared Create → Start              | Still-current visible intent authorizes one plan using defaults; no second confirmation         |
| Prepared Open → list                 | Exact source, Session, non-stale revision and stable checkout paths                             |
| Choose Open                          | A complete current draw token and decoded packet authorize that exact choice                    |
| Cancel preparation or list           | Drop unsubmitted intent; late discovery cannot start an operation                               |
| Start → observe                      | Retain one operation identity before fallible result consumption; uncertainty permits only Read |
| Pending → Read                       | One bounded request at a time, re-read after 250 ms doubling to 2 s; output continues           |
| Opened → navigate                    | Prove existing reuse or first-terminal start, exact fresh membership and unchanged origin       |
| Navigate → Ready                     | Existing cleanup, attach, control, geometry and first-frame sequence                            |
| Newer input, hide or source change   | Revoke entry permission permanently for this operation; retain facts                            |
| Failed/unknown terminal or lost read | Preserve checkout/Workspace evidence; never recreate or compensate                              |

Preparation judges the source by the origin's own facts: its Context member, zoom, geometry and
primary pane. A commit elsewhere in the Workbench advances only the revision and leaves the source
current. Entry waits for an in-flight publication refresh and then rechecks the origin. Missing
membership allows at most one extra read-only refresh within the original entry deadline. For an
accepted operation, an admitted surface that becomes unpresentable is suspended, while a first
blocked reopen is refused; both withdraw automatic entry while retaining the operation and receipt.
An Opened result without current presentation proof reports that the Workspace is ready and directs
the user to select it in the existing navigator; uncovering never replays automatic entry. Directory
comparison uses the Server association policy without inventing filesystem identity or dropping the
Worker guard. When Git's probe refuses the listing source, the picker says the directory is not in a
Git repository or Git does not trust its owner; other listing failures keep the generic listing notice.
The surface and status line say the repository is being read only while the source and listing are
read; a drawn form or checkout picker states no Worktree notice until a refusal or result.

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
facts, Start/Read separation, target replacement and bounded Viewer release. Removal witnesses cover
exact linked targets, no automatic submission, full bilingual confirmation, dirty failure, cancelled
recovery, fenced Resume/Read separation, stale attempts, no focus stealing after hidden completion,
cause-specific failure copy with held-close guidance, drawn Abandon, release of results whose
Workspace is gone and bounded Pending re-reads.

Owning suites include `state/worktree_launch/tests.rs`,
`connection/native/worktree_launch/tests.rs`, `ui/tests/worktree_launch.rs` and adjacent
state, transport, input and rendering suites. `state/worktree_launch/tests/removal.rs` owns the
Remove transitions, `worktree_lane/tests.rs` owns exact Resume exchanges and `ui/tests/worktree_launch.rs`
owns composed confirmation and recovery rendering. Run
`cargo test --locked -p orchbee --all-targets --all-features` and the applicable root gate.
This workflow has no separate TLA model. Named implementation traces do not establish exhaustive
interleavings or physical terminal usability; real mouse/keyboard experience is separate evidence.
Shared ownership and commit protocols retain their registered models.

On Windows, native Worktree forms render checkout paths with backslash separators. This is a display projection only; selection and operations retain the exact canonical path.
