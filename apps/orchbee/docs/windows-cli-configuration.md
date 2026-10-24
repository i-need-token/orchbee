# Windows CLI configuration

## File and validation

Windows CLI creation reads `APPDATA/Orchbee/config.toml`, or the absolute file named by `ORCHBEE_CONFIG_FILE`. A missing default file means unchanged defaults; a missing explicit file is an error. Invalid or empty location variables, unreadable files, directories, files larger than 64 KiB and invalid UTF-8/TOML are fatal before creation. A successfully parsed document is admitted field by field: an invalid supported leaf uses its existing safe default, an invalid profile uses that profile's existing safe default, and unknown fields are ignored. The reader reports every fallback as a warning without echoing values or commands. No configuration directory or file is created by reading or checking it.

```toml
[terminal]
new_cwd = "follow"
default_shell = "pwsh.exe"
shell_mode = "auto"
focus_new_panel = true

[worktrees]
directory = "~/Projects/worktrees"

[session]
resume_agents_on_restore = true

[update]
channel = "stable"
manifest_check = true
```

Only documented sections and keys affect Orchbee, including the `[keys]` and `[ui]` families described below. The known Herdr-only top-level families `onboarding`, `server`, `advanced`, `experimental` and `remote` are identified as unsupported instead of being treated as generic unknown fields. Known unsupported nested fields include `terminal.kitty_graphics`, `update.version_check`, Herdr-only presentation and clipboard settings, custom sound paths, and key actions that have no Orchbee action. Recognition never enables a plugin, installs a provider or runs a configured command. Orchbee does not automatically read a Herdr configuration file.

`terminal.new_cwd` accepts `follow`, `home`, `current`, `workspace`, or a path. An empty value means `follow`. Paths use the Windows CLI path adapter: relative paths resolve against the configuration file's parent, tilde paths use Windows home, and ambiguous drive-relative, UNC and device paths are rejected. An invalid path falls back only that leaf. A missing `APPDATA` leaves defaults available; an explicit malformed value does not silently fall back. The documented `ORCHBEE_RESUME_AGENTS_ON_RESTORE` environment override remains strict: anything other than exact `true` or `false` is fatal and cannot be hidden by file fallback.

Warnings use a bounded canonical reason grammar and a closed disposition vocabulary, and carry only a bounded canonical dotted field path. They are deduplicated and sorted by field and reason. A result carries at most 128 warning entries and an exact omitted count, so truncation is always visible. Unknown path segments are escaped and truncated on a UTF-8 boundary; distinct original fields are deduplicated before display truncation.

### Default configuration output

`orchbee --default-config` is a sole offline invocation. Argument parsing selects it before any environment read or runtime discovery, and it does not activate a Session. It writes deterministic UTF-8 TOML with one trailing newline to stdout. A fixed-English comment reference lists admitted setting defaults, choices and purposes; English keeps this copyable config artifact identical across CLI locales. Registered Client actions appear as commented binding arrays, including `[]` for actions without a default binding, so uncommenting a row opts into an override while leaving the generated file unchanged preserves the Client registry's evolving defaults. The action rows are rendered from the same default binding registry used by the Client. The reference also describes named themes and custom-command rules. The legacy leaves `theme.name`, `ui.toast.agent_delivery`, and `ui.sound.enabled` remain commented; when no shared preference record exists, uncommenting one explicitly migrates its value into shared user preferences. Settings manages current shared preferences. `config check` accepts the generated output without warnings, and Windows CLI parsing produces the same effective settings and bindings as an absent file. Reading the generated file through the native `UserPreferencesStore` also preserves shared preference values at their absent-file defaults.

## Named chrome themes

The shared [application appearance preference](../../../crates/workbench-protocol/docs/user-preferences.md)
supports Light (`light`), Dark (`dark`) and System (`system`); System is the
common default. In the native TUI, System follows the host-default background and uses
terminal-native contrast when no host background is observed. Windows TOML `[theme] name` remains a separate legacy
named-theme migration input: it accepts `terminal-native`, `one-light` or `one-dark`,
and the offline default TOML keeps the registry default as the commented example
`# name = "terminal-native"`, which migrates only when explicitly uncommented with no shared preference record. Invalid names or
types warn and default this field; `theme.custom` remains unsupported by this legacy
setting. See [native themes](native-themes.md) for polarity, color capabilities,
all chrome consumers, shared reload and rollback. Herdr import keeps its existing
conversion exclusions; select a theme explicitly in the candidate file.

`config check` v14 adds required `themeName` to admitted Windows user-config details;
its fatal shapes remain unchanged and versions 1–13 stay closed. Session configuration
v9 adds required `themeName` to successful snapshots; versions 1–8 stay closed.
Catalog v142 publishes these extensions without adding schemas or command paths.

