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
channel = "stable"
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

`[update] channel` accepts exactly `stable` (default) or `preview`, independently of `manifest_check`. `orchbee channel show [--format text|json]` reads the complete shared configuration without creating files, activating a Site or accessing the network. Text is the channel token; `orchbee.channel.show` v1 reports channel, default/file provenance, selected configuration path and file presence. An absent default file uses stable; an absent explicit file or malformed configuration fails. This is the requested release preference, not evidence about the currently installed cohort. Windows `config check` v9 and configuration failure v6 admit `update.channel` diagnostics; earlier schemas remain closed.

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
other settings or reload a running Client.

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
and real Windows CLI processes cover this contract. Full keybindings, complete reload parity and final
Windows replacement acceptance remain separate obligations.

## Configured Windows shell

`terminal.default_shell` is one literal executable, not a command line: an absolute local
Windows path or bare executable name, at most 4,096 UTF-8 bytes, without control characters.
Outer whitespace is trimmed; empty selects the existing platform default. Relative paths with
separators, drive-relative, UNC and device paths are rejected. Bare names resolve on the Worker's
absolute PATH directories, checking the literal name and then `.exe` when no extension was given;
no current-directory, PATHEXT or script fallback occurs. A configured missing executable fails
launch and never falls back. Empty configuration prefers the first compatible `pwsh.exe` in absolute PATH directories, then
the absolute `SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe`. COMSPEC/SHELL and
current-directory lookup do not override this policy. Selection never executes a probe.

`terminal.shell_mode` accepts exactly `auto` (default), `login`, or `non_login`. On Windows,
auto/non_login add no arguments; login adds `-l` only for sh, bash, zsh, ksh, dash, ash, mksh,
fish, csh and tcsh, case-insensitive with optional `.exe`. Other shells have no login flag.
Existing PowerShell prompt integration runs after shell selection. Explicit executables and
Launch Profiles retain their exact arguments and bypass configured default selection.

Client activation freezes these values alongside existing settings before commitment. The matched
Server forwards this private snapshot to Worker without rereading configuration. Worker owns the
snapshot as a fallback across lazy Host startup and later terminal preparation. Server-authorized
CLI/TUI/default-shell restore preparations capture the latest published defaults as described below.
Direct unmanaged Worker startup validates its configuration once before admission. Prepared
operations and replay retain their resolved launch facts. Editing the file alone does not publish
settings or change live shells; explicit reload updates defaults for subsequent preparations.
Windows `config check` v5 reports `defaultShell` and `shellMode`; prior
versions remain closed. Shell-specific creation errors use configuration failure v2; v1 keeps
its original field/reason set. These are configured values, not a claim that a binary can launch.

The shared parser and private snapshot admission, Worker launch resolver, explicit-override and
owned-loop tests, and actual Windows CLI checks cover this contract. No public wire shape or
persisted checkpoint changes; deployment and rollback use the whole matched cohort.

Automatic PATH candidates require bounded DOS/PE/COFF and optional-header inspection,
nonempty section headers within the file, executable-image/not-DLL flags and compatible
x86/x64/ARM64 machine type. Malformed, missing, inaccessible or incompatible candidates are
skipped. This proves a structural candidate, not that its dependencies load. The existing pinned
`winsafe` kernel wrapper supplies system architecture without authored unsafe code.
[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo)
reports emulated architecture for x86/x64 processes on ARM64; native ARM64 preference through an
emulated Worker is not claimed aligned with Herdr's IsWow64Process2 policy. Explicit configuration
still selects that executable. Real x64 selection and synthetic header/architecture partitions
cover this implementation; no ARM64 hardware acceptance is claimed.

## Windows Client key configuration

The bounded shared settings reader admits `[keys]` before Client startup. Each Client owns an
immutable binding snapshot until publication; file edits/reset affect the next Client or an explicit live reload. The Client resolves implemented
semantic actions against its default registry. User overrides replace that action's defaults and
conflicting default chords. Ambiguous user chords, unsupported actions and invalid syntax fail
admission. This strict failure policy differs from Herdr's diagnostic-and-disable policy and remains
an explicit parity gap.

`prefix` accepts one chord. Action values accept a string or at most 16 alternatives; an empty
string or array disables the action. `prefix+` selects the prefix layer; otherwise a safe direct
chord is required. `switch_tab` and `focus_agent` admit `1..9` with modifiers. Agent ordinals follow
the filtered machine Agent list; `focus_agent` is unbound by default. Complete modifiers and uppercase
letters normalize before collision checks. The prefix itself is reserved in both layers. Direct
printable keys, bare editing keys and Ctrl+C/D/Z are refused. Capture and mode ownership precede
direct dispatch; paste and releases never invoke bindings. Double prefix forwards canonical C0
where available, otherwise original bytes, without a hardcoded Ctrl+B fallback.

