# Windows CLI Workspace creation

## Scope and ownership

`orchbee workspace create` creates one usable, ungrouped Workspace with a default-shell terminal. No Project or label form is required and no AI program starts automatically. A Project is optional organization for Workspaces, including Workspaces from different repositories. Ordinary and worktree Workspaces have the same navigation identity.

The Server owns canonical Workspace, Directory, context, Tab, Panel and presentation identities and terminal admission. The CLI prepares inputs and submits one revision-fenced `workspace.launch`; it never launches the shell itself. All Windows forms, including explicit `--project`/`--label`, use this workflow. Mac/Linux command execution retains its separately declared surface.

```powershell
orchbee workspace create
orchbee workspace create --cwd C:/code/demo --label main
orchbee workspace create --project demo --workspace source --cwd workspace --focus
```

Optional inputs include `--project`, `--label`, source `--workspace`, exact `--client`, mutually exclusive `--focus`/`--no-focus` (default), and `--timeout` (1–3600000 ms, default 30000). Existing environment, exec/args, launch-profile and all-or-none terminal geometry options remain available. Explicit exec/profile selects the requested program. Focus requires an existing ready Client before mutation and never launches a TUI.

## Source and destination

Source selection and destination organization are independent. Only an explicit `--project` selects an existing Project; omission always selects ungrouped placement, regardless of the source's group or the number of Projects. There is no first-Project preparation phase. Explicit Project targets must resolve uniquely before launch.

| Directory policy                                                   | Result                                                     |
| ------------------------------------------------------------------ | ---------------------------------------------------------- |
| Explicit path                                                      | Use the resolved literal Windows path                      |
| `home` / `current`                                                 | Use validated Windows home / CLI cwd                       |
| `workspace`                                                        | Require the source Workspace's global primary Directory    |
| Default / `follow` with a proven matching Panel                    | Sample only its primary terminal root-process cwd          |
| No matching Panel, or explicitly absent root cwd                   | Use source Workspace primary Directory, then Windows home  |
| Missing root process, malformed cwd, query failure or interruption | Refuse before launch; never guess another process or Panel |

Explicit `--cwd` paths accept drive-absolute Windows paths, paths relative to the executing CLI's current directory, and home operands `~`, `~/...` or `~\...`. Syntax admission is context-free and completes before machine routing; the sender and relay retain the original argv. Only the final business process resolves explicit paths, before loading configuration or accessing the runtime. Home selection uses the first nonempty source in order: `USERPROFILE`, `HOMEDRIVE` plus `HOMEPATH`, then `HOME`; an invalid selected value refuses without falling back. Other tokens such as `~name`, `$HOME` and `%USERPROFILE%` remain literal. Resolution does not probe or canonicalize the filesystem, expand shell variables, or create directories.

An explicit Client overrides implicit caller context; otherwise a validated caller precedes a unique current ready Client. Invalid/partial caller identity, stale Clients and ambiguity refuse. Explicit path, `home` and `current` policies ignore unrelated source identity. Headless no-focus creation is valid when no caller or Client is present. The omitted label starts with the directory basename, or a localized Workspace fallback for a root directory. If a canonical Workspace already has that exact label, preparation chooses the first free ` (2)`, ` (3)`, and so on across global navigation. The 512-character limit includes the suffix, truncating only the basename when needed. Explicit labels stay exact; concurrent changes remain protected by the existing revision fence, without retry. Only an omitted label launches an automatic Workspace whose name later follows its first terminal; see [automatic Workspace names](native-location-labels.md#automatic-workspace-names). [CLI configuration](windows-cli-configuration.md) owns persistent directory defaults. Parsing does no shell expansion, filesystem probing or directory creation.

## Commit, focus and recovery

One absolute deadline covers preparation, launch and optional focus. Metadata/control exchanges are bounded at two seconds; launch uses the remaining invocation budget and signal wake. Preparation finishes before any mutation. One `WorkspaceLaunch` with `Ungrouped` or explicit `Project` placement atomically admits metadata, Directory and the first terminal. There is no separate ProjectCreate or refresh stage.

Known canonical IDs are retained before follow-up work. A pending or started terminal may proceed to optional focus. Failed, unavailable or unknown terminal startup retains the Workspace receipt and stops focus. Unknown mutation handoff retains its original request identity and stops without retry, rollback or replacement creation.

Focus reads a fresh same-Session snapshot at or beyond the commit revision, resolves the returned Workspace globally, and checks exact context/Tab/Panel membership. Project movement does not invalidate that scope. The Client must retain its semantic selection, or already select the exact newly created Workspace/Tab/Panel through first-Workspace reconciliation. Changed context, Tab, Panel or zoom refuses focus. One Context intent uses the fresh view revision and existing rendered/seen acknowledgement. Cancellation stops waiting, not accepted Server work; later failure preserves earlier receipts.

## Machine output and organization commands

All Windows creation forms emit `orchbee.workspace.creation` v1. `workspace.result` contains resource IDs and terminal facts from mutation v1, with required-nullable `projectId`. The envelope records directory provenance, optional Client/focus facts, stage and structured failure. The removed outer `project` receipt and `project` stage are rejected. Completion requires each requested phase to succeed; terminal pending remains explicit. Parser/preparation failures use their existing failure envelopes, including `workspace-creation` stage failures and retained unknown request identity.

Existing Workspaces can be organized without PTY, cwd, Git or filesystem effects:

```powershell
orchbee workspace set-project <workspace> --project <project>
orchbee workspace create-project <workspace> --label <label>
orchbee workspace ungroup <workspace>
orchbee project dissolve <project>
```

Create-project atomically creates a Project and moves the selected Workspace. Dissolve removes only the grouping and retains all Workspaces. IDs, terminal bindings, primary Directories and global order remain stable. `workspace move` and `workspace move-block` separately reorder navigation.

`workspace close <workspace> --confirm` closes only that Workspace, preserving checkout and branch. Explicit `project close-all <project> --confirm` closes captured members while retaining the Project; see [Project closure](windows-worktrees.md#project-close-all).

## Evidence

Grammar and schema tests cover optional organization, exact global targets, required-nullable Project metadata, offline help/completion and parse-before-effect. Scripted Windows process tests check one launch with zero/one/multiple Projects, advanced options, source precedence, exact effects, unknown identity with no replay, terminal failures and guarded focus. Raw-route and real-process witnesses cover grammar before context, unchanged sender/relay operands, final-process home/cwd resolution and context refusal before configuration or runtime effects. Fixture owners release temporary pipes, profiles and processes. Actual terminal acceptance belongs to the matched native cohort tests; these scoped tests do not claim every live race.

Executable names use the final CLI PATH and retain the resolved absolute path in receipts; see [Executable names](native-terminal-creation.md#executable-names).