## Herdr configuration import

Windows `orchbee config import-herdr [--input-file <source>] [--output-file <destination>]
[--dry-run] [--format text|json]` converts one Herdr file into a new Orchbee candidate. An omitted
`--input-file` selects `%APPDATA%\herdr\config.toml`; an omitted `--output-file` selects the file
Orchbee reads, `ORCHBEE_CONFIG_FILE` or else `%APPDATA%\Orchbee\config.toml`, which must not exist
yet. It does not connect to a Session, execute commands, or reload running Clients. `--machine` and
`--session` are rejected. File operands are single-use and never mean stdin; `-` is refused.
Relative operands use
one captured caller directory, and tilde uses the existing Windows home adapter. The resolved
ordinary local Windows paths reject UNC/device paths, alternate streams, device names, dot
components, trailing dots/spaces and empty leaves. Source-relative settings use the resolved
source operand's parent, independently of the destination.

The shared Windows settings owner admits one regular, non-reparse, single-link source handle
with write/delete sharing disabled. It caps acquisition before decoding at 64 KiB and parses
strict UTF-8 TOML. An unreadable, oversized, non-UTF-8 or syntactically invalid source is fatal.
`APPDATA` and `ORCHBEE_CONFIG_FILE` only select an omitted operand's default, and
`ORCHBEE_RESUME_AGENTS_ON_RESTORE` never influences the import. Only the declared file operands
and tilde expansion can use directory context or Windows home variables.

### Conversion model

The principal operation prepares an immutable, sparse candidate and a bounded field report.
Unspecified settings remain unspecified; import does not freeze either product's defaults.
Existing leaf/profile admission determines valid values and safe defaults. Invalid ordinary
leaves, prefix and profiles are omitted and reported; an invalid known action is emitted as an
empty binding array so only that action is disabled. Unsupported commands and values never
reach the candidate. The selected compatibility reference is Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34` on Windows, specifically its configuration model,
keybinding, sidebar and notification representations. This is a configuration subset conversion,
not a claim about complete Herdr runtime behavior.

| Explicit source fields                                      | Candidate behavior                                                                                                                                                    |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal.default_shell`, `terminal.shell_mode`             | Existing Windows literal shell admission                                                                                                                              |
| `terminal.new_cwd`, `worktrees.directory`                   | Preserve policies; freeze relative/tilde paths against the source context as absolute paths                                                                           |
| `session.resume_agents_on_restore`                          | Preserve the explicit valid boolean                                                                                                                                   |
| `ui.window_title`, `ui.sidebar.agents`, `ui.sidebar.spaces` | Preserve compatible explicit title/profile fields, including explicitly empty rows                                                                                    |
| `keys.prefix` and current built-in action registry          | Preserve admitted bindings; `zoom` wins over `fullscreen`; registry order resolves conflicts and disables losing actions                                              |
| `ui.toast.delivery` or legacy `ui.toast.enabled`            | Explicit `delivery` wins even when invalid; valid `herdr` becomes `orchbee` and sets both `delivery` and `agent_delivery`; absent input preserves Orchbee's defaults  |
| `ui.toast.delay_seconds`, `ui.toast.overlay.position`       | Preserve admitted delay and position                                                                                                                                  |
| `ui.sound.enabled`, `ui.sound.agents`                       | Preserve explicit boolean and `default`/`on`/`off`; `open_code` maps to `opencode`, `github_copilot` to `copilot`, and `agy` stays `agy`                              |
| Other fields                                                | Report and omit; this includes all Herdr update policy, onboarding/theme/server/advanced/experimental/remote, graphics, custom sound paths and custom/plugin commands |

Reports carry field identities, target fields, closed dispositions and canonical reasons, never
rejected values or command text. They are sorted and deduplicated before bounded field rendering,
with at most 128 visible entries and an exact omitted count. Only dry-run JSON includes
`candidateToml`, the explicitly requested preview payload; text/stderr do not echo configuration
values. A normal successful receipt reports the candidate's byte size and destination instead.
Before any destination access, preparation emits UTF-8 TOML with one trailing newline, rejects
an output over 64 KiB, and readmits the candidate at the destination context to verify the
intended effective subset. Source values omitted by the compatibility contract cannot become
active through output serialization.

### File workflow and recovery

