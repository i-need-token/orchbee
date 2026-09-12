# Testing

## Source and test paths

`orchbee/filename-naming-convention` enforces:

| Artifact                    | Naming contract                                                                                                                                                                                                         |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source directory            | Use kebab-case.                                                                                                                                                                                                         |
| Direct child of `locales/`  | Use the canonical BCP 47 identifier and casing, such as `en-US` or `zh-CN`.                                                                                                                                             |
| Unique runtime identity     | Count named runtime exports only; type-only exports and private implementation symbols do not change public identity.                                                                                                   |
| Authored function module    | When its sole named runtime export is a locally defined function, use the exact camelCase symbol, such as `formatDisplayLabel.ts`.                                                                                      |
| Authored class module       | When its sole named runtime export is a locally defined class, use the exact PascalCase symbol, such as `EventEmitter.ts`.                                                                                              |
| General module              | Grouped operations, contracts, data catalogs, constants, re-export facades, and other modules without one callable or constructable identity use responsibility-focused kebab-case.                                     |
| Generated source            | Keep the generator-owned kebab-case name.                                                                                                                                                                               |
| Authored function component | Use the exact PascalCase name of the sole runtime export, such as `FilterPanel.tsx`.                                                                                                                                    |
| Authored dedicated Hook     | Use the exact camelCase name of the sole Hook export, such as `useFilterPanel.ts`.                                                                                                                                      |
| Non-component TSX entry     | Use kebab-case.                                                                                                                                                                                                         |
| Unit test                   | Use the owning module's `__tests__` directory and the `.unit.test.ts` or `.unit.test.tsx` suffix. A companion preserves its source stem; a suite covering several implementation files uses a kebab-case behavior name. |

## Required unit tests

Test obligations follow behavior, invariants, and risk in every authored language. There is no
one-to-one requirement between implementation files and test files. Splitting or moving an
implementation without changing its behavior does not create a new test obligation.

| Scope                      | Contract                                                                                                                                                                                                                   |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Domain behavior            | Cover decisions, state transitions, boundary inputs, expected failures, and owned invariants at the lowest layer that can judge them.                                                                                      |
| Cohesive module            | One behavior suite may cover several implementation files. Keep its owner discoverable through its location, imports, and behavior-focused test names; use an existing owner contract when the mapping is not apparent.    |
| Thin projection or wiring  | Reuse an existing behavior or integration test that detects a broken connection. Do not add a companion merely to assert an export's existence, a function's type, a copied constant, or the implementation's own formula. |
| `.tsx` source              | Add tests for meaningful regressions, interactions, accessibility contracts, or integration boundaries; prepared props and static markup alone do not require tests.                                                       |
| Checked-in shadcn snapshot | Test Orchbee adaptations, public-package contracts, and relied-upon behavior instead of copying the upstream suite or requiring one test per generated file.                                                               |

For each added or materially changed test, identify the realistic defect it would catch. File
presence, assertion count, and coverage percentages do not establish that value. Review this
obligation with the change's claims; the filename linter enforces naming and placement only.
Keep existing visibility boundaries when sharing a suite; test convenience does not permit
imports of another owner's private implementation.

When consolidating or removing tests, retain the distinct behavior, failure, and boundary evidence
they provide and identify the surviving suite. Do not retain placeholder companions or create a
second per-file coverage manifest solely to satisfy discoverability.

## Test layers and cost

| Layer                | Evidence and execution cost                                                                                                                                                                                            |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Deterministic core   | Exercise combinations, boundaries, and transition sequences with explicit inputs, clocks, and capabilities; do not start a browser, process, or service when the claim needs none.                                     |
| Adapter or component | Check translation, interaction, cleanup, and the owned external capability; use the smallest faithful environment. Pure TypeScript logic uses a Node environment, while DOM-dependent behavior uses a DOM environment. |
| Process or system    | Prove real startup, IPC, PTY, operating-system, and shutdown behavior. Keep the cross-process cases required by those claims; avoid repeating the full core decision matrix through every startup path.                |

