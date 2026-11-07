# Terminal Settings

## Ownership and scope

Web owns Settings, drafts and client presentation sources. Desktop common values come from the
[device preference authority](../../../crates/workbench-protocol/docs/user-preferences.md) shared
with TUI through the Session's Server. Common values never write back to browser storage.
Seven host appearance leaves (fonts, weights, ligatures, pixel padding and opacity) and macOS
Option-as-Alt remain renderer-local. Every locale, appearance, interaction and feedback controller
requires the Session's preferences; none keeps a browser record of a common value.

The Session's Server and Worker own launch adaptation, terminal defaults and Agent integration effects.
Preferences are not Workbench content and never synchronize to remote devices.
Host metadata remains authoritative for VT state, palette, cursor and OSC behavior.
A theme update sets the renderer's source-neutral defaultColorOverride, as the
[Web README](../README.md) describes. Option-as-Alt crosses only as a semantic input decision.
Mounted geometry pins the first presentation configuration while forwarding live theme registration;
the mounted terminal still participates in later theme transactions and releases that registration on disposal.
Desktop supplies configured Host default colors separately from the active theme. Choosing Host restores
those colors even after renderer reload; it preserves the current palette and cursor configuration.
Remote diagnostics neither own SSH nor mutate remote state.

## Application locale preference

General offers System, English and 简体中文. System is a persisted choice following the client's
declared locale inputs. Confirmed Desktop snapshots update document lang/dir and product surfaces
without reload. User data, paths, commands and shared content are never translated.

The locale is the shared `locale` preference. A selection is sent to the Session and applies when
its confirmed snapshot arrives; save failures appear in the shared status. No browser record holds
the locale, and no earlier browser record is read or deleted.

## Appearance and interaction preferences

Application theme, the Light and Dark theme slots, terminal display theme and the two custom
colors are shared preferences. The
appearance controller writes only the seven host leaves to
`orchbee:web:terminal-host-appearance:v1`, because fonts, weights, ligatures, padding and opacity
fit this device's display. Its value is one complete JSON object with no unknown fields:

```ts
interface StoredTerminalHostAppearanceV1 {
	readonly backgroundOpacityPercent: number;
	readonly fontFamily: string;
	readonly fontLigatures: boolean;
	readonly fontSize: number;
	readonly fontWeight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
	readonly fontWeightBold: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
	readonly terminalPadding: number;
}
```

The record accepts only that complete shape. Missing data uses defaults; malformed or invalid data
uses defaults and reports an issue. No other key is read and no historical fields are synthesized.

The font family is trimmed and contains 1–256 characters. Font size is an integer from 8 through 48.
Both weights are integer hundreds from 100 through 900. Terminal padding is an integer from 0
through 32 CSS pixels. Custom colors are exactly `#RRGGBB` and are normalized to lowercase.
Background opacity is an integer percentage from 10 through 100.

Copy on select and trimming Agent tail blanks are the shared `copyOnSelect` and
`trimTrailingBlankRowsForAgents` preferences. Their controller keeps its own subscriptions and
publishes interaction configuration when a confirmed snapshot changes either value.

The complete product default is:

| Field                  | Default                                                            |
| ---------------------- | ------------------------------------------------------------------ |
| Application theme      | `system`                                                           |
| Light theme            | `orchbee-light`                                                    |
| Dark theme             | `orchbee-dark`                                                     |
| Font family            | `"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace` |
| Font size              | `13`                                                               |
| Regular font weight    | `400`                                                              |
| Bold font weight       | `700`                                                              |
| Font ligatures         | `true`                                                             |
| Terminal padding       | `0`                                                                |
| Terminal display theme | `theme`                                                            |
| Custom foreground      | `#141414`                                                          |
| Custom background      | `#fcfcfc`                                                          |
| Background opacity     | `100`                                                              |
| Copy on select         | `false`                                                            |
| Trim Agent tail blanks | `true`                                                             |
| Resume Agents          | `true`                                                             |

### CLI Agent trailing-blank-row preference

