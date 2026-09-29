# Windows CLI configuration

## File and validation

Windows CLI creation reads `APPDATA/Orchbee/config.toml`, or the absolute file named by `ORCHBEE_CONFIG_FILE`. A missing default file means unchanged defaults; a missing explicit file is an error. Invalid or empty location variables, unreadable files, directories, files larger than 64 KiB, invalid UTF-8/TOML, unknown keys and wrong types fail before creation. Diagnostics identify the field and reason without echoing file contents. No configuration directory or file is created by reading or checking it.

```toml
[terminal]
new_cwd = "follow"
default_shell = "pwsh.exe"
shell_mode = "auto"

[worktrees]
directory = "~/Projects/worktrees"

[session]
resume_agents_on_restore = true

[update]
manifest_check = true
```

Only the sections and keys shown here are accepted. `terminal.new_cwd` accepts `follow`, `home`, `current`, `workspace`, or a path. An empty value means `follow`. Paths use the Windows CLI path adapter: relative paths resolve against the configuration file's parent, tilde paths use Windows home, and ambiguous drive-relative, UNC and device paths are rejected. A missing `APPDATA` leaves defaults available; an explicit malformed value does not silently fall back.

## Creation policy

| Input                                           | Result                                                                                                    |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Explicit CLI cwd or worktree path               | Overrides the corresponding valid setting                                                                 |
| Omitted cwd in Workspace, Tab or Panel creation | Apply `terminal.new_cwd`, then the existing source-directory policy                                       |
| Omitted worktree path with configured root      | Query the source repository before any Project or Git mutation; select `root/repository-name/branch-slug` |
| No configured root                              | Keep the existing sibling checkout path                                                                   |
| Resume                                          | Use the path and branch already retained in the operation; do not reload settings                         |

Branch slugs lowercase ASCII letters, collapse nonalphanumeric runs to a hyphen, trim hyphens and use `worktree` if empty. Generated branches remain `worktree/<operation-id>` and are fixed in the submitted intent. The read-only repository query uses the same source and trust policy as creation. The Server still rechecks the source and owns every Git and canonical Workspace effect; the query is not a filesystem transaction. Explicit path avoids the extra query. Configuration is read once per invocation, including validation before an explicit override is applied.

The creation settings govern Windows CLI creation. The legacy explicit empty-Workspace command, TUI direct creation, existing terminals, Server configuration reload and keybindings retain their own contracts; the shell policy below also covers TUI creation.

## Check and correctness

Windows `config check` version 2 adds a sixth `user-config` check to the five runtime checks. Version 1 keeps its five-check schema. The added check reports the selected file, presence, normalized effective settings, or a bounded reason and field. Invalid creation configuration uses the separate `orchbee.cli.failure.configuration` version-1 envelope and exits 1. Failed source discovery stops configured-path creation before first-Project setup; no mutation is retried.

The shared Windows settings reader owns bounded file inspection and normalization; the creation workflows own precedence and existing effect receipts. Reads have no writer or cached mutable state; the explicit reset operation below owns configuration repair. Core evidence covers malformed and duplicate TOML, unknown keys, bounds, relative/tilde paths, override precedence and pinned Herdr slug vectors. Windows public IPC traces cover effective launch cwd, configured checkout intent, failed discovery with no mutation, explicit path without discovery and unchanged no-file behavior. Full application acceptance remains a separate gate. Reference behavior: Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `TerminalConfig`, `WorktreesConfig` and `default_checkout_path`.

## Restore policy

The shared Windows settings reader in `orchbee-local-site-runtime` owns bounded file inspection, strict TOML types and literal Windows path normalization. The CLI projects creation paths into Workbench types; no Workbench dependency is added to the runtime library. The existing pinned `toml` parser is a Windows dependency for the actual Client and Server consumers.

`[session] resume_agents_on_restore` is a TOML boolean, default `true`. `ORCHBEE_RESUME_AGENTS_ON_RESTORE` overrides it with exactly `true` or `false`; an override never hides an invalid file. This setting applies only when starting a new Site lifetime. Existing Site admission does not read it, and editing it does not change a running Site.

Client startup reads and validates once before durable Active publication or activation marker commitment. It passes the resolved boolean as a private Server argument; the Server validates that argument without rereading mutable settings. Direct managed Server startup without that argument reads the same file/environment owner before Worker launch. Invalid input leaves the preceding checkpoint and stopped state available for correction and retry. The private argument requires a matched cohort; rollback uses the preceding whole cohort.

Windows `config check` version 3 reports `resumeAgentsOnRestore` and `resumeAgentsOnRestoreSource` (`default`, `file`, `environment`). Schemas for versions 1 and 2 remain available. Diagnostics report only a bounded field/reason. Checks must cover default/file/environment precedence, malformed and non-UTF-8 values, strict types/keys, read bounds, before-commit refusal and corrected retry, unchanged existing-Site admission, and actual disabled/enabled restart. Live reload, names and managed-state restoration are separate obligations.

## Rule update policy

