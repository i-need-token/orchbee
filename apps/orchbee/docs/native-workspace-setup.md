# Native Workspace creation and organization

## Scope and ownership

A Workspace is independently usable. Project is optional organization, separate from its directory,
Git checkout, terminal context and execution lifetime. Server owns canonical navigation and
revisions; the Client owns selection and input. This reference extends
[context selection](native-context-selection.md) and the Server's
[directory contract](../../orchbee-server/docs/workspace-terminal-directory.md).

On Windows, New Workspace, `Ctrl+B a` and `Ctrl+B Shift+N` immediately prepare and submit a
Workspace with its first default shell. They require no Project, name form or second confirmation.
The default result is ungrouped. New Workspace on an exact Project row uses that Project, including
empty and noncurrent Projects. Creating a Workspace never starts an AI provider automatically.

Ordinary and worktree-backed Workspaces are peers. Each can contain multiple Agents, listed directly
under the Workspace. [Worktree launch](native-worktrees.md) owns checkout creation and opening;
[Workspace Close](native-workspace-close.md) owns terminal disposal.

## Creation defaults and result ownership

Read-only preparation captures the exact initiating context, selected Panel, primary Terminal
binding, revision and token. It samples the primary terminal's root-process directory, even if an
Agent presentation is foregrounded. An explicitly absent cwd falls back to the Workspace directory,
then Windows user home. An unavailable sample, missing root or invalid path refuses preparation;
the Client does not invent a successful sample. The directory basename supplies the initial name,
with a localized fallback for a root path. Generated names avoid every Workspace label in the
captured snapshot, adding ` (2)`, ` (3)` and subsequent suffixes as needed. Truncation leaves room
for the suffix within the 512-Unicode-scalar label bound. Explicit labels remain unchanged. Default
and Project-targeted creation use this same quick path.

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
resources and remains inspectable.

The compact sidebar is hidden only after the matching terminal reaches accepted first-frame Ready
and the original return permission still holds. Wide, hidden and other-overlay cases consume that
permission without changing layout. Cancellation and later navigation never rearm it. Screen
publication and preference persistence remain separate from terminal readiness.

## Optional Project organization

Right-click a Workspace and choose Move to Project. Select an existing Project, No Project, or
New Project. New Project asks only for its name; `WorkspaceCreateProject` creates the Project and
moves the existing Workspace in one canonical commit. Duplicate labels are permitted; captured IDs
determine the target. Project can contain Workspaces from multiple repositories.

Moving or ungrouping changes only `workspace.projectId`. It preserves Workspace, context, Tab,
Panel, Presentation, Directory and terminal identities, cwd, Git files, Agents and the initiating
Client's selection. Acting on a noncurrent Workspace does not enter it. Project Rename likewise
targets the exact row without selecting a child.

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

Run `cargo test --locked -p orchbee --all-targets --all-features` and the applicable root gate.
This workflow has no separate TLA model. Implementation traces and process tests do not replace
real-terminal mouse/keyboard, viewport, first-frame and interaction review. Shared commit and
ownership protocols retain their separately registered models.
