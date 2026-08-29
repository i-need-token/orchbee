# Terminal Settings

## Ownership and scope

`@orchbee/web` owns separate local terminal-appearance, terminal-interaction, terminal-feedback, and macOS terminal-input
preference records, their Browser storage codecs, the Settings overlay, and the stable client-side
sources observed by projected terminal attachments. It also owns the presentation and unprivileged
request clients for the optional Desktop terminal-launch-profile and terminal-default-settings
capabilities; Electron Main owns those records, persistence, Host commit, and launch resolution. The Browser-backed preference records are local to
one Browser origin or Desktop renderer user-data partition. Neither those records nor the Main-owned
profile and terminal-default records are part of the shared Workbench document or sent to a Gateway or remote terminal.
A local Desktop composition may also inject Agent-integration management inventory and commands;
Web owns their Settings and reminder presentation, while Main retains every process and filesystem
effect.
A theme-affecting preference derives one complete runtime-theme request that crosses the active
Desktop or Gateway projection transport to Terminal Host; per-key Option-as-Alt likewise crosses
only as a semantic input decision, never as the stored preference.

`@orchbee/terminal` owns the source-neutral renderer contract that consumes the derived complete
presentation configuration. Host frame metadata remains authoritative for terminal palette,
cursor, VT state, terminal color-scheme replies, and OSC behavior. Workbench coordinates one
complete Host theme revision with renderer overlays, cursor presentation, and the application
design-token surface. The Web/Desktop composition keeps the legacy source-neutral
`defaultColorOverride` at `null`.

This feature may analyze pasted Ghostty configuration into an ephemeral migration report. It does
not apply or persist that report, read Ghostty files, mutate a Host profile, expose a complete
palette editor, make an Electron window transparent, or synchronize preferences across clients.
It also projects a read-only remote-development diagnostic. That projection neither owns an SSH
process nor infers whether the current terminal is remote.

## Appearance and interaction preference records

The feature writes the key `orchbee:web:terminal-appearance:v2`. Its value is one complete JSON
object with no unknown fields:

```ts
interface StoredTerminalAppearancePreferencesV2 {
	readonly applicationTheme: 'light' | 'dark';
	readonly backgroundOpacityPercent: number;
	readonly customBackground: `#${string}`;
	readonly customForeground: `#${string}`;
	readonly fontFamily: string;
	readonly fontLigatures: boolean;
	readonly fontSize: number;
	readonly fontWeight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
	readonly fontWeightBold: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
	readonly terminalDisplayTheme: 'host' | 'light' | 'dark' | 'custom';
	readonly terminalPadding: number;
}
```

When version 2 is absent, one valid `orchbee:web:terminal-appearance:v1` record is migrated only in
memory by adding the new product defaults. No eager migration write occurs; every later semantic
write uses version 2. Invalid version-2 data never falls back to version 1.

The font family is trimmed and contains 1–256 characters. Font size is an integer from 8 through 48.
Both weights are integer hundreds from 100 through 900. Terminal padding is an integer from 0
through 32 CSS pixels. Custom colors are exactly `#RRGGBB` and are normalized to lowercase.
Background opacity is an integer percentage from 10 through 100.

The separate strict key `orchbee:web:terminal-interaction:v1` contains only
`copyOnSelect: boolean`. It has an independent controller, persistence issue, and source-neutral
subscription. Its product default is `true`.

The complete product default is:

| Field                  | Default                                                            |
| ---------------------- | ------------------------------------------------------------------ |
| Application theme      | `light`                                                            |
| Font family            | `"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace` |
| Font size              | `13`                                                               |
| Regular font weight    | `400`                                                              |
| Bold font weight       | `700`                                                              |
| Font ligatures         | `true`                                                             |
| Terminal padding       | `0`                                                                |
| Terminal display theme | `host`                                                             |
| Custom foreground      | `#141414`                                                          |
| Custom background      | `#fcfcfc`                                                          |
| Background opacity     | `100`                                                              |
| Copy on select         | `true`                                                             |

Workbench exposes increase, decrease, and reset font-size Actions through the same controller.
Increase and decrease apply a two-point step and clamp at `8` and `48`; reset restores only the
default `13` font size and preserves every other preference. A changed Action follows the ordinary
complete-snapshot publication and persistence workflow, while a boundary or already-default
request is unchanged. The behavior is application-wide rather than per Panel.

