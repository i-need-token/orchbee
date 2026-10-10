# Workspace terminal directories

Status: implemented local protocol and runtime slice; native Client UX and public delivery remain outside scope.
The product decision owner accepted this slice in the active development conversation. This record
owns the Server's Directory association and Workspace terminal-creation behavior. The
[Workbench authority](workbench-authority.md) owns its ledger and canonical commit machinery.

## Outcome and scope

An admitted local Client explicitly associates an empty, unconfigured Workspace with a directory
on the selected local Worker. The Worker validates that directory before Server accepts the
association. Later `tab.create` in that Workspace uses the accepted path and exact Worker. Missing
configuration or an unavailable Worker never falls back to home. Ordinary standalone startup is
unchanged. The first consumer is the local protocol and process tests, not a new TUI or Desktop UI.

Project owns ordered `Directory { directoryId, workerId, path }` records. Workspace holds an explicit
nullable `primaryDirectoryId`, referencing a Directory in its Project. Newly created Projects have
no Directories; newly created Workspaces are unconfigured and Empty. Directory identities use the
existing nonzero 128-bit identity form and Site allocation cursor. A Project admits at most 256
Directories, matching its Workspace bound. Paths follow the existing authorized POSIX cwd contract:
absolute UTF-8, 1 through 4,096 bytes, no NUL. Preserve path spelling and follow symbolic links when
validating; no realpath-based identity, enumeration, Git discovery or directory creation is implied.

The explicit `workspace.primary-directory.accept` action carries Project ID, Workspace ID and path
under the existing command request ID, Site and expected revision. The Server selects the current
local Worker; the Client supplies no Worker generation, fence or lease. This first operation creates
one Project Directory and selects it for that Workspace atomically. It is set-once and requires an
empty context. Sharing existing Directory records, rebinding, deletion and general Directory CRUD
remain outside this slice. An independent directory-association grant is required in addition to
navigation authority. Read-only snapshot access does not confer association or launch authority.

## Admission and transitions

| Phase                  | Input and guard                                                                                                                               | Result and owner effect                                                                                                        |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Admission              | Grant, live authority, exact replay/conflict, ledger capacity, Site/revision, exact Project/Workspace, empty context and no primary Directory | Definite refusal preserves canonical state; exact replay returns its retained result without validation                        |
| Prepared validation    | A compatible current Worker and bounded request slot exist                                                                                    | Capture exact Worker identity/generation and path; release registry and Site locks before I/O                                  |
| Worker observation     | Exact request/generation; directory metadata lookup on that Worker                                                                            | Correlated valid, invalid-directory, unavailable or wrong-worker result; no Host, shell or canonical mutation                  |
| Revalidation           | Validation succeeds before the carrier deadline                                                                                               | Reacquire registry then Site; repeat admission and require the captured Worker generation still current                        |
| Commit preparation     | Identity, Directory count, revision, snapshot encoding and ledger bytes fit                                                                   | Prepare complete projection and fixed result before any canonical mutation                                                     |
| Commit                 | All prepared checks succeed                                                                                                                   | Append Directory, select it, advance one revision and retain one immutable acceptance result atomically                        |
| Result loss            | Commit may already have happened                                                                                                              | Client does not replay effects blindly; an exact command replay returns the original retained acceptance                       |
| Workspace Tab creation | Exact context resolves to its Workspace and configured Project Directory                                                                      | Derive explicit cwd and exact Worker constraint; jointly commit membership and terminal intent using existing Create machinery |

Different commands racing to configure one Workspace cannot both commit. Concurrent exact requests
may perform duplicate read-only validations before either commits, but only one logical association
commits. Negative/unavailable validation and stale Worker generation consume no canonical identity,
revision or retained ledger entry. A revision conflict after validation does not silently rebase.
Association success is `workspace-primary-directory-accepted` with revision, Directory ID and Worker
ID. Missing configuration is `workspace-directory-unavailable`; an already configured Workspace or
nonempty context is `workspace-directory-conflict`. Invalid/unavailable directory observation uses
`directory-unavailable`; carrier/Worker unavailability retains existing typed unavailable results.

