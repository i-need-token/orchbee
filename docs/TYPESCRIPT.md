# TypeScript

## Compiler contract

| Option                       | Requirement                                                                       |
| ---------------------------- | --------------------------------------------------------------------------------- |
| `strict`                     | Treat all implicit uncertainty as a type error                                    |
| `noUncheckedIndexedAccess`   | Handle missing indexed values explicitly                                          |
| `exactOptionalPropertyTypes` | Distinguish an absent property from an explicit `undefined`                       |
| `erasableSyntaxOnly`         | Use JavaScript-runtime syntax; represent enum-like values with `as const` objects |
| `verbatimModuleSyntax`       | Keep runtime and type-only imports explicit                                       |
| `moduleResolution: nodenext` | Use `.js` extensions in relative TypeScript imports                               |

## Type escape hatches

- `typescript/no-explicit-any`: Use a concrete type or `unknown` followed by validation or narrowing.
- `typescript/no-empty-object-type`: Do not use `{}` or empty interfaces as broad placeholder types.
- `typescript/no-unsafe-function-type`: Use an explicit function signature instead of `Function`.
- `typescript/no-wrapper-object-types`: Use primitive `string`, `number`, `boolean`, `symbol`, and `bigint` types.
- `typescript/consistent-type-assertions`: Do not use type assertions in production code.
- `typescript/no-non-null-assertion`: Prove that a value exists instead of appending `!`.
- `typescript/ban-ts-comment`: Do not suppress TypeScript diagnostics in production code.

## Public boundaries

- `typescript/explicit-module-boundary-types`: Declare return types for exported functions and public methods.
- Parse untrusted values with Zod at the boundary and pass the inferred validated type internally.
- Return discriminated unions for expected failures; reserve thrown errors for violated invariants and unavailable dependencies.

```ts
// ❌
const config = JSON.parse(rawConfig) as Config;

// ✅
const config = ConfigSchema.parse(JSON.parse(rawConfig));
```

## Module syntax

- `typescript/no-require-imports`: Use ESM imports instead of `require()`.
- `typescript/no-namespace`: Use ES modules and object composition instead of TypeScript namespaces.
- `typescript/prefer-as-const`: Preserve literal values with `as const` when widening would discard useful information.

## Redundant types

- `typescript/no-inferrable-types`: Omit annotations that repeat a literal initializer's inferred type.
- `typescript/no-unnecessary-type-assertion`: Remove assertions that do not change the inferred type.

## Verification

`pnpm typecheck` is required because the current oxlint command does not run type-aware rules.
