# Verification

## Merge gate

`pnpm verify` is the single local and CI merge gate. Run it from the repository root before committing a completed change. It executes, in order:

1. `pnpm format:check`
2. `pnpm lint`
3. `pnpm test:tla:inventory`
4. `pnpm build`
5. `pnpm typecheck`
6. `pnpm test`
7. `pnpm test:exports`

A focused command is useful while iterating, but completion requires the root gate.

## Required tasks

| Task                 | Contract                                                                                                                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `format:check`       | Oxfmt-managed files match `.oxfmtrc.json`, and every native workspace formatter accepts its owned files.                                                                                                            |
| `lint`               | Code, documentation pairing, style, storage, architecture, dependency, project-contract, experience-catalog, export, and manifest rules pass.                                                                       |
| `test:tla:inventory` | The TLA+ execution inventory, artifact registration, owner commands, trigger mapping, and runner negative controls agree.                                                                                           |
| `build`              | Every buildable workspace produces its supported artifacts with its pinned toolchain and dependencies.                                                                                                              |
| `typecheck`          | Every workspace with a TypeScript contract satisfies it.                                                                                                                                                            |
| `test`               | Contract-checker tests, documentation-pairing negative tests, custom lint-rule tests, default workspace test suites, required controlled local system witnesses, and declared native cross-compilation checks pass. |
| `test:exports`       | Every library's packed exports, runtime formats, and declarations are consumable.                                                                                                                                   |

- `build` precedes `typecheck` because verification and production consumers resolve library artifacts, not another package's source.
- Recursive builds are topological: a clean checkout builds dependencies before dependents resolve their exports.
- Application module development does not consume library `dist`; Rsbuild resolves declared source entries into the development graph. Node-executed development preparation that consumes a library's runtime export must explicitly build that dependency before process startup.
- `pnpm verify` may rebuild package output without interrupting a running application server; verification and production continue to consume topologically built artifacts.
- Native workspace builds may resolve only dependencies pinned by immutable URL and content hash.
  The build must reject a different compiler or dependency digest before producing an artifact.
- Native cross-compilation checks prove source and dependency compilation for the declared target;
  platform behavior still requires a native runner.
- The required Desktop SSH Project-enrollment witness packages the actual Dev profile and runs against
  a temporary loopback `sshd` on macOS arm64 or glibc Linux x64. Unsupported local platforms report
  an explicit skip; the required Linux CI job provisions `sshd`, Xvfb, and X authentication and
  executes the witness. Its temporary home, keys, installed runtime, and application data are removed
  by the witness, and it never invokes the manual native-window test.

## Change-triggered TLA+ verification

`pnpm test:tla` runs every registered model through the shared bounded runner. The execution
inventory in `scripts/tla/models.json` registers each checked module/configuration, semantic owner,
claim authority, changed-path triggers, timeout, configuration variants, and required action
coverage. It is an execution index, not a second behavior specification; package-owned contracts
and TLA+ modules remain authoritative for model semantics.

TLA+ model checking is change-triggered evidence: `pnpm test` and the required CI `verify` job do
not invoke TLC implicitly. `pnpm verify` runs only the cheap `test:tla:inventory` consistency and
negative-control check. The separate CI `tla` job maps the Git diff to registered models and runs
only those checks. A missing or invalid diff base fails safe by selecting every model.

Run an affected workspace's `test:tla` command when a change modifies its TLA+ module,
configuration, owner contract, transition, invariant, or refinement mapping. Run the root command
when several model owners or shared model infrastructure change. `pnpm test:tla:changed -- <ref>`
uses the same manifest mapping as CI. A package-owned `verify:correctness` command may include its
TLA+ check when that command is the package's comprehensive correctness workflow.

Every registered check uses the pinned TLC SHA-256, one worker, a fixed fingerprint polynomial,
declared finite bounds, a per-model timeout, and coverage output. The runner fails when a required
semantic action is missing or has no reached witness and emits a machine-readable state-count,
duration, and model/config SHA-256 report. Configuration bounds and TLC's finite-state assumption
remain evidence limits; action coverage is not implementation refinement evidence.

Local TLC execution requires Java 17 or newer and uses `java` from `PATH`. Set
`ORCHBEE_TLA_JAVA` for an explicit executable, `ORCHBEE_TLA2TOOLS_JAR` for an already provisioned
copy with the pinned SHA-256, or `ORCHBEE_TLA_TIMEOUT_MILLISECONDS` to apply a positive timeout
override to every selected check.

An unrelated implementation change does not require replaying an unchanged model. Passing an
unchanged model also does not establish that its abstraction still corresponds to the
implementation; deterministic tests and traceability evidence own that correspondence. A
model-affecting change is incomplete until its selected TLA+ command passes and the completion
report identifies the checked model and bounds.

## CI contract

- `.github/workflows/verify.yml` installs the frozen lockfile and runs `pnpm verify` for pull requests, pushes to `main`, and manual runs.
- The required `verify` job does not run TLC. The required `tla` job selects and runs
  change-triggered checks from the tracked inventory; an unrelated change completes that job
  without provisioning Java or TLC.
- The workflow installs every exact compiler version required by a tracked native workspace and
  verifies the official toolchain archive before adding it to `PATH`.
- Require the workflow's `verify` and `tla` jobs in the repository ruleset.
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
