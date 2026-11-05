# Native Tab groups and capability coverage

The native TUI projects canonical group identities, names, colors, membership and pinned
state. The compact strip renders a separate group header whose reserved disclosure cell shows its
arrow on hover or while collapsed.
The Tab menu holds only actions on its Tab: rename, pinning, group membership and close.
Group headers and the new-Tab button own separate menus; selection and movement use Ctrl+click,
dragging and the global menu.
Keyboard menu navigation and painted pointer targets invoke the same owners. Configuration of a
background Tab keeps the current Tab, Panel, Presentation, terminal binding and viewer selected.

## Actual host capability comparison

This comparison concerns the admitted Workbench composition, not everything exported by the
reusable Tab Strip. The GUI owners are `WorkbenchTabBar`, `useWorkbench`,
`getWorkbenchTabCreationAvailability`, `WorkbenchProjectWorkspaceRuntime` and `TabModel`.
Native entries refer to the shared TUI on all supported platforms. This is a capability slice, not complete GUI/TUI parity.

| Core operation                                      | Admitted GUI path                                                                                                                                                                                                  | Native coverage and remaining gap                                                                                                                                                                                                   |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Function menus                                      | Tab, group, Panel and Workbench Action menus                                                                                                                                                                       | Separate Tab, group, new-Tab profile and Panel menus; group choices and colors cascade as submenus with keyboard back navigation. The global menu carries selection, movement, duplication and trailing Close for the keyboard.     |
| Create, rename, close one Tab                       | Terminal owner creates/closes; title action edits canonical name                                                                                                                                                   | Existing canonical native operations; grouped/pinned metadata remains representable.                                                                                                                                                |
| Multi-selection                                     | Client-local selected IDs and anchor; toggle, range and all                                                                                                                                                        | Ctrl press toggles a painted Tab's mark; the global menu selects all or keeps only the current Tab. Shift stays host-owned, so range selection has no native gesture. Native marking keeps the current terminal active.             |
| Pin/unpin                                           | Selected Tab set through `TabModel.setTabsPinned`                                                                                                                                                                  | Exact selected set, in canonical order; moves at the pinned boundary and removes group membership when pinning.                                                                                                                     |
| Create group from existing Tabs                     | Selected set through `createGroup`                                                                                                                                                                                 | Exact selected set, server-allocated group identity, name editor and color menu.                                                                                                                                                    |
| Assign/remove group membership                      | Selected set through `moveTabsToGroup`                                                                                                                                                                             | Exact selected set; joining unpins, source empties are removed and surviving groups stay contiguous.                                                                                                                                |
| Rename/recolor/ungroup                              | Group menu through `updateGroup`/`ungroup`                                                                                                                                                                         | Implemented for the complete canonical group, including all nine colors. Ungroup preserves terminals.                                                                                                                               |
| Move group                                          | `shiftGroupRelative`, accepted drag arrangement                                                                                                                                                                    | Pointer drag moves the full group as one collection; pinned and unpinned members never mix. The group menu has no movement rows.                                                                                                    |
| Collapse/expand                                     | Client view `collapsedGroupIds`, projected into `TabModel`                                                                                                                                                         | Header press; identity-keyed Client-local state. Native disclosure is not persisted and is retained for one context at a time.                                                                                                      |
| New Tab in group / new group with Tab               | A Server Client sends the same atomic placement. Local creation routes also honor it; Main-held remote routes refuse grouped placement and disable new-group creation                                              | Implemented through one atomic `tab.create` placement; new Tabs activate after validated creation. The local GUI route uses two model commits.                                                                                      |
| New Tab to right/below                              | A Server Client sends `after-tab` and disables the entry inside the pinned prefix. The local owner honors target group and index + 1, rejecting incompatible pinned-boundary placement                             | The global menu's **New Tab to right** targets the current Tab. Atomic placement keeps its group and index + 1; insertion inside the pinned prefix refuses before runtime allocation. Group-menu creation stays group-tail.         |
| Duplicate                                           | Selected invoked target expands to the selected set. A Server Client copies several sources as one Server operation; local creation enables both single and multiple sources, Main-held remote routes disable both | Selected target expands to the initial canonical selected IDs; unselected target stays exact. One Server operation for several Tabs, with bounded partial/unknown results and preserved selection. No live split-tree or PTY clone. |
| Close group / selected set / other or trailing Tabs | `onCloseTabs` delegates the exact ID set to `requestActiveTerminalTabsClose`; terminal owner supplies running-resource confirmation                                                                                | Exact target rules and one complete-set confirmation; the Server closes the set as one operation, each member by its own Close. See [close sets](native-tab-close-sets.md).                                                         |
| Move one Tab across a group boundary                | GUI model updates arrangement and grouping together                                                                                                                                                                | The global menu moves the current Tab left/right/first/last with GUI grouping semantics; collapsed neighbors are skipped using a local view hint. Legacy direction/index shortcuts retain their original ordering semantics.        |
| Move to another window                              | Library callbacks/runtime factory are optional; current application hosts do not install the new-window callbacks                                                                                                  | Not an admitted current-host parity claim. No native menu row or window-transfer operation.                                                                                                                                         |

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
the affected count for pinning, group assignment, removal and duplication. Rename, create to right and
individual movement still address one Tab. Menu proof captures the selected canonical members, so a changed
selection or a changed member invalidates the choice. Marked Tabs, and collapsed headers with marked
members, rest on the selection surface without changing their width. Marking deliberately preserves
the active terminal, unlike GUI selection that can also activate the marked Tab.

Chinese menu, path and drag copy calls each Tab **标签页** and a group **分组**; English retains **Tab**.
The localized wording distinguishes selected members from a whole group without changing captured IDs.

