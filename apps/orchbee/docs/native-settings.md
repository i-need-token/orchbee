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
closing the root. Header Close closes the whole surface. A plain left press and
matching release on the surrounding blank backdrop invokes that same exact
painted Close target; moving into the surface, a stale token, focus loss or a
changed layout cancels it. Closing hides an accepted save without cancelling it.

Save is the only write action. Opening, editing, cancelling and resizing do not
apply a draft. Enter or a click on the successfully drawn Save row consumes its
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
| Effects      | Only confirmed snapshots update locale, chrome, terminal presentation, copy policy and future feedback; launch/default changes affect new terminal creation.                                                  |
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
| Draft → edit / draw  | Keep edits local and revoke old drawing authority.                                                                                    |
| Draft → save         | Require a current successful draw, freeze a nonempty patch and consume submission before dispatch.                                    |
| Submit → commit      | Lock the shared record, compare source, validate the complete candidate and atomically publish; preserve omitted keys.                |
| Commit → effective   | Publish only the confirmed values; do not reinterpret a disk receipt as Host application acknowledgment.                              |
| External update      | Publish newest effective values and refresh clean views; preserve active edits or child tasks and their frozen basis with a conflict. |
| Refused / unknown    | Preserve the visible draft and effective values; require a matched explicit read before another stale-source or unknown Save.         |
| Geometry unavailable | Refuse an unpainted incarnation; suspend and retain an admitted one until a successful frame resumes it.                              |
| Cancel / close       | Discard only unsubmitted edits; hiding after submission cannot roll back a commit.                                                    |

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

## Correctness and evidence

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
storage/helper tests, GUI/TUI draft tests, launch capture, terminal presentation
and input tests cover concrete boundaries. Actual bidirectional GUI/TUI edits,
reopen, reset, errors and visible effects remain acceptance requirements.

Run `node scripts/tla/run-models.mjs --models native-settings-save` and the owning
Cargo/TypeScript checks. The affected root gate remains required.