The Terminal History setting exposes “Trim trailing blank rows for CLI agents”. It is enabled by
default and updates existing terminal presentations without recreating their Host attachments.
Turning it off immediately restores the exact projected VT grid.

The application derives one effective presentation policy from the stored preference and the
Server's Agent observation for that presentation identifier. It is `true` only while the preference
is enabled and the composition's Workbench Agent source holds an observation for that presentation.
A composition without an Agent source, a source that cannot be observed, and a terminal without an
observed Agent are all safe `false` results. Detection belongs to the Server; the composition does
not inspect terminal output or guess process names.
This provider-neutral decision is passed to `@orchbee/terminal`, which owns the bounded display
window and its fallback behavior; neither layer changes PTY geometry or retained history.

A change is sent to the Session and reaches installed presentations when its confirmed snapshot
arrives. Reset restores both interaction defaults. Deterministic controller tests cover default-on
reset and publication of confirmed changes. Derived-source
tests cover Agent start, end, replacement, missing capability, preference disablement, and disposal;
projected-terminal evidence owns the row-classification and Host-window boundaries.

### Agent resume preference

The Terminal History setting also exposes “Resume Agents after restart”, the shared
`resumeAgentsOnRestore` preference. The row reads and writes it through the shared user-preference
client, not the interaction controller, because no installed presentation consumes it. The Server
reads it at its next restart and before each checkpoint capture; the row has no capability gate.
The [Server contract](../../orchbee-server/docs/agent-session-resume.md) owns what it records and
resumes.

Workbench exposes increase, decrease, and reset font-size Actions through the same controller.
Increase and decrease apply a two-point step and clamp at `8` and `48`; reset restores only the
default `13` font size and preserves every other preference. A changed Action follows the ordinary
complete-snapshot publication and persistence workflow, while a boundary or already-default
request is unchanged. The behavior is application-wide rather than per Panel.

Terminal display themes derive one complete candidate from the latest reconciled Host state without
modifying the stored custom pair:

| Theme    | Host terminal candidate                                                                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `theme`  | Take the active color theme's scheme, default colors, cursor, selection, and ANSI 0–15; keep the current Host palette from index 16. |
| `host`   | Keep the complete current Host color scheme, defaults, nullable cursor color, and 256-color palette.                                 |
| `custom` | Set defaults from `customForeground` / `customBackground`; preserve the current Host scheme, cursor color, and full palette.         |

Changing between Theme and Host retains the Custom values. Custom color controls are editable
only while Custom is selected. Application theme, terminal display theme, complete fixed overlay
colors, and the Host-assigned theme revision form one publication transaction. A successful barrier
commits every mounted terminal before updating `data-theme` in the same JavaScript task.

## macOS Option-as-Alt preference

The Desktop renderer on macOS additionally owns `orchbee:web:macos-terminal-input:v1`. The strict
record contains only `optionAsAlt: 'off' | 'left' | 'right' | 'both'`; `off` is the product default.
Windows and Linux compositions do not expose the Option-as-Alt row and retain the
default; Cursor & Input remains a stable catalog destination on every platform.
Invalid or unreadable records fail to `off`, while a failed write keeps the accepted session value
and shows the existing persistence warning.

The client tracks physical `AltLeft` and `AltRight` ownership and resolves the selected side for
each admitted key event. It does not rewrite committed text or guess from the character produced by
the macOS layout. A complete input-configuration source updates every mounted projected terminal
immediately without replacing its attachment. `KeyInput.optionAsAlt` then carries only that event's
semantic decision; Terminal Host remains responsible for Ghostty's mode-dependent legacy,
modifyOtherKeys, and Kitty keyboard encoding.

## Desktop terminal launch profiles

Only the Desktop composition injects `TerminalLaunchProfileControl`, filled from the
Session's shared preferences, whose `launchProfiles` value the TUI edits too and the
[shared preference inventory](../../../crates/workbench-protocol/docs/user-preferences.md) owns. Its
snapshot exposes one bounded canonical catalog and the current default identity; it is the built-in
catalog only until the first read and never carries a persistence issue. A composition without the
control omits the Profiles category. The renderer never receives a store
path, Node error, filesystem primitive, or launch capability.

