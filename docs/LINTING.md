# Linting

## Commands

| Command                    | Responsibility                                                           |
| -------------------------- | ------------------------------------------------------------------------ |
| `pnpm verify`              | Run the complete merge gate defined by `docs/VERIFICATION.md`            |
| `pnpm lint`                | Run every required code, style, dependency, and project lint check       |
| `pnpm lint:code`           | Run oxlint categories, explicit rules, and `orchbee/*` project rules     |
| `pnpm lint:styles`         | Validate the token registry and authored frontend styling contract       |
| `pnpm lint:dependencies`   | Run Knip strict dependency, file, export, catalog, and manifest analysis |
| `pnpm lint:projects`       | Validate every schema-backed workspace project contract                  |
| `pnpm lint:fix`            | Apply safe oxlint fixes; remaining diagnostics require code changes      |
| `pnpm typecheck`           | Run TypeScript diagnostics that syntax-aware lint does not replace       |
| `pnpm test:lint-rules`     | Test the `orchbee/*` rules and English/Chinese documentation contract    |
| `pnpm test:style-contract` | Run regression tests for the frontend style-contract checker             |

## Enforcement boundary

- Project docs define every explicitly configured rule and every `orchbee/*` rule; upstream category rules remain governed by the installed oxlint version.
- Every rule documented as enforced must be enabled in `.oxlintrc.json`; `pnpm test:lint-rules` verifies this docs-to-config direction as well as config-to-doc coverage.
- `pnpm test:lint-rules` requires a one-to-one `docs/zh-CN/` counterpart for every authoritative English document and rejects structural, inline-identifier, or localized-link drift. Human review remains responsible for semantic translation fidelity.
- Error diagnostics fail `pnpm lint`; warning diagnostics remain visible but non-blocking until the affected package files are reviewed.
- `pnpm lint:dependencies` uses Knip `--strict`: every imported package must be declared directly by the importing workspace, and unused files, dependencies, exports, types, unresolved imports, duplicate exports, cycles, and stale catalog entries fail the command.
- Keep all runnable scripts, build configuration, test suites, and public package entries in `knip.json#workspaces`; never ignore a finding merely because an entry point was omitted from analysis.
- Type-aware oxlint rules are not active until `oxlint-tsgolint` is added during package review; never assume lint currently detects floating promises or unsafe third-party `any` propagation.
- `pnpm lint:styles` enforces the registry, generated TypeScript inventory, CSS Module, Tailwind-boundary, raw-value, responsive-threshold, JSX class, and inline-style rules defined by [`docs/frontend/design-tokens.md`](frontend/design-tokens.md). It proves that the generated public token names exactly match the CSS registry and that the generated full Tailwind compatibility map exactly covers the installed public theme, targets registered Orchbee tokens, and preserves the literal breakpoint and container values required at compile time. Its shadcn authoring exception remains path-scoped to checked-in generated source and support files.
- Never weaken a rule to pass existing code; fix the code or obtain human approval for a project-policy change.

## Generated source

The checked-in shadcn snapshot under `packages/ui/src/components/ui/*.tsx` and its generated `use-mobile.ts` hook remain close to the upstream registry so CLI refreshes stay reviewable. A path-scoped override exempts that source from repository authoring conventions that would require broad mechanical rewrites: explicit module-boundary return types, assertion style, shadowing style, equality style, nested-ternary style, component-directory grouping, direct React Hook calls in TSX, one-function TSX and one-Hook file boundaries, and the sibling-test requirement. The automatic JSX runtime also makes `react/react-in-jsx-scope` inapplicable there.

Correctness rules remain active for generated source, including React Hooks, JSX keys, module boundaries, cross-workspace imports, unsafe TypeScript constructs, and inline suppression bans. `ChartStyle` has the only additional exception for its upstream-controlled style element; [`docs/frontend/react.md`](frontend/react.md#correctness-and-safety) defines that constraint.

Do not extend a generated-source override to Orchbee-authored modules. Review each upstream refresh, preserve focused compatibility adaptations, and test the behaviors or integrations Orchbee owns.

## Custom rule ownership

| Rule                                        | Authority                                             |
| ------------------------------------------- | ----------------------------------------------------- |
| `orchbee/component-directory`               | `docs/frontend/react.md#component-directory-boundary` |
| `orchbee/filename-naming-convention`        | `docs/TESTING.md#source-and-test-paths`               |
| `orchbee/module-boundaries`                 | `docs/MODULE-BOUNDARIES.md#module-boundaries`         |
| `orchbee/no-cross-workspace-source-imports` | `docs/ARCHITECTURE.md#package-boundaries`             |
| `orchbee/no-inline-suppressions`            | `docs/CODESTYLE.md#inline-suppressions`               |
| `orchbee/no-react-hooks-in-tsx`             | `docs/frontend/react.md#component-hook-boundary`      |
| `orchbee/one-function-per-tsx`              | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/one-hook-per-file`                 | `docs/frontend/react.md#hook-file-boundary`           |
| `orchbee/require-unit-test`                 | `docs/TESTING.md#required-unit-tests`                 |
| `orchbee/zustand-scope`                     | `docs/frontend/react.md#zustand-and-context`          |

Every custom-rule change updates its implementation, authority, diagnostic, and regression test together.
