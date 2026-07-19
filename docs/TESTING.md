# Testing

## Source and test paths

- `orchbee/filename-naming-convention`: Use kebab-case source paths. A directory immediately under
  `locales/` is the exception: name it with the canonical BCP 47 locale identifier, including
  canonical casing such as `en-US` and `zh-CN`. Place unit tests in the source file's sibling
  `__tests__` directory and name them `<source-name>.unit.test.ts` or
  `<source-name>.unit.test.tsx`.

## Required unit tests

- `orchbee/require-unit-test`: Every `.ts` source file containing a top-level function, function expression, arrow function, or class must have its corresponding unit-test file.

The sibling-file rule is a minimum discoverability requirement, not evidence of behavioral coverage. Tests exercise observable outcomes, failure paths, and owned invariants; a placeholder test or a test that only repeats implementation details does not satisfy the project contract.

Tests are optional for `.tsx` source. Do not add a component test merely to repeat prepared props or static markup; add one when it protects a meaningful regression, interaction, accessibility contract, or integration boundary.

The checked-in shadcn registry snapshot follows the same optional TSX policy, and its generated mobile Hook is exempt from per-file sibling tests. They retain upstream implementations as reviewable source; test Orchbee adaptations, public-package contracts, and relied-upon integration behavior without copying the upstream component suite component by component.

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

## Package contracts

Library packages run `test:exports` after `build` to verify their packed public surface, both JavaScript module formats, and a strict external TypeScript consumer. Cross-package behavior belongs in consumer-facing contract tests rather than tests that import another package's private source.
