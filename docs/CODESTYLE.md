# Code Style

## Formatting

- `.oxfmtrc.json` is authoritative for Oxfmt-managed files; Rust uses the root toolchain's `rustfmt`; Zig source and build files use the exact workspace-pinned `zig fmt` implementation.
- Format changed files with their owning formatter during iteration. Use `pnpm format` for intentional repository-wide formatting; do not rewrite unrelated files to complete a scoped change. The final applicable gate in [`VERIFICATION.md`](VERIFICATION.md) owns formatting checks.
- `useTabs`: Use tabs in Oxfmt-managed files where the format permits them; native formatters own their indentation conventions.
- `singleQuote` and `jsxSingleQuote`: Use single quotes in JavaScript, TypeScript, and JSX strings unless escaping makes double quotes clearer.
- `semi`: Terminate statements with semicolons.
- `trailingComma`: Use trailing commas wherever Oxfmt supports them.
- Do not hand-align code with indentation spaces; let Oxfmt choose any spaces required for syntax or alignment.

## Modules

- `import/no-default-export`: Use named exports except in configuration files and JavaScript lint-plugin entry points.
- `import/no-cycle`: Keep module dependencies acyclic.
- `import/no-self-import`: Do not import a module from itself.
- `import/no-duplicates`: Merge imports from the same module.
- `typescript/consistent-type-imports`: Mark type-only dependencies with `import type` or an inline `type` specifier.

## Bindings and control flow

- `no-var`: Declare bindings with `const` or `let`.
- `prefer-const`: Use `const` when a binding is never reassigned.
- `no-shadow`: Give nested bindings distinct names.
- `eqeqeq`: Use strict equality operators.
- `unicorn/no-nested-ternary`: Replace nested ternaries with named branches or control flow.
- `no-console`: Route runtime diagnostics through an owned logging or host boundary.

## Inline suppressions

- `orchbee/no-inline-suppressions`: Remove `oxlint-disable` and `eslint-disable` comments from production and test code; fix the code or change the project rule with human approval.

## Dynamic code

- `no-eval`, `no-implied-eval`, `no-new-func`: Do not execute code assembled from strings.

## Comments

Comments explain constraints and non-obvious decisions; names and structure explain behavior.

```ts
// ❌ Increment the retry count.
retryCount += 1;

// ✅ The bridge allows three transient failures before surfacing the host error.
retryCount += 1;
```

## React

React-specific rules are authoritative in [`docs/frontend/react.md`](frontend/react.md).

## Zig

Zig-specific naming, resource, failure, boundary, concurrency, and testing rules are authoritative in [`docs/ZIG.md`](ZIG.md).

## Rust

Rust-specific types, visibility, numbers, failures, resources, and lint exceptions are authoritative in [`docs/RUST.md`](RUST.md).
