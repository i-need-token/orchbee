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

## Unit-test admission and removal

Do not add unit tests by default. This policy applies equally to Rust, Zig, and TypeScript.
Add a test only for a meaningful behavior risk that compiler checks, static analysis, or existing
evidence cannot adequately judge. Identify the realistic defect and the uncovered claim in the
existing review; no separate test-admission document is required. Implementation files, functions,
and test files have no one-to-one relationship. Splitting or moving unchanged behavior creates no
new test obligation.

Simple constants, direct forwarding, field assembly, elementary arithmetic, static presentation,
and framework or upstream-library behavior do not normally justify dedicated tests. An owned
compatibility contract or demonstrated regression can justify an exception. Judge the failure
consequence and evidence gap, not source length or ease of writing a test.

Reuse existing behavior, contract, integration, or visual evidence before adding cases. Algorithms,
state transitions, persistence, protocol compatibility, security boundaries, concurrency, and
recovery retain automated evidence for their relevant decisions and failure modes. Choose the
lowest-cost faithful layer; a verification obligation does not imply a new unit-test file.

| Scope                      | Contract                                                                                                                                                                                                                   |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Domain behavior            | Cover decisions, state transitions, boundary inputs, expected failures, and owned invariants at the lowest layer that can judge them.                                                                                      |
| Cohesive module            | One behavior suite may cover several implementation files. Keep its owner discoverable through its location, imports, and behavior-focused test names; use an existing owner contract when the mapping is not apparent.    |
| Thin projection or wiring  | Reuse an existing behavior or integration test that detects a broken connection. Do not add a companion merely to assert an export's existence, a function's type, a copied constant, or the implementation's own formula. |
| `.tsx` source              | Add tests for meaningful regressions, interactions, accessibility contracts, or integration boundaries; prepared props and static markup alone do not require tests.                                                       |
| Checked-in shadcn snapshot | Test Orchbee adaptations, public-package contracts, and relied-upon behavior instead of copying the upstream suite or requiring one test per generated file.                                                               |

File presence, assertion count, and coverage percentages do not establish test value. The filename
linter enforces naming and placement only; it does not require companion tests.
Derive expected outcomes from requirements, invariants, or an independent reference. Do not copy
the production algorithm into the assertion or update expected values solely to match new output.
Keep existing visibility boundaries when sharing a suite; test convenience does not permit
imports of another owner's private implementation.

Apply the same admission rule to existing tests. Remove obsolete cases, redundant companions, and
assertions that merely restate low-risk implementation details. Consolidate duplicate cases around
their behavior owner. Identify the surviving check for a protected claim, or explain why the
trivial behavior needs no dedicated test; deleting a low-value test does not require a replacement.
Preserve distinct meaningful behavior, failure, boundary, and regression evidence. A slow or failing
test is not obsolete for that reason. Do not create a per-file coverage manifest or retain empty
test roots, unused fixtures, or imports after removing their last consumer.

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

For an automatable behavior defect, add or strengthen a regression case that fails before the fix
and passes after it. An existing failing test, compiler check, or contract check can supply that
evidence when its failure and the fix's effect are demonstrated; do not duplicate it at another layer.

For a visual or environment-dependent defect that cannot be judged faithfully by a stable automated
check, use a controlled visual, integration, or reproduction check. State the observed failure,
checked result, environment, and automation limit in the existing review record. Protect any
separable domain decision with an automated test. Difficulty, cost, or author confidence alone does
not waive automation for state, data integrity, protocol, security, or recovery claims.

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
- Use explicit `test:cross` for release or platform validation of every maintained operating-system family; ordinary host tests do not invoke it. Do not claim runtime,
  ABI, PTY, IPC, packaging, or installer behavior from cross-compilation alone.
- Run platform behavior and integration tests on their native operating system before claiming
  support for that behavior.
- Pin every native dependency resolution and compiler input used by required tests; machine-local
  sibling checkouts and mutable revisions are not test fixtures.

## Package contracts

Library packages run `test:exports` after `build` to verify their packed public surface, both JavaScript module formats, and a strict external TypeScript consumer. Cross-package behavior belongs in consumer-facing contract tests rather than tests that import another package's private source.
