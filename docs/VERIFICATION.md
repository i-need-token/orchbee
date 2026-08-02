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

| Task           | Contract                                                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `format:check` | Oxfmt-managed files match `.oxfmtrc.json`, and every native workspace formatter accepts its owned files.                          |
| `lint`         | Code, style, storage, architecture, dependency, project-contract, experience-catalog, export, and manifest rules pass.            |
| `build`        | Every buildable workspace produces its supported artifacts with its pinned toolchain and dependencies.                            |
| `typecheck`    | Every workspace with a TypeScript contract satisfies it.                                                                          |
| `test`         | Contract-checker tests, custom lint-rule tests, default workspace test suites, and declared native cross-compilation checks pass. |
| `test:exports` | Every library's packed exports, runtime formats, and declarations are consumable.                                                 |

- `build` precedes `typecheck` because verification and production consumers resolve library artifacts, not another package's source.
- Recursive builds are topological: a clean checkout builds dependencies before dependents resolve their exports.
- Application development does not depend on library `dist`; Rsbuild resolves declared source entries into the development graph.
- `pnpm verify` may rebuild package output without interrupting a running application server; verification and production continue to consume topologically built artifacts.
- Native workspace builds may resolve only dependencies pinned by immutable URL and content hash.
  The build must reject a different compiler or dependency digest before producing an artifact.
- Native cross-compilation checks prove source and dependency compilation for the declared target;
  platform behavior still requires a native runner.

## Change-triggered TLA+ verification

`pnpm test:tla` runs every workspace that exposes a `test:tla` command. TLA+ model checking is
change-triggered evidence: `pnpm test`, `pnpm verify`, and the required CI `verify` job do not invoke
it implicitly.

Run an affected workspace's `test:tla` command when a change modifies its TLA+ module,
configuration, runner, tool pin, or a contract, transition, invariant, or refinement mapping that
the model abstracts. Run the root command when several model owners or shared model infrastructure
change. A package-owned `verify:correctness` command may include its TLA+ check when that command is
the package's comprehensive correctness workflow.

An unrelated implementation change does not require replaying an unchanged model. Passing an
unchanged model also does not establish that its abstraction still corresponds to the
implementation; deterministic tests and traceability evidence own that correspondence. A
model-affecting change is incomplete until its selected TLA+ command passes and the completion
report identifies the checked model and bounds.

## CI contract

- `.github/workflows/verify.yml` installs the frozen lockfile and runs `pnpm verify` for pull requests, pushes to `main`, and manual runs.
- The required job does not run change-triggered TLA+ checks. Review and the owning correctness
  contract determine when `pnpm test:tla` is required.
- The workflow installs every exact compiler version required by a tracked native workspace and
  verifies the official toolchain archive before adding it to `PATH`.
- Require the workflow's `verify` job in the repository ruleset.
- CI runs `git diff --exit-code` after verification; tracked generated changes fail until committed.
- The workflow uses read-only repository permissions.
- Credentials, write permissions, release actions, external checkouts, and other side effects require separate human review and must not enter the required job implicitly.

## External-source audits

- A content-addressed native source dependency used by a production build is a declared build input,
  not an external-source audit. Its immutable URL and digest belong in the owning native manifest.
- Package-specific audits requiring an external checkout, credential, service, device, or undeclared environment input remain explicit maintenance commands documented by the owning package and stay outside `pnpm verify`.
- Promote an audit to a separate CI job only when it pins and provisions every external input, has a stable owner, and adds no side effects to the required merge gate.

## Scaling threshold

| Decision                | Contract                                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Default                 | Run full workspace tasks while the required gate remains fast.                                                    |
| Affected-task threshold | Require at least three independently changing workspaces or a cold CI critical path consistently over 30 seconds. |
| Before changing         | Measure the bottleneck.                                                                                           |
| Global policy checks    | Keep explicit inputs even when package tasks become affected-only.                                                |