| Phase                 | Commit and failure boundary                                                                                                                                                      |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parse and acquire     | Complete grammar, path admission and one source snapshot; failure creates nothing                                                                                                |
| Prepare               | Complete transformation, report, size and round-trip validation; failure creates nothing                                                                                         |
| Preview               | Return the prepared candidate/report without probing, locking or creating the destination                                                                                        |
| Create                | Pin every existing parent-directory ancestor against redirection; require a private final parent; exclusively create a new ordinary single-link leaf and protect its permissions |
| Write and synchronize | Keep the pins and exclusive leaf handle until close; write once and synchronize; only full successful synchronization permits `created` success                                  |
| Report                | Return one prepared receipt; failed stdout delivery does not undo a known file commit or authorize replay                                                                        |

The destination parent must already exist; import creates no directories, backups, temporary
files or runtime state. An existing destination or a competing creator refuses without
overwriting it. Creation-before-write is intentionally recoverable rather than an atomic
filesystem transaction: after creation, permission/write/flush failure reports the exact new
file and `recoveryRequired: true`, without deleting a pathname that another owner could replace.
Before creation, failure reports `created: false` and no recovery residue. A hard process crash
can leave an empty, partial or complete new file without a receipt; inspect the explicitly chosen
destination. Nothing edits the source or an existing configuration. There is no automatic retry.
Try the candidate through `ORCHBEE_CONFIG_FILE`; return to the original selection to roll back.
The caller owns the delivered candidate and any explicitly reported recovery residue.

The embedded schema catalog v148 includes this command.
The closed `orchbee.config.import-herdr` v1 receipt distinguishes `preview`, `created` and
`failed`, carries source/destination, field report, candidate size, creation/recovery facts and
typed failure stage/reason. Exit 0 means preview or synchronized creation, exit 1 is a known
local failure, and malformed grammar exits 2 before acquisition. An interrupted process follows
its platform signal exit behavior and never prints a fabricated success receipt.

### Import correctness

| Claim                                                                                                    | Executable evidence                                                                              | Limits                                                                                                                          |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Sparse conversion preserves the supported explicit subset and disables only invalid actions              | Deterministic source-derived vectors, stable generated partitions and candidate round-trip tests | Pinned supported configuration subset and 64 KiB bounds; TOML parser and existing leaf admission are trusted                    |
| Preview has no destination or runtime effects; invalid preparation never creates a target                | Denied-capability workflow tests and real Windows CLI processes                                  | No physical terminal or provider interaction is needed                                                                          |
| Concurrent imports never overwrite an existing target; only synchronized complete output reports success | Writer fault cuts and controlled two-creator Windows traces                                      | Two invocations and absent/partial/complete target abstraction; Windows exclusive-create/share/ACL APIs are adapter assumptions |
| Public grammar, receipt, localized report and discovery agree                                            | Grammar, closed schema, text/JSON, help, completion and offline schema tests                     | Machine output is locale-neutral; text escapes untrusted identities                                                             |

Run `cargo test --locked -p orchbee -p orchbee-local-site-runtime --all-targets --all-features` and the applicable root gate. Controlled source/preview/writer tests check preservation, no preview effect, exclusive creation, preparation before creation, untouched existing targets and success only after full flush. They do not prove arbitrary Windows ACL behavior, power-loss durability, receipt delivery or unbounded Herdr parity. No separate import TLA model or universal Lean theorem is selected.

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

Windows `config check` version 13 extends the `user-config` check with the admitted warning list and omitted count. A warning result reports status `warning`, keeps the overall document successful and exits 0. Fatal file or environment admission reports status `fail` and exits 1. Versions 1 through 12 remain closed. The check reports the selected file, presence and normalized effective settings, and text output ends with a `Config file:` line naming that file as a copyable path; fatal creation configuration continues to use the separate `orchbee.cli.failure.configuration` envelope. Failed source discovery stops configured-path creation before first-Project setup; no mutation is retried.

The shared Windows settings reader owns bounded file inspection, deterministic field admission and normalization; the creation workflows own precedence and existing effect receipts. Reads have no writer or cached mutable state; the explicit reset operation below owns configuration repair. Core evidence covers malformed and duplicate TOML, field-local fallback, stable bounded warnings, relative/tilde paths, override precedence and pinned Herdr slug vectors. Windows public IPC traces cover effective launch cwd, configured checkout intent, failed discovery with no mutation, explicit path without discovery and unchanged no-file behavior. Full application acceptance remains a separate gate. Reference behavior: Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `TerminalConfig`, `WorktreesConfig` and `default_checkout_path`.

## Restore policy

The shared Windows settings reader in `orchbee-local-site-runtime` owns bounded file inspection, field-local typed admission and literal Windows path normalization. The CLI projects creation paths into Workbench types; no Workbench dependency is added to the runtime library. The existing pinned `toml` parser is a Windows dependency for the actual Client and Server consumers.

