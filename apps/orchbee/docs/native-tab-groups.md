# Native Tab groups and capability coverage

The Windows native TUI projects canonical group identities, names, colors, membership and pinned
state. The compact strip renders a separate group header, disclosure marker and member count.
The Tab menu exposes marking, pin/unpin, grouping and Tab movement;
the group menu exposes create, assign, remove, rename, color, ungroup and adjacent group movement.
Keyboard menu navigation and painted pointer targets invoke the same owners. Configuration of a
background Tab keeps the current Tab, Panel, Presentation, terminal binding and viewer selected.

## Actual host capability comparison

This comparison concerns the admitted Workbench composition, not everything exported by the
reusable Tab Strip. The GUI owners are `WorkbenchTabBar`, `useWorkbench`,
`getWorkbenchTabCreationAvailability`, `WorkbenchProjectWorkspaceRuntime` and `TabModel`.
Native entries refer to the Windows TUI. This is a capability slice, not complete GUI/TUI parity.

| Core operation                                      | Admitted GUI path                                                                                                                        | Native coverage and remaining gap                                                                                                                                                                     |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Function menus                                      | Tab, group, Panel and Workbench Action menus                                                                                             | Existing object/global menus; Tab menu now includes movement, pinning and groups. Presentation and anchoring differ by host.                                                                          |
| Create, rename, close one Tab                       | Terminal owner creates/closes; title action edits canonical name                                                                         | Existing canonical native operations; grouped/pinned metadata remains representable.                                                                                                                  |
| Multi-selection                                     | Client-local selected IDs and anchor; toggle, range and all                                                                              | Menu add/remove, range, all and clear; Ctrl/Shift press on a painted Tab. Native marking keeps the current terminal active.                                                                           |
| Pin/unpin                                           | Selected Tab set through `TabModel.setTabsPinned`                                                                                        | Exact selected set, in canonical order; moves at the pinned boundary and removes group membership when pinning.                                                                                       |
| Create group from existing Tabs                     | Selected set through `createGroup`                                                                                                       | Exact selected set, server-allocated group identity, name editor and color menu.                                                                                                                      |
| Assign/remove group membership                      | Selected set through `moveTabsToGroup`                                                                                                   | Exact selected set; joining unpins, source empties are removed and surviving groups stay contiguous.                                                                                                  |
| Rename/recolor/ungroup                              | Group menu through `updateGroup`/`ungroup`                                                                                               | Implemented for the complete canonical group, including all nine colors. Ungroup preserves terminals.                                                                                                 |
| Move group                                          | `shiftGroupRelative`, accepted drag arrangement                                                                                          | Previous/next neighboring group or ungrouped Tab; pinned boundary and ends do not wrap. No pointer drag arrangement.                                                                                  |
| Collapse/expand                                     | Client view `collapsedGroupIds`, projected into `TabModel`                                                                               | Header press or menu; identity-keyed Client-local state. Native disclosure is not persisted and is retained for one context at a time.                                                                |
| New Tab in group / new group with Tab               | Enabled for local creation routes; remote grouped placement is rejected, and remote new-group creation is disabled                       | Implemented through one atomic `tab.create` placement; new Tabs activate after validated creation. GUI uses two model commits.                                                                        |
| New Tab to right/below                              | Local owner honors target group and index + 1, rejecting incompatible pinned-boundary placement                                          | Missing relative placement; ordinary Tab-menu New Tab navigates to its target and appends ungrouped. Group-menu creation uses group-tail placement.                                                   |
| Duplicate                                           | Selected invoked target expands to the selected set; local creation enables both single and multiple sources, remote routes disable both | One exact menu target, inserted at source index + 1 with fresh terminal identities; current selection is preserved. Selected-set duplication remains missing. No live split-tree or PTY clone.        |
| Close group / selected set / other or trailing Tabs | `onCloseTabs` delegates the exact ID set to `requestActiveTerminalTabsClose`; terminal owner supplies running-resource confirmation      | Implemented exact target rules and one complete-set confirmation over existing Close lifecycle; incremental settlement and preserved recovery. See [close sets](native-tab-close-sets.md).            |
| Move one Tab across a group boundary                | GUI model updates arrangement and grouping together                                                                                      | Tab menu moves left/right/first/last with GUI grouping semantics; collapsed neighbors are skipped using a local view hint. Legacy direction/index shortcuts retain their original ordering semantics. |
| Move to another window                              | Library callbacks/runtime factory are optional; current application hosts do not install the new-window callbacks                        | Not an admitted current-host parity claim. No new native window-transfer operation.                                                                                                                   |

