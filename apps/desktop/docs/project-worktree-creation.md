# Project Worktree Creation

## Authority and scope

Desktop creates one managed Git Worktree from an existing Workspace. Workbench owns Workspace, optional Project membership, launch-resource namespaces and terminal structure; Main owns Git, managed paths, canonical commits and crash recovery. The renderer owns the localized form and a typed capability, never filesystem or process authority. Checkout deletion, movement, fetching and user-selected storage locations are excluded. Existing Worktree opening is covered by [project-worktree-adoption.md](project-worktree-adoption.md).

## Domain contract

| Field          | Contract                                                                                                                |
| -------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Input          | Exact source Workspace and source Directory IDs, observed repository ID, base ref, new branch and `createWorkspace`     |
| Namespace      | Source resources resolve through immutable `resourceScopeId`; raw Directory IDs are not global in GUI documents         |
| Success        | Directory and optional Workspace, first shell and current group membership are committed atomically                     |
| No Project     | Creating a Workspace also creates one group containing source and result; Directory-only creation leaves grouping alone |
| Regrouping     | Source current membership is read at canonical commit, not captured as execution authority                              |
| Source closure | Known Git result is retained; no source or destination Workspace is resurrected                                         |
| Failure        | Typed validation, Git, document, retained-result or recovery failure; no raw stderr in copy                             |
| Cancellation   | Before submission there is no effect; after acceptance Main owns settlement despite renderer retirement                 |

## Identity and relationships

A Project can contain several repositories and one repository can span Projects. Git identity derives from the canonical common directory, not grouping. Main allocates one opaque Worktree ID under `<data-root>/worktrees/<worktree-id>`. Its `source: created` Directory carries repository, branch and base metadata. Workspace identity remains distinct from physical checkout identity. Opening an associated Worktree focuses its existing Workspace rather than allocating a duplicate.

## Discovery and defaults

Discovery reads available local Directories in the source Workspace's resource scope, prefers its selected Directory then the scope fallback, deduplicates Git repositories and lists local refs without fetching. The renderer shows a repository selector only when needed, defaults base to branch or detached HEAD, and suggests an available `orchbee/workspace-N` branch. Branch is the first focused field; base remains editable, and managed location is read-only. Group labels never limit Git identity.

## Creation transaction

| Phase    | Owned action                                                                                       | Failure result                                   |
| -------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Resolve  | Capture exact Workspace, resource scope, context, source Directory and path                        | No effect                                        |
| Validate | Check repository, base, new branch, managed target and recovery readiness                          | No effect                                        |
| Prepare  | Persist pending-operation identity and Git authority                                               | No Git or product effect                         |
| Git      | Execute argument-vector `git worktree add -b <branch> -- <path> <base>`                            | Observe partial results before deciding recovery |
| Product  | Revalidate source identity and apply current grouping plus first shell through the canonical queue | Retain a known created Worktree                  |
| Publish  | Publish canonical structure; existing reconciler starts committed terminal intents                 | Product and Git facts remain committed           |
| Finish   | Clear the pending record and return exact Directory/Workspace IDs                                  | Record-cleanup failure never reverses success    |

Regrouping or dissolving the source group cannot invalidate source execution identity. Changing its scope, context, Directory/path, or closing it does. Publication does not select the new Workspace in other clients. The initiating client's latest activation intent waits for the exact result and publication; newer navigation cancels that intent.

## Compensation and recovery

The strict version `1` operation journal records explicit `sourceWorkspaceId`, `sourceResourceScopeId` and `sourceTerminalContextId` independently of grouping. Historical Project-only shapes are rejected without synthesizing missing identities. The journal filename stays stable; invalid or duplicate records are unavailable and their original bytes remain unchanged.

A `git-created` or `retained` result is never automatically deleted because canonical commit failed, the source moved/closed, or the renderer retired. Main persists `retained` and returns `worktree-retained`; the user can choose the existing Worktree later. Startup clears already committed targets independent of grouping and retains observed created targets. Retained records do not block other Worktree operations. Adoption clears the corresponding retained receipt after canonical commit.

Only an unconfirmed preparation with no observed checkout may use the adapter's exact ownership checks to compensate its own unchanged residue. It checks branch/base and tracked, untracked and ignored files, and never recursively deletes a path as fallback. Failed observation or uncertain ownership stays `recovery-required` and blocks conflicting work. The last actual recovery consumer owns journal cleanup.

## Concurrency and lifecycle

One renderer admits one Worktree request at a time. Main serializes by physical repository identity; the canonical coordinator serializes structural commits with other proposals. A response-delivery failure cannot undo Git or product state. Source group changes during async preparation use current membership, and closing the source returns a retained partial outcome without resurrection.

## Verification

Workbench tests cover atomic grouping, resource-scoped identity, first-shell allocation and stable selection. Desktop protocol, broker, client, runtime and ledger tests cover typed boundaries, current-group races, source closure, canonical-commit failure, retained restart records and reopening the same Worktree across groups. Existing Git-adapter tests exercise argument safety and precise residue ownership with temporary repositories. No continuous filesystem observer or native-window focus claim is made.
