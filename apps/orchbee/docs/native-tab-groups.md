# Native Tab groups and capability coverage

The Windows native TUI projects canonical group identities, names, colors, membership and pinned
state. The compact strip renders a separate group header, disclosure marker and member count.
The Tab menu exposes pin/unpin, grouping and the existing same-context Tab movement actions;
the group menu exposes create, assign, remove, rename, color, ungroup and adjacent group movement.
Keyboard menu navigation and painted pointer targets invoke the same owners. Configuration of a
background Tab keeps the current Tab, Panel, Presentation, terminal binding and viewer selected.

## Actual host capability comparison

This comparison concerns the admitted Workbench composition, not everything exported by the
reusable Tab Strip. The GUI owners are `WorkbenchTabBar`, `useWorkbench`,
`getWorkbenchTabCreationAvailability`, `WorkbenchProjectWorkspaceRuntime` and `TabModel`.
Native entries refer to the Windows TUI. This is a capability slice, not complete GUI/TUI parity.

| Core operation                                      | Admitted GUI path                                                                                                                   | Native coverage and remaining gap                                                                                                                                                                  |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Function menus                                      | Tab, group, Panel and Workbench Action menus                                                                                        | Existing object/global menus; Tab menu now includes movement, pinning and groups. Presentation and anchoring differ by host.                                                                       |
| Create, rename, close one Tab                       | Terminal owner creates/closes; title action edits canonical name                                                                    | Existing canonical native operations; grouped/pinned metadata remains representable.                                                                                                               |
| Pin/unpin                                           | Selected Tab set through `TabModel.setTabsPinned`                                                                                   | One addressed Tab; moves at the pinned boundary, removes group membership when pinning. Multi-selection is missing.                                                                                |
| Create group from existing Tabs                     | Selected set through `createGroup`                                                                                                  | One addressed Tab, server-allocated group identity, name editor and color menu. Multi-selection is missing.                                                                                        |
| Assign/remove group membership                      | Selected set through `moveTabsToGroup`                                                                                              | One addressed Tab; joining a group unpins, source empties are removed, groups stay contiguous.                                                                                                     |
| Rename/recolor/ungroup                              | Group menu through `updateGroup`/`ungroup`                                                                                          | Implemented for the complete canonical group, including all nine colors. Ungroup preserves terminals.                                                                                              |
| Move group                                          | `shiftGroupRelative`, accepted drag arrangement                                                                                     | Previous/next neighboring group or ungrouped Tab; pinned boundary and ends do not wrap. No pointer drag arrangement.                                                                               |
| Collapse/expand                                     | Client view `collapsedGroupIds`, projected into `TabModel`                                                                          | Header press or menu; identity-keyed Client-local state. Native disclosure is not persisted and is retained for one context at a time.                                                             |
| New Tab in group / new group with Tab               | Enabled for local creation routes; remote grouped placement is rejected, and remote new-group creation is disabled                  | Missing native menu route and orchestration. GUI new-group creation itself adds a Tab, then groups it in a second commit.                                                                          |
| Duplicate                                           | Enabled for local creation routes; disabled for remote routes                                                                       | Missing native duplicate command. GUI adds one Tab with source metadata at index + 1, without cloning a live split tree.                                                                           |
| Close group / selected set / other or trailing Tabs | `onCloseTabs` delegates the exact ID set to `requestActiveTerminalTabsClose`; terminal owner supplies running-resource confirmation | Missing native batch/group close operation and its complete-set confirmation/recovery. Individual Close is not a substitute for this capability.                                                   |
| Move one Tab across a group boundary                | GUI model updates arrangement and grouping together                                                                                 | Existing native direction/index movement rejects an order that would split a group or violate the pinned prefix. Explicit assignment plus movement is available, but the gesture semantics differ. |
| Move to another window                              | Library callbacks/runtime factory are optional; current application hosts do not install the new-window callbacks                   | Not an admitted current-host parity claim. No new native window-transfer operation.                                                                                                                |

Orientation, hover cards and vertical-strip preferences are host presentation choices. The missing
batch selection, terminal creation/duplication and group-close lifecycle paths above remain core
gaps; this implementation does not close the whole parity issue.

## Shared authority and receipts

`tabs.configure` is one closed operation on an existing `terminalContextId`: `create-group`,
`set-group`, `update-group`, `ungroup`, `set-pinned` or `move-group`. The existing authenticated
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

Collapsed state remains Client-local, matching the authority in
`packages/workbench/src/workbench/document/workbench-client-view.ts`. Header disclosure does not
select a member, submit a command, advance a shared revision or hide members from the navigator.
Group colors are user-selected paired foreground/background chips; monochrome preserves names,
counts, disclosure and active markers. Names pass through the existing terminal-text sanitizer and
cell-width clipping. Pointer regions come from the same paint as the header and member tokens.

## Compatibility and verification

Rust, TypeScript and the JSON command schema share the added wire cases. This is an extension of
the local unpublished OBWP v1 matched cohort: Client and Server activate/roll back together; older
closed decoders reject it. It adds no Desktop Workbench command or public CLI grammar. GUI edits
continue through the existing canonical shared-document proposal owner, not this native adapter.

`schemas/workbench/v1/tab-configuration-conformance-v1.json` runs against both GUI `TabModel` and
Rust `configure_tabs`. It covers pin/unpin, the pinned boundary, group assignment/removal/creation,
source cleanup, metadata updates and whole-group movement, including `[a:G,b:G,c:G]` creating a
new group from `b`. Separate authority tests cover atomic prepare/drop/commit, exact replay, stale
revision and Close fences. Native wire, hostile-receipt and UI tests cover correlation, new IDs,
unrelated changes, selection, complete-draw gates, keyboard handoff and pointer disclosure/menus.

The composed TestBackend matrix covers both locales, 120×40, 80×24, 80×20, 40×10 and 20×5,
native/light/dark/reduced-color/monochrome themes, expanded/collapsed groups and pinned Tabs.
For temporary whole-screen evidence, set `ORCHBEE_TUI_CAPTURE_DIR` to an existing task-owned
directory and run `cargo test -p orchbee tab_groups_composed_screen`. It emits the actual buffer
cells for the default theme at standard and compact sizes, including group menus. The invoking
task owns those captures and deletes them after their final reviewer is finished. Buffer evidence
does not verify physical font/palette behavior or live Desktop/native cross-runtime integration.
