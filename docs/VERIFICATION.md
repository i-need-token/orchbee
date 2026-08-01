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

| Task           | Contract                                                                                                               |
| -------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `format:check` | Formatter-managed files match `.oxfmtrc.json`.                                                                         |
| `lint`         | Code, style, storage, architecture, dependency, project-contract, experience-catalog, export, and manifest rules pass. |
| `build`        | Every buildable workspace produces its supported artifacts.                                                            |
| `typecheck`    | Every workspace satisfies its TypeScript compiler contract.                                                            |
| `test`         | Contract-checker tests, custom lint-rule tests, and every workspace test suite pass.                                   |
| `test:exports` | Every library's packed exports, runtime formats, and declarations are consumable.                                      |

- `build` precedes `typecheck` because verification and production consumers resolve library artifacts, not another package's source.
- Recursive builds are topological: a clean checkout builds dependencies before dependents resolve their exports.
- Application development does not depend on library `dist`; Rsbuild resolves declared source entries into the development graph.
- `pnpm verify` may rebuild package output without interrupting a running application server; verification and production continue to consume topologically built artifacts.

## CI contract

- `.github/workflows/verify.yml` installs the frozen lockfile and runs `pnpm verify` for pull requests, pushes to `main`, and manual runs.
- Require the workflow's `verify` job in the repository ruleset.
- CI runs `git diff --exit-code` after verification; tracked generated changes fail until committed.
- The workflow uses read-only repository permissions.
- Credentials, write permissions, release actions, external checkouts, and other side effects require separate human review and must not enter the required job implicitly.

## External-source audits

- Package-specific audits requiring an external checkout, credential, service, device, or undeclared environment input remain explicit maintenance commands documented by the owning package and stay outside `pnpm verify`.
- Promote an audit to a separate CI job only when it pins and provisions every external input, has a stable owner, and adds no side effects to the required merge gate.

## Scaling threshold

| Decision                | Contract                                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Default                 | Run full workspace tasks while the required gate remains fast.                                                    |
| Affected-task threshold | Require at least three independently changing workspaces or a cold CI critical path consistently over 30 seconds. |
| Before changing         | Measure the bottleneck.                                                                                           |
| Global policy checks    | Keep explicit inputs even when package tasks become affected-only.                                                |
