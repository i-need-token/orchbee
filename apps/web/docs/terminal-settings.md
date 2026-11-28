# Terminal Settings

## Ownership and scope

Web owns Settings, drafts and client presentation sources. Every value the page shows comes from
the settings file, `config.toml` on the Session's machine, which the
[settings contract](../../../crates/workbench-protocol/README.md#native-settings-contract) reads and
edits by key path and the [settings file owner](../../../crates/local-runtime/README.md#user-settings)
validates and writes. The TUI edits the same file. One `UserSettingsControl` per window holds the
last snapshot; the locale, appearance, interaction, feedback, macOS input, terminal-default and
launch-profile controls all read it, and every change is one edit. No setting keeps a browser record.

The Session's Server and Worker own launch adaptation and terminal defaults.
Settings are not Workbench content and never synchronize to remote devices.
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

The locale is the `general.language` key. A selection is one edit and applies when its snapshot
arrives; a refusal shows on the notice line. No browser record holds the locale.

## Appearance and interaction preferences

Application theme, the Light and Dark theme slots, terminal colors, the two custom colors, and the
terminal's font family, size, weights, ligatures, padding and background opacity are keys of the
settings file under `appearance.*` and `terminal.*`. The appearance controller reads them from the
snapshot, and an update sends one edit of every key it changes.

The page sends only values it admits; the settings file owner decides what it stores. The font
family is trimmed, parses as a CSS font-family list, and is at most 256 UTF-8 bytes. Font size is an integer from 8 through 48.
Both weights are integer hundreds from 100 through 900. Terminal padding is an integer from 0
through 32 CSS pixels. Custom colors are exactly `#RRGGBB` and are normalized to lowercase.
Background opacity is an integer percentage from 10 through 100.

The Font row takes a font list in a field, and its Browse button lists installed families through the composition's font catalog. The Web application
supplies the browser's Local Font Access (`queryLocalFonts`) wherever the browser offers it; Desktop's
Chromium offers it and its permission is granted, and a browser asks the person the first time the
list opens, which is why the list asks only then. Fixed-width families come first and alone until
Show all fonts is on; a search looks through every family. Each entry draws its name in its own
face, except families that put pictures where letters belong. The field takes a font list as typed:
Enter or leaving the field commits it, Escape takes back an uncommitted edit, and an empty field
restores the default. A list that does not parse or is over 256 UTF-8 bytes shows the invalid notice
and neither previews nor saves. Browse opens the installed families; a picked family moves to the
front of the field's list and keeps the rest, so prompt glyphs keep drawing where it lacks them, and
the built-in entry restores the default. Terminals and the preview draw any list naming no generic
family with Orchbee's symbols and a monospace font after it. Without Local Font Access, or after a
refusal, the list holds the built-in entry and the typed text. Font
size is a stepper of one pixel per press or Up and Down arrow; a typed size applies on Enter or when
the field loses focus. The page has no Zoom row because the application has no window zoom command.

Font size comes before Font, as in the reference, because the Font row carries a terminal preview
across its width: a prompt, a listing in the usual ANSI colors and the sixteen colors as cells,
drawn in the candidate font, size and colors. The candidate is the highlighted entry of the open
font list, a valid typed font list, a valid typed size, and the colors the window shows now, a previewed theme included;
otherwise the committed values. A growing preview therefore never moves the stepper. The colors
follow `windowTerminalColors`, the same resolution the Desktop window paints with: the active
slot's theme, with the custom pair as default text and background under Custom, and the theme's
canvas under Host because a window has no outer terminal. The Themes page starts with the same
preview, which follows a highlighted theme as the lists preview it. Search hides both previews.

Copy on select is the `terminal.copy_on_select` key. Its controller keeps its own subscription
and publishes interaction configuration when a snapshot changes the value.

### Agent trailing blank rows

Trimming an Agent's trailing blank rows is fixed behaviour with no setting. It updates existing
terminal presentations without recreating their Host attachments, and an Agent's exit immediately
restores the exact projected VT grid.

The application derives one effective presentation policy from the Server's Agent observation for
that presentation identifier. It is `true` only while the composition's Workbench Agent source
holds an observation for that presentation.
A composition without an Agent source, a source that cannot be observed, and a terminal without an
observed Agent are all safe `false` results. Detection belongs to the Server; the composition does
not inspect terminal output or guess process names.
This provider-neutral decision is passed to `@orchbee/terminal`, which owns the bounded display
window and its fallback behavior; neither layer changes PTY geometry or retained history.

A copy-on-select change is one edit and reaches installed presentations when its snapshot arrives.

### Agent confirmation preference

The Allow routine confirmations group holds one switch for each Agent type whose routine
confirmations the Server can answer, named by the Agent's product name: `agents.auto_allow.claude`
for Claude Code, `agents.auto_allow.codex` for Codex, `agents.auto_allow.gemini` for Gemini CLI,
and `agents.auto_allow.opencode` for OpenCode. A switch shows only while the Server publishes its
key, which an older Session's Server does not, and the group only with one. A switch reads and edits
its key through the window's settings control and has no capability gate; the Server applies a change at once, and Off also stops an answer it had already
decided. The group's closing line asks for more Agents in a GitHub issue and opens the project's
issue page where the host wires an external browser; elsewhere it is text. The
[Server contract](../../orchbee-server/docs/agent-confirmation.md) owns what is answered.

### Restore after restart preference

The Terminal category's After a restart group exposes “Restore terminals after restart”, the
`terminal.restore_after_restart` key, on by default. One switch resumes supported Agent
conversations and shows every other terminal's recent output again; its description warns that
output is stored on this computer and can contain secrets. The row edits the key through the
window's settings control, not the interaction controller, because no installed presentation
consumes it, and has no capability gate; the Server reads it at each checkpoint capture and at
startup. The [checkpoint contract](../../orchbee-server/docs/workbench-checkpoint.md#terminal-output-replay)
owns what is kept, deleted and replayed, and the
[Agent resume contract](../../orchbee-server/docs/agent-session-resume.md) what is resumed.

### Automatic updates preference

About ends its card with “Automatic updates”, the `update.automatic` key, on by default, under the
Updates row. A search lists it alone, without the version and Updates rows. The row edits the key through the settings control, applies at once and has no capability
gate; the Server reads it before each background update check. It renders only while the settings
registry marks the key for the page, which the
[automatic updates proposal](../../../proposals/platform/automatic-updates.md#phases) turns on with
its slice 1 switch. The description ends with “Nothing restarts until you click Update.”, because a
click on the window's Update control relaunches the window on the new version.

Workbench exposes increase, decrease, and reset font-size Actions through the same controller.
Increase and decrease apply a two-point step and clamp at `8` and `48`; reset restores only the
default `13` font size and preserves every other setting. A changed Action is one edit of
`terminal.font_size`, and Actions repeated before its snapshot arrives step from the last requested
size, while a boundary or already-default request is unchanged. The behavior is application-wide rather than per Panel.

Terminal display themes derive one complete candidate from the latest reconciled Host state without
modifying the stored custom pair:

| Theme    | Host terminal candidate                                                                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `theme`  | Take the active color theme's scheme, default colors, cursor, selection, and ANSI 0–15; keep the current Host palette from index 16. |
| `host`   | Keep the complete current Host color scheme, defaults, nullable cursor color, and 256-color palette.                                 |
| `custom` | Set defaults from `customForeground` / `customBackground`; preserve the current Host scheme, cursor color, and full palette.         |

Changing between Theme and Host retains the Custom values. The text and background color rows
appear only while Custom is selected. Application theme, terminal display theme, complete fixed overlay
colors, and the Host-assigned theme revision form one publication transaction. A successful barrier
commits every mounted terminal before updating `data-theme` in the same JavaScript task.

## macOS Option-as-Alt preference

The Desktop renderer on macOS reads `terminal.macos_option_as_alt`: `off`, `left`, `right` or
`both`, with `off` as the product default. Windows and Linux compositions do not expose the
Option-as-Alt row and retain the default; on macOS the row sits in the Terminal category's
Selection and keys group. A change is one edit; a refusal keeps the file's value and shows the
reason on the notice line.

The client tracks physical `AltLeft` and `AltRight` ownership and resolves the selected side for
each admitted key event. It does not rewrite committed text or guess from the character produced by
the macOS layout. A complete input-configuration source updates every mounted projected terminal
immediately without replacing its attachment. `KeyInput.optionAsAlt` then carries only that event's
semantic decision; Terminal Host remains responsible for Ghostty's mode-dependent legacy,
modifyOtherKeys, and Kitty keyboard encoding.

## Desktop terminal launch profiles

Only the Desktop composition injects `TerminalLaunchProfileControl`, filled from the settings
file's launch-profile keys for the Server's platform, which the TUI edits too. Its snapshot exposes
one bounded canonical catalog, the current default identity, whether the file sets the catalog,
whether the file is read-only, and the last refusal; it is the built-in catalog only until the first
snapshot. A composition without the control omits the Launch profiles group. The renderer never
receives a Node error, filesystem primitive, or launch capability.

Each profile has a stable identity, a user-visible name, and exactly one command choice:

- System shell with a login-shell decision. Login mode is enabled only when the window's connection
  bootstrap reports support and is unavailable on Windows.
- Custom executable with one absolute platform path and an ordered vector of literal arguments.
  Arguments are not tokenized, quoted, interpolated, or joined into a command string.

The stable built-in profile uses a localized display label rather than persisted product copy. User
names remain exact user data and duplicate names do not merge identities. The Launch profiles card
shows one profile per line with its Default label or a Use as default action, and an Edit action
that opens the profile's editor on a child page named after it; Done or Back saves the draft and
returns to the list, and a draft that cannot be saved keeps the page open with its notice. Add
profile, the card's last line, lists the shells installed on the Server's machine that the
connection bootstrap reports, then Custom…; a shell adds a custom-executable profile under its name
with its path and arguments, and Custom…, or Add profile itself when no shell was found, opens a
name box at the same height in its place that adds a system-shell profile. Either lands at the end
of the list, as the TUI does, and never copies the profile last selected or edited. The editor can rename a profile,
change its command and arguments, or delete a non-default profile after confirmation. The default
and final profile cannot be deleted. The group's Restore default asks for confirmation in the
group heading and replaces
the complete catalog with the built-in default only after the Server commits it. Pending operations disable competing
mutations and surface bounded localized failures; invalid drafts never send a mutation. A failed
commit retains the user's draft so they can correct or retry it instead of reconstructing their
input.

Create, edit, default selection, delete, and reset use explicit controls. They do not alter a
running terminal. Each change saves the complete catalog in one edit. A draft begun before the
catalog last changed is refused as `source-changed` without being sent, so another Client's change
is not overwritten from a stale draft; the file's refusals show their own reason. A created profile
follows its source and takes the first free `profile-{n}` identity, as in the TUI. The Server
decides what this machine can launch and refuses a relative executable as an invalid profile. The
Worker launches the default profile for each terminal created after the save. The Workbench's New
Tab button offers each saved profile for one Tab, as the [Web README](../README.md) describes for
`server-workbench-tabs.ts`, without changing the default. Profile resolution changes only executable
and arguments; environment, secrets, presentation, CWD, project or workspace data, remote
connections, and shell startup-file mutation remain outside this feature.

## Desktop terminal defaults

Only the Desktop composition injects `TerminalDefaultSettingsControl`, filled from the
`terminal.scrollback_bytes` key. The Terminal category's Output group exposes
`terminal.history` as Terminal history in decimal megabytes from `0` through `1000`, stored as
bytes up to `1_000_000_000`, and notes that it applies to new terminals. The row commits one edit,
and the TUI shows the same value in megabytes. The Worker gives that value to each terminal it
launches after the save, so no Host application step can fail, and existing terminals are
unchanged. The control only reads; the page
warns that scrollback is unavailable until the first snapshot arrives. A composition without the
control omits this group. The Terminal category remains visible only when that
composition supplies another current Terminal setting.

## Read, commit, and recovery model

`UserSettingsControl` in `src/user-settings/` exposes `getSnapshot`, `subscribe`, `set`, `restore`,
`edit` and `refresh`. Its state is the last snapshot, whether the Session can be reached, whether an
edit is pending, and the last failure with the keys it covers. `ServerUserSettingsClient` fills it
over the window's Session connection: a State stream of `read` carries every later change, whether
another Client made it or the Server found it in the file, and a snapshot older than the one held,
by revision, is dropped.

A commit is one `edit` of every key it changes; restoring a key removes it from the file so its
default applies, and the Server's editor removes a scalar key set to its default the same way. Controls show the file's value and change when the edited snapshot arrives; nothing
is applied optimistically. Text fields keep a draft while focused, commit on Enter or blur at most
once, and return to the file's value when the edit is refused. A refusal names one of the wire's
reasons, and the row shows one short localized sentence for it; an `unusable-file` refusal whose
detail is `not-file`, `io-error` or `too-large` says the file cannot be read. An edit whose outcome is unknown is
never replayed; the row asks the user to check the value first. While the Session cannot be reached,
the page shows the values last received and one dismissible notice.

A changed setting is one the file sets: its row shows the dot and Restore default, which removes
that key. While the file has a syntax error or cannot be read, the Server keeps serving the last
valid values and refuses every edit. The page shows those values with read-only controls, hides
Restore default, and shows one notice that names the file path, with the line and column of a
syntax error. The notice cannot be dismissed and clears when a snapshot reports a usable file.

System appearance follows a live prefers-color-scheme subscription. The custom color rows appear
only for Custom terminal colors. Sound defaults off and selects silent system notification delivery.

One appearance update follows this order:

1. Validate the complete candidate against the values the page admits; an invalid candidate sends
   nothing.
2. Send one edit of the keys that differ.
3. When the snapshot arrives, derive the immutable setting values, source-neutral presentation
   configuration, and, when theme-affecting values changed, a complete runtime-theme candidate.
   Publish presentation configuration only when those derived values changed, and start one
   latest-wins runtime-theme barrier when theme-derived values changed.

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

The visual transaction does not include the settings file. A presentation failure does not edit the
stored choice even though the previous complete theme remains visible.

Synchronous listener reentry is serialized as immutable presentation snapshots in first-in,
first-out order. Disposal during publication stops the remaining callbacks and prevents any pending
publication.

Keyboard shortcuts are the only Settings values with a browser record, as
[keyboard shortcuts](keyboard-shortcuts.md) describes.

Settings offers no page-wide reset: each changed row restores only its own key. The controller's
terminal reset restores the font, weight, ligature, padding and opacity keys in one edit. Disposal
clears subscribers and rejects later semantic updates as disposed; no later snapshot can change the
theme or a presentation source.

## Color themes

The active color theme is the Light slot or the Dark slot, chosen by the polarity Application theme
resolves to; System follows the operating system. `@orchbee/terminal` resolves a slot id, falling
back to the Orchbee theme of the slot's polarity for an id this build does not ship or one of the
other polarity. The controller sets `data-theme` to the polarity and `data-color-theme` to a named
theme's id on the theme target; the Orchbee pair sets no `data-color-theme`. Both attributes change
with the terminal publication transaction above.

The Themes category holds a Light theme group and a Dark theme group. Each group is one card that
names the theme its slot holds beside a Choose theme button. The group whose slot is in use carries
a muted In use now tag; when Appearance is fixed, the other group carries a one-line note saying
when it would apply. Choose theme closes Settings and opens that polarity's list over the window: a
search field, always shown, and the themes of the polarity by name, with a check on the slot's
effective theme, so an unknown stored id shows the Orchbee theme it resolves to.
The list is a step inside the admitted Settings modal session, so it needs no admission of its own:
it opens from Settings, always returns to Settings, and keeps the Workbench behind it inert, so
a press over a Panel only closes the list.

| Interaction                        | Result                                                                                                                                                |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Arrow keys, Home, End, or hover    | Highlight a theme and preview it at its own polarity on the application and every mounted terminal; nothing is saved                                  |
| Enter or a click                   | Store the theme in its polarity's slot and close the list; the app appearance is left alone, so the window shows the theme the app appearance selects |
| Escape or a press outside the list | Restore the committed theme, close the list, and reopen Settings at its Choose theme button                                                           |
| Focus leaves the list              | The same as a press outside                                                                                                                           |

The controller's snapshot carries `presented`, the committed preferences with any previewed theme in
its slot, so a preview reaches every reader without a write: the Desktop window's terminals repaint
in it, while the colors the window reports keep the committed theme. A committed pick stays shown
until the Session answers, so a confirmed pick never flashes the previous theme; a refused or failed
save restores the committed theme. Each group's Restore default returns its slot to the Orchbee
theme.

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

## Dialog shell and focus model

Settings is an application-owned modal dialog, not a route. It centers over the Workbench on a scrim,
below the `36px` top chrome where a Desktop window's own controls sit, and grows to at most
`container-5xl` by `container-3xl` (1024 by 768 px). The Workbench subtree stays mounted and inert, preserving terminal surface and attachment identity and preventing Workbench shortcuts from
consuming Settings input. On first open, the application captures a still-connected active element
and focuses Settings search. Repeated open does not replace that return target.

The navigation column holds search and the categories; the close button sits in the dialog's
top-end corner. The close button, a press on the scrim, and Escape when no control consumes it return
exactly where the user was; Escape first leaves an open child page, then clears a non-empty search.
Closing restores a connected
invoker or safe fallback. The scrim's top band still drags a Desktop window. Below `48rem` the
navigation stacks above the content and the dialog keeps a narrower margin from the window's edges.

The content pane centers one column of at most the `container-2xl` width. It holds dismissible
page notices, the category title, and groups: a small muted heading and one card whose rows are
separated by hairlines. A category with three or fewer settings may use one unnamed card. Each row
has a name, an always-visible description of at most two lines, and exactly one control at the
row's end. A changed setting shows a small accent dot after the row's name and its effect note,
centered on that line, and a Restore default action beside its control while the row is hovered or
focused; an unchanged row keeps that action's place empty, so a change never moves its control. A
theme list that is one setting carries the dot after its heading.

No action changes a row's height. A refusal, a failed save, or an invalid shortcut appears on the
notice line, a dismissible line that floats at the content column's foot and stays in view while the
page scrolls; it is the Desktop counterpart of the TUI's Settings status line. A launch profile's
editor and an Agent's diagnosis open as a child page that replaces the category's content under a
Back action; Escape goes back first, and going back restores the list's scroll offset and focus.
Every finite choice is
the shared Select from `@orchbee/ui`: a filled trigger whose list opens below it, end-aligned, with a
check on the chosen entry. Every change applies at once; the
page has no Save, Discard, or footer. Shell-profile edits save on the editor's Done, because the
Session's catalog mutation is a separate transaction.

## Settings information architecture and capability admission

The categories keep a stable product-outcome order instead of exposing implementation packages:

| Category      | Groups                                                                                 |
| ------------- | -------------------------------------------------------------------------------------- |
| General       | Language (unnamed card)                                                                |
| Appearance    | App appearance; Terminal colors, with custom text and background colors; Terminal text |
| Themes        | Light theme; Dark theme                                                                |
| Terminal      | Selection and keys; Output: terminal history; After a restart; Launch profiles         |
| File Bar      | Layout: position; Explorer: confirmations; Source Control: Git questions and warnings  |
| Notifications | When to notify; Sound and delivery                                                     |
| Keyboard      | Window and panel shortcuts                                                             |
| Agents        | Allow routine confirmations: one switch per Agent type                                 |
| Machines      | This computer and the saved machines (unnamed card); a machine's page                  |
| About         | Version; Updates; Automatic updates (unnamed card)                                     |

Machines appears only where the host reports saved machines, which today is the desktop window
once its machines process sends a first view; its rows and every sentence about a machine are the
TUI's Settings › Machines, worded by that process ([contract](../../orchbee/docs/machines.md#desktop-machines-process)),
and Manage opens a machine's page. Add machine…, the card's last line, opens a Dialog with the TUI's Add
Machine form: the color before the address, the SSH config hosts below it, which Down and Up
highlight and Enter or a click takes into the address, and the eight checks named as they run;
editing the address after a failure hides it. The machine takes its host's name, which its page
changes. A login that Connect, Retry or Log in starts asks in the
window's login Dialog, which stays over whatever holds the window, Settings closed or not, until the
question is answered, cancelled or, once its login ended, closed. A command's login offered to the
window opens the TUI's notice the same way, Log in or Not now, and a choice counts only once the
notice has shown for a second with no other key pressed on it.

Every category but Themes, File Bar, Machines and About ends with a "More options in config.toml" row that shows the path of
the settings file the Session reads, taken from its snapshot, with a copy action. The renderer has
no port that opens a file, so the row copies the path instead; Copy is unavailable until the first
snapshot arrives.

Navigation and search expose only categories and rows whose required capabilities are present: the
Terminal category needs at least one of its controls, Notifications needs the feedback
controller, and File Bar needs a window that offers the File Bar. Roadmap-only controls remain
absent: a File Bar row shows only once the settings registry marks its key for the page, which the
key's feature does when it ships.
About holds the version and the update control, which are information, and the Automatic updates
setting, so it has no settings-file row and search lists only Automatic updates from it. Every
composition that hands the page its build shows the version and Updates rows; a served Web Client shows
its Updates row unavailable for this installation and leaves Version out, because it knows neither.

The page leaves out the terminal's regular and bold font weights, ligatures, padding and background
opacity, trailing-blank-row trimming, the completion-reminder minimum, the terminal reset, the
reminder reset, and the remote-development diagnostic. Their stored values keep applying; the TUI's
Settings and `config.toml` edit them.

## About and Check for updates

About is the last category, as Warp places it. One unnamed card holds three rows, in the TUI's
words:

- **Version** shows the version the window runs, which Main states in the document's bootstrap, and
  the build (Release build, Test build, Development build) as its description.
- **Updates** has one control, except on a failed check, or on a Session too old to check, where
  Get installer sits beside Check for updates. While Main offers a newer window version it is the
  sidebar footer's Update: the same `workbench.client.update` Action, registered in a
  `WebApplication` scope above the Workbench's so both surfaces reach it, with the footer's tooltip
  sentence as the description and the same waiting notice; when switching fails or is refused,
  About shows the same failure notice on Settings' notice line, advising to quit and reopen Orchbee.
  While Main offers the move of an older Session onto this window's version, it is the same Update,
  described by why the move waits, and while that move runs it shows its notice's sentence with no
  control, as [Desktop's Session update](../../desktop/README.md#session-update) describes.
  Otherwise it is Check for updates, which is disabled and reads the stage of a run under way:
  “Checking…” while the run is queued or reads the feed, “Downloading 0.1.0-beta.26… 42%”, with the
  percentage formatted in the page's locale, while it downloads, and “Installing…” while it
  installs. When a check fails, the danger description names the phase it failed in (“Couldn't
  check for updates.”, “Download failed.”, or “Downloaded but couldn't install.”) and notes that
  installing the latest version from `orchbee.com` keeps terminals and Agents running, and, where
  the composition supplies `openExternal`, Get installer opens `https://orchbee.com/#install` in the
  person's browser. A Session too old to check reads
  “Available once the Session updates. Or install from orchbee.com—terminals and Agents keep
  running.” in a neutral tone, with the same Get installer.
- **Automatic updates** is the `update.automatic` switch of the
  [Automatic updates preference](#automatic-updates-preference).

Check for updates asks the Session Server to run its scheduled automatic-update job now, through the
`workbench.update-check` request; the client never starts `orchbee update` itself.
`WorkbenchUpdateCheckModel` holds the one status (Idle, Checking, Downloading with the version and
whole percent, Installing, Up to date, Failed with its phase, or Unavailable with a reason), and the
Desktop composition hands it to About and the footer as part of the window's one update state,
`WorkbenchClientUpdate`, beside Main's offer. The reason is decided before any
request, in this order: a development build; a window Main can offer no newer version, which the
bootstrap's `windowUpdatesOffered` says, and also a Server that answers `unavailable`; and a
Session whose negotiated capabilities, from the bootstrap's `negotiatedCapabilities`, lack
`update-check`. The window's one table from capability to actions, `serverSessionServes.ts`, reads
an unknown list as serving nothing, so a request a Session may not know is never sent. The same table
sends `phases` only to a Session that lists `update-phases`; another one answers Checking until its
run ends.

Pressing Check sends `start` and shows Checking. While About is shown and a run is under way, the
window sends `read` about once a second, one exchange at a time, and shows the stage it answers:
`downloading` as Downloading, whose percent is the whole share of the bytes received, and
`installing` as Installing. `up-to-date` shows Up to date; `check-failed`, `download-failed`, and
`install-failed` fail the check in that phase, and `failed` from a Session without phases, `denied`,
`rejected`, `idle` after a start, and an answer that is lost, late after ten seconds, of another
kind, or for another request fail it as the check; none is repeated. `installed` shows Installing
until Main offers the installed version, then the Update control takes its place; Main follows the
window pointer as it changes, and a check that sees no offer within 15 seconds goes back to Idle
rather than claim Up to date. Showing About again clears a finished result, then sends one `read`:
a run under way shows its stage, and a finished result shows only for a check this window started,
so a background run shows nothing but its offer.

## Settings discovery and navigation model

Search filters the page; it never changes a controller, storage, Host, terminal attachment, or
remote environment. The query is trimmed, NFKC-normalized, lowercased in the effective locale, and
split into terms; a row matches when every term appears in its name or description. A non-empty
query replaces the selected category with every available category's matching rows, grouped under
their category names in navigation order, and each match keeps its working control. A long list's
own filter box hides while the page is searched. Zero matches show a plain-words empty state.
Escape or clearing the query returns to the selected category. Changing category clears the query
and starts at the top of that category.

## Notifications and completion reminders

The Notifications category holds Agent notifications, completion reminders, and sound.
Reminders need standard OSC 133 command boundaries from the shell. Terminal Host loads Orchbee's
own integration for PowerShell on Windows, and for zsh and bash launched with no arguments other
than interactive or login flags on macOS and Linux, without writing any startup file of the
person's. Orchbee ships no Fish integration, so Fish and any other launch emit boundaries only
through the user's own configuration. The surface never reads, creates, edits, or backs up a shell
startup file.

Feedback reads the `notifications.*` keys: Agent mode (never/hidden), completion mode
(never/unfocused/always), minimum integer seconds (1–3600), and sound boolean. Defaults are never,
never, 5 and false. The page does not show the minimum; the TUI edits it. Changes and each row's
Restore default apply when the snapshot arrives; no browser record holds them.

Each client deduplicates by terminal identity and positive completion sequence, ignores facts more
than 60 seconds old or over 5 seconds in the future, and limits system notifications for one
terminal to one every 30 seconds. `unfocused` requires the document or that terminal root to lack
focus. Browser notification permission can be requested only by the Allow button in the Notifications
category's banner;
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
application and selects its exact Tab and Panel but cannot answer a question or grant approval.

The desktop window keeps notifying for the Sessions it holds but does not show, under the same
rules: this computer's while another machine is on screen, and every connected saved machine's.
Such a notification names its Workspace with `@` and the machine's name, and activating it opens
that Workspace on its machine.

The current Workbench unmounts terminal Surfaces that belong entirely to inactive tabs. A mounted
but unfocused terminal can remind immediately, and reattachment can consider a completion fact
within the 60-second freshness window. This is not a claim of complete background monitoring:
completion while a Surface remains unmounted beyond that window does not produce an immediate
notification. A future all-Tab command-completion monitor requires a separate lightweight
Workbench/Host event subscription rather than hidden renderers. Agent observation already uses
such an application-level subscription and does not share this command-completion limitation;
fully disconnected SSH delivery remains out of scope.

## Named theme pickers

Appearance selects System, Light or Dark; Light theme and Dark theme store separate catalog ids.
System follows this GUI Client's OS appearance and switches slots without restart.
Each theme list opens over the window from its Choose theme button and filters by polarity, showing
localized names. The highlight previews locally, and Enter stores the theme in its polarity's slot
without changing Appearance; under System, inactive-slot choices identify when they apply. Escape or
a press outside restores the confirmed selection and reopens Settings. Each slot's Restore default
restores its Orchbee theme. Preview never publishes controlling query colors. Unknown/wrong-polarity
ids resolve to that slot's Orchbee theme without rewriting the stored id. The [settings file
owner](../../../crates/local-runtime/README.md#user-settings) owns syntax/persistence; the [theme
crate](../../../crates/color-themes/README.md) owns data.
