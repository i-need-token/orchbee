# Windows Worktree operations

This record owns native CLI Worktree creation, opening, removal and recovery. Git effects belong to the Windows Worker; the Server owns retained operations and canonical Workspace association. The CLI waits for observations and never runs Git itself. `worktree list` supports source-based discovery and preserves its existing registered-directory query contract.

## Commands and defaults

```powershell
orchbee worktree create --project demo --cwd C:/code/demo --branch feature/example
orchbee worktree open --project demo --cwd C:/code/demo --branch feature/example
orchbee worktree remove --workspace example --confirm
orchbee worktree status <operation-id> --format json
orchbee worktree resume <operation-id> --generation <attempt> --confirm
```

Create and Open accept either `--workspace <target>` or `--cwd <absolute-path>`. Without either, a validated caller terminal selects its canonical Workspace; outside Orchbee, the current directory is the source. A partial, stale or invalid caller identity never falls back to cwd. `--project` can be omitted when inferred from the Workspace or when exactly one Project exists. Ambiguous or absent Projects fail before submission; this command does not create a default Project.

Create accepts optional `--branch`, `--base`, `--path` and `--label`. The default branch is `worktree/<full-operation-id>`, base is `HEAD`, and path is `<main-repository-parent>/<main-repository-name>-worktrees/<branch-slug>`. The slug replaces characters outside ASCII letters, digits, `_` and `-` with `-`; an existing path is never overwritten. Existing local branches ignore the base. Open requires exactly one `--path` or `--branch`; ambiguous branches require a path. Both main and linked checkouts may be opened. An existing single Workspace in the same Project is reused; `--label` also renames it. A new association creates a Workspace and launches its first default-shell terminal. The optional four geometry flags follow `tab create`; a committed Workspace with failed or unknown terminal startup remains visible in the result.

Remove requires `--workspace` and `--confirm`. It checks registration, common Git directory and dirty state before closing the Workspace. `--force` explicitly permits Git to discard checkout changes. Main/bare checkouts and checkouts associated with another known Workspace are refused. Successful removal preserves the branch. There is no recursive filesystem deletion fallback.

`--trust-repository` grants command-local Git trust without changing global or repository configuration. Paths are literal absolute drive paths on Windows; no `~` or environment expansion is performed. Source paths may point inside a checkout. Reference operands are nonempty, at most 1024 UTF-8 bytes, and cannot start with `-` or contain ASCII controls. Git validates branch syntax. No command changes Client focus. Mac/Linux mutation execution and Desktop command exposure are outside this Windows implementation.

## Waiting and machine output

Create, Open and Remove accept a caller-provided nonzero 32-lowercase-hex `--request-id` or generate an operation identity before submission. Exchange request IDs are separate. Equal identity and intent observes the retained operation; changed intent conflicts. `--detach` returns the first accepted observation. Default CLI waiting is 120000 ms; `--timeout` accepts 1–3600000 ms. Each exchange is bounded at two seconds and polls are separated by 50 ms. Timeout, Ctrl+C or connection loss stops waiting; accepted Server work continues. The CLI submits Start or Resume once and follows only with Read. An uncertain handoff is never automatically replayed.

`worktree status` is a pure read with a five-second total budget and does not activate the Site or start maintenance. `worktree resume` requires the exact `attempt` from a resumable failed status, supplied as `--generation`, and `--confirm` because the retained intent can remove user work. Attempt numbers range from 1 to 64. Records survive Client disconnection but not Server restart.

The `orchbee.worktree.operation` version-1 envelope is registered in `api schema` catalog version 22 (57 documents). It contains the operation ID, local Site identity when known, a correlated Server reply or explicit unavailable/unknown observation, and a separate nullable waiting disposition. Pending and detached observations exit 0; terminal launch failures retain their own nonzero exit; Git or association failures exit 3, unavailable admission exits 4, lost connection exits 5, internal or inconsistent observation exits 1 while retaining the last reply, timeout exits 7, physical/submission uncertainty exits 8, and cancellation exits 130. Parsing and pre-submission target failures use the existing CLI failure envelope. Raw bounded Git diagnostics remain byte arrays in JSON and are escaped in localized text.

## Retained continuation

