# TLA verification tooling

The private root workspace orchbee owns model selection, bounded execution and optional task-local
continuation and local success reuse. Product models remain with their workspaces. The repository merge gate runs cheap
inventory and implementation tests; actual model checking follows [Verification](../../docs/VERIFICATION.md).

## Commands

```sh
pnpm test:tla:inventory
pnpm test:tla:tooling
node scripts/tla/run-models.mjs --changed-since main
node scripts/tla/run-models.mjs --models tla-verification-execution
node scripts/tla/run-models.mjs --models tla-verification-execution --run-dir ../orchbee-tla-task
```

Run directories are explicitly supplied, owned by the caller, outside the checkout, and used for one
frozen task revision. Repeating a command with the same directory continues that run; selecting another
model extends its requested scope. Ordinary commands support dirty development checkouts. Resumable
commands require a clean checkout, including untracked files, and matching inputs, runtime and parameters. The caller
reclaims records, logs and snapshots after the last integration or recovery consumer finishes.

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

The execution model separates mutable cache admission/publication from immutable run records.
Cache freshness and publication properties supplement the existing record, coverage, resource and
completion properties. Filesystem/CLI witnesses cover invalidation before failure/cancellation,
atomic whole-model visibility, exact mismatch categories, raw-log integrity, mixed fresh/reused
coverage, changed inputs, ownership and full-refresh bypass. No legacy log is imported as evidence.

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
| Correctness     | The root execution model checks finite safety/progress; the claim table states refinement witnesses and limits. Historical aggregate rehydration additionally requires the named persistence-composition test.                                                                                                                 |
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

Selection unions current and baseline dependencies, including both rename/deletion paths. Removing
one inventory entry selects its remaining dependents rather than every model. Changes to top-level
execution metadata, workflow globals and invalid/non-ancestor history remain conservative.
Changed paths union base-to-HEAD, HEAD-to-index and index-to-worktree differences. Document and
workflow scopes inspect each adjacent pair of those snapshots; a staged relevant change cannot be
hidden by reverting the working file. Simultaneous staged and unstaged inventory edits, or a third
distinct intermediate inventory, select all models rather than omit an intermediate registration.
Tool runtime/helpers, supported TLC-job changes and verification/modeling standards select the root
execution model; this explicit route precedes the corresponding broad sharedTriggers entries.
Known test entrypoints stay in the mandatory cheap gate, which checks their actual invocation,
every registered configuration override, the root tooling command and the CI real-TLC witness.
Product owners independently match the original paths and union with tooling selection, including
any explicitly declared dependency on a test, workflow or inventory file. New product obligations
in standards require updates to their registrations/configurations, which select those models.

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

| Claim                             | Formal property                                      | Implementation evidence                                                                                           | Limits                                                  |
| --------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| Resource admission                | ResourceSafe                                         | Weighted FIFO traces and generated scheduling oracle                                                              | Declared workers/heaps; oversized singleton exception   |
| Valid evidence only               | CacheSafe, CommitSafe, RejectedSafe, RecordStability | Integrity, wrong-run, stale/partial/log-corruption and atomic-publication tests                                   | Trusted runtime and exclusive filesystem writer         |
| Failure closes admission          | FailureSafe, AdmissionSafe                           | Failure-before-stdout-drain and cancellation process traces                                                       | Child termination and I/O eventually complete           |
| Model-local complete coverage     | ModelSafe                                            | Mixed reused/fresh same-model coverage and same-owner different-model isolation                                   | Canonical variants and required markers                 |
| Historical aggregate stays failed | ModelSafe for current adjudication                   | Named trace: persisted model coverage failure survives cached configuration continuation while another model runs | Historical aggregate rehydration is outside finite Init |
| Finite completion                 | Completion                                           | Exhaustive named traces plus seeded/replayed stateful corpus                                                      | Fair child closure, record I/O and adjudication         |
| No missing selection              | N/A: stateless policy                                | Git/document/workflow positive controls beside each exclusion                                                     | Reviewed semantic ownership and supported parsing       |

For mutable success reuse, CacheFreshness and CacheAdmissionSafe are witnessed by exact-input,
corrupt-envelope and forced-refresh controls; CachePublicationSafe by whole-model coverage,
failed-model isolation, cancellation and publication-time identity-mismatch witnesses. CacheCompletion adds eventual
publication for eligible successful models under the same fair child/I/O assumptions, independently
of peer completion. A real-TLC boundary witness checks the completed envelope while a peer is held
before execution, then cancels the invocation and verifies exact reuse of the completed model.
Cancellation may preempt publication, so progress terminates at publication or observed cancellation.
The finite model permits interleaved adjudication/publication; the held-peer witness additionally
checks the runtime's stronger callback ordering before releasing the scheduler slot.

The finite model fixes `InputsAtPublication` for each run; it checks the identity condition at
publication against frozen proof inputs. Mutable filesystem drift after publication and final
invocation identity are outside this refinement claim. The real CLI drift witness changes an input
after publication, requires the final invocation to fail, then restores the exact bytes and verifies
reuse of the earlier successful models. A separate real CLI cancellation witness cancels after the
sole model's configurations close and requires its unpublished envelope to remain absent.

[TlaVerificationExecution](../../formal/tla/TlaVerificationExecution.tla) models three configurations,
two model IDs, two slots, CPU budget two and job weights one/two/three. Sixteen variants cover fresh,
parallel, oversized, successful/historical-failed continuation, stale identity, corrupt cached log,
incomplete fresh log, missing coverage, exact cache hits, stale/corrupt cache inputs, forced refresh
and publication-time identity mismatch. Named action markers require each semantic branch.

`pnpm test:tla:tooling` checks this model and the real pinned-TLC cache boundary witnesses. The CI
TLA job runs those boundary witnesses after its selected models, which provision the pinned jar.
The ordinary inventory gate keeps filesystem/cache negative controls Java-free; it does not silently
skip the real-TLC witnesses when their prerequisites are missing.

The stateful corpus uses seeds 1 through 64, bounded configuration/resource dimensions and at most
128 commands. Its independent oracle does not call production transition helpers. Versioned trace
serialization must replay deterministically; shrinking removes command chunks/actions and simplifies
counts/weights while preserving dependencies and the original failure fingerprint. Every declared
boundary must be reached. Reduced failures become replay regressions. These checks do not prove
unbounded model behavior or arbitrary operating-system/filesystem recovery.

The generated event oracle checks validated historical/pending record classes at the scheduler
boundary. File integrity and atomic publication are checked separately by record and CLI tests.
The runtime identity probe may launch Java for version information before proof validation; it
does not launch TLC. Default and variant invocations execute isolated copies of the primary module,
registered supporting modules and exact applied configuration. The pinned standard modules come
from the hash-verified jar.

The delayed-close process witness uses real Node child exits and sibling completion, with the Java
test adapter controlling stdio-close delay. It distinguishes immediate exit failure from waiting
for close; it does not establish arbitrary descendant pipe inheritance on every host.
The lease-acquisition cancellation witness sends an OS SIGTERM on POSIX and injects the equivalent
Node signal event on Windows; Windows console delivery itself is outside that fixture's evidence.
