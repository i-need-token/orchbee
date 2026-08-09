# Testing

## Source and test paths

`orchbee/filename-naming-convention` enforces:

| Artifact                    | Naming contract                                                                                                                                                                     |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source directory            | Use kebab-case.                                                                                                                                                                     |
| Direct child of `locales/`  | Use the canonical BCP 47 identifier and casing, such as `en-US` or `zh-CN`.                                                                                                         |
| Unique runtime identity     | Count named runtime exports only; type-only exports and private implementation symbols do not change public identity.                                                               |
| Authored function module    | When its sole named runtime export is a locally defined function, use the exact camelCase symbol, such as `formatDisplayLabel.ts`.                                                  |
| Authored class module       | When its sole named runtime export is a locally defined class, use the exact PascalCase symbol, such as `EventEmitter.ts`.                                                          |
| General module              | Grouped operations, contracts, data catalogs, constants, re-export facades, and other modules without one callable or constructable identity use responsibility-focused kebab-case. |
| Generated source            | Keep the generator-owned kebab-case name.                                                                                                                                           |
| Authored function component | Use the exact PascalCase name of the sole runtime export, such as `FilterPanel.tsx`.                                                                                                |
| Authored dedicated Hook     | Use the exact camelCase name of the sole Hook export, such as `useFilterPanel.ts`.                                                                                                  |
| Non-component TSX entry     | Use kebab-case.                                                                                                                                                                     |
| Unit test                   | Use a sibling `__tests__` directory and preserve the source stem in `<source-name>.unit.test.ts` or `<source-name>.unit.test.tsx`.                                                  |

## Required unit tests

- `orchbee/require-unit-test`: Every `.ts` source file containing a top-level function, function expression, arrow function, or class must have its corresponding unit-test file.

| Scope                      | Contract                                                                                                                                                                                                                   |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Required sibling test      | Provides discoverability, not proof of coverage. Test observable outcomes, failure paths, and owned invariants; placeholders and implementation-only assertions do not qualify.                                            |
| `.tsx` source              | Tests are optional; add them for meaningful regressions, interactions, accessibility contracts, or integration boundaries, not prepared props or static markup.                                                            |
| Checked-in shadcn snapshot | Retains upstream implementations as reviewable source and follows the optional TSX policy. Test Orchbee adaptations, public-package contracts, and relied-upon integration behavior instead of copying the upstream suite. |
| Generated mobile Hook      | Exempt from per-file sibling tests.                                                                                                                                                                                        |

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

Every bug fix adds a test that fails before the fix and passes after it.

## Localization contracts

Product-message changes keep the source descriptors and every shipped catalog complete, validate
ICU syntax and variables, and render representative behavior under both `en-US` and `zh-CN`.
Locale negotiation tests cover supported variants, invalid input, unsupported languages, and
Traditional Chinese fallback. The authoritative message and visual-review requirements live in
[`docs/frontend/localization.md`](frontend/localization.md#verification).

## Native application tests

- Keep every Zig test and test-only helper under the owning source tree's `__tests__` directory,
  and expose every test root through the application's non-interactive `test` script. Zig-specific
  layout, naming, and white-box access are authoritative in [`docs/ZIG.md`](ZIG.md#testing).
- Compile every maintained operating-system family through `test:cross`; do not claim runtime,
  ABI, PTY, IPC, packaging, or installer behavior from cross-compilation alone.
- Run platform behavior and integration tests on their native operating system before claiming
  support for that behavior.
- Pin every native source dependency and compiler input used by required tests; machine-local
  sibling checkouts and mutable revisions are not test fixtures.

## Package contracts

Library packages run `test:exports` after `build` to verify their packed public surface, both JavaScript module formats, and a strict external TypeScript consumer. Cross-package behavior belongs in consumer-facing contract tests rather than tests that import another package's private source.
