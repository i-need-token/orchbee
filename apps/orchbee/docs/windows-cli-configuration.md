# Windows CLI configuration

## File and validation

Orchbee reads `config.toml` in its environment's [data
root](../../../crates/local-runtime/README.md#data-root), `%USERPROFILE%/.orchbee/config.toml` for
release and `%USERPROFILE%/.<stem>/config.toml` with a stem such as `ob-dev-<digest>` for
development and test builds, so environments never share a file. `ORCHBEE_HOME` moves the whole data
root; the absolute file named by `ORCHBEE_CONFIG_FILE` replaces only the settings file and still
wins in every environment. The [environment
rules](../../../crates/local-runtime/README.md#windows-named-runtime-identity) list the other
locations. The running Session's Server reads it at startup, on reload and when its sampler sees a
change; `config check` reads it offline. Creation commands never read it. An invalid or empty
location variable is fatal to every read, Session startup and Client activation included. A missing
explicit file, an unreadable file, a directory or a file larger than 2 MiB is fatal to `config
check`, `channel show`, `channel set`, `update` and `config reload`, but never stops Session startup
or Client activation, because the person needs a terminal to repair it: the Session starts on the
defaults, its settings snapshot reports the file as `unreadable`, edits are refused while the file
exists but cannot be read, and the first edit creates a missing file. A file that is not valid UTF-8
or TOML gives the defaults and a `file` warning whose reader result names the line and column;
Session startup and Client activation continue on those defaults, while `config check`, `channel
show`, `channel set`, `update`, `config reload` and the Server's sampler treat it as fatal and keep
what they had. A parsed document is admitted field by field: an invalid supported leaf uses its
existing safe default, an invalid profile uses that profile's existing safe default, and unknown
fields are ignored. The reader reports every fallback as a warning without echoing values or
commands. No configuration directory or file is created by reading or checking it.

```toml
[terminal]
new_cwd = "follow"

[worktrees]
directory = "~/Projects/worktrees"

[update]
channel = "stable"
```

Only registered keys affect Orchbee; the [settings key table](../../../crates/local-runtime/README.md#user-settings) lists every table and key. Unknown sections, fields and actions warn and are ignored, and the retired `[ui]` names are simply unknown: nothing migrates or aliases them; loading configuration never enables a plugin, installs a provider or runs a configured command.

Paths use the Windows CLI path adapter: relative paths resolve against the configuration file's
parent, tilde paths use Windows home, and ambiguous drive-relative, UNC and device paths are
rejected. A path must also be a workbench directory path, so a bare verbatim drive such as `\\?\C:`
is invalid; the same rules apply to `worktrees.directory`. A missing `USERPROFILE`, with no
`ORCHBEE_HOME`, leaves `config check` on defaults, but Session activation then has no data root for
the Session's preference scope: it fails as configuration `USERPROFILE`/`missing` (exit 1) before
its activation marker commits. An explicit malformed value does not silently fall back.

Warnings use a bounded canonical reason grammar and a closed disposition vocabulary, and carry only
a bounded canonical dotted field path. A result carries at most 128 warning entries and an exact
omitted count, so truncation is always visible.

### Default configuration output

`orchbee config default` is an offline command that takes no other argument. Argument parsing
selects it before any environment read or runtime discovery, and it does not activate a Session. It
writes deterministic UTF-8 TOML with one trailing newline to stdout. Every line is a comment, table
headers included: a fixed-English reference generated from the settings registry gives each key's
purpose, default, allowed values and when a change applies; English keeps this copyable config
artifact identical across CLI locales. Uncommenting a header and a line opts into that one setting.
Registered Client actions appear as commented binding arrays, including `[]` for actions without a
default binding, so uncommenting a row opts into an override while leaving the generated file
unchanged preserves the Client registry's evolving defaults. The TUI Settings page edits this same
file. `config check` accepts the generated output without warnings, and Windows CLI parsing produces
the same effective settings and bindings as an absent file.

## Named chrome themes

In the native TUI, System follows the host-default background and uses terminal-native contrast when
no host background is observed. See [native themes](native-themes.md) for polarity, color
capabilities, all chrome consumers, shared reload and rollback.

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

Generated branches remain `worktree/<operation-id>` and are fixed in the submitted intent. The
read-only repository query uses the same source and trust policy as creation. The Server still
rechecks the source and owns every Git and canonical Workspace effect; the query is not a filesystem
transaction.

Creation reads its defaults from the running Session, not from the file. Right after the Session snapshot read and before any other request, a Workspace, Tab, Panel or Worktree creation that omits its cwd or checkout path reads the current configuration snapshot once (`workbench.configuration` `current`) and applies `newCwd` or `worktreesDirectory`. An explicit cwd or checkout path skips that read, and a plugin panel never applies `newCwd`. A busy reply is retried until the command deadline; a denied, unavailable or Session-mismatch reply stops the command before any effect. Configuration errors therefore belong to the Server: a fatal file refuses Session startup, a file that does not parse starts it on defaults with a `file` warning, a fatal edit keeps the last published snapshot and fails `config reload`, and field fallbacks appear as snapshot diagnostics and the Client warning. A broken file on disk no longer fails creation; creation keeps using the last published values, and without a running Session it fails as `service-unavailable`.

The creation settings govern Windows CLI creation. The legacy explicit empty-Workspace command, TUI direct creation, existing terminals, Server configuration reload and keybindings retain their own contracts; every creation surface takes its default program from the shared launch profile described below.

## Check and correctness

Failed source discovery stops configured-path creation before first-Project setup; no mutation is
retried.

`orchbee.cli.failure.configuration` v1 details are one closed object `{field, reason, line?,
column?}`. `field` is `file`, an environment variable name such as `ORCHBEE_CONFIG_FILE` or
`USERPROFILE`, or a settings key path such as `worktrees.directory`; the schema checks it by
pattern, so a new key needs no schema change.

The shared Windows settings reader owns bounded file inspection, deterministic field admission and
normalization; the creation workflows own precedence and existing effect receipts. Reads have no
writer or cached mutable state.

## Rule update policy

`[update] channel` accepts exactly `stable` (default) or `preview`. `orchbee channel show [--format
text|json]`, which a release build [withholds](#windows-release-installation), reads the complete
shared configuration without creating files, activating a Session or accessing the network. An
absent default file uses stable; an absent explicit file or malformed configuration fails. This is
the requested release preference, not evidence about the currently installed cohort.

Automatic detection-rule checks have no setting. They run only when the Server has a catalog from
`ORCHBEE_AGENT_DETECTION_CATALOG_URL`; Orchbee ships no default catalog. Client activation admits
the location and passes only its configuration path/explicit-file flag. It does not create
configuration files.

Persistent managed release Sessions with a catalog check rules at startup and every thirty minutes.
Debug/test builds and standalone endpoints disable automatic network work. `server agent-manifests`
exposes effective policy and latest automatic results. `server reload-agent-manifests` reloads rules
only and never reads `config.toml`, and `config reload` never changes rule checks. Explicit updates
remain available whether or not automatic checks run. See the [screen
model](../../orchbee-server/docs/agent-screen-model.md#automatic-checks).

## Automatic Windows shell

The default launch profile, `terminal.default_profile.windows` among
`[[terminal.profiles.windows]]`, selects the program for every default terminal launch. The built-in System shell profile resolves
the first compatible `pwsh.exe` in absolute PATH directories, then the absolute
`SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe`. COMSPEC/SHELL and current-directory
lookup do not override this policy, and selection never executes a probe. Existing PowerShell
prompt integration runs after selection. Explicit executables and other launch profiles retain
their exact arguments.

No private startup argument, Server admission value or Configuration snapshot field carries a shell.

Automatic PATH candidates require bounded DOS/PE/COFF and optional-header inspection, nonempty
section headers within the file, executable-image/not-DLL flags and compatible x86/x64/ARM64 machine
type. Malformed, missing, inaccessible or incompatible candidates are skipped. Discovery resolves
each candidate's canonical path and excludes images under an ancestor containing an
`AppxManifest.xml` file: Store/AppX activation rejects the Host's atomic Job-list launch. This
selection precedes terminal creation and continues through PATH or the system fallback; it never
retries creation or relaxes Job containment. Explicit launch-profile programs remain literal. These
checks prove a structural candidate, not that its dependencies load. The existing pinned `winsafe`
kernel wrapper supplies system architecture without authored unsafe code.
[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo)
reports emulated architecture for x86/x64 processes on ARM64; native ARM64 preference through an
emulated Worker is not claimed.

## Windows Client key configuration

The bounded shared settings reader admits `[keys]` before Client startup. Each Client owns an
immutable binding snapshot until publication; file edits/reset reach running Clients through the Session's configuration sampler or an explicit live reload. The Client resolves implemented
semantic actions against its default registry. User overrides replace that action's defaults and
conflicting default chords. An invalid explicit action remains an explicit disabled override, so its
built-in default cannot silently return; other actions and valid alternatives in the same action remain
available. Unknown actions are ignored with a warning. When explicit actions claim the same chord, the fixed action-registry order selects the
first and reports the later conflict. Diagnostics never retain the original binding or command.

An empty string or array remains an explicit disabled override and emits a bounded warning.
`prefix+` selects the prefix layer; otherwise a safe direct chord is required. Complete modifiers
and uppercase letters normalize before collision checks. The prefix itself is reserved in both
layers. Direct printable keys, bare editing keys and Ctrl+C/D/Z are refused. Capture and mode
ownership precede direct dispatch; paste and releases never invoke bindings. Double prefix forwards
canonical C0 where available, otherwise original bytes, without a hardcoded Ctrl+b fallback.

The menu is also where keys are found; there is no separate help action, and a former `help` key is
an unknown action.

There is no resize mode, and a former `resize_mode` key is an unknown action. Direct resize bindings
require a modifier or a function key; bare editing keys stay with the pane.

The resolved registry supplies dispatch, the keys action-menu rows show and principal live hints. A
menu row shows no key for an action with no effective binding. Invalid custom-command configuration
disables configured commands rather than executing a partial or unvalidated command.

The registry's display projection formats the normalized binding independently of accepted
configuration spelling. These display rules do not change TOML spelling, normalization, matching or
dispatch.

Action fields invoke existing native actions; Workspace means the native Workbench location, vertical split
is rightward and horizontal is downward. Agent previous/next defaults to prefix `{`/`}` and walks
every Agent of the stable [machine Agent
list](machines.md#agent-metadata-view-controls-and-navigation) that the active source view admits,
with exact physical target revalidation. The same owning contract defines configured ordinal Agent
focus and source-owned view JSON.

```toml
[keys]
prefix = "ctrl+a"
menu = ["prefix+space", "f1"]
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

The Workspace sidebar profile participates in the same admission/publication transition:
`appearance.sidebar.workspaces.rows` and `row_gap` are validated by the shared Windows settings
owner, then published as a bounded canonical profile. A Client validates keys, notifications and the
Workspace sidebar before installing any field. A bad private wire profile leaves the whole previous
Client configuration intact. Startup applies the Server's complete admitted baseline after the first
snapshot arrives; a local projection shown while connecting is provisional.

`config reload` requests a reload in the running Session and does not activate one. Server freezes the startup configuration location,
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

CLI creation applies the directory and worktree defaults published when it runs. The wire key
profile is capped at 64 KiB; up to 144 expanded triggers per action preserve the user file limit of
16 indexed alternatives. A cancellable state subscription uses the shared Client carrier and
publishes the latest committed configuration. Reload remains a one-shot request with a two-second
exchange limit; an uncertain reload is not replayed, and a fresh subscription recovers the current
state.

No persisted configuration migration or automatic retry is introduced.

## Live defaults for subsequent Windows terminals

The Server reads the default launch profile and scrollback from its last valid settings before each
new Host launch; a file that sets neither uses the built-in defaults. Configuration receipts carry no launch program.

Existing PTYs, prepared launches, exact replay and deferred Start never acquire later defaults.
Configuration reload commits the registry, then manifest scheduling; candidate validation precedes
commit, and scheduler admission refusal leaves publication unchanged. No Worker IPC or distributed
acknowledgement occurs under these locks. Successful reload confirms configuration admission, not
executable availability.

Launch preparation failure records the existing known pre-handoff failure, retains the Worker/Server
connection and creates no Host terminal. The same failed request replays its failure; corrected
settings require a fresh creation identity. Host startup/transport uncertainty retains its prior
retirement and no-retry policy. Matching private cohorts deploy or roll back together without a
durable migration.

## Outer window titles

`[appearance] window_title` defaults to `{hostname}: {workspace}` and reloads online. The [title
owner](windows-terminal-title.md#configured-templates) defines tokens, literal braces, invalid/empty
policy, bounded rendering and CLI override precedence.

## Custom-notification configuration

`notifications.cli_delivery` accepts off (default), orchbee, terminal or system for manual CLI
notifications. Clipboard policy remains independent. Startup is Client-local; explicit Session
reload publishes the complete bounded notification profile alongside keys, and Clients validate both
before applying either. See the complete [automatic notification
contract](windows-notifications.md#automatic-physical-agent-notifications).

Common Agent reminder mode and global sound remain authoritative; manual notification delivery
and per-Agent sound overrides retain their separate ownership.

## Windows release installation

`orchbee install [--no-path] [--allow-downgrade] [--format text|json]` installs the signed release
directory the running Client sits in (its `release.json` and `release.json.sig`) and makes it the
active version, of whichever channel that manifest names; it reads no settings file and needs no
Session. A release build verifies with the release keys and uses the release store root; a test
build verifies with the test key and uses `%LOCALAPPDATA%\Programs\<stem>`; a development build
refuses with `development-build`, exit `4`. Run from a version directory of the store, it verifies
and activates that version again. A version lower than the active one is refused with
`downgrade-refused`, exit `3`, before anything is copied, unless `--allow-downgrade` is given; a
store whose active version is for another platform refuses with `other-architecture`, exit `3`.
Where the automatic-update switch is on, a version the store holds after it [fell back from
it](../../../crates/local-runtime/README.md#fallback-after-a-failed-move) is refused with
`version-held`, exit `3`, before anything is copied, even with `--allow-downgrade`, until a higher
version has become active. `--no-path` is recorded in the pointer as `extensions.noPath`, so a later
`update` keeps the PATH step off for that store; an install with the PATH step clears the record.

`orchbee update [--format text|json]` installs the newest version of the configured channel beside
the running one and makes it the version that starts next; running terminals, Agents and Sessions
keep the files they started from. `orchbee channel set <stable|preview> [--format text|json]` saves
the channel first through the shared single-key settings editor, preserving comments and other TOML
values, then runs the same updater. A missing file is created holding only the channel; a file that
does not parse is never written. Update failure retains the saved preference. Both commands work
without a Session; neither starts, stops, replaces or reconnects a running Session. A release build
withholds the whole `channel` command tree, `channel show` included, until it is released: help,
completion and the `--skill` guide leave it out, and an invocation is refused with `stage:
"release"`, `reason: "withheld-in-release"`, `feature: "channel"`, exit `4`, before any
configuration read; see [release-withheld surfaces](../README.md#release-withheld-surfaces).

Capability admission runs before signal registration or any environment, file, network or process
effect. A development build refuses with `non-release-build` before saving a channel or touching the
installation; a test build updates its own store as its `install` does, trusting only the test key;
this check lives only in the CLI entry layer. Below it, the install and activation engine receives
the feed source, trusted keys, installation root, state lookup, version probe, completion step,
unpack step and loopback switch as values, so in-process tests drive it with the test key and
fixture roots. macOS and Linux run `update`, the automatic helper, `install` and `uninstall`, as
[macOS and Linux release installation](#macos-and-linux-release-installation) describes.

A release build reads the Orchbee update feed,
`https://orchbee.com/update/{channel}/{platform}/feed.json` with this updater's platform, such as
`windows-x86_64` or `macos-aarch64`, the one compiled address in `src/cli/update.rs`, which may
redirect up to five times to another HTTPS host. A feed address its server does not know, which the
public feed answers with 404 for a platform or channel without a release, ends the run as
`no-release`, exit `4`, in phase `manifest`, before anything else is requested; its text names the
channel and the platform. curl reports the last response's status on standard error (`--write-out`),
and only a 404 with curl's `--fail` exit `22` reads this way; a missing local feed stays `download`,
and a missing signature document or archive keeps its own refusal. `ORCHBEE_UPDATE_FEED`, when set
and not empty, replaces it with an explicit absolute local feed document or an HTTPS URL without a
query, in every build; a development or test build has no compiled feed and reports
`source-unconfigured` without the variable, so only a release build ever reads the public feed.
Every feed must also serve its signature document: the same location with `.sig` appended. A test
build alone also admits `http://127.0.0.1:<port>/…` for a loopback server that stands in for the
HTTPS host in a rehearsal on one computer, and an archive path relative to it; curl then runs with
`--proto =http --proto-redir =http --noproxy 127.0.0.1`. The switch is compiled from the environment
stem (`LOOPBACK` in `src/cli/update.rs`), and `Source::configured` refuses a loopback address again
in a release build, so no runtime input enables plain HTTP there. A compiled publisher key, not the
chosen location, is the trust authority: the feed's signature binds the archive's size and SHA-256
and the manifest's digest, the manifest's own signature is verified again, and per-file SHA-256
checks then bind every installed file to the signed manifest. `ORCHBEE_INSTALL_DIR` names the store
root in every environment and defaults to `LOCALAPPDATA/Programs/Orchbee` for release and
`LOCALAPPDATA/Programs/<stem>` for a test or development build. Install paths reject PATH
delimiters, expansion tokens and controls. One install or update has a 600-second deadline and
Ctrl+C cancellation; it covers the feed, the decision, unpacking, verification and activation, and
stands still while the archive transfers under its own limit: curl ends a transfer that averages
below 4 KiB/s over 60 seconds (`--speed-limit 4096 --speed-time 60`) and, at the latest, after the
time the signed size takes at that rate plus 600 seconds, never more than six hours (`--max-time`),
so a 25 MB archive may take about 112 minutes on a link that stays above the rate and a stalled
server ends within about a minute. Either end reports `timeout`, Ctrl+C ends the transfer at once,
and `download.lock` stays held throughout. Before the transfer, `update` runs the system tar with
`--version` on every platform: a missing tar is refused as `download-unavailable`, any other failure
of that check as `download`, and nothing is downloaded. On Windows it also refuses as
`install-location`, before any transfer or change, a path that CreateProcess would not start: the
absolute unpack folder `<store root>\.download\unpacked`, where tar starts, must be at most 258
UTF-16 units, and the absolute path of each program under `<store root>\versions\<version>\` at most
259 units. An unpack that is cancelled or times out is settled before its files are removed; one
whose end cannot be confirmed (`physical-unknown`, exit `8`) keeps the archive and `unpacked\`
beside the lock, names the folder in `cleanupPending`, and the next update removes them. HTTPS uses
curl 8.4 or newer from the OS system directory with HTTPS-only bounded redirects and bounded
downloads. curl starts with a cleared environment holding only `SystemRoot` and whichever of
`HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` are set, so a proxy reaches the transfer while no curlrc,
credential, `http_proxy` or TLS setting does; Windows looks environment names up without regard to
case, so `https_proxy` and its lower-case peers arrive under the same three names.

When neither `HTTPS_PROXY` nor `ALL_PROXY` is set, an HTTPS transfer of `orchbee update` and of the
automatic helper uses the per-user system proxy (`src/cli/update/acquire/system_proxy.rs`, read by
`system_proxy` in `src/cli/update/windows.rs`); a loopback transfer never does. The read covers only
the environment variables `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` (found under any case) and the
values `ProxyEnable` (DWORD), `ProxyServer` and `ProxyOverride` (strings) of
`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Internet Settings`, opened for query
only; when either proxy variable is set the key is not opened at all. Nothing is written, and the
automatic configuration script (`AutoConfigURL`), automatic detection (WPAD), the binary
`Connections` values and policy keys are never read. A nonzero `ProxyEnable` with a `ProxyServer` of
one `host[:port]` gives `--proxy http://host:port`; a per-scheme list such as
`http=a:1;https=b:2;socks=c:3` gives its `https` entry as an HTTP proxy, else its `socks` entry as
`socks4://`, else no proxy, as Windows connects directly then. A server without a port takes
Windows' defaults, 80 for HTTP and 1080 for SOCKS. Unless `NO_PROXY` is set, which then stays curl's
bypass list, `ProxyOverride` becomes `--noproxy`: names and addresses pass unchanged, `*.name`
becomes `name` (curl also matches `name` itself), IPv4 patterns ending in `*` octets become CIDR
ranges, and `*` stays; `<local>` and any other pattern have no curl form and are left out, which
never matters for the dotted download hosts. A proxy that is off, a missing key, a value of another
type or a server not understood, including one with user information or another scheme, gives no
proxy argument, so a setting never makes a download fail.

The local source model retains the existing Windows disk-absolute grammar independently of the
compiling host: `/` and `\` separate components, repeated separators and `.` follow non-verbatim
Windows component semantics, and `..`, UNC, device, verbatim and drive-relative paths are rejected.
macOS and Linux also admit an absolute POSIX path, in
which `/` alone separates components, a backslash is part of a name and a `..` component is refused;
`install` names its own directory this way there.

The feed, `src/cli/update/feed.rs`, is one small signed document per channel and platform that names
the newest release and the one archive carrying it. It is closed JSON: `schemaVersion: 1`, `product:
"orchbee"`, `channel` (`stable` or `preview`; it must be the selected channel, else
`manifest-invalid`), `platform` (this updater's own), `version` and `buildId` (the manifest's rules
below), `manifest: {sha256}`, the SHA-256 of the archive's `release.json` and so the identity the
pointer records, and `archive: {format, url, size, sha256}`: `format` is `zip` on Windows and
`tar.gz` on macOS and Linux, `url` an HTTPS URL without a query or a portable relative path resolved
beside the feed, `size` from 1 byte to 1 GiB, and `sha256` lowercase. An optional `extensions`
object is checked and ignored; any other member is refused. The members, their bounds and the signed
bytes are frozen from the first version that ships them, and the repository fixture
`src/cli/update/tests/fixtures/feed.json.signed` with its `.sig` is the release tool's written form.
From the verified feed and the store alone, before anything large is requested, the update decides
in this order: the pointer names the feed's version and manifest, and that version's `release.json`
has that digest: up to date; the active version is higher and the command is not `channel set`: up
to date; `versions\<version>` holds another manifest: `version-conflict`; it holds the feed's
manifest but is not active: that copy is verified again and activated, with no download. Otherwise
the update takes the private folder `<store root>\.download\` and its lock `download.lock`, decides
again, and downloads the archive into `.download\archive.zip` with `--max-filesize` set to the
signed size. Its length and SHA-256 must then equal the signed ones, else `checksum`, a longer or
shorter body included; a body cut short while the server had sent its length ends the transfer
itself, which reports `download`. Only then does the system `tar.exe` unpack it: an absolute path
from the system directory, a cleared environment holding only `SystemRoot`, the working directory
`.download\unpacked`, and only the arguments `-x -f ..\archive.zip --strip-components 1`, under the
run's Job, deadline and Ctrl+C, so no path of the computer is an argument. `curl` and `tar.exe` run
in a Job that ends them when the Client ends, however abruptly, so neither keeps writing into
`.download` after a killed run. The archive's one top-level folder is a signed release directory, so
the unpacked folder installs exactly as `install` installs one, its manifest signature and every
listed file verified again, and its version, channel, build and manifest digest must equal the
feed's, else `manifest-invalid`. Nothing in the unpacked folder ever runs: the smoke check runs the
staged copy after its digest matched, and a file the manifest does not list is never copied.
`download.lock` is always taken before `install.lock`, and `install` never takes it. The folder's
owner removes `archive.zip` and `unpacked\` after it takes the lock and again on every return, so a
crashed run leaves at most one of each for the next run; the folder and its lock file stay. When a
child's end is unknown or removal fails, nothing more is removed and `cleanupPending` names the
folder. There is no resume or retry: a failed run leaves the versions and the pointer as they were,
and the next run starts over.

The updater reads at most 64 KiB of feed, 1 MiB of manifest and 4 KiB of signature document, and
verifies a signature before it parses the document it signs. Each signature is Ed25519 (RFC 8032)
over a UTF-8 domain, LF, then the exact document bytes: `orchbee-update-feed-v1` for a feed and
`orchbee-release-manifest-v2` for a manifest, so a signature of one never verifies as the other. The
signature document is closed JSON with 1–4 `signatures`, each exactly `{keyId, algorithm: "ed25519",
signature}`, plus an optional `extensions` object; `signature` is padded standard base64 of 64 bytes
and `keyId` the first 16 lowercase hexadecimal digits of SHA-256 over the 32-byte public key. Both
documents refuse repeated members, unknown members outside `extensions`, and nesting deeper than 32.
One entry from a compiled key that passes strict verification admits the document; entries naming
other keys are skipped. Nothing is downloaded before a feed is admitted, and nothing is staged
before a manifest is.

`orchbee` compiles two public key tables in `src/cli/update/signature.rs` and chooses one at compile time from the environment stem: a release build trusts only `RELEASE_KEYS`, and development and test builds trust only `TEST_KEYS`, whose private half is the repository fixture `src/cli/update/tests/fixtures/test-only-signing-key.pem`. No runtime input or test switch changes the choice. `RELEASE_KEYS` holds the key the release workflow signs with and a spare kept offline; their private halves never enter the repository. A build whose table is empty refuses every update with `signing-keys-unavailable`. Adding a key is one `TrustedKey::from_hex("<64 hex digits>"),` line holding the raw 32-byte public key, which `node -e "process.stdout.write(require('node:crypto').createPublicKey(require('node:fs').readFileSync(process.argv[1])).export({format:'der',type:'spki'}).subarray(-32).toString('hex'))" <key.pem>` prints. Rotation adds the successor before any manifest is signed with it and removes a key only after it retires.

A release manifest is closed JSON with `schemaVersion: 2`, `product: "orchbee"`, `channel`, `platform`, `version`, `buildId`, `files`, `components` and an optional `extensions` object. `channel` is 1–32 bytes of `a`–`z`, `0`–`9` and `-`, and must be the selected `stable` or `preview`; `platform` is 1–32 bytes of `a`–`z`, `0`–`9`, `_` and `-`, and this updater accepts only its own platform, such as `windows-x86_64`; `version` is SemVer 2.0.0 without build metadata, at most 64 bytes; `buildId` is 1–96 bytes of ASCII letters, digits, `.`, `_`, `+` and `-`. Each file has exactly `path`, `url`, `sha256` (lowercase), and `size` (bytes). File URLs are HTTPS or portable relative paths resolved beside the manifest. A manifest lists at most 4096 files, 512 MiB per file and 2 GiB total. Portable paths reject traversal, Windows devices, alternate streams, case aliases, file/directory collisions and the reserved `release.json` and `release.json.sig`. Installation requires `cohort.json` and every file it names, and installs every listed file; there is no fixed file list. A `components` entry for `client`, `server`, `worker`, `terminal-host` or `agent-observation-capsule` has exactly `id`, `version`, `files` (paths listed in `files`) and `connections` (each contract name with `generations: {lowest, highest}` within 1–65535), plus optional SemVer `securityFloor` and `recommendedFloor` and `extensions`. Entries with another `id`, connection names this build does not know, and every `extensions` value are ignored, and a missing entry imposes nothing. Every declared file is copied into a private stage and hashed; cohort and capsule digests are admitted before the new Client executes its exact version smoke check. A local source is only read, so its folder needs no owner or access rule: a release directory an elevated installer or npm unpacked, on any drive, installs like one in the person's own folder. Each source file must be a regular file and not a link or junction, or the install stops with `download`.

The store root holds `versions\<version>\`, one immutable signed release directory per installed
version with `release.json` and `release.json.sig` unchanged; `active.json`, the pointer;
`install.lock`, the writer lock; and `.download\`, the update's download folder above, which the
store's own reads never open. Publication stages inside the root and renames the verified stage into
`versions\`. An existing version directory holding the same manifest is verified and reused, never
overwritten; one holding another manifest is refused with `version-conflict`, exit `3`. Activation
is the published copy's own completion: `install` and `update` run that copy's `orchbee
--complete-install [--no-path] [--allow-downgrade]`, which verifies its own signed release,
refuses a lower version again unless allowed, performs the PATH step unless `--no-path`, and then
replaces `active.json` by a flushed rename of `active.pending` under `install.lock`. A failed
completion leaves the pointer where it was; a failure or lost result retains the published path for
explicit retry, and a completion that claims success while the pointer names another version is
`activation-unknown`; a completion that times out or is cancelled keeps `timeout` or `cancelled`
unless the pointer names its version afterwards, in which case it is `activation-unknown`, the same
as one not confirmed to have ended and one that fails after the pointer names its version. The
pointer is closed JSON, `{"version": 1, "active": {"product", "manifest"}, "previous"?,
"extensions"?}`: each entry names a version directory and the lowercase SHA-256 of its
`release.json`, the version it replaces becomes `previous`, and every rewrite keeps `extensions`,
which holds the fallback record and the `noPath` member: `true` after an install made with
`--no-path`, and any other value reads as absent. The pointer alone names the active version; a
missing or unreadable pointer means none, which an install repairs. The store opens only these names
and the `.stage-*` and `.remove-*` folders its writers leave, so a `desktop\` folder beside them is
never read; `install` creates a missing root below its existing parent with a private access rule in
place of the one it would inherit, which a `desktop\` written later inherits, and admits an existing
root when it is already private. The Desktop installer creates the root for `desktop\` before it
runs the bundled `orchbee install`, so that root inherits the access list of
`%LOCALAPPDATA%\Programs`: when admission refuses a root only for its access list, its owner is one
the rule admits, it is no link, and it holds nothing but `desktop`, `desktop.previous`,
`unins000.exe` and `unins000.dat`, `install` replaces that list with the private rule, which Windows
carries down to those entries, and admits it. The folders above the root are never judged or
changed, except that a missing parent is created as a private folder below its nearest existing
folder, which must already be private. Private is the shared rule of the [local runtime's private
storage](../../../crates/local-runtime/README.md#private-storage-on-macos-and-linux): the person's
account, SYSTEM or Administrators owns the folder, and every allow entry of its access list names
one of the three. An ordinary session meets it with the person as owner; an elevated session of the
same account, and an elevated installer a moment earlier, meet it with Administrators as owner,
which Windows gives what they create. Any other root another account owns, or whose access list lets
in another account (Users, Authenticated Users or Everyone included), is refused and never repaired,
and so is any such folder below it. Existing processes keep their original files. Disk stays
bounded: a store keeps its active and previous versions and every other version a process still runs
from. Where automatic updates run, which today is test builds and in release builds only once
automatic updates are released, each completion that moved the pointer from a readable one removes,
still under `install.lock` and after the pointer move, every other version from which no process
runs: the shared process scan finds no process whose program file lies inside its folder, this one
included, and every `.exe`, `.dll` and `.node` in it opens for writing, the check Setup applies
before it replaces files. A Session or Host refers to a version only through the processes its
registrations name, and an Agent observation Capsule is a copy in the data root that names no
version, so no other reference keeps one. A version goes by a rename to `.remove-<process>-<number>`
in the store root and then a deletion, so `versions\` only ever holds whole versions; one that
cannot be moved stays whole, and the same completion also deletes the `.remove-*` and `.stage-*`
folders that earlier runs left, when nothing runs from them. Whatever cannot go now stays for a
later completion, and the cleanup never changes the completion's result or report. Unpublished
stages are cleaned after child settlement; unconfirmed settlement or cleanup failure transfers the
exact stage path to explicit recovery in the receipt, until such a later completion deletes it.

An update whose verified feed names the version and manifest the pointer already names finishes at
once without downloading, hashing, publishing or completing anything: `upToDate` is true, with
`reused` and `activated` true and `releasePath` naming the active version; `releaseVersion` and
`buildId` come from the feed. A feed offering a version lower than the active one, such as a channel
pointed back after a bad release, also finishes with `upToDate` true, nothing copied and the pointer
unchanged, never `downgrade-refused`; `channel set` is the person's choice and may move to a lower
version. A feed offering a version the store holds after a fallback finishes the same way, as does a
completion that meets `version-held` because a fallback held the version during the update. A
published path can be used directly even when activation needs retry.

The automatic helper `orchbee --automatic-update` is the entry by which a Session Server runs an [automatic update](../../../proposals/platform/automatic-updates.md#who-checks-and-installs): the store's active Client with that one argument and nothing more, since any other argument list is not the helper. Help, completion and the `--skill` guide leave it out, and its grammar never changes; a changed helper takes a new name. A build answers it only while [automatic updates run in it](../../../crates/local-runtime/README.md#automatic-update-switch): every test build, and a release build only once that switch is on, so until then a release build refuses the word as an unknown option. The helper is `update` of the selected channel with three differences. It saves no channel and asks the completion for no PATH step (`--no-path`), so the user `Path` is never read or written; instead, just before that completion and under `install.lock`, it refreshes `<store root>\bin\<stem>.exe` from the new version's `<stem>-launcher.exe` the way the PATH step does, but only when something already exists at that name, so a store without the launcher, such as one only the npm carrier installed, stays without it. On standard error it writes only its progress, never the download line: one JSON record per line, `{"schema":"orchbee.update.progress","version":1,"phase":"download","releaseVersion":…,"receivedBytes":…,"totalBytes":…}` while an archive transfers and `{"schema":"orchbee.update.progress","version":1,"phase":"install","releaseVersion":…}` once nothing more downloads and the run verifies and installs. A run's download counts against one total, the runtime archive and the window archive the run expects to download after it, which the received bytes never pass; every record names the version of the run's first part that names one, which is the release it brings. Download records come at most every 250 ms apart from a transfer's start and end, and a failed write changes nothing about the run, whose only result stays its document. The Session Server follows these records to show the run's download and install in Check for updates. And it answers only the `orchbee.update` document, with the members and exit status of `update`; a refusal before any update, by platform admission or the data root's layout, is the shared JSON failure document. A launcher that cannot be refreshed ends the run as `activation` before the pointer moves, as a failed PATH step does for `update`. A run that meets a held `download.lock`, or meets `install.lock` while it publishes or refreshes, ends as `busy`; a completion that meets `install.lock` ends as `activation`, because the completion's exit status does not tell the two apart. Neither moves the pointer, and the next run activates a version published before the stop from the store without downloading it again. A development build refuses with `non-release-build`, as `update` does.

The window part of `update` and of the automatic helper installs the [window
version](../../../proposals/platform/automatic-updates.md#installing-a-window-version) a feed
offers, on Windows, macOS and Linux, where [window updates
run](../../../crates/local-runtime/README.md#automatic-update-switch): every test build, and every
release build from the window self-update release on. It runs only after the runtime part ended
without an error, only when `<store root>\desktop\` is a folder, only when the feed carries
`extensions.desktop`, and only when the store's active runtime version is not lower than the offered
window version, since the window's bundled Client then hands its Session start to the store;
otherwise the run has no window part and the document no `desktop` member, so a failed or held
runtime part leaves the window for a later run. `extensions.desktop` is closed JSON, `{version,
manifest: {sha256}, archive: {format, url, size, sha256}, extensions?}`, `format` being `"zip"` on
Windows and macOS and `"tar.gz"` on Linux and each member bounded
as the feed's own member of that name, `manifest.sha256` being the SHA-256 of the window's
`desktop-files.json`; a malformed one never refuses the feed and fails only the window part, as
`manifest-invalid`. Before it publishes a window version, each executable of that version must fit
the same Windows program path limit, or the window part fails as `install-location` and publishes
nothing; the runtime part is not undone. The window store `desktop\` is a second store root with its
own `active.json`, `install.lock` and `versions\<version>\`; its pointer has the grammar of the
store's, `manifest` naming the SHA-256 of the version's `desktop-files.json`, which names every
other file of the version with its size and SHA-256. From the feed and the window store alone the
part decides: the window pointer names the offered version and list, and the stored list has that
digest: up to date; a higher window version is active: up to date, since a window version never
moves down; `desktop\versions\<version>` holds another list: `version-conflict`; it holds the
offered list: every listed file is verified and the version activated, with no download. Otherwise,
after the runtime part released it, the part takes `.download\` and `download.lock`, decides again,
writes `Downloading the Orchbee window <version> (<n> MB)…` (`正在下载 Orchbee 窗口程序 <version>（<n> MB）…`)
on standard error under `update`, fetches the window archive, a zip on every platform, under the
folder's archive name, checks its signed size and SHA-256, and unpacks it with the same system tar
and arguments, which read the format from the bytes. The unpacked folder must hold a
`desktop-files.json` with the digest the feed names and the offered version, else
`manifest-invalid`. Under the store root's `install.lock` and then `desktop\install.lock` the part
decides again, stages `desktop\.stage-*` with the list first, copies exactly the listed files, each
checked by size and SHA-256 as it is written and then flushed, a listed path that is missing or not
a regular file being `checksum`, publishes the stage as `desktop\versions\<version>` without
replacing, and moves the window pointer, the replaced version becoming `previous`. A stage holds
each folder it creates open until it publishes, so on macOS the window install first raises the
process's soft limit on open files to its hard limit, at most 10 240: an application bundle holds
hundreds of folders, more than the 256 a Terminal shell starts with. A running window is never
touched. Every run whose runtime part ended without an error and that finds `desktop\` then arranges
the window store, whether or not it had a window part and whatever that part did, under the store
root's `install.lock` and then `desktop\install.lock`, best effort and without a member of its own;
a lock another update holds skips it, and whatever runs or cannot be removed stays for the next run.
Once the window pointer names an active version, it places the window entry
`desktop\<stem>-desktop.exe`, the path the Start menu shortcut, the taskbar and the uninstall
registration name, as a byte copy of that version's `resources\cohort\<stem>-desktop-launcher.exe`:
an equal entry stays, and any other is replaced by renaming a flushed
`desktop\.new-<process>-<number>` over it, moving a running entry aside as
`desktop\.remove-flat-<process>-<number>.exe` first when Windows refuses a direct overwrite so the
launcher takes its path at once. Once the entry is placed, it removes what a flat layout, which a
window before the window self-update release kept directly in `desktop\`, left there: the flat
`resources\cohort\` whole once no process runs from it, since a Session an older window started may
run from there, and every other name (`.remove-flat-*` included) file by file once no process runs
from any of them, except `versions\`, `active.json`, `active.pending`, `install.lock`, the entry and
the `.stage-*`, `.new-*` and `.remove-<process>-<number>` names. It removes a `desktop.previous`
that an earlier Setup left in the store root once no process runs from it, and on Windows window
versions that are neither active nor previous, with the leftovers of stopped runs, as the runtime
completion removes runtime versions. No process runs from a set of files when the process scan finds
none whose program lies inside and every `.exe`, `.dll` and `.node` inside opens for writing. On
macOS the application bundle the person opens is the entry, which the window launcher backs up and
replaces with a copy of `pointer.active`, so the arrangement places and moves nothing and keeps only
`pointer.active` in `desktop/versions/` plus any version or leftover a process still runs from
(checked by process start paths), while runtime versions there keep the rule above and stay. Desktop
Main also runs `--tend-window-store` through its bundled Client shortly after startup so retired
window versions and flat-layout leftovers leave disk once a replaced process exits without waiting
for a later update. The exit status takes the runtime part's error first, then the window part's,
then a folder either part left for recovery. On Windows, Setup writes `desktop\setup-launch.json`
before its own launch of the window it installed, with `{"version":"<that window version>"}`, when
the launch is ticked and the install is not silent. The window that launch starts reads and deletes
the file: its switch goes ahead only when the file is at most 120 seconds old, parses, and names the
version the store offers, and the file is deleted on every read. A first window start deletes a
leftover file without acting on it. `layout_name` keeps the file out of the flat-layout cleanup, and
macOS has no such file.

A Client older than the store's active window version runs only the version retention of the arrangement above, on every run that reaches it: the window part of `update`, `--installer-window` or `--tend-window-store`. It places no entry, moves no flat layout and removes no name under `desktop\` except retired versions, because a newer window version adds names there that an older Client does not know. The comparison is the store's product version precedence between this Client's version and the version `active.json` names active; a Client that is not older arranges the store.

The window installer helper `orchbee --installer-window` installs a window directory that Setup or install.sh unpacked. They run it through the Client bundled in that directory, `<window>\resources\cohort\` on Windows, `<window>/resources/cohort/` on Linux and `<window>/<name>.app/Contents/Resources/cohort/` on macOS; a Client anywhere else refuses with `not-release-directory`, exit `4`. Help, completion and the `--skill` guide leave it out, and its grammar never changes; a build answers it only while window updates run in it, on Windows, macOS and Linux, and any other build, such as a release build before the window self-update release, refuses the word as an unknown option. It reads the window directory's `desktop-files.json` and installs that version into `<store root>\desktop\` with the window part's install step and locks, creating a missing `desktop\` as a private folder, whatever the store's runtime version: a window above it only starts its Session from its own folder. Then, whatever the install answered, it arranges the window store as the window part does. It opens nothing in the data root and answers one `orchbee.install` document in every case, with `channel` and `buildId` null and `pathStep` `not-requested`: exit `0`, or `3` with `downgrade-refused` when a higher window version is active and stays active, or with `version-conflict`. A development build refuses with `development-build`.

The Client-failure helper `orchbee --record-client-failure <version>` is the entry by which either launcher, or a macOS or Linux TUI that no launcher started, records a [failed Client](../../../proposals/platform/automatic-updates.md#what-counts-and-who-records-it) of a moved version: the previous version's Client with that word and exactly one version, since any other argument list is not the helper. Help, completion and the `--skill` guide leave it out, and its grammar never changes; a build answers it only while automatic updates run in it, on every platform. It records one Client failure of `<version>` in the store its own program runs from, counted only while `<version>` is the entry of the store's counting move, and that write may commit the fallback; a Client outside a store version folder records nothing. It opens no data root, writes nothing on standard output or error, and exits `0`.

The public feed path `/update/{channel}/{platform}/feed.json` is permanently frozen at `schemaVersion: 1` and the platform's v1 archive format (`zip` on Windows, `tar.gz` on macOS and Linux); additive metadata enters only `extensions`. Any future breaking change to the feed schema, archive format, or cohort layout must read a new versioned path (such as `/update/v2/{channel}/{platform}/feed.json`), leave the v1 path parked at the last stepping-stone (bridge) release that speaks both contracts, and retain `--complete-install` (with its sibling helpers `--installer-uninstall`, `--automatic-update`, `--record-client-failure`, and `--installer-window`) in newer binaries so older updaters, launchers, Servers, and installers can always activate the bridge or newer release.

Releases up to 0.1.0-beta.24 know each of these helpers, and `--tend-window-store`, only by an earlier spelling with a `-v1` suffix, such as `--complete-install-v1`. A later build answers both spellings, with the same grammar after the word, and help, completion and the `--skill` guide name neither. Whatever runs one of these helpers in such a release's program gives it the earlier spelling: `install` and `update` complete a version up to 0.1.0-beta.24 with `--complete-install-v1`, and the Server, both launchers, install.sh and Setup's uninstaller do the same for the helpers they run.

To prepare a signed release directory from a completed Windows cohort without publishing it, stage the cohort with `--artifact-stem orchbee` so it carries the release names, then package and sign it:

```powershell
node scripts/stage-local-runtime.mjs --output C:\path\to\cohort --artifact-stem orchbee
node scripts/package-native-release.mjs C:/path/to/cohort stable 0.1.0 build-id
node scripts/sign-release-manifest.mjs C:/path/to/cohort/release.json --key-env ORCHBEE_RELEASE_SIGNING_KEY
```

An update source is a zip holding that directory as its one top-level folder and the feed document naming it; the release tool writes both, and its test profile names the archive by its bare file name and signs the feed with the test key ([releasing](../../../docs/RELEASING.md)), so a folder holding the feed, its `.sig` and the zip is a complete local feed. A rehearsal of `install` packages a test cohort the same way: stage it with an `ob-test-<16 hex>` stem, package it, sign it with `--key-file` naming the test key fixture, point `ORCHBEE_INSTALL_DIR` at a scratch folder, and run the staged `<stem>.exe install --format json`. The packaging tool refuses a development cohort. A rehearsal of `update` prepares a second test cohort of a higher version the same way, zips it with a test feed beside it, and runs the installed `<store root>\bin\<stem>.exe update --format json` with `ORCHBEE_UPDATE_FEED` naming that feed in its folder or on a loopback server serving the folder, under the isolation below so that the PATH step writes the scratch `Path`.

The packaging tool writes the platform token of the host it runs on, `windows-x86_64` on the release runner, so a cohort is packaged on the platform that staged it. On macOS or Linux it writes `macos-<arch>` or `linux-<arch>` (`aarch64` or `x86_64`) and expects the cohort that staging writes there: the four programs named after the stem without `.exe`, no launchers, and no ConPTY bundle, which it refuses. On Windows it verifies the existing cohort, adapter capsule and pinned ConPTY bundle, inventories bounded regular files, assigns each component its files and the connection generations of `schemas/connection/v1/connections.json`, and exclusively writes `release.json` beside them. It refuses an existing `release.json` or `release.json.sig`, and a registry contract that no component lists. A test cohort's `<stem>-files[.exe]`, the File Bar's process, belongs to no component and must match the digest `cohort.json` declares; a release cohort carrying one is refused. It writes no floors; a person adds `securityFloor` or `recommendedFloor` before signing. The signing tool takes 1–4 distinct Ed25519 PKCS #8 PEM keys from absolute `--key-file` paths or named `--key-env` variables, refuses anything but a version 2 Orchbee manifest of at most 1 MiB, checks each signature it produces, exclusively writes `release.json.sig`, and never prints key material; the release workflow passes the protected secret `ORCHBEE_RELEASE_SIGNING_KEY` this way. For distribution, the release tool zips that directory, both documents included, as the archive's one top-level folder and writes the feed document naming the zip; the unpacked folder is read file by file at each manifest path, so file URLs are not consulted. Version must match the packaged Client's `--version`; buildId distinguishes releases with the same version.

The npm carrier is written from the signed release directories of one version, one directory per platform: `node scripts/package-npm.mjs --version <version> --output <absolute new directory> [--repository-url https://<host>/<owner>/<name>] <release directory>...`; the repository URL becomes every package's `repository`, which npm provenance compares with the building repository. It copies each directory unchanged, both documents included, under `release/` of a platform sub-package with `os` and `cpu` and no `bin` or script, and writes the main package from `apps/orchbee/npm/orchbee/` with exactly the sub-packages of that run as exact-version `optionalDependencies` and `publishConfig.tag` `next` for a pre-release, `latest` otherwise. A release stem names the packages `orchbee` and `@orchbee/cli-<platform>-<arch>`; a test stem names them `<stem>` and `@<stem>/cli-<platform>-<arch>` with the bin `<stem>`, so a rehearsal never produces a release name. It refuses a version other than the manifests', a second directory for one platform, mixed stems, a file that does not match its manifest digest, a release file npm would leave out, and an existing output directory. A macOS or Linux sub-package keeps the four programs executable, and a test release's `<stem>-files` too, and is written only on macOS or Linux, because npm keeps the executable bit it packs and sets one itself only for `bin` entries. The [release workflow](../../../.github/workflows/release.yml) packs and publishes the folders. The launcher, `bin/orchbee.mjs`, runs on Node 18 or newer (`engines`) and uses only Node built-ins. On each start it maps `process.platform` and `process.arch` to its listed sub-package, reads the version from that package and the stem from its own name, and reads `active.json` in the store root of the `ORCHBEE_INSTALL_DIR` rule above. With no active version or a lower one it runs the carried `<stem>.exe install --no-path --format json`, waits while that answers `busy` for another start, and stops with a localized line when the published path lies outside the root it reads. It then starts `versions\<version>\<stem>.exe` with the same arguments, standard handles, and environment and exits with its status, and follows the launcher's [relaunch protocol](../../orchbee-launcher/README.md#relaunch-protocol) with the same rule, the record of a failed Client included; console Ctrl+C, Ctrl+Break, and close reach the program directly and the launcher only outlives them. A platform without a sub-package gets one line naming the supported ones.

The PATH step runs inside the completion under `install.lock`, before the pointer moves. It copies the version's `<stem>-launcher.exe` to `<store root>\bin\<stem>.exe` when the bytes differ, so a release installs `bin\orchbee.exe`. A launcher already there, running or not, is renamed aside first and renamed back if its replacement cannot be renamed in, so `bin` never ends a step without one; an aside copy a running launcher still holds is removed by a later step. The step then edits the user `Path`, value `Path` of `HKCU\Environment`: `<store root>\bin` goes first when no entry names it, comparing without case, surrounding quotes or trailing separators; entries naming another folder inside the store root are removed in the same write, unless the root is a drive or share root; every other entry stays byte for byte. The value keeps its registry type (`REG_EXPAND_SZ` when absent), is read again just before the write and recomputed if another writer changed it, then flushed and read back. A value that is not text, would pass 32,760 characters, or holds text that cannot be read without loss is refused. The command writes the value and does not broadcast the change: a platform installer broadcasts at its own end (Inno Setup's `ChangesEnvironment`), and a broadcast from the command itself waits for an unsafe-code policy decision. Until then a program started earlier, Explorer included, sees the entry only after it rereads the user environment, so the text result of an `added` or `replaced` step adds one line: open a new terminal, and sign out and back in if the command is not found there. A failure stops the completion before the pointer moves with `path-launcher` or `path-variable` on its standard error; the installing Client reports `activation` and repeats that line. `update` always performs the step.

The launcher, [`apps/orchbee-launcher`](../../orchbee-launcher/README.md), reads `<store root>\active.json` with the shared pointer parser and starts `versions\<product>\<stem>.exe` with the command line after its own name unchanged, the same console, standard handles, environment and working directory, and exits with the Client's exit code. Where [automatic updates run](../../../crates/local-runtime/README.md#automatic-update-switch) it also follows the [relaunch protocol](../../orchbee-launcher/README.md#relaunch-protocol): each Client receives the launcher's process id as `ORCHBEE_RELAUNCH` and as `ORCHBEE_RELAUNCH_V1`, the only spelling releases up to 0.1.0-beta.24 read; status `75` starts the active version again with the same command line, at most three times in ten seconds, and a fourth request inside that time ends the launcher with `1`; a relaunched Client that fails to start or to join gives way to the version it replaced; and status `76`, a failed join, reaches the caller as `1`. After any start whose Client fails to start or to join, it reads the pointer again; when the pointer still makes that version active through a counting move whose `back` is true and names a `previous`, it runs `versions\<previous>\<stem>.exe --record-client-failure <version>`, spelled `--record-client-failure-v1` for a previous version up to 0.1.0-beta.24, outside the Client's Job with null standard streams, waits for it at most five seconds, leaves it running after that, and goes on as it would without it. The Client runs in a Job that ends it when the launcher is ended; the Server, Worker and Terminal Host the Client starts are created outside that Job and keep running. A missing or unreadable pointer, or a Client that cannot start, prints one localized line on standard error and exits `1`. A release directory carries the launcher as an inventoried file outside every component, and `cohort.json` does not list it.

A release or test Client that does not run from a store version folder, such as the Desktop window's bundled Client in `desktop\resources\cohort\` or one in an unpacked folder, hands a Session start to the store's active version when the selected Session has no Client endpoint. It finds the store root by the `ORCHBEE_INSTALL_DIR` rule above and reads the pointer under the store's private rule, trying a read that fails other than by absence three more times, 20 ms apart. When the active version is not lower than its own and `versions\<version>\<stem>.exe` is a private regular file with one link, it pins that folder and file and runs it with `[--session <name>] server start --format json`: the `--session` it was given, if any, the same environment, no console window, null standard input and error, at most 64 KiB of standard output kept, and the system folder as working folder. It waits for that Client within its own start deadline and never ends it: at the deadline it answers `timeout` while the store's Client keeps its own 20-second start. Whatever that Client answers, its own activation then runs as before and alone decides the outcome: it joins the Session the store's Client started, reports the refusal it meets, or starts its own Server; `result.outcome` is `started` when the store's Client exited 0 reporting `started`. So the window's Session runs from the store, a setup run can replace `desktop\` while its terminals keep running, and a newer store version is never replaced by an older Client's. The Client starts its own folder's Server as before when it is a development build, runs from any store's version folder (as `bin\<stem>.exe` and the npm carrier start it, so a Client started this way never hands a start on), finds no store root, no pointer or bytes that are no pointer, an unreadable pointer, an active version lower than its own or no store Client, or when the store's Client cannot start or exits non-zero. `server start`, `server connect`, the TUI and the remote link helper's `ensure-session` record each decision in the diagnostic log as `session-start-source` with `source` `store` or `own`, and for `own` a `reason`: `development`, `store-client`, `no-store-root`, `no-pointer`, `pointer-unreadable`, `store-lower`, `client-missing` or `store-start-failed`, the last with the store Client's exit `status` when it exited. The store Client's file name carries the build's environment stem and each stem has its own default root, so a release build only ever starts a release store's Client and a test build only its own store's.

The immutable release also stores the exact admitted `release.json` and its signature document, reserved outside the inventory. Reuse checks that `release.json` has not changed. The pointer entry naming the version, with the manifest SHA-256, binds those bytes to the verified installation, so later reads parse them without the signature document. The installation check of `doctor` (and the unlisted `status client` it shares its collector with) binds this manifest to a `versions\<version>` directory that the pointer names as active or previous and to the verified cohort before projecting installed release provenance; anything else reads as unknown, and it never substitutes the current configured preference.

The CLI-private `release_store` module owns the store's shape: immutable publication into
`versions\<version>` and the pointer's bytes. The pointer's grammar, with the strict JSON reader and
version and digest tokens of release documents, is the shared `orchbee-release-records` crate, which
the launcher also reads it with. The store-root rule and the pointer's file name and bound belong to
the local runtime's [store reader](../../../crates/local-runtime/README.md#release-store-reading),
which `install`, `update`, `uninstall` and Session start share and this module takes them from.
`ReleaseStore` takes `install.lock` through the private cache writer lock, pins installation
ancestors, and replaces `active.json` through `active.pending`; readers read the pointer without the
lock. A stage borrows that lock, creates only validated portable paths and exclusive private files,
and publishes by a no-replace directory rename to its version name. The application owns the
pointer's meaning, manifest validation, downloads, cohort admission, smoke checks, completion, the
PATH step and which versions to keep. Whether a process runs from a folder is the CLI's shared
`running_programs` check, the process scan uninstall waits on together with the write probe of
programs and libraries. `open_release_file` rejects unsafe ancestors, reparse points, an owner or
access list outside the private rule, and hard links. The macOS and Linux ports create a missing
root and its missing ancestors as private folders, create stages and folders with mode 0700, rename
only onto an absent name, list and delete without following a link, and make a staged program file
executable for its owner alone.

## Windows release uninstall

`orchbee uninstall [--delete-data] [--confirm] [--format text|json]` removes this environment's installation and needs no Session. It first resolves the store root (the `ORCHBEE_INSTALL_DIR` rule above) and the data root (`ORCHBEE_HOME`, otherwise `%USERPROFILE%\.<stem>`), computes everything it would remove as a plan, and fences it: the whole run is refused, with error `plan` and a `reason`, when a root is relative, spells `.` or `..`, lies less than two folders below a volume, or equals or holds the profile, an application-data, system, program, per-user programs or temporary folder (`unsafe-root`); when the roots overlap (`roots-overlap`); or when a target lies outside both roots (`outside-roots`), above one (`root-parent`), or below a link, junction or unreadable folder inside its root, or a root is itself a link or junction (`through-link`). A target that is itself a link is removed as a link; no removal follows one.

Uninstall removes only what the two layouts name, and only from a root that carries its layout's mark. The store's entries are `bin`, `desktop`, `versions`, `.download`, `install.lock`, `active.pending`, publication stages `.stage-<process>-<number>`, removals `.remove-<process>-<number>` and the pointer `active.json`, its mark, which must read as a pointer under the private rule `install` admits the root with, so uninstall recognizes every store `install` accepted, one an elevated session created included. The data root's entries are `config.toml`, `sessions`, `logs`, `agent-detection` and `internal`; its mark is `internal\version.json`, which must read as a layout record of any layout, and every writer that creates the root, Desktop included, writes it in the same step. A root that holds anything the run would remove but no readable mark refuses the run before anything changes, with `not-orchbee-store` or `not-orchbee-data`, naming the folder and `ORCHBEE_INSTALL_DIR` or `ORCHBEE_HOME` when that variable chose it. A missing or empty root, or one that holds nothing of its layout, is already absent. In a store root that carries its pointer, the Desktop installer's own files, `unins000.exe` and `unins000.dat`, are the installer's by its contract with the Client: the [Windows installer](../../desktop/README.md#windows-installer) keeps its uninstaller and uninstall log there under exactly these names. `uninstall` removes them with the store's entries, before the pointer, so the store root goes and nothing of the installer outlives its registration; the helper form below leaves them to the uninstaller that runs it. In any other folder those names are another program's. Anything else found in a root stays, the run still succeeds, and the listing, the final message and the document's `kept` say how many other items each folder keeps; the installer's files are not counted among them. Each mark goes after everything else of its layout, and a root goes last and only as an empty folder, so an interrupted run leaves a root the next run still recognizes. A data root that did not exist when the run began, which the run's own request record creates, goes again once empty.

Run inside a terminal of one of the environment's Sessions it refuses with `inside-session`, since it would end that terminal.

It then lists what goes and what stays: the program folder, the user `Path` entry and the installer entries present, the Agent integrations this environment may have enabled (see step 2), what leaves the data folder in plain words (or the whole data folder with `--delete-data`), how many other items each root keeps, every running Session with its open terminals and running Agents, and the [machine hub](machines.md#client-owner) of the data root by its process id, saying that every remote machine login it holds ends with it. A release build shows the list as the prompt of the typed confirmation, `orchbee`, and refuses `--confirm` and non-interactive callers as every guarded command does; a development or test build requires `--confirm` and prints the list on standard error under `text`. Before anything changes it refuses with exit `3` when a Session another installed version runs has open terminals (`session-unstoppable`) or a program outside every Session and its terminals, such as the Desktop application, runs from the store root (`program-running`). The helper form below does not count the platform uninstaller that started it and waits for it.

The steps then run in this order, each safe to repeat:

1. Every background process of the environment ends: each Session is stopped like `session stop`, one another version runs only when it reports no terminal; then the machine hub of the data root ends through `drain` and, after 10 seconds, `exit`, whether or not the listing named one, and a hub that will not end is left for the wait below. The run then waits until nothing but itself, the launcher and the helper form's waiting uninstaller runs from the store root; a program of a kind it cannot end stops it with `still-running`, naming the program, before step 2.
2. Every Agent integration this environment may have enabled is disabled through the Agent integration writer's own disable, in this process. Evidence is read before anything changes and starts no Agent program: the provider's files carry this environment's entry, Hermes's two entry files included; or, for Claude Code, Grok and GitHub Copilot, whose plugins only their own program reports, the program is on `PATH` and the data root holds this environment's Capsule folder `internal\integrations\agent-observation`, which only an enable creates and without which no Agent entry can name this environment. An Agent program starts only for a provider with evidence, and a refusal or failure of such a provider stops the run as before. Without evidence a provider is `not-enabled` when its program would have to start or no Capsule folder exists; otherwise the writer reads its files, and a refusal there, such as an unreadable file (`inspection-failed`), makes it `not-checked` and the run goes on, the final message naming it with the reason token and saying to remove Orchbee's entry from that Agent's configuration if the person did enable it. The writer leaves the rest of each Agent's configuration as it was, leaves another environment's entry as it is without stopping, as its [ownership rule](../../../crates/agent-integration-writer/README.md#capsule-location-and-ownership) says, and refuses a hook the person changed (`conflict`); a provider whose state only its program reports and whose program is not on `PATH` is `program-not-found`. The writer gives each provider command three seconds, so an Agent program slower than that to answer is `inspection-failed`.
3. The user `Path` entry goes by the exact inverse of the PATH step's edit: entries naming `<store root>\bin`, in any spelling that edit recognizes, and entries naming another folder inside the store root are removed; every other entry stays byte for byte and the value keeps its registry type. A value that held only those entries stays as an empty value of its type, which is how the PATH step finds an account whose `Path` was empty; one that was absent before the install also ends empty, since nothing records that it was absent. Nothing is broadcast. Then the Start menu shortcut, `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Orchbee.lnk`, and the uninstall registration, `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\com.orchbee.desktop_is1`, go; a development or test build uses its stem in both names.
4. `logs\`, `internal\integrations\`, `internal\cache\`, `internal\runtime\` and `internal\tmp\` leave the data root; `config.toml`, `sessions\`, `agent-detection\` and the rest of `internal\` stay. With `--delete-data` the [remembered login answers](machines.md#remembered-answers) of every Machine the catalog names leave the credential store first, then every entry of the data layout goes, `internal\version.json` last, then the data root when nothing else is in it.
5. The store's entries go last, the pointer after all of them, then the store root when nothing else is in it. While any other program runs from the store nothing in it is touched, so a second run can finish. The running Client's file, the launcher waiting for it and their folders cannot be deleted while they run: the command starts a detached `cmd.exe` from the system folder, inheriting no handle and working in the system folder, which once per second for about a minute deletes exactly those files and then their folders, without recursion; once they are gone it deletes the pointer and removes the store root with a plain `rd`, which leaves a root that keeps other items. It ends as soon as those files and the pointer are gone. It leaves no file of Orchbee's and no process behind; interrupted, it leaves the pointer while any of those files remains, so another run recognizes and finishes the store.

A step that fails stops the run: nothing further changes, standard error names the cause, what was
done and what remains, and how to continue, naming the program by its full path once the `Path`
entry is gone. Started through the npm carrier, whose launcher runs the Client under Node.js, the
final message adds `npm rm -g orchbee`, since a later start through the carrier installs again.

A platform uninstaller runs `orchbee --installer-uninstall [--delete-data]` after its own confirmation. It asks nothing, answers in text, and ends no terminal: with one open in any Session of the environment it changes nothing and exits `3` with error `terminals-open` and a line the uninstaller can show. The uninstaller that started it waits in the store root until it exits, so the helper recognizes it by what it is to the helper rather than by name: a process on the helper's own chain of parents, each link started no later than the process it started, whose program lies in the store root outside every entry the removal removes. That process neither stops the run as `program-running` nor keeps the store wait from finishing; any other program in the store root, and any of Orchbee's own programs even on that chain, still stops it as before. The helper leaves the installer's own files, `unins000.exe` and `unins000.dat`, in the store root, since the uninstaller still runs from the one and writes the other; the uninstaller removes them after the helper exits. Whichever of the uninstaller and the detached removal of step 5 finishes last removes the then empty store root: each deletes its own files before it removes the folder, so one of the two always finds it empty.

A test build reads `ORCHBEE_TEST_REGISTRY_KEY`, a key below `HKCU\Software\ob-test-scratch\` with
one plain name, and uses its `Environment` and `Uninstall` subkeys instead of the real ones, for
uninstall and for the PATH step of `install` and `update`; any other value refuses, with `location`
in uninstall and as a failed PATH step before the pointer moves, and no other build reads it. A
rehearsal stages, packages and test-signs a cohort as above, then runs every command with its own
`USERPROFILE`, `HOME`, `APPDATA`, `LOCALAPPDATA`, `TEMP`, `ORCHBEE_HOME`, `ORCHBEE_INSTALL_DIR`,
`ORCHBEE_TEST_REGISTRY_KEY` and a `PATH` holding no Agent program: `install --no-path`, a copy of
the version's `<stem>-launcher.exe` to `<store root>\bin\<stem>.exe` and the scratch `Path`,
registration and shortcut standing in for the PATH step and the installer, then `<store
root>\bin\<stem>.exe uninstall --confirm`.

## macOS and Linux release installation

`orchbee install`, `orchbee uninstall`, `orchbee update` and the automatic helper run on macOS and Linux, with the grammar, documents, exit statuses and steps of the Windows sections above; this section names what differs. `channel set` runs only on Windows.

`update` reads the feed of `macos-<architecture>` or `linux-<architecture>`, downloads its `tar.gz` archive into `.download/archive.tar.gz` with the system `/usr/bin/curl`, and unpacks it with the system `/usr/bin/tar` and the arguments `-x -f <archive> -C <unpacked> --strip-components 1`. The two paths are absolute and below the admitted store root, which holds no `:`, quote or control character, because the launch sets no working directory. tar receives an empty environment, and curl only those of `https_proxy`, `HTTPS_PROXY`, `all_proxy`, `ALL_PROXY`, `no_proxy` and `NO_PROXY` that are set; no system proxy setting is read. curl older than 8.4 reports `download-unavailable` for an HTTPS feed, as on Windows. Every child of `install`, `update` and the automatic helper starts through `posix_spawn` with an empty signal mask, `SIGPIPE` at its default action and standard input from `/dev/null`, as the leader of its own process group, which settles before the result. No Job ends them when the Client itself is killed with `SIGKILL`, so a killed run may leave curl or tar writing into `.download` until they end; the next run's lock holder removes what they left. The TUI and window update actions follow the same offer as on Windows; no launcher relaunches the TUI on macOS or Linux, so after an update it exits and names the command that opens the new version. A Linux TUI does not restart or move a Session: the Session takes the new version when no terminal runs. The Session Server starts the automatic helper as on Windows. The window part and `--installer-window` run as on Windows. On Linux the window list sits at the root of the window directory and marks a file its owner may run with `"executable": true`; such a file gets mode 0700 and every other file 0600 in folders of mode 0700. The arrangement places the entry `desktop/<stem>-desktop`, a copy of the active version's `resources/cohort/<stem>-desktop-launcher` renamed over the old one, then, for this build's default store root only, the person's menu entry `<data>/applications/<stem>.desktop`, from the version's template with the program of every `Exec=` line replaced by that entry and its arguments kept, and its icons `<data>/icons/hicolor/<n>x<n>/apps/<stem>.png` from the version's `resources/linux/`, `<data>` being an absolute `XDG_DATA_HOME` or else `~/.local/share`; it removes window versions as on Windows, finding their processes through `/proc`. On macOS the window list sits beside the signed bundle in the window directory and in each `desktop/versions/<version>/`, its paths start with `<name>.app/` and admit spaces and parentheses, a file the list marks `"executable": true` gets mode 0700 and every other file 0600 in folders of mode 0700, and its `links` are made as symbolic links after every file, so no file is written through one; verifying a stored version also compares each link's target. Its arrangement there places no entry, since the application bundle is the entry, and removes nothing, since no process check can be made on macOS. Run from a window application's bundled cohort, `<name>.app/Contents/Resources/cohort/` on macOS or `<window>/resources/cohort/` on Linux, `install` also makes the window store `desktop/` with its `install.lock` and `versions/`, under the store root's lock and then its own, after the held-version and lower-version refusals and before the copy, so the window part of every later update has a store to install into; it stores no window version: a copy dragged from the macOS disk image has no window list beside it, and an AppImage's or the Debian package's window stays where it was installed until a later update stores the next one.

The store root is `ORCHBEE_INSTALL_DIR` when set, otherwise `$HOME/.local/share/orchbee` for a release build and `$HOME/.local/share/<stem>` for a development or test build, the rule the npm launcher reads too. The root is used as spelled, never resolved, so the launcher's comparison with `releasePath` holds; a relative root, or one holding `:`, `$`, a backtick, a backslash, a quote, a control character or a `.` or `..` component, refuses with `install-location`. A missing root is created with every missing folder above it as private folders, mode 0700, so no folder above it has to be private, as a home folder others may read is not; an existing root is admitted only when the person owns it, nobody else has access and, on macOS, it has no access list, and is never repaired. No component of the root may be a link, so a root reached through one refuses with `storage`. Every staged file is created with mode 0600 and the cohort's four programs, `<stem>`, `<stem>-server`, `<stem>-worker` and `<stem>-terminal-host`, get mode 0700 after their bytes are copied, as does the `<stem>-files` a test release carries, since the manifest records no file modes and the cohort admission requires the owner's execute bit. The staged Client's version check and the published copy's completion run as owned child processes in their own process group, which settles before the result; the Client's path on macOS is resolved through links, so a Client started through the command link finds its own version directory. On macOS `install`, `update` and the automatic helper capture Ctrl+C and `SIGTERM` as Windows captures Ctrl+C: the run settles its children and answers `cancelled`, exit `130`, with its document, unless a completion it stopped had already moved the pointer, which is `activation-unknown`. On Linux, and for `uninstall` on both systems, they keep their inherited action, as for `server stop`: they end the command at once with `128 + signo` and no document. A completion already started finishes under `install.lock`, a stage left behind goes with a later completion's cleanup, and every uninstall step is safe to repeat.

The PATH step is one link: `~/.local/bin/<stem>`, `~/.local/bin/orchbee` in release, naming the version's Client, `<store root>/versions/<version>/<stem>`. `~/.local/bin` is created when missing. `pathStep` is `added` when no link existed, `present` when the link already named a version of this store (it then names this version), and `replaced` when it named a version's Client in another store. Anything else at the name, a file, a folder or a link to anything that is not a version's Client, stops the step with `path-launcher` before the pointer moves and is left unchanged; `--no-path` installs without it. A replacement renames a new link over the old one, so the name never stands empty. When the `PATH` of the installing process names no `$HOME/.local/bin`, the step also adds two lines to the startup file of the shell `SHELL` names, the system's default shell, zsh on macOS and bash on Linux, standing in when `SHELL` is unset: `~/.zshenv` for zsh, and for bash on macOS the first of `~/.bash_profile` and `~/.bash_login` that exists, else `~/.profile`, the files a login shell reads, which macOS terminals start, and on Linux `~/.bashrc`, which the interactive shells its terminals start read: a comment naming the stem and `case ":${PATH}:" in *":$HOME/.local/bin:"*) ;; *) export PATH="$HOME/.local/bin:$PATH" ;; esac`. A file that already holds them is left; the lines are appended through a link; a file that cannot be read as UTF-8 or written, or another shell, gets nothing and the install still succeeds. The text output then says new terminals find the command, or, where no file holds the lines, prints the line to add, `export PATH="$HOME/.local/bin:$PATH"`. Background updates never add them. Without the PATH step, as the npm launcher runs `install --no-path`, an existing link to this store follows the version that becomes active, as best effort, and nothing is created; the `noPath` member keeps that for later `update` runs. A Session Server of a release or test cohort starts with its cohort folder appended to `PATH` when no entry names it, so the Session's terminals find that cohort's own Client by name even when no profile names `~/.local/bin`; an earlier `orchbee` on `PATH` still wins, and a development cohort's folder is never added.

On macOS and Linux a release or test Client outside the store, such as the window application's bundled Client (in `<name>.app/Contents/Resources/cohort/` on macOS) or one in an unpacked folder, hands a Session start to the store's active version by the [store start](#windows-release-installation) rule, with its log records and its reasons. It passes the `--session` it was given, as on Windows; the store's Client gets `/` as its working folder and leads a process group of its own, so an interrupt the terminal sends the Client that hands the start on never reaches it. The version folder and the Client are admitted by the private rule above: the person owns them, nobody else has access, the Client is a regular file with one link and the folder has no access list. So the window's Session runs from `<store root>/versions/<version>/`, and replacing the application leaves every program the Session starts later unchanged. A Client started through the command link or the npm launcher runs from a version folder, so it never hands a start on. When the store has no active version (no pointer or bytes that are no pointer) or its active version is lower than the bundled Client's, and the Client is a window application's bundled one, in `<name>.app/Contents/Resources/cohort/` on macOS or `<window>/resources/cohort/` on Linux, as an AppImage or the Debian package's `/opt/<stem>/` carries it, it installs its own folder by running itself as `install --format json` (when no store version is active, PATH step included) or `install --no-path --format json` (when upgrading a lower store, refreshing an existing store link and making `desktop/` without repeating the profile step), with the environment, working folder, process group and output bound the store's Client gets and no `--session`, and waits for it until 8 seconds before its start deadline (or starts it detached in the background with `outcome` `started` when joining an already-running Session). On success the rule runs once more over the store the install left and hands the start to the version it activated, and the window's later starts find that store. The diagnostic log gets one `store-install` record per install: `outcome` `installed` or `started`; `failed` with `error`, the document's error code, `other` for any other answer or `start-failed` when it could not be run, and the exit `status` when it exited; or `unfinished` when the wait ended first, and that install then keeps running and is never ended. An install that did not complete leaves the start to the Client's own cohort with reason `no-pointer` or `store-lower`, and the next start tries again; an unreadable pointer, an equal or higher active version and every other Client keep their decisions.

Uninstall's third step removes the command link while it names a version of this store and leaves anything else at that name and the folder; it also removes every copy of the stem's two PATH lines from `~/.zshenv`, `~/.bash_profile`, `~/.bash_login`, `~/.profile` and `~/.bashrc`, rewriting a linked file's target and removing a plain file left empty; `pathEntry.key` is `null` and the listing names the link. No uninstall registration exists, so its `installerEntries` member is `absent` with a `null` location. On macOS the shortcut member is the same; on Linux it is `menu-entry`, the window store's menu entry, which the third step removes after the command link, with its icons, while each of its `Exec=` lines still starts this store's window entry, and which is otherwise `absent` and left. No machine hub runs. The plan refuses a root that equals or holds the home folder, `~/.local`, `~/.local/share`, `~/.local/bin`, `~/Library`, the temporary folder or one of the system's own folders (`/Applications`, `/Library`, `/System`, `/Users`, `/bin`, `/etc`, `/home`, `/opt`, `/private`, `/sbin`, `/tmp`, `/usr`, `/var`). Every build takes `--confirm`, as every guarded command there does, and prints the listing on standard error under `text`. The process scan reads `/proc/<pid>/exe` on Linux; on macOS it reads the start path each process was given from `/bin/ps`, resolved through links, so it finds every Session process and a Client the npm launcher started, but not a Client a shell started by its bare name. A running program's files can be deleted on both systems, so the store is removed at once and never scheduled; a program still running from it keeps running from its deleted files. The npm carrier is recognized by a parent program named `node`.

After each release, the [update check workflow](../../../.github/workflows/update-check.yml) updates
the version people have now to it through `update` on a fresh runner of each macOS row, as on
Windows.

## Windows Agent sidebar rows

An Agent row is one fixed line: what names the Agent, then its provider, or the provider alone.
The file has no Agent row keys. A file that still sets `appearance.sidebar.agents` receives an
`unknown-key` diagnostic for that table, which is ignored, and starts normally.

`config check` v1 and Session configuration receipts v1 still carry the required
`agentSidebarProfile`; it is always the fixed profile
`{"rows":null,"rows_by_agent":{},"row_gap":0}`, the one an empty file published while Agent rows
were configurable, so matched cohorts read it unchanged. The TUI does not read it.
`spacesSidebarProfile` is closed canonical JSON of at most 32 KiB.

## Windows Workspace sidebar layouts

The main sidebar places Workspaces under their Projects and eligible Agents under
their exact Workspace owner, with one shared scroll window. The
[sidebar navigation model](tui-shell-layout.md#sidebar-navigation-model) owns
disclosure and keyboard navigation. The Workspace picker remains available by
opening Machines then pressing Tab.
Row clicks retain complete machine/Session/context identity even when names coincide.
`appearance.sidebar.workspaces` controls Workspace rows; their Agent children are fixed one-line rows
with no gap between them. Workspace `row_gap` applies before the next Context row and at least one
row before each group.
A Project's first Workspace sits directly under its heading.

```toml
[appearance.sidebar.workspaces]
rows = [["workspace"], [{ token = "branch", fg = "#8bc", rules = [{ starts_with = "worktree/", dim = true }] }, "git_status"], ["$issue"]]
row_gap = 0
```

Defaults are `[["workspace", "git_status"], ["location"]]`, gap 0. `location` shows the branch
unless it equals the Workspace name, otherwise the Workspace folder, the first terminal's current
directory when Server recorded one, with a local home folder shortened to `~`. An explicit `rows =
[]` keeps an identity/navigation row. The other builtin is `state_text`; `$name` reads Workspace
reports and never replaces a builtin. `state_icon` remains accepted for compatibility but paints
nothing on a Workspace row. Tokens take a style and first-match rules; icons and Git counts reject
text rules. Missing/hidden values collapse. Git counts right-align at the end of the line that
configures them, flush with the row's trailing edge on the identity line; other text shares the
remaining cells. A folded Workspace replaces its second configured line with its Agents' status
line, which wraps onto further lines until every Agent shows.

State text and the folded status line use the complete physical Agent checkpoint, independent of
seen and of list/source filters. Git reads the committed Workspace directory
on its Worker, showing nonzero ahead/behind against the local upstream. Missing
upstream supplies no counts; nonrepositories supply no Git observation. Detached
has no branch; unborn keeps its branch name. No fetch, configuration write, hook/fsmonitor execution, terminal
startup or per-draw I/O occurs. Custom `$branch`/`$git_status` stay distinct.
See the [Workspace sidebar model](machines.md#workspace-sidebar-model) for fences,
aggregation, budgets, exact activation and acceptance limits.
