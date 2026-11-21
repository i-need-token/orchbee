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
equivalent; add `--plan` to print the plan without running checks. `scripts/verification-scope.mjs`
computes one immutable plan from the changed paths and the current pnpm and Cargo workspace graphs,
and `scripts/run-checks.mjs` turns it into steps. `pnpm verify`, `pnpm test` and CI run every host
check and suite and catch what a mapping here leaves out.

### Base and changed paths

The base must resolve to an ancestor commit of HEAD, because only such a base bounds the change. A
reference that does not resolve, or a commit that is not an ancestor, fails the command before any
plan or check, also with `--plan`; its message names the two remedies: rebase onto it, or pass the
fork point (`git merge-base <base> HEAD`). The changed paths are the union of base-to-HEAD,
HEAD-to-index and index-to-worktree changes plus nonignored untracked files, read from NUL-delimited
Git output; both ends of a rename count. Hidden Git inputs (skip-worktree, assume-unchanged), an
unknown path or an unsupported dependency graph select the complete gate, so an unrelated untracked
file can cause a full run; the resolver does not delete or ignore it.

### What always runs

oxfmt and each workspace's own format check, the TLA inventory check, the library build, every
command of the `lint` script except Clippy, type checking, and `test:lint-rules`, whose Markdown
link check covers every tracked file. Their inputs cross every workspace. The changed gate reads the
`format:check` and `lint` scripts from the root `package.json`, so a command added there runs in both
gates.

### What the plan narrows

| Check                                                                     | Runs when                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| rustfmt and Clippy                                                        | A Cargo owner is selected. Clippy takes the selected members with `--package`; the lint task's source and manifest policies still cover the workspace. A change to `scripts/run-rust-workspace.mjs`, which holds those policies, lints every member.                        |
| Workspace suites                                                          | Their owner is selected. pnpm suites use the normal workspace commands; Cargo members keep all targets, all features, the lockfile and documentation tests. Node suites finish before Rust tests start, and real-process tests first build the host executables they start. |
| Root tooling suites and the TLA runner tests                              | A root tooling input changes: anything under `scripts/`, a root pnpm manifest, `knip.config.mjs`, or a workflow file. One suite also runs when a repository file it reads changes; `toolingSuites` in the scope script lists those files.                                   |
| `test:exports`                                                            | A `packages/*` owner is selected or a root tooling input changes.                                                                                                                                                                                                           |
| Terminal Host test compilation for `x86_64-linux-gnu` and `aarch64-macos` | Terminal Host Zig source or build files change. The host suite compiles for its own system only.                                                                                                                                                                            |

An unselected suite is reported as unaffected: it is unexecuted, never a pass or a cache hit. There
is no test-result cache; native compilers keep their normal incremental behavior.

### Owners and consumers

A changed path selects the workspace that contains it, then every transitive consumer across
normal, development, optional, peer, build and target-specific local dependencies. The scope script
also records the edges package managers do not show:

- foreign edges for native process cohorts and Storybook discovery;
- test witnesses, suites that run another owner's built artifact while their own is unchanged, which
  join after closure without selecting their consumers;
- vendored Cargo packages under `third-party/` that Cargo resolves for a member: a change there
  selects the members that resolve it, and the package has no suite of its own; and