`[session] resume_agents_on_restore` is a TOML boolean, default `true`. `ORCHBEE_RESUME_AGENTS_ON_RESTORE` overrides it with exactly `true` or `false`; an override never hides an invalid file. This setting applies only when starting a new Session lifetime. Existing Session admission does not read it, and editing it does not change a running Session.

Client startup reads and validates once before durable Active publication or activation marker commitment. It passes the resolved restore boolean together with the frozen configuration location as private Server arguments. The Server validates those arguments and does not rederive the restore decision after consuming it. Before configuration publication, the Server reads one complete candidate at the frozen location, with the same inherited strict environment override, and makes that candidate the authoritative revision-0 baseline. Direct managed Server startup establishes the same startup decision and frozen location from its own first read. Fatal input leaves the preceding checkpoint and stopped state available for correction and retry. The private arguments require a matched cohort; rollback uses the preceding whole cohort.

Windows `config check` version 3 reports `resumeAgentsOnRestore` and `resumeAgentsOnRestoreSource` (`default`, `file`, `environment`). Schemas for versions 1 and 2 remain available. Diagnostics report only a bounded field/reason. Checks must cover default/file/environment precedence, malformed and non-UTF-8 files, field-local type and key admission, read bounds, fatal before-commit refusal and corrected retry, unchanged existing-Session admission, and actual disabled/enabled restart. Live reload, names and managed-state restoration are separate obligations.

## Rule update policy

`[update] channel` accepts exactly `stable` (default) or `preview`, independently of `manifest_check`. `orchbee channel show [--format text|json]` reads the complete shared configuration without creating files, activating a Session or accessing the network. Text is the channel token; `orchbee.channel.show` v1 reports channel, default/file provenance, selected configuration path and file presence. An absent default file uses stable; an absent explicit file or malformed configuration fails. This is the requested release preference, not evidence about the currently installed cohort. Windows `config check` v9 and configuration failure v6 admit `update.channel` diagnostics; earlier schemas remain closed.

`[update] manifest_check` is a TOML boolean, default true, with `default`/`file` provenance.
Windows `config check` v4 adds `manifestCheck` and `manifestCheckSource` while v1-v3 remain closed.
Client activation passes the validated check value/provenance and configuration path/explicit-file
flag together with the resume policy. The managed Server admits these private values as its
startup-policy snapshot, then reads the complete authoritative revision-0 candidate at the
frozen location; already-consumed restore policy does not change. Direct startup establishes both
roles through the same settings owner. Cohorts must match.
`read_windows_user_settings_at` supports bounded explicit reload at the frozen location, retaining
missing-default versus missing-explicit semantics. It does not create configuration files.

Persistent managed release Sessions check rules at startup and every thirty minutes. Debug/test builds
and standalone endpoints disable automatic network work. `server agent-manifests` exposes effective
policy and latest automatic results. `server reload-agent-manifests` validates the complete file and
atomically refreshes the rule-check switch with the detector. A fatal file/TOML error retains both;
field warnings apply the admitted fallback settings.
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
Session admission, reload, process launch or unrelated settings write occurs.

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
skipped. Discovery resolves each candidate's canonical path and excludes images under an ancestor
containing an `AppxManifest.xml` file: Store/AppX activation rejects the Host's atomic Job-list launch.
This selection precedes terminal creation and continues through PATH or the system fallback;
it never retries creation or relaxes Job containment. Configured selections remain literal.
These checks prove a structural candidate, not that its dependencies load. The existing pinned
`winsafe` kernel wrapper supplies system architecture without authored unsafe code.
[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo)
reports emulated architecture for x86/x64 processes on ARM64; native ARM64 preference through an
emulated Worker is not claimed aligned with Herdr's IsWow64Process2 policy. Explicit configuration
still selects that executable. Real x64 selection and synthetic header/architecture and direct/nested
package-marker partitions cover this implementation; no ARM64 hardware acceptance is claimed.

## Windows Client key configuration

The bounded shared settings reader admits `[keys]` before Client startup. Each Client owns an
immutable binding snapshot until publication; file edits/reset affect the next Client or an explicit live reload. The Client resolves implemented
semantic actions against its default registry. User overrides replace that action's defaults and
conflicting default chords. An invalid explicit action remains an explicit disabled override, so its
built-in default cannot silently return; other actions and valid alternatives in the same action remain
available. Unknown actions are ignored with a warning, and known Herdr-only actions are reported as
unsupported. When explicit actions claim the same chord, the fixed action-registry order selects the
first and reports the later conflict. Diagnostics never retain the original binding or command.