Each profile has a stable identity, a user-visible name, and exactly one command choice:

- System shell with a login-shell decision. Login mode is enabled only when the window's connection
  bootstrap reports support and is unavailable on Windows.
- Custom executable with one absolute platform path and an ordered vector of literal arguments.
  Arguments are not tokenized, quoted, interpolated, or joined into a command string.

The stable built-in profile uses a localized display label rather than persisted product copy. User
names remain exact user data and duplicate names do not merge identities. The editor can create a
new profile by copying the selected command, rename it, change its command and arguments, save the
draft, select it as default, or delete a non-default profile after confirmation. The default and
final profile cannot be deleted. Reset requires confirmation and replaces the complete catalog with
the built-in default only after the Server commits it. Pending operations disable competing
mutations and surface bounded localized failures; invalid drafts never send a mutation. A failed
commit retains the user's draft so they can correct or retry it instead of reconstructing their
input.

Create, edit, default selection, delete, and reset use explicit controls. They do not alter a
running terminal. Each change saves the complete catalog under the preference source the window last
observed, so a change another Client made in between is refused as `source-changed` rather than
overwritten, and a draft begun on an older source is refused without being sent. A created profile
follows its source and takes the first free `profile-{n}` identity, as in the TUI. The Server
decides what this machine can launch and refuses a relative executable as an invalid profile. The
Worker launches the default profile for each terminal created after the save. The Workbench's New
Tab button offers each saved profile for one Tab, as the [Web README](../README.md) describes for
`server-workbench-tabs.ts`, without changing the default. Profile resolution changes only executable
and arguments; environment, secrets, presentation, CWD, project or workspace data, remote
connections, and shell startup-file mutation remain outside this feature.

## Desktop terminal defaults

Only the Desktop composition injects `TerminalDefaultSettingsControl`, filled from the
Session's shared `scrollbackBytes` preference. The Terminal page exposes `terminal.scrollback` as a
decimal byte count from `0` through `1_000_000_000` and states that running terminals keep their
current limit. The row commits through the shared draft, and the TUI shows the same value in
megabytes. The Worker gives the value to each terminal it launches after the save, so no Host
application step can fail, and existing terminal history is unchanged. The control only reads:
save failures show in the shared status, and the row warns that scrollback is unavailable until the
first preference snapshot arrives. A composition without the control omits this row. The Terminal category remains visible only when that
composition supplies another current Terminal setting.

## Read, commit, and recovery model

Desktop common updates use confirmed shared persistence. A draft freezes its source on first edit;
stale source, invalid storage and write refusal preserve the draft and last confirmed runtime value.
Reload explicitly discards the draft. Reset sends null, restores defaults and claims the selected
keys. Unknown outcomes require rereading, never automatic replay. Pending controls are disabled;
Enter followed by blur commits at most once.

System appearance follows a live prefers-color-scheme subscription. Custom colors are enabled only
for Custom terminal theme. Sound defaults off and selects silent system notification delivery.

The local record algorithm below applies only to the host appearance leaves, keyboard shortcuts and
macOS input.
Strict codecs reject unknown fields and accept only the current shape at one v1 key.
Missing uses defaults; invalid or unreadable sources expose a recoverable issue.

One semantic update follows this order:

1. Validate a complete candidate preference record.
2. Derive an immutable complete preference snapshot, source-neutral presentation
   configuration, and, when theme-affecting fields changed, a complete runtime-theme candidate.
3. Commit the in-memory snapshot. Publish presentation configuration only when those derived values
   changed, and start one latest-wins runtime-theme barrier when theme-derived values changed.
4. Attempt one write of the complete preference record.

Font family, size, regular and bold weight, ligatures, padding, and opacity update mounted terminal
renderers live without replacing an attachment.

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

Desktop common-write admission can notify subscribers synchronously. Before committing
the host-only part of a mixed update, the appearance controller rechecks its lifetime
and local revision. A newer host edit wins. Disposing the local controller does not
cancel a common write already admitted by the shared client.

