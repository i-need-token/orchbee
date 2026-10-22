# Native Tab groups and capability coverage

The Windows native TUI projects canonical group identities, names, colors, membership and pinned
state. The compact strip renders a separate group header whose reserved disclosure cell shows its
arrow on hover or while collapsed.
The Tab menu follows the GUI creation/grouping, duplication/pinning and close sections;
selection and movement live in a submenu. Group headers and the new-Tab button own separate menus.
Keyboard menu navigation and painted pointer targets invoke the same owners. Configuration of a
background Tab keeps the current Tab, Panel, Presentation, terminal binding and viewer selected.

## Actual host capability comparison

This comparison concerns the admitted Workbench composition, not everything exported by the
reusable Tab Strip. The GUI owners are `WorkbenchTabBar`, `useWorkbench`,
`getWorkbenchTabCreationAvailability`, `WorkbenchProjectWorkspaceRuntime` and `TabModel`.
Native entries refer to the Windows TUI. This is a capability slice, not complete GUI/TUI parity.

| Core operation                                      | Admitted GUI path                                                                                                                        | Native coverage and remaining gap                                                                                                                                                                                                      |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Function menus                                      | Tab, group, Panel and Workbench Action menus                                                                                             | Separate Tab, group and new-Tab menus; group choices and selection/movement use submenus with separators and keyboard back navigation.                                                                                                 |
| Create, rename, close one Tab                       | Terminal owner creates/closes; title action edits canonical name                                                                         | Existing canonical native operations; grouped/pinned metadata remains representable.                                                                                                                                                   |
| Multi-selection                                     | Client-local selected IDs and anchor; toggle, range and all                                                                              | Selection submenu: add/remove, range, all and clear; Ctrl press marks a painted Tab. Shift stays host-owned. Native marking keeps the current terminal active.                                                                         |
| Pin/unpin                                           | Selected Tab set through `TabModel.setTabsPinned`                                                                                        | Exact selected set, in canonical order; moves at the pinned boundary and removes group membership when pinning.                                                                                                                        |
| Create group from existing Tabs                     | Selected set through `createGroup`                                                                                                       | Exact selected set, server-allocated group identity, name editor and color menu.                                                                                                                                                       |
| Assign/remove group membership                      | Selected set through `moveTabsToGroup`                                                                                                   | Exact selected set; joining unpins, source empties are removed and surviving groups stay contiguous.                                                                                                                                   |
| Rename/recolor/ungroup                              | Group menu through `updateGroup`/`ungroup`                                                                                               | Implemented for the complete canonical group, including all nine colors. Ungroup preserves terminals.                                                                                                                                  |
| Move group                                          | `shiftGroupRelative`, accepted drag arrangement                                                                                          | Previous/next neighboring group or ungrouped Tab; pinned boundary and ends do not wrap. Pointer drag moves the full group as one collection.                                                                                           |
| Collapse/expand                                     | Client view `collapsedGroupIds`, projected into `TabModel`                                                                               | Header press or menu; identity-keyed Client-local state. Native disclosure is not persisted and is retained for one context at a time.                                                                                                 |
| New Tab in group / new group with Tab               | Enabled for local creation routes; remote grouped placement is rejected, and remote new-group creation is disabled                       | Implemented through one atomic `tab.create` placement; new Tabs activate after validated creation. GUI uses two model commits.                                                                                                         |
| New Tab to right/below                              | Local owner honors target group and index + 1, rejecting incompatible pinned-boundary placement                                          | **New tab to right** targets the exact menu Tab without navigation. Atomic placement keeps its group and index + 1; insertion inside the pinned prefix refuses before runtime allocation. Group-menu creation stays group-tail.        |
| Duplicate                                           | Selected invoked target expands to the selected set; local creation enables both single and multiple sources, remote routes disable both | Selected target expands to the initial canonical selected IDs; unselected target stays exact. One existing Create lifecycle per source, with bounded partial/unknown results and preserved selection. No live split-tree or PTY clone. |
| Close group / selected set / other or trailing Tabs | `onCloseTabs` delegates the exact ID set to `requestActiveTerminalTabsClose`; terminal owner supplies running-resource confirmation      | Implemented exact target rules and one complete-set confirmation over existing Close lifecycle; incremental settlement and preserved recovery. See [close sets](native-tab-close-sets.md).                                             |
| Move one Tab across a group boundary                | GUI model updates arrangement and grouping together                                                                                      | Tab menu moves left/right/first/last with GUI grouping semantics; collapsed neighbors are skipped using a local view hint. Legacy direction/index shortcuts retain their original ordering semantics.                                  |
| Move to another window                              | Library callbacks/runtime factory are optional; current application hosts do not install the new-window callbacks                        | Not an admitted current-host parity claim. No new native window-transfer operation.                                                                                                                                                    |

