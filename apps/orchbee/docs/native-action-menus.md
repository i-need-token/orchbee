# Native action menus

## Interaction

Windows `Ctrl+B F2`, `[keys].menu`, and the sidebar Menu button (status-bar fallback)
open the global action menu. Unmodified right-click opens a menu for a drawn pane, tab or Workspace
row. Opening or cancelling does not change focus. Pane content retains its configured
`right_click = "panel"` policy; pane chrome remains Client-owned. Shift gestures retain
the host-terminal bypass. Machines retains `Ctrl+B Shift+M`.
The Menu button and Machines indicator occupy separate status cells and click regions.
The Workspace sidebar admits right-click only for an exact, still-present row on the
current machine and painted Site. It uses the existing Workspace menu and never
requests a machine or context switch on opening or cancellation.

Pointer opening anchors the frame to the triggering host cell, including sidebar
rows and their action buttons. The frame clamps to the entire viewport above the
status line; it can overlap navigation chrome. Keyboard opening remains centered
in the pane content area. Neither path changes the target or underlying focus.

Tab menus also expose pinning, movement and group management. Painted group headers open
the same [group configuration owner](native-tab-groups.md) without selecting a member.
Tab menus expose local toggle/range/all/clear selection and configure the captured selected set
when their target belongs to it. Pinning, group creation and removal show the affected count.
Selection or selected-member changes revoke the menu proof; background metadata actions keep
the current terminal attached. Menu movement uses GUI group boundaries, including collapsed neighbors.
Metadata actions preserve the current terminal; the submenu retains its exact addressed Tab.

Selected/other/trailing Close and group Close show their frozen affected count, then open one
[complete-set confirmation](native-tab-close-sets.md). Relative Close protects pinned Tabs;
direct selected/group Close includes them where applicable. Hidden collapsed members remain targets.

Up/Down or j/k select a row; Home/End select the first/last row; Enter executes the
selected action. Unmodified, button-free motion selects a painted row without
executing it; leaving the menu retains selection. A left click on a drawn row
executes it. Esc or a click outside cancels. Rows scroll within the menu, leaving the status line visible. A
fixed footer shows the selected row and total (`current/total`), retaining Esc when
the Enter hint cannot fit. The menu replaces the entire status row; its full,
compact and minimal hints retain Esc without underlying notice or Machines text.
Menu width includes one cell of padding on each side and borders; labels that exceed the
available width truncate on grapheme boundaries with the theme's ellipsis.
Selection fills the complete inner row. A text marker remains when the theme's
selection fill is indistinguishable from its overlay fill. A reason row is reserved
only when the menu has unavailable actions, so hover never changes row geometry.
Each frame derives its visible window from the current row budget, keeping the
selected action visible in the first paint after resizing. The successful draw
commits that same window for pointer admission; a larger frame fills available
rows without changing selection.
A cancellation or unavailability notice appears once on the status line; current
key hints are not replaced by a second copy of the session's notice.
A surface that cannot present its title, a row and its exit instruction closes with a
localized notice. Ordinary terminal output continues while the menu captures input.
The underlying pane cursor is hidden during capture;
cancelling restores the ordinary pane projection without changing focus.
Known unavailable actions use the full disabled text style, retaining its modifiers
at every color level. They can be selected to read a localized reason; Enter or clicking keeps the menu open and
cannot confirm or navigate. A fixed reason row leaves the exit instruction and
selection count visible even at the smallest presentable geometry. The status
hint omits execution instructions for the disabled selection.

Browsing has no time limit: reading, selection, repainting and disabled activation
do not start an execution timer. Confirming an available action creates one
absolute 30-second deadline. The same deadline bounds target navigation and
handoff; draw and navigation callbacks never renew it. Expiry cancels the
unhanded action without rolling back accepted navigation. Target, origin,
geometry, configuration and capture invalidation still cancel browsing.

Availability projects existing action-owner prerequisites, without a universal
Ready guard. Workspace directory configuration is disabled for a Workspace that
already has a directory; for the current Workspace its original empty-state and
setup-admission guards also apply. Released pointer capture disables Release
pointer capture. Unknown asynchronous prerequisites continue through their original
owners; the menu does not infer Git support, worker reachability or an operation
outcome from read-only machine observations.

