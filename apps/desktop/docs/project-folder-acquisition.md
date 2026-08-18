# Project Folder Acquisition

## Authority and scope

This reference owns Desktop acquisition of one existing local Folder for Workbench Project creation
or Folder addition. Workbench owns the user intent and Project mutation, `apps/web` owns the
Electron renderer adapter, `@orchbee/desktop-client-protocol` owns the records on the dedicated
renderer capability port, and Desktop Main owns the native dialog and filesystem observation.

Browser and Gateway clients retain their explicit path-entry adaptation and never receive this
capability. Creating a Folder, Git discovery or mutation, persistent availability monitoring,
remote Folder access, and generic renderer filesystem access are excluded.

## Domain contract

| Field           | Contract                                                                                                                                                                                    |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem         | A person selecting `Select Folder…` in the Electron Workbench must choose an existing local Folder through the operating-system dialog instead of entering an unverified path.              |
| Intent          | Acquire one Folder for either a new Project or one currently existing Project.                                                                                                              |
| Untrusted input | Renderer commands, request IDs, native dialog results, returned paths, asynchronous completion order, and messages received after either endpoint retires.                                  |
| Success         | Main returns one validated, absolute, host-normalized path; Workbench invokes the existing Project-create or Folder-add Action for the still-current target.                                |
| Cancellation    | Native cancellation returns `cancelled`, closes the pending interaction, restores focus through the native dialog lifecycle, and invokes no Workbench Action.                               |
| Failure         | Busy, permission-denied, missing, non-directory, dialog, validation, or control failure invokes no Workbench Action and projects one localized dismissible failure.                         |
| No-op           | A repeated UI request while one request is pending, a stale or unknown completion, completion after disposal, and a selected Folder already present in the target Project commit no change. |
| Side effects    | Main opens one window-modal native dialog and performs read-only path observation. Selection never creates, changes, or removes filesystem content.                                         |

## Canonical data and ownership

| Mutable fact                             | Sole owner                                         |
| ---------------------------------------- | -------------------------------------------------- |
| Current acquisition target and phase     | Mounted Workbench Folder-acquisition workflow      |
| Renderer request identity and completion | Electron Folder-acquisition client                 |
| One active native operation              | Desktop Folder-acquisition broker                  |
| Native dialog and path observation       | Desktop Folder-acquisition runtime                 |
| Project and Folder membership            | `WorkbenchProjectWorkspaceRuntime` through Actions |
| Renderer-document capability lifetime    | `DesktopApplication`                               |

The Workbench workflow has `idle`, `selecting`, `failed`, and terminal `disposed` phases. A
`selecting` phase contains one target: `create-project` or `add-project-folder(ProjectId)`. The
Electron renderer client separately owns the correlated request identity. The protocol result is
exactly one of `selected(Path)`, `cancelled`, or `failed(FailureCode)`.

## Path policy

| Decision              | Rule                                                                                                                                                            |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Selection cardinality | Request one existing Folder; multiple selection and file selection are rejected.                                                                                |
| Canonical path        | Require an absolute native path and apply the host `resolve` normalization before validation and publication.                                                   |
| Symbolic links        | Preserve the normalized path spelling selected by the person; directory and access checks follow the link target.                                               |
| Validation            | The selected path must still exist, resolve to a directory, and permit read and traversal at observation time.                                                  |
| Removable locations   | A mounted location may be selected; later removal is handled by existing terminal-launch and future Directory-reconciliation behavior, not by this acquisition. |
| Privacy               | The selected path may enter the canonical Workbench document but never enters capability diagnostics or failure text.                                           |
| Compensation          | Selection and validation are read-only, so a later rejected Workbench commit has no external effect to compensate.                                              |

## Transitions

| Event                         | Guard                                   | Result                                                                                              |
| ----------------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Request selection             | Workflow and broker are idle            | Record one target and request identity; open one native dialog.                                     |
| Repeat request                | A selection is pending                  | No-op at Workbench; a direct second protocol request receives `busy`.                               |
| Native selection validates    | Matching request and live capability    | Publish the normalized path; invoke the target Action only if its current semantic guard holds.     |
| Native selection is cancelled | Matching request and live capability    | Return to idle without an Action.                                                                   |
| Native selection fails        | Matching request and live capability    | Enter failed presentation without an Action.                                                        |
| Failure dismissed             | Workflow is failed                      | Return to idle.                                                                                     |
| Renderer document retires     | Any non-disposed phase                  | Dispose both port endpoints and the workflow; ignore any later native completion.                   |
| Unknown or stale completion   | Identity does not match current request | Retire the invalid client transport or ignore the already-retired endpoint; never invoke an Action. |

The operating-system dialog has no timeout: user choice owns its duration. Progress is claimed only
after the native operation resolves while both endpoints remain live. Port disposal cannot close an
already-open operating-system dialog, but it revokes delivery and Workbench mutation authority.

## Invariants

- Gateway and ordinary Browser clients cannot obtain the native Folder-acquisition port.
- One renderer document and one broker own at most one pending Folder request.
- A Workbench Action is invoked only for the matching live request's validated `selected` result.
- Cancellation, failure, duplicate target membership, disposal, and stale completion invoke no
  Project mutation.
- Main validates before publishing a selected path; Workbench never receives a generic filesystem
  primitive.
- No request is replayed after an ambiguous client or broker transport failure.

## Correctness and verification

| Claim                                        | Evidence                                                                                                                         | Limits                                                                                                 |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| One live request and selected-only mutation  | `DesktopLifecycle` `project-folder-acquisition` configuration, deterministic Workbench transition tests, and client/broker tests | TLC uses finite request and target sets; tests establish implementation refinement.                    |
| Cancellation, failure, and retirement safety | TLA+ cancellation/disposal actions, client/broker late-completion tests, and renderer unmount tests                              | Native operating-system dialog cancellation is represented by an injected deterministic result.        |
| Existing readable directory validation       | Runtime tests for absolute normalization, directory, symlink, permission, disappearance, and unexpected failures                 | Validation is an observation-time claim, not continuous availability.                                  |
| Electron-only capability delivery            | Bootstrap schema/preload/Main composition tests and Desktop-versus-Browser application tests                                     | A trusted Electron renderer holding the port may request a dialog without a second user-gesture proof. |
| Observable native integration                | Desktop system trace with an injected native selector plus manual current-platform inspection                                    | Automated tests do not claim control of the real macOS, Windows, or Linux system picker UI.            |

The `project-folder-acquisition` configuration checks at most two request identities and both
Workbench target kinds. It checks one-active-request, current-selected-only mutation, cancellation
and failure no-op, retirement revocation, and eventual settlement only after an explicitly modeled
native result.