The resolved registry supplies dispatch, help rows and principal live hints. Disabled actions stay
in help. Existing native defaults remain; unsupported Herdr fields fail explicitly. Complete reload parity,
custom commands, navigation-mode mappings, ordinal Agent/indexed Workspace keys and other unsupported
actions remain unfinished. Targeted evidence covers bounds, collisions/default precedence,
ownership, dispatch and discovery; no full regression or visual acceptance is claimed.

Supported action fields are `help`, `new_workspace`, `rename_workspace`, `workspace_picker`,
`detach`, `previous_workspace`, `next_workspace`, `previous_agent`, `next_agent`, `focus_agent`, `new_tab`, `rename_tab`, `previous_tab`,
`next_tab`, `move_tab_previous`, `move_tab_next`, `switch_tab`, `close_tab`,
`focus_pane_left/down/up/right` (four fields), `split_vertical`, `split_horizontal`,
`close_pane`, `zoom` (alias `fullscreen`), `toggle_sidebar` and `reload_config`. They invoke existing native actions;
Workspace means the native Workbench location, vertical split is rightward and horizontal is downward.
Agent previous/next defaults to prefix `{`/`}` and uses the same ephemeral filtered/sorted
[machine Agent list](machines.md#agent-metadata-view-controls-and-navigation), with exact physical
target revalidation. The same owning contract defines configured ordinal Agent focus and source-owned view JSON.

```toml
[keys]
prefix = "ctrl+a"
help = ["prefix+?", "f1"]
new_tab = "prefix+n"
switch_tab = "alt+1..9"
focus_agent = "prefix+alt+1..9"
zoom = ""
```

Windows `config check` v6 adds `keyPrefix` and `keyOverrides`; key configuration failures use
configuration failure v3. Older versions retain their closed field sets. The example lets new Tab
replace the default next-Tab chord; empty `zoom` disables its shortcut. Bindings currently apply to
the main TUI. CLI terminal-stream attach deliberately keeps its independent Ctrl+B/q escape;
pinned Herdr `e0507237` also uses a fixed escape in `src/client/attach.rs`, so configurable attach
is not a parity requirement. Complete keymap import and recovery through nonfatal diagnostics
remain future implementation.

Function keys use the actual xterm F1–F12 sequence numbers (skipping 16 and 22), modified
F1–F4, and kitty F1–F24. Modifier event tails retain press, repeat and release; invalid events or
unrepresentable Hyper/Meta modifier bits never collapse into ordinary shortcuts. Availability of
these keys still depends on the terminal host.

## Live configuration reload

The sidebar profile participates in the same admission/publication transition:
`ui.sidebar.agents.rows`, `rows_by_agent` and `row_gap` are validated by the shared
Windows settings owner, then published as a bounded canonical profile. A Client
validates keys, notifications and sidebar before installing any of them. A bad
profile leaves the whole previous Client configuration intact. Startup applies
the same admitted settings; configuration checking reports the supported layout
and its bounds. See the [Agent sidebar configuration model](machines.md#agent-sidebar-configuration-model)
for tokens, provider selection, rules, geometry and evidence obligations.

Windows `site reload-config` requests a reload in the running Site; `site config` reads the last
published revision. Neither activates a Site. Server freezes the startup configuration location,
serializes reloads, validates the bounded whole file and prepares its canonical key profile before
commit. Failure retains the previous revision, key profile and live manifest-check schedule.
Revision 0 is a startup baseline, not a live publication. The first successful reload publishes revision 1; later revisions advance only when admitted settings change. Readers may observe busy
while another reload owns admission. A submitted request with a lost reply is outcome-unknown;
Clients do not automatically resubmit mutations and can read current state for recovery.

Server commits the supported live manifest-check policy and publishes keys for connected main-TUI
Clients. Publication is not an acknowledgement from every Client. Each Client owns a cancellable
configuration watcher and applies only a validated newer profile between input packets; pending
prefix state is cleared, editor/confirmation ownership and user drafts remain intact. Reload failure
keeps current keys and produces a localized notice. The reload action itself is configurable through `reload_config`. The native default is prefix F5; Herdr uses prefix Shift+R, which conflicts with the existing native Project rename default. A profile such as `reload_config = "prefix+shift+r"` explicitly replaces that conflicting default.

Changed restore-on-start policy is returned in `restartRequired` relative to Site startup;
already-restored sessions remain owned by their current lifetime. Default shell and mode apply to
subsequent preparations without a restart. CLI directory/worktree defaults are caller reads on the
next invocation. This is partial Herdr reload parity: strict whole-file rejection, unimplemented
configuration families and Client-specific profile policy remain unfinished.
The wire key profile is capped at 64 KiB; up to 144 expanded triggers per action preserve the user file limit of 16 indexed alternatives. Polling uses an independent cancellable connection every 500 ms, with a two-second exchange limit. A successful reload is published state, not an acknowledgement that every Client has applied it.

No persisted configuration migration or automatic retry is introduced. Tests cover authorization,
bounded serialization, failure retention, no-change reload, publication application and shutdown.

## Live defaults for subsequent Windows terminals

Reload commits a validated `DefaultShellSelection` into the existing Server TerminalCoordinator. Its creation admission captures that immutable value in each new default `AuthorizedTerminalLaunch`; explicit executable/Profile requests carry no default override. The Worker validates the Windows shell syntax and resolves its own executable/PATH and mode flags before Host handoff. An absent override retains the Worker's startup fallback for unmanaged/legacy private requests. Existing PTYs, prepared launches, exact replay and deferred Start never acquire later defaults. New CLI/TUI/restore creation uses the same coordinator admission.

The commit lock order is configuration registry, Site coordinator, then manifest scheduling. Candidate validation and wire-value construction precede commit; scheduler admission refusal leaves both launch policy and publication unchanged. No Worker IPC or distributed acknowledgement occurs under these locks. Shell/mode changes apply to subsequent creation and cease to require Site restart; restore-on-start remains a startup-only requirement. Successful reload confirms configuration admission, not executable availability.

Launch preparation failure records the existing known pre-handoff failure, retains the Worker/Server connection and creates no Host terminal. The same failed request replays its failure; corrected settings require a fresh creation identity. The lazy loop also remains available before any Host exists. Host startup/transport uncertainty retains its prior retirement and no-retry policy. Evidence covers private framing, default/explicit/absent selection, preparation across reload, exact/conflicting replay, failure retention and real Windows pipe exchanges; full provider/multi-Client/visual acceptance remains deferred. Matching private cohorts deploy or roll back together without a durable migration.

`site config` and reload receipts expose the admitted `defaultShell` and `shellMode` for subsequent default preparations, including after a lost reload reply. Empty shell means the platform default; these values are not Worker executable-resolution or launch-success evidence. CLI receipt v2 requires both fields while the historical v1 schema stays closed and unchanged. Matched private Workbench snapshots require the same bounded literal (4096 UTF-8 bytes, no controls) and `auto`/`login`/`non_login` mode. Human output escapes the literal and localizes the empty-shell label.

## Outer window titles

`[ui] window_title` defaults to `{hostname}: {workspace}` and reloads online. The [title owner](windows-terminal-title.md#configured-templates) defines tokens, literal braces, invalid/empty policy, bounded rendering and CLI override precedence. `config check` v8 reports `windowTitle` and nullable `windowTitleDiagnostic`; invalid template syntax is advisory, while wrong types/unknown UI keys/over-4096-byte values fail before admission (`orchbee.cli.failure.configuration` v4). `site config`/`site reload-config` v4 expose the published raw template. Prior public versions remain closed.

## Custom-notification configuration

`ui.toast.delivery` accepts off (default), orchbee/herdr, terminal or system; legacy `ui.toast.enabled` supplies off/card only when delivery is absent. `ui.toast.herdr.position` selects the four-corner default. These settings configure custom CLI notifications, not Agent delay or clipboard policy. Startup is Client-local; explicit Site reload publishes a canonical bounded profile alongside keys, and Clients validate both before applying either. `site config`/reload v4 expose `notificationProfile`, `config check` v8 exposes delivery/position, and configuration failure v5 adds `ui.toast`. Earlier public versions remain closed. See [notification ownership](windows-notifications.md).

## Windows release installation

`orchbee update [--format text|json]` installs the configured channel. `orchbee channel set <stable|preview> [--format text|json]` validates and atomically saves the channel first, preserving comments and other TOML values, then runs the same updater. A missing explicitly selected file may be created by this writer. Update failure retains the saved preference. Both commands work without a Site; neither starts, stops, replaces or reconnects a running Site.

Set `ORCHBEE_UPDATE_MANIFEST` to an explicit absolute local release manifest or HTTPS URL. `{channel}` expands to the selected token. There is no implicit public feed. The chosen manifest is the trust authority; per-file SHA-256 checks prove content integrity against it, not an independent publisher signature. `ORCHBEE_INSTALL_DIR` defaults to `LOCALAPPDATA/Programs/Orchbee`. Install paths reject PATH delimiters, expansion tokens and controls. Source acquisition has a shared 600-second deadline and Ctrl+C cancellation. HTTPS uses curl 8.4 or newer from the OS system directory with HTTPS-only bounded redirects, no curlrc or inherited proxy/credential environment, and bounded downloads.

A release manifest is closed JSON with `schemaVersion: 1`, `product: "orchbee"`, `platform: "windows-x86_64"`, `channel`, `version`, `buildId` and `files`. Each file has exactly `path`, `url`, `sha256` (lowercase), and `size` (bytes). File URLs are HTTPS or portable relative paths resolved beside the manifest. The manifest is at most 64 KiB, with at most 4096 files, 512 MiB per file and 2 GiB total. Portable paths reject traversal, Windows devices, alternate streams, case aliases and file/directory collisions. Required contents include all four executables, schema-2 `cohort.json`, the adapter capsule, and the three ConPTY distribution files. Every declared file is copied into a private stage and hashed; cohort and capsule digests are admitted before the new Client executes its exact version smoke check.

The installation owner serializes writers and publishes an immutable directory named by the manifest SHA-256. Existing releases are verified and reused, never overwritten. User PATH activation is a later effect: a failure or lost result retains the published path for explicit retry. Existing processes keep their original files. Older releases are installation state and are retained because they can still be running; automatic retention and live handoff are separate work. Unpublished stages are cleaned after child settlement; unconfirmed settlement or cleanup failure transfers the exact stage path to explicit recovery in the receipt. No old release is deleted as staging cleanup.

`orchbee.update` v1 distinguishes channel saved, release published, PATH activated, reuse, interruption, failure phase/reason and pending cleanup. Status is 0 on completion, 1 on known failure, 2 on usage error, 7 on timeout, 8 on unconfirmed physical/activation outcome, and 130 on interruption. A published path can be used directly even when activation needs retry; a new sign-in refreshes inherited PATH. Full distribution publication, HTTPS source acceptance and real user PATH/whole-cohort startup acceptance are deployment obligations and are not implied by a local staging test.

To prepare a feed from a completed Windows cohort without publishing it:

```powershell
node scripts/package-native-windows.mjs C:/path/to/cohort stable 0.1.0 build-id
$env:ORCHBEE_UPDATE_MANIFEST = 'C:/path/to/cohort/release.json'
orchbee update --format json
```

The packaging tool verifies the existing cohort, adapter capsule and pinned ConPTY bundle, inventories bounded regular files, and exclusively writes `release.json` beside them. It refuses an existing feed. For HTTPS distribution, publish that directory at the feed URL; relative file URLs retain the same directory layout. Version must match the packaged Client's `--version`; buildId distinguishes releases with the same version.

The immutable release also stores the exact admitted `release.json`, reserved outside the inventory. Reuse checks that it has not changed. `status client` v2 binds this manifest to the directory identity and verified cohort before projecting installed release provenance; it never substitutes the current configured preference.

## Windows Agent sidebar layouts

The main Windows TUI applies `ui.sidebar.agents` at startup and after `site reload-config`.
For example:

```toml
[ui.sidebar.agents]
rows = [["state_icon", "state_text"], ["machine", "workspace", "tab"], ["pane", "$summary"], ["terminal_title_stripped"]]
row_gap = 1

[ui.sidebar.agents.rows_by_agent]
claude = [["state_icon", "agent"], [{ token = "$model", fg = "#8bc", rules = [{ contains = "fast", ignore_case = true, bold = true }] }]]
```

An omitted layout preserves the existing two-line default; an explicit empty layout
keeps the identity/navigation line. Missing or hidden values do not consume rows.
The full [sidebar model](machines.md#agent-sidebar-configuration-model) defines
provider aliases, rules, actual-height windows and unchanged physical authority.
Live `terminal_title`/`terminal_title_stripped` now read the exact physical OSC title.
The raw form retains activity glyphs; the stripped form removes one elevation prefix
and one recognized leading activity glyph. Missing or cleared titles disappear;
`$terminal_title` remains a separate custom value.
`ui.sidebar.spaces` is a separate unimplemented configuration family.

`config check` v10 and Site configuration receipts v5 expose required
`agentSidebarProfile` (closed canonical JSON, at most 32 KiB). Configuration failure
v7 includes `ui.sidebar.agents` and layout/color/token/rule/provider diagnostics.
All prior public versions retain their closed fields. Catalog v122 keeps 105 schemas
and 141 grammar paths. Matched private cohorts carry the new required profile
together; deployment/rollback is by whole cohort, without persistent migration.