Workspace Create does not read, replace, complete or notify the standalone first-Open slot. Its
member bindings are exposed through the existing authorized Snapshot and OBCS AttachBinding path.
The existing pending/final Create ledger, immutable completion tokens and unknown-outcome rules
remain authoritative. A path removed after association produces a truthful launch failure and
retains the committed membership; it never triggers another launch in home. Association validates
one point in time, not future accessibility. Server admission/result deadlines do not make arbitrary
filesystem syscalls interruptible; the process claim remains the supported local Linux filesystem
cohort. No new cross-generation restart/adoption guarantee is introduced.

## Correctness and evidence

Server is the sole writer of Directory identities, selection, revision and retained command results.
Worker owns only point-in-time cwd validation and existing launch admission. Client owns neither.
The association model must cover two Clients, concurrent validation, revision changes, Worker
replacement, result loss/replay, resource exhaustion and at-most-once commit. The Workspace Create
model must cover exact canonical Worker/cwd, joint commit, pending/final completion and an unchanged
standalone slot. Existing standalone models and action obligations remain required.

Strict schema and independent Rust/TypeScript bindings must reject dangling or cross-Project
references, duplicate identities, missing required nullable fields, invalid paths and bounds.
Deterministic and stateful tests must map each admission/revalidation branch to its state-preserving
or committing result. Process tests must exercise a real directory, two authorized observers,
Workspace Create and AttachBinding, an unchanged standalone slot, missing/deleted directories and
Worker loss without a home launch. Models alone do not establish filesystem or carrier behavior.

| Claim                                                                                                    | Executable evidence                                                                                                                                                                                                          | Limit                                                                                                                                                                                                          |
| -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Concurrent association, exact replay and revalidation preserve one canonical acceptance                  | `WorkspaceDirectoryAssociation` and `workbench_authority/tests/workspace_directory.rs`, `local_terminal_service/workspace_directory/tests.rs`                                                                                | Bounded request flights; in-memory Site only                                                                                                                                                                   |
| Directory count, encoded projection, identity, revision and both ledger limits preserve state on refusal | `directory_count_and_encoded_snapshot_limits_fail_before_commit`, `association_capacity_failures_preserve_canonical_state_and_ledger`, `exact_association_replay_survives_both_full_ledger_limits`                           | Implementation bounds; model capacity abstractions do not prove byte accounting                                                                                                                                |
| Workspace Create uses canonical placement and preserves standalone ownership                             | owner implementation tests, `workspace_create_uses_canonical_worker_and_cwd_without_touching_standalone`, `workspace_completion_does_not_settle_a_pending_first_open`, `workspace_completion_preserves_a_settled_first_open` | Empty, Creating and Settled standalone cases; no migration or recovery                                                                                                                                         |
| Worker replies retain exact request correlation and carrier retirement                                   | owner implementation tests including its cwd-validation configuration, protocol malformed cases and carrier echo-mutation tests                                                                                              | Existing queue and wire deadlines; no interruptible filesystem-call claim                                                                                                                                      |
| Validation can repeat before any Host startup                                                            | `validation_before_first_launch_uses_real_socket_and_never_creates_host_runtime`                                                                                                                                             | Real lazy Worker loop and Unix socket with a fixture cohort; no launched shell                                                                                                                                 |
| Two Clients observe one association and use a real Workspace shell                                       | `real_server_worker_workspace_directory_association_and_tab_lifecycle`                                                                                                                                                       | Real local Server, Worker and prestarted Host; missing-path retry, exact cwd marker, AttachBinding/Detach, independent standalone, deleted-path failure with retained membership, Worker loss without fallback |

The lazy-loop regression distinguishes `server_transport::send`'s Host-closure flag from a Server
write error: with no Host yet, a successful validation response must keep the Worker available for
the next validation or Launch. Its two consecutive probes exercise that boundary directly.

## Delivery and compatibility

Protocol, Server and Worker activate together as one unpublished local cohort. No shipped Desktop
state is imported and no legacy writer is switched. Persistence, schema migration, remote Workers,
SSH, installation, restart recovery and public release remain separate work. Rollback restores the
prior unused cohort; it cannot discard accepted execution effects or revive terminated processes.
This file remains with the Server as the maintained behavioral and evidence owner after delivery.
