# Native settings

## Interaction and scope

`Ctrl+b ,`, the sidebar Settings row and the global action menu open Settings on supported native
TUI hosts. Settings is a large dialog over the workspace that captures input while pane output continues. It
covers every common setting both Clients present in the
[settings registry](../../../crates/local-runtime/README.md#user-settings), including launch profiles
and new-terminal scrollback defaults, plus the TUI-only
completion-reminder threshold and the prefix key. Host-owned fonts, pixel metrics and management
workflows stay with their own owners. The native Settings feature maps each shared key once to its
row, group, category, control kind, availability, integer unit, bounds, step and effect horizon. The
settings registry owns keys, defaults and validation, and Workbench Protocol the value types and
their bounds.

The page follows the Desktop page's structure, wording and order in both locales. The left column
holds Back, search and the categories General, Appearance, Themes, Terminal, Notifications, Keyboard,
Agents, Machines on Windows and macOS, and About. The right column holds the category title, a line naming the settings file when it is
unusable, and framed groups under their headings. Every row shows its name, a one-line description
that is always visible, and its control at the row's end. Themes shows the light list and the dark
list; the list the current appearance paints is tagged in use, and System follows the canvas. A row
that only applies in some state, such as custom colors outside Custom or the reminder threshold while
reminders are off, is hidden until it applies. Terminal's After a restart group holds one switch,
`terminal.restore_after_restart`, which resumes supported Agents and replays every other terminal's
kept output. About's unnamed group ends with Automatic updates (`update.automatic`) under the
Updates row; it shows only while the settings registry marks that key for the page, which the
[automatic updates proposal](../../../proposals/platform/automatic-updates.md#phases) turns on with
its slice 1 switch, and a search lists it alone. Agents holds the Allow routine confirmations group: a
described heading, one switch for each Agent type the Server can answer for
(`agents.auto_allow.<agent>`), and a closing line inside the frame that asks for more Agents in a
GitHub issue. The page's last line names More options in `config.toml` and
its path; a path too long for the line drops its leading folders first, so the file name stays.
Text that does not fit its cells ends in the ellipsis at a grapheme boundary, counted in display
cells, and every TUI description fits its row in a dialog 100 columns wide inside, which a window
of 106 columns gives, in both locales, shortened from the Desktop wording where needed while keeping
its meaning. The dialog is at most 112 columns by 36 rows, centered in the window inside the overlay
margins; its frame carries the title and `[x]`, and a press outside it closes it. Below 56 inner
columns it shows one column at a time; Esc or the title's Back steps from rows to categories before
it leaves. The dialog requires 24 by 8 cells inside its frame. A frame below that is not presented: before the first successful
frame the page is refused with the enlarge-and-reopen notice, and later it suspends drawn interaction while staying the selected capture.

The page's last line is its status line. It names the keys for the focused row, or shows a Settings
notice. A notice about the file's state leaves once a snapshot finds the file usable; a refusal of
one change leaves with the next change sent or when the page closes. A row editor holding a text the
page refused names what it accepts there instead, in the Desktop's words, until the text changes.
Width drops trailing hints first; a narrow page, which shows no search field beside its rows, ranks
search and restore ahead of the arrow keys. Up/Down, PageUp/PageDown and Home/End move within the
focused column; Left or Shift+Tab moves to the categories and Right, Tab or Enter to the rows. `/` or the painted search field starts global search. The bounded query matches every term
against English and Simplified Chinese category names, labels and descriptions in stable category
order; values and user-authored profile text are excluded. Search shows its result count and an
explicit empty result; Enter or Tab focuses the matching row without changing it, and Esc clears the
query before stepping back. Text fields use the
[shared bounded line editor](native-line-editor.md), including Ctrl+w word deletion and literal
paste.

## Changes

Every change applies at once: there is no draft and no Save. A change sends one `workbench.settings`
`edit` naming only its key. A toggle applies on Enter, Space or a click. A choice opens an inline list
whose candidate previews when it is an appearance field and applies on acceptance. A text or number
row edits in place, applies on Enter, and Esc keeps the confirmed value. Left/Right and the painted
minus/plus controls step a number by its step and apply that step. Picking a theme line sends its slot,
and the appearance that shows it in one edit; the focused uncommitted line previews, and Esc returns
to the theme in use. Launch profiles open as a child page whose edits apply as one edit on leaving;
an invalid catalog first focuses the failing field, and leaving again discards it. Add profile
lists the installed shells, then Custom…, in the choice list; a shell adds a profile running it under
its name and Custom… adds a system-shell profile, each focused on its name. Custom colors are
unavailable unless Custom is selected and login shells are unavailable on Windows; a focused
unavailable row names its reason.

One edit is in flight at a time, and nothing the person does is dropped silently. Every key of one
input packet applies in order on the state the keys before it left, Enter and Del included; only a
key that closes the page keeps the rest of its packet. Changes made while one is in flight wait in
order and are sent one at a time, each after the previous one is answered,
so the Server still gets one row's keys per edit; restore takes the same turn. A second change to a
row whose change has not been sent yet replaces that waiting change. Up to 16 changes wait; a newer
request beyond them is refused with the Settings busy notice and changes nothing. A row with a change
in flight or waiting shows the last value asked for and the pending state, while every other row,
navigation and Back stay usable. A confirmed edit adopts the returned snapshot without a success
notice. A refused or unknown edit puts its row back to the file value, drops that row's later waiting
change (it was based on a value that never existed) while the other rows' changes keep their turn,
and shows the Server's localized reason on the status line: the file was busy, a value or key
was refused, the file has a syntax error, cannot be read or is not a regular file, is read-only or
would be too large, its location cannot be used, or the write failed. A syntax error points to
`orchbee config check` for the position. A snapshot that finds the file unparsable or unreadable
drops every waiting change and names that reason once. No request is replayed automatically.

A row whose key the file sets carries a changed marker after its title, effect included. Del on a
changed row, or its painted Restore
default action, sends one `restore` for that key; the launch-profile row restores both platform keys.
The focused changed row paints the action on its first line, before the row's control as on the
Desktop, and it never takes cells from the name or the description: when the first line has no room
it moves to the end of the description line while the whole description fits beside it, and
otherwise only Del offers it.
A theme list whose slot the file sets carries the marker after its heading, and Del on any of its lines
restores that slot alone; the appearance keeps its own row. There is no page-wide reset.

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

## Machines

Machines holds no setting: it shows no settings-file line, search skips it, and its rows never
reach the Server's settings; the TUI carries out what the person asks there under the rules of
[Settings › Machines](machines.md#machines-in-settings). The category opens with Add machine…, then
Find a machine once more than eight machines are saved, then a framed list of this computer and every
saved machine. A machine's row opens its page, titled with its name: Name, Address, Status, Free now
while the machine is online and Saved password while the system's password store can be used, then
a closing group of Stop its terminals and Remove machine. Every row takes two lines, its name with its value
or button first and its description below; the description is always visible. A button is drawn
as Updates' is, whole at the row's end, and Stop its terminals and Remove machine take the destructive
tone. The first Enter on Stop its terminals puts what stops on the description line in the warning
tone; until the second Enter or Esc, no other key acts. Esc on the list first clears its filter. How
a request ended shows on the status line until the next input.

## About

About closes the category list and shows no settings-file line. Its unnamed group holds Version,
which names this Client's version and its build kind (release, test or development), Updates, and
the Automatic updates switch; search lists only that switch, and Version and Updates never reach the
Server's settings. Version has
nothing to activate, so the status line names no primary key on it, nor on Updates while a check
runs or none can.

Updates ends in a button drawn by the shared button control: Check for updates, or the Update
control's Update while the release store names a newer version, with the control's own sentence as
its description. The button takes the ordinary fill, or `[ Label ]` brackets in monochrome and
NO_COLOR, and stays whole: the row's name gives up cells to it. The whole row is the button's target,
so the pointer anywhere on the row shows hover and press on the button and the row takes no pointer
surface. A row too narrow for the whole button beside two cells of its name shows the label as cut
text instead, as other rows show a value. A press on Check for updates
sends `workbench.update-check` `start` to the Session's Server, which runs its scheduled
automatic-update job now through the same helper and state ([automatic updates
proposal](../../../proposals/platform/automatic-updates.md#manual-check)). The row then shows
Checking… and reads the run with `read` about once a second while About shows. A run that ends up to
date or failed says so on the description line, in `status-success` or `status-error`; a run that
installed a version stays Checking until the store watcher, asked to read at once, makes the Update
control Ready, and the row then offers Update. Only a run this TUI asked for reports its end: a
scheduled run About saw under way returns the row to its idle description, and returning to About
clears an earlier result. A lost or unmatched answer ends an asked check as failed and never closes
the TUI or the connection. The row is unavailable, with the reason on its description line, in a
development build, where no update writes this platform's release store, on a saved-machine origin,
while the Session's handshake answer is unknown or lacks `update-check`, or after the Server answers
`unavailable`; no request reaches a Server that has not listed the capability.

## Agent integrations

Not offered in this release. The Agents category has no integrations entry in the TUI or GUI, and no setting, page or command lists, explains, enables or disables an integration. Entries an earlier version wrote are left alone; uninstall removes them. The engine's status is in the [Windows integration contract](windows-agent-integrations.md) and the [macOS integration contract](macos-agent-integrations.md).

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
| Open → change        | Require a successful draw of the page; keep the row's edit or restore in order behind the one in flight, or send it at once when none is, and mark that row pending.                  |
| Submit → commit      | Lock the file, apply every change or none in one replace and publish one snapshot; keep other keys and comments. A scalar key set to its default is removed, as a restore removes it. |
| Commit → effective   | Publish only the confirmed values; do not reinterpret a disk receipt as Host application acknowledgment.                                                                              |
| External update      | Show the newest confirmed values at once under the changes still in flight or waiting; an unusable file drops the waiting ones.                                                       |
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
own polarity; Enter applies it and Esc returns to the theme in use. Applying a theme sets its
slot and switches a fixed Appearance of the other polarity, so the applied
theme shows; under System it shows when the host matches, and the list's description says so. A
stored id this build does not ship opens on the Orchbee theme it resolves to and stays stored until
another theme is applied.
Below truecolor the lists stay visible but unavailable, and the line below the title gives the
24-bit reason or the `NO_COLOR` note.
Agent mode/global sound override automatic enablement without erasing host delivery
or per-Agent mute policy. Completion reminders consume physical-terminal sequence
facts once, require a fresh timestamp and minimum duration, and never replay when
a setting is enabled. They appear as localized transient status plus optional bell.

Agent tail folding is always on for a detected Agent and shares the retained-history
read/ack/recovery owner. It only
replaces a proved blank primary-screen suffix with an exactly matching acknowledged
window. Stale, resized, graphic or unavailable windows fall back to the live grid;
the cursor moves with its live row. Ordinary typing remains live. Selection,
scrollbar use and manual history navigation take ownership of the displayed window;
automatic following cannot move active selection/export work. The
[viewport contract](native-terminal-viewport.md) governs the underlying lease.

## Configuration stores and readers

The Session Server and the TUI keep every setting in one store, `config.toml`: the settings a Settings surface edits and the ones users edit only by hand, described in [Windows CLI configuration](windows-cli-configuration.md), live in the same file, and the [settings registry](../../../crates/local-runtime/README.md#published-key-file) lists every key. The chrome theme belongs to `appearance.mode` and the default launch program to `terminal.default_profile.<platform>`; notification policy, from the Agent reminder and global sound to manual delivery and per-Agent sound overrides, sits under `[notifications]`.

| Reader                                  | Reads                      | Rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Session Server                          | `config.toml`              | The runtime authority. It reads `config.toml` at startup, on reload, after each committed edit and, on Windows, when its sampler sees a change. Each change publishes a configuration and a settings snapshot over shared-carrier subscriptions; a refused candidate keeps the previous snapshot and publishes diagnostics, and an unparsable file keeps the last valid settings.                                                                                                    |
| Worker                                  | No settings file           | Server dispatch carries the launch intent; the Worker resolves device facts, such as System shell discovery, itself.                                                                                                                                                                                                                                                                                                                                                                 |
| Local Session activation                | The `config.toml` location | On Windows and macOS, freezes the location the Server reads and refuses only one it cannot admit; a Linux Server resolves it from its own environment. A file that is missing at an explicit location, unreadable or too large never stops the start, because the person needs a terminal to repair it: the Server starts on the defaults, publishes the file as `unreadable`, refuses edits while the file exists but cannot be read, and creates a missing file on the first edit. |
| `channel show`, `channel set`, `update` | `update.channel`           | Release operations run without a Session.                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `config check`                          | The whole `config.toml`    | Validates the file the user edits; applies no runtime policy.                                                                                                                                                                                                                                                                                                                                                                                                                        |

Every other Client consumer, including the TUI, Desktop windows, and CLI creation defaults, uses the Server's snapshots.

## Apply timing and capability boundaries

Every shared key persists through the same edit of `config.toml`. The
[settings registry](../../../crates/local-runtime/README.md#user-settings) owns keys, validation and
defaults. The native page applies each change at once and names new-terminal effects beside the row
name:

| Key                                | Before commit                                                                 | Effect after a confirmed edit or external observation                                                                                 |
| ---------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `locale`                           | Whole-frame authored-copy preview; System uses the startup locale environment | Current application copy and later localized feedback; program bytes are unchanged                                                    |
| `applicationTheme`                 | Whole-frame chrome preview                                                    | Current chrome; independent of terminal display theme                                                                                 |
| `lightTheme`                       | Picker candidate previews at Light                                            | Chrome and Theme terminal defaults while the light slot paints on a truecolor host                                                    |
| `darkTheme`                        | Picker candidate previews at Dark                                             | Chrome and Theme terminal defaults while the dark slot paints on a truecolor host                                                     |
| `terminalDisplayTheme`             | Theme/Host/Custom default-color preview                                       | Existing live and retained terminal presentation; explicit program colors remain exact                                                |
| `customForeground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked foregrounds in Custom; retained while other display themes are selected                                       |
| `customBackground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked backgrounds in Custom; retained while other display themes are selected                                       |
| `copyOnSelect`                     | No preview                                                                    | Later gesture completion, including a gesture already in progress; explicit Copy remains available                                    |
| `restoreTerminals`                 | No preview                                                                    | The Server's next capture keeps output and Agent session IDs, and its next restart restores them; Off deletes every kept copy at once |
| `agentNotificationMode`            | No preview                                                                    | Future eligible hidden-Agent transitions, without replay; host delivery and per-Agent mute still apply                                |
| `completionReminderMode`           | No preview                                                                    | Future fresh command completions; requires shell integration and respects focus policy                                                |
| `completionReminderMinimumSeconds` | No preview                                                                    | Later completion evaluation at the new duration threshold                                                                             |
| `soundEnabled`                     | No preview                                                                    | Eligible later reminders; host sound capability and per-Agent mute still apply                                                        |
| `scrollbackBytes`                  | No preview                                                                    | Newly created terminals capture the limit; existing/restored terminals keep theirs                                                    |
| `automaticUpdates`                 | No preview                                                                    | The Server's next background update check; Off stops the schedule and its downloads                                                   |
| `autoAllowClaude`                  | No preview                                                                    | Confirmations Claude Code shows from now on; Off also stops an answer already decided                                                 |
| `autoAllowCodex`                   | No preview                                                                    | Confirmations Codex shows from now on; Off also stops an answer already decided                                                       |
| `autoAllowGemini`                  | No preview                                                                    | Confirmations Gemini CLI shows from now on; Off also stops an answer already decided                                                  |
| `autoAllowOpencode`                | No preview                                                                    | Confirmations OpenCode shows from now on; Off also stops an answer already decided                                                    |
| `launchProfiles`                   | Child-page edits apply as one edit on leaving                                 | Subsequent default launches capture the catalog/default; existing programs keep their launch facts                                    |

Native host configuration is a separate admitted surface, not extra shared-editor
rows. Windows TOML changes publish when the Server's sampler sees the edit, about
once per second, or immediately through `config reload`. Configuration and settings use separate state subscriptions on the
Client's shared carrier, including while Settings is closed. The UI consumes their snapshots
on its existing tick, whose input wait is at most 50 ms; neither observer polls the Server.
An edit the Server commits publishes at once. An edit by another Session or by hand reaches the
Server through the same sampler; existing dispatcher work and scheduling can delay the sample and
its delivery, so this is not a strict one-second observation guarantee. macOS and Linux Servers do
not sample yet. Neither publication is an acknowledgment from every connected Client.

| Admitted native configuration or Client state                                             | Actual application policy                                                                                                                                 |
| ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `keys.prefix`, implemented action overrides, `keys.commands`                              | Complete admitted reload updates dispatch and its help/hints together; clears pending prefix; does not execute configured commands merely by loading them |
| `appearance.sidebar.workspaces.rows`, `row_gap` and their admitted token/rule fields      | Complete reload changes existing sidebar projections; layout and interaction remain with their sidebar owners                                             |
| `appearance.window_title`                                                                 | Live title subscription uses the admitted template after reload; enclosing-host title delivery remains a capability                                       |
| `notifications.cli_delivery`, `delay_seconds`, `position`; `notifications.sound_by_agent` | Reload updates manual delivery and later automatic feedback; confirmed shared Agent mode/global sound remain authoritative; per-Agent mute is preserved   |
| `worktrees.directory`                                                                     | Publication updates native Worktree launch for later queries and starts; Windows CLI creation reads the published value when it runs                      |
| `terminal.new_cwd`                                                                        | Windows CLI creation reads the published value when it runs; does not move an existing process                                                            |
| `update.channel`                                                                          | Consumed by subsequent release/update operations; does not recolor or restart the current TUI                                                             |
| Pointer capture (`--mouse off`, default prefix `m`), sidebar visibility/width             | Immediate Client-local interaction state; these actions do not persist a setting                                                                          |

Unknown configuration keys warn and are ignored; loading them does not change Client behavior. Fonts, pixel size, opacity and physical Option-key handling
belong to the enclosing host. Clipboard, terminal/OS notifications and sound need
host support; the TUI cannot promise delivery from a saved value. No new host mode,
restart, process launch or synthetic completion event is caused by appearance preview.

## Correctness and evidence

They compare a previewed candidate on the page's status row, the shell's unchanged confirmed state,
an outside change shown at once on the open page, and covering Client capture. The configuration
watcher cases also retain confirmed shared appearance and pointer-release state while updating live
bindings, their hints and both sidebar profiles together.

Two Clients that each edit their own key go through the Server, while an external editor may change
the file at any time without the lock. An edit holds the Server's reload admission from its read to
its publication, and the sampler reloads under the same admission. An edit writes only while the
file's content is unchanged since its read, reads once more after one change, and refuses a second.
A write changes only its own key; a file that does not parse is never written, refuses the edit and
publishes its syntax error over the last valid values. Each publication advances the revision by
one, and every published value set was once the whole file. The owning Cargo and TypeScript tests
check these rules over named traces; no model explores every interleaving.

## History units and host-color display

History capacity displays and edits decimal MB (1 MB = 1,000,000 bytes), from 0 to 1000 MB. Up to six fractional digits preserve individual bytes; storage and the step retain their existing byte domain: 1,000,000 bytes, and 1 second for the completion threshold. Disabled color fields show known effective RGB defaults; an unresolved host default shows its localized choice label instead of fallback palette values. These are presentation and entry-conversion changes; commit and new-terminal effect boundaries stay with the existing owners.

## Color-disabled guidance

The Shell retains whether a nonempty `NO_COLOR` disabled colors at startup and carries that reason through committed and preview themes. Appearance, theme slot, terminal theme and editable color descriptions explain the suppression in both languages. Validation, confirmation and unknown-save recovery messages take precedence. The hint never infers `NO_COLOR` from monochrome alone, reads the environment while rendering, or changes the saved theme choice.
