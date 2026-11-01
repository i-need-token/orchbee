# Native action menus

Menu text consumes `MenuLabel` and captured scope counts. The action owner projects these without forwarding launch profiles, concrete target IDs, draw proofs or captured requests; only geometric direction and selected color are needed by the shared label vocabulary. Profile display passes its optional saved name, while the original executable profile remains with its action.

## Interaction

Host capability admission is independent of Server availability. Optional OS effects without an
adapter are absent from menus; direct invocation rechecks the same capability and reports a
localized unsupported outcome. Shared Workbench actions retain their own admission rules. See
[TUI platform boundaries](tui-platforms.md).

`Ctrl+B F2` and `[keys].menu` open the global action menu. These keyboard routes remain
available without a permanent Actions button. Unmodified right-click opens a menu for a drawn
pane, Tab, Project or Workspace row; an Agent row has no menu. Opening or cancelling does not change focus. Pane content retains its configured
`right_click = "panel"` policy; pane chrome remains Client-owned. Shift gestures retain
the host-terminal bypass. Machines retains `Ctrl+B Shift+M`.
The sidebar footer opens Settings directly; no permanent Menu or Machines footer remains.
The Workspace sidebar admits right-click only for an exact, still-present row on the
current machine and painted Session. It uses the existing Workspace menu and never
requests a machine or context switch on opening or cancellation.

A context menu's frame title names its target; a Tab menu uses the Tab's shared displayed name. A row that acts on that target alone shows only its verb in both locales, such as Rename, Close, Dissolve, Pin or Zoom; Close drops its ellipsis with the noun. Rows that name another object or a counted set keep their full text. The global menu, dialog titles, tooltips and shortcut help keep each action's full name, which the prose of these documents also uses. Each row kind has one shared menu. A Project menu holds New Workspace ▸ and Rename, then a separated Dissolve. A Workspace menu holds Move to Project ▸, Rename and, while the Workspace has no directory, Set Directory…, then a separated New Worktree… and Open Worktree ▸, then a separated Close. A reason row exists only while some row of the menu is unavailable.

A pointer-opened menu follows the press: its frame starts at the pressed column on the row below the complete painted trigger (a sidebar row, Tab or group header, otherwise the pressed cell), else ends on the row above it, and shifts left to stay inside the viewport. Height yields to scrolling before covering the trigger row; fewer than three rows on both sides keeps the existing recovery surface. A keyboard-opened sidebar row menu opens beside its row, top edge level with it, moving up only to fit; other keyboard menus stay centered. While a pointer-opened row menu is open, its row keeps the hover fill; a keyboard-opened one keeps the sidebar focus on its row. Placement changes no target or focus. `docs/tui/layout.md` owns the general geometry.

Object and creation context menus omit header X controls, footer hints and selection counters, and publish no hidden hit targets for those omitted controls. A context submenu cascades beside its parent as an untitled frame: Right or Enter on a submenu row, or a click on it, opens the child level with that row. The parent stays painted without input; its opening row keeps the selection fill without the marker. Left or Esc in the child, or a click anywhere on the painted parent, returns to the parent with the opening row as its candidate; a click on another enabled parent row returns with that row as the candidate instead. Esc in the root and an outside click close the whole menu. A pointer that rests 200 ms on a submenu row opens it as a click would; on a row still waiting for discovery the rest holds and opens it when discovery enables the row, without another movement. While a child is open, a pointer that stays still for 300 ms over another enabled parent row returns there as that row's click does; every movement restarts the wait, so a diagonal path toward the child may cross parent rows, and reaching the child, the opening row or the frame cancels it. Esc, outside dismissal and returning from a submenu retain the existing owner and draw guards. The global menu's new-Tab submenu still replaces its parent and carries the query, and the global menu keeps its title and discovery controls.

Current Client support: these row compositions, cascading submenus and press-anchored placement exist only in the native TUI. The GUI sidebar and Tab strip menus in `packages/workbench` keep their earlier compositions until they adopt this contract.

The presentation decision consumes the root target, retained parent chain, exact menu token,
viewport, locale, theme and owner-derived unavailable reasons. It changes no action identity,
selection, capture or execution boundary. A refused frame publishes no row or control authority;
successful context frames expose only their actual content, controls and surface. The render
witnesses in `crates/workbench-client/src/tui_render/action_menu/tests.rs` compare context and global composition, both locales and
shipped theme families, and the width/height boundaries with and without a reserved reason.

