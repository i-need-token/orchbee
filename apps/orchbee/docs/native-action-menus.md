# Native action menus

Menu text consumes `MenuLabel` and captured scope counts. The action owner projects these without forwarding launch profiles, concrete target IDs, draw proofs or captured requests; only geometric direction and selected color are needed by the shared label vocabulary. Profile display passes its optional saved name, while the original executable profile remains with its action.

## Interaction

Host capability admission is independent of Server availability. Optional OS effects without an
adapter are absent from menus; direct invocation rechecks the same capability and reports a
localized unsupported outcome. Shared Workbench actions retain their own admission rules. See
[TUI platform boundaries](tui-platforms.md).

`Ctrl+b Space`, `Ctrl+b ?` and `[keys].menu` open the global action menu. These keyboard routes remain
available without a permanent Actions button. There is no separate shortcut-help overlay: the
global menu is where keys are found, and `Ctrl+b ?` is the same action under the key people try
first when they look for help. Its query row takes typing at once, so `Ctrl+b ?` followed by a word
searches. Unmodified right-click opens a menu for a drawn
pane, Tab, Project, Workspace or Agent row. Opening or cancelling does not change focus. A right
press never reaches the pane's program, even one that reports mouse tracking. Shift gestures retain
the host-terminal bypass. Machines retains `Ctrl+b Shift+M`.
The sidebar footer opens Settings directly; no permanent Menu or Machines footer remains.
The Panel menu offers Rename after terminal commands, using the primary Presentation's shared override; the global
Panel group exposes the same operation as Rename Panel. Empty input restores the original name.
Both entry points use the [Panel label editor](native-panel-labels.md) without a new default key.
Clear terminal precedes Rename in the Panel menu and is also in the global Panel group. Its
English and Chinese labels match the GUI terminal viewport command, which precedes the GUI
Panel section. It runs without a confirmation, as the CLI and GUI do: it invokes the existing
[exact-terminal clear](native-cli-query.md#clear-terminal-output) on the displayed primary or
child terminal, sends no Shell input and changes no process, file or controller. It adds no
clear-history option or new default key. The bounded FIFO carries the exact admitted viewer
incarnation once; stale work is dropped without retargeting. The status distinguishes complete,
cleanup-unconfirmed, refused, unavailable and unknown outcomes, and unknown delivery is never
retried. The clear needs this Client's attachment to the focused terminal, not control of it, so a
view-only Panel still clears. While the focused terminal waits to reattach after its attachment was
lost, the row shows the GUI's Terminal disconnected reason, and the request refuses by the same rule;
a refusal only the Server can answer, such as a full-screen program, stays a status after the attempt.
Rename continues to address the primary Presentation independently of the displayed child.
The global Panel group also holds Back to last focused panel, the [Last Panel](native-context-selection.md#last-panel) step (`Ctrl+b ;`), which shares the GUI
Workbench identity `workbench.panel.focus-last`.
The Workspace sidebar admits right-click only for an exact, still-present row on the
current machine and painted Session. It uses the existing Workspace menu and never
requests a machine or context switch on opening or cancellation.

An Agent row's menu holds Rename, which only a Panel's primary terminal offers, Allow confirmations where the Server answers them for that Agent, then a separated Close. Rename and Close name the row's Panel rather than its terminal: Rename edits the primary Presentation's shared override as the Panel menu does, and Close ends the whole Panel, asking first when one of its terminals runs something. The Panel actions act only on the focused Panel, so the menu first focuses the Agent's Panel, as a click on the row does, and offers the action once that Panel holds focus. An Agent in a child terminal has no Rename row.

Allow confirmations opens beside the row without moving focus and lists Off, This Agent only and Always for the Agent's type, with a check on the choice the Server reports; the check moves when the Server's row does. `Ctrl+b y` and the global menu's Allow confirmations row open the same answers under an "Allow confirmations" heading for the Agent in view: the focused Panel's, else another the active Tab shows. While the Server offers to answer that Agent's confirmations, they list This Agent only, Always for the Agent's type and No instead, with no check; No ends the offers for every Agent. The key has no `[keys]` name, the global row shows only while there is such an Agent, and a menu whose Agent's row leaves closes. The rest of a pasted packet after `Ctrl+b y` reaches no terminal.

A context menu's frame title names its target, and a `+` creation menu names what it creates; a Tab menu uses the Tab's shared displayed name, and a Workspace menu follows the Workspace's name as it changes under the open menu. A row that acts on that target alone shows only its verb in both locales, such as Rename, Close, Ungroup, Pin or Maximize / Restore; Close drops its ellipsis with the noun. Rows that name another object or a counted set keep their full text. The global menu, dialog titles and tooltips keep each action's full name, which the prose of these documents also uses. Each row kind has one shared menu. A Project menu holds New Workspace ▸ and Rename, then a separated Ungroup. A Workspace menu holds Move to Project ▸, Change machine ▸ where the TUI reaches saved machines, Rename and, while the Workspace has no directory, Set Directory…, then a separated New Worktree… and Open Worktree ▸, unless discovery shows its first terminal is outside Git, then a separated Close. Availability reasons take the frame's bottom edge; a Git refusal removes Git-only rows under a fresh draw token.

A pointer-opened menu follows the press: its frame starts at the pressed column on the row below the complete painted trigger (a sidebar row, Tab or group header, otherwise the pressed cell), else ends on the row above it, and shifts left to stay inside the viewport. Height yields to scrolling before covering the trigger row; fewer than three rows on both sides keeps the existing recovery surface. A keyboard-opened sidebar row menu opens beside its row, top edge level with it, moving up only to fit; other keyboard menus stay centered. While a pointer-opened row menu is open, its row keeps the hover fill; a keyboard-opened one keeps the sidebar focus on its row. Placement changes no target or focus. `docs/tui/layout.md` owns the general geometry.

Object and creation context menus omit header X controls, footer hints and selection counters, and publish no hidden hit targets for those omitted controls. A context submenu cascades beside its parent as an untitled frame: Right or Enter on a submenu row, or a click on it, opens the child level with that row. The parent stays painted without input; its opening row keeps the selection fill without the marker. Left or Esc in the child, or a click anywhere on the painted parent, returns to the parent with the opening row as its candidate; a click on another enabled parent row returns with that row as the candidate instead. Esc in the root and an outside click close the whole menu. A pointer that rests 200 ms on a submenu row opens it as a click would; on a row still waiting for discovery the rest holds and opens it when discovery enables the row, without another movement. While a child is open, a pointer that stays still for 300 ms over another enabled parent row returns there as that row's click does; every movement restarts the wait, so a diagonal path toward the child may cross parent rows, and reaching the child, the opening row or the frame cancels it. Esc, outside dismissal and returning from a submenu retain the existing owner and draw guards. The global menu's new-Tab submenu still replaces its parent and carries the query, and the global menu keeps its title and discovery controls.

GUI Clients take these compositions: the sidebar's Project and Workspace row menus and the panel menus in `apps/web`, and the Tab, group header and New Tab menus in `packages/tab-strip`. A Web or Desktop window opens no popup terminal yet and withholds popup configured commands. GUI menus carry no frame title, because each menu's accessible name already names its target, and they open at the press and cascade with the platform's context-menu behavior rather than the TUI's cell geometry and timings. The New Tab button names an unnamed profile as its Settings page does. Without a Workspace it opens no menu rather than disabled rows, and a profile the window cannot start, such as a system shell other than the default, refuses with the Settings recovery notice when chosen. A GUI pane menu opens from the terminal viewport, below the terminal's own commands such as Copy and Paste, and from the panel header's options button and right-click; the header's menu has no Type here row. It says Maximize or Restore, by the panel's state, where the TUI row reads Maximize / Restore, and has no GUI-only row. Its rows show their shortcuts, because opening a pane menu focuses its panel.

The presentation decision consumes the root target, retained parent chain, exact menu token,
viewport, locale, theme and owner-derived unavailable reasons. It changes no action identity,
selection, capture or execution boundary. A refused frame publishes no row or control authority;
successful context frames expose only their actual content, controls and surface.

A Tab menu holds Rename and Pin or Unpin, then Add to group ▸ and, while a target Tab is grouped,
Remove from group, then the red close rows: Close and, when it would close something, Close N other
Tabs…. While the target belongs to a
selected set, these rows name its count instead, as in Pin selected 2 Tabs.
Add to group ▸ lists the other groups in strip order, then a separated New group…. A group
header's menu, titled with the group's name and Tab count, holds New Tab in group, Rename and
Group color ▸, then Ungroup and the red Close; a click on the header already folds the group
and dragging moves it. Right-clicking the new-Tab button, or a click on the `[v]` beside it, lists
the saved launch profiles, the default first and marked Default, then a separated Agents section
under its heading: every Agent the picker lists as Recent or Installed, in its order, by product
name. `[v]` is painted only while the menu holds more than one row and the strip has room for it. An
Agent row starts that Agent in a new Tab of the current Workspace without a question, through the
[Start Agent picker](start-agent-picker.md)'s new-Tab place. The rows come from the Session's Agent catalog as already held:
opening the menu never asks the Server to look again, and the rows are fixed when the menu opens,
so a catalog that arrives later reorders nothing. While a lookup runs the section lists what is
known, and a provider still being checked or not found is never listed. Where the picker could not
start an Agent in a new Tab — no Agent launch on this host, a Session that cannot start Agents, no
catalog, or no current Workspace — the section is absent and the profiles stand alone. With a single profile and no section it opens no
menu, because a click already creates a Tab with the default. These menus open through the
[group configuration owner](native-tab-groups.md#pointer-arrangement-and-menu-navigation).
A pane menu, titled with the panel's name, leads with Copy for the focused panel, then holds
Find in terminal, Clear terminal, Rename, Split right and Split down, Maximize / Restore only beside another panel and Type here only while another window
holds that panel's size, then the red Close; dragging a divider resizes, keyboard resizing keeps its
keys, and copy mode keeps its key and global menu row. The global menu also exposes the new-Tab submenu for keyboard
users.

Copy (`复制`) is a pane-only row whose label and catalog identity (`terminal.selection.copy`) match the
GUI terminal menu. It starts one ordinary selection export on the copy-on-select clipboard path. It takes
no copy-mode ownership, keeps the Host selection, adds no clipboard write of its own, reports success
silently, and reports failure with the ordinary copy notice. The row is enabled while the focused panel's
selection is settled and idle. While a gesture or export is pending it is disabled as busy, and with no
selection it stays visible and disabled with the reason `Select text first`. A panel that is
not focused or has no frame omits the row, because the copy acts only on the focused panel's selection.
Keyboard copy mode keeps its own entry and keys.

Find in terminal sits in the pane menu between Copy and Clear terminal and in the global
Panel group before copy mode, both showing the Find shortcut. Choosing the pane row focuses that
panel, then opens the [Find bar](native-terminal-find.md) on it. Both rows share the shortcut's
rule: they are disabled with Terminal disconnected exactly when the shortcut would refuse with that
notice. A panel too small for the bar keeps the row enabled; choosing it shows the bar's notice.

The global menu, its new-Tab submenu, the `[v]` new-Tab menu and Start Agent ▸ include a query row;
the last two scroll after twelve rows. Printable characters and Space edit the
query directly; Backspace removes the previous grapheme, Ctrl+u clears it from the end, and the
other shared readline keys of the [native line editor](native-line-editor.md) edit it at its cursor
while arrows and Home/End keep navigating the menu. The query is limited
to 128 UTF-8 bytes, and menu capture keeps typed characters out of the terminal program. Matching
uses a case-insensitive fuzzy subsequence over localized action labels, reviewed localized semantic
aliases and the current effective shortcut. A matching child action makes its parent submenu
searchable; opening it carries the query into the ordinary child menu, where the child actions are
filtered under their existing ownership and availability rules. The query row shows the filtered
count, and an empty result set shows a localized no-results state. A Change machine list past eight
saved machines also leads with a query row under the same keys. It starts empty, since what found its
parent row is no machine's name, and matches a machine's name or address as the Settings filter does.

Keys that work only inside a mode also have rows, so a search finds every key the Client takes.
These key rows answer a search only: the unsearched menu lists what it can run, and a key row
appears once a query matches it. Each row names its mode before the key's own words, as in Copy mode
· Search, and shows the key; the plain prefix keys (Tab 1..9, sending the prefix to the terminal and
cancelling the prefix) name only themselves. Search also matches each key row on help, and on its
mode's word. A key row is always disabled, because it works in keyboard copy mode, while the sidebar
has the keyboard, or only as a key; it shows no reason, and choosing one changes nothing. A copy key
that needs the clipboard, and the sidebar's search key without sidebar search, are hidden as their
keys are. The new-Tab submenu retains quick default creation and also lists the confirmed device
launch profiles, including System default. Each saved choice names its profile and creates one Tab
with that profile's literal executable and arguments; it never changes the shared default. System
shell uses the native system-shell resolver only after the current origin establishes a direct local
Session, independently of the configured default profile. A remote or unknown origin refuses that
one-time system-shell choice; ordinary quick default creation still resolves on the executing
Worker. Custom choices retain their literal paths and arguments. The chosen name appears in the
creation notice. Removed, changed, invalid or unsupported choices refuse with a Settings recovery
notice, without falling back or creating a replacement. Profile choices in an empty compatibility
slot require selecting or creating a Workspace first. Esc and the existing parent-return, scrolling,
narrow-layout and draw-authority rules apply unchanged. The local adapter matches Desktop's
saved-profile semantics; command catalog IDs and CLI `--profile` retain their separate existing
catalog authority. Close Workspace…, Remove Worktree…, Close panel… and Tab Close intents declare
`ActionTone::Destructive`; the shared metadata and action/row controls resolve that label through
`accent-destructive`. The Tab Close confirmation uses the same intent tone. Cancel, metadata edits,
Project Ungroup, Tab group Ungroup and color choices remain ordinary; a red palette choice is a
value, not destructive intent. These labels are not error statuses. Disabled rows use
`text-disabled` and acquire no pointer state; enabled intent retains the row's independent focus,
provisional, hover and held-press cues. The theme owner supplies all shipped and reduced-color
values and any required native contrast pair. Ctrl+click toggles a Tab's selection mark, and the
global menu selects all Tabs or keeps only the current one; Shift gestures stay with the host
terminal, so range selection has no native gesture. Tab actions configure the captured selected set
when their target belongs to it. [Typed row scope and saved color](#tab-scope-and-current-values)
make the affected set explicit without changing the underlying operation. Selection or
selected-member changes revoke the menu proof; background metadata actions keep the current terminal
attached. Moving the current Tab uses GUI group boundaries, including collapsed neighbors. Metadata
actions preserve the current terminal; a submenu retains its exact addressed Tab.

Recurring menu and prefix actions share one native metadata projection: a stable action identity,
category and reviewed semantic search aliases. Dynamic group color, group index and parameterized
resize input remain typed `ClientAction` or `MenuAction` data rather than becoming row-derived
identities; direction-specific IDs reuse an owning GUI catalog only where that catalog already
defines the same operation. Configured command IDs remain opaque and unchanged. Menu rows obtain
effective chords from the resolved per-Client binding registry, including overrides and shadowing.
A menu row shows the first effective chord only when an explicit `MenuTarget` plus `MenuAction`
mapping has the same dispatch meaning; a current-Tab or current-Workspace binding is never advertised
for a background object merely because both actions share metadata. Menu execution still uses its
typed target, exact coordinator proof and owner-derived availability; metadata and displayed chords
grant no execution authority. Search aliases find only admitted actions in the global menu and its
searchable child menus; they do not expose command identities or hidden configuration commands.

The global Detach action states its effect in localized wording: only the current Client
detaches, and terminal programs keep running. Its search aliases add the words people use for
leaving, quit and exit (退出 and 关闭 in Chinese), so search finds it without renaming it.
This differs from closing a Panel, Tab or Workspace; the existing detach/reconnect behavior and
confirmation policy remain unchanged.

The global Update row, identity `workbench.client.update` as in the window, has no default key and
appears only while the
[Update control](../../../proposals/platform/automatic-updates.md#the-update-control) is Ready:
the release store, read every ten seconds, names a version newer than this Client active, on Windows
or macOS in a test or release build that the automatic-update switch admits. Its row carries the control's
sentence, Update · Switch to Orchbee {version}. Terminals keep running. Choosing it checks that the
new version joins the running Session: the Session's answer lists `compatibility-window` and the
active manifest's `client` component lists the `client-server` generation this connection
negotiated. On Windows the active version's own `orchbee` then decides in the background whether
the Session restarts first: it reads the Session's restart tier with `status server`, and when that
tier is `recommended` or `required`, or the version cannot join, it runs `server restart`, which
keeps every terminal, while the status line says Restarting Session… (`正在重启会话…`). A restart
that answers `restarted` switches as a joining Session does; any other outcome switches nothing. A
Terminal Host whose own tier is `required` cannot move to the new Worker, so no restart keeps its
terminals: the TUI asks Restart terminals to update? (`重启终端以完成更新？`) and arms Restart as a
move's question does. Cancel leaves the update waiting; Restart stops
the Session through the TUI's own connection, as a stop action does, and once the stop has ended
switches as a joining Session does, so the new version starts the Session and every terminal
restarts in its folder from the final capture. When
it joins, the TUI restores the console and exits with status `75` to an
announcing [launcher](../../orchbee-launcher/README.md#relaunch-protocol). Without one, a macOS TUI
replaces its own process with the store's active Client, read again, with the same arguments and
environment and no blocked signal, so the new version opens in the same terminal under the same
parent; elsewhere, or when that replacement cannot start, it exits with `0` and one line naming the
command that opens that version. A macOS TUI that no launcher started, the replacement included,
[records its own failure](../../../proposals/platform/automatic-updates.md#what-counts-and-who-records-it)
as a launcher would: a replacement that cannot start, a failed join that a launcher would see as
status `76`, and a panic each run the previous version's Client-failure helper. When it does not,
nothing switches and
the status line says the update applies when no terminal is running; a row chosen after the store
moved back says there is no update to switch to.

Each displayed chord occupies a separate right-aligned slot after the navigation track and localized
label, with at least one blank cell between the label and chord. The chord remains complete;
constrained rows truncate the label first and omit the whole chord only when the remaining row cannot
preserve the marker, a usable label and the gap.

Selected/other/trailing Close and group Close show their frozen affected count, then open one
[complete-set confirmation](native-tab-close-sets.md). Relative Close protects pinned Tabs;
direct selected/group Close includes them where applicable. Hidden collapsed members remain targets.

A Tab whose close stopped offers no separate row: its Close row takes the ordinary close path, as
[close status](native-tab-close-sets.md#close-status) describes.

In a menu with a query row, Up/Down select rows while printable characters,
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
rectangle. An availability refresh between the press and the release does not end the click: press
and release still count as one click when the row under the pointer is the same action, in the same
place, and enabled both at press and at release. A refresh that moved, removed, disabled or replaced
that row cancels it, as does a release on another row. Keyboard input, focus loss, capture release or a changed target/rectangle cancels a
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
independent shared pointer state. The global menu reserves a reason row only when it has an unavailable
action whose reason has words, and a context menu reserves none, so hover never changes row geometry. Each frame derives
its visible window from the current row budget. Keyboard navigation keeps the selected action visible, while a manually scrolled
viewport remains where the user placed it. The successful draw commits the exact first action,
visible action count and selected action for pointer and activation admission; a larger frame fills
available rows without changing selection.
A cancellation or owner unavailability outcome appears once after the menu closes; it does not
duplicate the open menu's footer or geometry-recovery status.
A newly requested global menu that cannot present its title, a row and its exit instruction is refused
with a localized notice. A context menu instead requires its border and at least one action row;
its minimum complete frame is 16 columns by 3 rows.
If an admitted menu later becomes too small, it enters `Suspended`, retains
its source, selection, viewport, parent and anchor, and accepts only its current local exit or return.
Restoring sufficient geometry resumes the same menu incarnation after a fresh successful paint.
Ordinary terminal output continues while the menu captures input.
The underlying pane cursor is hidden during capture;
cancelling restores the ordinary pane projection without changing focus.
Known unavailable actions use the full disabled text style, retaining its modifiers
at every color level. Only three reasons have words, each in one form: `Terminal disconnected`,
`Not in a Git repository` and `Select text first`; any other disabled row stays grey without a
reason. Selecting a row reveals its reason when it has one; Enter or clicking keeps the menu open and
cannot confirm or navigate. A fixed reason row leaves the global exit instruction and
selection count visible even at the smallest presentable geometry, while a context menu paints
the reason on its bottom edge, where its heading sits on the top one, truncated to that edge,
without adding a row or footer chrome. The global footer omits the Enter action
for a disabled or offscreen selection.
Actions unsupported by the current host are hidden rather than presented as disabled learning
rows; no placeholder or unsupported-host reason row is emitted. Context-relevant
actions whose ordinary prerequisites are unmet remain visible with their owner-derived disabled
reason.

A background publication read, such as the one a `cd` starts, disables no row. A row that leads to a
setup request, such as Move to Project ▸ or Rename on a Project, stays enabled through it; a chosen
one waits for the read to settle, ahead of any further background read, and captures its request
from what the read installed. The Client's own refresh still reports the row busy.

Browsing has no time limit: reading, selection, repainting and disabled activation
do not start an execution timer. Confirming an available action creates one
absolute 30-second deadline. The same deadline bounds target navigation and
handoff; draw and navigation callbacks never renew it. Expiry cancels the
unhanded action without rolling back accepted navigation. Target, origin,
configuration and capture invalidation still cancel browsing. Geometry follows the refusal and
suspension lifecycle above.

Availability projects existing action-owner prerequisites, without a universal
Ready guard. Released pointer capture disables Release pointer capture. Unknown asynchronous prerequisites continue through their original
owners; the menu does not infer Git support, worker reachability or an operation
outcome from read-only machine observations.

Project menus expose New Workspace and Rename Project, then a separated Ungroup, shown as Rename and Ungroup.
The New Workspace submenu uses the same choices as the exact Project's sidebar `+` button: New Workspace,
Choose directory for new Workspace…, the Project's folders, and the discovered openable Worktree paths. The Project's folders are its Workspaces' current directories in sidebar order, one row per directory spelling, each named by its folder name, or by its full path where folder names repeat; choosing one creates a Workspace there in the Project. The submenu heads them Folders, as it heads the Worktree paths. Both creation entries
use the same composition and availability rules, including empty and noncurrent Projects. Directory
entry runs the existing setup workflow; default creation runs the Project quick path. The submenu
stays a submenu even when only one choice is enabled. Rename
and Ungroup also target the drawn Project without entering a child. Ungroup removes only
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

Workspace menus lead with Start Agent ▸ while an Agent can start, then contain Move to Project ▸,
Change machine ▸ where the TUI reaches saved machines, Rename Workspace (shown as Rename), a
separated Worktree section and a separated close section. Start Agent ▸ lists the new-Tab menu's
Agents, fixed when the menu opens; a row starts that Agent in a new Tab of the addressed Workspace,
which it selects first when another Workspace is current. The row is absent while no Agent is found.
The Worktree section holds
New Worktree…, which opens the New Worktree form with its defaults for the sidebar Create route
and creates only on that form's Create, and Open
Worktree ▸, whose submenu lists the discovered non-bare checkouts other than the Workspace's own
(labelled like the Git Worktrees group) and opens the chosen one through the sidebar Open route.
Both act on the addressed Workspace without selecting it; their source and grouping follow
[native-worktrees.md](native-worktrees.md), so an ungrouped Workspace and the Worktree it adds land in
one new Project. Move to Project ▸ lists the other Projects in canonical order, then a
separated New Project… and, for a grouped Workspace, Remove from Project. Choices retain exact IDs, including duplicate labels, and a same-named Project shows its name
alone, as in the Sidebar. New Project asks
for one name and atomically creates the Project and moves the Workspace. Organization never selects
a noncurrent Workspace. Change machine ▸ lists where the Workspace can run and moves it as
[machines.md](machines.md#change-machine) describes; a move that succeeds opens the Workspace on its
new machine. Empty conditional creation sections publish no orphan separator.

Opening a Project or Workspace menu starts Git discovery of the addressed source. New Worktree…
and Open Worktree ▸ are absent from the first paint unless current sidebar branch information or
a matching successful discovery proves Git. Unknown, non-Git, failed, expired or changed sources
contribute neither Git-only rows nor their separator. A known Git source may show Open Worktree ▸
disabled until its checkout list arrives, and while the Workspace's own checkout is the only one. Until the first answer, a menu shows the last success the Client keeps for
the same source, the newest for each of up to 16 recent sources, while the source stands in the
checkout it describes; a failed answer drops it. A failed or expired answer is never reused: each later opening of a
Project or Workspace menu or submenu, including a return to its parent, asks Git again for the same
unchanged source, and until that answer arrives the rows follow the first-opening rule above; a
query still in flight is awaited until its 10 s deadline. Git refusing the directory with exit status 128, as not in a repository
or not trusted, is kept the same way for that exact directory only, since a folder outside Git may
hold a repository below it: a later menu there omits New Worktree…, Open Worktree ▸ and their
separator, and a Project's creation entries omit checkout choices. Any other probe failure, including one that ends without an exit status, is transient: it is not kept, and the next opening asks Git again. The current addressed answer also updates an already-open menu: a refusal or failed discovery removes both
Git-only actions and their separator immediately, including on the first opening. A directory that
becomes a repository regains them when discovery succeeds. Every membership change invalidates
old pointer and keyboard draw proofs before the rebuilt menu accepts input. A sidebar row that shows a branch there outweighs a kept refusal. Otherwise a Workspace menu opened from a sidebar row that already shows a
branch for the directory it stands in starts New Worktree… enabled; Open Worktree ▸ depends on the
checkout list, so it waits for Git, and a repository with linked checkouts may enable it once on its
first menu. A row chosen before the first answer
waits for it as below, and an answer that differs changes the rows then. A row without a branch proves
nothing, because a folder outside Git and a detached HEAD both show none, so its menu waits for Git. A
directory change of the first terminal keeps the menu open and starts discovery over, and an answer
speaks only for where it was read. Until the new answer arrives the rows keep a success whose checkout
contains the new directory, so a move inside a checkout neither greys them nor changes the height, and
they change only if the answer differs. Any other move, or a failed answer, removes the Git-only rows until positive evidence returns, and an open Open Worktree ▸ list closes. The sidebar row cannot decide this during a
move, because it holds its old facts until its own read arrives. A row chosen meanwhile waits for the
new answer, bounded by discovery's 10 s deadline, and runs only against it; an answer that no longer offers
the row, or a move out of the checkout, closes the menu with the localized unavailable notice.
Success enables them, appends the discovered paths to both Project creation entries and, when the
Workspace directory is a linked non-bare checkout, appends a destructive Remove Worktree… after
Close Workspace…. Git-only rows follow the current discovery result; unrelated actions preserve their identities. Completion changes availability
under a fresh menu token and requires repaint; it never changes the candidate or the selected
action's identity, and a click held on a row it leaves enabled in place completes on release.

Close Workspace… and Project Ungroup ask for authoritative conditional admission through the existing Close or setup owner.
Only definite `confirmation-required` for that conditional kind and captured revision opens a new
complete-draw confirmation; explicit Enter then uses a distinct unconditional request ID. Missing,
stale, unavailable or unknown evidence never means idle and never escalates automatically.
[Scoped Close](native-workspace-close.md) owns Workspace terminal disposal; Project Ungroup's warning
instead explains preservation. Acting on a background target preserves the unrelated current session.

The sidebar `+` entries use `MenuTarget::Create` for the global or exact-Project creation context.
Their frame title is New Workspace, which names what every row does, as the new-Tab button's
profile menu is titled New Tab; a Project entry never carries the Project's name, which would read
as adding a Project. The Project entry puts Quick Workspace first. Existing Worktree paths and their preceding separator
appear only after source discovery yields openable entries; an empty or unresolved list contributes
neither. This checkout list is shared with the row menu's New Workspace ▸. Successful asynchronous
composition mints a fresh menu token and requires a new complete draw; late replies cannot reopen
cancelled menus or retarget another episode. The [shell layout](tui-shell-layout.md#quick-creation-controls)
owns the entry geometry and direct Tab-strip create gesture.

The global menu groups its existing actions under Tab, Panel, Workspace and Session sections. Tab
includes new-Tab options, New Tab to right, Duplicate Tab, Rename Tab, Next and Previous Tab, moving the current Tab left,
right, to the start or to the end, Select all Tabs, Keep only current Tab selected, Close Tab… and
Close Tabs to the right…; these act on the current Tab and name the selected set where it applies.
Close Tab… is the current-Tab close request that `Ctrl+b x` makes, with that key and its confirmation
rules: the Server decides whether closing asks first, as it does when terminals still run. Panel
includes Back to last focused panel, directional focus, split, move and resize, close, maximize,
Find in terminal and copy mode. Workspace includes
creation, Worktree actions, Next and Previous Workspace, Rename, Change machine ▸ for the current
Workspace and Close Workspace; Session includes
the sidebar toggle, Focus sidebar, Next and Previous Agent, and Detach. Each configured command
follows as Run configured command · its name, running the same command its key runs. A row whose keyboard action has the same
meaning shows the current binding registry's shortcut and dispatches that same existing
ClientAction. Pane and Tab actions require a focused pane and show the localized
focused-panel-required reason when none is available; Close Tabs to the right… shows the
no-matching-Tabs reason when nothing would close. A Move panel row shows the GUI's reasons for the
same state: no Panel on that side of the focused one, or a maximized Tab. The move admission refuses by that same rule, so an enabled row always moves a Panel,
and a move key the row would not offer moves nothing and shows nothing. The same holds for the
other rows whose action can be a no-op, each sharing one rule with its execution. Maximize needs at
least two Panels in the current Tab and stays enabled while the Tab is maximized, because it then
restores. Back to last focused panel shows that no earlier Panel remains to return to; while focus has
not settled it stays enabled, because only the held step can tell. A Move Tab row applies the
Server's own Tab configuration rule to the current Workspace, and a Tab moves only among Tabs
pinned as it is. Moving left from the first of those Tabs wraps to their end, and moving right from
the last wraps to their start, as the GUI's move backward and forward do, so those rows refuse only a
Tab with none to pass: only one Tab, no other pinned Tab or no other unpinned Tab. `Ctrl+b <` and
`Ctrl+b >` run the Previous and Next rows themselves: a key gives its row's result in every position, group edges included, and stays silent where the row is disabled. Moving to the start or end also shows that an ungrouped Tab
already stands there; a grouped Tab there stays enabled, because the move takes it out of its group. Copy mode shows Terminal disconnected as Clear terminal does,
unless the focused Panel shows a launch failure, which Copy mode then presents. Other global actions
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
set is empty, while the global Close Tabs to the right… stays grey. Disabled rows remain visible. Counts are `NonZeroUsize` projection facts, never parsed labels or command
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
apply a value immediately; native menu navigation remains provisional until activation. Both use
canonical group color and the existing update owner; this is not a second shared color model.

## Model and ownership

The user wants to discover and execute existing actions without memorizing chords.

| Record field    | Contract                                                                                                                                                                                                                                                                                                                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs          | Drawn stable target, exact context/member/incarnation, projected stable action identity and typed input, current action prerequisites, complete decoded input packet, configuration, monotonic deadline and successful menu draw receipt                                                                                                                                            |
| Data            | One local episode, `Pending`/`Admitted`/`Suspended` presentation phase, independent whole-surface lease and exact row-window proof, `Recovery`/`Continuation`/`ActivationReady` input readiness, retained viewport and selection, owner-derived enabled/disabled/hidden availability, presentation token, frozen confirmed action, parent snapshot and optional exact target member |
| Outputs         | Local capture and drawing; bounded navigation through existing membership owners; at most one action handed to the full ClientView/dispatcher path; localized cancellation or refusal                                                                                                                                                                                               |
| Decisions       | Existing captures, including the native navigator, take priority. The sidebar remains an unchanged underlay. Choosing an action admits navigation; opening does not. Each action retains its original availability policy rather than a universal Ready requirement                                                                                                                 |
| Invariants      | No undrawn or stale activation, no focus on open/cancel, no action on a replacement target, no implicit retry, no input or decision-packet tail reaching a pane; ordinary Close keeps its confirmation; conditional Delete requires exact authority proof or a new reviewed decision                                                                                                |
| Model           | Finite safety machine for capture, drawing, target invalidation, owned and foreign navigation, cancellation, prerequisites and one action transfer                                                                                                                                                                                                                                  |
| Dependencies    | Existing native membership, terminal state, input decoder, hit map, layout, localization and action owners; additive conditional Delete wire actions remain Server-owned; no new dependency or persistent data                                                                                                                                                                      |
| Ownership       | WorkbenchClientUi owns menu admission and target navigation; the deterministic menu owns selection and draw authority; ClientView owns local view decisions; InputDispatcher alone applies shell, mode and layout effects; original operation owners alone execute mutations                                                                                                        |
| Reference scope | Discoverability for current Orchbee actions                                                                                                                                                                                                                                                                                                                                         |

`MenuTarget::Project` carries a `WorkbenchProjectId`, never a label or row index.
Admission freezes the complete canonical Project and, only when its ID matches the
current Workspace's Project, that Workspace/Directory source. Empty Projects and
Projects outside the current Workspace remain valid menu targets: default and directory creation
use the exact Project when the setup owner admits them. Rename and organization-only Ungroup
likewise target that Project without entering a child.
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

| Project context at opening                                   | Availability and effect                                                                                                                                   |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Current Workspace belongs to the exact Project               | Existing actions retain their own prerequisites; confirmation offers one action without navigation                                                        |
| An ungrouped Workspace, a different Project, or no Workspace | New Workspace ▸, rename and organization-only Ungroup target the exact Project; discovered checkout choices need its admitted derived first-member source |
| Project or captured Workspace/Directory replaced             | Cancel before confirmation or handoff; never substitute another child                                                                                     |
| Cancel                                                       | Preserve the native origin and caller-owned sidebar focus                                                                                                 |

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

Presentation tokens bind the episode, exact semantic target, keyboard selection and viewport revision. An
availability refresh mints a token of the same lineage, because it leaves the rows, their copy, the
selection and the viewport unchanged; every other revision starts a new lineage. Lineage serves only
a held click; hover, rest, wheel and keyboard input still require the exact current token. The
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
typed activation intent; the coordinator revalidates its source and the menu owner its token, which
is the current one or, when only availability changed since the press, the token of the draw still
on screen, and in both cases the row must be enabled now. Keyboard selection and availability
changes mint a new token and require a new draw. The pointer owns one rest timer for submenu
opening and parent return; when it fires, the coordinator revalidates the painted source and token
and the menu owner opens only a cascaded submenu row, through the same confirmation as a click. The
global menu's replacing new-Tab submenu still opens only on a click or key.
Pane reconciliation cancels underlying gestures while preserving the top ActionMenu owner's
painted hover and press across idle and draw boundaries. A matching release still requires the
same target, token, exposure and geometry; a covering owner or invalidated hit map revokes it. A
frame that paints the pressed row again under a refreshed token of the same lineage, with the same
rectangle, enablement and exposure, carries the press over to that token; any other difference
cancels it.

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
closes. Pointer release and maximize verify their frozen prerequisite before handoff.
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

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA
model. Shared identity, ownership and commit protocols retain their separately registered core
models.

An anchored sidebar menu retains the full covered row rectangle for placement while suppressing its underlying input targets.