Project menus expose New Workspace, rename, Worktree create/open, Dissolve Project and
Close All Workspaces. New Workspace accepts the exact drawn Project, including empty and
noncurrent Projects, through the [setup owner](native-workspace-setup.md#creation-defaults-and-result-ownership).
It uses the same immediate creation path as an ungrouped Workspace, without a Project picker.
Rename, dissolve and explicit Close All also target the drawn Project without entering a child.
Worktree entries require the current Workspace to belong to that Project; otherwise they explain
“Select a Workspace in this Project first.” Typed target proofs survive ClientView and InputDispatcher.
Setup retains the current source and focus, checks the original menu deadline before admission,
and uses its bounded preparation without timing optional human editing.

Workspace Move to Project opens a choice of existing Projects, No Project or New Project.
The latter asks for one name and atomically creates the Project and moves the Workspace.
Moving a noncurrent Workspace does not select it. Dissolve changes grouping only; Close All
opens the [separate scoped confirmation](native-workspace-close.md). Neither operation deletes Git checkouts.

Workspace Close uses the exact addressed Workspace without navigation and opens the normal
[scoped confirmation](native-workspace-close.md). Closing a background Workspace preserves
the unrelated current session.

The sidebar `+` entries use `MenuTarget::Create` to select the creation subset of the
existing global or exact-Project menu. The Project entry puts Quick Workspace first;
its normal prerequisites, target proof, disabled reasons and input capture remain unchanged.
The [shell layout](tui-shell-layout.md#quick-creation-controls) owns their geometry and
the direct Tab-strip create gesture.

Pane menus expose existing split, zoom, keyboard resize, copy, takeover and close
actions. Tab menus expose create, rename and close. Workspace menus expose quick
create, new tab, directory, rename, Move to Project, Worktree create/open and close. The global menu
exposes Workspace creation, Worktrees, reload, Settings, help, sidebar, navigator, Agent board,
pointer release and detach. Close actions still arm the original confirmation; the
menu never confirms deletion. Settings delegates to its existing editor; checkout deletion, policy editing and custom
or plugin-defined menus are outside this surface.

When the carrier is unavailable, the global menu keeps only client-local help,
sidebar, pointer release and detach. Member menus require a live carrier. An observed
geometry is required even before the first session state arrives.

## Model and ownership

The user wants to discover and execute existing actions without memorizing chords.

| Record field    | Contract                                                                                                                                                                                                                                                               |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs          | Drawn stable target, exact context/member/incarnation, current action prerequisites, complete decoded input packet, configuration, monotonic deadline and successful menu draw receipt                                                                                 |
| Data            | One local episode, bounded rows and selection with owner-derived unavailability reasons, presentation token, frozen confirmed action, owned navigation generation and optional exact target member                                                                     |
| Outputs         | Local capture and drawing; bounded navigation through existing membership owners; at most one action handed to the full ClientView/dispatcher path; localized cancellation or refusal                                                                                  |
| Decisions       | Existing captures, including the Windows navigator, take priority. Help and sidebar remain unchanged underlays. Choosing an action admits navigation; opening does not. Each action retains its original availability policy rather than a universal Ready requirement |
| Invariants      | No undrawn or stale activation, no focus on open/cancel, no action on a replacement target, no implicit retry, no input or decision-packet tail reaching a pane; close still needs its original confirmation                                                           |
| Model           | Finite safety machine for capture, drawing, target invalidation, owned and foreign navigation, cancellation, prerequisites and one action transfer                                                                                                                     |
| Dependencies    | Existing native membership, terminal state, input decoder, hit map, layout, localization and action owners; no new dependency, wire or persistent data                                                                                                                 |
| Ownership       | NativeUi owns menu admission and target navigation; the deterministic menu owns selection and draw authority; ClientView owns local view decisions; InputDispatcher alone applies shell, mode and layout effects; original operation owners alone execute mutations    |
| Reference scope | Discoverability for current OrchBee actions, not complete Herdr menu parity                                                                                                                                                                                            |

`MenuTarget::Project` carries a `WorkbenchProjectId`, never a label or row index.
Admission freezes the complete canonical Project and, only when its ID matches the
current Workspace's Project, that Workspace/Directory source. Empty Projects and
Projects outside the current Workspace remain valid menu targets with Quick Workspace available
when the setup owner admits it, as do Project rename, dissolve and scoped Close All;
Worktree actions retain their current-Workspace prerequisite.
The same Project and optional context proof must survive confirmation and handoff;
an origin change or a replaced record cancels the episode rather than retargeting it.

All object-entry adapters pass the successfully painted `DrawnMenuSource` Site and
revision. The sidebar owner also checks the current machine and stable focused row.
Right-click, the row's visible action entry and an object-menu key use that same target.
Object keys belong only to explicit sidebar navigation; terminal-owned keys retain
passthrough. The caller retains sidebar focus while the menu captures input and
reconciles a removed row on dismissal. The menu keeps no second sidebar focus state.
The global menu remains independently available.

The [Windows navigator](native-context-selection.md#windows-capturing-navigator)
consumes menu shortcuts and pointer entries while it owns input. Close it before
opening a menu; a menu may still open the navigator through its ordinary action.

| Project context at opening                       | Availability and effect                                                                                                             |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| Current Workspace belongs to the exact Project   | Existing actions retain their own prerequisites; confirmation offers one action without navigation                                  |
| Standalone, a different Project, or no Workspace | New Workspace, rename, dissolve and Close All target the exact Project without navigation; Worktree entries explain missing context |
| Project or captured Workspace/Directory replaced | Cancel before confirmation or handoff; never substitute another child                                                               |
| Cancel                                           | Preserve the native origin and caller-owned sidebar focus                                                                           |

## Transitions and cancellation

| State / event                                                | Guard and result                                                                                                                                                |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hidden → open                                                | Stable drawn source, no genuine capture or pending gesture; freeze target and current origin, capture opening packet                                            |
| Open → draw                                                  | Paint the same episode and bounded rows; only a successful draw acknowledges its token                                                                          |
| Open → select                                                | Move the bounded selection and revoke the previous token until redrawn                                                                                          |
| Open → availability change                                   | Refresh owner-derived reasons and revoke the previous token until redrawn; a known unavailable row cannot authorize confirmation                                |
| Open → disabled activation                                   | Keep selection and capture, reveal the reason, and transfer no action or navigation                                                                             |
| Open → time passage                                          | Retain browsing and capture without creating an execution deadline                                                                                              |
| Open → confirm                                               | Complete input, current drawn token and an available row; freeze exactly one action and prerequisite state, mint one absolute 30-second execution deadline      |
| Confirmed → navigation                                       | Distinguish AlreadyExact, owned Context generation, owned Tab generation and admitted local focus; never infer admission from a no-op                           |
| Navigation → handoff                                         | Only the owned prepare/apply result authorizes advancement; exact target still exists, source and prerequisites match; the original action decides availability |
| Handoff                                                      | Remove the one-shot intent before applying the full ClientView transition and dispatcher effects; no retries                                                    |
| Cancel / execution timeout / unavailable                     | Revoke the unhanded intent; accepted navigation is not rolled back, and no action executes                                                                      |
| Source / geometry / configuration / covering capture changes | Revoke the unhanded episode; a foreign navigation retains its own semantics and cannot complete the menu intent                                                 |

Presentation tokens bind the episode, rows, geometry and exact semantic target. They
do not bind ordinary pane output sequences. Output may repaint the underlay without
cancelling a menu. A new incarnation, removed target, changed structural source,
unpresentable menu, resize or conflicting capture revokes activation authority.

Hover refines `Select`: it never confirms, navigates or starts an execution deadline.
Only rows in the last successfully drawn window accept it. Consecutive hovers may
reuse that window's token until the next draw, but each changed selection mints a
new token: the old receipt cannot authorize either clicking or Enter. A repeated
hover on the same row changes nothing. Keyboard selection and availability changes
discard the hover window as well; a new draw replaces it. Source and geometry
invalidation retain their existing cancellation rules. No hover queue or timer is owned.

With pointer capture enabled, the dispatcher requests DEC `1003` only while a menu
is active. The lifecycle owner restores `1002` on dismissal or handoff and disables
`1003`, `1006` and `1002` on capture release or shutdown, including partial writes.
Released capture stays released even when the keyboard opens a menu. Menu capture
swallows motion and decision tails instead of sending them to an embedded program;
pane tracking and any pre-existing gesture retain their original ownership outside it.

Object-entry admission checks the Site and Workbench revision of the successfully painted
hit map before freezing live records. Workspace and current-context proofs retain the
exact Workspace and primary Directory, including Worker and path; Tab proofs retain
the full projected record. Owned navigation candidates and the final selected target
must match these proofs. An unchanged ID cannot authorize a replaced source.
Context Refresh and Tab attachment generations have separate typed navigation keys;
equal numeric values cannot settle each other's menu intent.

The opening right press precedes ordinary pointer focus/selection. Its matching
release remains captured after cancellation or handoff. The entire decoded decision
packet, including fragmented sequences and paste, remains captured after the menu
closes. Pointer release and zoom verify their frozen prerequisite before handoff.
Every menu close or handoff also captures already-polled input until a fresh frame
publishes. A pre-existing pane press, selection, link, divider drag, pending focus or queued wheel
stream refuses menu admission and remains with its original owner.
Arming a prefix alone preserves a divider drag until the action is chosen; a chosen
non-menu action retains its original cancellation behavior.
Focus loss or a genuinely new matching press expires an unreported opening release,
so it cannot swallow a later program-owned release.

Directory configuration exposes the same pure refusal query used by its setup
owner. Exact frozen Workspace/Directory records support a definite configured
refusal for non-current targets without navigating to inspect them. Admission is
queried again before keyboard or pointer confirmation and before handoff. An
availability change invalidates the draw token; target/source invalidation still
closes the episode under the existing rules.

## Correctness and verification

The owning implementation tests cover draw authority, packet tails, generated event replay, cancellation, source/target/focus drift, exact draft capture, at-most-once handoff, known-commit retention and execution deadlines independent of browsing time.

Maintain the concrete witnesses in `src/action_menu/tests.rs`, `src/terminal_input/dispatcher/tests/action_menu.rs`, `src/tui_shell/tests/action_menu.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