Local-storage writes for keyboard shortcuts, host appearance and macOS input share the
application-private `PreferenceWriteQueue`. Common device values use their confirmed shared
persistence path instead.
Each local-storage owner commits its complete session value before storage, serializes synchronous
writes, and coalesces pending reentrant writes to the latest value.
Only the current live revision may publish a write result. A nested update whose write has not
completed reports `applied-session-only`; disposal drops queued work. Validation, storage keys
and failure mapping remain with each preference owner. Deterministic controller traces
cover write ordering, coalescing, failure, and disposal; this mechanism does not synchronize tabs.

An invalid candidate does not change memory, theme, storage, or subscribers. A valid no-op does
nothing when persistence is healthy. A write exception does not roll back the session value; the
snapshot reports that the setting is applied for this session but is not saved. The next valid
update retries a complete write and clears that issue after success. One terminal preparation
failure retains every mounted terminal and the application surface at their previously committed
complete presentation without rolling back the preference.

Reset restores the host leaves through this algorithm and sends null for the six common
appearance keys, the two interaction keys and `resumeAgentsOnRestore` in one shared reset:

- Different host leaves become the product default, write once, and publish at most one terminal
  presentation snapshot when terminal-derived values changed.
- Default host leaves with a persistence issue rewrite the default record once without publishing
  a terminal presentation snapshot.
- Default host leaves with healthy persistence are unchanged and perform no write or publication.

Disposal clears subscribers and rejects later semantic updates, including Reset, as disposed.
No late update can change theme, storage, or a presentation source after disposal. If host-reset
publication retires the appearance controller, it cannot then start a common Reset.

## Color themes

The active color theme is the Light slot or the Dark slot, chosen by the polarity Application theme
resolves to; System follows the operating system. `@orchbee/terminal` resolves a slot id, falling
back to the Orchbee theme of the slot's polarity for an id this build does not ship or one of the
other polarity. The controller sets `data-theme` to the polarity and `data-color-theme` to a named
theme's id on the theme target; the Orchbee pair sets no `data-color-theme`. Both attributes change
with the terminal publication transaction above.

The Appearance section holds Application theme, then the Light theme and Dark theme pickers. Each
picker is an expanded listbox of the themes of its polarity, aligned to the top of its row so its
label stays beside the first themes; every option shows the theme's localized name (the id for a
theme without a name, such as a test fixture), a swatch of canvas, foreground, and ANSI 1–5, and a
check on the slot's effective theme, so an unknown stored id shows the Orchbee theme it resolves
to. Under System, each picker says when it applies.

| Interaction                               | Result                                                                                                                    |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Arrow keys, Home, End                     | Highlight a theme and preview it at its own polarity on the application and every mounted terminal; nothing is saved      |
| Enter, Space, or a click                  | Commit the theme to its slot; when Appearance is fixed to the other polarity, also set Appearance to the theme's polarity |
| Escape with a preview                     | Restore the committed theme and keep Settings open; without a preview, Escape closes Settings as usual                    |
| Focus leaves the list, or Settings closes | Restore the committed theme                                                                                               |

A committed pick stays shown until the Session answers, so a confirmed pick never flashes the
previous theme; a refused or failed save restores the committed theme. Reset returns both slots to
the Orchbee pair. Stories show the shipped themes; tests prove the pickers with a test-only fixture
pair that does not depend on the shipped data.

## Effective terminal colors and opacity

Host metadata remains unchanged. Default tokens use the Client's effective foreground/background;
unmodified ANSI 0–15 tags use its active named palette in both WebGPU and Canvas. A theme or polarity
change recolors retained cells through the local presentation source without a Host round trip.
Explicit RGB (including black), indexes 16–255 and child OSC 4 overrides retain Host resolution;
OSC 104 restores ordinary ANSI tagging. Inverse swaps resolved display roles, faint keeps its
existing intensity, explicit underline tags use the same Client palette, and absent underline
colors inherit displayed text. Selection/search overlays keep precedence. The local source also
supplies the named cursor; Host cursor metadata remains intact.