Terminal display themes derive one complete candidate from the latest reconciled Host state without
modifying the stored custom pair:

| Theme    | Host terminal candidate                                                                                                      |
| -------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `host`   | Keep the complete current Host color scheme, defaults, nullable cursor color, and 256-color palette.                         |
| `light`  | Set scheme `light` and defaults `#141414` / `#fcfcfc`; preserve the current Host cursor color and complete palette.          |
| `dark`   | Set scheme `dark` and defaults `#d4d4d4` / `#1e1e1e`; preserve the current Host cursor color and complete palette.           |
| `custom` | Set defaults from `customForeground` / `customBackground`; preserve the current Host scheme, cursor color, and full palette. |

Changing among Host, Light, and Dark retains the Custom values. Custom color controls are editable
only while Custom is selected. Application theme, terminal display theme, complete fixed overlay
colors, and the Host-assigned theme revision form one publication transaction. A successful barrier
commits every mounted terminal before updating `data-theme` in the same JavaScript task.

## macOS Option-as-Alt preference

The Desktop renderer on macOS additionally owns `orchbee:web:macos-terminal-input:v1`. The strict
record contains only `optionAsAlt: 'off' | 'left' | 'right' | 'both'`; `off` is the product default.
Browser, Gateway, Windows, and Linux compositions do not expose the Option-as-Alt row and retain the
default; Cursor & Input remains a stable catalog destination on every platform.
Invalid or unreadable records fail to `off`, while a failed write keeps the accepted session value
and shows the existing persistence warning.

The client tracks physical `AltLeft` and `AltRight` ownership and resolves the selected side for
each admitted key event. It does not rewrite committed text or guess from the character produced by
the macOS layout. A complete input-configuration source updates every mounted projected terminal
immediately without replacing its attachment. `KeyInput.optionAsAlt` then carries only that event's
semantic decision; Terminal Host remains responsible for Ghostty's mode-dependent legacy,
modifyOtherKeys, and Kitty keyboard encoding.

## Desktop local terminal launch profiles

Only a local Electron composition injects `DesktopTerminalLaunchProfileControl`. Its initial
snapshot exposes one bounded canonical catalog, the current default identity, whether the catalog
came from the built-in fallback or durable storage, an optional persistence issue, and the current
platform's login-shell capability. Browser Preview and authenticated Gateway compositions receive
no control and show only the remaining planned Profile rows. The renderer never receives a store
path, Node error, filesystem primitive, or launch capability.

Each profile has a stable identity, a user-visible name, and exactly one command choice:

- System shell with a login-shell decision. Login mode is enabled only when Main reports support and
  is unavailable on Windows.
- Custom executable with one absolute platform path and an ordered vector of literal arguments.
  Arguments are not tokenized, quoted, interpolated, or joined into a command string.

The stable built-in profile uses a localized display label rather than persisted product copy. User
names remain exact user data and duplicate names do not merge identities. The editor can create a
new profile by copying the selected command, rename it, change its command and arguments, save the
draft, select it as default, or delete a non-default profile after confirmation. The default and
final profile cannot be deleted. Reset requires confirmation and replaces the complete catalog with
the built-in default only after Main durably commits it. Pending operations disable competing
mutations and surface bounded localized failures; invalid drafts never send a mutation. A failed
commit retains the user's draft so they can correct or retry it instead of reconstructing their
input.

Create, edit, default selection, delete, and reset use explicit controls. They do not alter a
running terminal. A newly provisioned terminal captures the default profile only once, and its
durable binding retains that concrete launch across retries and recovery. Profile resolution changes
only executable and arguments; environment, secrets, presentation, CWD, project or workspace data,
one-off profile selection, remote connections, and shell startup-file mutation remain outside this
feature. An unavailable custom executable fails visibly through the terminal lifecycle without a
system-shell fallback. The full Main model is authoritative in
[`../../desktop/docs/terminal-launch-profiles.md`](../../desktop/docs/terminal-launch-profiles.md).

## Desktop terminal defaults

Only a local Electron composition injects `DesktopTerminalDefaultSettingsControl`. Its snapshot
contains the complete bounded Host default, persistence issue, source, pending state, and bounded
failure code. The Terminal page exposes `terminal.scrollback` as a decimal byte count from `0`
through `1_000_000_000` and states that running terminals keep their current limit. Browser Preview
and authenticated Gateway compositions receive no control and retain the same row as Planned.

