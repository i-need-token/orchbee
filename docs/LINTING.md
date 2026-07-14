# Linting

## Commands

| Command                  | Responsibility                                                           |
| ------------------------ | ------------------------------------------------------------------------ |
| `pnpm lint`              | Run every required code and dependency lint check                        |
| `pnpm lint:code`         | Run oxlint categories, explicit rules, and `orchbee/*` project rules     |
| `pnpm lint:dependencies` | Run Knip strict dependency, file, export, catalog, and manifest analysis |
| `pnpm lint:fix`          | Apply safe oxlint fixes; remaining diagnostics require code changes      |
| `pnpm typecheck`         | Run TypeScript diagnostics that syntax-aware lint does not replace       |
| `pnpm test:lint-rules`   | Run permanent regression tests for the `orchbee/*` rules                 |

## Enforcement boundary

- Project docs define every explicitly configured rule and every `orchbee/*` rule; upstream category rules remain governed by the installed oxlint version.
- Error diagnostics fail `pnpm lint`; warning diagnostics remain visible but non-blocking until the affected package files are reviewed.
- `pnpm lint:dependencies` uses Knip `--strict`: every imported package must be declared directly by the importing workspace, and unused files, dependencies, exports, types, unresolved imports, duplicate exports, cycles, and stale catalog entries fail the command.
- Keep all runnable scripts, build configuration, test suites, and public package entries in `knip.json#workspaces`; never ignore a finding merely because an entry point was omitted from analysis.
- Type-aware oxlint rules are not active until `oxlint-tsgolint` is added during package review; never assume lint currently detects floating promises or unsafe third-party `any` propagation.
- Never weaken a rule to pass existing code; fix the code or obtain human approval for a project-policy change.

## Approved dependency exceptions

| Workspace       | Dependency | Reason                                                     | Exit condition                                                                               |
| --------------- | ---------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Repository root | `zod`      | Human-approved retention for upcoming schema-boundary work | Remove the Knip exception when the first root consumer is added or the dependency is removed |

## Custom rule ownership

| Rule                                        | Authority                                     |
| ------------------------------------------- | --------------------------------------------- |
| `orchbee/filename-naming-convention`        | `docs/TESTING.md#source-and-test-paths`       |
| `orchbee/module-boundaries`                 | `docs/MODULE-BOUNDARIES.md#module-boundaries` |
| `orchbee/no-cross-workspace-source-imports` | `docs/ARCHITECTURE.md#package-boundaries`     |
| `orchbee/no-inline-suppressions`            | `docs/CODESTYLE.md#inline-suppressions`       |
| `orchbee/require-unit-test`                 | `docs/TESTING.md#required-unit-tests`         |

Every custom-rule change updates its implementation, authority, diagnostic, and regression test together.
