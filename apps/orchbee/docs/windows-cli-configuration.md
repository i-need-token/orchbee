# Windows CLI configuration

## File and validation

Orchbee reads `config.toml` in its environment's [data root](../../../crates/local-runtime/README.md#data-root), `%USERPROFILE%/.orchbee/config.toml` for release and `%USERPROFILE%/.<stem>/config.toml` with a stem such as `ob-dev-<digest>` for development and test builds, so environments never share a file. `ORCHBEE_HOME` moves the whole data root; the absolute file named by `ORCHBEE_CONFIG_FILE` replaces only the settings file and still wins in every environment. The [environment rules](../../../crates/local-runtime/README.md#windows-named-runtime-identity) list the other locations. The running Session's Server reads it at startup, on reload and when its sampler sees a change; `config check` reads it offline. Creation commands never read it. A missing default file means unchanged defaults. An invalid or empty location variable is fatal to every read, Session startup and Client activation included. A missing explicit file, an unreadable file, a directory or a file larger than 2 MiB is fatal to `config check`, `channel show`, `channel set`, `update` and `server reload-config`, but never stops Session startup or Client activation, because the person needs a terminal to repair it: the Session starts on the defaults, its settings snapshot reports the file as `unreadable`, edits are refused while the file exists but cannot be read, and the first edit creates a missing file. A file that is not valid UTF-8 or TOML gives the defaults and a `file` warning whose reader result names the line and column; Session startup and Client activation continue on those defaults, while `config check`, `channel show`, `channel set`, `update`, `server reload-config` and the Server's sampler treat it as fatal and keep what they had. A parsed document is admitted field by field: an invalid supported leaf uses its existing safe default, an invalid profile uses that profile's existing safe default, and unknown fields are ignored. The reader reports every fallback as a warning without echoing values or commands. No configuration directory or file is created by reading or checking it.

```toml
[terminal]
new_cwd = "follow"
focus_new_panel = true

[worktrees]
directory = "~/Projects/worktrees"

[update]
channel = "stable"

[agent_detection]
auto_update = true
```

