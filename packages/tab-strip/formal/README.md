# Correctness pilot

This directory is the first executable slice of the package correctness contract. Its scope is deliberately narrow: moving a horizontal group header inside one tab strip. It proves local claims about the migrated model and implementation; it does not claim complete equivalence with every version of the upstream browser.

## Evidence layers

| Layer                     | Artifact                                                                                                                              | What it establishes                                                                                                                                                       |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Behavior model            | `docs/horizontal-group-drag.md`                                                                                                       | Scope, source facts, state, events, invariants, and observable checkpoints                                                                                                |
| TLA+                      | `formal/tla/HorizontalGroupDrag.tla`                                                                                                  | The reachable six-tab graphs satisfy identity, membership, contiguity, whole-block movement, selection-anchor, cancellation, rejection, and drag-lifecycle invariants     |
| Lean                      | `formal/lean/MoveContiguousBlock.lean`                                                                                                | Moving a contiguous block left or right preserves the element permutation, total length, and the block's internal order                                                   |
| Refinement/property tests | `src/tab-strip/dragging/__tests__/horizontal-group-drag-machine.unit.test.ts`, `src/tab-strip/model/__tests__/tab-model.unit.test.ts` | Fixed-seed drag scenarios refine the Lean normal forms; generated model transitions own grouping, contiguity, identity, and member-order evidence                         |
| System traces             | `system-tests/horizontal-group-drag.spec.ts`                                                                                          | Deterministic browser-process pointer traces check expanded and collapsed commit, cancellation, overlay composition, selection, collection order, and final painted order |

Paths in this table are relative to the package root.

## State mapping

| Behavior concept        | TypeScript                                           | TLA+                                                                                  | Lean/system projection                                                                             |
| ----------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Authoritative tab order | `authoritative.tabs`                                 | exact six-identity `tabs` sequence plus `previousAuthoritative`                       | List input/output and horizontal DOM collection order                                              |
| Group membership        | `authoritative.tabs[].group`                         | per-tab `groupOf`, known-group and contiguity invariants                              | Generated model membership plus Lean member-order preservation                                     |
| Collapsed intent        | `authoritative.groups[].collapsed`                   | `collapsed` and `originalCollapsed`                                                   | `aria-expanded` at checkpoints                                                                     |
| Active/selection        | `activeTabId`, `selectedTabIds`, `selectionAnchorId` | `active`, `selected`, `selectionAnchor`, `recordedActive`, and derived active surface | Visible active tab or collapsed group-header DOM, selected identity, and test-fixture page heading |
| Drag phase              | `drag.kind`                                          | `phase`                                                                               | Overlay/root presence: armed, dragging, settling, idle                                             |
| Whole-group visual      | `visualKind` and `memberIds`                         | `visual` plus the exact ordered Research member block                                 | Ghost header and dragged tab identities                                                            |
| Boundary move           | `move-to(preRemovalTargetIndex)`                     | three bounded legal source-block positions and unchanged outsider order               | Lean left/right list normal form and final DOM/model order                                         |
| Cancellation            | `cancel` plus captured `original`                    | `CancelDrag` restores the six authoritative fields, including `selectionAnchor`       | Exact pre-drag semantic and painted snapshot                                                       |

The TLA+ model contains one pinned tab, two two-member groups, one ungrouped tab, and one dragged source group. It explores every initial collapsed subset and active identity, three source-block positions, and one source-membership invalidation between arm and activation. Named stale, missing, repeated, out-of-phase, and invalid-target inputs witness preservation of authoritative state and command output. It assumes no fairness for pointer activation, release, cancellation, or settling. Lean and generated property tests extend movement preservation to arbitrary valid member lists.

DOM focus remains adapter-owned. `system-tests/horizontal-group-drag.spec.ts` checks the focused collapsed-header projection; the Escape trace checks cancellation's semantic and painted restoration without assigning DOM focus to the TLA+ state. The deleted `TabGrouping` model only selected one pre-enumerated contiguous set and left tab order constant, so it added no maintained behavior beyond the stronger generated `TabModel` transition and invariant suite.

## Reproduction

Run the complete slice:

```sh
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip verify:correctness
```

Or run one evidence layer:

```sh
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:property
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:tla
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:lean
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:system
```

Tool versions are pinned. The repository TLA+ runner downloads `tla2tools 1.7.4` and verifies its pinned SHA-256; set `ORCHBEE_TLA2TOOLS_JAR` to use an already provisioned matching copy. The Lean launcher downloads `elan 4.2.1`, verifies its published SHA-256, installs it under the ignored package-local `.tools` directory, and executes Lean `4.30.0`. The browser suite uses the installed Google Chrome on macOS; set `TAB_STRIP_BROWSER_EXECUTABLE` to override the executable.

The property suite uses seed `20260721`. TLC uses a fixed fingerprint polynomial and one worker. The system suite fixes the viewport, locale, color scheme, device scale, browser worker count, and initial fixture state; it waits on semantic and animation completion instead of arbitrary sleeps.

## Provenance limit

The migrated implementation is pinned to the read-only upstream source repository at commit `e4f73f6` (`feat: harden Chromium tab interaction fidelity`), and the BSD attribution is retained in `NOTICE.reference`. The reference-browser trace corpus has not been captured as a portable artifact against a separately pinned browser build. The pilot therefore establishes internal consistency and regression protection for this migration; it does not prove parity with every browser release.