The controlling Client reports effective confirmed defaults, cursor and ANSI colors even under a
fixed appearance or Custom defaults. Worker applies them through `ApplyTerminalTheme`, independently
of local rendering; read-only Clients never submit another Client's palette. Only the period before
any report uses the Worker system fallback.
Background opacity is derived from `backgroundOpacityPercent`; `allowTransparency` is not a second
user preference. At 100%, renderer clear and the terminal root are opaque. From 10% through 99%,
the renderer clears the default background transparently and the terminal root presents the
effective background with the selected alpha. Explicit backgrounds, inverse cells, selection, and
search highlights remain opaque. The implementation must not use whole-root CSS `opacity`, because
terminal text must remain opaque.

## Modal shell and focus model

Settings is an application-owned centered, bounded modal shell, not a route or another Workbench
canvas. A scrim leaves the mounted Workbench perceptible while its subtree remains genuinely inert,
preserving terminal surface and attachment identity and preventing Workbench shortcuts from
consuming Settings input. On first open, the application captures a still-connected active element
and focuses global Settings search. Repeated open does not replace that return target. The content
viewport remains keyboard-scrollable and draws an inset focus indicator.

The rail owns an explicit Close settings button. Escape clears a non-empty search before closing Settings; close restores a connected invoker or safe fallback. Desktop injects Agent integration, shell profile, and terminal default controls; a composition without them omits their categories and rows.

Wide Settings use the following composition: one quiet rail
owns Close, search, monochrome product-outcome categories, and a restrained neutral selection; one
moderate content column owns a compact category title, optional introduction, section labels, direct
rows with hairline separators, descriptions, and a stable trailing control column. Radius and shadow
belong to the outer shell and transient surfaces rather than ordinary setting sections. Diagnostics,
profile management, and shell setup may retain the semantic boundaries required by
their task states without turning every setting into a card. Behavior and architecture follow Orchbee's shared settings contract.

At narrow widths, Settings shows one pane at a time. It opens to the searchable category list;
selecting a category or search result shows content, and the content-level Back affordance restores
the list. Controls stack below their copy when the trailing column no longer fits. Host appearance,
input and shortcut changes apply immediately, and shared values apply on confirmation; shell-profile drafts require
explicit Save because the Session's durable catalog mutation is a separate transaction. Reference
branding, copy, categories, and pixel geometry are not compatibility targets.

## Settings information architecture and capability admission

The catalog keeps a stable product-outcome order instead of exposing implementation packages. The
rail renders the following groups with quiet localized labels and restrained vertical separation;
those groups also retain deterministic category and search order:

| Group        | Categories                                                |
| ------------ | --------------------------------------------------------- |
| Application  | General, Appearance                                       |
| Terminal     | Text, Colors, Cursor & Input, Interaction, Terminal       |
| Workspace    | Profiles, Keyboard Shortcuts                              |
| Integrations | Agent Integrations, Shell Integration, Remote Development |

Application language and appearance, terminal typography and colors, input and interaction, shortcuts, local Agent integrations, Shell integration, remote diagnostics, Desktop launch profiles, and Desktop scrollback defaults retain their existing ownership. Navigation and search expose only implemented categories whose required capabilities are present. Roadmap-only controls remain absent.

Ordinary settings use direct rows between quiet hairlines. Only real control boundaries, focus
indicators, and semantic status, warning, or danger notices retain additional borders or surfaces.
This keeps every category in one low-chrome visual system while preserving meaningful state
boundaries.

## Settings discovery and navigation model

Settings search is a global discovery surface, not an in-page filter. Its deterministic catalog
contains every available category plus each current setting or management entry with a stable
identifier, owning category, localized label, optional section, description, and search keywords.
Capability-gated categories and settings enter the catalog only when their composition capability is
present. The catalog never reads a preference value and search never changes a controller, storage,
Host, terminal attachment, or remote environment.

The query is trimmed, Unicode-normalized, split into non-empty terms, and compared
case-insensitively in the effective locale. Category results match their category label. Setting
results require every term to match the setting label, section, description, or explicit keyword.
Direct label matches rank before section, description, and keyword matches; original
information-architecture order breaks remaining ties. Empty query preserves the category rail. A non-empty query
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

