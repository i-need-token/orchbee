# Verification

## Merge gate

`pnpm verify` is the single local and CI merge gate. Run it from the repository root before committing a completed change. It executes, in order:

1. `pnpm format:check`
2. `pnpm test:tla:inventory`
3. `pnpm build`
4. `pnpm lint`
5. `pnpm typecheck`
6. `pnpm test`
7. `pnpm test:exports`

A focused command is useful while iterating, but completion requires the root gate.

## Execution and evidence

- Do not overlap `pnpm verify` with another root gate, `pnpm test`, or a build in the same checkout.
  Independently scheduled checks may run in parallel only when their writable artifacts and
  fixtures are isolated. This does not disable parallelism already owned by a workspace task.
- Finish governed edits before the final gate. If relevant inputs change during or after a run,
  identify the stale evidence and rerun the required gate against the final inputs before claiming
  completion. A command's success is evidence for its checked inputs, not for later edits.
- Preserve the first failure's command, source state, platform/toolchain, seed when available,
  and full log outside tracked source. A later pass does not diagnose the failure or prove a fix;
  report unresolved failures alongside subsequent results. Diagnostic reruns require a stated
  question and retain prior outcomes; never retry until green or classify a failure as environment
  or test instability without evidence.
- Distinguish failed, interrupted, skipped, and unexecuted checks from passing checks. Partial gate
  success does not establish completion, and an aggregate summary is not a complete test inventory.

## Check output

New or changed human-facing verification output must define a bounded summary, actionable
failure diagnostics, and access to full detail. Reuse the existing output layer when compatible;
preserve machine-readable output contracts and explicit verbose modes. Reducing output never
authorizes dropping checks, swallowing warnings, or replacing a process failure with parsed success.

The root `test` and `verify` commands, Node test wrappers, and Rust workspace runner default to
stage summaries. `scripts/run-checks.mjs` owns the root command sequences;
`scripts/check-output.mjs` owns their shared presentation. This is an output-only boundary:
stages remain ordered and fail fast, while explicitly grouped independent checks run concurrently,
with the same workspace scope, compiler flags, and manual-test exclusions. A group drains every
member before reporting in declaration order and stopping on its first declared failing member.
Only the child process result determines success, never parsed log text.

- Each check reports its start, result, and duration from launch to its own process/stdio close,
  excluding time waiting for other group members or formatting summaries. Recognized test totals and bounded warning
  or skip notices supplement the result; these are advisory, not a complete test inventory.
- Complete combined stdout/stderr goes directly to private files in an OS temporary directory,
  whose path is printed before execution. Logs stay outside the worktree, survive failure or
  interruption, and remain until the user or OS removes them. They are not automatically uploaded;
  review them for sensitive diagnostics before sharing.
- A failure reports its log path, bounded recognized error headlines, and a beginning/end excerpt
  with explicit omission markers. Nonzero child exit codes are preserved; child signals produce a nonzero conventional
  signal exit code. Failure to launch a check or retain/read its log also fails the command.
- Nested wrappers inherit the outer capture instead of allocating duplicate logs or summaries.
  `pnpm test --verbose`, `pnpm verify --verbose`, and
  `node scripts/run-rust-workspace.mjs test --verbose` stream the underlying output without capture.
  The wrapped Node test commands accept the same `--verbose` option.
- Concurrent streaming prefixes lines with the check name. Long lines are split into bounded,
  labeled fragments without dropping text or splitting Unicode characters; a final unterminated
  fragment receives a newline. Each stream waits for its destination write to complete before
  forwarding more, so slow destinations cannot cause unbounded queued output.
- The wrapper uses asynchronous child execution and inherits the existing process group.
  Uncaptured single checks inherit stdin; captured or concurrently piped checks ignore stdin. It
  introduces no retry, timeout, background execution, or cancellation supervisor. Existing
  terminal/job cancellation still owns interruption; a printed start without a result is not a pass.

The execution model is a fixed ordered list: start a stage's members, retain their output, record
each close independently, drain the group, then either advance on all-zero results or stop on
failure. Summary and verbose modes execute
the same list. `scripts/check-output.test.mjs` exercises this public operation with successful,
failing, signalled, missing-executable, nested, and large-output children, plus lossless logs and
bounded diagnostics. It does not establish arbitrary descendant cancellation or platform behavior
on runners where those cases were not exercised. Standalone TLC output and its machine-readable
results remain unchanged.

