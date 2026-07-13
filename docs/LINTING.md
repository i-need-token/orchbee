# Linting

## Commands

| Command                | Responsibility                                                       |
| ---------------------- | -------------------------------------------------------------------- |
| `pnpm lint`            | Run oxlint categories, explicit rules, and `orchbee/*` project rules |
| `pnpm lint:fix`        | Apply safe oxlint fixes; remaining diagnostics require code changes  |
| `pnpm typecheck`       | Run TypeScript diagnostics that syntax-aware lint does not replace   |
| `pnpm test:lint-rules` | Run permanent regression tests for the `orchbee/*` rules             |

## Enforcement boundary

- Project docs define every explicitly configured rule and every `orchbee/*` rule; upstream category rules remain governed by the installed oxlint version.
- Error diagnostics fail `pnpm lint`; warning diagnostics remain visible but non-blocking until the affected package files are reviewed.
- Type-aware oxlint rules are not active until `oxlint-tsgolint` is added during package review; never assume lint currently detects floating promises or unsafe third-party `any` propagation.
- Never weaken a rule to pass existing code; fix the code or obtain human approval for a project-policy change.

## Custom rule ownership

| Rule                                        | Authority                                 |
| ------------------------------------------- | ----------------------------------------- |
| `orchbee/filename-naming-convention`        | `docs/TESTING.md#source-and-test-paths`   |
| `orchbee/no-cross-workspace-source-imports` | `docs/ARCHITECTURE.md#package-boundaries` |
| `orchbee/no-inline-suppressions`            | `docs/CODESTYLE.md#inline-suppressions`   |
| `orchbee/require-unit-test`                 | `docs/TESTING.md#required-unit-tests`     |

Every custom-rule change updates its implementation, authority, diagnostic, and regression test together.
