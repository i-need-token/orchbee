# Windows CLI configuration

## File and validation

Orchbee reads `config.toml` in its environment's [data root](../../../crates/local-runtime/README.md#data-root), `%USERPROFILE%/.orchbee/config.toml` for release and `%USERPROFILE%/.<stem>/config.toml` with a stem such as `ob-dev-<digest>` for development and test builds, so environments never share a file. `ORCHBEE_HOME` moves the whole data root; the absolute file named by `ORCHBEE_CONFIG_FILE` replaces only the settings file and still wins in every environment. The [environment rules](../../../crates/local-runtime/README.md#windows-named-runtime-identity) list the other locations. The running Session's Server reads it at startup, on reload and when its sampler sees a change; `config check` reads it offline. Creation commands never read it. A missing default file means unchanged defaults. An invalid or empty location variable is fatal to every read, Session startup and Client activation included. A missing explicit file, an unreadable file, a directory or a file larger than 2 MiB is fatal to `config check`, `channel show`, `channel set`, `update` and `config reload`, but never stops Session startup or Client activation, because the person needs a terminal to repair it: the Session starts on the defaults, its settings snapshot reports the file as `unreadable`, edits are refused while the file exists but cannot be read, and the first edit creates a missing file. A file that is not valid UTF-8 or TOML gives the defaults and a `file` warning whose reader result names the line and column; Session startup and Client activation continue on those defaults, while `config check`, `channel show`, `channel set`, `update`, `config reload` and the Server's sampler treat it as fatal and keep what they had. A parsed document is admitted field by field: an invalid supported leaf uses its existing safe default, an invalid profile uses that profile's existing safe default, and unknown fields are ignored. The reader reports every fallback as a warning without echoing values or commands. No configuration directory or file is created by reading or checking it.

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

`orchbee config default` is an offline command that takes no other argument. Argument parsing selects it before any environment read or runtime discovery, and it does not activate a Session. It writes deterministic UTF-8 TOML with one trailing newline to stdout. Every line is a comment, table headers included: a fixed-English reference generated from the settings registry gives each key's purpose, default, allowed values and when a change applies; English keeps this copyable config artifact identical across CLI locales. Uncommenting a header and a line opts into that one setting. Registered Client actions appear as commented binding arrays, including `[]` for actions without a default binding, so uncommenting a row opts into an override while leaving the generated file unchanged preserves the Client registry's evolving defaults. The action rows are rendered from the same default binding registry used by the Client. The TUI Settings page edits this same file. `config check` accepts the generated output without warnings, and Windows CLI parsing produces the same effective settings and bindings as an absent file.

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

Creation reads its defaults from the running Session, not from the file. Right after the Session snapshot read and before any other request, a Workspace, Tab, Panel or Worktree creation that omits its cwd or checkout path reads the current configuration snapshot once (`workbench.configuration` `current`) and applies `newCwd` or `worktreesDirectory`. An explicit cwd or checkout path skips that read, and a plugin panel never applies `newCwd`. A busy reply is retried until the command deadline; a denied, unavailable or Session-mismatch reply stops the command before any effect. Configuration errors therefore belong to the Server: a fatal file refuses Session startup, a file that does not parse starts it on defaults with a `file` warning, a fatal edit keeps the last published snapshot and fails `config reload`, and field fallbacks appear as snapshot diagnostics and the Client warning. A broken file on disk no longer fails creation; creation keeps using the last published values, and without a running Session it fails as `service-unavailable`.

The creation settings govern Windows CLI creation. The legacy explicit empty-Workspace command, TUI direct creation, existing terminals, Server configuration reload and keybindings retain their own contracts; every creation surface takes its default program from the shared launch profile described below.

## Check and correctness

Windows `config check` version 1 extends the `user-config` check with the admitted warning list and omitted count. A warning result reports status `warning`, keeps the overall document successful and exits 0. Fatal file or environment admission reports status `fail` and exits 1. A failing `user-config` check's details are `{field, reason}`; when `reason` is `invalid-toml` or `invalid-utf8`, and only then, they also carry the file's `path` and the 1-based `line` and `column` (counted in characters) where parsing stops, and text output prints that position as `<path>:<line>:<column>` before the localized reason, which names the line to fix. The check reports the selected file, presence and normalized effective settings, and text output ends with a `Config file:` line naming that file as a copyable path; the separate `orchbee.cli.failure.configuration` envelope remains for Session activation, `channel show` and configured-checkout failures, but no longer for file errors during creation. Failed source discovery stops configured-path creation before first-Project setup; no mutation is retried.

`orchbee.cli.failure.configuration` v1 details are one closed object `{field, reason, line?, column?}`. `field` is `file`, an environment variable name such as `ORCHBEE_CONFIG_FILE` or `USERPROFILE`, or a settings key path such as `worktrees.directory`; the schema checks it by pattern, so a new key needs no schema change. `reason` is one token from a closed list: the settings reader's reasons (`invalid-path`, `missing`, `io-error`, `not-file`, `too-large`, `invalid-utf8`, `invalid-toml`, `unknown-key`, `wrong-type`, the field-local value reasons and `source-query-failed`/`source-query-interrupted`) plus the editor's `read-only`, `file-in-use`, `locked` and `changed`. `line` and `column` appear exactly for `invalid-toml` and `invalid-utf8`. The failure kind is `check-failed` and the exit status 1. Text output gives each editor reason and each syntax reason its own localized fix: clear the read-only attribute, close the editor holding the file, try again after another edit, or fix the named line.

The shared Windows settings reader owns bounded file inspection, deterministic field admission and normalization; the creation workflows own precedence and existing effect receipts. Reads have no writer or cached mutable state; the explicit reset operation below owns configuration repair. Core evidence covers malformed and duplicate TOML, field-local fallback, stable bounded warnings, relative/tilde paths, override precedence and slug vectors. Windows public IPC traces cover a published launch cwd and checkout root winning over the file, explicit overrides without the configuration read, failed discovery with no mutation, and file errors that fail `config check` but not creation. Full application acceptance remains a separate gate.

## Rule update policy

