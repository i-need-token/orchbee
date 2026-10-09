# Project Folder Acquisition

## Authority and scope

This reference owns Desktop selection of one existing local Folder, or explicit creation of one new
child Folder, for Workbench Project creation or existing-Folder addition. Workbench owns the user intent and Project mutation, `apps/web` owns the
Electron renderer adapter, `@orchbee/desktop-client-protocol` owns the records on the dedicated
renderer capability port, and Desktop Main owns the native dialog, filesystem observation and bounded child creation.

Browser and Gateway clients retain their explicit path-entry adaptation and never receive this
capability. Git discovery or mutation, persistent availability monitoring,
remote Folder access, and generic renderer filesystem access are excluded.

## Existing-Folder selection contract

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

## Selection data and ownership

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

## Selection path policy

| Decision              | Rule                                                                                                                                                            |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Selection cardinality | Request one existing Folder; multiple selection and file selection are rejected.                                                                                |
| Canonical path        | Require an absolute native path and apply the host `resolve` normalization before validation and publication.                                                   |
| Symbolic links        | Preserve the normalized path spelling selected by the person; directory and access checks follow the link target.                                               |
| Validation            | The selected path must still exist, resolve to a directory, and permit read and traversal at observation time.                                                  |
| Removable locations   | A mounted location may be selected; later removal is handled by existing terminal-launch and future Directory-reconciliation behavior, not by this acquisition. |
| Privacy               | The selected path may enter the canonical Workbench document but never enters capability diagnostics or failure text.                                           |
| Compensation          | Selection and validation are read-only, so a later rejected Workbench commit has no external effect to compensate.                                              |

## Selection transitions

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

## Selection invariants

- Gateway and ordinary Browser clients cannot obtain the native Folder-acquisition port.
- One renderer document and one broker own at most one pending Folder request.
- A Workbench Action is invoked only for the matching live request's validated `selected` result.
- Cancellation, failure, duplicate target membership, disposal, and stale completion invoke no
  Project mutation.
- Main validates before publishing a selected path; Workbench never receives a generic filesystem
  primitive.
- No request is replayed after an ambiguous client or broker transport failure.

## New Folder transaction

The `New Folder…` entry first selects a local parent through the same native single-directory selector. Main
retains that normalized path behind a renderer-lifetime selection token. Selection and cancellation
are read-only. The Workbench Dialog shows the parent and collects exactly one child name; only
explicit **Create** authorizes the filesystem effect. Browser and Gateway never receive this
optional creation capability. Existing selection and Add Folder keep their behavior above.

| State or boundary       | Contract                                                                                                                                                                                                                                                                                 |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Draft                   | Parent selection token plus name; Cancel/Escape discards the draft without a filesystem or Project mutation.                                                                                                                                                                             |
| Creating                | One exact request and monotonically increasing attempt; disable edits, dismissal and repeated submission. Retirement cancels preparation before mkdir; an already issued mkdir must settle.                                                                                              |
| Filesystem commit       | Successful non-recursive `mkdir` creates one absent directory. Main consumes the parent token and retains the exact request/path receipt until acknowledgement or Main-lifetime fallback reporting.                                                                                      |
| Known no effect         | Invalid name, parent validation failure, or rejected atomic mkdir leaves the parent selected. A corrected name can be explicitly submitted with a new attempt; an old attempt is never replayed.                                                                                         |
| Created but unavailable | Validation after successful mkdir failed. Preserve and display the created path; do not create a Project, delete the Folder or retry mkdir.                                                                                                                                              |
| Opening                 | Retain the actual Project-create `ActionExecution`, await its result, then Desktop's existing document-coordinator flush and exact receipt validation.                                                                                                                                   |
| Opened                  | The exact Project/Workspace/Directory/path/context/first-presentation receipt is still canonical and current; the existing Workspace activation Action accepts that exact destination. First-shell provisioning or failure remains terminal-lifecycle-owned.                             |
| Created but not opened  | Action, publication, exact-target or activation failure retains the Folder and any already committed Project facts. Display the path and offer the existing Select Folder recovery.                                                                                                      |
| Outcome unknown         | Transport retirement after submission cannot prove whether mkdir committed. Display parent/name, preserve content, and allow only Select Folder recovery; never replay creation.                                                                                                         |
| Retirement              | Cancel uncommitted queued Actions and fence every continuation. Main retains any unacknowledged created-path receipt and reports it through a localized native message while Main survives. There is no process-exit guarantee, persistent recovery, automatic rollback or quit barrier. |

Names contain 1–255 UTF-16 code units and at most 255 UTF-8 bytes. Reject whitespace-only names,
`.`/`..`, separators, absolute/drive/UNC forms, control characters, `<>:"|?*`, trailing dots or Unicode whitespace,
Win32 device aliases (including aliases with extensions and superscript COM/LPT digits), unpaired Unicode surrogates, and ADS.
Do not silently trim or rewrite names. Main revalidates the name and selected parent before
`mkdir(path, { recursive: false })`; the normalized child's dirname must equal the selected parent.
Keep the selected symlink spelling and follow the existing validation policy without `realpath`
relocation. Atomic no-overwrite creation reports `EEXIST` as a visible conflict, including files and
symlinks. Filesystem validation is observation-time evidence, not protection against another process
replacing an ancestor between observation and mkdir.

While route preparation is pending, keep the opening UI pending. The existing activation Action waits for exact route acceptance; retirement, replacement, rejection or its bounded deadline produce recovery. After acceptance the workflow checks canonical and current receipt again before ACK.

Only one unacknowledged created receipt may exist per broker. The same broker remains retained in Main memory across channel retirement until exact ACK, completed native reporting, or Main exit. A failed native report keeps that in-memory receipt; no delivery guarantee extends past Main exit. Later requests cannot overwrite it;
an acknowledgement must identify its exact creation request. Acknowledge only after accepted exact
publication and activation or after presenting the recovery outcome. Recovery displays the exact created path to its initiating user; paths never enter diagnostics. Releasing the owning draft or workflow asks Main to report any unacknowledged receipt. The receipt contains no
provisioning authority and never causes automatic deletion, Project replay or navigation.

The Project-create Action returns its captured transition receipt, not a later active Workspace.
Its existing `project-created` transition constructs Primary Workspace, one terminal context, Tab,
Panel and managed provision intent at the exact Folder CWD. Desktop's existing canonical proposal
and terminal-intent reconciliation start the shell. A successful flush alone is insufficient: a
stale proposal can settle by replacing the local projection, so both canonical and current snapshots
must still contain the full captured receipt, including its unique first presentation.

The `DesktopLifecycle` `project-folder-creation` scenario and owner tests model explicit submission, separate filesystem
and Project commits, exact acknowledgement, known-no-effect retry, retirement and retained effects.
The implementation's deterministic cases cover the name boundary, no-overwrite, post-mkdir failure,
protocol correlation and replay, queued Action retirement, stale publication and both locales. These
are maintained verification artifacts; source review does not assert that tests, TLC or native
integration have executed.

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