Orientation, hover cards and vertical-strip preferences are host presentation choices. Relative
creation and selected-set duplication remain core gaps. Close deliberately removes only
the vacated source group when it becomes empty, preserving unrelated empty groups; GUI
`TabModel.closeTabs` prunes all empty groups. The [Close owner boundary](native-tab-close-sets.md#group-cleanup-boundary)
records this intentional exception. This implementation does not establish complete parity.

## Shared authority and receipts

`tabs.configure` is one closed operation on an existing `terminalContextId`: `create-group`,
`set-group`, `update-group`, `ungroup`, `set-pinned`, `move-group`, `create-group-for-tabs`,
`set-tabs-group`, `set-tabs-pinned` or `move-tab`. Set operations carry 1–256 unique Tab IDs and
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
set; an unselected background target configures itself. The affected count appears beside pinning,
group creation and removal actions. Menu proof captures the selected canonical members, so a changed
selection or a changed member invalidates the choice. Multi-selection renders `*` markers and
selected/total counts on group headers, including collapsed headers. Marking deliberately preserves
the active terminal, unlike GUI selection that can also activate the marked Tab.

Collapsed state remains Client-local, matching the authority in
`packages/workbench/src/workbench/document/workbench-client-view.ts`. Header disclosure does not
select a member, submit a command, advance a shared revision or hide members from the navigator.
Group colors are user-selected paired foreground/background chips; monochrome preserves names,
counts, disclosure and active markers. Names pass through the existing terminal-text sanitizer and
cell-width clipping. Pointer regions come from the same paint as the header and member tokens.

## Terminal creation and duplication

The group menu's **New Tab in Group**, the Tab/global menu's **New Group with Tab**, and the Tab
menu's **Duplicate Tab** use `WorkbenchAction::TabCreate` and the existing native `CreateOperation`.
Optional typed `placement` is `in-group { groupId }`, `new-group`, or `duplicate { tabId }`.
Omission keeps ordinary ungrouped/unpinned append. Placement and all created membership commit in
one revision; there is no create-then-configure window with noncontiguous groups.

In-group creation inserts after the group's final member (at the end for an empty group).
This matches the GUI group menu, not its separate **New Tab to Right/Below** action. That action
passes the target group and index + 1 through `executeWorkbenchTerminalCreate` to
`WorkbenchProjectWorkspaceRuntime.createTerminalInTarget`; `canInsertTerminalTab` verifies the
exact requested index and rejects incompatible placement instead of silently appending.
New-group creation appends one unpinned Tab in a fresh unnamed grey group and prunes empty groups,
matching `createNewTabGroup`; the group limit applies after that cleanup. Group labels admit
0–512 Unicode scalar values through `WorkbenchTabGroupLabel`, while Tab/Presentation labels stay
nonempty. Empty headers and menu targets display localized **Unnamed group** / **未命名分组**;
identity, color, count and disclosure remain usable without writing that fallback into shared state.

Duplicate addresses only the explicit menu Tab, including an inactive Tab, and never expands a
Client-local selected set. The admitted local GUI does expand a selected menu target to the selected
set: `getWorkbenchTabCreationAvailability` does not reject multiple sources, and `useTabStripActions`
iterates the live IDs through `TabModel.duplicateTab`. This is a remaining native gap.
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
then the same exact cleanup/install path. There is no multi-duplicate coordinator or all-or-nothing
batch claim.

## Compatibility and verification

Rust, TypeScript and the JSON command schema share the added wire cases. This is an extension of
the local unpublished OBWP v1 matched cohort: Client and Server activate/roll back together; older
closed decoders reject it. It adds no Desktop Workbench command or public CLI grammar. GUI edits
continue through the existing canonical shared-document proposal owner, not this native adapter.

`schemas/workbench/v1/tab-configuration-conformance-v1.json` runs against both GUI `TabModel` and
Rust `configure_tabs`. It covers pin/unpin, the pinned boundary, group assignment/removal/creation,
source cleanup, metadata updates, disjoint/mixed selected sets and whole-group/individual movement, including `[a:G,b:G,c:G]` creating a
new group from `b`. Movement also covers expanded/collapsed neighbors and first/last boundaries. Separate authority tests cover atomic prepare/drop/commit, exact replay, stale
revision and Close fences. Native wire, hostile-receipt and UI tests cover correlation, new IDs,
unrelated changes, selection, complete-draw gates, keyboard handoff and pointer disclosure/menus.

`tab-creation-conformance-v1.json` runs against GUI `TabModel`/`createNewTabGroup` and Rust
`create_terminal_tab`, including middle-group insertion, pinned/grouped duplication and empty-group
cleanup at capacity. A managed Workbench composition test witnesses the fresh single terminal and
unchanged source tree/selection. Authority tests cover prepare/drop, stale/missing targets, capacity,
pending/unknown replay and fingerprint conflicts. Native tests cover actual menu dispatch, exact
receipt tampering, selected identity, unknown provenance and Snapshot-only recovery.
`NativeClientCreateBarrier` adds a bounded Ready-origin duplicate variant: `DuplicateKeepsSelection`
and `FreshViewerUsesSelectedProvenance` separate membership success from selected-terminal readiness.

The composed TestBackend matrix covers both locales, 120×40, 80×24, 80×20, 40×10 and 20×5,
native/light/dark/reduced-color/monochrome themes, expanded/collapsed groups and pinned Tabs.
For temporary whole-screen evidence, set `ORCHBEE_TUI_CAPTURE_DIR` to an existing task-owned
directory and run `cargo test -p orchbee composed_screen`. It emits the actual buffer
cells for the default theme at standard and compact sizes, including group menus and selected-set menus. The invoking
task owns those captures and deletes them after their final reviewer is finished. Buffer evidence
does not verify physical font/palette behavior or live Desktop/native cross-runtime integration.