`[update] channel` accepts exactly `stable` (default) or `preview`, independently of `agent_detection.auto_update`. `orchbee channel show [--format text|json]`, which a release build [withholds](#windows-release-installation), reads the complete shared configuration without creating files, activating a Session or accessing the network. Text is the channel token; `orchbee.channel.show` v1 reports channel, default/file provenance, selected configuration path and file presence. An absent default file uses stable; an absent explicit file or malformed configuration fails. This is the requested release preference, not evidence about the currently installed cohort. Windows `config check` v1 and configuration failure v1 admit `update.channel` diagnostics.

`[agent_detection] auto_update` is a TOML boolean, default true, with `default`/`file` provenance.
It allows automatic detection-rule checks only when the Server has a catalog from
`ORCHBEE_AGENT_DETECTION_CATALOG_URL`; Orchbee ships no default catalog.
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
reads `config.toml`; `config reload` publishes the rule-check switch with the rest of the
configuration. A fatal file/TOML error retains the previous switch; field warnings apply the admitted
fallback settings.
Turning checks off removes scheduling; turning them back on schedules an immediate check. An
active update returns busy for reload. Other running settings and keybindings are not hot-reloaded
by this command. Explicit updates remain available when automatic checks are disabled. See the
[screen model](../../orchbee-server/docs/agent-screen-model.md#automatic-checks-and-configuration-snapshots).
Release acceptance fixtures must explicitly disable checks when public network access is outside
the test's purpose. A reader predating the current key names ignores the renamed tables with warnings;
user overrides and already committed rule caches are retained.

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

`find` opens [Find in terminal](native-terminal-find.md), default `prefix+f`. A safe direct chord
such as `ctrl+shift+f` works where the host delivers it; an empty array disables the entry. Keys
inside the bar remain fixed.

`new_worktree` and `open_worktree` enter [Worktree launch](native-worktrees.md), default
`prefix+W` and `prefix+O`. Safe direct chords and disabling use the same registry.

`settings` opens [Settings](native-settings.md), default `prefix+f4`. `close_workspace` asks to close
the current Workspace with its existing [confirmation](native-workspace-close.md), default `prefix+D`
(capital D). `rename_panel` and `clear_terminal` are unbound by default and run the Panel menu's
Rename and Clear terminal on the focused Panel, with their menu outcomes. `earlier_layouts` opens the [Earlier layouts picker](native-action-menus.md) and is unbound
by default. `last_panel` steps back to the last focused Panel in any Tab or Workspace
([Last Panel](native-context-selection.md#last-panel)), default `prefix+;`. `focus_panel_left`,
`focus_panel_down`, `focus_panel_up` and `focus_panel_right` default to the prefix followed by the
matching arrow key, then `prefix+h`, `prefix+j`, `prefix+k` and `prefix+l`; help and menus show the
arrow first.

`menu` opens the [native action menu](native-action-menus.md), default `prefix+space`.
The same safe direct chords and explicit disabling apply. Machines keeps `prefix+M`.

`open_notification_target` opens the Agent of the current Agent notification card, default
`prefix+o`, as the [notification contract](windows-notifications.md#opening-the-notification-target)
describes. Safe direct chords and disabling use the same registry.

`resize_mode` enters the [keyboard resize mode](native-keyboard-resize.md), default `prefix+P`.
`resize_panel_left`, `resize_panel_right`, `resize_panel_up` and `resize_panel_down` are unbound
by default and accept the same safe chords. They move the nearest eligible divider five percentage
points without entering the mode; for example, `resize_panel_left = "ctrl+left"`. Mode arrows are
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
`copy_mode`, `find`, `resize_mode`, `resize_panel_left/down/up/right`, and `focus_panel_left/down/up/right` (four fields), `split_vertical`, `split_horizontal`,
`close_panel`, `rename_panel`, `last_panel`, `clear_terminal`, `zoom` and `toggle_sidebar`. They invoke existing native actions;
Workspace means the native Workbench location, vertical split is rightward and horizontal is downward.
Agent previous/next defaults to prefix `{`/`}` and walks every Agent of the stable
[machine Agent list](machines.md#agent-metadata-view-controls-and-navigation) that the active source
view admits, with exact physical target revalidation. The same owning contract defines configured ordinal Agent focus and source-owned view JSON.

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

Windows `config reload` requests a reload in the running Session and does not activate one. Server freezes the startup configuration location,
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
time reloads the file with the same admission, preflight and commit as `config reload`, except
that an unchanged candidate publishes nothing. A file that cannot be read or parsed keeps the last
valid settings: the settings snapshot publishes the file's `unreadable` or `syntax-error` status
over them, and the configuration snapshot stays as it was until the file is valid again; a refused commit retries the same file at the next sample. The first
sample after startup reloads once, so an edit made while the Session started is not missed. Sampled
publications wake every configuration subscriber; they have no reply and show no notice beyond the
configuration-warning summary. `config reload` remains the way to confirm a publication
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
is dismissed; the warning does not suppress Agent notification cards. A snapshot the Client cannot admit keeps current settings and warning record and produces a
localized failure notice. The TUI has no reload action: the Server's sampler publishes file edits, and `config reload` confirms one immediately.

CLI creation applies the directory and worktree defaults published when it runs. Other configuration families and Client-specific profile policy remain unfinished. The wire key profile is capped at 64 KiB; up to 144 expanded triggers per action preserve the user file limit of 16 indexed alternatives. A cancellable state subscription uses the shared Client carrier and publishes the latest committed configuration. Reload remains a one-shot request with a two-second exchange limit; an uncertain reload is not replayed, and a fresh subscription recovers the current state. A successful reload is published state, not an acknowledgement that every Client has applied it.

No persisted configuration migration or automatic retry is introduced. Deterministic tests cover provisional startup, initial complete application, fatal preservation, atomic settings/diagnostic generations, monotonic later application and warning publication/clearing. Connection tests cover propagation without assuming delivery fairness. Authorization, bounded serialization, no-change reload and shutdown remain implementation obligations. The settings commit/publication core has a narrower writer/CAS scope and does not prove this whole reload workflow.

`config reload` version 1 adds the required `diagnostics` object to every
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

`[appearance] window_title` defaults to `{hostname}: {workspace}` and reloads online. The [title owner](windows-terminal-title.md#configured-templates) defines tokens, literal braces, invalid/empty policy, bounded rendering and CLI override precedence. Invalid syntax, a wrong type or an over-4096-byte value now defaults this leaf and appears in the `config check` v1 warning record; unknown `[appearance]` keys are ignored with their own warning. The effective `windowTitle` remains valid and `windowTitleDiagnostic` therefore stays null. `config reload` v1 exposes the effective published template and diagnostics.

## Custom-notification configuration

`notifications.cli_delivery` accepts off (default), orchbee, terminal or system for manual CLI notifications. `notifications.position` selects the four-corner default. `notifications.agent_reminder` and `notifications.sound` govern automatic Agent notifications and sound. `notifications.delay_seconds` supplies the delay (one second by default, 0–3600); `[notifications.sound_by_agent]` supplies per-Agent sound overrides without changing explicit `notification show` sound. Clipboard policy remains independent. Startup is Client-local; explicit Session reload publishes the complete bounded notification profile alongside keys, and Clients validate both before applying either. `config reload` v1 carries the 16 KiB `notificationProfile`, spelled like the file: one `[notifications]` table with `cli_delivery`, `agent_delivery` (profile only), `delay_seconds`, `position` and `sound`, and `[notifications.sound_by_agent]`; a profile that cannot be admitted is refused with field `notifications`. `config check` v1 exposes manual and automatic policy, and configuration failure v1 adds the automatic delay/sound diagnostics. See the complete [automatic notification contract](windows-notifications.md#automatic-physical-agent-notifications).

Common Agent reminder mode and global sound remain authoritative; manual notification delivery
and per-Agent sound overrides retain their separate ownership.

## Windows release installation

`orchbee install [--no-path] [--allow-downgrade] [--format text|json]` installs the signed release directory the running Client sits in (its `release.json` and `release.json.sig`) and makes it the active version, of whichever channel that manifest names; it reads no settings file and needs no Session. A release build verifies with the release keys and uses the release store root; a test build verifies with the test key and uses `%LOCALAPPDATA%\Programs\<stem>`; a development build refuses with `development-build`, exit `4`. A Client that does not sit beside a signed release refuses with `not-release-directory`, exit `4`. Run from a version directory of the store, it verifies and activates that version again. A version lower than the active one is refused with `downgrade-refused`, exit `3`, before anything is copied, unless `--allow-downgrade` is given; a store whose active version is for another platform refuses with `other-architecture`, exit `3`. Unless `--no-path` is given, the completion performs the PATH step below and `pathStep` reports what it did to the user `Path`: `added`; `present` when it already named `<store root>\bin`; or `replaced` when entries naming other folders inside the store root, which the earlier layout added, were removed in the same write. `not-requested` answers `--no-path`, `not-reached` a run that ended before the step completed, and `unavailable` a build without the step. The installing Client classifies the outcome from the user `Path` it read before the completion ran. `orchbee.install` v1 reports `phase`, `channel`, `releaseVersion`, `buildId`, `releasePath`, `reused`, `activated`, `activeBefore`, `pathStep`, `cleanupPending`, `error` and `interrupted`; under `text` a success prints the version, the active Client's path and the PATH line, and a failure writes only its localized diagnostic to standard error.

`orchbee update [--format text|json]` installs the newest version of the configured channel beside the running one and makes it the version that starts next; running terminals, Agents and Sessions keep the files they started from. `orchbee channel set <stable|preview> [--format text|json]` saves the channel first through the shared single-key settings editor, preserving comments and other TOML values, then runs the same updater. A missing file is created holding only the channel; a file that does not parse is never written. A channel that cannot be saved or read reports error `configuration` with the editor's own refusal in `configurationFailure`. Update failure retains the saved preference. Both commands work without a Session; neither starts, stops, replaces or reconnects a running Session. A release build withholds the whole `channel` command tree, `channel show` included, until it is released: help, completion and the `--skill` guide leave it out, and an invocation is refused with `stage: "release"`, `reason: "withheld-in-release"`, `feature: "channel"`, exit `4`, before any configuration read; see [release-withheld surfaces](../README.md#release-withheld-surfaces). `update` is offered in every build; 0.1.0-beta.1 withheld it with `feature: "update"`.

The shared update workflow, signature verification, source/manifest admission, bounded transfer policy, verification, publication, activation decisions and installed-release provenance compile on every host. The channel save and read use the shared settings reader and editor, which run on every host. Named native ports own the update source environment, installation-root admission, no-follow manifest opening, system executable lookup, capsule verification and owned process execution. Capability admission runs before signal registration or any environment, file, network or process effect. A development build refuses with `non-release-build` before saving a channel or touching the installation; a test build updates its own store as its `install` does, trusting only the test key; this check lives only in the CLI entry layer. Below it, the install and activation engine receives the feed source, trusted keys, installation root, state lookup, version probe, completion step, unpack step and loopback switch as values, so in-process tests drive it with the test key and fixture roots. Linux/macOS retain the generic unsupported-platform result (exit 4); they neither save a channel nor perform an update.

A release build reads the Orchbee update feed, `https://orchbee.com/update/{channel}/windows-x86_64/feed.json`, the one compiled address in `src/cli/update/windows.rs`, which may redirect up to five times to another HTTPS host. `ORCHBEE_UPDATE_FEED`, when set and not empty, replaces it with an explicit absolute local feed document or an HTTPS URL without a query, in every build; a development or test build has no compiled feed and reports `source-unconfigured` without the variable, so only a release build ever reads the public feed. `{channel}` expands to the selected token. Every feed must also serve its signature document: the same location with `.sig` appended. A test build alone also admits `http://127.0.0.1:<port>/…` for a loopback server that stands in for the HTTPS host in a rehearsal on one computer, and an archive path relative to it; curl then runs with `--proto =http --proto-redir =http --noproxy 127.0.0.1`. The switch is compiled from the environment stem (`LOOPBACK` in `src/cli/update.rs`), and `Source::configured` refuses a loopback address again in a release build, so no runtime input enables plain HTTP there. A compiled publisher key, not the chosen location, is the trust authority: the feed's signature binds the archive's size and SHA-256 and the manifest's digest, the manifest's own signature is verified again, and per-file SHA-256 checks then bind every installed file to the signed manifest. `ORCHBEE_INSTALL_DIR` names the store root in every environment and defaults to `LOCALAPPDATA/Programs/Orchbee` for release and `LOCALAPPDATA/Programs/<stem>` for a test or development build. Install paths reject PATH delimiters, expansion tokens and controls. One install or update has a 600-second deadline and Ctrl+C cancellation; it covers the feed, the decision, unpacking, verification and activation, and stands still while the archive transfers under its own limit: curl ends a transfer that averages below 4 KiB/s over 60 seconds (`--speed-limit 4096 --speed-time 60`) and, at the latest, after the time the signed size takes at that rate plus 600 seconds, never more than six hours (`--max-time`), so a 25 MB archive may take about 113 minutes on a link that stays above the rate and a stalled server ends within about a minute. Either end reports `timeout`, Ctrl+C ends the transfer at once, and `download.lock` stays held throughout. The feed and its signature keep curl's 600-second limit. HTTPS uses curl 8.4 or newer from the OS system directory with HTTPS-only bounded redirects and bounded downloads. curl starts with a cleared environment holding only `SystemRoot` and whichever of `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` are set, so a proxy reaches the transfer while no curlrc, credential, `http_proxy` or TLS setting does; Windows looks environment names up without regard to case, so `https_proxy` and its lower-case peers arrive under the same three names.

The local source model retains the existing Windows disk-absolute grammar independently of the compiling host: `/` and `\` separate components, repeated separators and `.` follow non-verbatim Windows component semantics, and `..`, UNC, device, verbatim and drive-relative paths are rejected. The original external source limit is 4096 bytes; resolving an admitted relative artifact retains the existing parent/join behavior. The existing source fixture covers these boundaries and compares them with the original Windows standard-library parser and join on Windows.

The feed, `src/cli/update/feed.rs`, is one small signed document per channel and platform that names the newest release and the one archive carrying it. It is closed JSON: `schemaVersion: 1`, `product: "orchbee"`, `channel` (`stable` or `preview`; it must be the selected channel, else `manifest-invalid`), `platform` (this updater's own), `version` and `buildId` (the manifest's rules below), `manifest: {sha256}`, the SHA-256 of the archive's `release.json` and so the identity the pointer records, and `archive: {format, url, size, sha256}`: `format` is `zip` on Windows, `url` an HTTPS URL without a query or a portable relative path resolved beside the feed, `size` from 1 byte to 1 GiB, and `sha256` lowercase. An optional `extensions` object is checked and ignored; any other member is refused. The members, their bounds and the signed bytes are frozen from the first version that ships them, and the repository fixture `src/cli/update/tests/fixtures/feed.json.signed` with its `.sig` is the release tool's written form. From the verified feed and the store alone, before anything large is requested, the update decides in this order: the pointer names the feed's version and manifest, and that version's `release.json` has that digest: up to date; the active version is higher and the command is not `channel set`: up to date; `versions\<version>` holds another manifest: `version-conflict`; it holds the feed's manifest but is not active: that copy is verified again and activated, with no download. Otherwise the update takes the private folder `<store root>\.download\` and its lock `download.lock`, decides again, and downloads the archive into `.download\archive.zip` with `--max-filesize` set to the signed size. Its length and SHA-256 must then equal the signed ones, else `checksum`, a longer body included. Only then does the system `tar.exe` unpack it: an absolute path from the system directory, a cleared environment holding only `SystemRoot`, the working directory `.download\unpacked`, and only the arguments `-x -f ..\archive.zip --strip-components 1`, under the run's Job, deadline and Ctrl+C, so no path of the computer is an argument. `curl` and `tar.exe` run in a Job that ends them when the Client ends, however abruptly, so neither keeps writing into `.download` after a killed run. The archive's one top-level folder is a signed release directory, so the unpacked folder installs exactly as `install` installs one, its manifest signature and every listed file verified again, and its version, channel, build and manifest digest must equal the feed's, else `manifest-invalid`. Nothing in the unpacked folder ever runs: the smoke check runs the staged copy after its digest matched, and a file the manifest does not list is never copied. No `tar.exe`, or curl older than 8.4 for an HTTPS feed, reports `download-unavailable`; a failed unpack reports `storage`; a second update while one holds the lock reports `busy` before requesting the archive. `download.lock` is always taken before `install.lock`, and `install` never takes it. The folder's owner removes `archive.zip` and `unpacked\` after it takes the lock and again on every return, so a crashed run leaves at most one of each for the next run; the folder and its lock file stay. When a child's end is unknown or removal fails, nothing more is removed and `cleanupPending` names the folder. There is no resume or retry: a failed run leaves the versions and the pointer as they were, and the next run starts over.

The updater reads at most 64 KiB of feed, 1 MiB of manifest and 4 KiB of signature document, and verifies a signature before it parses the document it signs. Each signature is Ed25519 (RFC 8032) over a UTF-8 domain, LF, then the exact document bytes: `orchbee-update-feed-v1` for a feed and `orchbee-release-manifest-v2` for a manifest, so a signature of one never verifies as the other. The signature document is closed JSON with 1–4 `signatures`, each exactly `{keyId, algorithm: "ed25519", signature}`, plus an optional `extensions` object; `signature` is padded standard base64 of 64 bytes and `keyId` the first 16 lowercase hexadecimal digits of SHA-256 over the 32-byte public key. Both documents refuse repeated members, unknown members outside `extensions`, and nesting deeper than 32. One entry from a compiled key that passes strict verification admits the document; entries naming other keys are skipped. The refusals are `signing-keys-unavailable` when the build trusts no key, before any source is read; `signature-untrusted` when no entry names a compiled key; `signature-invalid` when a local document is missing, or any document is oversized or malformed or no named key verifies; and `manifest-invalid` for an oversized feed or manifest or one that fails admission after verification. Nothing is downloaded before a feed is admitted, and nothing is staged before a manifest is.

`orchbee` compiles two public key tables in `src/cli/update/signature.rs` and chooses one at compile time from the environment stem: a release build trusts only `RELEASE_KEYS`, and development and test builds trust only `TEST_KEYS`, whose private half is the repository fixture `src/cli/update/tests/fixtures/test-only-signing-key.pem`. No runtime input or test switch changes the choice. `RELEASE_KEYS` holds the key the release workflow signs with and a spare kept offline; their private halves never enter the repository. A build whose table is empty refuses every update with `signing-keys-unavailable`. Adding a key is one `TrustedKey::from_hex("<64 hex digits>"),` line holding the raw 32-byte public key, which `node -e "process.stdout.write(require('node:crypto').createPublicKey(require('node:fs').readFileSync(process.argv[1])).export({format:'der',type:'spki'}).subarray(-32).toString('hex'))" <key.pem>` prints. Rotation adds the successor before any manifest is signed with it and removes a key only after it retires.

A release manifest is closed JSON with `schemaVersion: 2`, `product: "orchbee"`, `channel`, `platform`, `version`, `buildId`, `files`, `components` and an optional `extensions` object. `channel` is 1–32 bytes of `a`–`z`, `0`–`9` and `-`, and must be the selected `stable` or `preview`; `platform` is 1–32 bytes of `a`–`z`, `0`–`9`, `_` and `-`, and this updater accepts only its own platform, such as `windows-x86_64`; `version` is SemVer 2.0.0 without build metadata, at most 64 bytes; `buildId` is 1–96 bytes of ASCII letters, digits, `.`, `_`, `+` and `-`. Each file has exactly `path`, `url`, `sha256` (lowercase), and `size` (bytes). File URLs are HTTPS or portable relative paths resolved beside the manifest. A manifest lists at most 4096 files, 512 MiB per file and 2 GiB total. Portable paths reject traversal, Windows devices, alternate streams, case aliases, file/directory collisions and the reserved `release.json` and `release.json.sig`. Installation requires `cohort.json` and every file it names, and installs every listed file; there is no fixed file list. A `components` entry for `client`, `server`, `worker`, `terminal-host` or `agent-observation-capsule` has exactly `id`, `version`, `files` (paths listed in `files`) and `connections` (each contract name with `generations: {lowest, highest}` within 1–65535), plus optional SemVer `securityFloor` and `recommendedFloor` and `extensions`. Entries with another `id`, connection names this build does not know, and every `extensions` value are ignored, and a missing entry imposes nothing. Every declared file is copied into a private stage and hashed; cohort and capsule digests are admitted before the new Client executes its exact version smoke check. A local source is only read, so its folder needs no owner or access rule: a release directory an elevated installer or npm unpacked, on any drive, installs like one in the person's own folder. Each source file must be a regular file and not a link or junction, or the install stops with `download`.

The store root holds `versions\<version>\`, one immutable signed release directory per installed version with `release.json` and `release.json.sig` unchanged; `active.json`, the pointer; `install.lock`, the writer lock; and `.download\`, the update's download folder above, which the store's own reads never open. Publication stages inside the root and renames the verified stage into `versions\`. An existing version directory holding the same manifest is verified and reused, never overwritten; one holding another manifest is refused with `version-conflict`, exit `3`. Activation is the published copy's own completion: `install` and `update` run that copy's `orchbee --complete-install-v1 [--no-path] [--allow-downgrade]`, which verifies its own signed release, refuses a lower version again unless allowed, performs the PATH step unless `--no-path`, and then replaces `active.json` by a flushed rename of `active.pending` under `install.lock`. A failed completion leaves the pointer where it was; a failure or lost result retains the published path for explicit retry, and a completion that claims success while the pointer names another version is `activation-unknown`. The pointer is closed JSON, `{"version": 1, "active": {"product", "manifest"}, "previous"?, "extensions"?}`: each entry names a version directory and the lowercase SHA-256 of its `release.json`, the version it replaces becomes `previous`, and every rewrite keeps `extensions`. The pointer alone names the active version; a missing or unreadable pointer means none, which an install repairs. The store opens only these names and its own `.stage-*` folders, so a `desktop\` folder beside them is never read; `install` creates a missing root with a private access rule, which a `desktop\` written later inherits, and admits an existing root, such as the one the Desktop installer creates for `desktop\` before it runs the bundled `orchbee install`, only when it is already private. Private is the shared rule of the [local runtime's private storage](../../../crates/local-runtime/README.md#private-storage-on-macos-and-linux): the person's account, SYSTEM or Administrators owns the folder, and every allow entry of its access list names one of the three. An ordinary session meets it with the person as owner; an elevated session of the same account, and an elevated installer a moment earlier, meet it with Administrators as owner, which Windows gives what they create. A folder another account owns, or whose access list lets in another account (Users, Authenticated Users or Everyone included), is refused and never repaired. A `storage` failure adds one localized line after its message on standard error, in text and JSON alike: the folder or file the store refused or failed on, then why, which is another owner, an access list that lets another account in, a link or junction, or the Windows error code. The document keeps its members. Existing processes keep their original files. Older releases are installation state and are retained because they can still be running; automatic retention and live handoff are separate work. Unpublished stages are cleaned after child settlement; unconfirmed settlement or cleanup failure transfers the exact stage path to explicit recovery in the receipt. No old release is deleted as staging cleanup.

`orchbee.update` v1 distinguishes channel saved, release published, activation, reuse, interruption, failure phase/reason and pending cleanup. An update whose verified feed names the version and manifest the pointer already names finishes at once without downloading, hashing, publishing or completing anything: `upToDate` is true, with `reused` and `activated` true and `releasePath` naming the active version; `releaseVersion` and `buildId` come from the feed. A feed offering a version lower than the active one, such as a channel pointed back after a bad release, also finishes with `upToDate` true, nothing copied and the pointer unchanged, never `downgrade-refused`; `channel set` is the person's choice and may move to a lower version. Text output prints `Up to date:` and the active version in both cases. `upToDate` is optional and false otherwise; 0.1.0-beta.1 omits it. `phase` keeps its values: `manifest` covers reading, verifying and deciding from the feed, `download` taking the download folder and fetching, checking and unpacking the archive, and `verify` admitting the unpacked or stored release before `publish`. `configurationFailure` is the configuration failure's closed `{field, reason, line?, column?}` object when `error` is `configuration` and null otherwise; text output prints the same localized fix as the configuration failure envelope. Status is 0 on completion, 1 on known failure, 2 on usage error, 3 on `version-conflict`, `downgrade-refused` or `other-architecture`, 4 on `development-build` or `not-release-directory` (`install` only), 7 on timeout, 8 on unconfirmed physical/activation outcome, and 130 on interruption. A published path can be used directly even when activation needs retry. Full distribution publication, HTTPS source acceptance and real whole-cohort startup acceptance are deployment obligations and are not implied by a local staging test.

To prepare a signed release directory from a completed Windows cohort without publishing it, stage the cohort with `--artifact-stem orchbee` so it carries the release names, then package and sign it:

```powershell
node scripts/stage-local-runtime.mjs --output C:\path\to\cohort --artifact-stem orchbee
node scripts/package-native-windows.mjs C:/path/to/cohort stable 0.1.0 build-id
node scripts/sign-release-manifest.mjs C:/path/to/cohort/release.json --key-env ORCHBEE_RELEASE_SIGNING_KEY
```

An update source is a zip holding that directory as its one top-level folder and the feed document naming it; the release tool writes both, and its test profile names the archive by its bare file name and signs the feed with the test key ([releasing](../../../docs/RELEASING.md)), so a folder holding the feed, its `.sig` and the zip is a complete local feed. A rehearsal of `install` packages a test cohort the same way: stage it with an `ob-test-<16 hex>` stem, package it, sign it with `--key-file` naming the test key fixture, point `ORCHBEE_INSTALL_DIR` at a scratch folder, and run the staged `<stem>.exe install --format json`. The packaging tool refuses a development cohort. A rehearsal of `update` prepares a second test cohort of a higher version the same way, zips it with a test feed beside it, and runs the installed `<store root>\bin\<stem>.exe update --format json` with `ORCHBEE_UPDATE_FEED` naming that feed in its folder or on a loopback server serving the folder, under the isolation below so that the PATH step writes the scratch `Path`.

The packaging tool verifies the existing cohort, adapter capsule and pinned ConPTY bundle, inventories bounded regular files, assigns each component its files and the connection generations of `schemas/connection/v1/connections.json`, and exclusively writes `release.json` beside them. It refuses an existing `release.json` or `release.json.sig`, and a registry contract that no component lists. It writes no floors; a person adds `securityFloor` or `recommendedFloor` before signing. The signing tool takes 1–4 distinct Ed25519 PKCS #8 PEM keys from absolute `--key-file` paths or named `--key-env` variables, refuses anything but a version 2 Orchbee manifest of at most 1 MiB, checks each signature it produces, exclusively writes `release.json.sig`, and never prints key material; the release workflow passes the protected secret `ORCHBEE_RELEASE_SIGNING_KEY` this way. For distribution, the release tool zips that directory, both documents included, as the archive's one top-level folder and writes the feed document naming the zip; the unpacked folder is read file by file at each manifest path, so file URLs are not consulted. Version must match the packaged Client's `--version`; buildId distinguishes releases with the same version.

The npm carrier is written from the signed release directories of one version, one directory per platform: `node scripts/package-npm.mjs --version <version> --output <absolute new directory> [--repository-url https://<host>/<owner>/<name>] <release directory>...`; the repository URL becomes every package's `repository`, which npm provenance compares with the building repository. It copies each directory unchanged, both documents included, under `release/` of a platform sub-package with `os` and `cpu` and no `bin` or script, and writes the main package from `apps/orchbee/npm/orchbee/` with exactly the sub-packages of that run as exact-version `optionalDependencies` and `publishConfig.tag` `next` for a pre-release, `latest` otherwise. A release stem names the packages `orchbee` and `@orchbee/cli-<platform>-<arch>`; a test stem names them `<stem>` and `@<stem>/cli-<platform>-<arch>` with the bin `<stem>`, so a rehearsal never produces a release name. It refuses a version other than the manifests', a second directory for one platform, mixed stems, a file that does not match its manifest digest, a release file npm would leave out, and an existing output directory. A macOS or Linux sub-package keeps the four programs executable and is written only on macOS or Linux, because npm keeps the executable bit it packs and sets one itself only for `bin` entries. The [release workflow](../../../docs/VERIFICATION.md#release-workflow) packs and publishes the folders. The launcher, `bin/orchbee.mjs`, runs on Node 18 or newer (`engines`) and uses only Node built-ins. On each start it maps `process.platform` and `process.arch` to its listed sub-package, reads the version from that package and the stem from its own name, and reads `active.json` in the store root of the `ORCHBEE_INSTALL_DIR` rule above. With no active version or a lower one it runs the carried `<stem>.exe install --no-path --format json`, waits while that answers `busy` for another start, and stops with a localized line when the published path lies outside the root it reads. It then starts `versions\<version>\<stem>.exe` with the same arguments, standard handles, and environment and exits with its status; console Ctrl+C, Ctrl+Break, and close reach the program directly and the launcher only outlives them. A platform without a sub-package gets one line naming the supported ones.

The PATH step runs inside the completion under `install.lock`, before the pointer moves. It copies the version's `<stem>-launcher.exe` to `<store root>\bin\<stem>.exe` when the bytes differ, so a release installs `bin\orchbee.exe`. A launcher already there, running or not, is renamed aside first and renamed back if its replacement cannot be renamed in, so `bin` never ends a step without one; an aside copy a running launcher still holds is removed by a later step. The step then edits the user `Path`, value `Path` of `HKCU\Environment`: `<store root>\bin` goes first when no entry names it, comparing without case, surrounding quotes or trailing separators; entries naming another folder inside the store root are removed in the same write, unless the root is a drive or share root; every other entry stays byte for byte. The value keeps its registry type (`REG_EXPAND_SZ` when absent), is read again just before the write and recomputed if another writer changed it, then flushed and read back. A value that is not text, would pass 32,760 characters, or holds text that cannot be read without loss is refused. The command writes the value and does not broadcast the change: a platform installer broadcasts at its own end (Inno Setup's `ChangesEnvironment`), and a broadcast from the command itself waits for an unsafe-code policy decision. Until then a program started earlier, Explorer included, sees the entry only after it rereads the user environment, so the text result of an `added` or `replaced` step adds one line: open a new terminal, and sign out and back in if the command is not found there. A failure stops the completion before the pointer moves with `path-launcher` or `path-variable` on its standard error; the installing Client reports `activation` and repeats that line. `update` always performs the step.

The launcher, [`apps/orchbee-launcher`](../../orchbee-launcher/README.md), reads `<store root>\active.json` with the shared pointer parser and starts `versions\<product>\<stem>.exe` with the command line after its own name unchanged, the same console, standard handles, environment and working directory, and exits with the Client's exit code. The Client runs in a Job that ends it when the launcher is ended; the Server, Worker and Terminal Host the Client starts are created outside that Job and keep running. A missing or unreadable pointer, or a Client that cannot start, prints one localized line on standard error and exits `1`. A release directory carries the launcher as an inventoried file outside every component, and `cohort.json` does not list it.

The immutable release also stores the exact admitted `release.json` and its signature document, reserved outside the inventory. Reuse checks that `release.json` has not changed. The pointer entry naming the version, with the manifest SHA-256, binds those bytes to the verified installation, so later reads parse them without the signature document. `status client` v1 binds this manifest to a `versions\<version>` directory that the pointer names as active or previous and to the verified cohort before projecting installed release provenance; anything else reads as unknown, and it never substitutes the current configured preference.

The CLI-private `release_store` module owns the store's shape: immutable publication into `versions\<version>` and the pointer's bytes. The pointer's grammar, with the strict JSON reader and version and digest tokens of release documents, is the shared `orchbee-release-records` crate, which the launcher also reads it with. `ReleaseStore` takes `install.lock` through the private cache writer lock, pins installation ancestors, and replaces `active.json` through `active.pending`; readers read the pointer without the lock. A stage borrows that lock, creates only validated portable paths and exclusive private files, and publishes by a no-replace directory rename to its version name. The application owns the pointer's meaning, manifest validation, downloads, cohort admission, smoke checks, completion, the PATH step and retained versions. Explicit cleanup removes only its unpublished stage; unconfirmed child settlement uses `retain_for_recovery` and reports the retained path. `open_release_file` rejects unsafe ancestors, reparse points, an owner or access list outside the private rule, and hard links. Path admission, stage ownership and publication/recovery decisions compile on every host; stage allocation and filesystem mutations use named platform ports, while the existing private-file ports own locks and handle admission. Unix ports return typed Unsupported before filesystem effects and cannot create a successful store or stage. The existing path-admission test runs on every host; native Windows tests exercise writer exclusion, immutable publication, staging cleanup and pointer replacement under the lock.

## Windows release uninstall

`orchbee uninstall [--delete-data] [--confirm] [--format text|json]` removes this environment's installation and needs no Session. It first resolves the store root (the `ORCHBEE_INSTALL_DIR` rule above) and the data root (`ORCHBEE_HOME`, otherwise `%USERPROFILE%\.<stem>`), computes everything it would remove as a plan, and fences it: the whole run is refused, with error `plan` and a `reason`, when a root is relative, spells `.` or `..`, lies less than two folders below a volume, or equals or holds the profile, an application-data, system, program, per-user programs or temporary folder (`unsafe-root`); when the roots overlap (`roots-overlap`); or when a target lies outside both roots (`outside-roots`), above one (`root-parent`), or below a link, junction or unreadable folder inside its root, or a root is itself a link or junction (`through-link`). A target that is itself a link is removed as a link; no removal follows one.

Uninstall removes only what the two layouts name, and only from a root that carries its layout's mark. The store's entries are `bin`, `desktop`, `versions`, `.download`, `install.lock`, `active.pending`, publication stages `.stage-<process>-<number>` and the pointer `active.json`, its mark, which must read as a pointer under the private rule `install` admits the root with, so uninstall recognizes every store `install` accepted, one an elevated session created included. The data root's entries are `config.toml`, `sessions`, `logs`, `agent-detection` and `internal`; its mark is `internal\version.json`, which must read as a layout record of any layout, and every writer that creates the root, Desktop included, writes it in the same step. A root that holds anything the run would remove but no readable mark refuses the run before anything changes, with `not-orchbee-store` or `not-orchbee-data`, naming the folder and `ORCHBEE_INSTALL_DIR` or `ORCHBEE_HOME` when that variable chose it. A missing or empty root, or one that holds nothing of its layout, is already absent. In a store root that carries its pointer, the Desktop installer's own files, `unins000.exe` and `unins000.dat`, are the installer's by its contract with the Client: the [Windows installer](../../desktop/README.md#windows-installer) keeps its uninstaller and uninstall log there under exactly these names. `uninstall` removes them with the store's entries, before the pointer, so the store root goes and nothing of the installer outlives its registration; the helper form below leaves them to the uninstaller that runs it. In any other folder those names are another program's. Anything else found in a root stays, the run still succeeds, and the listing, the final message and the document's `kept` say how many other items each folder keeps; the installer's files are not counted among them. Each mark goes after everything else of its layout, and a root goes last and only as an empty folder, so an interrupted run leaves a root the next run still recognizes. A data root that did not exist when the run began, which the run's own request record creates, goes again once empty.

Run inside a terminal of one of the environment's Sessions it refuses with `inside-session`, since it would end that terminal.

It then lists what goes and what stays: the program folder, the user `Path` entry and the installer entries present, the Agent integrations whose entries are this environment's or cannot be told from files alone, what leaves the data folder in plain words (or the whole data folder with `--delete-data`), how many other items each root keeps, every running Session with its open terminals and running Agents, and the [machine hub](machines.md#client-owner) of the data root by its process id, saying that every remote machine login it holds ends with it. A release build shows the list as the prompt of the typed confirmation, `orchbee`, and refuses `--confirm` and non-interactive callers as every guarded command does; a development or test build requires `--confirm` and prints the list on standard error under `text`. Before anything changes it refuses with exit `3` when a Session another installed version runs has open terminals (`session-unstoppable`) or a program outside every Session and its terminals, such as the Desktop application, runs from the store root (`program-running`). The helper form below does not count the platform uninstaller that started it and waits for it.

The steps then run in this order, each safe to repeat:

1. Every background process of the environment ends: each Session is stopped like `session stop`, one another version runs only when it reports no terminal; then the machine hub of the data root ends through `drain` and, after 10 seconds, `exit`, whether or not the listing named one, and a hub that will not end is left for the wait below. The run then waits until nothing but itself, the launcher and the helper form's waiting uninstaller runs from the store root; a program of a kind it cannot end stops it with `still-running`, naming the program, before step 2.
2. Every Agent integration is disabled through the Agent integration writer's own disable, whether or not anything shows it enabled, in this process. The writer leaves the rest of each Agent's configuration as it was, leaves another environment's entry as it is without stopping, as its [ownership rule](../../../crates/agent-integration-writer/README.md#capsule-location-and-ownership) says, and refuses a hook the person changed (`conflict`); a provider whose state only its program reports and whose program is not on `PATH` is `program-not-found`.
3. The user `Path` entry goes by the exact inverse of the PATH step's edit: entries naming `<store root>\bin`, in any spelling that edit recognizes, and entries naming another folder inside the store root are removed; every other entry stays byte for byte and the value keeps its registry type. A value that held only those entries stays as an empty value of its type, which is how the PATH step finds an account whose `Path` was empty; one that was absent before the install also ends empty, since nothing records that it was absent. Nothing is broadcast. Then the Start menu shortcut, `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Orchbee.lnk`, and the uninstall registration, `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\com.orchbee.desktop_is1`, go; a development or test build uses its stem in both names.
4. `logs\`, `internal\integrations\`, `internal\cache\`, `internal\runtime\` and `internal\tmp\` leave the data root; `config.toml`, `sessions\`, `agent-detection\` and the rest of `internal\` stay. With `--delete-data` the [remembered login answers](machines.md#remembered-answers) of every Machine the catalog names leave the credential store first, then every entry of the data layout goes, `internal\version.json` last, then the data root when nothing else is in it.
5. The store's entries go last, the pointer after all of them, then the store root when nothing else is in it. While any other program runs from the store nothing in it is touched, so a second run can finish. The running Client's file, the launcher waiting for it and their folders cannot be deleted while they run: the command starts a detached `cmd.exe` from the system folder, inheriting no handle and working in the system folder, which once per second for about a minute deletes exactly those files and then their folders, without recursion; once they are gone it deletes the pointer and removes the store root with a plain `rd`, which leaves a root that keeps other items. It ends as soon as those files and the pointer are gone. It leaves no file of Orchbee's and no process behind; interrupted, it leaves the pointer while any of those files remains, so another run recognizes and finishes the store.

A step that fails stops the run: nothing further changes, standard error names the cause, what was done and what remains, and how to continue, naming the program by its full path once the `Path` entry is gone. Started through the npm carrier, whose launcher runs the Client under Node.js, the final message adds `npm rm -g orchbee`, since a later start through the carrier installs again. `orchbee.uninstall` v1 reports `mode`, `caller`, `storeRoot`, `dataRoot`, `sessions`, `integrations`, `pathEntry`, `installerEntries`, `data` (the exact entries), `store` (`scheduled` when the last files go after the command ends), `kept` (each root that keeps other items, with their count), `remaining`, `npmCarrier`, `error`, `reason` and `interrupted`. Status is 0 on completion, 1 for `location`, `plan` or `inventory`, 2 on usage error, 3 when a refusal changed nothing, 4 on another platform, 9 when a step failed, and 130 on interruption.

A platform uninstaller runs `orchbee --installer-uninstall-v1 [--delete-data]` after its own confirmation. It asks nothing, answers in text, and ends no terminal: with one open in any Session of the environment it changes nothing and exits `3` with error `terminals-open` and a line the uninstaller can show. The uninstaller that started it waits in the store root until it exits, so the helper recognizes it by what it is to the helper rather than by name: a process on the helper's own chain of parents, each link started no later than the process it started, whose program lies in the store root outside every entry the removal removes. That process neither stops the run as `program-running` nor keeps the store wait from finishing; any other program in the store root, and any of Orchbee's own programs even on that chain, still stops it as before. The helper leaves the installer's own files, `unins000.exe` and `unins000.dat`, in the store root, since the uninstaller still runs from the one and writes the other; the uninstaller removes them after the helper exits. Whichever of the uninstaller and the detached removal of step 5 finishes last removes the then empty store root: each deletes its own files before it removes the folder, so one of the two always finds it empty.

A test build reads `ORCHBEE_TEST_REGISTRY_KEY`, a key below `HKCU\Software\ob-test-scratch\` with one plain name, and uses its `Environment` and `Uninstall` subkeys instead of the real ones, for uninstall and for the PATH step of `install` and `update`; any other value refuses, with `location` in uninstall and as a failed PATH step before the pointer moves, and no other build reads it. A rehearsal stages, packages and test-signs a cohort as above, then runs every command with its own `USERPROFILE`, `HOME`, `APPDATA`, `LOCALAPPDATA`, `TEMP`, `ORCHBEE_HOME`, `ORCHBEE_INSTALL_DIR`, `ORCHBEE_TEST_REGISTRY_KEY` and a `PATH` holding no Agent program: `install --no-path`, a copy of the version's `<stem>-launcher.exe` to `<store root>\bin\<stem>.exe` and the scratch `Path`, registration and shortcut standing in for the PATH step and the installer, then `<store root>\bin\<stem>.exe uninstall --confirm`. The ignored integration test `cli_windows_uninstall` performs that rehearsal in both modes, with stand-ins for the installer's own files in the store root and one other item in each root under `--delete-data`, and runs the helper from a stand-in uninstaller in the store root beside another program there, when `ORCHBEE_TEST_UNINSTALL_CLIENT` names the packaged Client.

## Windows Agent sidebar layouts

The main Windows TUI applies `appearance.sidebar.agents` at startup and after `config reload`.
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