Assess new and changed suites against these layers. A test's filename or runner target does not
make real process or filesystem work a cheap unit test. Shared fixtures may reuse immutable
preparation, but each test owns its mutable state and cleanup; fixture reuse must not make results
depend on execution order.

- Prefer observable completion and controllable time over unconditional sleeps. Use real elapsed
  time when the claim concerns an operating-system deadline; retain a justified upper bound.
- Explain why serialization or a concurrency limit is necessary and which resource it protects.
  More workers do not automatically reduce elapsed time; do not remove isolation, locks, or
  deadlines merely to obtain a faster run.
- Measure wall-clock duration separately from compilation, module loading, environment setup,
  and test-body time. Parallel worker totals are not additive wall-clock time. Compare equivalent
  inputs, machine load, and cold or warm caches before claiming an improvement.
- Maintain useful focused commands for core and expensive boundary tests. Full-gate scope,
  affected-test selection, and changes to required checks remain governed by
  [Verification](VERIFICATION.md); a slow test is not permission to skip required evidence.

## Test execution

- `vitest/expect-expect`: Every test must contain an assertion.
- `vitest/no-disabled-tests`: Do not commit skipped tests.
- `vitest/no-focused-tests`: Do not commit focused tests.
- `vitest/no-identical-title`: Keep test titles unique within their suite.

## Type-error tests

Tests may use `@ts-expect-error` only with a description of at least ten characters; `@ts-ignore` and `@ts-nocheck` remain forbidden.

Type assertions are allowed in tests for platform mocks and deliberately invalid boundary inputs; `any`, `oxlint-disable`, and `eslint-disable` remain forbidden.

Test-local helpers may rely on inferred return types.

```ts
// ❌
// @ts-ignore
acceptsOnlyStrings(1);

// ✅
// @ts-expect-error -- verifies that numbers are rejected
acceptsOnlyStrings(1);
```

## Regression coverage

Every bug fix adds or strengthens a regression case that fails before the fix and passes after it.
Extend the owning behavior suite when appropriate; a new file or duplicate higher-layer scenario
is not required. An already-failing case may supply this evidence when its failure and the fix's
effect are demonstrated.

## Localization contracts

Product-message changes keep the source descriptors and every shipped catalog complete, validate
ICU syntax and variables, and render representative behavior under both `en-US` and `zh-CN`.
Locale negotiation tests cover supported variants, invalid input, unsupported languages, and
Traditional Chinese fallback. The authoritative message and visual-review requirements live in
[`docs/frontend/localization.md`](frontend/localization.md#verification).

## Native application tests

- Keep Rust unit-test bodies in adjacent test-module files or the crate's `tests/` tree, expose only
  the narrow conditional module declaration from production source, and run every Cargo member
  through the root test gate. Rust-specific layout and failure-boundary rules are authoritative in
  [`docs/RUST.md`](RUST.md#testing).
- Keep every Zig test and test-only helper under the owning source tree's `__tests__` directory,
  and expose every test root through the application's non-interactive `test` script. Zig-specific
  layout, naming, and white-box access are authoritative in [`docs/ZIG.md`](ZIG.md#testing).
- Compile every maintained operating-system family through `test:cross`; do not claim runtime,
  ABI, PTY, IPC, packaging, or installer behavior from cross-compilation alone.
- Run platform behavior and integration tests on their native operating system before claiming
  support for that behavior.
- Pin every native dependency resolution and compiler input used by required tests; machine-local
  sibling checkouts and mutable revisions are not test fixtures.

## Package contracts

Library packages run `test:exports` after `build` to verify their packed public surface, both JavaScript module formats, and a strict external TypeScript consumer. Cross-package behavior belongs in consumer-facing contract tests rather than tests that import another package's private source.