`[update] manifest_check` is a TOML boolean, default true, with `default`/`file` provenance.
Windows `config check` v4 adds `manifestCheck` and `manifestCheckSource` while v1-v3 remain closed.
Client activation passes the validated check value/provenance and configuration path/explicit-file
flag together with the resume policy. The managed Server admits this complete private snapshot
without rereading the file; direct startup uses the shared settings reader once. Cohorts must match.
`read_windows_user_settings_at` supports bounded explicit reload at the frozen location, retaining
missing-default versus missing-explicit semantics. It does not create configuration files.

Persistent managed release Sites check rules at startup and every thirty minutes. Debug/test builds
and standalone endpoints disable automatic network work. `site agent-manifests` exposes effective
policy and latest automatic results. `site reload-agent-manifests` validates the complete file and
atomically refreshes the rule-check switch with the detector; invalid configuration retains both.
Turning checks off removes scheduling; turning them back on schedules an immediate check. An
active update returns busy for reload. Other running settings and keybindings are not hot-reloaded
by this command. Explicit updates remain available when automatic checks are disabled. See the
[screen model](../../orchbee-server/docs/agent-screen-model.md#automatic-checks-and-configuration-snapshots).
Release acceptance fixtures must explicitly disable checks when public network access is outside
the test's purpose. Rolling back to a reader predating this key requires removing the update section;
user overrides and already committed rule caches are retained.

## Reset custom keybindings

Windows `config reset-keys [--format text|json]` is an offline configuration repair operation.
It uses the same file location as `config check`, but a missing file (including an explicit path)
or no top-level `keys` value succeeds without creating or writing anything. It removes `[keys]`,
its nested tables and command arrays, or a safely removable top-level assignment. Other settings,
comments outside the removed tables and line endings remain byte-for-byte. Invalid UTF-8/TOML, files over 64 KiB and edits whose
parsed result differs from the original minus `keys` fail before writing. This allows recovery from
custom keys that the current settings reader does not yet support; it does not validate or repair
other settings, implement custom binding dispatch, or reload a running Client.

The shared Windows settings repair owner pins the private parent directory tree, rejects reparse
points/nonregular or multiply linked files, and reopens the file exclusively before comparing its
bytes with the prepared input. Changed input or a concurrent file holder refuses the edit. A unique
adjacent `.bak-keys-...` backup is created without overwriting an existing file, admitted as private,
and flushed before rewriting the original through its exclusive handle. The original's file identity
and permissions remain. A write/flush failure restores and flushes the original before releasing it;
failed recovery retains the complete backup and reports `recoveryRequired: true`. A process crash
after rewriting begins also leaves the previously flushed backup available. This is recoverable
in-place replacement, not a filesystem transaction across power loss.

The closed `orchbee.config.reset-keys` v1 receipt identifies `missing`, `no-keys`, `reset` or `failed`,
the selected path, nullable backup path, bounded error field/reason and recovery requirement. Exit 0
means reset or no change, exit 1 means failure, and usage errors exit 2 before file access. Text output
is localized and escapes paths; configuration values are never echoed. Successful backup files are
user recovery artifacts, explicitly handed to the caller in the receipt for restoration/removal.
Unneeded failed-attempt backups are removed; cleanup failure exposes their exact path. No automatic
Site admission, reload, process launch or unrelated settings write occurs.

Pure transformation tests, exclusive-file/backup checks, injected write/recovery failure witnesses
and real Windows CLI processes cover this contract. Full keybindings, config reload and final
Windows replacement acceptance remain separate obligations.

## Configured Windows shell

`terminal.default_shell` is one literal executable, not a command line: an absolute local
Windows path or bare executable name, at most 4,096 UTF-8 bytes, without control characters.
Outer whitespace is trimmed; empty selects the existing platform default. Relative paths with
separators, drive-relative, UNC and device paths are rejected. Bare names resolve on the Worker's
absolute PATH directories, checking the literal name and then `.exe` when no extension was given;
no current-directory, PATHEXT or script fallback occurs. A configured missing executable fails
launch and never falls back. Empty configuration still uses COMSPEC then SystemRoot/cmd.exe;
automatic PowerShell preference remains a separate parity obligation.

`terminal.shell_mode` accepts exactly `auto` (default), `login`, or `non_login`. On Windows,
auto/non_login add no arguments; login adds `-l` only for sh, bash, zsh, ksh, dash, ash, mksh,
fish, csh and tcsh, case-insensitive with optional `.exe`. Other shells have no login flag.
Existing PowerShell prompt integration runs after shell selection. Explicit executables and
Launch Profiles retain their exact arguments and bypass configured default selection.

Client activation freezes these values alongside existing settings before commitment. The matched
Server forwards this private snapshot to Worker without rereading configuration. Worker owns the
snapshot across lazy Host startup and later terminal preparation, including CLI/TUI creation and
default-shell restore. Direct unmanaged Worker startup validates its configuration once before
admission. Prepared operations and replay retain their resolved launch facts. Editing the file
does not change live shells or running Worker policy; a new Site lifetime is required until general
reload is implemented. Windows `config check` v5 reports `defaultShell` and `shellMode`; prior
versions remain closed. Shell-specific creation errors use configuration failure v2; v1 keeps
its original field/reason set. These are configured values, not a claim that a binary can launch.

The shared parser and private snapshot admission, Worker launch resolver, explicit-override and
owned-loop tests, and actual Windows CLI checks cover this contract. No public wire shape or
persisted checkpoint changes; deployment and rollback use the whole matched cohort.