The renderer validates drafts before sending a complete update. Host rejection keeps the prior
value; persistence failure after Host acceptance keeps the session value visible and shows that it
was not saved. A later submission retries persistence. This client never sends a live terminal
configuration update or evicts history. The authoritative Main transaction is
[`../../desktop/docs/terminal-settings.md`](../../desktop/docs/terminal-settings.md).

## Read, commit, and recovery model

Startup reads each current local key once through injected `getItem` and parses `JSON.parse(...)`
directly into a strict Zod schema. Appearance additionally reads version 1 only when version 2 is
missing. A missing record uses the product default without a warning. A read exception, malformed
JSON, unknown field, unsupported shape, or out-of-range value uses the complete default and records
a recoverable persistence issue; partial recovery is forbidden.

One semantic update follows this order:

1. Validate a complete candidate preference record.
2. Derive an immutable complete preference snapshot, source-neutral presentation
   configuration, and, when theme-affecting fields changed, a complete runtime-theme candidate.
3. Commit the in-memory snapshot. Publish presentation configuration only when those derived values
   changed, and start one latest-wins runtime-theme barrier when theme-derived values changed.
4. Attempt one write of the complete preference record.

Font family, size, regular and bold weight, ligatures, padding, and opacity update mounted terminal
renderers live without replacing an attachment. The independent interaction controller follows the
same validate, session-commit, publish, and write ordering for `copyOnSelect`.

For a runtime-theme barrier, every mounted projected terminal first preflights its renderer
overlays. A failed preflight sends no Host mutation. A ready participant then completes the Host
compare-and-set or reconciliation and waits for a staged frame carrying the exact committed
`themeRevision`. After every participant returns a prepared handle, the controller synchronously
checks all handles, commits every terminal store/overlay/cursor pair, and changes the application
theme. Any failure, cancellation, supersession, hidden/suspended delay, or lost eligibility keeps
the prior complete application presentation visible and aborts all otherwise prepared handles. A
Host revision that committed before a peer failed remains quarantined rather than being rolled back
or partially displayed; a later theme request, participant registration change, or disposal
supersedes that stage. With no mounted participant, `data-theme` applies immediately.

The visual transaction does not include Browser storage or the in-memory preference snapshot. A
write failure therefore retains the accepted session preference as before, and a presentation
failure does not rewrite the stored choice even though the previous complete theme remains visible.

Synchronous listener reentry is serialized as immutable presentation snapshots in first-in,
first-out order. A newer committed revision cannot be overwritten by an older revision's later
storage write. Disposal during publication stops the remaining callbacks and prevents any pending
publication or persistence work.

An invalid candidate does not change memory, theme, storage, or subscribers. A valid no-op does
nothing when persistence is healthy. A write exception does not roll back the session value; the
snapshot reports that the setting is applied for this session but is not saved. The next valid
update retries a complete write and clears that issue after success. One terminal preparation
failure retains every mounted terminal and the application surface at their previously committed
complete presentation without rolling back the preference.

Reset behaves as follows:

- Different preferences become the complete product default, write once, and publish at most one
  terminal presentation snapshot when terminal-derived values changed.
- Default preferences with a persistence issue rewrite the default record once without publishing
  a terminal presentation snapshot.
- Default preferences with healthy persistence are unchanged and perform no write or publication.

Disposal clears subscribers and rejects later semantic updates as disposed. No late update can
change theme, storage, or a presentation source after disposal.

## Effective terminal colors and opacity

Host runtime theme replacement is the only color mutation path used by this Settings feature. After
commit, its frame metadata and resolved palette cells remain the renderer authority. The renderer's
`defaultColorOverride` is always null: default tokens use the Host default foreground/background;
explicit RGB, projected ANSI/palette colors, `none`, and explicit black retain their Host-projected
value. Inverse swaps the already resolved display roles. The default underline uses the Host
foreground, and the cursor uses the Host cursor color or its Host-defined foreground fallback.

Background opacity is derived from `backgroundOpacityPercent`; `allowTransparency` is not a second
user preference. At 100%, renderer clear and the terminal root are opaque. From 10% through 99%,
the renderer clears the default background transparently and the terminal root presents the
effective background with the selected alpha. Explicit backgrounds, inverse cells, selection, and
search highlights remain opaque. The implementation must not use whole-root CSS `opacity`, because
terminal text must remain opaque.