`prefix` accepts one chord. Action values accept a string or at most 16 alternatives. An empty
string or array remains an explicit disabled override and emits a bounded warning. `prefix+` selects the prefix layer; otherwise a safe direct
chord is required. `switch_tab` and `focus_agent` admit `1..9` with modifiers. Agent ordinals follow
the filtered machine Agent list; `focus_agent` is unbound by default. Complete modifiers and uppercase
letters normalize before collision checks. The prefix itself is reserved in both layers. Direct
printable keys, bare editing keys and Ctrl+C/D/Z are refused. Capture and mode ownership precede
direct dispatch; paste and releases never invoke bindings. Double prefix forwards canonical C0
where available, otherwise original bytes, without a hardcoded Ctrl+B fallback.

`copy_mode` enters [keyboard copy mode](native-keyboard-copy.md), default `prefix+U` (capital U). It can use the same safe direct chords or an empty array to disable entry. In-mode motions remain fixed. Herdr import maps an explicitly supplied valid `keys.copy_mode` to this native action; older readers report it as unsupported.

`new_worktree` and `open_worktree` enter [Worktree launch](native-worktrees.md), default
`prefix+W` and `prefix+O`. Safe direct chords, disabling and explicit Herdr key import use
the same registry. `remove_worktree` remains unsupported.

`menu` opens the [native action menu](native-action-menus.md), default `prefix+f2`.
The same safe direct chords and explicit disabling apply. Machines keeps `prefix+M`.

`resize_mode` enters the [keyboard resize mode](native-keyboard-resize.md), default `prefix+P`.
`resize_pane_left`, `resize_pane_right`, `resize_pane_up` and `resize_pane_down` are unbound
by default and accept the same safe chords. They move the nearest eligible divider five percentage
points without entering the mode; for example, `resize_pane_left = "ctrl+left"`. Mode arrows are
fixed at five points, Shift+arrows at one, and Esc exits. Extra resize keys while pending are consumed,
not queued. Profiles opting into these action names require a matching current cohort; remove those
keys before using an older reader. Other existing profiles retain their defaults. Direct resize bindings require a modifier or a function key; bare editing keys stay with the pane.

The resolved registry supplies dispatch, help rows and principal live hints. Help omits actions with
no effective binding; a bound action that requires a session stays listed with a connection hint when
no session is available. Existing native defaults remain for actions without an explicit override;
unsupported Herdr fields warn without changing the effective registry. Invalid custom-command configuration disables
configured commands rather than executing a partial or unvalidated command. Navigation-mode mappings,
ordinal Agent/indexed Workspace keys and other unsupported actions remain unfinished. Targeted evidence covers bounds, collisions/default precedence,
ownership, dispatch and discovery; no full regression or visual acceptance is claimed.

The registry's display projection formats the normalized binding independently of accepted
configuration spelling. Help rows and live hints keep ASCII letters lowercase unless the binding
contains Shift; shifted letters display uppercase. Unicode glyph mode uses arrow glyphs and, on
Apple hosts, Apple modifier symbols. Simplified glyph mode spells `Up`, `Down`, `Left`, `Right` and
`Ctrl`, `Alt`, `Shift`, `Super`, including on Apple hosts. A prefix and its member are separated by
one space. These display rules do not change TOML spelling, normalization, matching or dispatch.

Supported action fields are `help`, `menu`, `new_worktree`, `open_worktree`, `new_workspace`, `rename_workspace`, `workspace_picker`,
`detach`, `previous_workspace`, `next_workspace`, `previous_agent`, `next_agent`, `focus_agent`, `new_tab`, `rename_tab`, `previous_tab`,
`next_tab`, `move_tab_previous`, `move_tab_next`, `switch_tab`, `close_tab`,
`copy_mode`, `resize_mode`, `resize_pane_left/down/up/right`, and `focus_pane_left/down/up/right` (four fields), `split_vertical`, `split_horizontal`,
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

Windows `config check` v13 retains `keyPrefix` and `keyOverrides` and adds the bounded warning record;
historical key configuration failure v3 remains closed for older cohorts. The machine `keyPrefix`
field keeps its stable platform-independent report spelling (`Ctrl+A` for this example), while TUI
shortcut discovery shows the actual unshifted gesture (`Ctrl+a`). The example lets new Tab replace
the default next-Tab chord; empty `zoom` disables its shortcut. Bindings currently apply to the main
TUI. CLI terminal-stream attach deliberately keeps its independent Ctrl+B/q escape;
pinned Herdr `e0507237` also uses a fixed escape in `src/client/attach.rs`, so configurable attach
is not a parity requirement. Complete keymap import remains a separate slice.

Function keys use the actual xterm F1–F12 sequence numbers (skipping 16 and 22), modified
F1–F4, and kitty F1–F24. Modifier event tails retain press, repeat and release; invalid events or
unrepresentable Hyper/Meta modifier bits never collapse into ordinary shortcuts. Availability of
these keys still depends on the terminal host.

