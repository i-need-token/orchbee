# TypeScript

## Compiler contract

| Option                       | Requirement                                                                       |
| ---------------------------- | --------------------------------------------------------------------------------- |
| `strict`                     | Treat all implicit uncertainty as a type error                                    |
| `noUncheckedIndexedAccess`   | Handle missing indexed values explicitly                                          |
| `exactOptionalPropertyTypes` | Distinguish an absent property from an explicit `undefined`                       |
| `erasableSyntaxOnly`         | Use JavaScript-runtime syntax; represent enum-like values with `as const` objects |
| `verbatimModuleSyntax`       | Keep runtime and type-only imports explicit                                       |
| `module: preserve`           | Preserve source module syntax for the workspace build tool                        |
| `moduleResolution: bundler`  | Match Rslib, Rspack, Vite, and Vitest module resolution                           |

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
- Bundled TypeScript source omits runtime extensions from relative imports; write `./validation`, except for the Node source-execution case below.
- Library builds must add runtime extensions to emitted bundleless declarations. With Rslib, set `redirect.dts.extension` to `true` on the declaration-producing output.
- A workspace whose TypeScript output runs directly in Node.js must override the base config with `module` and `moduleResolution` set to `nodenext`; that workspace uses explicit `.js` extensions in relative source imports.
- Build tools that execute TypeScript source through Node's native type stripping use explicit `.ts` paths throughout that loaded source graph. Their owning no-emit typecheck enables `allowImportingTsExtensions` and retains `erasableSyntaxOnly` and `verbatimModuleSyntax`. Document the required runtime capability and verify the actual Node entrypoint; a bundler-only test does not prove Node resolution. Node does not apply TypeScript path aliases or type checking. This case does not permit publishing raw TypeScript through library package exports. See [Node's TypeScript execution contract](https://nodejs.org/docs/latest-v22.x/api/typescript.html).

## Redundant types

- `typescript/no-inferrable-types`: Omit annotations that repeat a literal initializer's inferred type.
- `typescript/no-unnecessary-type-assertion`: Remove assertions that do not change the inferred type.

## Verification

`pnpm typecheck` is required because the current oxlint command does not run type-aware rules.