| State or event     | Admission and next step                                                                  | Retained truth                                                     |
| ------------------ | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Start              | Exact Site grant, Project, source and Worker incarnation; one pending operation globally | Intent and operation ID before Worker handoff                      |
| Create             | Observe source, create with Git, verify checkout and branch, attach Workspace            | Candidate path and possible Git effect survive failed verification |
| Open               | Observe the selected checkout, reuse or create its Workspace                             | Canonical Workspace identity and terminal launch outcome           |
| Remove             | Inspect first, commit Workspace Close, wait for known Closed, remove with Git            | Close request ID and Closed fact survive later Git failure         |
| Resume             | Same intent and Worker incarnation, exact failed attempt, explicit eligibility           | New fenced attempt; old callbacks cannot overwrite it              |
| Read or disconnect | Read retained state only; disconnect does not cancel continuation                        | No duplicate Git or Workspace creation                             |
| Site stop          | Stop and join the retained submitter before Close maintenance and Worker freeze          | Cancellation is not rollback; Worker owns physical settlement      |

The Server retains at most 64 operations and permits at most 64 attempts per operation. Worker jobs have their separate retained 64-admission bound. A continuation has a cooperative 120-second work budget; each Git step is bounded at 40 seconds, while the Worker owns its 30-second execution and five-second physical cleanup budgets. Reads and duplicate observations remain available when fresh admission fills. Unknown physical or transport settlement is not resumable. A Create with possible Git effects resumes by observing its retained path and branch rather than issuing another Git add. Remove recovery keeps an already closed Workspace closed and uses the retained repository root after the source checkout has gone.

Registry-to-Site locking protects admission and canonical checks; IPC occurs outside both locks. While Remove is pending, new Workspace launches and primary-directory admissions are reserved globally. Existing associations are rechecked before close and deletion. These checks do not exclude arbitrary external Git/filesystem writers or constitute a filesystem transaction. Terminal outcomes are immutable once published. Maintenance owns its thread and explicitly joins on Site shutdown; Drop only requests cancellation.

## Source-based discovery

`worktree list --workspace <target>` and `worktree list --cwd <absolute-path>` select one source, with optional `--trust-repository`. With neither selector, a validated caller terminal selects its Workspace; outside Orchbee the current directory is used. Invalid caller identity never falls back. Listing needs no default Project, creates no Workspace and changes no focus. The existing `--project <target> --directory <id>` form retains its original request and result contract; the source form uses `orchbee.worktree.query`.

Source queries use a five-second total CLI budget, do not activate a Site, and never replay an exchange. The source and legacy selector sets are mutually exclusive. JSON emits `{schema:"orchbee.worktree.query",version:1,result:<typed-reply>}` from catalog v22/57; a Server failure remains a typed reply. Exits are 0 for Found, 3 for missing/denied/changed source or Git failure, 4 for unsupported/unavailable, 5 for transport/malformed listing, 7 for timeout, and 1 for I/O/spawn/output-limit failure. Pre-admission/transport failures use the generic CLI failure schema; Ctrl+C exits 130 after releasing the signal owner. Human text escapes untrusted paths/labels and labels main/linked, detached/bare, locked/prunable and Workspace identities.

The Server resolves the source and live Worker incarnation, releases all gates during Git IPC, then rechecks both before publishing. One read-only Worker job observes the common Git directory, source checkout root and porcelain listing. Command-local trust applies only to the selected canonical repository, without persistent configuration changes. Admission checks only drive-path syntax. Filesystem canonicalization and trust preparation run inside the owned task, followed by cancellation/deadline checks before child creation; missing directories return a typed preparation failure. OS filesystem calls are cooperative, not forcibly preempted. The read owner retains its five-second budget, one physical slot, bounded output and sixty-second receipt; it consumes no mutation-operation capacity. Windows Git drive paths pass the same absolute-path validation as other Worktree entries.

The result reports the main repository root, common directory, source checkout, Git entry facts and known canonical Workspace associations. Multiple associations remain explicit, never an arbitrary first Workspace. Explicit source-Workspace membership is supported by its own Git observation; other associations require a known matching checkout path. This is an observation of Git followed by a canonical membership check, not an atomic filesystem transaction or proof about unknown path aliases.

| Transition                          | Requirement                                   | Result                                          |
| ----------------------------------- | --------------------------------------------- | ----------------------------------------------- |
| Admit                               | Valid read policy, source and live Worker     | Capture source membership and Worker generation |
| Observe                             | No coordinator/registry gate held             | One bounded read-only Git job                   |
| Recheck                             | Same source membership and Worker incarnation | Join current known Workspace identities         |
| Source or Worker changes            | Do not reuse the old observation              | Explicit failure                                |
| Timeout, cancellation or disconnect | Worker retains physical cleanup ownership     | No mutation and no inferred success             |

