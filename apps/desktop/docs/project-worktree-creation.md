# Project Worktree Creation

## Authority and scope

This reference owns Desktop creation of one managed Git Worktree for an existing Workbench Project.
Workbench owns Project, Directory, Workspace, and terminal-context semantics; Desktop Main owns Git,
the managed path, canonical-document commit, compensation, and crash recovery. The Electron renderer
owns the localized form and invokes the typed capability without receiving filesystem or process
authority.

Deletion, movement, import of external Worktrees, existing-branch checkout, automatic fetch, base
rewriting, user-selectable storage locations, and cross-device path synchronization are excluded.

## Domain contract

| Field          | Contract                                                                                                                                                         |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | A person creates an isolated Git Worktree from a Project and normally receives a Workspace using it without placing files beside the source repository.          |
| Project input  | One current Project ID and one current repository choice derived from its available Directories.                                                                 |
| Git input      | One existing local base ref and one new valid branch name.                                                                                                       |
| Product option | `createWorkspace`, enabled by default.                                                                                                                           |
| Success        | Git contains the new branch and Worktree, the canonical Project contains one `worktree` Directory, and the optional Workspace references it.                     |
| Cancellation   | Closing before submission changes neither Git nor the canonical document. After submission, Main owns the operation through commit or recovery.                  |
| Failure        | Typed discovery, validation, Git, document, compensation, and recovery failures expose localized diagnostics without raw stderr.                                 |
| No-op          | A stale Project, stale repository choice, duplicate branch, unavailable base, repeated request, or retired renderer never targets another Project or repository. |

## Identity and relationships

- A Project may contain Directories from zero, one, or several Git repositories.
- Repository identity derives from the canonical Git common directory and is independent of Project
  identity.
- Main allocates one opaque managed Worktree identity. Its path is
  `<data-root>/worktrees/<worktree-id>`.
- The committed Worktree Directory has `source: created`, required Git metadata, and one stable
  Directory identity derived from the managed Worktree identity.
- One optional new Workspace references that Directory. Existing and later Workspaces may also
  reference it; the Worktree and Workspace are never one domain object.

## Discovery and defaults

Main resolves only paths already present in the canonical Project. It observes each available
Directory through Git, deduplicates repositories by repository ID, and returns local refs already
present on disk. Discovery performs no fetch and no mutation.

The renderer prefers the active Workspace primary Directory when it belongs to the target Project,
then the Project primary Directory, then the first discovered repository. The base defaults to that
Directory's attached branch; detached state uses its exact `HEAD`. The new branch defaults to the
first unused `orchbee/workspace-N` and is the first focused field. A repository selector appears
only when discovery finds more than one repository, and its popup is product-rendered. The base
combobox filters discovered local refs but accepts arbitrary user text, including a commit ID,
verbatim. Base and branch remain editable; the lower-priority base and read-only managed location
follow the new branch. A valid prefilled form submits with Enter. This delivery has no application-level
or Project-level location setting.

## Creation transaction

| Phase          | Owned action                                                                                                              | Failure result                                                               |
| -------------- | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Resolve        | Re-read the canonical Project and selected repository                                                                     | No effect                                                                    |
| Validate       | Verify base commit, new branch absence, target absence, and recovery readiness                                            | No effect                                                                    |
| Prepare        | Persist one pending-operation record before Git mutation                                                                  | No Git or product effect                                                     |
| Git commit     | Run argument-vector `git worktree add -b <branch> -- <path> <base>`                                                       | Compensate any owned partial result                                          |
| Product commit | Build the next document through the Workbench-owned transition and serialize it through the canonical Desktop coordinator | Remove the created Worktree and branch                                       |
| Publish        | Publish the committed canonical document and reconcile terminal intent                                                    | Git and product state remain committed                                       |
| Finish         | Remove the pending-operation record and return success                                                                    | A retained marker is recoverable and never relabels committed work as failed |

The Workbench transition adds the Directory and optional Workspace atomically. It rejects a missing
Project, duplicate Directory path or identity, invalid Worktree descriptor, or unavailable source.
When `createWorkspace` is true it also allocates one terminal context and provision intent rooted at
the new Worktree path.

## Compensation and recovery

Before Git mutation, Main durably records operation ID, repository, branch, base, target, and phase.
If Git succeeds but the canonical product commit does not, Main removes only an exact registered
Worktree whose branch still points to the validated base and whose tracked, untracked, and ignored
files remain unchanged. It then deletes only that operation's unchanged branch. Main performs no
recursive filesystem fallback. A changed or ambiguously owned path or branch is retained for
recovery instead of risking user data.

Startup examines retained records after loading the canonical document. A record whose target is
already referenced by the canonical Project is complete and may be cleared. Otherwise Main retries
the same bounded compensation. A failed compensation remains recorded, blocks a conflicting new
operation, and exposes one localized recovery-required diagnostic without copying raw paths or Git
output into product text.

## Concurrency and lifecycle

- One renderer owns at most one discovery or creation request at a time.
- Main serializes creation by repository identity; different repositories may proceed independently.
- The canonical document coordinator serializes product mutation with renderer proposals.
- Renderer retirement suppresses late response delivery. Once Main accepts a creation command, Main
  owns it through product commit or compensation even if the requesting renderer retires.
- A response-delivery failure after canonical commit never rolls back Git or product state.
- Raw command output and absolute source paths remain in owned diagnostics and never enter product
  copy or telemetry.

## Verification

Deterministic Workbench tests cover atomic Directory-only and Directory-plus-Workspace transitions,
repository consistency, terminal-context materialization, and rejection invariants. Protocol,
client, broker, Git-adapter, coordinator, and recovery-ledger tests cover boundary validation,
argument safety, editable defaults, ambiguous branch ownership, changed and ignored files, canonical
commit failure, compensation failure, response suppression after renderer retirement, and restart
recovery. Temporary real Git repositories verify command refinement without network access.

The registered TLA+ model bounds the state space to two repositories and one operation state per
repository. It checks product-requires-Git, successful owner parity, durable recovery for partial Git,
response-after-commit, and eventual terminal or retained-recovery outcomes under fair Main-owned
effects. The model abstracts Git commands, filesystem atomicity, and form behavior; the deterministic
adapter and UI evidence cover those refinements.
