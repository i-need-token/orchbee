# Verification

## Merge gate

`pnpm verify` is the single local and CI merge gate. Run it from the repository root before committing a completed change. It executes, in order:

1. `pnpm format:check`
2. `pnpm lint`
3. `pnpm build`
4. `pnpm typecheck`
5. `pnpm test`
6. `pnpm test:exports`

A focused command is useful while iterating, but completion requires the root gate.

## Required tasks

| Task           | Contract                                                                                  |
| -------------- | ----------------------------------------------------------------------------------------- |
| `format:check` | Formatter-managed files match `.oxfmtrc.json`.                                            |
| `lint`         | Code, style, architecture, dependency, project-contract, export, and manifest rules pass. |
| `build`        | Every buildable workspace produces its supported artifacts.                               |
| `typecheck`    | Every workspace satisfies its TypeScript compiler contract.                               |
| `test`         | Custom lint-rule tests and every workspace test suite pass.                               |
| `test:exports` | Every library's packed exports, runtime formats, and declarations are consumable.         |

The build precedes type-checking because workspace libraries expose built artifacts and verification and production consumers do not compile another package's source. The recursive build is topological, so a clean checkout produces dependency artifacts before dependent workspaces resolve their package exports.

Application development does not depend on library `dist` directories. Rsbuild's source-build
plugin resolves declared workspace source entries into the development graph, so `pnpm verify`
may clean and rebuild package output without interrupting a running application server. The
verification and production paths continue to validate and consume topologically built package
artifacts.

## CI contract

`.github/workflows/verify.yml` installs the frozen lockfile and runs the same full `pnpm verify` gate for pull requests, pushes to `main`, and manual runs. Configure the repository ruleset to require the workflow's `verify` job before merging.

CI runs `git diff --exit-code` after verification. A formatter, generator, or build-time consistency check that changes a tracked file therefore fails until the generated state is committed.

The workflow uses read-only repository permissions. New credentials, write permissions, release actions, external checkouts, or other side effects require separate human review and must not be added implicitly to the required job.

## External-source audits

Package-specific audits that require an external source checkout, credential, service, device, or
other undeclared environment input remain explicit maintenance commands documented by the owning
package. They are not part of `pnpm verify`.

Promote such an audit into a separate CI job only after the job pins and provisions every external
input, has a stable owner, and does not add side effects to the required merge gate.

## Scaling threshold

The repository runs full workspace tasks while the required gate remains fast. Introduce affected-task orchestration only after there are at least three independently changing workspaces or the cold CI critical path consistently exceeds 30 seconds. Measure the bottleneck first; global policy checks still require explicit inputs even when package tasks become affected-only.
