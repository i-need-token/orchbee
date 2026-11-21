# Verification

## Merge gate

`pnpm verify` is the host verification gate; CI runs its stages across the jobs of the [CI contract](#ci-contract). Run the applicable root gate from the repository root before committing a completed change. It prepares check inputs without a full application build or cross-compilation, in this order:

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
to an ancestor commit of HEAD, because only such a base bounds the change. A reference that does not
resolve, or a commit that is not an ancestor, fails the command before any plan or check, also with
`--plan`; its message names the reference and the two remedies: rebase onto it, or pass the fork
point (`git merge-base <base> HEAD`) to check the branch as it stands. A base that does not bound
the change therefore never yields an affected plan, and a typo or an unrebased branch is reported
instead of silently running every suite.
The resolver includes committed, staged, unstaged, deleted, renamed
and nonignored untracked paths. It reads NUL-delimited Git output; both ends of a rename matter.
It unions base-to-HEAD, HEAD-to-index and index-to-worktree changes so reverting a working file
cannot hide its staged change.
Hidden Git inputs, an unknown path or an unsupported dependency graph select the full test scope.
An unrelated untracked file can therefore cause a full fallback; the resolver does not delete or
ignore it.

A changed `package.json`, `pnpm-lock.yaml` or `pnpm-workspace.yaml` selects every pnpm owner, and a
changed `Cargo.toml` or `Cargo.lock` selects every Cargo owner. A manifest inside an owner also
selects that owner, whatever its package manager. Normal consumer closure follows, so the reviewed
foreign edges carry the change across languages. A manifest change reaches no further than a change
to every file of that package manager's owners, whose cross-language consumers those edges already
record.

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
native process cohorts, shared protocol fixtures and Storybook discovery. A test witness, a suite
that runs another owner's built artifact while its own artifact is unchanged, joins the plan after
closure without selecting its consumers. A vendored Cargo package, a path package under
`third-party/` that Cargo resolves for a member while the workspace excludes it, owns its directory
without a suite of its own: a change there selects the members that resolve it and their closure,
and it is never listed as unaffected. Other paths under `third-party/` remain unknown.
Changes to those relationships must update the resolver and its positive controls in the same change.
Missing or ambiguous owners fail conservatively. The command reports selected and unaffected suites
separately from executed results; an unaffected suite is unexecuted, never a pass or a cache hit.

Shared paths need explicit ownership. The existing `docs/tui/` standards select the native
terminal client and its consumers by review policy; no test reads them. Root verification commands,
their declared helpers/tests select root tooling evidence; the complete prerequisites above still
run. New root helpers remain unknown until reviewed. The Rust module graph check, its test and the
[Rust module record](LINTING.md#rust-module-record) are root tooling inputs: `lint` runs the check
over the whole workspace in every gate, affected or not, in about two seconds, so tightening the
record selects no suite.
Documentation without a test consumer selects no suite and reports `documentation <- <path>`:
anything under `proposals/`, which is never current authority, and Markdown under `docs/` or
directly at the repository root, which no product test or build reads. The explicit mappings above
claim their paths first. Other files under `docs/` or at the root remain unknown, and Markdown
inside an owner selects that owner. A new test or build that reads such documentation must add an
explicit mapping in the same change.

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
They also select the connection handshake crate, whose registry test checks the
[frozen fixtures](../schemas/connection/README.md#frozen-fixtures).

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
| `verify:no-rust-tests`    | Run every stage of `verify` except the Cargo host test suites and their native prerequisites; Rust lint stays.                                                  |
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
  same tasks run rustfmt, Clippy, build, and tests over every member with `--locked`. The test
  task adds `--no-fail-fast`, so one run reports every failing test target instead of stopping at
  the first; a Windows CI job takes tens of minutes, so a run that names one failure at a time
  costs a run per failure. The result is unchanged: any failing target still fails the task.
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
  stays inside the CI job limits (30 minutes for `terminal-host-native`, and 60 for the macOS
  `client-commands` job, which runs the tests after its Rust part) so the gate reports before CI
  cancels the job. Measurements: Windows 11 x64, Zig 0.16.0, 2026-10-31. On expiry the runner lists
  the test binaries still running and the process tree that run started, terminates only that tree,
  and fails with exit 124. Either timeout fails the stage, never passes or skips it. Remeasure
  before changing either bound.
- The Terminal Host's program-output fuzz pass joins `zig build test` as a bounded Debug pass:
  three fixed seeds of two reader batches each took 5.6 to 7.8 s, inside the 120 s per-test bound,
  because Ghostty's Debug integrity checks make one batch cost seconds. Its volume runs only on
  explicit request in the release build mode, `zig build test-output-fuzz -Doptimize=ReleaseSafe`,
  with `-Doutput-fuzz-seed` and `-Doutput-fuzz-batches` for a longer pass; 900 batches (18.7 MB)
  and the hyperlinked-listing resize that Debug skips took 1.5 s after a 65 s compile, and one seed
  of 50,000 batches (1.1 GB) took 133 s. Measurements: Windows 11 x64, Zig 0.16.0, 2026-11-15. Run
  the release pass when a change touches the Host's handling of program output, resize, or
  projection encoding, and record a failing seed and batch count with the first failure.
- Native cross-compilation checks prove source and dependency compilation for the declared target;
  platform behavior still requires a native runner.
- `verify:rust-native` reuses the same Rust runner tasks and native test preparation as `lint` and
  `test`. It adds no step of its own, so a Windows host gate and the Windows CI jobs cannot drift
  from the Linux root gate. Run it on Windows when a change touches platform-gated Rust code.
  `--part <name>` runs one test part instead of the whole test task, for CI's parallel jobs:
  `workspace` is the lint task, the tests of every package except the terminal client `orchbee`,
  and every documentation test; `client` is that package's unit tests and its `integration`
  target; `client-commands` is its other test targets. The parts together run every target once.
  The runner lists the client's test targets at run time, so a new one joins `client-commands` by
  itself, and it fails on a client target kind that no part names.

## Change-triggered TLA+ verification

`pnpm test:tla` is the complete TLA verification gate. It runs the Java-free inventory and runner tests, then checks every registered configuration and the real TLC cache/mutation witnesses. Model checking always starts from fresh state and does not reuse success results. The complete invocation fails if wall time exceeds 60 seconds. Measure three complete fresh-state runs with dependencies and pinned tools installed when changing this budget or the portfolio; disclose machine/load and provisioning time separately.

`scripts/tla/models.json` registers 18 protocol models with 22 configurations (one per named variant, otherwise one per model), with semantic owners, claims, implementation triggers, required action coverage and a 20-second configuration deadline. This is the whole maintained portfolio, not a fast subset in front of a hidden required full suite. New models must fit the complete budget and the admission criteria in [Modeling and Correctness](MODELING-AND-CORRECTNESS.md). A timeout is a failure, not a way to skip work.

The ordinary `pnpm verify` gate runs `test:tla:inventory`; it does not implicitly run Java. CI runs no TLA+ model checking or TLC. Product protocol state, transitions, assumptions or refinement changes require the affected registered model, run locally with `pnpm test:tla:changed -- <base>` or `pnpm test:tla`, and its implementation tests. Workspace commands delegate to the shared runner; workspaces with only local implementation evidence expose no empty TLA command. `node scripts/tla/run-models.mjs --all` is the model-only baseline; it is not the complete TLA gate.

`pnpm test:tla:changed -- <base>` selects affected protocols and optionally reuses exact unchanged mathematical results under the [tooling cache contract](../scripts/tla/README.md#local-success-cache). Reuse does not prove changed product code. Full `pnpm test:tla` never reuses those results. Runner/helper, verification-standard or trigger-policy changes select the compact full model baseline and require the real tooling witnesses. Known inventory test entrypoints stay in the mandatory inventory gate. There is no workflow metamodel.

Selection unions current and baseline dependencies, both rename paths and committed/staged/unstaged/untracked changes. Removed registrations do not select unrelated survivors. Changed shared metadata, invalid or non-ancestor bases and hidden Git inputs select all models. Current and baseline exclusions are evaluated independently so a same-diff exclusion cannot hide an owner change. Optional document scopes must classify all supported H2 headings; ambiguity falls back to whole-file selection. Product dependencies on tooling paths are still honored. CI workflows are not a selection input, so a workflow edit selects no model.

Every configuration uses pinned TLC 1.7.4 and SHA-256, fingerprint polynomial 0, explicit finite bounds, required action coverage, declared workers and isolated state/Java temporary directories. `-lncheck final` checks all configured temporal properties after the reachable graph is complete. Success requires normal child/stdio closure, a complete successful report, zero queued states and every required action witness. Output retains machine-readable counts, duration, worker/heap budgets, input digest and log path. Partial exploration is not progress evidence.

Independent configurations share a FIFO resource queue bounded by processors and 60% of available memory. The default heap ceiling is 4096 MiB plus a 512 MiB process reserve. `--max-heap-mib N` accepts decimal integers 256 through 8192; `--jobs N` explicitly caps process slots within the same memory budget. Declared workers count against CPU admission; an oversized job runs alone. These are admission bounds, not guarantees about GC threads, actual RSS or external load. Caller-owned logs and recovery state are reclaimed after their last consumer.

Local UI selection, menus, rendering configuration, fixed CLI workflows and deterministic business reducers use their existing controlled/generated implementation tests. Owner documents must name those witnesses and state that they do not provide exhaustive TLC assurance. Retained cores cover shared identity, grants, commit/publication, unknown outcomes, persistence and queue progress within their declared abstractions.

## CI contract

- `.github/workflows/verify.yml` installs the frozen lockfile and runs for pull requests, pushes to `main`, and manual runs.
- The Linux `verify` job runs `pnpm verify:no-rust-tests`: every stage of `pnpm verify` except the
  Cargo test suites and their native prerequisites. Linux is not a release platform yet and several
  of its Cargo test targets fail, so that stage made every run red and hid the other results. Rust
  lint stays in the job and keeps the Linux configuration compiling. Its `lint` stage is also the
  only CI run of the Rust module graph check, which reads source text for every platform and so
  needs no native runner. Restore `pnpm verify` in this job when Linux support resumes.
- A push to `main` never cancels the run in progress. A complete run outlasts the gap between
  pushes, so cancelling on every push left `main` without one finished run. GitHub keeps
  one waiting run per concurrency group: the newest commit is verified next, and that run covers
  the commits between. Pull-request runs still cancel on a new push.
- Read the newest completed run on `main` before landing an integration batch and before pushing a
  release tag: `gh run list --workflow verify.yml --branch main --status completed --limit 1`. A
  red job is fixed, reverted or reported to the owner in that batch, never left for the next run to
  repeat.
- CI supplies `--verbose` so complete diagnostics remain in the existing job log after the
  temporary runner is removed; a local temporary-log path alone is not durable CI evidence.
- The `rust-windows` job runs `pnpm verify:rust-native` on `windows-latest`, so every Rust
  member gets Clippy and its Cargo test suites under the Windows platform configuration that the
  Linux `verify` job never compiles. It runs as three parallel jobs, one per test part of
  [Required tasks](#required-tasks): as one job it ran for about an hour, 41 minutes of that in
  tests and most of those in the terminal client's process tests, and a run ends with its slowest
  job. They and `rust-macos` are the only jobs that run the Cargo test suites. Formatting, documentation,
  TypeScript and TLA checks are platform-independent and stay in `verify`. The Windows TUI process
  tests that share `apps/orchbee/tests/support/windows_tui_session.rs` take turns inside their test
  binary: four of their real Session pairs at once saturate the four-vCPU runner, and shells then
  echo typed input seconds late.
- The `rust-macos` job runs the same three parts of `pnpm verify:rust-native` on `macos-15`
  (Apple Silicon), and its `client-commands` job also runs the native Terminal Host tests and
  formatting check, which `terminal-host-native` runs on `windows-latest`. macOS runners are the
  account's scarcest, so the Terminal Host tests (about 8 minutes) share the shortest part's job
  (about 7 minutes) instead of taking a fourth runner; that job still ends before the slowest
  Windows part (26 to 33 minutes; measured on `main`, 2026-11-16). A red macOS job counts like any
  other. No Verify job runs on an Intel Mac runner; Intel macOS is left to the release workflow's
  `macos-x86_64` row.
- Every job that builds the Terminal Host, here and in the release workflow, first runs the local
  action `.github/actions/zig-packages`. It restores Zig's package archives from the Actions cache,
  keyed on the runner's OS and architecture, the Terminal Host's `build.zig.zon` and `build.zig`,
  and the action itself, so later Zig builds run offline. On a miss it prefetches the arocc package
  with curl, because Zig's own download of it fails on GitHub's Windows and macOS runners with
  HttpConnectionClosing, then lists the build's steps, up to four times, and caches the result.
  Listing runs the build script, which fetches every package the build requests, lazy ones
  included; `zig build --fetch` leaves those to the build. Package downloads on hosted runners fail
  now and then mid-transfer, and each such failure turned a run red before any test ran. Zig checks
  every downloaded package against its pinned hash before it enters the archive store.
- Platform exclusions are explicit. Linux stays parked: its Cargo test suites run in no job, as
  the `verify` job above describes. No job runs `test:cross`; cross-compilation stays explicit.
- No job runs TLA+ model checking or TLC, and no job provisions Java. Of the TLA tooling, the
  `verify` job runs only the Java-free `test:tla:inventory` stage of `pnpm verify`. Affected registered models run
  locally, as Change-triggered TLA+ verification above requires.
- The workflow installs every exact compiler version required by a tracked native workspace through
  its pinned official toolchain mechanism. Directly downloaded archives require a checked digest
  before they are added to `PATH`. Every third-party GitHub Action across `.github/workflows/*.yml`
  and `.github/actions/**/*.yml` is pinned to its full 40-hex commit SHA with a `# vX.Y.Z` tag
  comment.
- Require the workflow's `verify` job in the repository ruleset.
- CI runs `git diff --exit-code` after verification; tracked generated changes fail until committed.
- The workflow uses read-only repository permissions.
- Credentials, write permissions, release actions, external checkouts, and other side effects require separate human review and must not enter the required job implicitly.

## Release workflow

`.github/workflows/release.yml` is never a required check. Its top-level permission is
`contents: read`; the owner's steps are in [Releasing](RELEASING.md). It has two ways in:

| Start                              | Path    | Identity and key                                | Publishes                               |
| ---------------------------------- | ------- | ----------------------------------------------- | --------------------------------------- |
| By hand, `version` empty           | Dry run | Test identity, the repository's test key        | Nothing; artifacts kept three days      |
| By hand, `version` set (rehearsal) | Release | Release identity, `ORCHBEE_RELEASE_SIGNING_KEY` | Nothing; artifacts kept one day         |
| Pushed tag `v<version>`            | Release | Release identity, `ORCHBEE_RELEASE_SIGNING_KEY` | A GitHub Release and npm, no human step |

- One row per release platform token carries `build` and `publish` flags. The dry run's `plan` job
  selects the Windows `build` rows. `node scripts/release-publish.mjs plan` gives the release path
  every `build` row on a rehearsal and the rows with both flags on a tag run, split into a Windows
  matrix and a macOS matrix, and refuses a run without a Windows row or a row of a platform family
  the workflow has no jobs for. `windows-x86_64`, `macos-aarch64` (`macos-15`) and `macos-x86_64`
  (`macos-15-intel`) have both flags, so a tag run builds, smokes and publishes all three, and a
  failure on any of them stops the tag.
- Dry run: `build` installs the pinned toolchains, with Zig and Inno Setup 6.7.1 downloaded and
  checked by digest, runs `pnpm desktop:installer:test --build-id <commit>.<run number>`, and then
  `node scripts/release-artifacts.mjs collect`, which gathers the signed release directory, its
  `.zip`, the window archive `<stem>-desktop-<version>-<platform>.zip`, the update feed document
  with its signature by the repository's test key, the setup file, the npm tarballs, and
  `SHA256SUMS`. `smoke` and `smoke-npm` take that
  artifact to fresh runners, check it with `node scripts/release-artifacts.mjs check`, and run
  `node scripts/release-smoke-windows.mjs` one step at a time: the installer route, then the npm
  route, each ending with the leftover scan. Run those steps only on a disposable Windows machine;
  they install and remove for real.
- Release path, in order; each job stops the run with a sentence naming what is wrong:

  | Job                                  | Does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Needs                                                                                                      |
  | ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
  | `release-plan`                       | Takes the version from the tag or the input; on a tag run, `release-publish.mjs notes` reads the release notes, the annotated tag's message without a trailing signature block, through the GitHub API, and fails for a lightweight tag or an empty message; runs `node scripts/release-version.mjs check v<version>`; then `release-publish.mjs gate-paths` reads the published releases through the GitHub API and selects the Windows release gate paths (`gate` / `gate-matrix`: previous, second-previous, middle and oldest in the 90-day window from `gateFirstVersion` 0.1.0-beta.14 on) plus the Apple silicon macOS gate path (`macos-gate` / `macos-gate-matrix`: the newest published release in that window carrying `orchbee-<ver>-macos-aarch64.feed.json` and `.sig`)                                                                                                                                                                                                                                                                | Every version source at that version                                                                       |
  | `release-preconditions`              | `release-publish.mjs preconditions`: the signing key is an Ed25519 PKCS #8 PEM key whose public half is in `RELEASE_KEYS`; on a tag run `NPM_TOKEN` is set                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | Environment `release`                                                                                      |
  | `release-build`                      | Stages the cohort with the release stem and writes `release.json` (`stable`, the version, `<commit>.<run number>`); uploads the cohort and, separately, `release.json`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | No secret                                                                                                  |
  | `release-build-macos`                | On each macOS row's runner, in bash: the pinned Zig macOS archive checked by digest and the Zig packages action as in `verify.yml`, then the same staging and `release.json` as `release-build`, refusing a manifest whose platform is not the row's                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | No secret                                                                                                  |
  | `release-sign`                       | For every row, signs only `release.json` with `scripts/sign-release-manifest.mjs`, which with the feed module it imports uses Node built-ins only, in a job that runs no other repository code                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Environment `release`                                                                                      |
  | `release-package`                    | `node apps/desktop/scripts/package.mjs --profile release --installer --cohort <signed cohort>`, then `release-artifacts.mjs collect --profile release`, which also writes the window archive and the unsigned update feed document naming both archives by their GitHub release asset addresses; uploads the feed document again on its own for `release-feed`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | No secret                                                                                                  |
  | `release-package-macos`              | Restores the executable bit download-artifact drops from the four programs, then `package.mjs --profile release --installer --cohort <signed cohort>`, which writes the ad-hoc signed app, its window file list and its disk image, and `release-artifacts.mjs collect --profile release`, which adds the `.tar.gz`, the window archive and its feed document                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | No secret                                                                                                  |
  | `release-feed`                       | After `release-package`, receiving only the feed document: `node scripts/release-feed.mjs gate` writes the gate copy `<stem>-<version>-<platform>.gate.feed.json`, the same document naming each archive by its file name, and one step signs both with `scripts/sign-release-manifest.mjs` and the release key; uploads the feed's `.sig` as `release-feed-signature-<platform>` and the signed gate copy as `release-gate-feed-<platform>`, kept as long as the release artifacts. Runs for every row on every release-path run, rehearsals included, and runs only scripts that use Node built-ins                                                                                                                                                                                                                                                                                                                                                                                                                                                | Environment `release`                                                                                      |
  | `release-smoke`, `release-smoke-npm` | The dry run's smoke steps on fresh runners with those exact files and the release names; a release build must refuse each withheld command with exit `4`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | No secret                                                                                                  |
  | `release-smoke-macos`                | On a fresh runner of each macOS row, `release-artifacts.mjs check`, then `node scripts/release-smoke-macos.mjs` one step at a time: `disk-image` mounts the image read-only without Finder, runs `codesign --verify --deep --strict` and the bundled program's `--version`; `archive` installs from the `.tar.gz` under a scratch `HOME`, checks the active version, the link and the PATH lines in `~/.zshenv`, and uninstalls, after which the link and that file are gone; `install-script` pipes the website's `install.sh --desktop` into `/bin/sh`, under a shell's soft open-file limit of 256, against a local gate copy of the row's feed signed with the repository's test key (`feed.json.sig`), checks signature verification, the active version, the link, the PATH lines in `~/.zshenv`, the copied application's signature, the store's signed window version and its emptied temporary folder, and uninstalls; `npm` installs the npm tarballs into a scratch prefix, starts once, and uninstalls; each ends with the leftover scan | No secret                                                                                                  |
  | `release-window-update`              | From `release-plan` on, beside the release build, on each Windows row's runner: the toolchains as in `build`, `pnpm desktop:package:test --build-id <commit>.<run number>`, then `node scripts/release-smoke-windows.mjs window-update <packaged Test window> --work <new directory> --diagnostics <directory>`, the real-window Update check below; a failure uploads the diagnostics as `release-window-update-<platform>` for three days                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | No secret                                                                                                  |
  | `release-window-update-macos`        | The same on each macOS row's runner, in bash, with the toolchains of `release-build-macos` and `node scripts/release-smoke-macos.mjs window-update`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | No secret                                                                                                  |
  | `release-gate`                       | After `release-feed`, only when `gate == 'true'`: one fresh `windows-latest` runner per selected older release downloads this run's `windows-x86_64` release artifacts with the signed gate copy beside them and runs `node scripts/release-smoke-windows.mjs gate v<older> --release <the older release's download address> --feed <gate copy> --work <new directory>`, which installs the older release from its published archive, pre-seeds `<store root>\desktop\` via `--installer-window-v1` when both feeds carry `extensions.desktop`, lets its own automatic helper install this release and window version from the gate copy while a terminal runs a probe, and checks active runtime and window versions, Client join, gapless probe output, idle move and layout preservation                                                                                                                                                                                                                                                          | No secret                                                                                                  |
  | `release-gate-macos`                 | After `release-feed`, only when `macos-gate == 'true'`: a fresh `macos-15` runner downloads `release-assets-macos-aarch64` and `release-gate-feed-macos-aarch64` and runs `node scripts/release-smoke-macos.mjs gate v<older> --release <the older release's download address> --feed <gate copy> --work <new directory>`, proving the `macos-aarch64` runtime and `<store root>/desktop/` window store (`codesign --verify --deep --strict` on the stored `Orchbee.app` bundle), Client join, gapless probe output, idle move and layout preservation before stopping the Session cleanly                                                                                                                                                                                                                                                                                                                                                                                                                                                           | No secret                                                                                                  |
  | `release-join`                       | Downloads every row's release artifacts and feed signature, places each signature beside its feed, runs `check` on each row, then `release-artifacts.mjs join`: one row is copied unchanged; several rows give one directory with one main npm package whose `optionalDependencies` list every row's platform package, the other package bytes kept, and one `SHA256SUMS`, which `check` then admits; uploads it as `release-joined`, so a re-run publishes the same bytes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | No secret                                                                                                  |
  | `release-publish`                    | Tag runs only (its condition also requires a `push` event), after `release-join`, every smoke job, `release-window-update`, `release-window-update-macos`, `release-gate` and `release-gate-macos` (a skipped macOS smoke or window Update check counts only when the plan has no macOS row, and a skipped gate only when the plan selected no path for that platform): downloads `release-joined`, and `release-artifacts.mjs check` verifies it with each feed's signature; then `release-publish.mjs github` creates or completes the draft release, never a pre-release, with the Install section and the tag's notes; then `release-publish.mjs npm` publishes platform packages first, the main package last, with `--provenance` and the dist-tag `next` for a pre-release, `latest` otherwise; then `release-publish.mjs publish` publishes the draft as the Latest release; then `gh workflow run update-check.yml` starts the update check                                                                                                 | Environment `release`; `actions: write`, `contents: write` and `id-token: write`, granted to this job only |

- The real-window Update check (`scripts/release-window-update.mjs`) drives a Test build, because
  only a development or Test window reads `ORCHBEE_DESKTOP_DEBUG_PORT` and opens a DevTools port on
  the loopback interface; a release build never does. Two copies of the Test window whose
  `app.asar` package version reads `0.0.0-window-update.1` and `0.0.0-window-update.2`, their window
  file lists written again (on macOS signed ad hoc again), stand in for older window versions. In a
  scratch store root and data root, the Test window's bundled Client runs `install --no-path`, each
  window version is installed by its own `--installer-window-v1`, the oldest starts as a person
  starts it, and a terminal the CLI opens prints a tick every second. Over the DevTools protocol,
  Settings › About › Update relaunches into the next version; the sidebar footer's Update shows the
  waiting notice with the newer version's bundled Client moved aside and the failure notice with
  the file the relaunch starts moved aside, then relaunches into the Test window's own version.
  Each relaunch must end the older window process, add `window-relaunch-started` to the diagnostic
  log, run one new window process from the expected program, show the Workspace the older window
  showed, name the new version in Settings › About, and leave the terminal printing. Nothing is
  downloaded; the stand-ins cannot prove that a published older window hands over, which the
  release gate proves for the runtime and window store.
- Secrets reach only the step that needs them, by environment variable: never an argument, a log
  line, or an artifact. A job that holds a secret installs no dependencies and runs only repository
  scripts that use Node built-ins, plus `gh` and `npm` in `release-publish`.
- The update feed document (`<stem>-<version>-<platform>.feed.json`, format and signature domain in
  the [release distribution proposal](../proposals/platform/release-distribution.md#release-feed))
  names the `.zip` (the `.tar.gz` on macOS) by address, exact size, and SHA-256, and the SHA-256
  of its `release.json`. In
  `extensions.desktop` it names the window archive the same way, with the SHA-256 of the
  `desktop-files.json` that `apps/desktop/scripts/package.mjs` wrote into the packaged window,
  beside the bundle on macOS ([window archive](../proposals/platform/automatic-updates.md#window-archive-and-file-list)); no
  reader acts on it before the window self-update release. Its signature is the one published
  top-level file `SHA256SUMS` does not list: `check` and the draft step admit it only when it
  verifies under the feed domain with the build's table, `RELEASE_KEYS` for the release stem and
  `TEST_KEYS` otherwise, and `check` confirms the feed against both archives and the release
  directory: the window archive holds one top-level folder whose list matches the digest, names
  exactly its other files by size and SHA-256, and bundles the feed's `release.json`. The draft
  step refuses a feed without its signature. The gate copy is never published: placed with its
  `.sig` beside the archives, it is a local feed for the release gate, and `check` admits it
  unlisted only with a signature that verifies the same way and archives named by their bare file
  names.
- A rehearsal cannot publish: `release-publish` requires a `push` event and a plan whose `publish`
  output is `true`, which only a tag run gives; every other job only uploads workflow artifacts, and
  `update-check.yml` starts only from `release-publish`. Jobs after a macOS job or the release
  gate, either of which may be skipped, state their condition with `!cancelled()` and each needed
  job's result, because GitHub skips a job whose ancestor was skipped. A failed gate path stops
  the release before Publish.
- The environment `release` has no required reviewers; cancelling a tag run before
  `release-publish` starts is the only stop. A failed npm step leaves the release a draft.
- A re-run never duplicates or replaces a publication. A draft release for the tag is completed with
  the assets it lacks and then published; a published release holding exactly this run's assets is
  only confirmed; a published release lacking an asset, an asset with another SHA-256, an asset this
  run did not make, or two releases for the tag stop the job without a change. An npm version the
  registry already holds with this run's tarball is skipped; with another tarball the job fails.
- The installer and its programs carry no Authenticode signature; the release manifest's Ed25519
  signature is the only one.
- The release path's artifacts carry release names and a release signature. A rehearsal's are
  rebuilt by the tag run, so an installed rehearsal build blocks the published one of the same
  version with `version-conflict`; never install them.

## Update check workflow

`.github/workflows/update-check.yml` proves that a published release can be updated to across
every self-updating platform and then rolls out and verifies the website update channel
([Releasing](RELEASING.md#each-release)). It is never a required check.

- Starts: `workflow_dispatch` with the input `tag`, a published tag such as `v0.1.0-beta.2`, which
  `release-publish` sends after publishing because a release published with its `GITHUB_TOKEN`
  raises no workflow; and `release: published`, for a release published by hand. A draft's assets
  answer 404 to an anonymous request, so no check can run before publication.
- Needs (`update-check` matrix): `permissions: contents: read`, no secret and no environment; every
  request is an anonymous HTTPS GET to the public release with the system curl.
- Does (`update-check`), in one job per platform whose release builds update themselves, each on a
  fresh runner of the one `release.yml` builds that row on (`windows-x86_64` on `windows-latest`,
  `macos-aarch64` on `macos-15`, `macos-x86_64` on `macos-15-intel`; Linux release builds withhold
  `update`):
  `node scripts/release-smoke-<windows|macos>.mjs update <tag> --release <release download folder> --work <runner temp>`,
  whose shared flow is `scripts/release-update-check.mjs`. The rows run independently, so one
  platform's failure leaves the others' results.
- The version people have now is the one the row's tracked update pair names
  (`apps/website/public/update/stable/<platform>/` at the checked-out commit: the tag for a release
  run, the chosen branch for a manual one), whose signature must verify with `RELEASE_KEYS`. Its
  archive (the `.zip` on Windows, the `.tar.gz` on macOS) is fetched from the address the pair
  signs, checked against the signed size and SHA-256, unpacked with the system tar and installed by
  its own `install`; when both the tracked feed and the offered feed carry `extensions.desktop`,
  its window archive is also fetched, verified, unpacked and installed into `<store root>/desktop/`
  via `--installer-window-v1` (skipped only for pre-window releases below `0.1.0-beta.14` that exit
  2 or 4 without an `orchbee.install` document). Then the installed command runs `update` with
  `ORCHBEE_UPDATE_FEED` naming the release's `orchbee-<version>-<platform>.feed.json` asset and
  must report `activated` with the release's version (and `result.desktop.activated === true` for
  the new window version when `desktop/` was seeded).
  - Windows runs `<store root>\bin\orchbee.exe update`, after which `active.json` and the
    launcher's `--version` name the release, and when `desktop\` was seeded `desktop\active.json`,
    `desktop\orchbee-desktop.exe` and `desktop\versions\<new>\orchbee-desktop.exe` match the new
    window version.
  - macOS works with HOME and TMPDIR in the work folder, so the store root
    `~/.local/share/orchbee`, the command link `~/.local/bin/orchbee` and the data root lie inside
    it. It runs `update` through the link, after which `active.json` names the release, the
    version's Client and the link answer `--version` with it, the link names that Client, and when
    `desktop/` was seeded `desktop/active.json` names the new window version and
    `codesign --verify --deep --strict` accepts `desktop/versions/<new>/Orchbee.app`; no process
    may run from the work folder afterwards, and the work folder is deleted. The release must be
    this Mac's own platform, so a row is never proven on the other architecture.
- Without a tracked pair (the first update-capable release of a platform), or when the pair
  already names the release, the release itself is installed from its archive and its `update`
  fills an empty scratch store through `ORCHBEE_INSTALL_DIR`, which still exercises feed,
  download, unpacking, publication and activation from GitHub. The scratch store sits beside the
  store root: the store refuses to create a missing root below a folder other accounts can read,
  such as the runner's temporary folder.
- A release that publishes no row of a platform fails that platform's row at its feed, which
  answers 404; move the channel only for the platforms the release published.
- A failure names the stage that failed and prevents `website-release` from advancing the channel.
  The Windows step installs for real, so run it only on a disposable Windows machine; the macOS
  step touches nothing outside its work folder. Their stand-in for a rehearsal on one computer is
  a test build's stem with `--stem`, a loopback address for `--release` serving the release's
  assets, and `--current` naming a folder with a test-signed pair; only a test stem admits a
  loopback address. Either script refuses a release stem with a loopback address (exit 2), an
  existing work folder, and a feed that is not public yet, which is how a draft answers.
- `website-release` (`ubuntu-latest`, `needs: [update-check]`, `environment: website`): runs only
  after every `update-check` row passes. Checks `node scripts/website-release.mjs should-advance <version>`
  against `main` first (exiting 0 without requiring secrets when `<version>` is strictly below
  `main`'s `release.ts` version, or equal with the live edge already matching), validates
  `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` via
  `node scripts/website-release.mjs preconditions`, runs `pnpm website:release <version>`, enforces
  `node scripts/website-release.mjs assert-clean-diff` (`release.ts` and
  `apps/website/public/update/<channel>/<platform>/feed.json.{signed,sig}` only, with
  `schemaVersion: 1` enforced on `/update/<channel>/`), runs `@orchbee/website` tests and
  `pnpm website:build`, commits and pushes to `main` with retry regeneration on concurrent pushes,
  deploys via `wrangler deploy`, and runs `node scripts/website-release.mjs verify-live` to compare
  every live `/update/<channel>/<platform>/feed.json`, `.sig` and `/install.sh` byte for byte
  against the tracked files.

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