## Overlay and focus model

The Settings experience is an application-owned full-window overlay, not a route. Keeping the
Workbench mounted preserves terminal surface and attachment identity. On first open it captures a
still-connected active element and focuses Settings search. Repeated open is unchanged and does not
replace the return target. The Workbench subtree is genuinely inert while Settings is open, so its
terminal and Action shortcuts cannot consume Settings input. The scrolling content viewport remains
in the sequential focus order and draws an inset focus indicator, so a category stays
keyboard-scrollable even when every setting control is a disabled Planned preview.

Back closes Settings. Escape first clears a non-empty Settings search and closes Settings only when
the query is empty; closing restores the captured element, then the Settings Action target, then the
Workbench root as fallbacks. Desktop may inject semantic Agent-integration, Browser-access, local
shell-profile, and terminal-default controls.
The existing Browser-access dialog remains above Settings; closing it returns focus to its Settings
entry, while Settings remains open. Browser and Gateway compositions do not receive this Desktop
capability bundle.

The interaction structure follows the accepted Cursor and Coze references without copying either
brand: a compact Settings rail owns Back, search, icon-led product-outcome categories, and the current
selection; a centered scrolling content column owns one category title and a stable sequence of
section labels, low-chrome grouped rows, descriptions, and right-aligned controls. Content-heavy
diagnostics, migration input, and shell setup keep the same surface, radius, type, spacing, and
semantic-border vocabulary instead of inventing category-specific cards. At narrow widths the rail becomes
a horizontally scrollable top navigation and setting controls stack below their copy. Browser-backed
appearance, input, shortcut, and feedback changes apply immediately; shell-profile drafts require
explicit Save because Main-owned durable catalog mutation is a separate transaction. Reference
branding, copy, categories, and pixel geometry are not compatibility targets.

## Settings information architecture and planned catalog

The Settings rail groups product outcomes instead of exposing implementation packages. The current
information architecture is stable enough for future settings to enter without creating another
top-level surface:

| Group         | Categories                                                              |
| ------------- | ----------------------------------------------------------------------- |
| Application   | General, Appearance                                                     |
| Terminal      | Text, Colors, Cursor & Input, Interaction, Terminal                     |
| Workspace     | Windows & Tabs, Profiles, Keyboard Shortcuts                            |
| Integrations  | Agent Integrations, Shell Integration, Remote Development               |
| Data & Access | Ghostty Migration and the capability-gated Desktop Browser Access entry |

The appearance theme, terminal font family and size, regular and bold weights, ligatures, padding,
effective default colors, default-background opacity, copy-on-select, Reset, macOS Option-as-Alt,
keyboard shortcuts, Agent integrations, Shell integration, remote diagnostic, migration report, Browser-access entry,
capability-gated Desktop launch-profile editor, and capability-gated Desktop scrollback default
retain their existing behavior and ownership. The remaining General, Appearance, Text, Colors, Cursor & Input,
Interaction, Terminal, Windows & Tabs, and Profile appearance, connection, and automation rows form
a localized planned catalog derived from recurring settings across Ghostty, WezTerm, Kitty,
Alacritty, Windows Terminal, iTerm2, Tabby, Konsole, GNOME Terminal, and Foot. This catalog covers
startup and restore policy, window presentation, typography and shaping, palettes and contrast,
cursor and keyboard protocols, clipboard and mouse behavior, shell and emulation policy, history
and feedback, windows and tabs, and profile capabilities outside the accepted launch-profile scope.
Without the Desktop terminal-default capability, the scrollback row remains part of that planned
catalog and cannot mutate Host state.

Planned rows are explicitly marked, expose only disabled preview controls, and never read or write
a preference. Implementing one requires its own accepted behavioral authority, model, persistence
boundary, platform capability, and verification. A visible row is therefore product direction, not
a compatibility or availability claim.

Neutral grouped settings use one rounded subtle surface with no outer stroke. Only row separators,
real input boundaries, focus indicators, and semantic status or danger notices retain borders. This
keeps every category in one low-chrome visual system while preserving meaningful state boundaries.

## Settings discovery and navigation model

Settings search is a global discovery surface, not an in-page filter. Its deterministic catalog
contains every available category plus each current or explicitly planned setting with a stable
identifier, owning category, localized label, optional section, description, and search keywords.
Capability-gated categories and settings enter the catalog only when their composition capability is
present. The catalog never reads a preference value and search never changes a controller, storage,
Host, terminal attachment, migration report, or remote environment.