| Input                      | Guard                          | Result                                                                                                                        |
| -------------------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| Edit query                 | Any                            | Recompute the global projection and select its first result when present.                                                     |
| Arrow Down or Arrow Up     | Searching with results         | Move the bounded selection by one; the current content page and every preference remain stable.                               |
| Enter                      | Searching with results         | Activate the selected result.                                                                                                 |
| Pointer activation         | A visible result               | Activate that result.                                                                                                         |
| Activate a category result | Any                            | Select the category, clear the query, restore navigation, show content, and focus its heading.                                |
| Activate a setting result  | Its category remains available | Select the category, clear the query, restore navigation, show content, reveal and focus its stable row or management target. |
| Escape                     | Query is non-empty             | Clear search, restore navigation, keep Settings open, and retain the current category.                                        |
| Escape                     | Query is empty                 | Follow the Settings overlay close and focus-restoration contract.                                                             |

The search input retains keyboard focus while moving through results. Result activation moves focus
to the destination heading, row, or management target, never directly changes a value, and never
invokes its action. Changing category through the normal rail clears no user data and starts at the
top of that category.

The adopted reference projection was observed in Cursor 3.14.7 on macOS on 2026-08-17. The adopted
scope is global category-and-setting discovery, rail replacement during a query, stable right-side
content while typing, explicit empty results, Arrow navigation, Enter activation, Escape clearing,
query clearing after activation, category restoration, and target reveal. Cursor branding, copy,
catalog contents, fuzzy-search implementation, result limits, pixel geometry, animation timing, and
ranking ties are explicitly excluded.

## Desktop Agent integrations

Only the Desktop composition injects `AgentIntegrationManagementControl`, as
`ServerAgentIntegrationClient`. A composition without it omits the Agent Integrations category and
its search entries. The renderer consumes one
canonical snapshot with a row for every wire provider in registry order, taken from
`AGENT_INTEGRATION_PROVIDERS` in `@orchbee/workbench-client-protocol`, and displays curated Agent
names, bounded states and failures, and the shared privacy explanation. It receives no Agent command output, executable path, adapter resource
path, filesystem capability, or remote-host authority.

