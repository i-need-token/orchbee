# Verification

## Merge gate

`pnpm verify` is the local and CI host verification gate. Run the applicable root gate from the repository root before committing a completed change. It prepares check inputs without a full application build or cross-compilation, in this order:

1. `pnpm format:check`
2. `pnpm test:tla:inventory`
3. `pnpm build:packages`
4. `pnpm lint`
5. `pnpm typecheck`
6. `pnpm test`
7. `pnpm test:exports`

A focused command is useful while iterating, but completion requires the applicable root gate.
Use the daily affected gate below for a scoped completed change; use all host suites when required
explicitly, including CI, or when scope cannot be resolved. Passing the applicable gate satisfies
a repository or package requirement for the root gate; do not run both forms solely because another
standard spells the command as `pnpm verify`. Other standards should reference this section rather
than define a second root-gate policy.

Routine edits and merges do not require a separate application build. `pnpm build` aliases
`pnpm build:packages`, producing only shared library artifacts consumed by checks. Use an owning
workspace's build when changing its bundling, assets, native linking or packaging contract. Use
`pnpm build:full` explicitly to build every package, application and Cargo target for the host.
`pnpm test:cross` explicitly compiles the maintained foreign targets. `pnpm verify:full` replaces
the library preparation stage with `build:full` and adds `test:cross` after the host tests; it is
for deliberate full build/platform validation, not the routine merge gate. Unknown affected scope
selects all host tests, never these optional full-build or cross-compilation stages. A merge whose
checked inputs are unchanged does not need a duplicate gate or build.

## Affected verification

`pnpm verify:changed -- <base>` is the daily root gate. `pnpm verify --changed-since <base>` is
equivalent; add `--plan` to inspect the exact resolution without running checks. The base must resolve
to an ancestor commit of HEAD. The resolver includes committed, staged, unstaged, deleted, renamed
and nonignored untracked paths. It reads NUL-delimited Git output; both ends of a rename matter.
It unions base-to-HEAD, HEAD-to-index and index-to-worktree changes so reverting a working file
cannot hide its staged change.
An invalid base, changed dependency manifest, unknown path or unsupported dependency graph selects
the full test scope. An unrelated untracked file can therefore cause a full fallback; the resolver
does not delete or ignore it.

Only expensive test scope is narrowed in this implementation. Formatting, TLA inventory, library
preparation, lint, type checking, export checks and root tooling tests remain complete. Selected
real-process tests prepare their required host executables before running; application bundles and
unrelated native binaries are not prerequisites. There is no ordinary test-result cache or new
build-result cache; native compilers retain their normal incremental behavior.
Affected pnpm suites use the normal workspace commands. Affected Cargo
members retain all targets, all features, the lockfile and documentation tests. Node suites finish
before Rust tests start. `pnpm verify` and `pnpm test` run all host suites without cross-compilation.

The private root verification owner maintains `scripts/verification-scope.mjs`. Its stateless policy
takes current pnpm/Cargo workspace dependencies and changed paths, and returns one immutable plan.
The plan contains the affected owner and transitive consumer closure across normal, development,
optional, peer, build and target-specific local dependencies. Reviewed foreign edges also cover
native process cohorts, shared protocol fixtures and Storybook discovery.
Changes to those relationships must update the resolver and its positive controls in the same change.
Missing or ambiguous owners fail conservatively. The command reports selected and unaffected suites
separately from executed results; an unaffected suite is unexecuted, never a pass or a cache hit.

Shared paths need explicit ownership. The existing `docs/tui/` standards select the native
terminal client and its consumers. Root verification commands, their declared helpers/tests select
root tooling evidence; the complete prerequisites above still run. New root helpers or standards
remain unknown until reviewed. An unknown document owner selects the full test scope.

The evidence is Git boundary fixtures, dependency-closure positive and negative controls, command
selection witnesses and the existing real Cargo runner tests. These establish the supported ownership
mapping, not arbitrary dynamic file access. New undeclared cross-owner inputs require a mapping or
full fallback. Final evidence remains tied to unchanged checked inputs under the execution rules below.
All host suites remain available to catch mapping omissions; CI continues running that baseline.
This policy does not waive change-triggered TLA checks or establish a fixed latency guarantee.