The query is trimmed, Unicode-normalized, split into non-empty terms, and compared
case-insensitively in the effective locale. Category results match their category label. Setting
results require every term to match the setting label, section, description, or explicit keyword.
Direct label matches rank before section, description, and keyword matches; available settings get a
bounded priority over nearby Planned matches, every Planned result is labeled, and original
information-architecture order breaks remaining ties. Empty query preserves the grouped navigation rail. A non-empty query
replaces that rail with one global result list while the current content page remains complete and
unchanged. Zero matches render a localized empty state in the rail rather than blanking the content
page. Search results remain one labeled listbox; the zero-match projection contains one disabled,
unselected option with a polite live announcement, preserving the combobox relationship without
creating an actionable result.

The owned interaction state is one of:

| State                    | Query     | Result selection                              | Rail projection       |
| ------------------------ | --------- | --------------------------------------------- | --------------------- |
| Browsing                 | Empty     | None                                          | Grouped categories    |
| Searching with results   | Non-empty | One bounded index, initially the first result | Global search results |
| Searching without result | Non-empty | None                                          | Localized empty state |

Transitions are deterministic:

| Input                      | Guard                          | Result                                                                                           |
| -------------------------- | ------------------------------ | ------------------------------------------------------------------------------------------------ |
| Edit query                 | Any                            | Recompute the global projection and select its first result when present.                        |
| Arrow Down or Arrow Up     | Searching with results         | Move the bounded selection by one; the current content page and every preference remain stable.  |
| Enter                      | Searching with results         | Activate the selected result.                                                                    |
| Pointer activation         | A visible result               | Activate that result.                                                                            |
| Activate a category result | Any                            | Select the category, clear the query, restore grouped navigation, and scroll content to its top. |
| Activate a setting result  | Its category remains available | Select the category, clear the query, restore grouped navigation, and reveal its stable target.  |
| Escape                     | Query is non-empty             | Clear search, restore grouped navigation, keep Settings open, and retain the current category.   |
| Escape                     | Query is empty                 | Follow the Settings overlay close and focus-restoration contract.                                |

The search input retains keyboard focus while moving or activating a result. Result activation may
scroll only the Settings rail or content viewport; it never focuses a setting control or invokes its
action. Changing category through the normal rail clears no user data and starts at the top of that
category.

The adopted reference projection was observed in Cursor 3.14.7 on macOS on 2026-08-17. The adopted
scope is global category-and-setting discovery, rail replacement during a query, stable right-side
content while typing, explicit empty results, Arrow navigation, Enter activation, Escape clearing,
query clearing after activation, category restoration, and target reveal. Cursor branding, copy,
catalog contents, fuzzy-search implementation, result limits, pixel geometry, animation timing, and
ranking ties are explicitly excluded.

## Desktop Agent integrations

Only a local Electron composition injects `AgentIntegrationManagementControl` and
`AgentIntegrationReminderController`. Browser Preview and authenticated Gateway clients omit the
Agent Integrations category, its search entries, and the reminder. The renderer consumes one
canonical ten-entry snapshot and displays curated Agent names, bounded states and failures, and the
shared privacy explanation. It receives no Agent command output, executable path, adapter resource
path, filesystem capability, or remote-host authority.

Opening Settings, selecting Agent Integrations, rendering rows, and searching are read-only. Refresh
requests a new read-only Main inventory. Enable and Disable are buttons, not optimistic switches;
one click sends one semantic request and the row follows Main's re-inspected snapshot. Pending work
disables competing changes. A conflict, unsupported/manual-only entry, ambiguous inspection, or
unavailable local Agent cannot be enabled. Remote hosts are explained as manual-only and the
renderer never writes terminal input or attempts remote setup.

The non-modal reminder is eligible only for a locally available, `managed-local`, issue-free entry
whose state is `not-installed` or `disabled`. Canonical order selects the first eligible entry, and
the controller offers at most one reminder per application session. Rendering and **Review
integration** perform no integration mutation; Review hides the reminder and opens this Settings
category. Explicit close persists that one Agent ID in the strict, bounded
`orchbee:web:agent-integration-reminders:v1` record. A Settings disable records the same dismissal
before it requests Main; a verified successful enable removes it. Missing or invalid storage falls
back to no dismissals, and write failure retains the current-session behavior.

