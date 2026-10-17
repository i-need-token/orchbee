# Native settings

## Interaction and scope

`Ctrl+B F4` opens Settings on supported native TUI hosts. Windows also exposes
its sidebar Settings row and global action menu. The capturing surface edits a
local draft while pane output continues. It covers every common preference in
[the shared inventory](../../../crates/workbench-protocol/docs/user-preferences.md#inventory-and-effect),
including launch profiles and new-terminal scrollback defaults. Host-owned fonts,
pixel metrics and management workflows follow the inventory's explicit boundaries.
The native Settings feature maps the thirteen shared keys once to their row,
category, control kind, availability, integer unit, bounds, steps and effect
horizon. The editor, localization and renderer currently consume the row,
category, control, availability and bounded-integer facts. Effect metadata
documents and tests the feature contract; the existing domain owners still apply
those effects. Source comparison, atomic commit and reset behavior stay with their
existing state and protocol owners. Workbench Protocol remains the value, default
and validation owner.

Tab/Up/Down moves between controls; Left/Right or Space changes a choice. Text
fields use the [shared bounded line editor](native-line-editor.md), including Ctrl+W word deletion,
and literal paste. Rendering projects the complete raw value once and maps its cursor to a visible
grapheme boundary, so removing controls cannot leave the caret inside a painted cluster. Launch profiles
have their own page, one literal argument per editor, explicit default selection,
and confirmation before deleting a profile. Entering that child records the parent
row, viewport and focus; Back restores them while retaining accepted draft edits. Custom colors are disabled unless
Custom is selected; login shells are unavailable on Windows. The selected row
is revealed by keyboard navigation and remains a stable navigation anchor while the
pointer scrolls or the surface resizes. Category captions and setting
names match the GUI's English/Simplified Chinese vocabulary.

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

Settings centers horizontally and vertically in the full host viewport above the transient status
row, including the sidebar and top strip. Its preferred extent is 92 columns by 26 rows, clamped
to that viewport; sidebar visibility or width does not shift it. Existing minimum-content admission,
refusal and suspension preserve drafts and keep the status row uncovered. Placement does not resize
PTYs. Root-compositor cases cover both locales, 120x40, 120x30, 100x24 and 80x24, hidden/visible
sidebars and 24/30/60-column preferences; these are render/input-map checks rather than physical
host gesture evidence.

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
the surface token and actual painted row slice: an anchor that remains painted
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
backdrop through that same exact painted Close target. Unsaved changes, text entry,
the launch-profile child, confirmations, reconciliation and an in-flight read or
save retain the surface and its task on an outside click. This guard uses the same
active-task predicate that freezes the draft basis on external updates. Moving
into the surface, a stale token, focus loss or a changed layout cancels a backdrop
gesture. Explicit Close and Esc retain their existing local exit behavior;
deliberately closing hides an accepted save without cancelling it.

Save is the only write action. Opening, editing, cancelling and resizing do not
publish runtime policy. Language, application theme and terminal default colors
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
Enter or a click on the successfully drawn Save row consumes its
authority exactly once. Reset-all requires confirmation and remains a draft until
Save; resetting claims all common keys even if their values are already defaults.
The launch-profile page also offers a confirmed catalog-only reset, preserving
other draft settings. Clean open views follow confirmed external changes; active
edits, text entry, the launch-profile child and confirmations retain their original
saved basis while separately exposing the newest confirmed effective snapshot and a
conflict. A known conflict cannot submit its stale source. Reload confirms discarding
edits and performs a new read. A definite save failure can retry only from a fresh
rendered context. A stale-source refusal or unknown outcome disables Save until a
matched explicit Reload read succeeds; passive observations, redraws, cached snapshots
and old read completions do not clear that requirement. No request is replayed automatically.

Esc discards an unsubmitted draft; after submission it hides the surface while
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
lane has a two-second exchange deadline, serializes foreground requests with its
one-second observation poll, and interrupts/joins its owned connection on close.
A replacement Session/editor episode cannot inherit an old completion.

## Domain and ownership

| Record field | Contract                                                                                                                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | GUI and TUI users edit the same device preferences without lost updates or divergent defaults.                                                                                                                |
| Inputs       | Frozen configuration identity, complete confirmed snapshot, exact source token, typed dirty patch, painted-row receipt, explicit save/reset intent.                                                           |
| Authority    | Workbench Protocol owns values/bounds; Local Session Runtime owns the private record/lock/commit; the native Session and matched Desktop helper use that same store.                                          |
| State        | Newest effective runtime values, independently frozen draft basis, local changes, viewport/focus anchor and painted activation authority, one admitted exchange, and explicit refusal/unknown reconciliation. |
| Effects      | Confirmed snapshots govern runtime preferences; an independent reversible projection previews only authored copy, chrome and terminal default colors. Launch/default changes affect new terminal creation.    |
| Refusal      | Invalid or unsafe source, scope mismatch, stale source, unsupported launch choice, busy lane or unavailable persistence preserves existing data and confirmed behavior.                                       |

The Session freezes its native configuration identity at startup. Clients send no
paths, raw TOML, alternate device selector or arbitrary file write. Shared values
live in a bounded sibling private preference record; the old TOML remains an
explicit one-time import source. Desktop Main and renderer import only their
allowed unclaimed keys. Reset claims prevent later imports from restoring old
values. See [scope and persistence](../../../crates/workbench-protocol/docs/user-preferences.md#scope-persistence-and-migration).

| Transition           | Required behavior                                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Hidden → open        | Read a complete snapshot and freeze its source.                                                                                       |
| Draft → edit / draw  | Keep edits local, project reversible appearance to the whole frame, and revoke old drawing authority.                                 |
| Draft → save         | Require a current successful draw, freeze a nonempty patch and consume submission before dispatch.                                    |
| Submit → commit      | Lock the shared record, compare source, validate the complete candidate and atomically publish; preserve omitted keys.                |
| Commit → effective   | Publish only the confirmed values; do not reinterpret a disk receipt as Host application acknowledgment.                              |
| External update      | Publish newest effective values and refresh clean views; preserve active edits or child tasks and their frozen basis with a conflict. |
| Refused / unknown    | Preserve the visible draft and effective values; require a matched explicit read before another stale-source or unknown Save.         |
| Geometry unavailable | Refuse an unpainted incarnation; suspend and retain an admitted one until a successful frame resumes it.                              |
| Cancel / close       | Discard unsubmitted edits and reveal newest confirmed appearance; hiding after submission cannot roll back a commit.                  |

Application themes change chrome independently of terminal colors. Host terminal
colors use the frame/window's captured defaults; Light/Dark/Custom change only
default markers, preserving explicit program colors and color-query semantics.
Copy-on-select governs later gesture completion; explicit Copy remains available.
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

## Apply timing and capability boundaries

Every shared key persists through the same atomic Save and private record. The
[shared inventory](../../../crates/workbench-protocol/docs/user-preferences.md#inventory-and-effect)
owns values, validation and defaults. The native editor exposes these effect
horizons in localized row help:

| Key                                | Draft behavior                                                                | Effect after confirmed Save or external observation                                                                     |
| ---------------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `locale`                           | Whole-frame authored-copy preview; System uses the startup locale environment | Current application copy and later localized feedback; program bytes are unchanged                                      |
| `applicationTheme`                 | Whole-frame chrome preview                                                    | Current chrome; independent of terminal display theme                                                                   |
| `terminalDisplayTheme`             | Host/Light/Dark/Custom default-color preview                                  | Existing live and retained terminal presentation; explicit program colors remain exact                                  |
| `customForeground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked foregrounds in Custom; retained while other display themes are selected                         |
| `customBackground`                 | Accepted valid color previews in Custom only                                  | Existing default-marked backgrounds in Custom; retained while other display themes are selected                         |
| `copyOnSelect`                     | Draft only                                                                    | Later gesture completion, including a gesture already in progress; explicit Copy remains available                      |
| `trimTrailingBlankRowsForAgents`   | Draft only                                                                    | Existing detected Agents through the exact retained-window/acknowledgment owner; user selection/history retains control |
| `agentNotificationMode`            | Draft only                                                                    | Future eligible hidden-Agent transitions, without replay; host delivery and per-Agent mute still apply                  |
| `completionReminderMode`           | Draft only                                                                    | Future fresh command completions; requires shell integration and respects focus policy                                  |
| `completionReminderMinimumSeconds` | Draft only                                                                    | Later completion evaluation at the new duration threshold                                                               |
| `soundEnabled`                     | Draft only                                                                    | Eligible later reminders; host sound capability and per-Agent mute still apply                                          |
| `scrollbackBytes`                  | Draft only                                                                    | Newly created terminals capture the limit; existing/restored terminals keep theirs                                      |
| `launchProfiles`                   | Child edits remain in the parent draft                                        | Subsequent default launches capture the catalog/default; existing programs keep their launch facts                      |

Native host configuration is a separate admitted surface, not extra shared-editor
rows. Windows TOML changes require an explicit `server reload-config` (default
prefix F5), followed by the Client's 500 ms configuration observer. Shared preferences
are observed independently once per second, including while Settings is closed.
Neither publication is an acknowledgment from every connected Client.

| Admitted native configuration or Client state                                                                                     | Actual application policy                                                                                                                                 |
| --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `keys.prefix`, implemented action overrides, `keys.commands`                                                                      | Complete admitted reload updates dispatch and its help/hints together; clears pending prefix; does not execute configured commands merely by loading them |
| `ui.sidebar.agents.rows`, `rows_by_agent`, `row_gap`; `ui.sidebar.spaces.rows`, `row_gap` and their admitted token/rule fields    | Complete reload changes existing sidebar projections; layout and interaction remain with their sidebar owners                                             |
| `ui.window_title`                                                                                                                 | Live title subscription uses the admitted template after reload; enclosing-host title delivery remains a capability                                       |
| `ui.toast.delivery`, legacy `enabled`, `agent_delivery`, `delay_seconds`, `herdr.position`; `ui.sound.enabled`, `ui.sound.agents` | Reload updates manual delivery and later automatic feedback; confirmed shared Agent mode/global sound remain authoritative; per-Agent mute is preserved   |
| Legacy `theme.name`                                                                                                               | Seeds unclaimed shared appearance once; cannot overwrite confirmed common preferences on later TOML reloads                                               |
| `terminal.default_shell`, `terminal.shell_mode`                                                                                   | Reload changes later launch preparation only; confirmed shared launch profiles take precedence; Windows common profiles reject login shells               |
| `terminal.new_cwd`, `worktrees.directory`                                                                                         | Windows CLI reads them on the next relevant creation invocation; do not move an existing process                                                          |
| `session.resume_agents_on_restore`                                                                                                | Startup-only policy; reload reports `restartRequired` and does not rerun restore                                                                          |
| `update.manifest_check`                                                                                                           | Reload updates the existing rule-check scheduler; automatic checks retain their managed-release/capability restrictions                                   |
| `update.channel`                                                                                                                  | Consumed by subsequent release/update operations; does not recolor or restart the current TUI                                                             |
| Pointer capture (`--mouse off`, default prefix `m`), sidebar visibility/width                                                     | Immediate Client-local interaction state; these actions do not persist a shared preference                                                                |

`ui.mouse_capture`, `ui.mouse_scroll_lines`, Herdr sidebar-width fields and other
recognized unsupported keys remain reported as unsupported; loading them does not
change Client behavior. Fonts, pixel size, opacity and physical Option-key handling
belong to the enclosing host. Clipboard, terminal/OS notifications and sound need
host support; the TUI cannot promise delivery from a saved value. No new host mode,
restart, process launch or synthetic completion event is caused by appearance preview.

## Correctness and evidence

The [editor preview cases](../src/native_settings/tests/appearance.rs) cover every
admitted key's exact one-key Save patch, frozen basis, child return, reset, invalid
color entry, conflict projection, unknown result and episode retirement. The
[whole-frame cases](../src/terminal_session/ui/settings/tests/appearance.rs) drive
the real Shell drawing boundary with an existing native terminal, both locales,
80x24/120x40 and truecolor/256/16/monochrome capability. They compare background
chrome, default and explicit program cells, cancellation, external updates and
covering Client capture. These deterministic witnesses do not establish clipboard,
notification, sound or physical emulator delivery. The configuration watcher cases
also retain confirmed shared appearance and pointer-release state while updating
live bindings, their hints and both sidebar profiles together.

`SettingsCommitPublication.tla/.cfg` bounds two independently prepared OS-process
writers, separate Main/renderer imports, explicit edit/reset values, and one
noncooperating external edit. `Acquire` owns the shared lock; `Write` rechecks the
source and preserves other keys. Imports fill only unclaimed keys. Publication
carries an immutable committed snapshot; apply failure and lost replies do not
roll back or authorize another write. No-op receipts, filesystem primitives,
capability discovery and UI geometry remain implementation-test obligations.

An import's captured source, values and claims are needed only while it owns the
lock. Both release paths reset that scratch to canonical defaults; the next import
captures a fresh read set. This merges states that differ only in obsolete input,
while retaining every actor, input value, attempted/completed import, durable claim,
commit, publication and receipt, and every permitted external-edit ordering.

Checked properties cover single-writer ownership, unrelated-key preservation,
reset/claim preservation across migration, committed publication, complete values,
immutable publication identities and nonreplayed unknown outcomes. Negative
controls remove the final source comparison and the import claim guard, and must
produce overwrite/reset-resurrection counterexamples. Protocol conformance,
storage/helper tests, GUI/TUI draft tests, field-focused correction and guarded
outside-click lifetimes, launch capture, terminal presentation
and input tests cover concrete boundaries. Actual bidirectional GUI/TUI edits,
reopen, reset, errors and visible effects remain acceptance requirements.

Run `node scripts/tla/run-models.mjs --models native-settings-save` and the owning
Cargo/TypeScript checks. The affected root gate remains required.