A Tab menu holds Rename and Pin or Unpin, then Add to group ▸ and, while a target Tab is grouped,
Remove from group, then the red close rows: Resume closing… while its interrupted Close can resume,
Close and, when it would close something, Close N other Tabs…. While the target belongs to a
selected set, these rows name its count instead, as in Pin selected 2 Tabs.
Add to group ▸ lists the other groups in strip order, then a separated New group…. A group
header's menu, titled with the group's name and Tab count, holds New Tab in group, Rename and
Group color ▸, then Ungroup and the red Close; a click on the header already folds the group
and dragging moves it. Right-clicking the new-Tab button lists the saved launch profiles, the
default first and marked Default; with a single profile it opens no menu, because a click already
creates a Tab with the default. These menus open through the
[group configuration owner](native-tab-groups.md#pointer-arrangement-and-menu-navigation).
A pane menu, titled with the panel's name, holds Split right and Split down, Zoom only beside
another panel and Take control only while another Client holds that panel's size, then the red
Close; dragging a divider resizes, and keyboard resize and copy mode keep their keys and
global menu rows. The global menu also exposes the new-Tab submenu for keyboard users.
The global menu and its new-Tab submenu include a query row. Printable characters and Space edit the
query directly; Backspace removes the previous grapheme and Ctrl+U clears it. The query is limited
to 128 UTF-8 bytes, and menu capture keeps typed characters out of the terminal program. Matching
uses a case-insensitive fuzzy subsequence over localized action labels, reviewed localized semantic
aliases and the current effective shortcut. A matching child action makes its parent submenu
searchable; opening it carries the query into the ordinary child menu, where the child actions are
filtered under their existing ownership and availability rules. The query row shows the filtered
count, and an empty result set shows a localized no-results state.
The new-Tab submenu retains quick default creation and also lists the confirmed device launch
profiles, including System default. Each saved choice names its profile and creates one Tab with
that profile's literal executable and arguments; it never changes the shared default. System shell
uses the native system-shell resolver only after the current origin establishes a direct local
Session, independently of the configured default profile. A tunneled or unknown origin refuses
that one-time system-shell choice; ordinary quick default creation still resolves on the executing
Worker. Custom choices retain their literal paths and arguments. The chosen
name appears in the creation notice. Removed, changed, invalid or unsupported choices refuse with
a Settings recovery notice, without falling back or creating a replacement. Profile choices in an
empty compatibility slot require selecting or creating a Workspace first. Esc and the existing
parent-return, scrolling, narrow-layout and draw-authority rules apply unchanged. The local adapter
matches Desktop's saved-profile semantics; command catalog IDs and CLI `--profile` retain their
separate existing catalog authority. Profile resolution, immutable-default and menu/render witnesses
live in `crates/workbench-client/src/workbench_client/ui/launch_profiles/tests.rs` and the adjacent menu suites.
Close Workspace…, Remove Worktree…, Close panel… and Tab Close/Resume intents declare `ActionTone::Destructive`;
the shared metadata and action/row controls resolve that label through `accent-destructive`.
The Tab Close confirmation uses the same intent tone. Cancel, metadata edits, Dissolve Project,
Ungroup and color choices remain ordinary; a red palette choice is a value, not destructive intent.
These labels are not error statuses. Disabled rows use `text-disabled` and
acquire no pointer state; enabled intent retains the row's independent focus, provisional, hover and held-press cues. The theme
owner supplies all shipped and reduced-color values and any required native contrast pair.
Ctrl+click toggles a Tab's selection mark, and the global menu selects all Tabs or keeps only the
current one; Shift gestures stay with the host terminal, so range selection has no native gesture.
Tab actions configure the captured selected set when their target belongs to it.
[Typed row scope and saved color](#tab-scope-and-current-values)
make the affected set explicit without changing the underlying operation.
Selection or selected-member changes revoke the menu proof; background metadata actions keep
the current terminal attached. Moving the current Tab uses GUI group boundaries, including collapsed neighbors.
Metadata actions preserve the current terminal; a submenu retains its exact addressed Tab.

Recurring menu and prefix actions share one native metadata projection: a stable action identity,
category and reviewed semantic search aliases. Dynamic group color, group index and parameterized
resize input remain typed `ClientAction` or `MenuAction` data rather than becoming row-derived
identities; direction-specific IDs reuse an owning GUI catalog only where that catalog already
defines the same operation. Configured command IDs remain opaque and unchanged. Prefix help obtains
effective chords from the resolved per-Client binding registry, including overrides and shadowing.
A menu row shows the first effective chord only when an explicit `MenuTarget` plus `MenuAction`
mapping has the same dispatch meaning; a current-Tab or current-Workspace binding is never advertised
for a background object merely because both actions share metadata. Menu execution still uses its
typed target, exact coordinator proof and owner-derived availability; metadata and displayed chords
grant no execution authority. Search aliases find only admitted actions in the global menu and its
searchable child menus; they do not expose command identities or hidden configuration commands.

Shortcut Help and the global Detach action share localized wording: only the current Client
detaches, and terminal programs keep running. This differs from closing a Panel, Tab or Workspace;
the existing detach/reconnect behavior and confirmation policy remain unchanged.

Each displayed chord occupies a separate right-aligned slot after the navigation track and localized
label, with at least one blank cell between the label and chord. The chord remains complete;
constrained rows truncate the label first and omit the whole chord only when the remaining row cannot
preserve the marker, a usable label and the gap.

Selected/other/trailing Close and group Close show their frozen affected count, then open one
[complete-set confirmation](native-tab-close-sets.md). Relative Close protects pinned Tabs;
direct selected/group Close includes them where applicable. Hidden collapsed members remain targets.

A resumable Tab-close row appears only after the close owner has presented the exact recovery status.
Opening the menu freezes an opaque permit for that target, operation, attempt and status incarnation.
While the menu replaces the ordinary status chip, availability and the one-shot handoff revalidate that permit
and the exact menu source. They neither keep the covered status marked as displayed nor fall back to
ordinary Close. The resulting Resume confirmation must present its own exact prompt and status before
it accepts confirmation.

In the searchable global menu and new-Tab submenu, Up/Down select rows while printable characters,
including Space, edit the query. In other menus, Up/Down or j/k select a row. Home/End select the
first/last row; Enter executes the selected action or opens its submenu. A menu opened by the
pointer, and a submenu opened by a click, starts without a candidate: no row shows the selection
fill, marker or reason until the first navigation or confirmation key reveals one, without
executing. Up and End reveal the last row; Down, Home, Right and Enter reveal the first. A menu or
submenu opened from the keyboard starts on its first row. Keyboard selection reveals its row with the smallest viewport movement. Once the
host and native adapter receive idle motion under global admission or the scoped menu request,
unmodified, button-free motion
moves the provisional menu candidate to an enabled drawn row, using the full-row selection fill
and navigation marker. It never executes or changes the active Workspace or Tab; only a rest on a
cascaded submenu row opens it, as above. Leaving retains the candidate; subsequent arrows continue from it. Disabled rows,
separators and group headings do not move the candidate. Hover preserves the viewport and action token; Enter
requires a successful draw of the current candidate. Without received idle reports there is no hover feedback. A left click on a
drawn row executes it only after a matching release on the same typed source, token, row and painted
rectangle. Keyboard input, focus loss, capture release or a changed target/rectangle cancels a
pending click. The global menu's header X closes it; its Back action returns to its parent. Right
opens a selected submenu; Left or Esc returns to its parent and restores the stable selected action,
anchor and freshly queried availability. Returning requires a fresh successful paint; old child
tokens cannot execute.

Esc at the root or a click outside cancels. Wheel outside the list is inert. Inside it, each notch
contributes three rows to a bounded, viewport-capped stream without wrapping or executing; wheel
movement changes only the viewport and may leave keyboard selection offscreen. Reversal drops the
previous direction's backlog and retains the first new notch. Separators are noninteractive and
consume layout rows, while the global footer counts filtered actions only. Rows scroll within the menu, leaving the
status line visible. The global menu's fixed footer shows the selected row and filtered total (`current/total`) plus ordered,
atomic key hints styled as key and localized verb. Exit is first; Enter and navigation disappear as
whole items when space runs out. Shared Back or Close actions are shown when they fit. The menu
footer owns routine key discovery, so a normally presented global menu floats no session status.
During geometry recovery, the status chip instead shows one localized, key-free refusal or suspension
notice plus one shared Esc Back or Close hint; it never mixes underlying notice or Machines text.
Menu width includes the borders, one cell of padding on each side (the left one holds the
navigation marker) and one blank cell after the widest row's content, so no label or chord touches
the right border; labels that exceed the available width truncate on grapheme boundaries with the
theme's ellipsis.
Selection fills the complete inner row and uses the shared navigation-marker track; hover uses the
independent shared pointer state. A reason row is reserved only when the menu has unavailable
actions, so hover never changes row geometry. Each frame derives its visible window from the current
row budget. Keyboard navigation keeps the selected action visible, while a manually scrolled
viewport remains where the user placed it. The successful draw commits the exact first action,
visible action count and selected action for pointer and activation admission; a larger frame fills
available rows without changing selection.
A cancellation or owner unavailability outcome appears once after the menu closes; it does not
duplicate the open menu's footer or geometry-recovery status.
A newly requested global menu that cannot present its title, a row and its exit instruction is refused
with a localized notice. A context menu instead requires its border, at least one action row and
any reserved reason row; its minimum complete frame is 16 columns by 3 rows without a reason.
If an admitted menu later becomes too small, it enters `Suspended`, retains
its source, selection, viewport, parent and anchor, and accepts only its current local exit or return.
Restoring sufficient geometry resumes the same menu incarnation after a fresh successful paint.
Ordinary terminal output continues while the menu captures input.
The underlying pane cursor is hidden during capture;
cancelling restores the ordinary pane projection without changing focus.
Known unavailable actions use the full disabled text style, retaining its modifiers
at every color level. They can be selected to read a localized reason; Enter or clicking keeps the menu open and
cannot confirm or navigate. A fixed reason row leaves the global exit instruction and
selection count visible even at the smallest presentable geometry, while a context menu reserves
the reason without adding footer chrome. The global footer omits the Enter action
for a disabled or offscreen selection.
Actions unsupported by the current host are hidden rather than presented as disabled learning
rows; no placeholder or unsupported-host reason row is emitted. Context-relevant
actions whose ordinary prerequisites are unmet remain visible with their owner-derived disabled
reason.

Browsing has no time limit: reading, selection, repainting and disabled activation
do not start an execution timer. Confirming an available action creates one
absolute 30-second deadline. The same deadline bounds target navigation and
handoff; draw and navigation callbacks never renew it. Expiry cancels the
unhanded action without rolling back accepted navigation. Target, origin,
configuration and capture invalidation still cancel browsing. Geometry follows the refusal and
suspension lifecycle above.

Availability projects existing action-owner prerequisites, without a universal
Ready guard. Set Directory… is absent for a Workspace drawn with a directory and
disabled when a directory is configured while the menu is open; for the current Workspace its
original empty-state and setup-admission guards also apply. Released pointer capture disables Release
pointer capture. Unknown asynchronous prerequisites continue through their original
owners; the menu does not infer Git support, worker reachability or an operation
outcome from read-only machine observations.

Project menus expose New Workspace ▸ and Rename Project, then a separated Dissolve Project, shown as Rename and Dissolve. New
Workspace ▸ lists New Worktree…; a Folders group with the directories recorded by the Project's
Workspaces (navigation order, deduplicated case-insensitively, labelled by directory name, or by the
full path when names repeat); and a Git Worktrees group with the discovered non-bare checkouts
(labelled by branch without `refs/heads/`, else by path). Group headings use `text-secondary` and
are never selectable. A folder row creates a Workspace in that directory through the existing
Project quick path of the [setup owner](native-workspace-setup.md#creation-defaults-and-result-ownership),
including empty and noncurrent Projects, without a Project picker. While the Project records no
folder, as an empty Project does, New Workspace leads the submenu and runs the same Project quick
path as the row's create action, so the submenu always holds a usable row. It stays a submenu even
when that is its only enabled row, because each row kind has one shared menu. Rename
and Dissolve also target the drawn Project without entering a child. Dissolve Project removes only
that organization: every member Workspace, terminal, program and identity survives ungrouped, so the
row is not destructive. It never composes Close All with removal.
Explicit Close All compatibility keeps its separate owner. Worktree discovery uses the addressed
derived source in [native-worktrees.md](native-worktrees.md), including for noncurrent Projects;
it never borrows an unrelated current Workspace. Workbench keeps the immutable capture while ClientView receives only its plain intent.
InputDispatcher retains that capture on the current call stack through the view decision and terminal
mode lease, then submits it only for the same accepted action. A refused or rewritten decision cannot
execute an unrelated capture. The original source, baseline, desired state, focus and deadline remain
intact; execution returns the actual disposition to the same view transition.
Setup retains the current source and focus, checks the original menu deadline at admission, and uses
bounded preparation without timing optional human editing.

Workspace menus contain Move to Project ▸, Rename Workspace (shown as Rename), Set Directory… when
it applies, a separated Worktree section and a separated close section. The Worktree section holds
New Worktree…, which creates a Worktree with defaults through the sidebar Create route, and Open
Worktree ▸, whose submenu lists the discovered non-bare checkouts other than the Workspace's own
(labelled like the Git Worktrees group) and opens the chosen one through the sidebar Open route.
Both act on the addressed Workspace without selecting it; their source and grouping follow
[native-worktrees.md](native-worktrees.md), so an ungrouped Workspace and the Worktree it adds land in
one new Project. Move to Project ▸ lists the other Projects in canonical order, then a
separated New Project… and, for a grouped Workspace, Remove from Project. Choices retain exact IDs, including duplicate labels. Names that collide
after sanitization or display-width truncation receive the sidebar's shortest unique trailing ID
suffix: at least eight hexadecimal characters, extended by four up to the full ID. The complete suffix
is budgeted before truncating the name; geometry that cannot show it refuses the surface. New Project asks
for one name and atomically creates the Project and moves the Workspace. Organization never selects
a noncurrent Workspace. Empty conditional creation sections publish no orphan separator.

Opening a Project or Workspace menu starts Git discovery of the addressed source. New Worktree…
and Open Worktree ▸ stay disabled with a localized reason until discovery succeeds; a non-Git or
failed source keeps them disabled, and Open Worktree ▸ also stays disabled while the Workspace's own
checkout is the only one. A directory change of the first terminal keeps the menu open, returns these
rows to waiting and starts discovery over. Success enables them, appends the Git Worktrees group to New Workspace ▸ and, when the
Workspace directory is a linked non-bare checkout, appends a destructive Remove Worktree… after
Close Workspace…. New rows only follow existing ones, so no row above them moves; the reason row
disappears once no row is unavailable, so the frame may shrink. Completion changes availability
under a fresh menu token and requires repaint; it never changes the candidate or the selected
action's identity.

Close Workspace… and Dissolve Project ask for authoritative conditional admission through the existing Close or setup owner.
Only definite `confirmation-required` for that conditional kind and captured revision opens a new
complete-draw confirmation; explicit Enter then uses a distinct unconditional request ID. Missing,
stale, unavailable or unknown evidence never means idle and never escalates automatically.
[Scoped Close](native-workspace-close.md) owns Workspace terminal disposal; Dissolve Project's warning
instead explains preservation. Acting on a background target preserves the unrelated current session.

The sidebar `+` entries use `MenuTarget::Create` for the global or exact-Project creation context.
The Project entry puts Quick Workspace first. Existing Worktree paths and their preceding separator
appear only after source discovery yields openable entries; an empty or unresolved list contributes
neither. This checkout list is independent of the row menu's New Workspace ▸. Successful asynchronous
composition mints a fresh menu token and requires a new complete draw; late replies cannot reopen
cancelled menus or retarget another episode. The [shell layout](tui-shell-layout.md#quick-creation-controls)
owns the entry geometry and direct Tab-strip create gesture.

The global menu groups its existing actions under Tab, Panel, Workspace and Session sections. Tab
includes new-Tab options, New Tab to right, Duplicate Tab, Rename Tab, moving the current Tab left,
right, to the start or to the end, Select all Tabs, Keep only current Tab selected and Close Tabs
to the right…; these act on the current Tab and name the selected set where it applies. Panel
includes directional split and move, close, zoom, keyboard resize and copy mode. Workspace includes
creation and Worktree actions; Session includes Detach. A row whose keyboard action has the same
meaning shows the current binding registry's shortcut and dispatches that same existing
ClientAction. Pane and Tab actions require a focused pane and show the localized
focused-panel-required reason when none is available; Close Tabs to the right… shows the
no-matching-Tabs reason when nothing would close. Other global actions
follow these sections. Ordinary confirmed Close retains its compatibility behavior; sidebar
conditional Delete follows the authority admission above. Settings delegates to its existing editor;
checkout deletion, policy editing and custom or plugin-defined menus are outside this surface.

When the carrier is unavailable, the global menu remains independently available; pane and
Tab actions show the focused-panel-required reason. Member menus require a live
carrier. An observed geometry is required even before the first session state arrives.

## Tab scope and current values

Across these menus, titles, drag hints, Close and Rename tasks, English user copy uses **Tab** and
Chinese uses **标签页**; a group remains **group** / **分组**. Localization changes no semantic action
or target identity.

The Tab menu's frame title names the addressed Tab with the strip's shared display name; a group
menu's title adds the group's Tab count. The group name comes first: a title too wide for the frame
drops the whole “ · Group (N Tabs)” count, and only a name that still overflows truncates at its
end. Each row
states its own scope, including the selected-set count where it applies: selection marks elsewhere
never imply that every action uses them. Its submenus cascade as untitled frames beside their
parent; no scope header, Back button, generic X, footer or selection counter is painted. Left/Esc
or a click on the parent restore the retained parent selection and require a fresh draw, without
writing a color or changing terminal focus.

| Action                                                     | Captured scope                                                                                                                  |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Rename, create to right, move one Tab                      | Addressed Tab, even when other Tabs are selected                                                                                |
| Duplicate, pin/unpin, assign/remove group, direct Close    | Selected canonical set if the target belongs to it; otherwise the addressed Tab                                                 |
| New Tab in group, group rename/color, ungroup, Close group | Entire group containing the addressed Tab, independently of selection marks                                                     |
| Other/trailing Close                                       | Existing close resolver's nonempty set; pinned and selected-set protection and the rightmost-selected boundary remain unchanged |
| Select all or keep only current selected                   | Current Workspace's Client-local marks                                                                                          |

Single-target rows say **Pin** or **Close** in a Tab menu and **Pin Tab** or **Close Tab** in the
global menu; multi-target rows include the selected count.
The global menu's Tab rows address the current Tab. Close N other Tabs… disappears when its exact
set is empty, while the global Close Tabs to the right… stays with its reason. Relevant owner-derived disabled
reasons remain visible. Counts are `NonZeroUsize` projection facts, never parsed labels or command
targets. The existing coordinator captures and revalidates source, member/group metadata, marks,
focus and action availability before any handoff.
The private Tab target also captures the ordered canonical Tab IDs, pin states and group IDs used
by the scope resolvers. Input, offer and handoff revalidate these exact facts. An append, removal,
same-count ID replacement, reorder, pin or group-membership change retires that menu; reopening
requires a new token and successful draw. An unchanged resolver source survives a revision-only
refresh. This presentation fence does not replace the existing fresh complete-set Close decision.

The color page initially addresses the saved canonical color. Its checked track stays on that
applied color while arrows or received hover move the separate provisional navigation track.
Every option also paints a paired palette chip and localized name; the checked/name tracks remain
meaningful in monochrome. Only explicit activation after an exact successful draw can submit the
existing group update. Browsing, Back, cancellation and reopening never write or replay a color.
Changed scope or checked facts revoke the old row proof without resetting the candidate or
renewing an action deadline. The parent still requires a fresh successful draw after return.

The GUI's group editor already paints checked color radios and palette previews. Its radio keys
apply a value immediately; native menu navigation remains provisional until activation. Both
use canonical group color and the existing update owner; this is not a second shared color model.
The implementation oracles cover exact cross-group selected/unselected scopes, pinned relative
Close, zero-target filtering, scope/checked proof invalidation, saved-color cancellation and
committed reopen. Render assertions cover both locales, narrow/wide cell geometry, independent
checked/candidate/palette tracks, cascaded child placement and destructive row/button
adoption. They do not establish executed-test results or physical host appearance.

## Model and ownership

The user wants to discover and execute existing actions without memorizing chords.

| Record field    | Contract                                                                                                                                                                                                                                                                                                                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs          | Drawn stable target, exact context/member/incarnation, projected stable action identity and typed input, current action prerequisites, complete decoded input packet, configuration, monotonic deadline and successful menu draw receipt                                                                                                                                            |
| Data            | One local episode, `Pending`/`Admitted`/`Suspended` presentation phase, independent whole-surface lease and exact row-window proof, `Recovery`/`Continuation`/`ActivationReady` input readiness, retained viewport and selection, owner-derived enabled/disabled/hidden availability, presentation token, frozen confirmed action, parent snapshot and optional exact target member |
| Outputs         | Local capture and drawing; bounded navigation through existing membership owners; at most one action handed to the full ClientView/dispatcher path; localized cancellation or refusal                                                                                                                                                                                               |
| Decisions       | Existing captures, including the native navigator, take priority. Help and sidebar remain unchanged underlays. Choosing an action admits navigation; opening does not. Each action retains its original availability policy rather than a universal Ready requirement                                                                                                               |
| Invariants      | No undrawn or stale activation, no focus on open/cancel, no action on a replacement target, no implicit retry, no input or decision-packet tail reaching a pane; ordinary Close keeps its confirmation; conditional Delete requires exact authority proof or a new reviewed decision                                                                                                |
| Model           | Finite safety machine for capture, drawing, target invalidation, owned and foreign navigation, cancellation, prerequisites and one action transfer                                                                                                                                                                                                                                  |
| Dependencies    | Existing native membership, terminal state, input decoder, hit map, layout, localization and action owners; additive conditional Delete wire actions remain Server-owned; no new dependency or persistent data                                                                                                                                                                      |
| Ownership       | WorkbenchClientUi owns menu admission and target navigation; the deterministic menu owns selection and draw authority; ClientView owns local view decisions; InputDispatcher alone applies shell, mode and layout effects; original operation owners alone execute mutations                                                                                                        |
| Reference scope | Discoverability for current Orchbee actions                                                                                                                                                                                                                                                                                                                                         |

`MenuTarget::Project` carries a `WorkbenchProjectId`, never a label or row index.
Admission freezes the complete canonical Project and, only when its ID matches the
current Workspace's Project, that Workspace/Directory source. Empty Projects and
Projects outside the current Workspace remain valid menu targets: their folder rows create a
Workspace when the setup owner admits it, as do Project rename and organization-only Dissolve.
Worktree actions separately freeze the derived addressed member and its complete source proof.
The same Project and optional context proof must survive confirmation and handoff;
an origin change or a replaced record cancels the episode rather than retargeting it.

All object-entry adapters pass the successfully painted `DrawnMenuSource` Session and
revision. The sidebar owner also checks the current machine and stable focused row.
Right-click, the row's visible action entry and an object-menu key use that same target.
Object keys belong only to explicit sidebar navigation; terminal-owned keys retain
passthrough. The caller retains sidebar focus while the menu captures input and
reconciles a removed row on dismissal. The menu keeps no second sidebar focus state.
The global menu remains independently available.

The [native navigator](native-context-selection.md#capturing-navigator)
consumes menu shortcuts and pointer entries while it owns input. Close it before
opening a menu; a menu may still open the navigator through its ordinary action.

| Project context at opening                                   | Availability and effect                                                                                                                                        |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Current Workspace belongs to the exact Project               | Existing actions retain their own prerequisites; confirmation offers one action without navigation                                                             |
| An ungrouped Workspace, a different Project, or no Workspace | New Workspace ▸, rename and organization-only Dissolve target the exact Project; New Worktree… and Git Worktrees need its admitted derived first-member source |
| Project or captured Workspace/Directory replaced             | Cancel before confirmation or handoff; never substitute another child                                                                                          |
| Cancel                                                       | Preserve the native origin and caller-owned sidebar focus                                                                                                      |

## Transitions and cancellation

| State / event                                    | Guard and result                                                                                                                                                        |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hidden → Pending                                 | Stable drawn source, no genuine capture or pending gesture; freeze target and current origin, capture opening packet                                                    |
| Pending → Admitted                               | A complete draw reports the exact token, first action, visible action count and selected action                                                                         |
| Pending → geometry blocked                       | Refuse the new surface, show one localized notice and transfer no action                                                                                                |
| Admitted → Suspended                             | A later evaluated geometry block revokes draw authority but retains source, selection, viewport, parent and anchor                                                      |
| Suspended → Admitted                             | The same incarnation completes a fresh exact draw                                                                                                                       |
| Admitted → keyboard select                       | Move selection, enable cursor-follow, mint a token and require redraw                                                                                                   |
| Admitted → wheel viewport                        | Move only the retained viewport, disable cursor-follow, mint a token and require redraw; selection is unchanged                                                         |
| Admitted → same-surface revision                 | Preserve the whole-surface lease, revoke only exact row authority and publish `Continuation` while selection, viewport, labels or availability await a matching redraw  |
| Admitted → covered or not painted                | Mint a new token, revoke both the exact row proof and whole-surface lease, and retain ownership as `Recovery` until a fresh complete draw                               |
| Open → availability change                       | Refresh owner-derived reasons and revoke the previous token until redrawn; a known unavailable row cannot authorize confirmation                                        |
| Open → disabled activation                       | Keep selection and capture, reveal the reason, and transfer no action or navigation                                                                                     |
| Open → time passage                              | Retain browsing and capture without creating an execution deadline                                                                                                      |
| Admitted → confirm                               | Complete input, exact current draw and an available selected row; freeze one action and prerequisite state, mint one absolute 30-second execution deadline              |
| Confirmed → navigation                           | Distinguish AlreadyExact, owned Context generation, owned Tab generation and admitted local focus; never infer admission from a no-op                                   |
| Navigation → handoff                             | Only the owned prepare/apply result authorizes advancement; exact target still exists, source and prerequisites match; the original action decides availability         |
| Handoff                                          | Remove the one-shot intent before applying the full ClientView transition and dispatcher effects; no retries                                                            |
| Cancel / execution timeout / unavailable         | Revoke the unhanded intent; accepted navigation is not rolled back, and no action executes                                                                              |
| Source / configuration / covering capture change | Revoke the unhanded episode; a foreign navigation retains its own semantics and cannot complete the menu intent. Resize follows the retained presentation states above. |

Presentation tokens bind the episode, exact semantic target, keyboard selection and viewport revision. The
whole-surface lease separately proves that this owner occupied the latest frame; the successful
receipt additionally records the exact painted window. `Recovery` accepts only local Esc, including
Esc returning from a retained child. `Continuation` admits repeated navigation while a normal
same-surface revision awaits exact redraw. `ActivationReady` additionally requires the current
selected action inside that exact window. None of these proofs binds ordinary pane output
sequences. Output may repaint the underlay without cancelling a menu. A new incarnation, removed
target, changed structural source or conflicting capture revokes the episode. Resize immediately
revokes both presentation proofs; an evaluated size failure then refuses `Pending` or suspends an
already admitted incarnation.
The renderer completes geometry for every cascade frame, row and required-exit-hint admission before
mutating the candidate buffer or hit map. `NotPresented` paints nothing and publishes neither menu targets nor a modal
pointer backdrop.

Hover updates only the Client-local provisional menu candidate. Only an exact enabled typed row
from the last published frame may do so; the coordinator revalidates source plus token before
delegating to the menu owner. The exact painted row-window proof remains available for successive
motion reports in one packet, while keyboard confirmation requires its selected row to match the
current candidate. Hover preserves viewport, token, availability, native navigation and execution
deadline; stationary-pointer reconciliation never moves the candidate. Matching release emits the
typed activation intent and revalidates its source and token. Keyboard selection and availability
changes mint a new token and require a new draw. The pointer owns one rest timer for submenu
opening and parent return; when it fires, the coordinator revalidates the painted source and token
and the menu owner opens only a cascaded submenu row, through the same confirmation as a click. The
global menu's replacing new-Tab submenu still opens only on a click or key.
Pane reconciliation cancels underlying gestures while preserving the top ActionMenu owner's
painted hover and press across idle and draw boundaries. A matching release still requires the
same target, token, exposure and geometry; a covering owner or invalidated hit map revokes it.

Global pointer capability remains Session-owned. After the lifecycle completely establishes
global `ButtonOnly` (`1003l`, then `1002h` and `1006h`), the endpoint performs one ordered
`DECRQM ?1003$p` probe with a DA1 sentinel. Only an exact Reset reply (`DECRPM ?1003;2$y`) before
the deadline admits `AnyMotion`; Set, unavailable, DA1-first, a 300 ms timeout, late and duplicate
outcomes never publish global admission and never trigger a second probe. `Unknown` and `Unavailable` remain
globally `ButtonOnly`, where click, drag and wheel continue and idle hover is absent. An admitted
upgrade writes `1002l` before `1003h` and publishes `AnyMotion` only after the complete mode write
succeeds. Release and restoration clear `1003`, `1006` and `1002`; re-arm reuses the settled
capability. While capture is already armed, an action menu without global admission requests
`MenuMotion`: `1002h`, then `1003h`, retaining the button fallback and `1006`. The request admits
actual received motion only for the capturing menu and does not change the global probe result.
Every close, handoff or source invalidation restores `1003l`, then `1002h`; an independently admitted
global upgrade instead remains active. Failed mode writes publish no lease and require full
lifecycle cleanup. Released capture stays released even when the keyboard opens a menu. Menu capture swallows
motion and decision tails instead of sending them to an embedded program; pane tracking and any
pre-existing gesture retain their original ownership outside it.

Object-entry admission checks the Session and Workbench revision of the successfully painted
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

The owning implementation tests cover shared menu/help identity, opaque configured-command IDs,
effective binding discovery, hidden unsupported actions, draw authority, packet tails, generated
event replay, cancellation, source/target/focus drift, exact draft capture, at-most-once handoff,
known-commit retention and execution deadlines independent of browsing time. Candidate-hover
witnesses cover successive pre-paint motions, keyboard continuation, disabled and unpainted rows,
source and token refusal, viewport retention, pointer leave, and no implicit action. Rest witnesses
cover the open delay and restarted return grace, parent-row return targets beside the opening row,
and that a rest opens only a cascaded submenu row and never the global new-Tab submenu.

Global-menu witnesses assert the Tab, Panel, Workspace and Session section boundaries, directional
split and move actions, both localized labels, current effective shortcuts, parity with keyboard
`ClientAction` dispatch, and the localized focused-panel-required reason when no pane is focused.
Search witnesses cover effective-shortcut and localized-alias matching, nested new-Tab child hits,
filtered identity and availability, grapheme Backspace, Ctrl+U clearing, query bounds and localized
no-results presentation.

Maintain the concrete witnesses in `crates/workbench-client/src/action_menu/tests.rs`, `crates/workbench-client/src/workbench_client/ui/action_menus/tests.rs`, `crates/workbench-client/src/terminal_input/dispatcher/tests/action_menu.rs`, `crates/workbench-client/src/tui_render/tests.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

`crates/workbench-client/src/workbench_client/ui/action_menus/sidebar/tests.rs` covers asynchronous token replacement,
cancelled episodes and keyboard cascade navigation. `crates/workbench-client/src/tui_render/action_menu/tests/placement.rs`
covers press and row anchoring, cascade sides, the painted parent and the hidden candidate;
`crates/workbench-client/src/tui_render/action_menu/tests/sidebar.rs` covers complete duplicate/truncated Project suffixes in both locales.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.

An anchored sidebar menu retains the full covered row rectangle for placement while suppressing its underlying input targets.