Desktop Main's exact effects, reconciliation, conflicts, rollback, and remote exclusion are
authoritative in the [Desktop Agent integration guide](../../desktop/docs/agent-integrations.md).

## Remote-development diagnostic

The Remote development category documents the Desktop launch baseline
`TERM=xterm-256color` and exposes one constant POSIX-shell diagnostic command. The command reports a
version marker, the observed `TERM`, `COLORTERM`, and `TERM_PROGRAM`, whether TERM matches the
baseline, and exactly one terminfo outcome:

| Outcome                        | Meaning                                                |
| ------------------------------ | ------------------------------------------------------ |
| `TERMINFO=term-unset`          | The remote shell has no TERM value.                    |
| `TERMINFO=unsafe-term`         | TERM is not safe to print or pass to `infocmp`.        |
| `TERMINFO=infocmp-unavailable` | The remote shell cannot find an `infocmp` executable.  |
| `TERMINFO=available`           | `infocmp "$TERM"` succeeds.                            |
| `TERMINFO=missing`             | `infocmp` exists but cannot resolve the observed TERM. |

The command contains no user interpolation and runs its helper and temporary value inside one
subshell. It only reads environment values, prints values of at most 128 characters that match a
conservative display-safe character set, checks `command -v infocmp`, and evaluates
`command infocmp`'s exit status. Unsafe or oversized values receive a stable marker instead of being
echoed as potential terminal control input. It does not invoke SSH, access a network, install
terminfo, create a directory, read a configuration file, write remote state, or leave a helper
function in the user's interactive shell.
Settings does not execute or paste the command; the read-only field uses normal text selection so
the user remains responsible for choosing where and whether to run it.

The explicitly sourced shell integration defines an `ssh` wrapper only when `ssh` is still a file,
no earlier alias or function owns the name, a private local Agent-observation socket exists, and
stdin and stdout are interactive terminals. It leaves configuration, control, forwarding-only, and
version/query modes unchanged. An eligible call preserves every user argument and adds only
`StreamLocalBindMask=0177`, `StreamLocalBindUnlink=yes`, and a reverse Unix-socket forward from a
random `/tmp/orchbee-agent-v2-*.sock` on the remote host to the local Desktop socket. OpenSSH
authentication, explicit arguments, and user configuration remain authoritative. Orchbee adds no
`SendEnv` rule, persists no host or credential, installs no remote file, and maintains no remote
cache. A standard requested SSH PTY still carries TERM according to OpenSSH behavior. OSC 7
hostnames, process titles, and terminal output are not remote-identity authority.

## Shell integration and completion reminders

The Shell integration category explains that Desktop automatically loads the Orchbee-owned
integration for supported new local Zsh, non-login Bash, and Fish terminals. Exact `source`
commands remain available as a manual fallback for unsupported launch forms and other clients. The
surface never reads, creates, edits, or backs up a shell startup file and never executes a displayed
command. Zsh uses `preexec` and `precmd`; Fish uses `fish_preexec` and `fish_postexec`; Bash requires
version 4.4 or newer. The Bash 3.2 shipped by macOS is explicitly unsupported. Removing a
user-added `source` line and starting a new shell rolls back only that manual fallback.

Desktop supplies `ORCHBEE_SHELL_INTEGRATION_DIR` from its trusted application-resource path for
new POSIX terminals. After launch-profile selection, supported local launch forms use a one-shot
Zsh, Bash, or Fish loader that restores the shell's normal startup path before sourcing the shared
integration; unsupported forms continue unchanged with the manual fallback available. The scripts
emit standard OSC 133 command boundaries and OSC 7 current-directory state without transporting
command text. Ambient environment cannot replace the resource path, and automatic loading never
changes running or restored terminals, Windows terminals, remote connections, or user startup
files.

For an explicitly installed Agent observation adapter, the integration also enables the
eligible SSH reverse-socket path described above; a remote adapter writes bounded OSC 777
correlation hints directly to resolved direct, tmux, Zellij, or nested-mux outer TTYs. Terminal Host
projects only bounded semantic signals.

