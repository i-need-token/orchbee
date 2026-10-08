# Native action menus

## Interaction

Windows `Ctrl+B F2`, `[keys].menu`, and the status-bar Menu button open the global
action menu. Unmodified right-click opens a menu for a drawn pane, tab or Workspace
row. Opening or cancelling does not change focus. Pane content retains its configured
`right_click = "panel"` policy; pane chrome remains Client-owned. Shift gestures retain
the host-terminal bypass. Machines retains `Ctrl+B Shift+M`.
The Menu button and Machines indicator occupy separate status cells and click regions.
The Workspace sidebar admits right-click only for an exact, still-present row on the
current machine and painted Site. It uses the existing Workspace menu and never
requests a machine or context switch on opening or cancellation.

Up/Down or j/k select a row; Home/End select the first/last row; Enter executes the
selected action. A left click on a drawn row executes it. Esc or a click outside
cancels. Rows scroll within the content area, leaving the status line visible. A
fixed footer shows the selected row and total (`current/total`), retaining Esc when
the Enter hint cannot fit. The menu replaces the entire status row; its full,
compact and minimal hints retain Esc without underlying notice or Machines text.
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

Project menus expose the existing Workspace creation picker, quick Workspace creation,
Project rename and Worktree create/open actions. The creation picker is labeled
“New Workspace (choose Project)…” because its owner still allows choosing a Project.
These entries require the explicitly current Workspace to belong to the exact Project.
Otherwise every entry stays disabled with “Select a Workspace in this Project first.”
The menu never chooses a child Workspace or creates a context for an empty Project.
Project actions do not navigate; the existing action owner receives the current context.

Pane menus expose existing split, zoom, keyboard resize, copy, takeover and close
actions. Tab menus expose create, rename and close. Workspace menus expose quick
create, new tab, directory, rename, Worktree create/open and close. The global menu
exposes Workspace creation, Worktrees, reload, Settings, help, sidebar, navigator, Agent board,
pointer release and detach. Close actions still arm the original confirmation; the
menu never confirms deletion. Settings delegates to its existing editor; checkout deletion, policy editing and custom
or plugin-defined menus are outside this surface.

When the carrier is unavailable, the global menu keeps only client-local help,
sidebar, pointer release and detach. Member menus require a live carrier. An observed
geometry is required even before the first session state arrives.

## Model and ownership

The user wants to discover and execute existing actions without memorizing chords.

| Record field    | Contract                                                                                                                                                                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs          | Drawn stable target, exact context/member/incarnation, current action prerequisites, complete decoded input packet, configuration, monotonic deadline and successful menu draw receipt                                                                              |
| Data            | One local episode, bounded rows and selection with owner-derived unavailability reasons, presentation token, frozen confirmed action, owned navigation generation and optional exact target member                                                                  |
| Outputs         | Local capture and drawing; bounded navigation through existing membership owners; at most one action handed to the full ClientView/dispatcher path; localized cancellation or refusal                                                                               |
| Decisions       | Existing genuine captures take priority. Help, sidebar and navigator remain unchanged underlays. Choosing an action admits navigation; opening does not. Each action retains its original availability policy rather than a universal Ready requirement             |
| Invariants      | No undrawn or stale activation, no focus on open/cancel, no action on a replacement target, no implicit retry, no input or decision-packet tail reaching a pane; close still needs its original confirmation                                                        |
| Model           | Finite safety machine for capture, drawing, target invalidation, owned and foreign navigation, cancellation, prerequisites and one action transfer                                                                                                                  |
| Dependencies    | Existing native membership, terminal state, input decoder, hit map, layout, localization and action owners; no new dependency, wire or persistent data                                                                                                              |
| Ownership       | NativeUi owns menu admission and target navigation; the deterministic menu owns selection and draw authority; ClientView owns local view decisions; InputDispatcher alone applies shell, mode and layout effects; original operation owners alone execute mutations |
| Reference scope | Discoverability for current OrchBee actions, not complete Herdr menu parity                                                                                                                                                                                         |

`MenuTarget::Project` carries a `WorkbenchProjectId`, never a label or row index.
Admission freezes the complete canonical Project and, only when its ID matches the
current Workspace's Project, that Workspace/Directory source. Empty Projects and
Projects outside the current Workspace remain valid menu targets with disabled actions.
The same Project and optional context proof must survive confirmation and handoff;
an origin change or a replaced record cancels the episode rather than retargeting it.

