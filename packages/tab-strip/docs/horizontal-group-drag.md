# Horizontal group-drag contract

## Scope

This contract owns one behavior: dragging a horizontal tab-group header within one tab strip. It
covers activation, temporary backing-model changes, whole-group reordering, selection,
cancellation, the dragged visual, settling, and boundaries between adjacent groups.

The behavior baseline is the read-only upstream implementation at commit `e4f73f6` together with
the tests and formal artifacts migrated into this package. This contract records the behavior that
the package preserves; it does not make a general parity claim for later browser releases.

The following behavior is outside this model:

- individual or multi-tab dragging;
- pinned-tab dragging;
- vertical strips;
- cross-window detach and attach;
- touch long-press and context-menu behavior; and
- exact color, typography, and non-drag hover styling.

## Upstream reference facts

The following facts come from the Chromium source and tests used as the migration baseline:

| Fact                                                                                                                         | Upstream source evidence                                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| A group-header drag records the active tab's index within the group, or index zero when the active tab is outside the group. | `dragging/tab_drag_controller.cc`, construction of `GroupHeaderDragData`                                  |
| Attach selects every dragged tab and keeps the recorded group tab active.                                                    | `dragging/tab_drag_controller.cc`, `AttachImpl` and `ResetSelection`                                      |
| With `kCollapseTabGroupDuringDrag`, an initially expanded group is temporarily collapsed in the backing model.               | `common/tab_drag_handler.cc`, `StartedDragging`                                                           |
| A group already collapsed before dragging remains collapsed.                                                                 | `dragging/tab_drag_controller_interactive_uitest.cc`, `DragCollapsedGroupHeaderRemainsCollapsed`          |
| A header drag moves the group as one block through `MoveGroupTo`.                                                            | `dragging/dragging_tabs_session.cc`, attached move path                                                   |
| A group cannot stop inside another group; it stays put until the dragged block crosses the neighboring group's far boundary. | `DragGroupHeaderRightAvoidsOtherGroups` and `DragGroupHeaderLeftAvoidsOtherGroups`                        |
| Stopping a group-header drag restores an expanded group that was collapsed only for the drag.                                | `common/tab_drag_handler.cc`, `StoppedDragging`                                                           |
| Stopping a group-header drag reduces selection to the recorded active tab in the group.                                      | `common/tab_drag_handler.cc`, `StoppedDragging`; `dragging/tab_drag_controller.cc`, final selection reset |

The migrated presentation also preserves these recorded observations:

- an initially expanded group remains a full composite dragged visual containing its header and
  tabs;
- non-dragged tabs and groups move out of the dragged visual's target space instead of being
  painted underneath it;
- no insertion indicator appears at a location where the group cannot be committed; and
- release settles the full dragged visual into the final group bounds without a duplicate header,
  underline, or active outline.

## State

The semantic state contains:

| Field            | Meaning                                                                         |
| ---------------- | ------------------------------------------------------------------------------- |
| `tabs`           | Ordered tab identities with pinned and group membership facts                   |
| `groups`         | Known groups and their user-visible collapsed intent                            |
| `activeTabId`    | The one active tab, or no tab only when the strip is empty                      |
| `selectedTabIds` | The selected identities                                                         |
| `drag`           | `idle`, `armed`, `dragging`, or `settling` plus the data required by that phase |

The visible active surface is derived. An active ungrouped tab or member of an expanded group
projects to that tab. An active member of a collapsed group projects to its group header while the
member's `activeTabId` remains authoritative. The header uses the same active-tab shell as an
ordinary active tab; the colored group chip is content inside that shell.

An armed or active group drag records:

- the group identity and ordered member identities;
- the tab active in the group, or the first member when the active tab is outside it;
- the complete pre-drag authoritative state needed by cancellation;
- whether the group was collapsed before dragging;
- whether the backing model was collapsed only for the drag;
- whether the dragged visual is an `expanded-group` or `collapsed-header`;
- the latest valid pre-removal target index; and
- the logical settle destination.

