# TLA verification tooling

The private root workspace orchbee owns model selection, bounded execution and optional task-local
continuation and local success reuse. Product models remain with their workspaces. The repository merge gate runs cheap
inventory and implementation tests; actual model checking follows [Verification](../../docs/VERIFICATION.md).

## Commands

```sh
pnpm test:tla
pnpm test:tla:inventory
pnpm test:tla:tooling
node scripts/tla/run-models.mjs --changed-since main
node scripts/tla/run-models.mjs --models native-terminal-viewport
```

`pnpm test:tla` is the complete uncached baseline: inventory/runner tests first, then all retained model configurations and real TLC tooling witnesses in isolated concurrent groups. The complete invocation fails if it exceeds 60 seconds; success cannot be obtained by skipping configurations or reusing their results. Dependencies and the pinned JDK/jar must be installed for repeatable timing. Record three fresh-state runs when changing the portfolio or runner budget. Download and machine load affect measured wall time and are not mathematical model properties.

The portfolio has 15 protocol models and 17 configurations. A configuration has a 20-second deadline. The owner contracts state exactly which concurrency claims remain formal; local UI flows, deterministic transformations and fixed workflows use controlled or generated implementation tests.

`--run-dir` remains available for explicitly owned frozen-revision recovery outside the checkout. It requires a clean checkout and matching inputs/runtime/parameters. Ordinary commands permit dirty development inputs. The invoking task owns logs, state and snapshots and reclaims them after their last diagnosis or integration consumer.

## Local success cache

`pnpm test:tla:changed -- <base>` opts into `--cache-dir .tools/tla-cache`. Other selectors may
explicitly supply that option. `--all --cache-dir .tools/tla-cache` forces fresh execution and
refreshes successful evidence; `pnpm test:tla` remains an uncached full baseline. `--run-dir` and
`--cache-dir` are mutually exclusive: immutable task continuation retains its historical-failure
semantics. The local cache is derived evidence owned by this runner, removable at any time, with one
atomic envelope per registered model containing its canonical configuration records. An exclusive
lease protects reads, invalidation and publication; retired model envelopes and orphan temporary
files are pruned under that lease. The root must be the checkout's ignored `.tools/tla-cache` or a
canonical external directory, initially empty or bearing the matching ownership marker. Symlinks,
repository ancestors, unknown files and ambiguous writers are rejected. No upload or remote trust
is introduced; CI continues to execute its checks without this local cache.

Each configuration identity binds exact primary/supporting module and rendered configuration
bytes/paths, canonical variant names, required coverage, workers, heap, deadline, semantic TLC argv,
the pinned jar, concrete Java executable and runtime payload, Node identity and environment digest,
and the sorted production helper set and bytes. It deliberately excludes unrelated revisions,
claim-document text and other models. It proves the mathematical configuration, not correspondence
with changed product implementation; affected tests and refinement review remain required.
Injected Node/Java options, custom classpaths or module lookup paths refuse cache participation.

Load and validate every selected envelope before admitting a child. Missing, stale or corrupt
evidence is a reported miss; invalidate the whole affected envelope before fresh work. Forced full
execution invalidates all selected envelopes before the first child, including work a later failure
leaves unstarted. Permission, I/O, lock and unsafe-path errors fail rather than pretending to miss.
A matching configuration can reuse its complete log while another variant runs fresh. Logs retain
their raw bytes as base64 plus SHA-256; reuse writes and verifies a new regular run-local log and
rebuilds its report path. Output identifies fresh and reused configurations separately.

