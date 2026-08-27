# Project Worktree Adoption

## Authority and scope

This reference owns Desktop discovery and adoption of existing Git Worktrees for a current
Workbench Project. Workbench owns Project, Directory, Workspace, and terminal-context semantics;
Desktop Main owns Git observation, point-in-time validation, and the canonical-document commit. The
Electron renderer receives bounded labels and opaque identities, never repository or Worktree
paths.

Discovery and adoption perform no Git mutation. Creating, deleting, moving, pruning, repairing, or
fetching Worktrees, continuously watching Git state, and removing an adopted Worktree Directory are
excluded. Managed Worktree creation remains owned by
[`project-worktree-creation.md`](project-worktree-creation.md).

## Domain contract

| Field        | Contract                                                                                                                                                       |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A person opening New Workspace can see existing Worktrees from every Git repository represented by the Project and create a Workspace directly from one.       |
| Discovery    | One current Project ID and optional preferred Directory ID produce repository groups in stable Project order and point-in-time Worktree choices.               |
| Grouping     | Each repository appears under the Project Directory that first represents it after preferred ordering; several repositories remain separate groups.            |
| Exclusions   | A repository's current source Worktree and every path already represented by a Project Directory are omitted from its discovered choices.                      |
| Adoption     | One Project, source Directory, repository, and opaque Worktree identity select exactly one point-in-time revalidated Worktree.                                 |
| Success      | The canonical Project contains one `source: discovered` Worktree Directory and one active Workspace referencing it, or reuses an equivalent current Directory. |
| Failure      | A stale Project, source, repository, Worktree, unavailable path, invalid descriptor, or document conflict changes neither Git nor the canonical document.      |
| Cancellation | Dismissing the menu invokes no adoption; renderer retirement suppresses response delivery without cancelling a Main-owned canonical commit already started.    |

## Discovery read model

`DesktopProjectWorktreeDiscovery.repositories` remains the creation repository catalog and adds one
Worktree inventory per repository. An available inventory contains ordered entries with only an
opaque stable identity and display label. An unavailable inventory is explicit and never becomes an
empty-success claim. The label is the local branch name, or the final path segment for a detached
Worktree. Absolute paths, Git common directories, and raw command output remain Main-owned.

Main resolves only current available Project Directories. It observes repository identity, reads
`git worktree list --porcelain -z`, canonicalizes existing Worktree paths, rejects bare and prunable
records, and deduplicates repositories by canonical Git common directory. Discovery performs no
fetch, prune, or filesystem mutation. Reopening a Project menu requests a fresh point-in-time
inventory; the product does not claim continuous synchronization.

## Adoption transition and workflow

Workbench owns a serializable `project-worktree-adopted` transition. It requires one available
source Directory, one valid available `worktree` Directory with `source: discovered`, repository
consistency, and unique Directory identity and path. It atomically appends the Directory and one
Workspace, expands the Project, activates the Workspace, and emits one terminal-context creation
command rooted at the Worktree.

Desktop Main executes adoption in this order:

| Phase      | Owned action                                                                                                                           | Failure result                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| Resolve    | Re-read the canonical Project, source Directory, and repository identity                                                               | No effect                            |
| Revalidate | Re-list Worktrees and match the opaque identity to one available canonical path                                                        | No effect                            |
| Decide     | Reuse a current equivalent Directory, or build one validated `source: discovered` descriptor                                           | No effect                            |
| Commit     | Apply `workspace-created` for the equivalent Directory or `project-worktree-adopted` for the new Directory through the canonical owner | One atomic canonical document commit |
| Publish    | Publish the committed document and return Directory and Workspace identities                                                           | The committed outcome remains valid  |

The canonical coordinator serializes the commit. Adoption shares repository serialization with
managed creation so a concurrent creation cannot be mistaken for the selected discovery entry. No
compensation ledger is required because adoption mutates no external Git or filesystem state.

## Invariants and lifecycle

- Renderer adoption commands carry only identities returned by discovery; they cannot submit a
  source or target path.
- An opaque Worktree identity derives from repository identity and canonical Worktree path and is
  stable while that registration is stable.
- A Project never gains duplicate Directory identities or paths. If the path became represented
  after discovery, adoption creates the Workspace against that equivalent current Directory.
- A detached Worktree has `branch: null`; its exact `HEAD` remains authoritative metadata.
- One renderer owns at most one Worktree discovery, creation, or adoption request at a time.
- Deleting the adopted Workspace retains its Worktree Directory and never changes the external Git
  Worktree, matching ordinary Workspace deletion.
- Rolling back this capability requires no durable-data migration: adopted Directory and Workspace
  records already satisfy the Workbench shared-document schema and remain usable.

## Verification

Deterministic Workbench tests cover the atomic adoption transition, detached metadata, repository
consistency, duplicate rejection, terminal-context materialization, and equivalent-Directory reuse.
Protocol, client, broker, runtime, and real temporary-Git-repository tests cover opaque boundaries,
multi-repository grouping, porcelain parsing, current and represented-path exclusion, stale
selection rejection, canonical commit failure, and renderer retirement. Component and real Desktop
interaction checks cover menu refresh, grouping, direct Workspace creation, and right-side submenu
placement. The evidence excludes continuous filesystem observation and external Git mutation.