## Required tasks

| Task                 | Contract                                                                                                                                                                                                         |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `format:check`       | Oxfmt-managed files match `.oxfmtrc.json`, and rustfmt plus every application-owned native formatter accepts its owned files.                                                                                    |
| `lint`               | Code, Rust workspace and Clippy, documentation pairing, style, storage, architecture, dependency, project-contract, experience-catalog, export, and manifest rules pass.                                         |
| `test:tla:inventory` | The TLA+ execution inventory, artifact registration, owner commands, trigger mapping, and runner negative controls agree.                                                                                        |
| `build`              | Every buildable pnpm and Cargo workspace produces its supported artifacts with its pinned toolchain and dependencies.                                                                                            |
| `typecheck`          | Every workspace with a TypeScript contract satisfies it.                                                                                                                                                         |
| `test`               | Contract-checker tests, documentation-pairing negative tests, custom lint-rule tests, pnpm and Cargo test suites, required controlled local system witnesses, and declared native cross-compilation checks pass. |
| `test:exports`       | Every library's packed exports, runtime formats, and declarations are consumable.                                                                                                                                |

- `build` precedes `lint` and `typecheck` because dependency analysis and production consumers resolve library artifacts, not another package's source. A clean checkout must produce those exports before Knip classifies dependency usage.
- Root builds finish all shared packages before starting applications; each phase remains topological. Story discovery can consume another owner's package dependencies without a direct runtime dependency from the host, so applications must not race those package builds.
- Application module development does not consume library `dist`; Rsbuild resolves declared source entries into the development graph. Node-executed development preparation that consumes a library's runtime export must explicitly build that dependency before process startup.
- `pnpm verify` may rebuild package output without interrupting a running application server; verification and production continue to consume topologically built artifacts.
- Rust workspace work uses the exact root toolchain and committed Cargo lockfile; Zig source
  dependencies use immutable URLs and content hashes. Required builds reject a different compiler
  or dependency resolution before producing an artifact.
- `scripts/run-rust-workspace.mjs` connects the root pnpm gate to Cargo. Before the first accepted
  Rust member exists, it validates the pinned toolchain and empty workspace metadata; afterward the
  same tasks run rustfmt, Clippy, build, and tests over every member with `--locked`.
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
coverage. Triggers follow the abstraction's implementation owners and their dependencies; unrelated
styles do not trigger document or terminal-lifecycle models. Keep positive owner-path regression
cases beside exclusions when narrowing triggers. It is an execution index, not a second behavior specification; package-owned contracts
and TLA+ modules remain authoritative for model semantics.

The runner uses Node built-ins and its separately pinned Java jar. Root `package.json` and
`pnpm-lock.yaml` changes therefore use the mandatory inventory/command checks without selecting
all TLC models. The TLA+ runner, inventory, CI toolchain workflow, and governing verification or
modeling standards remain shared triggers. Retain broader owner triggers where a protocol package
or native process is the abstraction boundary; a smaller pattern alone is not correctness evidence.

TLA+ model checking is change-triggered evidence: `pnpm test` and the required CI `verify` job do
not invoke TLC implicitly. `pnpm verify` runs only the cheap `test:tla:inventory` consistency and
negative-control check. The separate CI `tla` job maps the Git diff to registered models and runs
only those checks. A missing or invalid diff base fails safe by selecting every model.

Run an affected workspace's `test:tla` command when a change modifies its TLA+ module,
configuration, owner contract, transition, invariant, or refinement mapping. Run the root command
when several model owners or shared model infrastructure change. `pnpm test:tla:changed -- <ref>`
uses the same manifest mapping as CI. A package-owned `verify:correctness` command may include its
TLA+ check when that command is the package's comprehensive correctness workflow.