Fresh evidence requires successful child/stdio closure and a complete successful TLC terminal
report. Every invocation recomputes model-local coverage over all canonical configurations.
Only a complete successful model may publish its single envelope after input/tool revalidation.
Publish during that model's adjudication, before its scheduler slot is released; never wait for
other models to finish. A different model's failure or cancellation observed after the atomic
rename does not retract the published proof. Failed or incomplete models
publish nothing. Cancellation is checked immediately before atomic rename; an already initiated
rename may precede a concurrently observed cancellation. The invocation revalidates identity again
at completion and fails on drift. Previously published evidence stays keyed to its original inputs.
Ordinary cache corruption is recoverable by fresh execution; immutable run-directory corruption
still rejects continuation. Malicious local writers and power-loss durability are outside the claim.

Filesystem and CLI witnesses cover mutable cache admission/publication and immutable run records: invalidation before failure/cancellation, atomic whole-model visibility, exact mismatch categories, raw-log integrity, mixed fresh/reused coverage, changed inputs, ownership and full-refresh bypass. The workflow has no separate TLA model. No legacy log is imported as evidence.

## Modeling record

| Field           | Contract                                                                                                                                                                                                                                                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem         | Unrelated documentation and CI edits can select costly models; unweighted concurrency oversubscribes TLC workers; interruption repeats completed configurations. Preserve canonical properties, variants, workers and budgets while reducing unnecessary work.                                                                 |
| Inputs          | Validated canonical inventory, Git base and changed paths/documents across base, HEAD, index and worktree snapshots, declared resource limits, selected model IDs, frozen tracked inputs and explicit run directory. Invalid current inventory fails; unavailable history or unsupported scope parsing selects conservatively. |
| Data            | Configuration key is model ID plus variant name and execution-input digest. Model aggregate key binds model ID, complete canonical variant set and required coverage. Inventory owner is descriptive metadata. Immutable run manifest, unique log, committed outcome record and requested scope have separate authority.       |
| Outputs         | Fresh/reused configuration passes, configuration failures, model verification/coverage failures, skipped pending work and cumulative requested-model summary. No selected models is a successful no-op. Cancellation or incomplete selected evidence is a failure.                                                             |
| Invariants      | Preserve FIFO; bound process/memory slots and summed declared workers; stop new admission on current failure; drain active children; reuse only validated complete records; never combine coverage across model IDs; never convert a historical failure into a pass.                                                           |
| Decisions       | Valid history versus fallback; scoped versus shared document dependency; head job fits versus waits or oversized singleton; fresh versus reusable, historical failure, absent record or invalid committed evidence; complete model coverage versus failure.                                                                    |
| Model           | Deterministic selection/resource policy inside a process and persistence workflow. runModelChecks owns configuration admission and model-local adjudication; the shared runner owns child processes and task-run lifecycle.                                                                                                    |
| Transitions     | See the transition table. Child closure precedes success validation; immutable log closure/hash precedes atomic outcome-record publication.                                                                                                                                                                                    |
| Dependencies    | Git, Node, pinned TLC jar, Java, available CPU/memory, bounded child processes and task-owned filesystem and an opt-in local success cache. No external service, automatic retry or artifact upload.                                                                                                                           |
| Ownership       | One runner lease writes an explicit run directory. One configuration record owns its outcome; one model-ID aggregate owns its coverage result. Caller owns recovery and final reclamation. Product workspaces own their existing models and claims.                                                                            |
| Verification    | Selection positive/negative Git fixtures; weighted FIFO/failure/drain traces; seeded stateful oracle with replay and shrinking; record/log integrity, lease, cancellation and continuation tests; real pinned TLC default/support/variant and reuse checks.                                                                    |
| Correctness     | Implementation tests cover the workflow claims below with explicit finite traces and failure controls; no workflow metamodel is retained.                                                                                                                                                                                      |
| Reference scope | Existing runner invocation semantics, pinned TLC 1.7.4, fp 0, final liveness and coverage 1. Existing model workers, heap-sizing rule, properties, variants and deadlines remain authoritative.                                                                                                                                |

## Transitions and evidence publication

