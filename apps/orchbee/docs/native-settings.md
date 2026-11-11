# Native settings

## Interaction and scope

`Ctrl+B F4`, the sidebar Settings row and the global action menu open Settings
on supported native TUI hosts. The capturing surface edits a
local draft while pane output continues. It covers every common setting both Clients present in
the [settings registry](../../../crates/local-runtime/README.md#user-settings), including launch
profiles and new-terminal scrollback defaults. Host-owned fonts, pixel metrics and management
workflows stay with their own owners.
The native Settings feature maps the thirteen shared keys once to their row,
category, control kind, availability, integer unit, bounds, steps and effect
horizon. The editor, localization and renderer currently consume the row,
category, control, availability and bounded-integer facts. Effect metadata
documents and tests the feature contract; the existing domain owners still apply
those effects. The batch edit, restoring defaults and snapshot publication stay with the
settings editor and the Session Server. The settings registry owns keys, defaults and validation,
and Workbench Protocol the value types and their bounds.

The root surface groups settings into Appearance, Terminal, Notifications and Launch profiles,
and on a local origin adds an [Agent integrations](#agent-integrations) entry.
Each section has a quiet heading and divider; every setting has one stable label track and one
value track. The current setting uses a neutral current-context band and bold label, which
terminal-native also underlines across the row, while the modal frame identifies keyboard
ownership. Checked booleans, `~` changed-draft markers, disabled
values and picker navigation have independent cues. Reset all, Reload, Close and Save remain in
a fixed action area below the list; they never appear among scrolling settings. The root keeps
one contextual description and one state line rather than a persistent preview paragraph or
row-index counter. A narrow surface removes the contextual description before any required action
and uses two fixed action rows; labels truncate by display width and exact integers keep their value.

`/` or the painted Search row starts global search. The bounded 256-byte query matches every term
against English and Simplified Chinese category names, labels and concise descriptions in stable
category order; values and user-authored profile text are excluded. Only categories with results
remain. Search shows its result count and an explicit empty result. Enter or Tab leaves query entry
and focuses the matching row without editing it; Enter with no result stays in search. Esc clears
an active search or retained filter before closing the root. Filtering preserves the draft and
saved source. A nonempty filter's Back button takes the same local step as Esc; header Close always
closes the whole surface.

Tab/Up/Down moves between settings and fixed actions; PageUp/PageDown moves by a page, and
Home/End reaches the first/last focus target. Left/Right changes a finite choice or bounded value
directly in the draft. Enter/Space or a row click opens a finite-choice selector in the same modal;
boolean activation toggles its draft. A breadcrumb names a child editor or selector, whose concise
description and Cancel/Confirm footer replace root content and actions. Picker navigation changes
only a provisional candidate; appearance candidates preview locally, while other choices have no
runtime effect. Acceptance changes the parent draft, and cancellation restores its prior preview.
Child footers measure complete localized Cancel/Confirm labels and their Esc/Enter bindings.
When the pair cannot fit side by side, it occupies two fixed rows before the child viewport is
allocated. A minimal picker retains at least one choice row, and a text editor retains its
entry and cursor. Confirmations admit only their complete localized decision and both controls;
unsupported geometry publishes no controls. Preflight and painting consume the same resolved regions.
At compact height, picker wheel movement scrolls its own viewport and relocates only the
provisional cursor to the nearest visible choice; it never accepts a value or moves the parent viewport.
`Ctrl+s` invokes the fixed Save action only from root browsing, using the same painted authority as
its button. Text
fields use the [shared bounded line editor](native-line-editor.md), including Ctrl+W word deletion,
and literal paste. Rendering projects the complete raw value once and maps its cursor to a visible
grapheme boundary, so removing controls cannot leave the caret inside a painted cluster. Launch profiles
have their own page, one literal argument per editor, explicit default selection,
and confirmation before deleting a profile. Entering that child records the parent
row, viewport and focus; Back restores them while retaining accepted draft edits. Custom colors are disabled unless
Custom is selected; login shells are unavailable on Windows. The selected row
is revealed by keyboard navigation and remains a stable navigation anchor while the
pointer scrolls or the surface resizes. Setting names preserve the GUI's
English/Simplified Chinese vocabulary. Native category grouping and keyboard navigation adapt that
shared inventory to the cell grid; GUI behavior is unchanged.

An empty custom executable is marked Required. Launch-field details identify the
profile, field and value requirements in both locales. A failed Save reveals the
first invalid profile field, including a non-default profile and the exact literal
argument, without changing other draft values, the saved basis or the default
profile. Back restores the Save location; correction or confirmed removal/reset
allows another intentional Save. Editing, removal, reset and matched Reload retire
old validation feedback. Catalog structure failures retain the generic refusal.

The completion threshold stepper uses 1-second small and 10-second large steps.
Scrollback uses 1,000,000-byte small and 10,000,000-byte large steps. Steps preserve
the exact current value, saturate at the owned domain bounds and never round to a
multiple. Left/Right and painted minus/plus controls use the small step;
Shift+Left/Right uses the large step outside a text child. Enter retains the exact
bounded editor for direct values.

Settings centers horizontally and vertically in the full host viewport, including the sidebar, top
strip and last row; the floating status reserves no row of it. Its preferred extent is 92 columns by 30 rows, clamped
to that viewport; sidebar visibility or width does not shift it. Existing minimum-content admission,
refusal and suspension preserve drafts and keep the status chip uncovered. Placement does not resize
PTYs. Root-compositor cases cover both locales, 120x40, 120x30, 100x24 and 80x24, hidden/visible
sidebars and 24/30/60-column preferences; these are render/input-map checks rather than physical
host gesture evidence.

Settings preflight and painting consume the theme's `paired_pointer_surfaces` projection. Reverse-video
hover/press fallbacks become one band of primary text on chrome across the complete row, including
labels, values, markers and blank cells; in terminal-native and Monochrome it is the host's own
reverse band, as on the Tab strip. Content in the band takes its foreground without DIM; picker
choices and fixed footer actions use the same projection. Named RGB surfaces keep their palette,
and explicit content colors keep their roles on them. This local theme copy changes presentation
without changing hit geometry, pointer admission, selection, the draft or save authority.

The renderer projects typed shared actions, rows and value controls. Header Close,
footer actions, rows and minus/plus controls publish the same generic
activate-on-release interaction, including visible typed disabled targets that
cannot arm, activate or click through. Plain left release must match the press's
exact current token and rectangle; keyboard edits, focus loss or layout changes
revoke that press. Pointer capture currently admits button events only, so these
controls do not claim ordinary hover feedback until the host admits any-motion
delivery. Wheel is admitted only inside the active list: three rows per notch,
bounded per pass by its visible height, without wrapping or changing the selected
row, semantic focus, draft or effective value. It changes the requested viewport
and stops automatic selection following. The exact `SettingsDraw` receipt carries
the surface token, category-list viewport and actual painted setting/action identities: an anchor that remains painted
retains row activation, while a nonpainted anchor cannot activate. Keyboard
navigation reveals that anchor again; a painted pointer target may establish a new
one. Child editors and confirmations retain input and expose their own
Confirm/Cancel. Confirmations place their effective Esc/Enter bindings inside the
Cancel/Confirm buttons in secondary, normal-weight text, without a duplicate hint
row. Only unmodified Enter confirms; Space remains a list choice or text input,
never a confirmation. Explicitly modified Enter and key releases cannot confirm;
legacy carriage return remains supported. Esc backs out of a child editor or confirmation before
closing the root. Header Close closes the whole surface. Only an idle General
draft admits a plain left press and matching release on the surrounding blank
backdrop through that same exact painted Close target. Unsaved changes, search/filter, text entry,
a choice picker, the launch-profile child, confirmations, reconciliation and an in-flight read or
save retain the surface and its task on an outside click. This guard uses the same
active-task predicate that freezes the draft basis on external updates. Moving
into the surface, a stale token, focus loss or a changed layout cancels a backdrop
gesture. Explicit Close and Esc retain their existing local exit behavior;
deliberately closing hides an accepted save without cancelling it.

Save is the only write action. Opening, editing, cancelling and resizing do not
publish runtime policy. Language, appearance, theme slots, Colors and terminal default colors
preview locally across the whole frame, including background chrome, live and
retained terminal rows, and hints. Explicit program colors and terminal bytes
remain owned by the program. Invalid color text keeps the last accepted draft
color. Closing restores the newest confirmed appearance; accepting a color field
or returning from the launch-profile child retains the parent preview.

Preview carries only appearance fields. Locally changed leaves overlay the newest
confirmed effective appearance without rebasing the saved basis; unrelated external
updates therefore do not erase preview. A same-key conflict keeps the local preview
and existing conflict/Reload gate. Reset-all previews every default only after its
confirmation. A visible pending Save keeps preview; hiding it releases preview while
the accepted commit continues. An unknown result withdraws preview until an explicit
matched read. A covering Client surface uses confirmed appearance and restores the
retained preview when Settings becomes the active capture again.
Enter, `Ctrl+s` or a click on the successfully drawn Save button consumes its
authority exactly once. Save sends one batch `edit` holding only the keys the draft changed.
Reset-all requires confirmation and remains a draft until Save; it restores every common key to
its default even if its value already is the default. The launch-profile page also offers a
confirmed catalog-only reset that restores this platform's profiles and default profile,
preserving other draft settings. Clean open views follow confirmed external changes; active
edits, text entry, the launch-profile child and confirmations retain their original
saved basis while separately exposing the newest confirmed effective snapshot and a
conflict when its values differ. A snapshot whose revision is not newer than the last one
observed is dropped. Reload confirms discarding edits and performs a new read. A refused edit
wrote nothing; it keeps the draft and names its reason: the file was busy, a value or key was
refused, the file has a syntax error, is read-only or would be too large, its location cannot be
used, or the write failed. A syntax error points to `orchbee config check` for the position. A
definite refusal can retry only from a fresh rendered context. A `changed` refusal or unknown
outcome disables Save until a matched explicit Reload read succeeds; passive observations,
redraws, cached snapshots and old read completions do not clear that requirement. No request is
replayed automatically.

After child and search return steps, Esc discards an unsubmitted root draft; after submission it hides the surface while
the accepted save can continue. Captured packet/paste tails remain owned through
cancellation and the next redraw. The pane cursor stays hidden under Settings.
The surface requires at least 36 columns by 10 rows and complete localized footer
actions. A confirmation also requires its complete decision text, including draft
scope, so it can require more width. If the first evaluated frame
cannot meet that geometry, the opening surface is refused and hidden with a localized
enlarge-and-reopen notice; an already accepted read or save still settles under its
original token and cannot reopen the surface. After at least one successful frame,
smaller geometry suspends drawn interaction authority while retaining Settings as
the selected recovery capture: non-Escape input is consumed and Escape closes
locally, so input cannot fall through to a pane or retained workflow underneath.
The draft, frozen basis, effective snapshot, child state, viewport and focus anchor
remain for the next successful frame. A missing receipt caused by another owner
covering the surface revokes exact draw authority but is not a geometry refusal;
that covering owner alone holds top capture while Settings remains retained.
Pending feedback begins after 200 ms. The ordered settings
lane has a two-second exchange deadline and a state subscription on the shared carrier.
A save remains a one-shot request; snapshot revisions order the subscription's observations and
the replies, so no fresh subscription follows a write. Closing cancels its requests and
subscription without a forwarding thread.
A replacement Session/editor episode cannot inherit an old completion.

## Agent integrations

The General list ends with an Agent integrations entry on a Session reached through a local origin.
A saved-machine origin omits the entry, as the GUI offers the page only in Desktop: remote hosts
stay manual. Search matches the entry's title, its description and every Agent name. Activating
the entry opens a child page, like Launch profiles, that never joins the preference draft; Back or
Esc returns to the entry, and the draft, filter and saved basis are unchanged.

The page lists the same twenty Agents as the GUI in the Agent integration contract's canonical
order, under a "Local Agent status adapters" heading, then a disabled "Remote hosts stay manual"
row with the GUI's manual-only description. Each Agent row carries a checkbox for the enabled
state and the GUI's status word, with the same precedence: Working, Manual setup, Status
unavailable, Agent not found, then the reported state. Rows start Unknown and change only from a
Worker report. Entering the page is the explicit request to inspect: it starts one refresh unless
a job already runs. Opening, navigating, searching and closing Settings read and write nothing,
so without that request no provider command runs and no provider configuration is read. The
footer offers Refresh status and Back, and the state line shows the last failure, a running job,
the not-yet-checked hint or the GUI's disclosure.

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

| Record field | Contract                                                                                                                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | GUI and TUI users edit the same device preferences without lost updates or divergent defaults.                                                                                                                |
| Inputs       | Frozen `config.toml` location, complete confirmed snapshot and its revision, the changed keys, painted-row receipt, explicit save/reset intent.                                                               |
| Authority    | Workbench Protocol owns values/bounds; Local Runtime owns the settings registry and the locked batch edit of `config.toml`; the Session Server serves and publishes it.                                       |
| State        | Newest effective runtime values, independently frozen draft basis, local changes, viewport/focus anchor and painted activation authority, one admitted exchange, and explicit refusal/unknown reconciliation. |
| Effects      | Confirmed snapshots govern runtime preferences; an independent reversible projection previews only authored copy, chrome and terminal default colors. Launch/default changes affect new terminal creation.    |
| Refusal      | A syntax error, unknown key, refused value, changed or busy file, unusable location, busy lane or failed write preserves existing data and confirmed behavior.                                                |

The Session freezes its `config.toml` location at startup. Clients send no paths, raw TOML, alternate device selector or arbitrary file write; an edit names registered keys only. An absent file supplies every default, and a file that stops parsing keeps the last valid values. See the [settings contract](../../../crates/workbench-protocol/README.md#native-settings-contract).

| Transition           | Required behavior                                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Hidden → open        | Read a complete snapshot and freeze it as the draft basis.                                                                            |
| Draft → edit / draw  | Keep edits local, project reversible appearance to the whole frame, and revoke old drawing authority.                                 |
| Draft → save         | Require a current successful draw, freeze a nonempty batch edit and consume submission before dispatch.                               |
| Submit → commit      | Lock the file, apply every change or none in one replace and publish one snapshot; keep other keys and comments.                      |
| Commit → effective   | Publish only the confirmed values; do not reinterpret a disk receipt as Host application acknowledgment.                              |
| External update      | Publish newest effective values and refresh clean views; preserve active edits or child tasks and their frozen basis with a conflict. |
| Refused / unknown    | Preserve the visible draft and effective values; require a matched explicit read before another Save after `changed` or unknown.      |
| Geometry unavailable | Refuse an unpainted incarnation; suspend and retain an admitted one until a successful frame resumes it.                              |
| Cancel / close       | Discard unsubmitted edits and reveal newest confirmed appearance; hiding after submission cannot roll back a commit.                  |

Application themes change chrome independently of terminal colors. Host terminal colors under
application System, and Theme or Host under Terminal colors, keep the outer terminal's own
defaults; the other terminal choices change only default markers, preserving explicit program
colors and color-query semantics.
Disabled foreground/background rows show the effective display colors rather than unused Custom
values; reduced-color hosts show the selected terminal appearance when RGB is unavailable.
Copy-on-select governs later gesture completion; explicit Copy remains available.

Appearance holds Appearance, Light theme, Dark theme and Colors, with the GUI's wording in both
languages; theme names stay untranslated. A theme picker lists the shipped themes of its polarity
in catalog order, Orchbee first, and scrolls when they outgrow the viewport. Each row ends in a
swatch, a foreground sample on the theme's canvas and five ANSI fills, shown only at truecolor and
where the row has room. Moving through a picker previews the candidate at its own polarity with
Colors set to Theme; Enter accepts it into the draft and Esc restores the prior preview. Accepting
a theme sets its slot, switches a fixed Appearance of the other polarity and sets Colors to Theme,
so the accepted theme shows; under System it shows when the host matches, and the slot's
description says so. A stored id this build does not ship opens on the Orchbee theme it resolves
to and stays stored until another theme is accepted. Reset all restores both slots and Colors.
Below truecolor the three rows stay visible but unavailable with the 24-bit reason, and an open
theme picker closes when the host loses truecolor.
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
defaults. The native editor exposes these effect
horizons in localized row help:

| Key                                | Draft behavior                                                                | Effect after confirmed Save or external observation                                                                     |
| ---------------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `locale`                           | Whole-frame authored-copy preview; System uses the startup locale environment | Current application copy and later localized feedback; program bytes are unchanged                                      |
| `applicationTheme`                 | Whole-frame chrome preview                                                    | Current chrome; independent of terminal display theme                                                                   |
| `lightTheme`                       | Picker candidate previews at Light with Colors set to Theme                   | Chrome and Theme terminal defaults while the light slot paints on a truecolor host                                      |
| `darkTheme`                        | Picker candidate previews at Dark with Colors set to Theme                    | Chrome and Theme terminal defaults while the dark slot paints on a truecolor host                                       |
| `tuiColorSource`                   | Whole-frame Theme or Terminal colors preview                                  | This Client's chrome source and Theme terminal defaults; the GUI ignores it                                             |
| `terminalDisplayTheme`             | Theme/Host/Custom default-color preview                                       | Existing live and retained terminal presentation; explicit program colors remain exact                                  |
| `customForeground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked foregrounds in Custom; retained while other display themes are selected                         |
| `customBackground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked backgrounds in Custom; retained while other display themes are selected                         |
| `copyOnSelect`                     | Draft only                                                                    | Later gesture completion, including a gesture already in progress; explicit Copy remains available                      |
| `trimTrailingBlankRowsForAgents`   | Draft only                                                                    | Existing detected Agents through the exact retained-window/acknowledgment owner; user selection/history retains control |
| `resumeAgentsOnRestore`            | Draft only                                                                    | The Server's next restart from its checkpoint; Off also stops recording Agent session IDs at its next capture           |
| `restoreTerminalOutput`            | Draft only                                                                    | The Server's next capture keeps recent output and its next restart replays it; Off deletes every kept copy at once      |
| `agentNotificationMode`            | Draft only                                                                    | Future eligible hidden-Agent transitions, without replay; host delivery and per-Agent mute still apply                  |
| `completionReminderMode`           | Draft only                                                                    | Future fresh command completions; requires shell integration and respects focus policy                                  |
| `completionReminderMinimumSeconds` | Draft only                                                                    | Later completion evaluation at the new duration threshold                                                               |
| `soundEnabled`                     | Draft only                                                                    | Eligible later reminders; host sound capability and per-Agent mute still apply                                          |
| `scrollbackBytes`                  | Draft only                                                                    | Newly created terminals capture the limit; existing/restored terminals keep theirs                                      |
| `launchProfiles`                   | Child edits remain in the parent draft                                        | Subsequent default launches capture the catalog/default; existing programs keep their launch facts                      |

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

The [browse/choice traces](../crates/workbench-client/src/native_settings/tests/browse.rs) cover bilingual all-term search,
value exclusion, atomic query bounds, empty results, exact one-leaf picker acceptance, cancelled
appearance preview, external changes, stale pointer choices and filtered profile return. The
[composition cases](../crates/workbench-client/src/tui_render/settings/tests/browse.rs) pin the category/label/value body,
fixed actions across scrolling and size tiers, hidden-hit removal, search cursor and same-modal
picker acceptance in both locales. The [input packet cases](../crates/workbench-client/src/terminal_input/dispatcher/tests/settings.rs)
keep query entry and local return out of pane input and persistence. These local navigation changes
do not change the shared commit/publication protocol.

The [composed pointer cases](../crates/workbench-client/src/tui_render/settings/tests/pointer.rs) drive admitted motion and
press reports through the painted Settings hit map. They check complete label/value, checkbox,
bounded-number, picker and footer bands under native, named, ANSI-16 and monochrome themes in both
locales, including a disabled decrement and secondary shortcut text. Pointer feedback retains the
same draw receipt, hit geometry, selected setting and draft values; actual host presentation still
requires separate screenshot review.

The [theme picker cases](../crates/workbench-client/src/native_settings/tests/themes.rs) cover a
candidate's preview at its polarity, Esc restoration, the keys Save edits, an unshipped stored id, Reset
all and reduced-color unavailability. The [theme picker render cases](../crates/workbench-client/src/tui_render/settings/tests/themes.rs)
cover swatches, their absence without truecolor, scrolling and the System notice.

The [editor preview cases](../crates/workbench-client/src/native_settings/tests/appearance.rs) cover every
admitted key's exact one-key Save edit, frozen basis, child return, reset, invalid
color entry, conflict projection, unknown result and episode retirement. The
[whole-frame cases](../src/tui_shell/tests/settings.rs) drive
the real Shell drawing boundary with an existing native terminal, both locales,
80x24/120x40 and truecolor/256/16/monochrome capability. They compare background
chrome, default and explicit program cells, cancellation, external updates and
covering Client capture. These deterministic witnesses do not establish clipboard,
notification, sound or physical emulator delivery. The configuration watcher cases
also retain confirmed shared appearance and pointer-release state while updating
live bindings, their hints and both sidebar profiles together.

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
obligations of the editor, snapshot and Server configuration tests. GUI/TUI draft tests,
field-focused correction and guarded outside-click lifetimes, launch capture, terminal
presentation and input tests cover the Client boundaries. Actual bidirectional GUI/TUI edits,
reopen, reset, errors and visible effects remain acceptance requirements.

Run `node scripts/tla/run-models.mjs --models native-settings-save` and the owning
Cargo/TypeScript checks. The affected root gate remains required.

## History units and host-color display

History capacity displays and edits decimal MB (1 MB = 1,000,000 bytes), from 0 to 1000 MB. Up to six fractional digits preserve individual bytes; storage and small/large steps retain their existing byte domain. Disabled color fields show known effective RGB defaults; an unresolved host default shows its localized choice label instead of fallback palette values. Agent blank-space trimming uses plain-language copy. These are presentation and draft-conversion changes; Save and new-terminal effect boundaries stay with the existing owners.

## Color-disabled guidance

The Shell retains whether a nonempty `NO_COLOR` disabled colors at startup and carries that reason through committed and preview themes. Appearance, theme slot, Colors, terminal theme and editable color descriptions explain the suppression in both languages. Validation, confirmation and unknown-save recovery messages take precedence. The hint never infers `NO_COLOR` from monochrome alone, reads the environment while rendering, or changes the saved theme choice.