Orientation, hover cards and vertical-strip preferences are host presentation choices. The native
menus now cover the admitted creation, duplication, grouping and close operations above. Close
deliberately removes only the vacated source group when it becomes empty, preserving unrelated
empty groups; GUI `TabModel.closeTabs` prunes all empty groups. The
[Close owner boundary](native-tab-close-sets.md#group-cleanup-boundary) records this intentional
exception. Host presentation differences remain explicit limits.

## Shared authority and receipts

`tabs.configure` is one closed operation on an existing `terminalContextId`: `create-group`,
`set-group`, `update-group`, `ungroup`, `set-pinned`, `move-group`, `create-group-for-tabs`,
`set-tabs-group`, `set-tabs-pinned`, `move-tab` or `move-tabs-by-drag`. Set operations carry 1–256 unique Tab IDs and
reject the entire command if any member is absent. Input order never replaces canonical order. The existing authenticated
command/revision CAS and request ledger own admission, replay and atomic candidate publication.
The Server allocates a group identity only for creation and returns it in `tabs-configured`;
all other successful changes return a required-null `groupId`. A no-op returns `unchanged` at the
original revision. Stale revisions, missing targets, bounds and any unfinished Close in the context
refuse before writes. There are no terminal or Worker effects in this operation.

The native adapter reuses the setup transaction's queue, deadlines, known/unknown result handling
and snapshot recovery. It retains the bounded canonical input snapshot behind an `Arc`, rather
than reconstructing a writable document from rendered Tabs. An exact-revision receipt must equal
the expected full snapshot, including unrelated contexts, navigation, trees, labels and bindings.
A created group ID must be absent from every baseline context. Later revisions follow the existing
canonical refresh rules because a peer may already have made another valid edit. Unknown delivery
retires the connection and never retries the mutation automatically.

`configure_tabs` is the pure Rust transition shared by authority planning and receipt checks.
It preserves every existing Tab/Panel/Split/Slot/Presentation identity and terminal binding. Group
cleanup follows GUI `removeEmptyGroups`; creating a group from the middle of a surviving source
group inserts after that source block instead of splitting it. `TabModel.moveTabsToGroup` enforces
the same boundary. This is canonical metadata, not a second TUI-only group model or persistence path.

Selection marks are identity-keyed and Client-local. The active Tab is always included; ordinary
Tab navigation clears additional marks. A menu target inside the selected set configures that whole
set; an unselected background target configures itself. Typed single/selected-set row copy exposes
the affected count for pinning, group creation, removal and duplication. Rename, create to right and
individual movement still address one Tab. Menu proof captures the selected canonical members, so a changed
selection or a changed member invalidates the choice. Marked Tabs, and collapsed headers with marked
members, rest on the selection surface without changing their width. Marking deliberately preserves
the active terminal, unlike GUI selection that can also activate the marked Tab.

Chinese menu, path and drag copy calls each Tab **标签页** and a group **分组**; English retains **Tab**.
The localized wording distinguishes selected members from a whole group without changing captured IDs.

Collapsed state remains Client-local, matching the authority in
`packages/workbench/src/workbench/document/workbench-client-view.ts`. Header disclosure does not
select a member, submit a command, advance a shared revision or hide members from the navigator.
A group name uses its color as a bold accent over a faint tint; current and pointer surfaces replace
only the tint through the [theme owner](native-themes.md#selection-and-ownership). Monochrome
preserves names, disclosure and the current surface's fallback. Names pass through the existing
terminal-text sanitizer and cell-width clipping. Pointer regions come from the same paint as the
header and member tokens.

## Pointer arrangement and menu navigation

The new-Tab button's right-click menu contains New Tab, New Tab in Recent Group and New Group
with Tab; the global menu exposes the same submenu to keyboard users. The recent group is local
to the current context: active group, last retained group, then the last existing group.
The Tab root exposes Group actions directly beside group assignment; selection/movement does not
contain group management. Direct header right-click still opens the same exact group menu.
That menu contains in-group creation, rename, a nine-color submenu, disclosure, adjacent movement,
ungroup and Close. Ungroup explicitly keeps Tabs; Close targets every canonical group member.
Tab group choices contain New Group and other existing groups.
Selection/movement remain keyboard-accessible from the Tab submenu; Shift gestures belong to the
host. Move to New Window is hidden on the unsupported native host.

The [menu projection](native-action-menus.md#tab-scope-and-current-values) keeps a group's saved
color check, each palette chip and provisional menu navigation separate. Opening selects the
saved color without a write; navigation and cancellation preserve it. Explicit activation uses
the existing `update-group` operation, and reopening reads the committed value. Cross-group marks
do not change which whole group is recolored.

`tui_pointer::tabs::TabDrag` admits a plain left press only against the last successful frame's
exact Session/revision, context, ordered metadata and token rectangles. A two-cell horizontal move
or a row change starts a static grabbed highlight and insertion marker; terminal output continues.
Motion is private. A valid left release emits exactly one `move-tabs-by-drag` with a bounded
selected ID set, original-order insertion boundary and nullable destination group.
Pinned and unpinned members never mix. Complete selected groups retain identity; partial members
take the destination group. Untouched groups remain contiguous. An expanded member is an in-group
target; a header selects the before/after collection boundary, including collapsed groups.
Terminal cell geometry has no hover-expansion or animation timing.

Escape, outside release, modified pointer input, focus loss, capture release, competing overlays, resize, changed
source or changed painted rectangles cancels without a write. Cancelled gestures consume their
remaining release. Ordinary clicks activate or disclose on release. As with native marking,
dragging preserves the current terminal; the GUI may activate a dragged member on press.
Headers carry no member count or brackets: the tint and adjacency mark membership. A collapsed header
retains name, color and disclosure, takes the current surface while it hides the active Tab, and
carries its members' most urgent Agent state and unread dot.

The pure configuration corpus compares ten drag outcomes with GUI `TabModel.moveTabsByDrag`.
The Rust selection/boundary matrix checks pinned prefix, contiguous groups and preserved terminal
data. `tui_pointer::tabs::tests` covers release-only writes and cancellation traces;
`ui::tab_groups::tests::interaction` covers exact-source admission, canonical receipt installation,
distinct menu scopes and fresh paint after submenu back. The existing setup transaction owns
stale/unknown outcomes; no drag-specific retry, ledger, persistence or distributed protocol is added.

## Terminal creation and duplication

The group menu's **New Tab in Group**, the new-Tab menu's **New Group with Tab**, and the Tab
menu's **Duplicate Tab** use `WorkbenchAction::TabCreate` and the existing native `CreateOperation`.
Optional typed `placement` is `in-group { groupId }`, `new-group`, `after-tab { tabId }`, or `duplicate { tabId }`.
Omission keeps ordinary ungrouped/unpinned append. Placement and all created membership commit in
one revision; there is no create-then-configure window with noncontiguous groups.

In-group creation inserts after the group's final member (at the end for an empty group).
This matches the GUI group menu, not its separate **New Tab to Right/Below** action. That action
passes the target group and index + 1 through `executeWorkbenchTerminalCreate` to
`WorkbenchProjectWorkspaceRuntime.createTerminalInTarget`; `canInsertTerminalTab` verifies the
exact requested index and rejects incompatible placement instead of silently appending. The native
Tab menu exposes **New tab to right** through `after-tab`: the authority derives the target group
and exact successor index under revision CAS. A new Tab is unpinned, so placing it inside the pinned
prefix refuses before identity/runtime allocation; placing it after the last pinned Tab succeeds.
The GUI action can appear enabled before that execution-time rejection. Global New Tab still appends.
New-group creation appends one unpinned Tab in a fresh unnamed grey group and prunes empty groups,
matching `createNewTabGroup`; the group limit applies after that cleanup. Group labels admit
0–512 Unicode scalar values through `WorkbenchTabGroupLabel`, while Tab/Presentation labels stay
nonempty. Empty headers and menu targets display localized **Unnamed group** / **未命名分组**;
identity, color, count and disclosure remain usable without writing that fallback into shared state.

Duplicate expands an invoked selected Tab to the canonical ordered selected set; an unselected
background Tab remains one exact target. The admitted local GUI has the same scope:
`getWorkbenchTabCreationAvailability` admits multiple sources, and `useTabStripActions` iterates
those IDs through `TabModel.duplicateTab`. Sources are captured once and never include created copies.
Single duplication copies raw Tab label, native automatic/fixed naming mode, pin and
group membership, and inserts immediately after its source. GUI `TabModel.duplicateTab` uses
`addTab` with `activate:false`; admitted `PanelWorkspaceStore` reconciliation creates one new
Panel/Presentation with the copied title as fallback and no custom-title policy. Native naming
keeps its own canonical automatic/fixed contract. Neither route clones a live split tree, child
Presentations, PTY, launch profile or current directory. The existing terminal creation owner
allocates all fresh identities and launches one fresh local terminal.

Foreground creation clears extra selection marks only after installing a validated snapshot that
still contains the created Tab, for both grouped and ungrouped creation. It selects that Tab and
expands its group. Submission, receipts before installation, and a later snapshot that already removed
the created Tab preserve marks.
Duplicate preserves the current Tab/Panel identity and local group disclosure/marking. It uses the
existing creation barrier, so the selected viewer is cleaned up and attached with a fresh generation;
this is not continuous attachment. Its attachment decision uses the selected terminal's provenance,
not the duplicate's outcome. Source deletion in a later valid snapshot uses the existing fallback.

Authority admission checks exact source/group, revision, Close fences and structural capacity before
runtime effects. The shared pure `create_terminal_tab` transition drives both atomic Server planning
and exact-revision native receipt checks over the full canonical snapshot, including other contexts.
Created Tab/Panel/Presentation and optional group IDs must be fresh. Later snapshots use established
strict identity/binding rules. A malformed receipt never installs an optimistic candidate.

Pending, failed, unavailable and unknown terminal outcomes retain committed membership and their
original provenance. Background duplicate exposes a localized pending/not-started/unknown notice;
it cannot report terminal success based only on membership. Uncertain submission is never replayed
automatically. A known commit with unavailable projection recovers through an explicit Snapshot read,
then the same exact cleanup/install path. Selected duplication retains each member's exact operation,
known commit and terminal provenance separately from its temporary status. It reports submitted,
metadata-committed and terminal-started counts; there is no all-or-nothing batch claim or new bulk command.

The private selected-duplication state bounds the sequence to 120 seconds. Each existing 50-second
Create budget is capped by that absolute deadline, with its one five-second cleanup reserve. Before
every Start it compares the complete Session Snapshot, context, marks and selected concrete PaneRef /
Presentation / binding. The revision CAS remains authoritative. Only the exact next revision and
pure single-duplicate successor, with unrelated contexts/groups unchanged, may continue the sequence.

Membership preparation checks the frozen old fence before changing the cache, desired member or
disposing the old pane record. Its non-cloneable witness belongs to that correlated operation and
install, and is consumed once after the new pane is minted. The old fence survives detach preparation.
Only that witness may adopt the fresh generation; the original Tab/Panel/Presentation must reach
Ready again and the prior lease, reply and causal poll settlement must finish before the next Start.
Idle observation and enqueue share the mailbox admission mutex. Busy burns no identity, waits for
the fair UI tick, and leaves the current lifecycle's polls/replies running.

Navigation, marking and focus cancellation are latched, including away/back changes. Peer drift,
capacity refusal, pending/failed/unavailable/unknown outcomes, projection recovery and expiry halt
the remaining sources. The current operation still follows its existing cleanup/recovery owner;
late facts may refine counts but never resume the tail. Sequence status yields to later errors and
expires five seconds after completion or halt; the facts remain until a new explicit sequence replaces them.

## Compatibility and verification

The owning implementation tests cover group membership, pinned ordering, duplicate source identity, sequential receipt/install/Ready barriers, cancellation and refusal without automatic replay.

Maintain the concrete witnesses in `src/terminal_session/state/tab_groups.rs`, `src/terminal_session/state/tab_duplication.rs`, `src/terminal_session/mailbox/tests/tab_duplication.rs`, `src/tui_pointer/tests/tab_groups.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