Strict request/result and binary-frame checks, shared Rust/TypeScript fixtures, source subdirectory/linked-checkout cases, denied trust, source/Worker changes and real Windows CLI reads provide evidence. Legacy listing remains covered independently.

## Verification

Maintained tests cover strict grammar and confirmation, caller identity versus cwd, partial outcomes and escaped diagnostics, operation/exchange correlation, no replay after EOF, authorization and capacity, detached reuse, dirty refusal before Close, failed removal after Close and explicit recovery, and cancellation before Site shutdown. Real disposable Windows Git fixtures cover source subdirectories, main-checkout opening, branch reuse, force removal, branch preservation and a successful hook changing the selected branch. Rust and TypeScript consume the same Worktree conformance corpus; JSON Schema covers structural validation and bindings enforce byte bounds and cross-field relationships. These are same-author tests and review; they do not claim independent review or TLA execution.

## Workspace group closure

`workspace close <target> --group --confirm` closes a captured worktree group without deleting any Git checkout or branch. A known main-checkout Workspace with other associated Workspaces rejects ordinary Close with `workspace-group-close-required`; a linked-checkout Workspace closes only itself, even with `--group`. Membership uses observed checkout/common-directory facts and the exact canonical Directory and Worker incarnation, never labels or arbitrary path prefixes. Create and Open also retain their source Workspace's observed checkout, including a primary Directory inside that checkout. A changed Directory invalidates that observation. An exact main-root Directory may join a known repository group; ambiguous repository identity is refused.

Before each child submission, the group also requires a live Close maintenance owner; absence returns a resumable admission failure without accepting even an empty Workspace.

The Server freezes at most 128 members at admission, closes the selected source last, and rechecks each not-yet-submitted member's Directory and Worker before handing it to the existing Workspace Close owner. New Workspaces are not silently added. Each child Close retains its own identity, generation and lifecycle truth. Only a token for the retained group member can bypass the ordinary parent guard; there is no wire bypass flag. A failure preserves all closed, pending and unsubmitted members. Git and canonical closure are not a transaction, and closed terminals are not resurrected.

The `workbench.workspace-group-close` request has Start, Read and attempt-fenced Resume actions. The result names the operation independently of the exchange and reports the captured Project/Workspace labels with required-nullable compact child Close records. A record always preserves the accepted request ID; its required-nullable `observation` contains the attempt generation and status when observed. An accepted but unobserved child cannot authorize replay or Resume. Requests are bounded at 16 KiB and results at 512 KiB before allocation. There are at most 64 retained operations, one Pending globally, and 64 attempts per operation. Equal Start identity and scope returns the retained observation; a changed scope conflicts. Read needs no live Workspace. Records survive Client disconnection, not Server restart.

| State or event                     | Guard and transition                                                         | Observable truth                                                    |
| ---------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Start                              | Authorized lifecycle grant, available owner, current source/Worker, capacity | Freeze exact ordered members before child submission                |
| Child admission                    | Exact retained member and current canonical source                           | Store one child Close ID before waiting                             |
| Child Closed                       | Known Close tombstone                                                        | Advance to next member, source last                                 |
| Child blocked or admission refused | Stop the sequence                                                            | Failed with partial members; known safe failures may resume         |
| Unknown outcome or changed source  | Do not submit another Close                                                  | Nonresumable failure, original child truth preserved                |
| Resume                             | Exact failed attempt and declared eligibility                                | New attempt, same members and child identities                      |
| Read, timeout or caller exit       | Read does not advance state; caller only stops waiting                       | Retained owner continues independently                              |
| Site stop                          | Stop and join group submitter before Close maintenance freeze                | No further child admission; existing Close owner retains settlement |

`workspace close-status --group --request <operation-id>` reads the retained group after any member has disappeared. `workspace close-resume --group --request <operation-id> --generation <attempt> --confirm` resumes a known eligible failure. Close and Resume support detached observation and bounded waiting; cancelling that wait does not cancel accepted closure. The separate `orchbee.workspace.group-close` version-1 document projects these facts without changing ordinary Workspace Close output. Native execution is Windows-only; Desktop command exposure is tracked separately.

The correctness case maps membership selection to deterministic main/linked/duplicate/source-change cases, ordered continuation to partial failure/replay/generation traces, and transport to shared Rust/TypeScript/schema fixtures and bounded frame tests. Real Windows traces must prove terminal closure while Git paths remain and explicit Site-stop joining. These are bounded same-author evidence; no atomic filesystem claim, independent review or TLA execution is implied. Reference semantics are Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `workspace_close_indices` and `handle_workspace_close`; focus/event projection remains governed by the separate Client contract.
