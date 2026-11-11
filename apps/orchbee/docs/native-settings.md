# Native settings

## Interaction and scope

`Ctrl+B F4`, the sidebar Settings row and the global action menu open Settings on supported native
TUI hosts. Settings is a full page that captures the client area while pane output continues. It
covers every common setting both Clients present in the
[settings registry](../../../crates/local-runtime/README.md#user-settings), including launch profiles
and new-terminal scrollback defaults, plus the TUI-only interface colors, Agent blank-space trimming,
the completion-reminder threshold and the prefix key. Host-owned fonts, pixel metrics and management
workflows stay with their own owners. The native Settings feature maps each shared key once to its
row, group, category, control kind, availability, integer unit, bounds, step and effect horizon. The
settings registry owns keys, defaults and validation, and Workbench Protocol the value types and
their bounds.

The page follows the Desktop page's structure, wording and order in both locales. The left column
holds Back, search and the categories General, Appearance, Themes, Terminal, Notifications, Keyboard
and Agents. The right column holds the category title, a line naming the settings file when it is
unusable, and framed groups under their headings. Every row shows its name, a one-line description
that is always visible, and its control at the row's end. Themes shows the light list and the dark
list; the list the current appearance paints is tagged in use, and System follows the canvas. A row
that only applies in some state, such as custom colors outside Custom or the reminder threshold while
reminders are off, is hidden until it applies. The last line names More options in `config.toml` and
its path. Below 56 columns the page shows one column at a time; Esc or the title's Back steps from
rows to categories before it leaves. The page requires 24 columns by 8 rows. A frame below that is
not presented: before the first successful frame the page is refused with the enlarge-and-reopen
notice, and later it suspends drawn interaction while staying the selected capture.

The page's last line is its status line. It names the keys for the focused row, or shows a Settings
notice, which stays until the next change replaces it. Up/Down, PageUp/PageDown and Home/End move
within the focused column; Left or Shift+Tab moves to the categories and Right, Tab or Enter to the
rows. `/` or the painted search field starts global search. The bounded query matches every term
against English and Simplified Chinese category names, labels and descriptions in stable category
order; values and user-authored profile text are excluded. Search shows its result count and an
explicit empty result; Enter or Tab focuses the matching row without changing it, and Esc clears the
query before stepping back. Text fields use the
[shared bounded line editor](native-line-editor.md), including Ctrl+W word deletion and literal
paste.

## Changes

Every change applies at once: there is no draft and no Save. A change sends one `workbench.settings`
`edit` naming only its key. A toggle applies on Enter, Space or a click. A choice opens an inline list
whose candidate previews when it is an appearance field and applies on acceptance. A text or number
row edits in place, applies on Enter, and Esc keeps the confirmed value. Left/Right and the painted
minus/plus controls step a number by its step and apply that step. Picking a theme line sends its slot,
the appearance that shows it and Colors set to Theme in one edit; the focused uncommitted line previews, and Esc returns
to the theme in use. Launch profiles open as a child page whose edits apply as one edit on leaving;
an invalid catalog first focuses the failing field, and leaving again discards it. Custom colors are
unavailable unless Custom is selected and login shells are unavailable on Windows; a focused
unavailable row names its reason.

One edit is in flight at a time. Its row shows the pending state while every other row, navigation
and Back stay usable; further changes are ignored until it settles. A confirmed edit adopts the
returned snapshot without a success notice. A refused or unknown edit puts the row back to the file
value and shows the Server's localized reason on the status line: the file was busy, a value or key
was refused, the file has a syntax error, cannot be read or is not a regular file, is read-only or
would be too large, its location cannot be used, or the write failed. A syntax error points to
`orchbee config check` for the position. No request is replayed automatically.

A row whose key the file sets carries a changed marker. Del on a changed row, or its painted Restore
default action, sends one `restore` for that key; the launch-profile row restores both platform keys.
There is no page-wide reset.

Snapshots from the settings subscription show on the open page at once, whoever wrote them, while
focus stays on its row. A snapshot whose revision is not newer than the last one observed is dropped.
While the newest snapshot finds `config.toml` unparsable or unreadable, the page names the file and
the line and column of a syntax error, keeps the last valid values, and every change is refused with
that reason until a snapshot finds the file usable.

Settings preflight and painting consume the theme's `paired_pointer_surfaces` projection, so hover
and press bands follow the Tab strip's fallbacks under terminal-native and Monochrome themes. Clicks
on a category, row or control publish the generic activate-on-release interaction; a plain left
release must match the press's token and rectangle. The wheel scrolls the column under the pointer,
three rows per notch, without changing focus or any value; keyboard navigation reveals the focused
row again. Closing the page discards an uncommitted inline edit or preview, while an edit already
sent still settles under its token. Captured packet and paste tails remain owned through closing and
the next redraw, and the pane cursor stays hidden under Settings. Pending feedback for the first read
begins after 200 ms. The ordered settings lane has a two-second exchange deadline and a state
subscription on the shared carrier; an edit is a one-shot request whose reply and the subscription's
observations are ordered by revision. Closing cancels its requests and subscription. A replacement
Session or editor episode cannot inherit an old completion.

## Agent integrations

The Agents category holds an Agent integrations entry on a Session reached through a local origin.
A saved-machine origin omits the entry, as the GUI offers the page only in Desktop: remote hosts
stay manual. Search matches the entry's title, its description and every Agent name. Activating
the entry opens a child page, like Launch profiles, that edits no preference; Back or Esc returns to
the entry with the filter unchanged.

The page lists the same twenty Agents as the GUI in the Agent integration contract's canonical
order, under a "Local Agent status adapters" heading, then a disabled "Remote hosts stay manual"
row with the GUI's manual-only description. Each Agent row carries a checkbox for the enabled
state and the GUI's status word, with the same precedence: Working, Manual setup, Status
unavailable, Agent not found, then the reported state. Rows start Unknown and change only from a
Worker report. Entering the page is the explicit request to inspect: it starts one refresh unless
a job already runs. Opening, navigating, searching and closing Settings read and write nothing,
so without that request no provider command runs and no provider configuration is read. The
page ends with a Refresh status row, and the line below the title shows the last failure, a running
job, the not-yet-checked hint or the GUI's disclosure.

Entering the page, Refresh status, Enter/Space or a click on an Agent row, and Explain each start
one Server-authorized Worker job over the shared carrier, through the same `workbench.agent-integration` start, poll and
cancel exchanges as the GUI. One job runs at a time; while it runs the Agent rows, Refresh and
Explain are disabled and the affected rows show Working with a pending status glyph. A row takes
the GUI's switch rule: manual-only, conflicting, unknown and missing Agents cannot change, and
the detail line names the reason. Polls start after 20 ms and back off to 250 ms on the UI tick;
each exchange has five seconds, and a job still running after 50 seconds receives a cancellation
the Client does not wait for. Settlement is the GUI's: a definite refusal before start keeps the
rows, a change whose effect may have begun reads as unknown with the verification message, an
unlisted Agent reads as Status unavailable, and nothing is resubmitted automatically. Closing
Settings does not cancel a job; it settles while Settings is closed and shows on reopen. A new
Session drops the old job's result and starts uninspected.

Each Agent row ends in a quiet Explain action, `e` on the page, which runs the read-only
`explain` operation and opens that Agent's diagnosis child page. Its headline names a definite
finding (Ready, Manual setup, "Definitely missing: the Agent", "Definitely missing: the
integration", Turned off, Update needed, Blocked by a conflict) or "Result unknown" for a timed-out,
unreadable or undetermined inspection, whose sentence says the state is unknown, not missing. The
facts below follow `orchbee agent integration explain`: Agent presence, integration status, any
issue, the Orchbee adapter, the provider's marketplace and plugin or guarded entry and adapter
source, and what turning it on would do, naming a command only by its executable file name. Paths
and command lines are never shown. `e` on the diagnosis page reruns it, and a report also updates
that Agent's row. Rows narrower than 40 columns beside the action drop it and keep `e`; a short
diagnosis clips its facts after the headline and sentence.

## Domain and ownership

| Record field | Contract                                                                                                                                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | GUI and TUI users edit the same device preferences without lost updates or divergent defaults.                                                                                                             |
| Inputs       | Frozen `config.toml` location, complete confirmed snapshot and its revision, the one changed key, painted-row receipt, explicit edit/restore intent.                                                       |
| Authority    | Workbench Protocol owns values/bounds; Local Runtime owns the settings registry and the locked batch edit of `config.toml`; the Session Server serves and publishes it.                                    |
| State        | Newest confirmed values, an uncommitted inline edit or preview, viewport/focus anchor and painted activation authority, one in-flight edit, and the last refusal notice.                                   |
| Effects      | Confirmed snapshots govern runtime preferences; an independent reversible projection previews only authored copy, chrome and terminal default colors. Launch/default changes affect new terminal creation. |
| Refusal      | A syntax error, unknown key, refused value, changed or busy file, unusable location, busy lane or failed write preserves existing data and confirmed behavior.                                             |

The Session freezes its `config.toml` location at startup. Clients send no paths, raw TOML, alternate device selector or arbitrary file write; an edit names registered keys only. An absent file supplies every default, and a file that stops parsing keeps the last valid values. See the [settings contract](../../../crates/workbench-protocol/README.md#native-settings-contract).

| Transition           | Required behavior                                                                                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hidden → open        | Read a complete snapshot and show it.                                                                                                                                                 |
| Open → change        | Require a current successful draw and no edit in flight; send one edit or restore for the row's key and mark only that row pending.                                                   |
| Submit → commit      | Lock the file, apply every change or none in one replace and publish one snapshot; keep other keys and comments. A scalar key set to its default is removed, as a restore removes it. |
| Commit → effective   | Publish only the confirmed values; do not reinterpret a disk receipt as Host application acknowledgment.                                                                              |
| External update      | Show the newest confirmed values at once; a pending row stays pending until its own outcome.                                                                                          |
| Refused / unknown    | Put the row back to the confirmed value and show the reason; replay nothing.                                                                                                          |
| Geometry unavailable | Refuse an unpainted incarnation; suspend and retain an admitted one until a successful frame resumes it.                                                                              |
| Cancel / close       | Discard an uncommitted inline edit or preview and reveal the confirmed appearance; closing cannot roll back a sent edit.                                                              |

Application themes change chrome independently of terminal colors. Host terminal colors under
application System, and Theme or Host under Terminal colors, keep the outer terminal's own
defaults; the other terminal choices change only default markers, preserving explicit program
colors and color-query semantics.
Disabled foreground/background rows show the effective display colors rather than unused Custom
values; reduced-color hosts show the selected terminal appearance when RGB is unavailable.
Copy-on-select governs later gesture completion; explicit Copy remains available.

Themes holds the light list and the dark list, with the GUI's wording in both languages; theme names
stay untranslated. Each list holds the shipped themes of its polarity in catalog order, Orchbee
first. Each line ends in a swatch, a foreground sample on the theme's canvas and five ANSI fills,
shown only at truecolor and where the line has room. Focusing an uncommitted line previews it at its
own polarity with Colors set to Theme; Enter applies it and Esc returns to the theme in use. Applying a theme sets its
slot, switches a fixed Appearance of the other polarity and sets Colors to Theme, so the applied
theme shows; under System it shows when the host matches, and the list's description says so. A
stored id this build does not ship opens on the Orchbee theme it resolves to and stays stored until
another theme is applied.
Below truecolor the lists stay visible but unavailable, and the line below the title gives the
24-bit reason or the `NO_COLOR` note.
Agent mode/global sound override automatic enablement without erasing host delivery
or per-Agent mute policy. Completion reminders consume physical-terminal sequence
facts once, require a fresh timestamp and minimum duration, and never replay when
a setting is enabled. They appear as localized transient status plus optional bell.

Agent tail folding shares the retained-history read/ack/recovery owner. It only
replaces a proved blank primary-screen suffix with an exactly matching acknowledged
window. Stale, resized, graphic or unavailable windows fall back to the live grid;
the cursor moves with its live row. Ordinary typing remains live. Selection,
scrollbar use and manual history navigation take ownership of the displayed window;
automatic following cannot move active selection/export work. The
[viewport contract](native-terminal-viewport.md) governs the underlying lease.

## Configuration stores and readers

The Session Server and the TUI keep every setting in one store, `config.toml`: the settings a Settings surface edits and the ones users edit only by hand, described in [Windows CLI configuration](windows-cli-configuration.md), live in the same file, and the [settings registry](../../../crates/local-runtime/README.md#published-key-file) lists every key. The chrome theme belongs to `appearance.mode` and the default launch program to `terminal.default_profile.<platform>`; notification policy, from the Agent reminder and global sound to manual delivery and per-Agent sound overrides, sits under `[notifications]`.

| Reader                                  | Reads                      | Rule                                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------------------- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session Server                          | `config.toml`              | The runtime authority. It reads `config.toml` at startup, on reload, after each committed edit and, on Windows, when its sampler sees a change. Each change publishes a configuration and a settings snapshot over shared-carrier subscriptions; a refused candidate keeps the previous snapshot and publishes diagnostics, and an unparsable file keeps the last valid settings.                         |
| Worker                                  | No settings file           | Server dispatch carries the launch intent; the Worker resolves device facts, such as System shell discovery, itself.                                                                                                                                                                                                                                                                                      |
| Local Session activation                | The `config.toml` location | Freezes the location the Server reads and refuses only one it cannot admit. A file that is missing at an explicit location, unreadable or too large never stops the start, because the person needs a terminal to repair it: the Server starts on the defaults, publishes the file as `unreadable`, refuses edits while the file exists but cannot be read, and creates a missing file on the first edit. |
| `channel show`, `channel set`, `update` | `update.channel`           | Release operations run without a Session.                                                                                                                                                                                                                                                                                                                                                                 |
| `config check`                          | The whole `config.toml`    | Validates the file the user edits; applies no runtime policy.                                                                                                                                                                                                                                                                                                                                             |

Every other Client consumer, including the TUI, Desktop windows, and CLI creation defaults, uses the Server's snapshots.

## Apply timing and capability boundaries

Every shared key persists through the same edit of `config.toml`. The
[settings registry](../../../crates/local-runtime/README.md#user-settings) owns keys, validation and
defaults. The native page applies each change at once and names new-terminal effects beside the row
name:

| Key                                | Before commit                                                                 | Effect after a confirmed edit or external observation                                                                   |
| ---------------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `locale`                           | Whole-frame authored-copy preview; System uses the startup locale environment | Current application copy and later localized feedback; program bytes are unchanged                                      |
| `applicationTheme`                 | Whole-frame chrome preview                                                    | Current chrome; independent of terminal display theme                                                                   |
| `lightTheme`                       | Picker candidate previews at Light with Colors set to Theme                   | Chrome and Theme terminal defaults while the light slot paints on a truecolor host                                      |
| `darkTheme`                        | Picker candidate previews at Dark with Colors set to Theme                    | Chrome and Theme terminal defaults while the dark slot paints on a truecolor host                                       |
| `tuiColorSource`                   | Whole-frame Theme or Terminal colors preview                                  | This Client's chrome source and Theme terminal defaults; the GUI ignores it                                             |
| `terminalDisplayTheme`             | Theme/Host/Custom default-color preview                                       | Existing live and retained terminal presentation; explicit program colors remain exact                                  |
| `customForeground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked foregrounds in Custom; retained while other display themes are selected                         |
| `customBackground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked backgrounds in Custom; retained while other display themes are selected                         |
| `copyOnSelect`                     | No preview                                                                    | Later gesture completion, including a gesture already in progress; explicit Copy remains available                      |
| `trimTrailingBlankRowsForAgents`   | No preview                                                                    | Existing detected Agents through the exact retained-window/acknowledgment owner; user selection/history retains control |
| `resumeAgentsOnRestore`            | No preview                                                                    | The Server's next restart from its checkpoint; Off also stops recording Agent session IDs at its next capture           |
| `restoreTerminalOutput`            | No preview                                                                    | The Server's next capture keeps recent output and its next restart replays it; Off deletes every kept copy at once      |
| `agentNotificationMode`            | No preview                                                                    | Future eligible hidden-Agent transitions, without replay; host delivery and per-Agent mute still apply                  |
| `completionReminderMode`           | No preview                                                                    | Future fresh command completions; requires shell integration and respects focus policy                                  |
| `completionReminderMinimumSeconds` | No preview                                                                    | Later completion evaluation at the new duration threshold                                                               |
| `soundEnabled`                     | No preview                                                                    | Eligible later reminders; host sound capability and per-Agent mute still apply                                          |
| `scrollbackBytes`                  | No preview                                                                    | Newly created terminals capture the limit; existing/restored terminals keep theirs                                      |
| `launchProfiles`                   | Child-page edits apply as one edit on leaving                                 | Subsequent default launches capture the catalog/default; existing programs keep their launch facts                      |

Native host configuration is a separate admitted surface, not extra shared-editor
rows. Windows TOML changes publish when the Server's sampler sees the edit, about
once per second, or immediately through `server reload-config` (default prefix F5). Configuration and settings use separate state subscriptions on the
Client's shared carrier, including while Settings is closed. The UI consumes their snapshots
on its existing tick, whose input wait is at most 50 ms; neither observer polls the Server.
An edit the Server commits publishes at once. An edit by another Session or by hand reaches the
Server through the same sampler; existing dispatcher work and scheduling can delay the sample and
its delivery, so this is not a strict one-second observation guarantee. macOS and Linux Servers do
not sample yet. Neither publication is an acknowledgment from every connected Client.

| Admitted native configuration or Client state                                                                                                      | Actual application policy                                                                                                                                 |
| -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `keys.prefix`, implemented action overrides, `keys.commands`                                                                                       | Complete admitted reload updates dispatch and its help/hints together; clears pending prefix; does not execute configured commands merely by loading them |
| `appearance.sidebar.agents.rows`, `rows_by_agent`, `row_gap`; `appearance.sidebar.workspaces.rows`, `row_gap` and their admitted token/rule fields | Complete reload changes existing sidebar projections; layout and interaction remain with their sidebar owners                                             |
| `appearance.window_title`                                                                                                                          | Live title subscription uses the admitted template after reload; enclosing-host title delivery remains a capability                                       |
| `notifications.cli_delivery`, `delay_seconds`, `position`; `notifications.sound_by_agent`                                                          | Reload updates manual delivery and later automatic feedback; confirmed shared Agent mode/global sound remain authoritative; per-Agent mute is preserved   |
| `terminal.focus_new_panel`                                                                                                                         | Publication updates the split focus preference for later splits; defaults to true                                                                         |
| `worktrees.directory`                                                                                                                              | Publication updates native Worktree launch for later queries and starts; Windows CLI creation reads the published value when it runs                      |
| `terminal.new_cwd`                                                                                                                                 | Windows CLI creation reads the published value when it runs; does not move an existing process                                                            |
| `agent_detection.auto_update`                                                                                                                      | Reload updates the existing rule-check scheduler; automatic checks retain their managed-release/capability restrictions                                   |
| `update.channel`                                                                                                                                   | Consumed by subsequent release/update operations; does not recolor or restart the current TUI                                                             |
| Pointer capture (`--mouse off`, default prefix `m`), sidebar visibility/width                                                                      | Immediate Client-local interaction state; these actions do not persist a setting                                                                          |

Unknown configuration keys warn and are ignored; loading them does not change Client behavior. Fonts, pixel size, opacity and physical Option-key handling
belong to the enclosing host. Clipboard, terminal/OS notifications and sound need
host support; the TUI cannot promise delivery from a saved value. No new host mode,
restart, process launch or synthetic completion event is caused by appearance preview.

## Correctness and evidence

The [editor cases](../crates/workbench-client/src/native_settings/tests.rs) cover the complete
shared metadata with one place for every setting, legacy carriage return, one edit per change with
only its row pending while navigation stays free, refusal and confirmation of an edit with an outside
change in flight, one restore per changed row (including both launch-profile keys), the narrow
column steps, one edit for a theme line's slot and appearance, and the unusable-file line. The
[page render cases](../crates/workbench-client/src/tui_render/settings/tests.rs) cover one column
below 56 columns and two above, always-visible descriptions in both locales, and refusal below the
page minimum. The [whole-frame cases](../src/tui_shell/tests/settings.rs) drive the real Shell
drawing boundary with an existing native terminal, both locales, 120x40 and 50x20, and
truecolor/256/16/monochrome capability. They compare a previewed candidate on the page's status row,
the shell's unchanged confirmed state, an outside change shown at once on the open page, and covering
Client capture. These deterministic witnesses do not establish clipboard, notification, sound or
physical emulator delivery. The configuration watcher cases also retain confirmed shared appearance
and pointer-release state while updating live bindings, their hints and both sidebar profiles
together.

The [integration settlement cases](../crates/workbench-client/src/native_settings/integrations/tests.rs)
cover unknown results of a change that may have begun, definite refusals before start, unlisted
Agents after Refresh and the diagnosis split between definite findings and unknown results.

`SettingsCommitPublication.tla/.cfg` bounds two Clients that each edit their own key through
the Server and one external editor that ignores the lock, changing values or breaking the syntax
twice. An edit holds the Server's reload admission from its read to its publication, and the
sampler reloads under the same admission. `Read` records the file's content; `Replace` writes only
while that content is unchanged; `Redo` reads once more after a change and `RefuseChanged` refuses
the second one. A file that does not parse refuses the edit, is never written and publishes its
syntax error over the last valid values.

Checked properties:

| Property                        | What it checks                                                              |
| ------------------------------- | --------------------------------------------------------------------------- |
| `OneWriter`                     | An edit reads, writes and reloads only while it holds the reload admission. |
| `OtherKeysPreserved`            | A write changes only its own key of the file it replaces.                   |
| `UnparsableFileNeverWritten`    | A file that does not parse is never replaced.                               |
| `SnapshotIsNewest`              | The current snapshot has the highest published revision.                    |
| `PublicationIdentity`           | A revision names exactly one snapshot.                                      |
| `RevisionAdvances`              | Each publication advances the revision by one.                              |
| `PublishedValuesWereFileStates` | Every published value set was once the whole file; no snapshot mixes two.   |
| `SnapshotShowsLastValid`        | A syntax error keeps the last valid values in the snapshot.                 |
| `RefusalChangesNothing`         | A refused edit writes nothing and publishes nothing.                        |
| `ReplyIsCurrent`                | An edit replies with the snapshot its own reload published.                 |
| `FileEventuallyPublished`       | Once writes stop, the snapshot shows the file's values or its syntax error. |

The negative control removes the content recheck from `Replace` and must produce an
`OtherKeysPreserved` counterexample. TOML bytes and comments, the cross-process lock, unreadable
files, live-policy apply failure, absent-file defaults and filesystem primitives remain
obligations of the editor, snapshot and Server configuration tests. GUI/TUI editor tests,
field-focused correction, launch capture, terminal
presentation and input tests cover the Client boundaries. Actual bidirectional GUI/TUI edits,
reopen, reset, errors and visible effects remain acceptance requirements.

Run `node scripts/tla/run-models.mjs --models native-settings-save` and the owning
Cargo/TypeScript checks. The affected root gate remains required.

## History units and host-color display

History capacity displays and edits decimal MB (1 MB = 1,000,000 bytes), from 0 to 1000 MB. Up to six fractional digits preserve individual bytes; storage and the step retain their existing byte domain: 1,000,000 bytes, and 1 second for the completion threshold. Disabled color fields show known effective RGB defaults; an unresolved host default shows its localized choice label instead of fallback palette values. Agent blank-space trimming uses plain-language copy. These are presentation and entry-conversion changes; commit and new-terminal effect boundaries stay with the existing owners.

## Color-disabled guidance

The Shell retains whether a nonempty `NO_COLOR` disabled colors at startup and carries that reason through committed and preview themes. Appearance, theme slot, Colors, terminal theme and editable color descriptions explain the suppression in both languages. Validation, confirmation and unknown-save recovery messages take precedence. The hint never infers `NO_COLOR` from monochrome alone, reads the environment while rendering, or changes the saved theme choice.
