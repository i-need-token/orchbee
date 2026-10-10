# Horizontal group drag correctness

This directory owns the executable evidence for moving a horizontal group header inside one Tab Strip. Local drag and grouping workflows use implementation tests; they have no separate TLA model or exhaustive interleaving claim.

## Evidence and mapping

| Evidence                  | Artifact relative to the package                                                                                                      | Claim and limit                                                                                                                                                 |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Contract                  | `docs/horizontal-group-drag.md`                                                                                                       | Source facts, state, events, invariants and observable checkpoints                                                                                              |
| Lean                      | `formal/lean/MoveContiguousBlock.lean`                                                                                                | Left/right contiguous movement preserves permutation, length and internal block order                                                                           |
| Property/refinement tests | `src/tab-strip/dragging/__tests__/horizontal-group-drag-machine.unit.test.ts`, `src/tab-strip/model/__tests__/tab-model.unit.test.ts` | Fixed-seed drag traces refine Lean normal forms; generated transitions cover identity, grouping, contiguity, selection, stale/rejected actions and cancellation |
| System traces             | `system-tests/horizontal-group-drag.spec.ts`                                                                                          | Pointer commit/cancel, expanded/collapsed groups, focus, overlay, semantic and painted order                                                                    |

Authoritative Tab order and group membership map to list input/output and DOM collection order. Collapsed intent maps to `aria-expanded`; active/selected/anchor identities map to semantic checkpoints and the visible active surface. Drag phase and visual member identity map to overlay/root presence and dragged items. Boundary moves compare Lean list normal forms; cancellation compares the exact captured semantic and painted snapshot. DOM focus remains adapter-owned.

## Reproduction

```sh
pnpm --filter @orchbee/tab-strip verify:correctness
pnpm --filter @orchbee/tab-strip test:property
pnpm --filter @orchbee/tab-strip test:lean
pnpm --filter @orchbee/tab-strip test:system
```

The Lean launcher verifies the published SHA-256 of elan 4.2.1, installs under the ignored package-local `.tools` directory and runs Lean 4.30.0. Browser tests use installed Google Chrome on macOS; `TAB_STRIP_BROWSER_EXECUTABLE` overrides its executable. Property seed is 20260721. System tests fix viewport, locale, color scheme, device scale, worker count and initial state, and wait for semantic/animation completion.

## Provenance limit

The migrated source is pinned to upstream commit `e4f73f6`; BSD attribution remains in `NOTICE.reference`. No portable reference-browser corpus has been captured against a separately pinned browser build. The evidence provides internal consistency and regression protection, not parity with every browser release.