Pointer coordinates, DOM nodes, timers, and animation objects are not semantic state. Adapters
convert them into target observations, logical clock events, and render commands.

## Events

| Event          | Required data                  | Meaning                                                                                                                                        |
| -------------- | ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `ArmGroup`     | Group identity                 | Record a possible header drag without changing authoritative state                                                                             |
| `ActivateDrag` | None                           | Cross the activation threshold and enter the drag loop                                                                                         |
| `MoveTo`       | Valid pre-removal target index | Preview and apply a whole-group reorder                                                                                                        |
| `Commit`       | None                           | Keep the final order and restore post-drag collapse and selection semantics                                                                    |
| `Cancel`       | None                           | Restore the complete pre-drag authoritative state                                                                                              |
| `Settle`       | None                           | End the release animation after the final logical frame                                                                                        |
| `ToggleGroup`  | Group identity                 | Collapse or expand the group without changing an active member's identity; collapse changes its visible surface from child tab to group header |

Repeated or out-of-phase events are explicit no-ops. An event referencing a missing group, missing
member, or invalid target must not partially mutate state.

## Invariants

These names are stable evidence identifiers:

| Claim                               | Required truth                                                                                                                |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `UniqueTabIdentity`                 | No two tabs have the same identity                                                                                            |
| `KnownGroupMembership`              | Every non-null tab group references a known group                                                                             |
| `PinnedPrefix`                      | Pinned tabs form one contiguous prefix and are not grouped                                                                    |
| `GroupContiguous`                   | Every group's members occupy one contiguous range                                                                             |
| `ActiveTabExists`                   | A non-null active identity exists in `tabs`                                                                                   |
| `StableActiveSurfaceExists`         | Outside temporary `dragging`, the active identity has a visible tab or collapsed group-header surface                         |
| `CollapsedActiveUsesGroupHeader`    | If a collapsed group contains the active tab, its header is the active surface while the child identity remains authoritative |
| `SelectionExists`                   | Every selected identity exists in `tabs`                                                                                      |
| `DragSourceExists`                  | Every armed, dragging, or settling group and member still exists                                                              |
| `GroupMovePermutation`              | Moving a group preserves the complete ordered set of tab identities up to permutation                                         |
| `GroupMoveAtomic`                   | A group-header move preserves membership and moves the entire group as one block                                              |
| `GroupTargetBoundary`               | A committed group boundary never splits another group                                                                         |
| `CollapsedDragStaysCompact`         | A group collapsed before activation remains collapsed and uses one compact header visual                                      |
| `ExpandedDragKeepsFullVisual`       | A group expanded before activation uses a full header-plus-tabs visual while its backing model is temporarily collapsed       |
| `CompletionRestoresCollapsedIntent` | Commit restores the exact pre-drag collapsed intent                                                                           |
| `CompletionSelectsRecordedActive`   | Commit leaves only the recorded group member selected and active; a collapsed result projects it through the header           |
| `CancelRestoresAuthoritativeState`  | Cancel restores order, membership, collapse, active, selection, anchor, and focus facts                                       |
| `NoInvalidDropFeedback`             | The observable projection exposes no drop location that `MoveTo` would reject                                                 |
| `SettledVisualsDoNotOverlap`        | At settle, distinct collection slots have no undeclared occupied-bounds overlap                                               |

The last two claims require deterministic geometry traces. The remaining claims are owned by the
semantic transition and can be checked without a browser.

## Transition contract

| Phase                 | Event          | Guard                                                     | Next phase | Semantic result                                                                                      |
| --------------------- | -------------- | --------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------- |
| `idle`                | `ArmGroup`     | Group exists and has members                              | `armed`    | Capture the pre-drag state and recorded active member                                                |
| `armed`               | `ActivateDrag` | Captured members still form the group                     | `dragging` | Select dragged members, activate the recorded member, and optionally collapse only the backing model |
| `dragging`            | `MoveTo`       | Target is a collection boundary outside the dragged block | `dragging` | Move the complete group block atomically                                                             |
| `dragging`            | `Commit`       | Structural invariants hold                                | `settling` | Restore collapsed intent and retain the recorded active member                                       |
| `armed` or `dragging` | `Cancel`       | Always                                                    | `idle`     | Restore the captured authoritative state                                                             |
| `settling`            | `Settle`       | Logical settle duration completed                         | `idle`     | Remove the dragged projection without changing authoritative state                                   |
| `idle`                | `ToggleGroup`  | Group exists                                              | `idle`     | Toggle collapse while preserving active identity and remapping only its visible surface              |