| State/event                            | Guard and effect                                                                                                                                                       |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pending to active                      | Validated run; FIFO head fits process/memory and CPU-worker budgets. Oversized declared worker count runs alone.                                                       |
| Child closes                           | Preserve complete log; exit, signal and original deadline determine success/failure. Failure closes admission before output drains.                                    |
| Validated to passed                    | Exit zero, complete TLC success and terminal state report; close/hash immutable unique log, then atomically rename one record referencing its exact hash.              |
| Pending to reused                      | Same frozen run, execution identity and intact committed pass/log. Reuse original coverage and TLC duration; report it as reused.                                      |
| Historical failure                     | Preserve committed failure; another explicit invocation may check pending work. The full requested scope remains failed.                                               |
| Invalid committed record               | Wrong schema, identity, digest, path or log integrity aborts continuation before TLC. No automatic repair/rerun.                                                       |
| Absent record or orphan temporary file | No pass evidence; work remains pending. Reclaim orphan owned artifacts after writers stop.                                                                             |
| Model adjudication                     | Verify only after every canonical configuration passes and the union of that model's log coverage satisfies its original markers. Persist coverage failure separately. |
| Successful cache model                 | Revalidate exact inputs and atomically publish the complete envelope during adjudication, unless cancellation has been observed. Other models need not be terminal.    |
| Cancel/current failure                 | Close new admission; terminate or boundedly drain owned children, report skipped work and release the lease after writers stop.                                        |

Atomicity covers one record publication under an exclusive lease, with no overwrite of a committed
configuration outcome. A log and record are separate files. Power-loss durability and malicious
filesystem/runtime writers are outside the claim. A stale or ambiguous lease requires caller recovery
after confirming Node/JVM writers stopped. Paths are canonicalized; symlink/reparse and traversal
escapes are rejected. Automatic cleanup targets only verified owned attempt children, never the
caller-supplied run root. Resumable identity includes Node executable/version/platform/options,
Java identity, pinned jar, helper and registered claim/input bytes, inventory, tracked revision and
every byte staged for TLC. The immutable manifest binds an independent run ID to the canonical run
root; relocating the directory does not preserve continuation identity. Registered inputs must
belong to HEAD and cannot hide changes through Git skip-worktree or assume-unchanged flags. The
cheap inventory gate rejects ignored inputs while permitting new, nonignored development files.
Revalidate frozen checkout/input identity at completion; a changed checkout or revision fails the
invocation and its summary remains incomplete.

Configuration and model pass publication check observed cancellation immediately before invoking
the atomic record rename. Cancellation after a temporary JSON write leaves no committed pass;
already committed configurations remain valid. An already initiated atomic rename may be ordered
before concurrent cancellation; the contract does not cancel an in-flight filesystem operation.

## Selection and scheduling

Tool runtime/helpers, supported TLC-job changes and verification/modeling standards select the compact full portfolio. Known test entrypoints stay in the mandatory inventory gate, which checks invocation, configuration overrides, the root tooling command and CI real-TLC witnesses. Product dependencies are matched independently and unioned with that selection. New product obligations must update their registrations and configurations.

Reviewed source exclusions are exact tracked files in `triggerExclusions`, never directory globs.
They remove only wildcard source triggers; primary/config/supporting modules, claims and exact
triggers cannot be excluded. New sibling files remain selected. Evaluate current and baseline
registrations independently and union the results, so adding an exclusion cannot hide a source
change in the same diff. Trigger/exclusion-only inventory changes select tooling evidence; all
other per-model metadata changes retain their conservative model selection.
Local menu presentation and copy-mode chrome use their owning menu/copy model or deterministic
Cargo render/localization evidence. They do not change retained-history protocol or relay authority.
Shared session/input/render roots, viewport coordination and protocol paths remain dependencies.
The inventory gate rejects stale, untracked, wildcard or direct-dependency exclusions.

