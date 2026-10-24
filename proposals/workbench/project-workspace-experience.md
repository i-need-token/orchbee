# Project and Workspace Platform Effects

| Field             | Value                                                                                                                                                                                          |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Retained as future GUI planning input under native runtime topology. Current Desktop capabilities remain running; future deletion, launch and cross-device effects require separate admission. |
| Owner             | Workbench product maintainers and platform application owners                                                                                                                                  |
| Current authority | [`packages/workbench/docs/project-workspace.md`](../../packages/workbench/docs/project-workspace.md)                                                                                           |
| Exit condition    | Implement separately accepted platform slices, move maintained facts into their owning application and package guides, then delete this proposal                                               |

Simplified Chinese review translation:
[project-workspace-experience.zh-CN.md](project-workspace-experience.zh-CN.md). English remains
canonical.

## Current foundation

`@orchbee/workbench` already owns the in-memory Project, Directory, Workspace, active-location, Tab,
Panel, and terminal-presentation composition. The browser preview supports synthetic Folder paths
without filesystem or Git effects. Those implemented facts are maintained by the current authority
and are not repeated here.

The next phase follows [native runtime topology](../platform/native-runtime-topology.md): Server owns canonical Workbench state and authorization; Worker owns filesystem, Git and terminal effects; Desktop retains native pickers and Client platform adapters; Web owns interaction and presentation. The TypeScript model is the current implementation, not a requirement to retain canonical state ownership in the future.

## Native Folder acquisition

Desktop now supplies a typed platform capability for selecting one existing Folder. Electron Main
owns the native picker and point-in-time directory validation, then returns a host-normalized path to
the same Project-create or Directory-add intent used by the browser preview. The maintained policy
and evidence live in
[`apps/desktop/docs/project-folder-acquisition.md`](../../apps/desktop/docs/project-folder-acquisition.md).

- Initial Project creation acquires exactly one Folder.
- Cancellation, denied permission, invalid paths, and failed selection leave Workbench state
  unchanged.
- Point-in-time filesystem validation completes before an authoritative Directory is submitted.
- The Folder name is the default Project name; the user may change product metadata independently.
- Project management reuses the same Folder-acquisition capability for later additions.
- Workbench receives no generic filesystem primitive.
- One renderer owns at most one request, and renderer retirement revokes late completion.
- Add Folder rechecks the current Project and exact duplicate paths before committing.

The owning Desktop documentation defines current Folder admission and creation. The next phase preserves path validation, cancellation, failure and recovery semantics while reconnecting them to Server/Worker ownership. Ongoing external filesystem observation and removable-location availability still require explicit contracts and evidence.

## Git Worktree effects

Git observation and mutation belong to a platform or Git capability owner. Workbench may consume
validated Directory descriptors whose semantic kind is `worktree`.

The accepted Desktop creation slice is maintained by
[`apps/desktop/docs/project-worktree-creation.md`](../../apps/desktop/docs/project-worktree-creation.md),
with its environment-isolated default path maintained by
[`apps/desktop/docs/data-root.md`](../../apps/desktop/docs/data-root.md).
Read-only discovery and adoption of existing Worktrees are maintained by
[`apps/desktop/docs/project-worktree-adoption.md`](../../apps/desktop/docs/project-worktree-adoption.md).

The accepted product direction is:

- creating a Worktree is a Project-level operation and may optionally create a Workspace using that
  Directory;
- existing Worktrees are grouped by represented repository in New Workspace and are adopted only
  after point-in-time Main revalidation, without Git mutation;
- the create-Workspace option defaults to enabled;
- creation uses Desktop's environment-isolated fixed managed root; application-level and
  Project-level location overrides require a later accepted settings slice;
- deleting a Workspace retains its Worktree by default;
- deleting the Worktree requires an explicit user choice and must be rejected while another
  Workspace, terminal CWD, or final Project Directory still depends on it;
- Git deletion succeeds before the Workspace and Directory are removed from product state; failure
  preserves them and exposes an owned diagnostic.

Deletion still requires a separately accepted slice defining dependency checks, confirmation,
terminal-CWD handling, Git ordering, and recovery.

## Launch configuration and remaining durability

Workbench currently models launch-configuration identity and selection but does not define an
executable payload. A later owner must define command, arguments, environment, secret references,
platform applicability, validation, execution failure, and duplication semantics before real launch
configuration execution.

Desktop already persists the canonical shared Workbench document and durable terminal intent. Any
expansion beyond that local authority must continue to respect the shared-document and client-view
ownership split. Client View recovery, missing Directory reconciliation, external filesystem
changes, cross-device synchronization, and conflict handling require their own accepted behavior and
recovery evidence.

## Exclusions

Except for the accepted existing-Folder picker, managed-Worktree creation, and existing-Worktree
adoption contracts, this proposal does not authorize other filesystem mutation or Git
implementation, generic renderer access to native capabilities, remote Folder connections,
automatic deletion of source data, PTY ownership in Workbench, or cross-device synchronization.
Each additional effectful platform slice still requires its own bounded authorization and
verification.
