# Project Worktree Adoption

## Authority and scope

Desktop discovers and opens existing Git Worktrees from a current Workspace. Workbench owns organization and terminal structure; Main owns Git observation, path validation and canonical commit. Commands carry source Workspace, source Directory, repository and opaque Worktree identities, never renderer-selected paths. Discovery and adoption do not change Git. Creation is covered by [project-worktree-creation.md](project-worktree-creation.md).

## Discovery read model

Discovery reads available local Directories in the source Workspace's immutable resource namespace. It groups choices by physical Git repository, independently of Project membership. One Project may represent several repositories; another Project may represent the same repository. Preferred Directory ordering affects labels only.

Main reads `git worktree list --porcelain -z`, canonicalizes existing paths, excludes bare/prunable records and the current source, and deduplicates physical repository identities. Already represented local Worktree choices are rendered from current resources, while external choices carry opaque stable identity and branch or detached-path labels. Unavailable inventory is explicit, never an empty success. Reopening a Workspace menu refreshes this point-in-time inventory without fetch, pruning or filesystem mutation.

## Adoption transition and workflow

| Phase      | Owned action                                                                           | Result                                                            |
| ---------- | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Resolve    | Capture exact source Workspace, scope, context and Directory/path                      | No effect                                                         |
| Revalidate | Re-list and match the opaque Worktree identity and repository                          | No effect on stale or unavailable selection                       |
| Decide     | Search all groups and resource namespaces for an associated physical Worktree          | Return its existing Workspace if found                            |
| Commit     | Revalidate source identity in the canonical queue and apply `project-worktree-adopted` | Directory, Workspace, first shell and current membership together |
| Publish    | Publish and return exact identities; clear a matching retained creation receipt        | No forced selection in another client                             |

A moved source uses its current Project. An ungrouped source and the result gain a group atomically; a closed source is never recreated. If only an equivalent Directory exists, its identity is reused, upgrading a Folder descriptor when necessary. An already-associated Worktree returns its existing Workspace across group boundaries and allocates no second terminal context. Repository identity never depends on group labels or membership.

## Invariants and lifecycle

Adoption shares physical-repository serialization with creation and canonical commit ordering with all proposals. Detached Worktrees retain `branch: null` and exact `HEAD`. Canonical commit failure leaves external Git untouched. Closing an adopted Workspace retains its resources and checkout. Renderer retirement suppresses late delivery; the initiating client's activation intent waits for publication and is retired by newer navigation. No compensation ledger is needed for the adoption effect itself.

## Verification

Pure Workbench and Desktop runtime tests cover scoped identities, multiple repositories, current group changes, equivalent Directory reuse, stale source rejection and reopening an associated Worktree after it moves. Protocol/client/broker and Git-adapter tests cover opaque boundaries, parsing, canonical commit failure and retirement. Hook and component tests cover discovery menus and result/publication ordering without late navigation takeover. Continuous external Git observation and native-window focus remain outside this evidence.