Registered document scopes classify every supported top-level section. Preamble and reference-link
definitions are shared. Unknown, missing, duplicate or unsupported headings cannot silently narrow
selection. Scope changes select their model IDs. Only reviewed unrelated sections may be excluded;
proposals remain broad dependencies and never current claim authority. Workflow narrowing recognizes
the self-contained TLC job and complete workflow globals; ambiguous YAML/cross-job dependencies select all.

Process slots retain the existing 60% memory budget and 512 MiB per-process reserve. The heap ceiling
defaults to 4096 MiB; `--max-heap-mib N` selects a decimal integer ceiling from 256 through 8192 MiB,
without a sign or leading zeros.
Automatic slots use the selected ceiling plus the reserve. An explicit jobs count divides the same
aggregate budget; each actual heap is capped by the selected ceiling and must reach 256 MiB.
The CPU budget counts unchanged declared TLC workers. The jobs option caps processes and does not
override that worker budget. FIFO does not backfill lighter jobs. A worker count exceeding host capacity
runs exclusively. Auxiliary JVM/GC threads, OS scheduling, actual RSS and unrelated processes are
outside this declared-resource bound. Lower instantaneous concurrency does not enlarge the allocated heap.

| Heap-ceiling input                                  | Decision and observable result                                             |
| --------------------------------------------------- | -------------------------------------------------------------------------- |
| Absent                                              | Use the existing 4096 MiB default.                                         |
| Canonical integer from 256 through 8192             | Budget slots from that ceiling; clamp actual heaps to the allocated share. |
| Missing value, noncanonical integer or out of range | Reject before jar provisioning, Java invocation or run/cache publication.  |
| Allocated share below 256 MiB                       | Reject the concurrency before execution.                                   |

The frozen run context binds both the selected ceiling and actual heap. Changing the ceiling cannot
continue that run, even when memory pressure yields the same actual heap. Configuration/cache proof
identities retain the actual heap and deadline. Core boundary and generated resource-admission tests
cover normalization and aggregate limits; CLI tests witness the JVM argument and frozen identity.

## Claims and executable witnesses

The verification runner is tested through its implementation. It has no separate workflow TLA model.

| Claim                                                           | Maintained evidence                                      | Limit                                                             |
| --------------------------------------------------------------- | -------------------------------------------------------- | ----------------------------------------------------------------- |
| FIFO resource admission, failure drain and model-local coverage | `model-checks.test.mjs`, `execution-properties.test.mjs` | Declared CPU/heap budgets and finite generated schedules          |
| Complete immutable records and honest continuation              | `model-records.test.mjs`, `run-models.test.mjs`          | Exclusive writer, checked inputs and actual child/stdio closure   |
| Exact cache identity, invalidation and publication              | `model-cache.test.mjs`, `model-cache-cli.test.mjs`       | Local trusted filesystem; no power-loss durability claim          |
| Conservative selection                                          | `model-inventory.test.mjs`, `document-scopes.test.mjs`   | Registered dependencies and supported Git/document/workflow forms |
| Protocol properties detect their motivating faults              | `model-mutations.test.mjs`                               | Six guard/ownership mutants of the retained protocol cores        |

The scheduling corpus uses seeds 1 through 64, at most 128 commands, an independent oracle, replay and shrinking. The CLI suites retain cancellation, dirty/ignored/hidden inputs, historical failures, delayed stdio closure, partial results, mixed fresh/reused coverage and real pinned-TLC checks. Isolated fixtures run asynchronously with bounded concurrency; no case is dropped to meet the budget.

`pnpm test:tla:tooling` runs real TLC cache and mutation witnesses. Tool provisioning verifies the pinned SHA-256 and uses unique temporary downloads. Each JVM has its own state and standard-module extraction directory. Successful validation requires process and stdio closure, a complete success report, zero queued states and required action coverage. A killed or timed-out run is never accepted as a pass.

The process fixtures control child completion and publication order. They do not prove arbitrary operating-system scheduling, descendant pipe behavior, malicious filesystem writers or Windows console signal delivery.
