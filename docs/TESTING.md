# Testing

## Source and test paths

- `orchbee/filename-naming-convention`: Use kebab-case source paths; place unit tests in the source file's sibling `__tests__` directory and name them `<source-name>.unit.test.ts` or `<source-name>.unit.test.tsx`.

## Required unit tests

- `orchbee/require-unit-test`: Every `.ts` or `.tsx` source file containing a top-level function, function expression, arrow function, or class must have its corresponding unit-test file.

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
