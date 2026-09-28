# Windows CLI configuration

## File and validation

Windows CLI creation reads `APPDATA/Orchbee/config.toml`, or the absolute file named by `ORCHBEE_CONFIG_FILE`. A missing default file means unchanged defaults; a missing explicit file is an error. Invalid or empty location variables, unreadable files, directories, files larger than 64 KiB, invalid UTF-8/TOML, unknown keys and wrong types fail before creation. Diagnostics identify the field and reason without echoing file contents. No configuration directory or file is created by reading or checking it.

```toml
[terminal]
new_cwd = "follow"

[worktrees]
directory = "~/Projects/worktrees"

[session]
resume_agents_on_restore = true
```

Only these three sections and keys are accepted. `terminal.new_cwd` accepts `follow`, `home`, `current`, `workspace`, or a path. An empty value means `follow`. Paths use the Windows CLI path adapter: relative paths resolve against the configuration file's parent, tilde paths use Windows home, and ambiguous drive-relative, UNC and device paths are rejected. A missing `APPDATA` leaves defaults available; an explicit malformed value does not silently fall back.

## Creation policy

| Input                                                    | Result                                                                                                    |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Explicit CLI cwd or worktree path                        | Overrides the corresponding valid setting                                                                 |
| Omitted cwd in Workspace, Tab or Panel creation | Apply `terminal.new_cwd`, then the existing source-directory policy                                       |
| Omitted worktree path with configured root               | Query the source repository before any Project or Git mutation; select `root/repository-name/branch-slug` |
| No configured root                                       | Keep the existing sibling checkout path                                                                   |
| Resume                                                   | Use the path and branch already retained in the operation; do not reload settings                         |

Branch slugs lowercase ASCII letters, collapse nonalphanumeric runs to a hyphen, trim hyphens and use `worktree` if empty. Generated branches remain `worktree/<operation-id>` and are fixed in the submitted intent. The read-only repository query uses the same source and trust policy as creation. The Server still rechecks the source and owns every Git and canonical Workspace effect; the query is not a filesystem transaction. Explicit path avoids the extra query. Configuration is read once per invocation, including validation before an explicit override is applied.

The creation settings govern Windows CLI creation. The legacy explicit empty-Workspace command, TUI direct creation, existing terminals, Server configuration reload, custom shells and keybindings retain their own contracts.

## Check and correctness

Windows `config check` version 2 adds a sixth `user-config` check to the five runtime checks. Version 1 keeps its five-check schema. The added check reports the selected file, presence, normalized effective settings, or a bounded reason and field. Invalid creation configuration uses the separate `orchbee.cli.failure.configuration` version-1 envelope and exits 1. Failed source discovery stops configured-path creation before first-Project setup; no mutation is retried.

The shared Windows settings reader owns bounded file inspection and normalization; the creation workflows own precedence and existing effect receipts. There is no configuration writer or cached mutable state. Core evidence covers malformed and duplicate TOML, unknown keys, bounds, relative/tilde paths, override precedence and pinned Herdr slug vectors. Windows public IPC traces cover effective launch cwd, configured checkout intent, failed discovery with no mutation, explicit path without discovery and unchanged no-file behavior. Full application acceptance remains a separate gate. Reference behavior: Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `TerminalConfig`, `WorktreesConfig` and `default_checkout_path`.

## Restore policy

The shared Windows settings reader in `orchbee-local-site-runtime` owns bounded file inspection, strict TOML types and literal Windows path normalization. The CLI projects creation paths into Workbench types; no Workbench dependency is added to the runtime library. The existing pinned `toml` parser is a Windows dependency for the actual Client and Server consumers.

`[session] resume_agents_on_restore` is a TOML boolean, default `true`. `ORCHBEE_RESUME_AGENTS_ON_RESTORE` overrides it with exactly `true` or `false`; an override never hides an invalid file. This setting applies only when starting a new Site lifetime. Existing Site admission does not read it, and editing it does not change a running Site.

Client startup reads and validates once before durable Active publication or activation marker commitment. It passes the resolved boolean as a private Server argument; the Server validates that argument without rereading mutable settings. Direct managed Server startup without that argument reads the same file/environment owner before Worker launch. Invalid input leaves the preceding checkpoint and stopped state available for correction and retry. The private argument requires a matched cohort; rollback uses the preceding whole cohort.

Windows `config check` version 3 reports `resumeAgentsOnRestore` and `resumeAgentsOnRestoreSource` (`default`, `file`, `environment`). Schemas for versions 1 and 2 remain available. Diagnostics report only a bounded field/reason. Checks must cover default/file/environment precedence, malformed and non-UTF-8 values, strict types/keys, read bounds, before-commit refusal and corrected retry, unchanged existing-Site admission, and actual disabled/enabled restart. Live reload, names and managed-state restoration are separate obligations.
