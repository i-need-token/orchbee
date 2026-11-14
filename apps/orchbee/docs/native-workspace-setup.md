# Native Workspace creation and organization

## Scope and ownership

A Workspace is independently usable. Project is optional organization, separate from its directory,
Git checkout, terminal context and execution lifetime. Server owns canonical navigation and
revisions; the Client owns selection and input. This reference extends
[context selection](native-context-selection.md) and the Server's
[directory contract](../../orchbee-server/docs/workspace-terminal-directory.md).

On Windows, New Workspace, the empty state's Create Workspace, `Ctrl+b a` and `Ctrl+b Shift+N`
immediately prepare and submit a Workspace with its first default shell. They open no directory or
name form and require no confirmation. The default result is ungrouped. New Workspace from an exact
Project row's menu or heading uses that Project, including empty and noncurrent Projects. Creating a Workspace never
starts an AI provider automatically.

Quick preparation leaves the sidebar and current Tab intact and immediately displays a pending
sidebar item using the generated name and a current top Tab. The top Tab uses the localized automatic Tab 1
title from [Tab labels](native-tab-labels.md). Accepted creation progress appears in the content region without a modal.
This feedback improvement is scoped to the Windows TUI. GUI creation retains its existing
activation owner and the same canonical creation semantics.

Ordinary and worktree-backed Workspaces are peers. Each can contain multiple Agents, listed directly
under the Workspace. [Worktree launch](native-worktrees.md) owns checkout creation and opening;
[Workspace Close](native-workspace-close.md) owns terminal disposal.

## Creation defaults and result ownership

