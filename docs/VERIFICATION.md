# Verification

## Merge gate

`pnpm verify` is the single local and CI merge gate. Run it from the repository root before committing a completed change. It executes, in order:

1. `pnpm format:check`
2. `pnpm lint`
3. `pnpm typecheck`
4. `pnpm test`
5. `pnpm build`
6. `pnpm test:exports`

A focused command is useful while iterating, but completion requires the root gate.

## Required tasks

| Task           | Contract                                                                           |
| -------------- | ---------------------------------------------------------------------------------- |
| `format:check` | Formatter-managed files match `.oxfmtrc.json`.                                     |
| `lint`         | Code, architecture, dependency, project-contract, export, and manifest rules pass. |
| `typecheck`    | Every workspace satisfies its TypeScript compiler contract.                        |
| `test`         | Custom lint-rule tests and every workspace test suite pass.                        |
| `build`        | Every buildable workspace produces its supported artifacts.                        |
| `test:exports` | Every library's packed exports, runtime formats, and declarations are consumable.  |

## CI contract

`.github/workflows/verify.yml` installs the frozen lockfile and runs the same full `pnpm verify` gate for pull requests, pushes to `main`, and manual runs. Configure the repository ruleset to require the workflow's `verify` job before merging.

CI runs `git diff --exit-code` after verification. A formatter, generator, or build-time consistency check that changes a tracked file therefore fails until the generated state is committed.

The workflow uses read-only repository permissions. New credentials, write permissions, release actions, external checkouts, or other side effects require separate human review and must not be added implicitly to the required job.

## External-source audits

`pnpm audit:ghostty-panel` compares Orchbee's terminal provenance and generated constraints with a Ghostty source checkout. It requires Ghostty at `../ghostty` or a `GHOSTTY_SOURCE` path, so it is an explicit maintenance command rather than part of `pnpm verify`.

Run the audit when updating Ghostty alignment data. It may become a separate CI job only after that job checks out a pinned Ghostty revision and declares its external inputs.

## Scaling threshold

The repository runs full workspace tasks while the required gate remains fast. Introduce affected-task orchestration only after there are at least three independently changing workspaces or the cold CI critical path consistently exceeds 30 seconds. Measure the bottleneck first; global policy checks still require explicit inputs even when package tasks become affected-only.