Collapsed state remains Client-local, matching the authority in
`apps/web/src/workbench/document/workbench-client-view.ts`. Header disclosure does not
select a member, submit a command, advance a shared revision or hide members from the navigator.
A group name uses its color as a bold accent over a faint tint; current and pointer surfaces replace
only the tint through the [theme owner](native-themes.md#selection-and-ownership). Monochrome
preserves names, disclosure and the current surface's fallback. Names pass through the existing
terminal-text sanitizer and cell-width clipping. Pointer regions come from the same paint as the
header and member tokens.

## Pointer arrangement and menu navigation

The new-Tab button's right-click menu lists the saved launch profiles, the default first and
marked Default, and opens only with more than one profile. The global menu's new-Tab submenu holds
New Tab, New Tab in Recent Group, New Group with Tab and the profiles. The recent group is local
to the current context: active group, last retained group, then the last existing group.
A group header's right-click opens the exact group menu: in-group creation, rename and a
nine-color submenu, then Ungroup and Close. Ungroup explicitly keeps Tabs; Close targets every
canonical group member. Header clicks fold and dragging moves, so neither is a menu row.
The Tab menu's Add Tab to group ▸ lists the other existing groups, then New group….
Selection and movement stay keyboard-accessible from the global menu; Shift gestures belong to the
host.

The [menu projection](native-action-menus.md#tab-scope-and-current-values) keeps a group's saved
color check, each palette chip and provisional menu navigation separate. Opening selects the
saved color without a write; navigation and cancellation preserve it. Explicit activation uses
the existing `update-group` operation, and reopening reads the committed value. Cross-group marks
do not change which whole group is recolored.

`tui_pointer::tabs::TabDrag` admits a plain left press only against the last successful frame's
exact Session/revision, context, ordered metadata and token rectangles. A Tab's token rectangle
spans its close track whether the control is revealed or hidden, so a pointer that reveals or hides
`[x]` neither moves nor cancels the drag. A two-cell horizontal move or a row change starts a static
grabbed highlight over whole tokens and an insertion marker on the edge cell of the token the drop
lands beside; terminal output continues.
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

The group menu's **New Tab in Group**, the global new-Tab submenu's **New Group with Tab**, and the
global menu's **Duplicate Tab** use `WorkbenchAction::TabCreate` and the existing native `CreateOperation`.
Optional typed `placement` is `in-group { groupId }`, `new-group`, `after-tab { tabId }`, or `duplicate { tabId }`.
Omission keeps ordinary ungrouped/unpinned append. Placement and all created membership commit in
one revision; there is no create-then-configure window with noncontiguous groups.

In-group creation inserts after the group's final member (at the end for an empty group).
This matches the GUI group menu, not its separate **New Tab to Right/Below** action. That action
passes the target group and index + 1 through `executeWorkbenchTerminalCreate` to
`WorkbenchProjectWorkspaceRuntime.createTerminalInTarget`; `canInsertTerminalTab` verifies the
exact requested index and rejects incompatible placement instead of silently appending. The native
global menu exposes **New Tab to right** for the current Tab through `after-tab`: the authority derives the target group
and exact successor index under revision CAS. A new Tab is unpinned, so placing it inside the pinned
prefix refuses before identity/runtime allocation; placing it after the last pinned Tab succeeds.
A Server Client disables that GUI action there; the local route's can appear enabled before that
execution-time rejection. Global New Tab still appends.
New-group creation appends one unpinned Tab in a fresh unnamed grey group and prunes empty groups,
matching `createNewTabGroup`; the group limit applies after that cleanup. Group labels admit
0–512 Unicode scalar values through `WorkbenchTabGroupLabel`, while Tab/Presentation labels stay
nonempty. Empty headers and menu targets display localized **Unnamed group** / **未命名分组**;
identity, color, count and disclosure remain usable without writing that fallback into shared state.

Duplicate expands an invoked selected Tab to the canonical ordered selected set; an unselected
background Tab remains one exact target. The admitted local GUI has the same scope:
`getWorkbenchTabCreationAvailability` admits multiple sources, and `useTabStripActions` iterates
those IDs through `TabModel.duplicateTab`. Sources are captured once and never include created copies.
A Server Client duplicates several sources through the retained
[Tab duplication operation](../../orchbee-server/docs/workbench-authority.md#tab-duplication-operation);
one source stays an ordinary duplicate Create.
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
then the same exact cleanup/install path.

Several sources duplicate as one retained Server operation. The Client sends one Start with the
sources in canonical order, its cached revision and its viewport geometry, then reads the operation
by ID every 100 ms; the copies arrive through ordinary refresh like any other Client's change, so
selection, marks and the selected viewer stay as they are and nothing reattaches. Navigation,
marking and focus no longer stop the operation, and no Client can cancel it. A lost reply sends the
same Start again, which observes the same operation; a Start refused for a revision race goes again,
at most four times, once this Client holds that revision and still shows every source in the same
order. Only one operation is followed at a time; Duplicate is ignored while one runs. The status
line reports submitted, created and terminal-started counts while it runs and for five seconds after
it ends, with the existing wording: a gone source, or sources that changed before the Server took
the Start, read as changed, a refused copy as not created, a copy whose terminal did not start or is
unknown as check-and-no-replay, the Server's 120-second limit or this Client's 130-second follow
limit as time limit, and a lost connection or stopping Session as check-and-no-replay. A Client that loses its connection stops following; the operation
goes on and its copies appear on the next refresh.

## Compatibility and verification

The owning implementation tests cover group membership, pinned ordering and duplicate source identity; the Server's tests witness ordered, once-only copying of several sources and its stops.

Maintain the concrete witnesses in `crates/workbench-client/src/workbench_state/tab_groups.rs`, `crates/workbench-client/src/tui_pointer/tests/tab_groups.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