Read-only preparation captures the exact initiating context, revision and token. Started from the
focused context, by a key or command, the directory is the current directory of the terminal the
focused Panel shows, a displayed child terminal included, so a `cd` there moves the default. Started
from a Project row without a chosen folder, or without a focused Panel, it is the current directory
of the focused Workspace's first terminal: the primary terminal of the first Tab's leading Panel.
Preparation asks the Server once with the
[current-directory read](../../orchbee-server/README.md#workspace-current-directory-reads), naming
that Panel and Presentation when there is one; the Server samples that terminal and otherwise
answers the Workspace's recorded directory. The Client attaches no Viewer of its own. When the Server refuses the read or no longer finds the Workspace, the
directory is the focused Workspace's recorded directory in the captured snapshot. Without one,
including an empty Session, it is the directory the Client was launched from. A failed exchange
fails preparation like any other carrier failure. A missing or invalid directory refuses
preparation; the Client does not invent one.
The directory basename supplies the initial name,
with a localized fallback for a root path. Generated names avoid every Workspace label in the
captured snapshot, adding ` (2)`, ` (3)` and subsequent suffixes as needed. Truncation leaves room
for the suffix within the 512-Unicode-scalar label bound. An unedited generated name launches an
automatic Workspace whose name later follows its first terminal; see [automatic Workspace names](native-location-labels.md#automatic-workspace-names).
Explicit labels remain unchanged. Default
and Project-targeted creation use this same preparation path. A folder chosen in a Project row
menu's New Workspace submenu replaces the default directory.

One revision-fenced `WorkspaceLaunch` accepts the directory and commits Workspace, Directory,
Context, Tab, Panel and primary Presentation together. Project membership is a nullable property;
no preliminary Default Project command is issued. Worker validates the literal directory and starts
the configured default shell. Failed admission publishes no partial Workspace. A committed pending,
failed or unknown shell retains its structural IDs and does not create a replacement.

The mailbox owns FIFO admission, bounded preparation/command leases and retained evidence. A known
receipt is saved before any fallible snapshot read or UI handoff. Exact Session, revision, directory,
context and terminal member proofs precede cleanup, attachment and first-frame readiness. Other
Clients keep their selection. A newer explicit focus or surface intent revokes the originating
Client's automatic entry/return permission; a delayed receipt still validates the committed
resources and remains inspectable. When the first launch replaces `NoWorkspace`, the Client still
installs the verified Workspace and member because the empty location is no longer valid. This
canonical repair does not restore revoked return permission or dismiss the newer Navigator surface.

The compact sidebar is hidden only after the matching terminal reaches accepted first-frame Ready
and the original return permission still holds. Wide, hidden and other-overlay cases consume that
permission without changing layout. Cancellation and later navigation never rearm it. Screen
publication and preference persistence remain separate from terminal readiness.

The provisional display invents no resource IDs or resource actions. Canonical selection and the
source terminal remain retained until validated handoff. Admission invalidates the old pointer map
before the next input event; live pane probes, selection and history admission reject the hidden
source while the provisional content is current. Typing cannot reach it, and gesture cancellation
still cleans up its original owner. Newer focus or surface intent, including during
preparation, lets creation continue in the background and permanently revokes automatic landing.
Definite refusal removes the provisional display. Unknown or committed-but-unreconciled outcomes
retain their existing request/resource recovery facts without replay or replacement.
Carrier retirement immediately hides creation feedback while preserving those recovery facts.

When the initiating Client enters the created Workspace, its Project or Recents group expands and
sidebar navigation follows that Workspace. Focus and topology may arrive in either order; a topology
publication alone does not expand a folded group, and a later explicit fold stays folded. A newer
navigation intent still revokes automatic entry by the rules above.

The known Workspace ID replaces the provisional row. The active Client's validated command-lane
snapshot supplies canonical sidebar topology without waiting for the independent metadata observer.
A newer same-Session checkpoint supersedes older topology, including when it removed the Workspace.
Labels never deduplicate creations, and a prior receipt never supplies a newer launch's identity.

The provisional row occupies the append position in its eventual machine and Project or Recents
group from the first frame. Its name aligns with Workspace names and stands alone: no pending glyph
or progress text, which the content region already shows. It shares the canonical list's
row gaps and viewport window; foreground creation anchors that window to the same slot before and
after the exact-ID handoff. A folded destination Project or Recents heading shows neither a
temporary child nor current feedback; the window reveals that heading instead.

## Optional Project organization

Right-click a Workspace and open Move to Project. It lists the other Projects in canonical order,
using exact captured IDs, then New Project… and, for a grouped Workspace, Remove from Project.
New Project asks only for its name; `WorkspaceCreateProject` creates the Project and
moves the existing Workspace in one canonical commit. Duplicate labels are permitted; captured IDs
determine the target. Project can contain Workspaces from multiple repositories.

The Move to Project submenu changes only `workspace.projectId`. It preserves Workspace, context, Tab,
Panel, Presentation, Directory and terminal identities, cwd, Git files, Agents and the initiating
Client's selection. Acting on a noncurrent Workspace does not enter it. Project Rename likewise
targets the exact row without selecting a child.

Sidebar drag and keyboard sorting instead submit one `navigation.configure` through the same
nonmodal setup lane. The source captures Session, revision and complete Navigation; release or keyboard
admission rechecks that source and requires the painted grouping and display order to match it, or
match it with the one move a drag shows live; the
authority rechecks CAS. The shared pure transition supplies
the exact expected membership and order for receipt verification; a same-revision partial or
unrelated edit is divergent. A known receipt is retained before refresh, and subsequent recovery
never repeats the move. Sorting changes no terminal selection or physical resource. The
[sidebar layout contract](tui-shell-layout.md) owns drop feedback, cancellation and local bindings.

Admission immediately projects the exact pure navigation transition over its immutable baseline.
This reversible display never writes the canonical cache or claims a commit revision. Definite
refusal or an unknown outcome removes the speculative order; known receipts and recovery facts
remain with the setup lane. Confirmed topology replaces it without waiting for metadata polling,
and a newer observed canonical revision wins over an older pending move. A second sort still
requires canonical admission; speculative rows never authorize another move.

Dissolve Project removes organization and leaves its Workspaces ungrouped. It loses nothing else, so
it runs at once as the unconditional `ProjectDissolve` and reports "Project dissolved · Workspaces
retained" in the status line, or only "Project dissolved" when the Project had no member Workspace;
`ProjectDissolveIfIdle` remains for the CLI. Close all Workspaces is
a separate explicit Project action with its own destructive confirmation and retained member set.
Ordinary Workspace Close never expands into a Git family or Project. An automatically created
Project is retained when only one Workspace remains.

| Transition                                        | Guard and result                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------- |
| Open the Move to Project submenu                  | Capture exact Session, snapshot, target and current focus; no mutation       |
| Choose an existing Project or Remove from Project | Submit one membership command; a no-op preserves revision                    |
| Choose New Project…                               | Show one bounded name field; no empty Project is created before submission   |
| Complete draw or pointer choice                   | Only the current token and a fully presented choice authorize submission     |
| Cancel before submission                          | Release capture without changing canonical state                             |
| Known commit                                      | Retain immutable evidence before reading the committed snapshot              |
| Snapshot available                                | Validate the result and preserve the current terminal and focus              |
| Definite refusal                                  | Preserve state; never rebase or replay automatically                         |
| Ambiguous handoff or failed follow-up read        | Keep known facts and expose recovery; never infer absence or repeat creation |

The Move to Project submenu follows the [action-menu input contract](native-action-menus.md).
Bounded UTF-8 and bracketed paste use the existing modal decoder. Names allow 512 Unicode scalars;
directory paths allow 4,096 UTF-8 bytes without NUL.
Input remaining in the submission packet cannot confirm another step or reach a terminal.
Resize, stale draw tokens and late outcomes cannot retarget an action.

Header X and Cancel use the existing cancellation path; form fields are clickable and Submit uses the same owner as Enter.

An opening attempt that cannot present the required shell is refused with a localized status notice
and retains no hidden input owner. Once the shell has completed one draw, insufficient geometry
suspends that same surface incarnation: its token, stage, selected Project, draft text and editor
cursor remain intact. While suspended, ordinary edits, selection, scrolling and submission are
consumed without mutation; only the current local `Esc` return is available. A later complete draw
resumes the same surface. Absence from a frame because another owner covers it only revokes the
current draw proof; it is neither refusal nor cancellation.

New Project… opens its name editor directly from the addressed Move to Project submenu. `Esc` or Cancel
closes the unsubmitted name draft. Quick Workspace work that already crossed its submission boundary
continues under its original operation identity when the surface is hidden; presentation never
replays it.

## Explicit directory and label editing

The Windows creation menu and global action menu also offer **New Workspace in directory…**.
This uses the same read-only preparation, then displays the prepared path and generated name in
the existing two-field draft with directory focus. It submits nothing until an explicit, drawn
Submit/Enter; Esc cancels without creating resources and displays no pending item before then.
Up/Down or a drawn directory row selects the prepared directory, recent workspace directories, then
other known workspace directories, deduplicated by `same_directory_spelling` (on Windows ASCII
case, either separator and a trailing separator) and limited to eight choices. Recency belongs to
the Client's existing volatile navigation memory; no filesystem scan or new history file is
created. An automatic name follows path edits until the user edits the name. A Project creation
menu retains its exact Project target. The edited directory and name belong only to this launch.
One-click creation and its existing shortcuts continue to submit immediately.

When the Server refuses the submitted directory, the draft reopens with every typed field and
directory focus, and the Directory field's error row names the Worker's cause: the folder does not
exist, the path is not a folder, or the folder cannot be accessed, or that the folder cannot be used
when the Server names no cause. The Client never probes the path itself, since the Worker may be on
another machine. The error shows while the field still holds the refused text; it does not block
submitting that text again, so a folder created meanwhile can be retried. A directory draft for an
existing Workspace reopens the same way. A draft whose baseline moved during the flight stays
closed and leaves the not-applied status.

The review lists these choices under the Directory field's help row, below a `Recent` heading, at
most three rows at a time. A row shows the folder name and, right-aligned in `text-secondary`, the
folder that holds it, spelled from `~` inside the home folder that the field itself expands. The row
matching the field's text under that comparison carries the shared picker cursor and selection
fill; no help sentence names Up/Down. A row shows hover and fills the field on click without
submitting; the heading has no target. A longer list keeps the selected row in view and shows its
visible range, such as `2-4/6`, at the heading's end. A parent cut in the middle keeps at least
eight cells, or all of a shorter one; otherwise it goes before the name truncates. Under height pressure the review gives
up blank rows, then help rows, then the heading and the Project line, then list rows down to one.
Candidate hit regions never share the footer. A host unable to paint the complete review cannot
submit it.

The draft explains that the default is the terminal process directory. PowerShell's logical `$PWD`
can differ after ordinary `cd`; this path is not claimed to be that logical location. Users can
review the actual sampled path and supply an existing folder, including Unicode and spaces,
without changing a global shell profile. This directory-review entry is scoped to the Windows
TUI; GUI creation keeps its existing directory-selection owner and the shared launch contract.

The TUI offers no way to give an existing Workspace its first directory; `orchbee workspace
accept-directory` remains that route for an Empty, unconfigured Workspace, and New Workspace in
directory… covers the need in the TUI. For an Empty Workspace with a directory, the Empty-workbench
guidance keeps the directory as a localized fact and renders the configured effective Create Tab
chord through the shared key-hint component only while no capture owner is active. Its label is the
action's shortcut-help title. Empty guidance appears only for a connected Client, so it never
repeats the help table's connection qualifier. An Empty Workspace without a directory shows only the
fact that none is configured, with no hint, as the GUI does.

[Location label editing](native-location-labels.md) owns Workspace and Project rename. Labels are
editable presentation, never selection or execution identity. Bare Windows
[directory startup](native-directory-startup.md) uses the same independent Workspace model.

## Global Workspace ordering

`workspace move` and the Clients' block moves change global display order without changing Project
membership or terminal identity. The ordered IDs remain a complete permutation of canonical
Workspaces. Insertion boundaries and caller-ordered blocks retain the existing CLI semantics;
no-op preserves the revision. Grouping derives presentation from membership and does not redefine
physical Git association. See [CLI workflows](cli-workflows.md).

## Correctness and verification

Maintain the state, carrier, input, rendering and integration witnesses for one-step launch,
ungrouped and Project-targeted creation, atomic new-Project membership, noncurrent moves,
dissolution, unchanged terminal bindings and selection, late completion and non-replay recovery.
The owning suites include `state/workspace_setup`, `state/tests/workspace_setup`,
`connection/native/tests/quick_workspace` and `terminal_input/dispatcher/tests/workspace_setup`.
`ui/tests/workspace_setup/responsive` controls preparation, launch, focus, refusal and recovery order;
`tui_render/workspace_sidebar/tests/creation` checks exact-identity observer handoff. These local
implementation traces do not provide exhaustive TLC assurance.
`ui/tests/workspace_setup/sidebar` covers immediate reorder, UI wakeup and refusal/commit settlement;
`machine_spaces/tests/navigation_projection` covers stale observations, newer deletion and exact
metadata identity. `machine_picker/tests/spaces/anchoring` covers mutation, resize and provisional-slot
scroll anchors. These deterministic cases do not establish physical paint latency.
Those sidebar traces include append-position and scroll-window stability with multiple Workspaces;
`ui/tests/quick_workspace` covers formless quick submission plus the explicit directory review, its
folder rows and selection fill, cancellation and exact edited-path submission;
`state/tests/quick_workspace` covers the `~` spelling of listed folders.

Run `cargo test --locked -p orchbee --all-targets --all-features` and the applicable root gate.
This workflow has no separate TLA model. Implementation traces and process tests do not replace
real-terminal mouse/keyboard, viewport, first-frame and interaction review. Shared commit and
ownership protocols retain their separately registered models.