Opening Settings, selecting Agent Integrations, rendering rows, and searching are read-only. Refresh
runs one `list` job and a switch one `enable` or `disable` job on the Session's Worker through the
[Workbench job direction](../../../crates/workbench-protocol/README.md#agent-integration-jobs), the
writer the CLI's `orchbee agent integration` uses. The window polls a job from 20 ms up to every
250 ms and, after 50 seconds, asks it to stop. Rows start unknown and each completed report replaces
them; the Worker's report decides for each row whether it is managed or needs manual setup. A switch
position reflects the last report and never moves optimistically. Pending work labels the affected
row **Working…** and disables every competing switch. The Worker alone decides whether a change may
proceed, so the window checks nothing before it asks. A change the window cannot follow to its
report reads as unverified, marks its row unknown, and is never repeated. Remote hosts are explained
as manual-only and the renderer never writes terminal input or attempts remote setup. The
[writer](../../../crates/agent-integration-writer/README.md) is authoritative for these effects.

Each row's Explain button runs one read-only `explain` job for that Agent, under the same one-job
rule, and opens its diagnosis beneath the row until Close. The headline separates a definite
finding (Ready, Manual setup, "Definitely missing: the Agent", "Definitely missing: the
integration", Turned off, Update needed, Blocked by a conflict) from "Result unknown", which a
timed-out, unreadable or undetermined inspection and an unfinished job produce. The facts follow
`orchbee agent integration explain`: Agent presence, integration status, any issue, the Orchbee
adapter, the provider's marketplace and plugin or guarded entry and adapter source, and what
turning it on would do, naming a command by its executable file name only. The control drops paths
and command arguments before publishing the diagnosis; a report also refreshes that Agent's row,
and an explain failure stays in the diagnosis rather than the page alert. The TUI's
[Agent integrations page](../../orchbee/docs/native-settings.md#agent-integrations) offers the same
operations and copy.

Neither Client offers an integration reminder when an Agent launches. A future reminder would be
designed for both Clients together.

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

## Shell integration and completion reminders

The Shell integration category holds completion reminders, Agent notifications, and their reset.
Reminders need standard OSC 133 command boundaries from the shell. On Windows, Terminal Host loads
Orchbee's PowerShell integration itself; Orchbee ships no Zsh, Bash, or Fish integration for Desktop
terminals, so those shells emit boundaries only through the user's own configuration. The surface
never reads, creates, edits, or backs up a shell startup file.

Feedback uses the shared preferences: Agent mode (never/hidden), completion mode
(never/unfocused/always), minimum integer seconds (1–3600), and sound boolean. Defaults are never,
never, 5 and false. Changes and Reset apply when the confirmed snapshot arrives; no browser record
holds them.

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

## Required evidence

- Host appearance and macOS input storage tests cover missing, malformed, unknown, out-of-range,
  read-throw, write-fail then success, Reset repair, and healthy no-op behavior.
- Application-locale controller tests cover System resolution, an explicit shared preference
  winning, a confirmed selection applying live, invalid input, and disposal. Component tests cover
  both choices, product-copy updates, and search discovery.
- Source tests prove complete single presentation publication, independent interaction publication,
  live attachment updates, application-theme isolation, terminal-setting isolation, stable
  subscription, and disposal. Color-theme tests use a fixture pair to cover the
  `data-color-theme` boundary, unknown and wrong-polarity fallback, preview without a write,
  cancel and close restoring, commit to the polarity slot with the Appearance switch, and a refused
  save restoring; a component test covers keyboard preview, Escape, Enter, and Reset.
- The Server-path preference client has no unit test of its own; the retired Desktop client's
  MessageChannel tests left with it. Its current evidence is the Settings runs recorded under step 4
  of the [native runtime topology proposal](../../../proposals/platform/native-runtime-topology.md):
  another Client's save arriving live, a window save reaching the TUI, a stale draft refused without
  a write, values kept across a Server restart, and profile Reset. Unknown write outcomes and
  subscriber reentry have no current evidence.
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
  updates, Reset, read-only remote diagnostics, inertness,
  idempotent open, focus restoration. Story browser review covers
  the bounded shell, every available category, populated and empty search projections, wide and
  narrow one-pane layouts, and both light and dark themes.
- Terminal-launch-profile protocol tests cover strict snapshots and malformed records. Settings
  component tests cover
  literal custom executable arguments, the future-terminal boundary, create and default selection,
  delete constraints, reset confirmation, platform capabilities, and localized failures.
- Terminal-default-settings component tests cover bounds, capability gating, localized
  validation, and the save to the shared preferences. The Server-path clients that fill the launch-profile,
  terminal-default and Agent-integration controls have no tests of their own.
- Terminal-feedback model tests cover default-off behavior, shared updates and Reset, duration
  and focus policy, stale and future facts, sequence deduplication, notification rate limiting,
  explicit permission requests, and disposal. Component tests cover both locales, reminder controls,
  permission state, and Reset.
- Agent-integration protocol and Settings tests cover canonical ordering, read-only startup, one
  pending mutation, explicit user actions, remote exclusion, localization, and no automatic
  mutation.
- No composition or Desktop window test covers Settings on the Server path.

## Named theme pickers

Appearance selects System, Light or Dark; Light theme and Dark theme store separate catalog ids.
System follows this GUI Client's OS appearance and switches slots without restart. Picking a
theme while fixed to the other polarity switches Appearance too; under System, inactive-slot
choices identify when they apply. Pickers filter by polarity and show localized names with
canvas/foreground/ANSI swatches. Focus previews locally, Enter/click commits, and Escape restores
the confirmed selection. Reset restores the Orchbee pair. Preview never publishes controlling
query colors. Unknown/wrong-polarity ids resolve to that slot's Orchbee theme without rewriting
the stored id. The [shared preference contract](../../../crates/workbench-protocol/docs/user-preferences.md)
owns syntax/persistence; the [theme crate](../../../crates/color-themes/README.md) owns data.