Terminal feedback uses the separate key `orchbee:web:terminal-feedback:v2`, whose strict JSON
value contains `agentNotificationMode` (`never` or `hidden`), `completionReminderMode` (`never`,
`unfocused`, or `always`), and `completionReminderMinimumSeconds` (an integer from 1 through
3,600). The defaults are `never`, `never`, and 5 seconds, respectively. A valid version-1
completion-reminder record migrates with Agent notifications off. Invalid or unreadable storage
falls back to the complete default with a recoverable warning; a failed write retains the
current-session value and reports that it was not saved. Reset restores all three defaults.

Each client deduplicates by terminal identity and positive completion sequence, ignores facts more
than 60 seconds old or over 5 seconds in the future, and limits system notifications for one
terminal to one every 30 seconds. `unfocused` requires the document or that terminal root to lack
focus. Browser notification permission can be requested only by the dedicated Settings button;
terminal output, startup, storage restoration, and configuration analysis never request it. A
qualifying event still presents a localized status inside the terminal when the system capability
is unavailable. The notification contains duration and exit status only—never command text,
arguments, directory, environment, output, or user identity.

Agent status notifications use a separate exact semantic-event subscription rather than terminal
screen inference. The setting defaults to Off; `hidden` admits attention, successful outcome, and
failed outcome events only when every navigable attachment is currently hidden by the local Tab,
split/child presentation, or inner tmux/Zellij pane. Existing system permission must already be
granted—the event path never requests it.

Copy is localized and curated from bounded Agent identity
and event kind; it never includes prompts, commands, tool content, output, paths, or error text.
Each semantic event identity is considered once. Activating the notification focuses the
application and selects its exact Tab and pane but cannot answer a question or grant approval.

The current Workbench unmounts terminal Surfaces that belong entirely to inactive tabs. A mounted
but unfocused terminal can remind immediately, and reattachment can consider a completion fact
within the 60-second freshness window. This is not a claim of complete background monitoring:
completion while a Surface remains unmounted beyond that window does not produce an immediate
notification. A future all-Tab command-completion monitor requires a separate lightweight
Workbench/Host event subscription rather than hidden renderers. Agent observation already uses
such an application-level subscription and does not share this command-completion limitation;
fully disconnected SSH delivery remains out of scope.

## Ghostty migration report

The Settings migration category analyzes user-pasted text without reading a file, invoking
Ghostty, logging the source, changing a preference, or writing Browser storage. The source and its
report exist only while that category is mounted; leaving the category or closing Settings clears
both. There is no Apply or Import action.

The analyzer is a deliberately bounded compatibility projection of Ghostty revision
`91f66da24527fa02d92b5fd0b41cd020f553a64c`, not a Ghostty configuration parser. It recognizes
newline-delimited `key = value` assignments, surrounding whitespace, whole-line comments, CRLF,
the first equals sign, and one pair of surrounding double quotes. It does not load `config-file`,
resolve themes, merge files, run commands, or model CLI precedence and platform defaults.

Every non-empty, non-comment line produces exactly one source-ordered finding:

| Finding   | Meaning                                                                                                                    |
| --------- | -------------------------------------------------------------------------------------------------------------------------- |
| Supported | The normalized value has one lossless current Orchbee setting target; it has not been applied.                             |
| Manual    | A nearby user outcome exists, but the Ghostty value or combined semantics cannot be represented safely.                    |
| Ignored   | The line is superseded, resets a source default, or is outside the migration whitelist; an unknown key may also be a typo. |
| Invalid   | The bounded analyzer cannot interpret the line or value. Other valid lines still receive findings.                         |

Only these final effective values can be Supported:

| Ghostty key           | Orchbee target       | Supported source values                                                                                          |
| --------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `font-family`         | Terminal font family | Exactly one active non-empty family of at most 256 characters without a CSS list delimiter                       |
| `font-size`           | Terminal font size   | A decimal value that is an integer from 8 through 48                                                             |
| `foreground`          | Custom foreground    | Three- or six-digit hexadecimal RGB, with or without `#`; requires Custom display theme                          |
| `background`          | Custom background    | Three- or six-digit hexadecimal RGB, with or without `#`; requires Custom display theme                          |
| `background-opacity`  | Background opacity   | A value from 0.1 through 1 whose percentage is an integer, unless effective `background-opacity-cells` is `true` |
| `macos-option-as-alt` | Use Option as Alt    | `false`, `true`, `left`, or `right`, mapped to Off, Both, Left, or Right                                         |