Every registered check uses the pinned TLC SHA-256, its declared worker count (one by default,
optionally two through the model's `workers` field), a fixed fingerprint polynomial,
declared finite bounds, a per-model timeout, and coverage output. The runner fails when a required
semantic action is missing or has no reached witness and emits a machine-readable state-count,
duration, worker count, and model/config SHA-256 report. Configuration bounds and TLC's finite-state assumption
remain evidence limits; action coverage is not implementation refinement evidence.

The runner uses `-lncheck final` to check liveness once the complete reachable graph is available,
preserving fairness and every configured property while avoiding repeated partial-graph checks.
Safety checks still run during exploration. A timeout before final liveness checking is a failure,
never progress evidence. Keep coverage collection enabled for required action witnesses and wait
for Java's stdio to close before parsing its final report.

Independent model/configuration checks share a queue of at most two TLC processes.
The default is two slots on machines with at least four available processors and 8 GiB of memory
(respecting a reported process memory limit), otherwise one. Pass `--jobs 1` or `--jobs 2` to choose
explicitly. Variants of one model can occupy both slots; required coverage is checked only after
all its variants succeed and combines evidence only within that model.
Each configuration owns its state directory and a file-backed Java log; the runner emits the whole
log after Java closes so concurrent counterexamples cannot interleave. A failure stops new models
and variants, drains already-running checks within their own timeouts, and fails the command.
Use `--jobs 1` with unchanged declared worker counts for sequential timing comparisons on the same machine.

For the pinned TLC action-header format, coverage requires a positive generated-state count from
`distinct:generated`, or the sole count of a single-count header. A transition to an already
discovered state is still a reached witness; expression evaluation costs are not action evidence.
This interpretation follows the pinned [TLC action reporter](https://github.com/tlaplus/tlaplus/blob/v1.7.4/tlatools/org.lamport.tlatools/src/tlc2/tool/coverage/ActionWrapper.java).
`scripts/tla/model-inventory.test.mjs` pins the return-to-existing-state regression and missing,
zero-generated, and expression-only negative controls without changing any model or required action.

Local TLC execution requires Java 17 or newer and uses `java` from `PATH`. Set
`ORCHBEE_TLA_JAVA` for an explicit executable, `ORCHBEE_TLA2TOOLS_JAR` for an already provisioned
copy with the pinned SHA-256, or `ORCHBEE_TLA_TIMEOUT_MILLISECONDS` to apply a positive timeout
override to every selected check.

Each TLC invocation, including concurrent variants of one model, owns a unique temporary directory
for both model-checking state and `java.io.tmpdir`. The pinned resolver extracts standard modules
under fixed filenames and deletes them when its JVM exits; sharing the JVM temporary directory can
truncate or remove another invocation's modules. Keep the directory until its child closes, then
clean it through the existing runner lifecycle. This isolation does not change model concurrency,
worker counts, deadlines, configuration, or coverage requirements.

An unrelated implementation change does not require replaying an unchanged model. Passing an
unchanged model also does not establish that its abstraction still corresponds to the
implementation; deterministic tests and traceability evidence own that correspondence. A
model-affecting change is incomplete until its selected TLA+ command passes and the completion
report identifies the checked model and bounds.

## CI contract

- `.github/workflows/verify.yml` installs the frozen lockfile and runs `pnpm verify` for pull requests, pushes to `main`, and manual runs.
- CI supplies `--verbose` so complete diagnostics remain in the existing job log after the
  temporary runner is removed; a local temporary-log path alone is not durable CI evidence.
- The required `verify` job does not run TLC. The required `tla` job selects and runs
  change-triggered checks from the tracked inventory; an unrelated change completes that job
  without provisioning Java or TLC.
- The workflow installs every exact compiler version required by a tracked native workspace through
  its pinned official toolchain mechanism. Directly downloaded archives require a checked digest
  before they are added to `PATH`.
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

Measure elapsed time by stage separately from output bytes and lines. Compare equivalent command
scope, toolchain, and cold or warm cache conditions; quieter output alone is not a runtime saving.
The threshold below opens a design review, not permission to skip required checks.

| Decision                | Contract                                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Default                 | Run full workspace tasks while the required gate remains fast.                                                    |
| Affected-task threshold | Require at least three independently changing workspaces or a cold CI critical path consistently over 30 seconds. |
| Before changing         | Measure the bottleneck.                                                                                           |
| Global policy checks    | Keep explicit inputs even when package tasks become affected-only.                                                |
