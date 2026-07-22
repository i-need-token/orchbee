# Correctness pilot

This directory is the first executable slice of the package correctness contract. Its scope is
deliberately narrow: moving a horizontal group header inside one tab strip. It proves local claims
about the migrated model and implementation; it does not claim complete equivalence with every
version of the upstream browser.

## Evidence layers

| Layer                     | Artifact                                                            | What it establishes                                                                                                                                                                   |
| ------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Behavior model            | `docs/horizontal-group-drag.md`                                     | Scope, source facts, state, events, invariants, and observable checkpoints                                                                                                            |
| TLA+                      | `formal/tla/HorizontalGroupDrag.tla`                                | The reachable abstract graph satisfies lifecycle, boundary, active-surface, collapse, selection, cancellation, and feedback invariants; fair interactions eventually return to `idle` |
| Lean                      | `formal/lean/MoveContiguousBlock.lean`                              | Moving a contiguous block left or right preserves the element permutation, total length, and the block's internal order                                                               |
| Refinement/property tests | `src/dragging/__tests__/horizontal-group-drag-machine.unit.test.ts` | Fixed-seed scenarios compare the TypeScript block move to the Lean normal forms and compare semantic transitions to the mutable model                                                 |
| System traces             | `system-tests/horizontal-group-drag.spec.ts`                        | Deterministic browser-process pointer traces check expanded and collapsed commit, cancellation, overlay composition, selection, collection order, and final painted order             |

Paths in this table are relative to the package root.

## State mapping

| Behavior concept        | TypeScript                                              | TLA+                                                                       | Lean/system projection                                                                                 |
| ----------------------- | ------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Authoritative tab order | `authoritative.tabs`                                    | `order` at collection granularity                                          | List input/output and horizontal DOM collection order                                                  |
| Collapsed intent        | `authoritative.groups[].collapsed`                      | `collapsed` and `originalCollapsed`                                        | `aria-expanded` at checkpoints                                                                         |
| Active/selection        | `activeTabId`, `selectedTabIds`, derived active surface | `active`, `selected`, `recordedActive`, `ActiveSurface`, `VisibleSurfaces` | Visible active tab or collapsed group-header DOM, selected tab identity, and test-fixture page heading |
| Drag phase              | `drag.kind`                                             | `phase`                                                                    | Overlay/root presence: armed, dragging, settling, idle                                                 |
| Whole-group visual      | `visualKind` and `memberIds`                            | `visual` abstraction                                                       | Ghost header and dragged tab identities                                                                |
| Boundary move           | `move-to(preRemovalTargetIndex)`                        | Adjacent collection moves                                                  | Lean left/right list normal form and final DOM/model order                                             |
| Cancellation            | `cancel` plus captured `original`                       | `CancelDrag`                                                               | Exact pre-drag observable snapshot                                                                     |

TLA+ deliberately represents a tab group as one collection node. Lean and the TypeScript
refinement tests discharge the lower-level claim that the node's member list is neither split nor
reordered. The browser trace checks that the final painted order agrees with the semantic
collection order.

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

Tool versions are pinned. The TLA+ launcher downloads `tla2tools 1.7.4` and verifies its
published SHA-1. Set `TAB_STRIP_TLA2TOOLS_JAR` to use an already provisioned copy with the same
digest. The Lean launcher downloads `elan 4.2.1`, verifies its published SHA-256, installs it under
the ignored package-local `.tools` directory, and executes Lean `4.30.0`. The browser suite uses
the installed Google Chrome on macOS; set `TAB_STRIP_BROWSER_EXECUTABLE` to override the executable.

The property suite uses seed `20260721`. TLC uses a fixed fingerprint polynomial and one worker.
The system suite fixes the viewport, locale, color scheme, device scale, browser worker count, and
initial fixture state; it waits on semantic and animation completion instead of arbitrary sleeps.

## Provenance limit

The migrated implementation is pinned to the read-only upstream source repository at commit
`e4f73f6` (`feat: harden Chromium tab interaction fidelity`), and the BSD attribution is retained
in `NOTICE.reference`. The reference-browser trace corpus has not been captured as a portable
artifact against a separately pinned browser build. The pilot therefore establishes internal
consistency and regression protection for this migration; it does not prove parity with every
browser release.