Changes under `schemas/terminal/` select the Rust protocol crate, TypeScript protocol package,
and Terminal Host, then their normal consumer closure. Their existing test commands each run one
independent check against `schemas/terminal/v1/constants.json`; fixture generators do not produce
that registry. Canonical payload and native surface checks continue to read the same authority.

## Execution and evidence

- Do not overlap `pnpm verify` with another root gate, `pnpm test`, or a build in the same checkout.
  Independently scheduled checks may run in parallel only when their writable artifacts and
  fixtures are isolated. Root Node workspace tests finish before Rust workspace tests start:
  separate caches do not protect native process startup deadlines from CPU-heavy Node suites.
  This does not disable parallelism already owned by a workspace task.
- Run the applicable root gate against the complete integration candidate. A commit, report, handoff,
  or internal slice boundary does not require another run when checked inputs remain unchanged.
  This does not omit separately required change-triggered correctness evidence or introduce a cache
  for ordinary gate stages.
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
`scripts/check-output.mjs` owns their shared presentation. This presentation boundary does not select scope:
stages remain ordered and fail fast, while explicitly grouped independent checks run concurrently,
with the same workspace scope, compiler flags, and manual-test exclusions. A group drains every
member before reporting in declaration order and stopping on its first declared failing member.
The generic wrapper judges each command by its process result, never parsed log text.
The dependency command owns the explicit aggregate acceptance policy in
[Linting](LINTING.md#retained-development-consumers): it preserves the complete and strict raw
results and labels an accepted strict exit one separately. This is a check policy, not output
presentation. Its Knip children have a 180-second deadline; process errors, signals and timeouts
always fail. Its separately retained stdout/stderr evidence remains available in verbose mode too.

- Each check reports its start, result, and duration from launch to its own process/stdio close,
  excluding time waiting for other group members or formatting summaries. Recognized test totals and bounded warning
  or skip notices supplement the result; these are advisory, not a complete test inventory.
- Complete combined stdout/stderr goes directly to private files in an OS temporary directory,
  whose path is printed before execution. Logs stay outside the worktree and survive failure or
  interruption. The invoking task owns their retention and removes them after the last diagnosis,
  review, or recovery consumer finishes; the runner does not reclaim them automatically.
  They are not automatically uploaded; review them for sensitive diagnostics before sharing.
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
  introduces no retry, timeout, background execution, or cancellation supervisor; a deadline
  belongs to the check command that owns it, such as the Knip children and Zig tests. Existing
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

| Task                      | Contract                                                                                                                                                        |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `format:check`            | Oxfmt-managed files match `.oxfmtrc.json`, and rustfmt plus every application-owned native formatter accepts its owned files.                                   |
| `lint`                    | Code, Rust workspace and Clippy, style, storage, architecture, dependency, project-contract, experience-catalog, export, and manifest rules pass.               |
| `test:tla:inventory`      | The TLA+ execution inventory, artifact registration, owner commands, trigger mapping, and runner negative controls agree; every trigger matches a tracked file. |
| `build`, `build:packages` | Shared library workspaces produce the artifacts consumed by lint, type checking, tests and export checks.                                                       |
| `build:full`              | Explicitly build every buildable pnpm and Cargo workspace for the host with its pinned toolchain and dependencies.                                              |
| `typecheck`               | Every workspace with a TypeScript contract satisfies it.                                                                                                        |
| `test`                    | Contract-checker tests, custom lint-rule and dependency-scope tests, pnpm and Cargo host test suites, and required controlled local system witnesses pass.      |
| `test:cross`              | Explicitly compile the declared native foreign-target test graphs.                                                                                              |
| `verify:rust-native`      | Run Rust workspace Clippy, the native test prerequisites and every Cargo host test suite under the current host's platform configuration.                       |
| `test:exports`            | Every library's packed exports, runtime formats, and declarations are consumable.                                                                               |

- `build:packages` precedes `lint` and `typecheck` because dependency analysis and production consumers resolve library artifacts, not another package's source. A clean checkout must produce those exports before Knip classifies dependency usage.
- `build:full` finishes all shared packages before starting applications; each phase remains topological. Story discovery can consume another owner's package dependencies without a direct runtime dependency from the host, so applications must not race those package builds.
- `scripts/run-checks.mjs` selects native test prerequisites by the tested consumer and host platform. Prepare only the host executable and peer binaries that real-process witnesses consume, then run Cargo tests. Cargo tests and Clippy compile their own inputs; do not precede them with a redundant workspace build. Standalone Rust runner invocations require the same real-process prerequisites when applicable.
- Application module development does not consume library `dist`; Rsbuild resolves declared source entries into the development graph. Node-executed development preparation that consumes a library's runtime export must explicitly build that dependency before process startup.
- `pnpm verify` may rebuild package output without interrupting a running application server; verification and production continue to consume topologically built artifacts.
- Rust workspace work uses the exact root toolchain and committed Cargo lockfile; Zig source
  dependencies use immutable URLs and content hashes. Required builds reject a different compiler
  or dependency resolution before producing an artifact.
- `scripts/run-rust-workspace.mjs` connects the root pnpm gate to Cargo. Before the first accepted
  Rust member exists, it validates the pinned toolchain and empty workspace metadata; afterward the
  same tasks run rustfmt, Clippy, build, and tests over every member with `--locked`.
- The Rust lint task runs Clippy twice: over every target with all features, then over the
  default-feature library and binary targets of every member. Development dependencies re-enable
  test-support features under `--all-targets`, so only the second pass compiles the product feature
  set and reports the warnings a release build would emit, such as imports that only test-support
  code reads.
- The Terminal Host Zig tests run under two deadlines, because one hung test otherwise stalls
  `pnpm verify` indefinitely. Zig's `--test-timeout 120s` bounds each unit test, reports a hung test
  by name, and continues with the next one; the slowest test binary took at most 18 s in total, so
  the bound leaves more than six times that. A 20-minute deadline bounds the whole `zig build test`
  run, including compilation and the native witnesses outside Zig's test runner. A cold run took
  3.7 minutes and a warm run 41 seconds, so the bound leaves over five times the cold run, and it
  stays below the 30-minute `terminal-host-native` CI job limit so the gate reports before CI
  cancels the job. Measurements: Windows 11 x64, Zig 0.16.0, 2026-10-31. On expiry the runner lists
  the test binaries still running and the process tree that run started, terminates only that tree,
  and fails with exit 124. Either timeout fails the stage, never passes or skips it. Remeasure
  before changing either bound.
- Native cross-compilation checks prove source and dependency compilation for the declared target;
  platform behavior still requires a native runner.
- `verify:rust-native` reuses the same Rust runner tasks and native test preparation as `lint` and
  `test`. It adds no step of its own, so a Windows host gate and the Windows CI job cannot drift
  from the Linux root gate. Run it on Windows when a change touches platform-gated Rust code.

## Change-triggered TLA+ verification

`pnpm test:tla` is the complete TLA verification gate. It runs the Java-free inventory and runner tests, then checks every registered configuration and the real TLC cache/mutation witnesses. Model checking always starts from fresh state and does not reuse success results. The complete invocation fails if wall time exceeds 60 seconds. Measure three complete fresh-state runs with dependencies and pinned tools installed when changing this budget or the portfolio; disclose machine/load and provisioning time separately.

`scripts/tla/models.json` registers 13 protocol models, one configuration each, with semantic owners, claims, implementation triggers, required action coverage and a 20-second configuration deadline. This is the whole maintained portfolio, not a fast subset in front of a hidden required full suite. New models must fit the complete budget and the admission criteria in [Modeling and Correctness](MODELING-AND-CORRECTNESS.md). A timeout is a failure, not a way to skip work.

The ordinary `pnpm verify` gate runs `test:tla:inventory`; it does not implicitly run Java. Product protocol state, transitions, assumptions or refinement changes require the affected registered model and its implementation tests. Workspace commands delegate to the shared runner; workspaces with only local implementation evidence expose no empty TLA command. `node scripts/tla/run-models.mjs --all` is the model-only baseline; it is not the complete TLA gate.

`pnpm test:tla:changed -- <base>` selects affected protocols and optionally reuses exact unchanged mathematical results under the [tooling cache contract](../scripts/tla/README.md#local-success-cache). Reuse does not prove changed product code. Full `pnpm test:tla` never reuses those results. Runner/helper, verification-standard, supported TLC-job or trigger-policy changes select the compact full model baseline and require the real tooling witnesses. Known inventory test entrypoints stay in the mandatory inventory gate. There is no workflow metamodel.

Selection unions current and baseline dependencies, both rename paths and committed/staged/unstaged/untracked changes. Removed registrations do not select unrelated survivors. Changed shared metadata, invalid or non-ancestor bases, hidden Git inputs and ambiguous workflows select all models. Current and baseline exclusions are evaluated independently so a same-diff exclusion cannot hide an owner change. Optional document scopes must classify all supported H2 headings; ambiguity falls back to whole-file selection. Product dependencies on tooling paths are still honored. The CI TLA job executes its selected models and real cache/mutation witnesses; setup/download and CI job overhead are distinct from local verification timing.

Every configuration uses pinned TLC 1.7.4 and SHA-256, fingerprint polynomial 0, explicit finite bounds, required action coverage, declared workers and isolated state/Java temporary directories. `-lncheck final` checks all configured temporal properties after the reachable graph is complete. Success requires normal child/stdio closure, a complete successful report, zero queued states and every required action witness. Output retains machine-readable counts, duration, worker/heap budgets, input digest and log path. Partial exploration is not progress evidence.

Independent configurations share a FIFO resource queue bounded by processors and 60% of available memory. The default heap ceiling is 4096 MiB plus a 512 MiB process reserve. `--max-heap-mib N` accepts decimal integers 256 through 8192; `--jobs N` explicitly caps process slots within the same memory budget. Declared workers count against CPU admission; an oversized job runs alone. These are admission bounds, not guarantees about GC threads, actual RSS or external load. Caller-owned logs and recovery state are reclaimed after their last consumer.

Local UI selection, menus, rendering configuration, fixed CLI workflows and deterministic business reducers use their existing controlled/generated implementation tests. Owner documents must name those witnesses and state that they do not provide exhaustive TLC assurance. Retained cores cover shared identity, grants, commit/publication, unknown outcomes, persistence and queue progress within their declared abstractions.

## CI contract

- `.github/workflows/verify.yml` installs the frozen lockfile and runs `pnpm verify` for pull requests, pushes to `main`, and manual runs.
- CI supplies `--verbose` so complete diagnostics remain in the existing job log after the
  temporary runner is removed; a local temporary-log path alone is not durable CI evidence.
- The `rust-windows` job runs `pnpm verify:rust-native` on `windows-latest`, so every Rust
  member gets Clippy and its Cargo test suites under the Windows platform configuration that the
  Linux `verify` job never compiles. Formatting, documentation, TypeScript and TLA checks are
  platform-independent and stay in `verify`.
- Platform exclusions are explicit. macOS has no Rust job because the native Client is only an
  unsigned per-user Apple Silicon development cohort. `terminal-host-native` runs the native
  Terminal Host tests and formatting check on `windows-latest` only: GitHub rejects the
  `macos-14` runner label while this repository is private, so a macOS entry could only fail.
  The job keeps its macOS setup step; once the repository is public, restore the macOS job by
  setting its matrix to `os: [macos-14, windows-latest]`. Until then macOS has no CI evidence.
  No job runs `test:cross`; cross-compilation stays explicit.
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

## Release workflow

`.github/workflows/release.yml` is never a required check. Its top-level permission is
`contents: read`; the owner's steps are in [Releasing](RELEASING.md). It has two ways in:

| Start                              | Path    | Identity and key                                | Publishes                          |
| ---------------------------------- | ------- | ----------------------------------------------- | ---------------------------------- |
| By hand, `version` empty           | Dry run | Test identity, the repository's test key        | Nothing; artifacts kept three days |
| By hand, `version` set (rehearsal) | Release | Release identity, `ORCHBEE_RELEASE_SIGNING_KEY` | Nothing; artifacts kept one day    |
| Pushed tag `v<version>`            | Release | Release identity, `ORCHBEE_RELEASE_SIGNING_KEY` | A draft GitHub Release, then npm   |

- One row per release platform token carries `build` and `publish` flags. The dry run's `plan` job
  selects the `build` rows; the release path takes the rows with both, and
  `node scripts/release-publish.mjs plan` refuses none or more than one, because one main npm
  package must list every platform package and combining rows is not built. Only
  `windows-x86_64` has both.
- Dry run: `build` installs the pinned toolchains, with Zig and Inno Setup 6.7.1 downloaded and
  checked by digest, runs `pnpm desktop:installer:test --build-id <commit>.<run number>`, and then
  `node scripts/release-artifacts.mjs collect`, which gathers the signed release directory, its
  `.zip`, the setup file, the npm tarballs, and `SHA256SUMS`. `smoke` and `smoke-npm` take that
  artifact to fresh runners, check it with `node scripts/release-artifacts.mjs check`, and run
  `node scripts/release-smoke-windows.mjs` one step at a time: the installer route, then the npm
  route, each ending with the leftover scan. Run those steps only on a disposable Windows machine;
  they install and remove for real.
- Release path, in order; each job stops the run with a sentence naming what is wrong:

  | Job                                  | Does                                                                                                                                                                                                                                                                                                                          | Needs                                                                                    |
  | ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
  | `release-plan`                       | Takes the version from the tag or the input and runs `node scripts/release-version.mjs check v<version>`                                                                                                                                                                                                                      | Every version source at that version                                                     |
  | `release-preconditions`              | `release-publish.mjs preconditions`: the signing key is an Ed25519 PKCS #8 PEM key whose public half is in `RELEASE_KEYS`; on a tag run `NPM_TOKEN` is set                                                                                                                                                                    | Environment `release`                                                                    |
  | `release-build`                      | Stages the cohort with the release stem and writes `release.json` (`stable`, the version, `<commit>.<run number>`); uploads the cohort and, separately, `release.json`                                                                                                                                                        | No secret                                                                                |
  | `release-sign`                       | Signs only `release.json` with `scripts/sign-release-manifest.mjs`, which uses Node built-ins only, in a job that runs no other repository code                                                                                                                                                                               | Environment `release`                                                                    |
  | `release-package`                    | `node apps/desktop/scripts/package.mjs --profile release --installer --cohort <signed cohort>`, then `release-artifacts.mjs collect --profile release`                                                                                                                                                                        | No secret                                                                                |
  | `release-smoke`, `release-smoke-npm` | The dry run's smoke steps on fresh runners with those exact files and the release names; a release build must refuse each withheld command with exit `4`                                                                                                                                                                      | No secret                                                                                |
  | `release-publish`                    | Tag runs only: `release-publish.mjs github` creates or completes the draft release, a pre-release exactly when the version has a pre-release part; then `release-publish.mjs npm` publishes platform packages first, the main package last, with `--provenance` and the dist-tag `next` for a pre-release, `latest` otherwise | Environment `release`; `contents: write` and `id-token: write`, granted to this job only |

- Secrets reach only the step that needs them, by environment variable: never an argument, a log
  line, or an artifact. A job that holds a secret installs no dependencies and runs only repository
  scripts that use Node built-ins, plus `gh` and `npm` in `release-publish`.
- A re-run never duplicates or replaces a publication. A draft release for the tag is completed with
  the assets it lacks; a published release, an asset with another SHA-256, an asset this run did not
  make, or two releases for the tag stop the job without a change. An npm version the registry
  already holds with this run's tarball is skipped; with another tarball the job fails.
- The installer and its programs carry no Authenticode signature; the release manifest's Ed25519
  signature is the only one.
- The release path's artifacts carry release names and a release signature. A rehearsal's are
  rebuilt by the tag run, so an installed rehearsal build blocks the published one of the same
  version with `version-conflict`; never install them.

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
| Default                 | Use affected host verification for scoped changes; keep full builds and cross-compilation explicit.               |
| Affected-task threshold | Require at least three independently changing workspaces or a cold CI critical path consistently over 30 seconds. |
| Before changing         | Measure the bottleneck.                                                                                           |
| Global policy checks    | Keep explicit inputs even when package tasks become affected-only.                                                |
