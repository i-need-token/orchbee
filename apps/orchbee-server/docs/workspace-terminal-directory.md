# Workspace terminal directories

Status: implemented local protocol and runtime slice; native Client UX and public delivery remain outside scope.
The product decision owner accepted this slice in the active development conversation. This record
owns the Server's Directory association and Workspace terminal-creation behavior. The
[Workbench authority](workbench-authority.md) owns its ledger and canonical commit machinery.

## Outcome and scope

An admitted local Client associates an empty unconfigured Workspace with a directory on the selected Worker. The Worker validates the path before Server accepts it. Later Tab/Panel creation uses that exact accepted path and Worker; missing configuration or unavailable Worker never falls back to home. A Workspace can be ungrouped and still owns its Context and directory independently.

Navigation owns a global, identity-sorted pool of `Directory { directoryId, workerId, path }`
records. Workspace holds a required nullable `primaryDirectoryId` referencing that pool, and a
separate required nullable `projectId` for optional grouping. Project contains only an identity and
label; it does not own Directories or terminal resources. Workspaces in different Projects or no
Project may reference the same Directory. Regrouping and dissolving a Project preserve Directory
identities, references, Worker placement and cwd. Unused Directory records remain valid.

Newly created Workspaces are unconfigured and Empty. Directory identities use the existing nonzero
128-bit identity form and Session allocation cursor. The global pool admits at most 65,536 Directories,
subject to the 1 MiB snapshot bound; the historical 256-Directory per-Project limit no longer governs
current admission. Paths follow the authorized local-platform cwd contract: absolute UTF-8,
1 through 4,096 bytes, no NUL, including admitted Windows drive-absolute paths.
Preserve path spelling and follow symbolic links when
validating; no realpath-based identity, enumeration, Git discovery or directory creation is implied.

The explicit `workspace.primary-directory.accept` action carries Workspace ID and path
under the existing command request ID, Session and expected revision. The Server selects the current
local Worker; the Client supplies no Worker generation, fence or lease. This first operation creates
one global Directory and selects it for that Workspace atomically, including when the Workspace
is ungrouped. It is set-once and requires an empty context. This command does not select or deduplicate
existing Directory records. Rebinding, deletion and general Directory CRUD remain outside this
operation. An independent directory-association grant is required in addition to
navigation authority. Read-only snapshot access does not confer association or launch authority.

## Admission and transitions

| Phase                  | Input and guard                                                                                                                          | Result and owner effect                                                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Admission              | Grant, live authority, exact replay/conflict, ledger capacity, Session/revision, exact Workspace, empty context and no primary Directory | Definite refusal preserves canonical state; exact replay returns its retained result without validation                        |
| Prepared validation    | A compatible current Worker and bounded request slot exist                                                                               | Capture exact Worker identity/generation and path; release registry and Session locks before I/O                               |
| Worker observation     | Exact request/generation; directory metadata lookup on that Worker                                                                       | Correlated valid, invalid-directory, unavailable or wrong-worker result; no Host, shell or canonical mutation                  |
| Revalidation           | Validation succeeds before the carrier deadline                                                                                          | Reacquire registry then Session; repeat admission and require the captured Worker generation still current                     |
| Commit preparation     | Identity, Directory count, revision, snapshot encoding and ledger bytes fit                                                              | Prepare complete projection and fixed result before any canonical mutation                                                     |
| Commit                 | All prepared checks succeed                                                                                                              | Append Directory, select it, advance one revision and retain one immutable acceptance result atomically                        |
| Result loss            | Commit may already have happened                                                                                                         | Client does not replay effects blindly; an exact command replay returns the original retained acceptance                       |
| Workspace Tab creation | Exact context resolves to its Workspace and configured global Directory                                                                  | Derive explicit cwd and exact Worker constraint; jointly commit membership and terminal intent using existing Create machinery |

Different commands racing to configure one Workspace cannot both commit. Concurrent exact requests
may perform duplicate read-only validations before either commits, but only one logical association
commits. Negative/unavailable validation and stale Worker generation consume no canonical identity,
revision or retained ledger entry. A revision conflict after validation does not silently rebase.
Association success is `workspace-primary-directory-accepted` with revision, Directory ID and Worker
ID. Missing configuration is `workspace-directory-unavailable`; an already configured Workspace or
nonempty context is `workspace-directory-conflict`. Invalid/unavailable directory observation uses
`directory-unavailable`; carrier/Worker unavailability retains existing typed unavailable results.

Created members are exposed through authorized snapshots and `attach-binding`. Pending/final command results, immutable completion tokens and unknown-outcome rules remain authoritative. A path removed after association produces a truthful launch failure and retains committed membership; no second home launch follows. Association validates one point in time, not future accessibility. Deadlines do not make arbitrary filesystem syscalls interruptible; process claims remain bounded by the supported host/filesystem cohort.

## Correctness and evidence

Server alone writes Directory identities, selection, revision and retained command results. Worker owns point-in-time path validation and launch admission. Tests must cover two Clients, concurrent validation, revision changes, Worker replacement, result loss/replay, capacity, exact cwd/Worker, joint commit and at-most-once execution. Unrelated Workspaces and terminals must survive unchanged. This workflow has no separate retained TLA model.

Strict schema and independent Rust/TypeScript bindings must reject dangling Directory or Project
references, duplicate identities, missing required nullable fields, invalid paths and bounds.
Directory references across Project groups are valid. Retained checkpoints admit only the current
normalized navigation; historical per-Project shapes are unreadable, as defined by the
[checkpoint contract](workbench-checkpoint.md#current-normalized-navigation).
Deterministic and stateful tests must map each admission/revalidation branch to its state-preserving
or committing result. Process tests must exercise a real directory, two authorized observers,
Workspace Create and AttachBinding, preserved unrelated Workspace membership, missing/deleted directories and
Worker loss without a home launch. Pure core tests alone do not establish filesystem or carrier behavior.

| Claim                                                                         | Maintained evidence                                                                                                                                                            | Limit                                                   |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------- |
| Admission, revalidation and replay preserve exact Directory ownership         | [Authority directory tests](../src/workbench_authority/tests/workspace_directory.rs) and [service directory tests](../src/local_terminal_service/workspace_directory/tests.rs) | Deterministic in-memory/transport fixtures              |
| Tab creation uses the Workspace Worker/cwd and preserves unrelated membership | [Session creation tests](../src/site_coordinator/tests/tab_create.rs)                                                                                                          | Exact prepared and retained operation facts             |
| Two Clients observe the same real Workspace shell and failure outcomes        | [Workspace directory process witness](../tests/client_process/workspace_directory.rs)                                                                                          | Supported Linux cohort; other platforms remain separate |

The Unix socket and Linux shell witnesses require their native host and built process prerequisites.
A Windows host gate or cross-target compilation does not execute those witnesses.

The lazy-loop regression distinguishes `server_transport::send`'s Host-closure flag from a Server
write error: with no Host yet, a successful validation response must keep the Worker available for
the next validation or Launch. Its two consecutive probes exercise that boundary directly.

## Delivery and compatibility

Protocol, Server and Worker activate together as one unpublished local cohort. No shipped Desktop
state is imported and no legacy writer is switched. Directory configuration retention and strict
version 1–5 migration to normalized version 6 follow the [checkpoint contract](workbench-checkpoint.md).
Association success alone does not prove restart or fresh terminal launch. Remote Workers,
SSH, installation and public release remain separate work. Rollback restores the
prior unused cohort; it cannot discard accepted execution effects or revive terminated processes.
This file remains with the Server as the maintained behavioral and evidence owner after delivery.