## Live configuration reload

The sidebar profile participates in the same admission/publication transition:
`ui.sidebar.agents.rows`, `rows_by_agent` and `row_gap` are validated by the shared
Windows settings owner, then published as a bounded canonical profile. A Client
validates keys, notifications, both sidebars and the named theme before installing any field. A bad
private wire profile leaves the whole previous Client configuration intact. Startup applies
the Server's complete admitted baseline after the first snapshot arrives; a local projection shown
while connecting is provisional. Configuration checking reports the supported layout and its bounds.
See the [Agent sidebar configuration model](machines.md#agent-sidebar-configuration-model)
for tokens, provider selection, rules, geometry and evidence obligations.

Windows `server reload-config` requests a reload in the running Session; `server config` reads the last
published revision. Neither activates a Session. Server freezes the startup configuration location,
serializes reloads, parses the bounded whole file and prepares one admitted candidate containing
effective settings and warnings before commit. A fatal read or parse failure retains the previous
revision, every profile, diagnostics and live manifest-check schedule. A warning candidate atomically
replaces effective settings and diagnostics; a Client never combines settings from one revision with
diagnostics from another. Revision 0 is the authoritative startup baseline. A Client applies its first
complete Server snapshot once, including revision 0, then rejects duplicate or older revisions. The first
successful reload publishes revision 1; later revisions advance when either admitted settings or diagnostics change.
This includes clearing a warning whose fallback value equals the newly valid explicit value. Readers may observe busy
while another reload owns admission. A submitted request with a lost reply is outcome-unknown;
Clients do not automatically resubmit mutations and can read current state for recovery.

The 64 KiB source limit and each profile's individual bound are admission inputs; the composed
response must also fit the unchanged 256 KiB configuration-response limit. Before installing
revision 0 or applying any live policy, and before every reload commit, Server runs the same exact
serializer preflight over a complete `ConfigurationResult` (`Current` at startup and `Reloaded` on
reload) with fixed-width legal correlation identifiers. An admitted source whose combined escaped
profiles and diagnostics exceed that response bound is rejected without truncating otherwise valid
settings. Startup installs no configuration state or live policy; reload retains the complete prior
snapshot and live policy.

Server commits the supported live manifest-check policy and publishes keys, profiles, the named theme and the bounded
warning record for connected main-TUI Clients. Publication is not an acknowledgement from every
Client. Each Client owns a cancellable configuration watcher, rejects stale or duplicate revisions and
applies one complete validated snapshot between input packets. Pending prefix state is cleared;
editor/confirmation ownership and user drafts remain intact. A warning snapshot applies its effective
settings and shows a localized summary directing the user to `orchbee config check`; a later clean
snapshot clears that warning. Configuration health is retained independently from transient notices:
a checkpoint-recovery notice wins the first startup display, then the warning returns after that notice
is dismissed; the warning does not suppress Agent notification cards. Reload failure keeps current settings and warning record and produces a
localized failure notice. The reload action itself is configurable through `reload_config`. The native default is prefix F5; Herdr uses prefix Shift+R, which conflicts with the existing native Project rename default. A profile such as `reload_config = "prefix+shift+r"` explicitly replaces that conflicting default.

Changed restore-on-start policy is returned in `restartRequired` relative to Session startup;
already-restored sessions remain owned by their current lifetime. Default shell and mode apply to
subsequent preparations without a restart. CLI directory/worktree defaults are caller reads on the
next invocation. This is partial Herdr reload parity: unimplemented
configuration families and Client-specific profile policy remain unfinished.
The wire key profile is capped at 64 KiB; up to 144 expanded triggers per action preserve the user file limit of 16 indexed alternatives. Polling uses an independent cancellable connection every 500 ms, with a two-second exchange limit. A successful reload is published state, not an acknowledgement that every Client has applied it.

No persisted configuration migration or automatic retry is introduced. Deterministic tests cover provisional startup, initial complete application, fatal preservation, atomic settings/diagnostic generations, monotonic later application and warning publication/clearing. The admitted payload includes `themeName`; a theme-only change uses the same publication owner, and Clients resolve their cached palette after complete admission. Connection tests cover propagation without assuming delivery fairness. Authorization, bounded serialization, no-change reload and shutdown remain implementation obligations. The settings commit/publication core has a narrower writer/CAS scope and does not prove this whole reload workflow.

`server config` and `server reload-config` version 8 add the required `diagnostics` object to every
successful snapshot. Text output prints the visible count, exact omitted count and each bounded
field/reason/disposition record. Versions 1 through 7 remain closed and do not accept this member.

## Live defaults for subsequent Windows terminals

Production now reads the [shared device launch profile](../../../crates/workbench-protocol/docs/user-preferences.md)
and scrollback from one confirmed snapshot before each new Host launch. Explicit legacy TOML
shell values seed an unclaimed preference only; once persisted or claimed by Desktop/TUI, later
TOML reloads cannot override that profile. The configuration receipts below describe the admitted
legacy configuration, not the effective shared catalog. The following request-carried override
path remains for compatibility adapters without a shared store.

Reload commits a validated `DefaultShellSelection` into the existing Server TerminalCoordinator. Its creation admission captures that immutable value in each new default `AuthorizedTerminalLaunch`; explicit executable/Profile requests carry no default override. The Worker validates the Windows shell syntax and resolves its own executable/PATH and mode flags before Host handoff. An absent override retains the Worker's startup fallback for unmanaged/legacy private requests. Existing PTYs, prepared launches, exact replay and deferred Start never acquire later defaults. New CLI/TUI/restore creation uses the same coordinator admission.

The commit lock order is configuration registry, Session coordinator, then manifest scheduling. Candidate validation and wire-value construction precede commit; scheduler admission refusal leaves both launch policy and publication unchanged. No Worker IPC or distributed acknowledgement occurs under these locks. Shell/mode changes apply to subsequent creation and cease to require Session restart; restore-on-start remains a startup-only requirement. Successful reload confirms configuration admission, not executable availability.

Launch preparation failure records the existing known pre-handoff failure, retains the Worker/Server connection and creates no Host terminal. The same failed request replays its failure; corrected settings require a fresh creation identity. The lazy loop also remains available before any Host exists. Host startup/transport uncertainty retains its prior retirement and no-retry policy. Evidence covers private framing, default/explicit/absent selection, preparation across reload, exact/conflicting replay, failure retention and real Windows pipe exchanges; full provider/multi-Client/visual acceptance remains deferred. Matching private cohorts deploy or roll back together without a durable migration.

`server config` and reload receipts expose the admitted `defaultShell` and `shellMode` for subsequent default preparations, including after a lost reload reply. Empty shell means the platform default; these values are not Worker executable-resolution or launch-success evidence. CLI receipt v2 requires both fields while the historical v1 schema stays closed and unchanged. Matched private Workbench snapshots require the same bounded literal (4096 UTF-8 bytes, no controls) and `auto`/`login`/`non_login` mode. Human output escapes the literal and localizes the empty-shell label.

## Outer window titles

`[ui] window_title` defaults to `{hostname}: {workspace}` and reloads online. The [title owner](windows-terminal-title.md#configured-templates) defines tokens, literal braces, invalid/empty policy, bounded rendering and CLI override precedence. Invalid syntax, a wrong type or an over-4096-byte value now defaults this leaf and appears in the `config check` v13 warning record; unknown `[ui]` keys are ignored with their own warning. The effective `windowTitle` remains valid and `windowTitleDiagnostic` therefore stays null. `server config`/`server reload-config` v8 expose the effective published template and diagnostics. Prior public versions remain closed.

## Custom-notification configuration

`ui.toast.delivery` accepts off (default), orchbee/herdr, terminal or system for manual CLI notifications; legacy `ui.toast.enabled` supplies off/card only when delivery is absent. `ui.toast.overlay.position` selects the four-corner default. Before common preferences are confirmed, automatic physical-Agent notifications use the separate `ui.toast.agent_delivery` (orchbee by default) and `ui.toast.delay_seconds` (one second by default, 0–3600). `[ui.sound] enabled` is false by default and, with `[ui.sound.agents]`, controls only automatic built-in cues; it does not change explicit `notification show` sound. Clipboard policy remains independent. Startup is Client-local; explicit Session reload publishes the complete bounded notification profile alongside keys, and Clients validate both before applying either. `server config`/reload v7 carry the 16 KiB `notificationProfile`, `config check` v12 exposes manual and automatic policy, and configuration failure v9 adds the automatic delay/sound diagnostics. Earlier public versions remain closed. See the complete [automatic notification contract](windows-notifications.md#automatic-physical-agent-notifications).

Common Agent reminder mode and sound, once confirmed, override those legacy automatic leaves;
manual notification delivery and per-Agent sound overrides retain their separate ownership.

## Windows release installation

`orchbee update [--format text|json]` installs the configured channel. `orchbee channel set <stable|preview> [--format text|json]` validates and atomically saves the channel first, preserving comments and other TOML values, then runs the same updater. A missing explicitly selected file may be created by this writer. Update failure retains the saved preference. Both commands work without a Session; neither starts, stops, replaces or reconnects a running Session.

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

The immutable release also stores the exact admitted `release.json`, reserved outside the inventory. Reuse checks that it has not changed. `status client` v3 binds this manifest to the directory identity and verified cohort before projecting installed release provenance; it never substitutes the current configured preference.

## Windows Agent sidebar layouts

The main Windows TUI applies `ui.sidebar.agents` at startup and after `server reload-config`.
For example:

```toml
[ui.sidebar.agents]
rows = [["state_icon", "state_text"], ["machine", "workspace", "tab"], ["pane", "$summary"], ["terminal_title_stripped"]]
row_gap = 1

[ui.sidebar.agents.rows_by_agent]
claude = [["state_icon", "agent"], [{ token = "$model", fg = "#8bc", rules = [{ contains = "fast", ignore_case = true, bold = true }] }]]
```

An omitted layout uses the one-line title default; an explicit empty layout
keeps the identity/navigation line. Missing or hidden values do not consume rows.
The full [sidebar model](machines.md#agent-sidebar-configuration-model) defines
provider aliases, rules, actual-height windows and unchanged physical authority.
Live `terminal_title`/`terminal_title_stripped` now read the exact physical OSC title.
The raw form retains activity glyphs; the stripped form removes one elevation prefix
and one recognized leading activity glyph. Missing or cleared titles disappear;
`$terminal_title` remains a separate custom value.
`ui.sidebar.spaces` has its own Workspace layout and profile, described below.

`config check` v11 and Session configuration receipts v6 expose required
`agentSidebarProfile` and `spacesSidebarProfile` (closed canonical JSON, at most 32 KiB). Configuration failure
v7 includes `ui.sidebar.agents` and layout/color/token/rule/provider diagnostics.
All prior public versions retain their closed fields. Catalog v123 keeps 105 schemas
and 141 grammar paths. Matched private cohorts carry the new required profile
together; deployment/rollback is by whole cohort, without persistent migration.

## Windows Workspace sidebar layouts

The main sidebar places Workspaces under their Projects and eligible Agents under
their exact Workspace or Standalone owner, with one shared scroll window. The
[sidebar navigation model](tui-shell-layout.md#sidebar-navigation-model) owns
disclosure and keyboard navigation. The Workspace picker remains available by
opening Machines then pressing Tab, or clicking the sidebar range footer.
Row clicks retain complete machine/Session/context identity even when names coincide.
`ui.sidebar.spaces` controls Workspace rows; `ui.sidebar.agents` controls their
Agent children. Agent `row_gap` applies before and between Agent rows; Workspace
`row_gap` applies before the next Context row and at least one row before each group.
A Project's first Workspace sits directly under its heading.

```toml
[ui.sidebar.spaces]
rows = [["state_icon", "workspace"], [{ token = "branch", fg = "#8bc", rules = [{ starts_with = "worktree/", dim = true }] }, "git_status"], ["$issue"]]
row_gap = 0
```

Defaults are `[["state_icon", "workspace", "git_status"], ["location"]]`, gap 0.
`location` shows the branch, otherwise the Workspace folder with a local home folder shortened to
`~`. An explicit `rows = []` keeps an identity/navigation row. The other builtin is
`state_text`; `$name` reads Workspace reports and never replaces a builtin.
Styles and first-match rules share Agent admission, but icons/Git counts reject
text rules and Workspace layouts have no `rows_by_agent`. Limit 16 rows, 16 tokens
per row, 16 rules per text token and `row_gap` 0–65535. Missing/hidden values collapse.
Operational state paints in the row's leading state track, and Git counts right-align at the
end of the line that configures them, flush with the row's trailing edge on the identity line;
other text shares the remaining cells.

State uses the complete physical Agent checkpoint, independent of seen and of list/source
filters. Git reads the committed Workspace directory
on its Worker, showing nonzero ahead/behind against the local upstream. Missing
upstream supplies no counts; nonrepositories supply no Git observation. Detached
has no branch; unborn keeps its branch name. No fetch, configuration write, hook/fsmonitor execution, terminal
startup or per-draw I/O occurs. Custom `$branch`/`$git_status` stay distinct.
See the [Workspace sidebar model](machines.md#workspace-sidebar-model) for fences,
aggregation, budgets, exact activation and acceptance limits.

Configuration failure v8 admits `ui.sidebar.spaces`. Session/Client reload validates
keys, notifications and both sidebar profiles before publishing any of them.
Each profile caps at 32 KiB; the matched-cohort configuration frame caps at 256 KiB
including escaping. Older public CLI versions retain their closed fields.

`terminal.focus_new_panel` accepts a boolean and defaults to `true`. Native Client splits select the newly committed pane; `false` preserves the original pane. The Client reads this preference at startup, so restart it after editing this field. Invalid types produce a field-local diagnostic and use the default. CLI `panel split --focus` retains its explicit command policy.
