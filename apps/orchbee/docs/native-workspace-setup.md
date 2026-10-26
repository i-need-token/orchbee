# Native Workspace creation and organization

## Scope and ownership

A Workspace is independently usable. Project is optional organization, separate from its directory,
Git checkout, terminal context and execution lifetime. Server owns canonical navigation and
revisions; the Client owns selection and input. This reference extends
[context selection](native-context-selection.md) and the Server's
[directory contract](../../orchbee-server/docs/workspace-terminal-directory.md).

On Windows, New Workspace, the empty state's Create Workspace, `Ctrl+B a` and `Ctrl+B Shift+N`
immediately prepare and submit a Workspace with its first default shell. They open no directory or
name form and require no confirmation. The default result is ungrouped. New Workspace on an exact
Project row uses that Project, including empty and noncurrent Projects. Creating a Workspace never
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

Read-only preparation captures the exact initiating context, revision and token. The directory is
the focused Workspace's recorded directory. Without one, including an empty Session,
it is the directory the Client was launched from. A terminal's later working directory,
including a `cd` inside that Workspace, never moves this default, and preparation samples no
terminal. A missing or invalid directory refuses preparation; the Client does not invent one.
The directory basename supplies the initial name,
with a localized fallback for a root path. Generated names avoid every Workspace label in the
captured snapshot, adding ` (2)`, ` (3)` and subsequent suffixes as needed. Truncation leaves room
for the suffix within the 512-Unicode-scalar label bound. Explicit labels remain unchanged. Default
and Project-targeted creation use this same preparation path.

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

The known Workspace ID replaces the provisional row. The active Client's validated command-lane
snapshot supplies canonical sidebar topology without waiting for the independent metadata observer.
A newer same-Session checkpoint supersedes older topology, including when it removed the Workspace.
A collapsed Project shows the current
marker only while the checkpoint still places that exact Workspace in it. Labels never deduplicate creations, and
a prior receipt never supplies a newer launch's identity.

The provisional row occupies the append position in its eventual machine and Project group from
the first frame. It shares the canonical list's row gaps and viewport window; foreground creation
anchors that window to the same slot before and after the exact-ID handoff. A collapsed destination
Project carries the current marker instead of exposing a temporary child outside the group.

## Optional Project organization

Right-click a Workspace and open Project. An ungrouped Workspace offers New Project; a grouped one
offers Leave Project. Existing Projects follow in canonical order, using exact captured IDs.
The compatibility organization chooser still offers existing Projects, No Project and New Project.
New Project asks only for its name; `WorkspaceCreateProject` creates the Project and
moves the existing Workspace in one canonical commit. Duplicate labels are permitted; captured IDs
determine the target. Project can contain Workspaces from multiple repositories.

The grouping chooser changes only `workspace.projectId`. It preserves Workspace, context, Tab,
Panel, Presentation, Directory and terminal identities, cwd, Git files, Agents and the initiating
Client's selection. Acting on a noncurrent Workspace does not enter it. Project Rename likewise
targets the exact row without selecting a child.

Sidebar drag and keyboard sorting instead submit one `navigation.configure` through the same
nonmodal setup lane. The source captures Session, revision and complete Navigation; release or keyboard
admission rechecks that source and requires the painted grouping and display order to match it; the
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

Dissolve Project removes organization and leaves its Workspaces ungrouped. Close all Workspaces is
a separate explicit Project action with its own destructive confirmation and retained member set.
Ordinary Workspace Close never expands into a Git family or Project. An automatically created
Project is retained when only one Workspace remains.

| Transition                                 | Guard and result                                                             |
| ------------------------------------------ | ---------------------------------------------------------------------------- |
| Open a grouping chooser                    | Capture exact Session, snapshot, target and current focus; no mutation       |
| Choose an existing Project or No Project   | Submit one membership command; a no-op preserves revision                    |
| Choose New Project                         | Show one bounded name field; no empty Project is created before submission   |
| Complete draw or pointer choice            | Only the current token and a fully presented choice authorize submission     |
| Cancel before submission                   | Release capture without changing canonical state                             |
| Known commit                               | Retain immutable evidence before reading the committed snapshot              |
| Snapshot available                         | Validate the result and preserve the current terminal and focus              |
| Definite refusal                           | Preserve state; never rebase or replay automatically                         |
| Ambiguous handoff or failed follow-up read | Keep known facts and expose recovery; never infer absence or repeat creation |