`ActivateDrag` may temporarily change the backing group's collapsed value to remove the original
collection from layout. This is a projection mechanism, not a change in the user's collapsed
intent, so both facts remain explicit in state.

## Observable checkpoints

| Checkpoint            | Required observation                                                                                                       |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `armed`               | No authoritative order or collapse change; no dragged overlay                                                              |
| `activated-expanded`  | Full group overlay exists; backing slot is removed or compacted; neighboring slots have valid non-overlapping destinations |
| `activated-collapsed` | One compact header overlay exists; group remains collapsed; recorded member is active                                      |
| `invalid-target`      | Order and last valid target remain unchanged; no commit indicator appears at the invalid point                             |
| `valid-target`        | Whole-group order and displaced-slot destinations agree with the same collection boundary                                  |
| `cancelled`           | Pre-drag authoritative snapshot and ordinary rendering are restored                                                        |
| `committed`           | Final order is valid; collapsed intent is restored; the recorded member remains active                                     |
| `settled`             | Drag overlay is absent and occupied slot bounds contain no undeclared overlap                                              |

## Evidence map

| Claim family                               | TLA+                    | Lean                             | Property tests      | Deterministic system trace       | Upstream evidence                       |
| ------------------------------------------ | ----------------------- | -------------------------------- | ------------------- | -------------------------------- | --------------------------------------- |
| Structural identities and membership       | Required                | `moveGroup` preservation         | Required            | Checkpoints                      | Source model behavior                   |
| Whole-group movement and target boundaries | Required                | Permutation and contiguity       | Required            | Left and right traces            | Interactive UI tests                    |
| Collapse and selection lifecycle           | Required                | Not applicable: stateful policy  | Required            | Expanded and collapsed traces    | Drag handler/controller source          |
| Cancellation                               | Required                | Optional restoration lemma       | Required            | Escape and pointer-cancel traces | Controller revert path                  |
| Dragged visual and displacement            | Abstract ownership only | Not applicable: geometry adapter | Geometry properties | Required                         | Source layout plus recorded observation |
| Settle and non-overlap                     | Abstract release phase  | Not applicable: geometry adapter | Geometry properties | Required                         | Recorded observation                    |

## Implementation bridge

`src/dragging/horizontal-group-drag-machine.ts` owns group-drag phase, selection policy, temporary
backing collapse, whole-group movement, cancellation restoration, and visual kind. The React
integration may measure geometry and execute commands but must not independently choose those
semantics.

`src/dragging/horizontal-group-drag-model-adapter.ts` applies transition commands to `TabModel` and
asserts that its canonical snapshot refines the pure state after every event. The active identity
and visible presentation remain separate facts: collapsing an active group does not transfer
activation outside that group.

Every counterexample is serialized as initial canonical state plus semantic events. DOM
coordinates may be attached as system-test input, but the regression identity is the shortest
semantic sequence that violates a named claim.

## Completion criteria

This behavior slice remains complete only while:

- the pure transition is the only owner of scoped semantic decisions;
- TLC checks every applicable invariant in the bounded model;
- Lean checks group-move preservation without admitted obligations;
- fixed-seed property tests cover directions, adjacent groups, collapsed and expanded sources,
  commit, cancel, repeated events, and invalid targets;
- deterministic browser traces cover every observable checkpoint;
- mappings among TypeScript, TLA+, Lean, and trace schemas remain documented; and
- the package's unit tests, build, export contract, and repository verification continue to pass.