Later supported scalar assignments supersede earlier accepted assignments for the same key within
the pasted text. `font-family` retains Ghostty's empty-value reset and repeatable fallback-list
meaning; multiple active families are Manual instead of being guessed into a CSS list. Named
colors, named or light/dark theme files, `window-theme`, key bindings, working directories,
included configuration, font-style details, and cell-opacity semantics are Manual. Other keys are
Ignored with a non-whitelist diagnostic.

A supported color finding identifies one lossless value target, not a complete theme conversion.
The report tells the user to select Custom display theme and preserves the other current Orchbee
Custom color; it never invents a counterpart color that was absent from the pasted text.

Analysis rejects the complete source rather than truncating it when it exceeds 262,144 JavaScript
UTF-16 code units, 4,096 lines, or 4,096 code units on one line. These are Orchbee resource bounds,
not claims about Ghostty's byte limits.

## Required evidence

- Storage tests cover missing, malformed, unknown, version-1 in-memory migration, invalid version-2
  precedence, out-of-range, read-throw, write-fail then success, Reset repair, and healthy no-op behavior.
- Source tests prove complete single presentation publication, independent interaction publication,
  live attachment updates, application-theme isolation, terminal-setting isolation, stable
  subscription, and disposal.
- macOS input tests cover strict storage recovery, all four Option-side values, live source
  publication, physical-side resolution, mounted-terminal updates, and Host encoding with the
  semantic flag both disabled and enabled.
- Renderer tests cover Host and override modes, explicit black/RGB/palette, inverse, underline,
  cursor, Canvas 2D and WebGPU alpha, and unchanged Host metadata.
- Deterministic search tests cover catalog identity and capability gating, localized category,
  section, label, description, and keyword matches, direct-match ranking, multi-term normalization,
  stable ordering, and empty results. Component tests cover both locales, global cross-category
  results, stable content during a query, pointer and keyboard activation, layered Escape, target
  reveal, the disabled empty-result option, keyboard-scrollable content, invalid drafts, immediate
  updates, Reset, read-only remote diagnostics, migration analysis and disposal, inertness,
  idempotent open, focus restoration, and nested Browser-access behavior. Story browser review covers
  every category and populated, Planned, and empty search projections in both light and dark themes.
- Terminal-launch-profile protocol and client tests cover strict snapshots, correlation, one pending
  mutation, malformed records, failure projection, and disposal. Settings component tests cover
  literal custom executable arguments, the future-terminal boundary, create and default selection,
  delete constraints, reset confirmation, platform capabilities, and localized failures.
- Terminal-default-settings protocol, Main controller, store, broker, renderer client, composition,
  and component tests cover strict bounds, fourteen-port bootstrap ordering, Host-first commit, Host
  rejection, session-visible write failure, retry, future-terminal-only capture, capability gating,
  localized validation, and visible failures. The TLA+ model checks no live eviction.
- Terminal-feedback model tests cover default-off behavior, strict persistence recovery, duration
  and focus policy, stale and future facts, sequence deduplication, notification rate limiting,
  explicit permission requests, and disposal. Component tests cover both locales, all three manual
  source commands, the Bash boundary, reminder controls, permission state, and Reset.
- Agent-integration protocol, Main adapter/controller/broker, renderer client, composition, storage,
  Settings, and reminder tests cover canonical ordering, read-only startup, one pending mutation,
  exact-entry conflicts and disable, reconciliation, malformed messages, explicit user actions,
  per-Agent dismissal, session offer limits, remote exclusion, localization, and no automatic
  mutation. The TLA+ model checks serialization, precommit preservation, reconciliation, reload,
  and disposal safety.
- Migration analyzer tests cover accepted syntax, exact target boundaries, scalar supersession,
  font resets and fallback lists, opacity-cell interaction, malformed lines, resource limits, and
  one-finding-per-assignment ordering without depending on React or storage.
- Browser, Gateway, and Desktop composition tests prove the shared sources, local-only profile
  capability, and Settings entry; a
  Desktop Electron witness proves an existing projected terminal stays attached while Settings is
  opened and closed. The current macOS system witness additionally starts a temporary loopback
  `sshd` with isolated keys and configuration, then proves an ordinary SSH PTY receives
  `TERM=xterm-256color`, resolves its terminfo entry, and does not receive `COLORTERM` or
  `TERM_PROGRAM` without explicit forwarding. A separate macOS Electron witness sources the
  shipped Zsh resource, observes one real semantic completion and localized reminder, and proves
  the projected attachment identity is unchanged.