The chooser supports keyboard navigation and mouse selection. The selected window bounds long
lists; labels and paths are safe terminal text. Bounded UTF-8 and bracketed paste use the existing
modal decoder. Names allow 512 Unicode scalars; directory paths allow 4,096 UTF-8 bytes without NUL.
Input remaining in the submission packet cannot confirm another step or reach a terminal.
Resize, stale draw tokens and late outcomes cannot retarget an action.

Header X and Cancel use the existing cancellation path; form fields are clickable and Submit uses the same owner as Enter. Project rows submit the exact painted choice on matching left release. Wheel only inside the Project list moves by three choices per notch, capped by visible rows per pass and clamped at either end. Any resulting selection requires a new complete draw before confirmation; a wheel followed by Enter in one packet cannot confirm an undrawn choice.

An opening attempt that cannot present the required shell is refused with a localized status notice
and retains no hidden input owner. Once the shell has completed one draw, insufficient geometry
suspends that same surface incarnation: its token, stage, selected Project, draft text and editor
cursor remain intact. While suspended, ordinary edits, selection, scrolling and submission are
consumed without mutation; only the current local `Esc` return is available. A later complete draw
resumes the same surface. Absence from a frame because another owner covers it only revokes the
current draw proof; it is neither refusal nor cancellation.

The New Project name editor is a child of the Project chooser. `Esc`, header Back or the matching
pointer action returns one level, restores the chooser's selection and draft, and discards only the
unsubmitted name edit. The chooser's own return closes the root draft. Quick Workspace work that
already crossed its submission boundary continues under its original operation identity when the
surface is hidden; presentation never replays it.

## Explicit directory and label editing

The Windows creation menu and global action menu also offer **New Workspace in directory…**.
This uses the same read-only preparation, then displays the prepared path and generated name in
the existing two-field draft with directory focus. It submits nothing until an explicit, drawn
Submit/Enter; Esc cancels without creating resources and displays no pending item before then.
Up/Down or a drawn directory row selects the prepared directory, recent workspace directories, then
other known workspace directories, deduplicated and limited to eight choices. Recency belongs to
the Client's existing volatile navigation memory; no filesystem scan or new history file is
created. An automatic name follows path edits until the user edits the name. A Project creation
menu retains its exact Project target. The edited directory and name belong only to this launch.
One-click creation and its existing shortcuts continue to submit immediately.

The draft explains that the default is the terminal process directory. PowerShell's logical `$PWD`
can differ after ordinary `cd`; this path is not claimed to be that logical location. Users can
review the actual sampled path and supply an existing folder, including Unicode and spaces,
without changing a global shell profile. This directory-review entry is scoped to the Windows
TUI; GUI creation keeps its existing directory-selection owner and the shared launch contract.

The Workspace Directory action (`Ctrl+B e` by default) accepts the first directory only for an
Empty, unconfigured Workspace. The path is an existing absolute directory; no shell expansion,
filesystem creation or home fallback occurs. Directory acceptance alone does not launch a shell.
The Empty-workbench guidance keeps the directory as a localized fact and renders the configured
effective Workspace Directory or Create Tab chord through the shared key-hint component only while
no capture owner is active. The menu and draft share the same pure admission query for configured,
nonempty, absent and busy targets.

[Location label editing](native-location-labels.md) owns Workspace and Project rename. Labels are
editable presentation, never selection or execution identity. Bare Windows
[directory startup](native-directory-startup.md) uses the same independent Workspace model.

## Global Workspace ordering

`workspace move` and `workspace move-block` change global display order without changing Project
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
`ui/tests/quick_workspace` covers formless quick submission plus the explicit directory review,
cancellation and exact edited-path submission.

Run `cargo test --locked -p orchbee --all-targets --all-features` and the applicable root gate.
This workflow has no separate TLA model. Implementation traces and process tests do not replace
real-terminal mouse/keyboard, viewport, first-frame and interaction review. Shared commit and
ownership protocols retain their separately registered models.

The explicit directory review paints up to two candidate paths in content rows, followed by separate hint and action rows. Candidate hit regions never share the footer. A host unable to paint the complete review cannot submit it.