- exact shared inputs: `schemas/terminal/` selects the Rust protocol crate, the TypeScript protocol
  package, the Terminal Host and the connection handshake crate, whose registry test checks the
  [frozen fixtures](../schemas/connection/README.md#frozen-fixtures); the listed `schemas/workbench/`
  parity files and `schemas/connection/v1/connections.json` select their readers.

A changed `package.json`, `pnpm-lock.yaml` or `pnpm-workspace.yaml` selects every pnpm owner, and a
changed `Cargo.toml` or `Cargo.lock` every Cargo owner; a manifest inside an owner also selects that
owner. A workflow file selects the root tooling suites only: no test reads it, and its real check is
its run on GitHub.

Documentation without a test reader selects no suite and reports `documentation <- <path>`:
anything under `proposals/`, and Markdown under `docs/`, under `schemas/`, or directly at the
repository root. The `docs/tui/` standards are the exception by review policy: they select the
native terminal client. Markdown inside an owner selects that owner. Other files outside an owner
remain unknown and select the complete gate.

A change that adds a cross-owner input, or a test that reads a file outside its owner, adds the
mapping and its positive control in `scripts/verification-scope.test.mjs` in the same change. This
policy does not waive change-triggered TLA checks.

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
The dependency command prints Knip's own report under
[Linting](LINTING.md#dependency-graph-boundaries). Its Knip children have a 180-second deadline;
process errors, signals and timeouts always fail.

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
| `lint`                    | Code, Rust workspace and Clippy, style, storage, architecture, dependency, export, and manifest rules pass.                                                     |
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
- `scripts/run-rust-workspace.mjs` connects the root pnpm gate to Cargo. It validates the pinned
  toolchain, then runs rustfmt, Clippy, build, and tests over every member with `--locked`. The test
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
  the bound leaves more than six times that. A 25-minute deadline bounds the whole `zig build test`
  run, including compilation and the native witnesses outside Zig's test runner. GitHub's Windows
  runner is the slowest host: with the moves between Hosts, its run took 15.0 to 17.7 minutes, and
  their black-box run adds about one more, so the bound leaves over six minutes above the slowest
  run. It stays inside the CI job limits (30 minutes for `terminal-host-native`, whose earlier steps
  take about 2 minutes, and 60 for the macOS `client-commands` job, which runs the tests after its
  Rust part) so the gate reports before CI cancels the job. Measurements: `windows-latest`, Zig
  0.16.0, 2026-11-21. On expiry the runner lists the test binaries still running and the process
  tree that run started, terminates only that tree, and fails with exit 124. Either timeout fails
  the stage, never passes or skips it. Remeasure before changing either bound.
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

`scripts/tla/models.json` registers every protocol model and its configurations (one per named variant, otherwise one per model), with semantic owners, claims, implementation triggers, required action coverage and a 20-second configuration deadline. This is the whole maintained portfolio, not a fast subset in front of a hidden required full suite. New models must fit the complete budget and the admission criteria in [Modeling and Correctness](MODELING-AND-CORRECTNESS.md). A timeout is a failure, not a way to skip work.

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
  other. GitHub cancels a macOS job that no runner took within 15 minutes and reports the run as
  failed although nothing ran; when every other job passed, the `macos-runner-shortage` job ends
  that run as cancelled instead. No Verify job runs on an Intel Mac runner; Intel macOS is left to
  the release workflow's `macos-x86_64` row.
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

## Release and update-check workflows

`.github/workflows/release.yml` and `.github/workflows/update-check.yml` are never required checks.
The comments in those files and in the scripts they run state what each job does, in what order,
and why; the owner's steps are in [Releasing](RELEASING.md). Three rules bind every change to them:

- Secrets reach only the step that needs them, by environment variable: never an argument, a log
  line, or an artifact. A job that holds a secret installs no dependencies and runs only repository
  scripts that use Node built-ins, plus `gh` and `npm` in `release-publish`.
- Only `release-publish` holds write permissions, and it runs only for a pushed tag. A rehearsal
  and a dry run publish nothing.
- The Windows smoke, gate and update steps install and remove for real: run them only on a
  disposable Windows machine. The macOS steps touch nothing outside their work folder.

## External-source audits

- A content-addressed native source dependency used by a production build is a declared build input,
  not an external-source audit. Its immutable URL and digest belong in the owning native manifest.
- Package-specific audits requiring an external checkout, credential, service, device, or undeclared environment input remain explicit maintenance commands documented by the owning package and stay outside `pnpm verify`.
- Promote an audit to a separate CI job only when it pins and provisions every external input, has a stable owner, and adds no side effects to the required merge gate.