Only registered keys affect Orchbee; the [settings key table](../../../crates/local-runtime/README.md#user-settings) lists every table and key. Unknown sections, fields and actions warn and are ignored, and the retired `[ui]` names are simply unknown: nothing migrates or aliases them; loading configuration never enables a plugin, installs a provider or runs a configured command.

`terminal.new_cwd` accepts `follow`, `home`, `current`, `workspace`, or a path. An empty value means `follow`. Paths use the Windows CLI path adapter: relative paths resolve against the configuration file's parent, tilde paths use Windows home, and ambiguous drive-relative, UNC and device paths are rejected. A path must also be a workbench directory path, so a bare verbatim drive such as `\\?\C:` is invalid; the same rules apply to `worktrees.directory`. An invalid path falls back only that leaf. A missing `USERPROFILE`, with no `ORCHBEE_HOME`, leaves `config check` on defaults, but Session activation then has no data root for the Session's preference scope: it fails as configuration `USERPROFILE`/`missing` (exit 1) before its activation marker commits. An empty or relative `ORCHBEE_HOME` is the configuration failure `ORCHBEE_HOME`/`invalid-path`. An explicit malformed value does not silently fall back.

Warnings use a bounded canonical reason grammar and a closed disposition vocabulary, and carry only a bounded canonical dotted field path. They are deduplicated and sorted by field and reason. A result carries at most 128 warning entries and an exact omitted count, so truncation is always visible. Unknown path segments are escaped and truncated on a UTF-8 boundary; distinct original fields are deduplicated before display truncation.

### Default configuration output

`orchbee --default-config` is a sole offline invocation. Argument parsing selects it before any environment read or runtime discovery, and it does not activate a Session. It writes deterministic UTF-8 TOML with one trailing newline to stdout. Every line is a comment, table headers included: a fixed-English reference generated from the settings registry gives each key's purpose, default, allowed values and when a change applies; English keeps this copyable config artifact identical across CLI locales. Uncommenting a header and a line opts into that one setting. Registered Client actions appear as commented binding arrays, including `[]` for actions without a default binding, so uncommenting a row opts into an override while leaving the generated file unchanged preserves the Client registry's evolving defaults. The action rows are rendered from the same default binding registry used by the Client. The TUI Settings page edits this same file. `config check` accepts the generated output without warnings, and Windows CLI parsing produces the same effective settings and bindings as an absent file.

## Named chrome themes

`appearance.mode` supports Light (`light`), Dark (`dark`) and System (`system`); System is the
default. In the native TUI, System follows the host-default background and uses terminal-native
contrast when no host background is observed. See [native themes](native-themes.md) for polarity, color capabilities, all chrome
consumers, shared reload and rollback.

`appearance.mode`, `appearance.light_theme` and `appearance.dark_theme` are the Session Server's and
the TUI's runtime source of application appearance. A `[theme]` table is an unknown section that warns and is ignored, and neither
`config check` v1 details nor Session configuration v1 snapshots carry a theme field.

## Creation policy

| Input                                           | Result                                                                                                    |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Explicit CLI cwd or worktree path               | Overrides the corresponding valid setting                                                                 |
| Omitted cwd in Workspace, Tab or Panel creation | Apply the Session's published `terminal.new_cwd`, then the existing source-directory policy               |
| Omitted worktree path with a published root     | Query the source repository before any Project or Git mutation; select `root/repository-name/branch-slug` |
| No published root                               | Keep the existing sibling checkout path                                                                   |
| Resume                                          | Use the path and branch already retained in the operation; do not reload settings                         |

Branch slugs lowercase ASCII letters, collapse nonalphanumeric runs to a hyphen, trim hyphens and use `worktree` if empty. Generated branches remain `worktree/<operation-id>` and are fixed in the submitted intent. The read-only repository query uses the same source and trust policy as creation. The Server still rechecks the source and owns every Git and canonical Workspace effect; the query is not a filesystem transaction. Explicit path avoids the extra query.

Creation reads its defaults from the running Session, not from the file. Right after the Session snapshot read and before any other request, a Workspace, Tab, Panel or Worktree creation that omits its cwd or checkout path reads the current configuration snapshot once (`workbench.configuration` `current`) and applies `newCwd` or `worktreesDirectory`. An explicit cwd or checkout path skips that read, and a plugin pane never applies `newCwd`. A busy reply is retried until the command deadline; a denied, unavailable or Session-mismatch reply stops the command before any effect. Configuration errors therefore belong to the Server: a fatal file refuses Session startup, a file that does not parse starts it on defaults with a `file` warning, a fatal edit keeps the last published snapshot and fails `server reload-config`, and field fallbacks appear as snapshot diagnostics and the Client warning. A broken file on disk no longer fails creation; creation keeps using the last published values, and without a running Session it fails as `service-unavailable`.

The creation settings govern Windows CLI creation. The legacy explicit empty-Workspace command, TUI direct creation, existing terminals, Server configuration reload and keybindings retain their own contracts; every creation surface takes its default program from the shared launch profile described below.

## Check and correctness

Windows `config check` version 1 extends the `user-config` check with the admitted warning list and omitted count. A warning result reports status `warning`, keeps the overall document successful and exits 0. Fatal file or environment admission reports status `fail` and exits 1. A failing `user-config` check's details are `{field, reason}`; when `reason` is `invalid-toml` or `invalid-utf8`, and only then, they also carry the file's `path` and the 1-based `line` and `column` (counted in characters) where parsing stops, and text output prints that position as `<path>:<line>:<column>` before the localized reason, which names the line to fix. The check reports the selected file, presence and normalized effective settings, and text output ends with a `Config file:` line naming that file as a copyable path; the separate `orchbee.cli.failure.configuration` envelope remains for Session activation, `channel show` and configured-checkout failures, but no longer for file errors during creation. Failed source discovery stops configured-path creation before first-Project setup; no mutation is retried.

`orchbee.cli.failure.configuration` v1 details are one closed object `{field, reason, line?, column?}`. `field` is `file`, an environment variable name such as `ORCHBEE_CONFIG_FILE` or `USERPROFILE`, or a settings key path such as `worktrees.directory`; the schema checks it by pattern, so a new key needs no schema change. `reason` is one token from a closed list: the settings reader's reasons (`invalid-path`, `missing`, `io-error`, `not-file`, `too-large`, `invalid-utf8`, `invalid-toml`, `unknown-key`, `wrong-type`, the field-local value reasons and `source-query-failed`/`source-query-interrupted`) plus the editor's `read-only`, `file-in-use`, `locked` and `changed`. `line` and `column` appear exactly for `invalid-toml` and `invalid-utf8`. The failure kind is `check-failed` and the exit status 1. Text output gives each editor reason and each syntax reason its own localized fix: clear the read-only attribute, close the editor holding the file, try again after another edit, or fix the named line.

The shared Windows settings reader owns bounded file inspection, deterministic field admission and normalization; the creation workflows own precedence and existing effect receipts. Reads have no writer or cached mutable state; the explicit reset operation below owns configuration repair. Core evidence covers malformed and duplicate TOML, field-local fallback, stable bounded warnings, relative/tilde paths, override precedence and slug vectors. Windows public IPC traces cover a published launch cwd and checkout root winning over the file, explicit overrides without the configuration read, failed discovery with no mutation, and file errors that fail `config check` but not creation. Full application acceptance remains a separate gate.

## Rule update policy

`[update] channel` accepts exactly `stable` (default) or `preview`, independently of `agent_detection.auto_update`. `orchbee channel show [--format text|json]`, which a release build [withholds](#windows-release-installation), reads the complete shared configuration without creating files, activating a Session or accessing the network. Text is the channel token; `orchbee.channel.show` v1 reports channel, default/file provenance, selected configuration path and file presence. An absent default file uses stable; an absent explicit file or malformed configuration fails. This is the requested release preference, not evidence about the currently installed cohort. Windows `config check` v1 and configuration failure v1 admit `update.channel` diagnostics.

`[agent_detection] auto_update` is a TOML boolean, default true, with `default`/`file` provenance.
It allows automatic detection-rule checks only when the Server has a catalog from
`ORCHBEE_AGENT_DETECTION_MANIFEST_CATALOG_URL`; Orchbee ships no default catalog.
Windows `config check` v1 adds `manifestCheck` and `manifestCheckSource`.
Client activation admits the location and passes only its configuration path/explicit-file flag. The
managed Server reads the complete authoritative revision-0 candidate at that frozen location once,
before any process or durable effect, through `read_startup_settings_at`; direct startup resolves the
location through the same settings owner and reads it once with `read_startup_settings`. Neither
read stops on the file itself. Cohorts must match.
`read_user_settings_at` supports bounded explicit reload at the frozen location, retaining
missing-default versus missing-explicit semantics. It does not create configuration files.

Persistent managed release Sessions check rules at startup and every thirty minutes. Debug/test builds
and standalone endpoints disable automatic network work. `server agent-manifests` exposes effective
policy and latest automatic results. `server reload-agent-manifests` reloads rules only and never
reads `config.toml`; `server reload-config` publishes the rule-check switch with the rest of the
configuration. A fatal file/TOML error retains the previous switch; field warnings apply the admitted
fallback settings.
Turning checks off removes scheduling; turning them back on schedules an immediate check. An
active update returns busy for reload. Other running settings and keybindings are not hot-reloaded
by this command. Explicit updates remain available when automatic checks are disabled. See the
[screen model](../../orchbee-server/docs/agent-screen-model.md#automatic-checks-and-configuration-snapshots).
Release acceptance fixtures must explicitly disable checks when public network access is outside
the test's purpose. A reader predating the current key names ignores the renamed tables with warnings;
user overrides and already committed rule caches are retained.

## Reset custom keybindings

Windows `config reset-keys [--format text|json]` is an offline configuration repair operation.
It uses the same file location as `config check`, but a missing file (including an explicit path)
or no top-level `keys` value succeeds without creating or writing anything. It removes `[keys]`,
its nested tables and command arrays, or a safely removable top-level assignment. Other settings,
comments outside the removed tables and line endings remain byte-for-byte. Invalid UTF-8/TOML, files over 2 MiB and edits whose
parsed result differs from the original minus `keys` fail before writing. This allows recovery from
custom keys that the current settings reader does not yet support; it does not validate or repair
other settings or reload a running Client.

The shared Windows settings repair owner pins the private parent directory tree, rejects reparse
points/nonregular or multiply linked files, and reopens the file exclusively before comparing its
bytes with the prepared input. Changed input or a concurrent file holder refuses the edit. The
backup is a copy with the settings file's name in a new private
`internal/backups/<YYYY-MM-DD>-reset-keys` folder of the data root, or `-2`, `-3` and so on when
that day's folder exists, so no earlier backup is overwritten. It is flushed before rewriting the
original through its exclusive handle. The original's file identity
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

## Automatic Windows shell

The default launch profile, `terminal.default_profile.windows` among
`[[terminal.profiles.windows]]`, selects the program for every default terminal launch. The built-in System shell profile resolves
the first compatible `pwsh.exe` in absolute PATH directories, then the absolute
`SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe`. COMSPEC/SHELL and current-directory
lookup do not override this policy, and selection never executes a probe. Existing PowerShell
prompt integration runs after selection. Explicit executables and other launch profiles retain
their exact arguments.

`terminal.default_shell` and `terminal.shell_mode` are retired: the native configuration file
does not select a launch program, so an existing key warns as unknown and is ignored. No private
startup argument, Server admission value or Configuration snapshot field carries a shell.

Automatic PATH candidates require bounded DOS/PE/COFF and optional-header inspection,
nonempty section headers within the file, executable-image/not-DLL flags and compatible
x86/x64/ARM64 machine type. Malformed, missing, inaccessible or incompatible candidates are
skipped. Discovery resolves each candidate's canonical path and excludes images under an ancestor
containing an `AppxManifest.xml` file: Store/AppX activation rejects the Host's atomic Job-list launch.
This selection precedes terminal creation and continues through PATH or the system fallback;
it never retries creation or relaxes Job containment. Explicit launch-profile programs remain literal.
These checks prove a structural candidate, not that its dependencies load. The existing pinned
`winsafe` kernel wrapper supplies system architecture without authored unsafe code.
[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo)
reports emulated architecture for x86/x64 processes on ARM64; native ARM64 preference through an
emulated Worker is not claimed. An explicit launch-profile program
still selects that executable. Real x64 selection and synthetic header/architecture and direct/nested
package-marker partitions cover this implementation; no ARM64 hardware acceptance is claimed.

## Windows Client key configuration

The bounded shared settings reader admits `[keys]` before Client startup. Each Client owns an
immutable binding snapshot until publication; file edits/reset reach running Clients through the Session's configuration sampler or an explicit live reload. The Client resolves implemented
semantic actions against its default registry. User overrides replace that action's defaults and
conflicting default chords. An invalid explicit action remains an explicit disabled override, so its
built-in default cannot silently return; other actions and valid alternatives in the same action remain
available. Unknown actions are ignored with a warning. When explicit actions claim the same chord, the fixed action-registry order selects the
first and reports the later conflict. Diagnostics never retain the original binding or command.

`prefix` accepts one chord. Action values accept a string or at most 16 alternatives. An empty
string or array remains an explicit disabled override and emits a bounded warning. `prefix+` selects the prefix layer; otherwise a safe direct
chord is required. `switch_tab` and `focus_agent` admit `1..9` with modifiers. Agent ordinals follow
the filtered machine Agent list; `focus_agent` is unbound by default. Complete modifiers and uppercase
letters normalize before collision checks. The prefix itself is reserved in both layers. Direct
printable keys, bare editing keys and Ctrl+C/D/Z are refused. Capture and mode ownership precede
direct dispatch; paste and releases never invoke bindings. Double prefix forwards canonical C0
where available, otherwise original bytes, without a hardcoded Ctrl+b fallback.

`copy_mode` enters [keyboard copy mode](native-keyboard-copy.md), default `prefix+U` (capital U). It can use the same safe direct chords or an empty array to disable entry. In-mode motions remain fixed.

`new_worktree` and `open_worktree` enter [Worktree launch](native-worktrees.md), default
`prefix+W` and `prefix+O`. Safe direct chords and disabling use the same registry.

`settings` opens [Settings](native-settings.md), default `prefix+f4`. `close_workspace` asks to close
the current Workspace with its existing [confirmation](native-workspace-close.md), default `prefix+D`
(capital D). `rename_pane`, `clear_terminal` and `edit_scrollback` are unbound by default and run the
Panel menu's Rename, Clear terminal and Edit scrollback on the focused Panel, with their menu
outcomes. `earlier_layouts` opens the [Earlier layouts picker](native-action-menus.md) and is unbound
by default. `last_pane` steps back to the last focused Panel in any Tab or Workspace
([Last Panel](native-context-selection.md#last-panel)), default `prefix+;`.

`menu` opens the [native action menu](native-action-menus.md), default `prefix+f2`.
The same safe direct chords and explicit disabling apply. Machines keeps `prefix+M`.

`open_notification_target` opens the Agent of the current Agent notification card, default
`prefix+o`, as the [notification contract](windows-notifications.md#opening-the-notification-target)
describes. Safe direct chords and disabling use the same registry.

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
unknown fields warn without changing the effective registry. Invalid custom-command configuration disables
configured commands rather than executing a partial or unvalidated command. Navigation-mode mappings,
ordinal Agent/indexed Workspace keys and other unsupported actions remain unfinished. Targeted evidence covers bounds, collisions/default precedence,
ownership, dispatch and discovery; no full regression or visual acceptance is claimed.

The registry's display projection formats the normalized binding independently of accepted
configuration spelling. Help rows and live hints keep ASCII letters lowercase unless the binding
contains Shift; shifted letters display uppercase. Unicode glyph mode uses arrow glyphs and, on
Apple hosts, Apple modifier symbols. Simplified glyph mode spells `Up`, `Down`, `Left`, `Right` and
`Ctrl`, `Alt`, `Shift`, `Super`, including on Apple hosts. A prefix and its member are separated by
one space. These display rules do not change TOML spelling, normalization, matching or dispatch.

Supported action fields are `help`, `settings`, `menu`, `new_worktree`, `open_worktree`, `new_workspace`, `rename_workspace`, `close_workspace`, `workspace_picker`,
`earlier_layouts`, `detach`, `previous_workspace`, `next_workspace`, `previous_agent`, `next_agent`, `focus_agent`, `open_notification_target`, `new_tab`, `rename_tab`, `previous_tab`,
`next_tab`, `move_tab_previous`, `move_tab_next`, `switch_tab`, `close_tab`,
`copy_mode`, `resize_mode`, `resize_pane_left/down/up/right`, and `focus_pane_left/down/up/right` (four fields), `split_vertical`, `split_horizontal`,
`close_pane`, `rename_pane`, `last_pane`, `clear_terminal`, `edit_scrollback`, `zoom`, `toggle_sidebar` and `reload_config`. They invoke existing native actions;
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

Windows `config check` v1 retains `keyPrefix` and `keyOverrides` and adds the bounded warning record;
The machine `keyPrefix`
field keeps its stable platform-independent report spelling (`Ctrl+A` for this example), while TUI
shortcut discovery shows the actual unshifted gesture (`Ctrl+a`). The example lets new Tab replace
the default next-Tab chord; empty `zoom` disables its shortcut. Bindings currently apply to the main
TUI. CLI terminal-stream attach deliberately keeps its independent `Ctrl+b q` escape.

Function keys use the actual xterm F1–F12 sequence numbers (skipping 16 and 22), modified
F1–F4, and kitty F1–F24. Modifier event tails retain press, repeat and release; invalid events or
unrepresentable Hyper/Meta modifier bits never collapse into ordinary shortcuts. Availability of
these keys still depends on the terminal host.

## Live configuration reload

The sidebar profile participates in the same admission/publication transition:
`appearance.sidebar.agents.rows`, `rows_by_agent` and `row_gap` are validated by the shared
Windows settings owner, then published as a bounded canonical profile. A Client
validates keys, notifications and both sidebars before installing any field. A bad
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

The Server also samples the frozen location about once per second. A changed size or modification
time reloads the file with the same admission, preflight and commit as `server reload-config`, except
that an unchanged candidate publishes nothing. A file that cannot be read or parsed keeps the last
valid settings: the settings snapshot publishes the file's `unreadable` or `syntax-error` status
over them, and the configuration snapshot stays as it was until the file is valid again; a refused commit retries the same file at the next sample. The first
sample after startup reloads once, so an edit made while the Session started is not missed. Sampled
publications wake every configuration subscriber; they have no reply and show no notice beyond the
configuration-warning summary. `server reload-config` remains the way to confirm a publication
immediately.

The 2 MiB source limit and each profile's individual bound are admission inputs; the composed
response must also fit the unchanged 256 KiB configuration-response limit. Before installing
revision 0 or applying any live policy, and before every reload commit, Server runs the same exact
serializer preflight over a complete `ConfigurationResult` (`Current` at startup and `Reloaded` on
reload) with fixed-width legal correlation identifiers. An admitted source whose combined escaped
profiles and diagnostics exceed that response bound is rejected without truncating otherwise valid
settings. Startup installs no configuration state or live policy; reload retains the complete prior
snapshot and live policy.

Server commits the supported live manifest-check policy and publishes keys, profiles and the bounded
warning record for connected main-TUI Clients. Publication is not an acknowledgement from every
Client. Each Client owns a cancellable configuration watcher, rejects stale or duplicate revisions and
applies one complete validated snapshot between input packets. Pending prefix state is cleared;
editor/confirmation ownership and user drafts remain intact. A warning snapshot applies its effective
settings and shows a localized summary directing the user to `orchbee config check`; a later clean
snapshot clears that warning. Configuration health is retained independently from transient notices:
a checkpoint-recovery notice wins the first startup display, then the warning returns after that notice
is dismissed; the warning does not suppress Agent notification cards. Reload failure keeps current settings and warning record and produces a
localized failure notice. The reload action itself is configurable through `reload_config`. The native default is prefix F5; prefix Shift+R is the existing native Project rename default. A profile such as `reload_config = "prefix+shift+r"` explicitly replaces that conflicting default.

CLI creation applies the directory and worktree defaults published when it runs. Other configuration families and Client-specific profile policy remain unfinished. The wire key profile is capped at 64 KiB; up to 144 expanded triggers per action preserve the user file limit of 16 indexed alternatives. A cancellable state subscription uses the shared Client carrier and publishes the latest committed configuration. Reload remains a one-shot request with a two-second exchange limit; an uncertain reload is not replayed, and a fresh subscription recovers the current state. A successful reload is published state, not an acknowledgement that every Client has applied it.

No persisted configuration migration or automatic retry is introduced. Deterministic tests cover provisional startup, initial complete application, fatal preservation, atomic settings/diagnostic generations, monotonic later application and warning publication/clearing. Connection tests cover propagation without assuming delivery fairness. Authorization, bounded serialization, no-change reload and shutdown remain implementation obligations. The settings commit/publication core has a narrower writer/CAS scope and does not prove this whole reload workflow.

`server config` and `server reload-config` version 1 add the required `diagnostics` object to every
successful snapshot. Text output prints the visible count, exact omitted count and each bounded
field/reason/disposition record.

## Live defaults for subsequent Windows terminals

The Server reads the default launch profile and scrollback from its last valid settings before each
new Host launch; a file that sets neither uses the built-in defaults. Configuration receipts carry no launch program.

Existing PTYs, prepared launches, exact replay and deferred Start never acquire later defaults.
Configuration reload commits the registry, then manifest scheduling; candidate validation precedes
commit, and scheduler admission refusal leaves publication unchanged. No Worker IPC or distributed
acknowledgement occurs under these locks. Successful reload confirms configuration admission, not
executable availability.

Launch preparation failure records the existing known pre-handoff failure, retains the Worker/Server connection and creates no Host terminal. The same failed request replays its failure; corrected settings require a fresh creation identity. The lazy loop also remains available before any Host exists. Host startup/transport uncertainty retains its prior retirement and no-retry policy. Evidence covers failure retention, exact/conflicting replay and real Windows pipe exchanges; full provider/multi-Client/visual acceptance remains deferred. Matching private cohorts deploy or roll back together without a durable migration.

## Outer window titles

`[appearance] window_title` defaults to `{hostname}: {workspace}` and reloads online. The [title owner](windows-terminal-title.md#configured-templates) defines tokens, literal braces, invalid/empty policy, bounded rendering and CLI override precedence. Invalid syntax, a wrong type or an over-4096-byte value now defaults this leaf and appears in the `config check` v1 warning record; unknown `[appearance]` keys are ignored with their own warning. The effective `windowTitle` remains valid and `windowTitleDiagnostic` therefore stays null. `server config`/`server reload-config` v1 expose the effective published template and diagnostics.

## Custom-notification configuration

`notifications.cli_delivery` accepts off (default), orchbee, terminal or system for manual CLI notifications. `notifications.position` selects the four-corner default. `notifications.agent_reminder` and `notifications.sound` govern automatic Agent notifications and sound. `notifications.delay_seconds` supplies the delay (one second by default, 0–3600); `[notifications.sound_by_agent]` supplies per-Agent sound overrides without changing explicit `notification show` sound. Clipboard policy remains independent. Startup is Client-local; explicit Session reload publishes the complete bounded notification profile alongside keys, and Clients validate both before applying either. `server config`/reload v1 carry the 16 KiB `notificationProfile`, spelled like the file: one `[notifications]` table with `cli_delivery`, `agent_delivery` (profile only), `delay_seconds`, `position` and `sound`, and `[notifications.sound_by_agent]`; a profile that cannot be admitted is refused with field `notifications`. `config check` v1 exposes manual and automatic policy, and configuration failure v1 adds the automatic delay/sound diagnostics. See the complete [automatic notification contract](windows-notifications.md#automatic-physical-agent-notifications).

Common Agent reminder mode and global sound remain authoritative; manual notification delivery
and per-Agent sound overrides retain their separate ownership.

## Windows release installation

`orchbee install [--no-path] [--allow-downgrade] [--format text|json]` installs the signed release directory the running Client sits in (its `release.json` and `release.json.sig`) and makes it the active version, of whichever channel that manifest names; it reads no settings file and needs no Session. A release build verifies with the release keys and uses the release store root; a test build verifies with the test key and uses `%LOCALAPPDATA%\Programs\<stem>`; a development build refuses with `development-build`, exit `4`. A Client that does not sit beside a signed release refuses with `not-release-directory`, exit `4`. Run from a version directory of the store, it verifies and activates that version again. A version lower than the active one is refused with `downgrade-refused`, exit `3`, before anything is copied, unless `--allow-downgrade` is given; a store whose active version is for another platform refuses with `other-architecture`, exit `3`. Unless `--no-path` is given, the completion performs the PATH step below and `pathStep` reports what it did to the user `Path`: `added`; `present` when it already named `<store root>\bin`; or `replaced` when entries naming other folders inside the store root, which the earlier layout added, were removed in the same write. `not-requested` answers `--no-path`, `not-reached` a run that ended before the step completed, and `unavailable` a build without the step. The installing Client classifies the outcome from the user `Path` it read before the completion ran. `orchbee.install` v1 reports `phase`, `channel`, `releaseVersion`, `buildId`, `releasePath`, `reused`, `activated`, `activeBefore`, `pathStep`, `cleanupPending`, `error` and `interrupted`; under `text` a success prints the version, the active Client's path and the PATH line, and a failure writes only its localized diagnostic to standard error.

`orchbee update [--format text|json]` installs the configured channel. `orchbee channel set <stable|preview> [--format text|json]` saves the channel first through the shared single-key settings editor, preserving comments and other TOML values, then runs the same updater. A missing file is created holding only the channel; a file that does not parse is never written. A channel that cannot be saved or read reports error `configuration` with the editor's own refusal in `configurationFailure`. Update failure retains the saved preference. Both commands work without a Session; neither starts, stops, replaces or reconnects a running Session. A release build withholds the whole `update` and `channel` command trees, `channel show` included, until they are released: help, completion and the `--skill` guide leave them out, and an invocation is refused with `stage: "release"`, `reason: "withheld-in-release"`, `feature: "update"` or `"channel"`, exit `4`, before any configuration read; see [release-withheld surfaces](../README.md#release-withheld-surfaces).

The shared update workflow, signature verification, source/manifest admission, bounded transfer policy, verification, publication, activation decisions and installed-release provenance compile on every host. The channel save and read use the shared settings reader and editor, which run on every host. Named native ports own the update source environment, installation-root admission, no-follow manifest opening, system executable lookup, capsule verification and owned process execution. Capability admission runs before signal registration or any environment, file, network or process effect. A development or test build, whose cohort carries an environment artifact stem, refuses with `non-release-build` before saving a channel or touching the installation; this check lives only in the CLI entry layer. Below it, the install and activation engine receives the manifest source, trusted keys, installation root, state lookup, version probe and completion step as values, so in-process tests drive it with the test key and fixture roots. Linux/macOS retain the generic unsupported-platform result (exit 4); they neither save a channel nor perform an update.

Set `ORCHBEE_UPDATE_MANIFEST` to an explicit absolute local release manifest or an HTTPS URL without a query. `{channel}` expands to the selected token. There is no implicit public feed. Every source must also serve the signature document `release.json.sig`: the same location with `.sig` appended. A compiled publisher key, not the chosen location, is the trust authority; per-file SHA-256 checks then bind every installed file to the signed manifest. `ORCHBEE_INSTALL_DIR` names the store root in every environment and defaults to `LOCALAPPDATA/Programs/Orchbee` for release and `LOCALAPPDATA/Programs/<stem>` for a test or development build. Install paths reject PATH delimiters, expansion tokens and controls. Source acquisition has a shared 600-second deadline and Ctrl+C cancellation. HTTPS uses curl 8.4 or newer from the OS system directory with HTTPS-only bounded redirects, no curlrc or inherited proxy/credential environment, and bounded downloads.

The local source model retains the existing Windows disk-absolute grammar independently of the compiling host: `/` and `\` separate components, repeated separators and `.` follow non-verbatim Windows component semantics, and `..`, UNC, device, verbatim and drive-relative paths are rejected. The original external source limit is 4096 bytes; resolving an admitted relative artifact retains the existing parent/join behavior. The existing source fixture covers these boundaries and compares them with the original Windows standard-library parser and join on Windows.

The updater reads at most 1 MiB of manifest and 4 KiB of signature document, and verifies a signature before it parses the manifest. Each signature is Ed25519 (RFC 8032) over UTF-8 `orchbee-release-manifest-v2`, LF, then the exact manifest bytes. The signature document is closed JSON with 1–4 `signatures`, each exactly `{keyId, algorithm: "ed25519", signature}`, plus an optional `extensions` object; `signature` is padded standard base64 of 64 bytes and `keyId` the first 16 lowercase hexadecimal digits of SHA-256 over the 32-byte public key. Both documents refuse repeated members, unknown members outside `extensions`, and nesting deeper than 32. One entry from a compiled key that passes strict verification admits the manifest; entries naming other keys are skipped. The refusals are `signing-keys-unavailable` when the build trusts no key, before any source is read; `signature-untrusted` when no entry names a compiled key; `signature-invalid` when a local document is missing, or any document is oversized or malformed or no named key verifies; and `manifest-invalid` for an oversized manifest or one that fails admission after verification. Nothing is staged before a manifest is admitted.

`orchbee` compiles two public key tables in `src/cli/update/signature.rs` and chooses one at compile time from the environment stem: a release build trusts only `RELEASE_KEYS`, and development and test builds trust only `TEST_KEYS`, whose private half is the repository fixture `src/cli/update/tests/fixtures/test-only-signing-key.pem`. No runtime input or test switch changes the choice. `RELEASE_KEYS` stays empty until the product owner generates the release key outside the repository, so until then a release build refuses every update with `signing-keys-unavailable`. Adding a key is one `TrustedKey::from_hex("<64 hex digits>"),` line holding the raw 32-byte public key, which `node -e "process.stdout.write(require('node:crypto').createPublicKey(require('node:fs').readFileSync(process.argv[1])).export({format:'der',type:'spki'}).subarray(-32).toString('hex'))" <key.pem>` prints. Rotation adds the successor before any manifest is signed with it and removes a key only after it retires.

A release manifest is closed JSON with `schemaVersion: 2`, `product: "orchbee"`, `channel`, `platform`, `version`, `buildId`, `files`, `components` and an optional `extensions` object. `channel` is 1–32 bytes of `a`–`z`, `0`–`9` and `-`, and must be the selected `stable` or `preview`; `platform` is 1–32 bytes of `a`–`z`, `0`–`9`, `_` and `-`, and this updater accepts only its own platform, such as `windows-x86_64`; `version` is SemVer 2.0.0 without build metadata, at most 64 bytes; `buildId` is 1–96 bytes of ASCII letters, digits, `.`, `_`, `+` and `-`. Each file has exactly `path`, `url`, `sha256` (lowercase), and `size` (bytes). File URLs are HTTPS or portable relative paths resolved beside the manifest. A manifest lists at most 4096 files, 512 MiB per file and 2 GiB total. Portable paths reject traversal, Windows devices, alternate streams, case aliases, file/directory collisions and the reserved `release.json` and `release.json.sig`. Installation requires `cohort.json` and every file it names, and installs every listed file; there is no fixed file list. A `components` entry for `client`, `server`, `worker`, `terminal-host` or `agent-observation-capsule` has exactly `id`, `version`, `files` (paths listed in `files`) and `connections` (each contract name with `generations: {lowest, highest}` within 1–65535), plus optional SemVer `securityFloor` and `recommendedFloor` and `extensions`. Entries with another `id`, connection names this build does not know, and every `extensions` value are ignored, and a missing entry imposes nothing. Every declared file is copied into a private stage and hashed; cohort and capsule digests are admitted before the new Client executes its exact version smoke check.

The store root holds `versions\<version>\`, one immutable signed release directory per installed version with `release.json` and `release.json.sig` unchanged; `active.json`, the pointer; and `install.lock`, the writer lock. Publication stages inside the root and renames the verified stage into `versions\`. An existing version directory holding the same manifest is verified and reused, never overwritten; one holding another manifest is refused with `version-conflict`, exit `3`. Activation is the published copy's own completion: `install` and `update` run that copy's `orchbee --complete-install-v1 [--no-path] [--allow-downgrade]`, which verifies its own signed release, refuses a lower version again unless allowed, performs the PATH step unless `--no-path`, and then replaces `active.json` by a flushed rename of `active.pending` under `install.lock`. A failed completion leaves the pointer where it was; a failure or lost result retains the published path for explicit retry, and a completion that claims success while the pointer names another version is `activation-unknown`. The pointer is closed JSON, `{"version": 1, "active": {"product", "manifest"}, "previous"?, "extensions"?}`: each entry names a version directory and the lowercase SHA-256 of its `release.json`, the version it replaces becomes `previous`, and every rewrite keeps `extensions`. The pointer alone names the active version; a missing or unreadable pointer means none, which an install repairs. The store opens only these names and its own `.stage-*` folders, so a `desktop\` folder beside them is never read; `install` creates a missing root with a private access rule, so an installer runs the bundled `orchbee install` before it writes `desktop\`, which then inherits that rule. Existing processes keep their original files. Older releases are installation state and are retained because they can still be running; automatic retention and live handoff are separate work. Unpublished stages are cleaned after child settlement; unconfirmed settlement or cleanup failure transfers the exact stage path to explicit recovery in the receipt. No old release is deleted as staging cleanup.

`orchbee.update` v1 distinguishes channel saved, release published, activation, reuse, interruption, failure phase/reason and pending cleanup. An update refuses a lower version than the active one; `channel set` is the person's choice and may move to a lower version. `configurationFailure` is the configuration failure's closed `{field, reason, line?, column?}` object when `error` is `configuration` and null otherwise; text output prints the same localized fix as the configuration failure envelope. Status is 0 on completion, 1 on known failure, 2 on usage error, 3 on `version-conflict`, `downgrade-refused` or `other-architecture`, 4 on `development-build` or `not-release-directory` (`install` only), 7 on timeout, 8 on unconfirmed physical/activation outcome, and 130 on interruption. A published path can be used directly even when activation needs retry. Full distribution publication, HTTPS source acceptance and real whole-cohort startup acceptance are deployment obligations and are not implied by a local staging test.

To prepare a feed from a completed Windows cohort without publishing it, stage the cohort with `--artifact-stem orchbee` so it carries the release names, then package and sign it:

```powershell
node scripts/stage-local-runtime.mjs --output C:\path\to\cohort --artifact-stem orchbee
node scripts/package-native-windows.mjs C:/path/to/cohort stable 0.1.0 build-id
node scripts/sign-release-manifest.mjs C:/path/to/cohort/release.json --key-env ORCHBEE_RELEASE_SIGNING_KEY
$env:ORCHBEE_UPDATE_MANIFEST = 'C:/path/to/cohort/release.json'
orchbee update --format json
```

A rehearsal of `install` packages a test cohort the same way: stage it with an `ob-test-<16 hex>` stem, package it, sign it with `--key-file` naming the test key fixture, point `ORCHBEE_INSTALL_DIR` at a scratch folder, and run the staged `<stem>.exe install --format json`. The packaging tool refuses a development cohort.

The packaging tool verifies the existing cohort, adapter capsule and pinned ConPTY bundle, inventories bounded regular files, assigns each component its files and the connection generations of `schemas/connection/v1/connections.json`, and exclusively writes `release.json` beside them. It refuses an existing `release.json` or `release.json.sig`, and a registry contract that no component lists. It writes no floors; a person adds `securityFloor` or `recommendedFloor` before signing. The signing tool takes 1–4 distinct Ed25519 PKCS #8 PEM keys from absolute `--key-file` paths or named `--key-env` variables, refuses anything but a version 2 Orchbee manifest of at most 1 MiB, checks each signature it produces, exclusively writes `release.json.sig`, and never prints key material; the release workflow passes the protected secret `ORCHBEE_RELEASE_SIGNING_KEY` this way. For HTTPS distribution, publish that directory, both documents included, at the feed URL; relative file URLs retain the same directory layout. Version must match the packaged Client's `--version`; buildId distinguishes releases with the same version.

The npm carrier is written from the signed release directories of one version, one directory per platform: `node scripts/package-npm.mjs --version <version> --output <absolute new directory> <release directory>...`. It copies each directory unchanged, both documents included, under `release/` of a platform sub-package with `os` and `cpu` and no `bin` or script, and writes the main package from `apps/orchbee/npm/orchbee/` with exactly the sub-packages of that run as exact-version `optionalDependencies` and `publishConfig.tag` `next` for a pre-release, `latest` otherwise. A release stem names the packages `orchbee` and `@orchbee/cli-<platform>-<arch>`; a test stem names them `<stem>` and `@<stem>/cli-<platform>-<arch>` with the bin `<stem>`, so a rehearsal never produces a release name. It refuses a version other than the manifests', a second directory for one platform, mixed stems, a file that does not match its manifest digest, a release file npm would leave out, and an existing output directory. A macOS or Linux sub-package keeps the four programs executable and is written only on macOS or Linux, because npm keeps the executable bit it packs and sets one itself only for `bin` entries. Packing and publishing the folders is the owner's step. The launcher, `bin/orchbee.mjs`, runs on Node 18 or newer (`engines`) and uses only Node built-ins. On each start it maps `process.platform` and `process.arch` to its listed sub-package, reads the version from that package and the stem from its own name, and reads `active.json` in the store root of the `ORCHBEE_INSTALL_DIR` rule above. With no active version or a lower one it runs the carried `<stem>.exe install --no-path --format json`, waits while that answers `busy` for another start, and stops with a localized line when the published path lies outside the root it reads. It then starts `versions\<version>\<stem>.exe` with the same arguments, standard handles, and environment and exits with its status; console Ctrl+C, Ctrl+Break, and close reach the program directly and the launcher only outlives them. A platform without a sub-package gets one line naming the supported ones.

The PATH step runs inside the completion under `install.lock`, before the pointer moves. It copies the version's `<stem>-launcher.exe` to `<store root>\bin\<stem>.exe` when the bytes differ, so a release installs `bin\orchbee.exe`. A launcher already there, running or not, is renamed aside first and renamed back if its replacement cannot be renamed in, so `bin` never ends a step without one; an aside copy a running launcher still holds is removed by a later step. The step then edits the user `Path`, value `Path` of `HKCU\Environment`: `<store root>\bin` goes first when no entry names it, comparing without case, surrounding quotes or trailing separators; entries naming another folder inside the store root are removed in the same write, unless the root is a drive or share root; every other entry stays byte for byte. The value keeps its registry type (`REG_EXPAND_SZ` when absent), is read again just before the write and recomputed if another writer changed it, then flushed and read back. A value that is not text, would pass 32,760 characters, or holds text that cannot be read without loss is refused. The command writes the value and does not broadcast the change: a platform installer broadcasts at its own end (Inno Setup's `ChangesEnvironment`), and a broadcast from the command itself waits for an unsafe-code policy decision. Until then a program started earlier, Explorer included, sees the entry only after it rereads the user environment, so the text result of an `added` or `replaced` step adds one line: open a new terminal, and sign out and back in if the command is not found there. A failure stops the completion before the pointer moves with `path-launcher` or `path-variable` on its standard error; the installing Client reports `activation` and repeats that line. `update` always performs the step.

The launcher, [`apps/orchbee-launcher`](../../orchbee-launcher/README.md), reads `<store root>\active.json` with the shared pointer parser and starts `versions\<product>\<stem>.exe` with the command line after its own name unchanged, the same console, standard handles, environment and working directory, and exits with the Client's exit code. The Client runs in a Job that ends it when the launcher is ended; the Server, Worker and Terminal Host the Client starts are created outside that Job and keep running. A missing or unreadable pointer, or a Client that cannot start, prints one localized line on standard error and exits `1`. A release directory carries the launcher as an inventoried file outside every component, and `cohort.json` does not list it.

The immutable release also stores the exact admitted `release.json` and its signature document, reserved outside the inventory. Reuse checks that `release.json` has not changed. The pointer entry naming the version, with the manifest SHA-256, binds those bytes to the verified installation, so later reads parse them without the signature document. `status client` v1 binds this manifest to a `versions\<version>` directory that the pointer names as active or previous and to the verified cohort before projecting installed release provenance; anything else reads as unknown, and it never substitutes the current configured preference.

The CLI-private `release_store` module owns the store's shape: immutable publication into `versions\<version>` and the pointer's bytes. The pointer's grammar, with the strict JSON reader and version and digest tokens of release documents, is the shared `orchbee-release-records` crate, which the launcher also reads it with. `ReleaseStore` takes `install.lock` through the private cache writer lock, pins installation ancestors, and replaces `active.json` through `active.pending`; readers read the pointer without the lock. A stage borrows that lock, creates only validated portable paths and exclusive private files, and publishes by a no-replace directory rename to its version name. The application owns the pointer's meaning, manifest validation, downloads, cohort admission, smoke checks, completion, the PATH step and retained versions. Explicit cleanup removes only its unpublished stage; unconfirmed child settlement uses `retain_for_recovery` and reports the retained path. `open_release_file` rejects unsafe ancestors, reparse points, foreign ACLs and hard links. Path admission, stage ownership and publication/recovery decisions compile on every host; stage allocation and filesystem mutations use named platform ports, while the existing private-file ports own locks and handle admission. Unix ports return typed Unsupported before filesystem effects and cannot create a successful store or stage. The existing path-admission test runs on every host; native Windows tests exercise writer exclusion, immutable publication, staging cleanup and pointer replacement under the lock.

## Windows Agent sidebar layouts

The main Windows TUI applies `appearance.sidebar.agents` at startup and after `server reload-config`.
For example:

```toml
[appearance.sidebar.agents]
rows = [["state_icon", "state_text"], ["machine", "workspace", "tab"], ["panel", "$summary"], ["terminal_title_stripped"]]
row_gap = 1

[appearance.sidebar.agents.rows_by_agent]
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
`appearance.sidebar.workspaces` has its own Workspace layout and profile, described below.

`config check` v1 and Session configuration receipts v1 expose required
`agentSidebarProfile` and `spacesSidebarProfile` (closed canonical JSON, at most 32 KiB). Configuration failure
v1 includes the Agent layout field and layout/color/token/rule/provider diagnostics; diagnostics now
name it `appearance.sidebar.agents`.
The catalog lists the current documents and command paths. Matched private cohorts carry the new required profile
together; deployment/rollback is by whole cohort, without persistent migration.

## Windows Workspace sidebar layouts

The main sidebar places Workspaces under their Projects and eligible Agents under
their exact Workspace owner, with one shared scroll window. The
[sidebar navigation model](tui-shell-layout.md#sidebar-navigation-model) owns
disclosure and keyboard navigation. The Workspace picker remains available by
opening Machines then pressing Tab.
Row clicks retain complete machine/Session/context identity even when names coincide.
`appearance.sidebar.workspaces` controls Workspace rows; `appearance.sidebar.agents` controls their
Agent children. Agent `row_gap` applies before and between Agent rows; Workspace
`row_gap` applies before the next Context row and at least one row before each group.
A Project's first Workspace sits directly under its heading.

```toml
[appearance.sidebar.workspaces]
rows = [["workspace"], [{ token = "branch", fg = "#8bc", rules = [{ starts_with = "worktree/", dim = true }] }, "git_status"], ["$issue"]]
row_gap = 0
```

Defaults are `[["workspace", "git_status"], ["location"]]`, gap 0.
`location` shows the branch, otherwise the Workspace folder, the first terminal's current
directory when Server recorded one, with a local home folder shortened to `~`. An explicit `rows = []` keeps an identity/navigation row. The other builtin is
`state_text`; `$name` reads Workspace reports and never replaces a builtin. `state_icon` remains
accepted for compatibility but paints nothing on a Workspace row.
Styles and first-match rules share Agent admission, but icons/Git counts reject
text rules and Workspace layouts have no `rows_by_agent`. Limit 16 rows, 16 tokens
per row, 16 rules per text token and `row_gap` 0–65535. Missing/hidden values collapse.
Git counts right-align at the end of the line that configures them, flush with the row's
trailing edge on the identity line; other text shares the remaining cells. A folded Workspace
replaces its second configured line with its Agents' status line.

State text and the folded status line use the complete physical Agent checkpoint, independent of
seen and of list/source filters. Git reads the committed Workspace directory
on its Worker, showing nonzero ahead/behind against the local upstream. Missing
upstream supplies no counts; nonrepositories supply no Git observation. Detached
has no branch; unborn keeps its branch name. No fetch, configuration write, hook/fsmonitor execution, terminal
startup or per-draw I/O occurs. Custom `$branch`/`$git_status` stay distinct.
See the [Workspace sidebar model](machines.md#workspace-sidebar-model) for fences,
aggregation, budgets, exact activation and acceptance limits.

Workspace layout diagnostics name `appearance.sidebar.workspaces`. Session/Client reload validates
keys, notifications and both sidebar profiles before publishing any of them.
Each profile caps at 32 KiB; the matched-cohort configuration frame caps at 256 KiB
including escaping.

`terminal.focus_new_panel` accepts a boolean and defaults to `true`. Native Client splits select the newly committed pane; `false` preserves the original pane. The Client applies this setting from each published configuration snapshot, so the next split after a publication uses it. Invalid types produce a field-local diagnostic and use the default. CLI `panel split --focus` retains its explicit command policy.