All object-entry adapters pass the successfully painted `DrawnMenuSource` Site and
revision. The sidebar owner also checks the current machine and stable focused row.
Right-click, the row's visible action entry and an object-menu key use that same target.
Object keys belong only to explicit sidebar navigation; terminal-owned keys retain
passthrough. The caller retains sidebar focus while the menu captures input and
reconciles a removed row on dismissal. The menu keeps no second sidebar focus state.
The global menu remains independently available.

| Project context at opening                       | Availability and effect                                                                             |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| Current Workspace belongs to the exact Project   | Existing actions retain their own prerequisites; confirmation offers one action without navigation  |
| Standalone, a different Project, or no Workspace | All entries explain the missing context; keyboard and pointer activation cannot confirm or navigate |
| Project or captured Workspace/Directory replaced | Cancel before confirmation or handoff; never substitute another child                               |
| Cancel                                           | Preserve the native origin and caller-owned sidebar focus                                           |

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

`formal/tla/NativeActionMenus.tla` checks finite safety for two episodes and targets,
geometry invalidation, owned/foreign navigation, pre-existing capture and packet boundaries.
No fairness or unbounded progress is claimed. Core tests check draw authority,
selection boundaries, repeated activation, cancellation and generated event traces
with replay and failure shrinking.
`BrowseTimePasses` stutters in the open phase; `ExecutionExpires` cancels only
confirmed, transferring or offered episodes. Injected-clock cases distinguish
time spent browsing from the single execution deadline. This abstraction models
expiry eligibility rather than wall-clock duration or carrier scheduling.
Native integration tests map owned generations to accepted membership callbacks,
and input/render traces map capture and geometry to the observable TUI.
The `project` row class refines the captured current-Workspace prerequisite:
`OpenProjectCurrent` admits that context, while `OpenProjectOther` represents an empty
or noncurrent Project. Project rows cannot use any `Admit` transition;
`NoProjectNavigation` checks that exclusion. They reuse `AlreadyExact`, disabled
activation, source invalidation and one-shot handoff. The abstraction does not model
Project labels, directories or membership records; deterministic tests check those proofs.

| Owned claim                                                        | Abstract mapping                                                                                            | Executable evidence                                                                                                          | Limits                                                                         |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| A disabled action cannot navigate or transfer                      | `available` guards Confirm/Handoff; DisabledActivation stutters; availability changes revoke draw authority | Menu core generated traces and disabled keyboard/pointer activation tests; NativeUi directory and pointer prerequisite tests | Two episodes and targets; known owner refusals only                            |
| Sidebar right-click preserves focus and endpoint identity          | Existing Open origin/target freeze and source invalidation                                                  | Painted sidebar pointer traces for current/foreign machine, Site mismatch, missing row and cancellation                      | Windows native sidebar; no switch-on-open                                      |
| A refusal remains understandable in a short menu                   | Selected unavailable reason is a projection of the same admission value                                     | en-US/zh-CN render traces at full, compact and minimum geometry; actual Windows Terminal screenshots                         | Screenshots provide visual discovery and acceptance, not model refinement      |
| A Project menu never guesses a Workspace or retargets an action    | Project open classes, `NoProjectNavigation`, `CorrectTarget` and `NoDisabledTransfer`                       | NativeUi Project menu traces in `action_menus::tests::project`                                                               | Two abstract targets; native canonical Project and Workspace/Directory records |
| Missing Project context remains readable without an execution hint | Disabled row projection                                                                                     | `project_menu_disabled_reason_renders_in_both_locales_at_small_geometry`                                                     | en-US/zh-CN; native and RGB themes; standard through unpresentable sizes       |

Render checks cover en-US/zh-CN, terminal-native/RGB themes, wide labels and standard,
compact, minimal and unpresentable sizes. Native Windows ConPTY checks exercise the
main path without opening or focusing a user Desktop window. Real Windows Terminal,
provider-specific sessions and the full historical model suite are separate evidence.

The existing full repository gate remains required for integration. Menu tests do not
replace the original action, confirmation, Worktree or clipboard evidence.
