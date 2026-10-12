# Native settings

## Interaction and scope

`Ctrl+B F4` opens Settings on supported native TUI hosts. Windows also exposes
its sidebar Settings row and global action menu. The capturing surface edits a
local draft while pane output continues. It covers every common preference in
[the shared inventory](../../../crates/workbench-protocol/docs/user-preferences.md#inventory-and-effect),
including launch profiles and new-terminal scrollback defaults. Host-owned fonts,
pixel metrics and management workflows follow the inventory's explicit boundaries.

Tab/Up/Down moves between controls; Left/Right or Space changes a choice. Text
fields use the shared bounded line editor and literal paste. Launch profiles
have their own page, one literal argument per editor, explicit default selection,
and confirmation before deleting a profile. Custom colors are disabled unless
Custom is selected; login shells are unavailable on Windows. The selected row
stays visible when the surface scrolls or resizes. Category captions and setting
names match the GUI's English/Simplified Chinese vocabulary.

Rows, profile/argument arrows, Back/Close, text confirmation and confirmation-dialog buttons are painted mouse controls. Plain left release must match the press's exact current token and rectangle; keyboard edits, focus loss or layout changes revoke that press. Wheel is admitted only inside the active list: three rows per notch, bounded per pass by its visible height, without wrapping or changing values. Child editors and confirmations retain input and expose their own Confirm/Cancel. Header X follows Esc, including hiding an accepted save without cancelling it.

Save is the only write action. Opening, editing, cancelling and resizing do not
apply a draft. Enter or a click on the successfully drawn Save row consumes its
authority exactly once. Reset-all requires confirmation and remains a draft until
Save; resetting claims all common keys even if their values are already defaults.
The launch-profile page also offers a confirmed catalog-only reset, preserving
other draft settings. Clean open views follow confirmed external changes; active
edits, text entry and confirmations retain their original source and show a conflict.
Reload confirms discarding edits and reads current values. A save failure or unknown outcome requires a fresh rendered
submission context; no request is replayed automatically.

Esc discards an unsubmitted draft; after submission it hides the surface while
the accepted save can continue. Captured packet/paste tails remain owned through
cancellation and the next redraw. The pane cursor stays hidden under Settings.
The surface requires at least 36 columns by 10 rows; smaller areas close it with
an actionable notice. Pending feedback begins after 200 ms. The ordered settings
lane has a two-second exchange deadline, serializes foreground requests with its
one-second observation poll, and interrupts/joins its owned connection on close.
A replacement Site/editor episode cannot inherit an old completion.

## Domain and ownership

| Record field | Contract                                                                                                                                                                |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | GUI and TUI users edit the same device preferences without lost updates or divergent defaults.                                                                          |
| Inputs       | Frozen configuration identity, complete confirmed snapshot, exact source token, typed dirty patch, drawn editor token, explicit save/reset intent.                      |
| Authority    | Workbench Protocol owns values/bounds; Local Site Runtime owns the private record/lock/commit; the native Site and matched Desktop helper use that same store.          |
| State        | Last confirmed runtime values, independently frozen draft basis, local changes, one admitted exchange, and explicit refusal/unknown status.                             |
| Effects      | Only confirmed snapshots update locale, chrome, terminal presentation, copy policy and future feedback; launch/default changes affect new terminal creation.            |
| Refusal      | Invalid or unsafe source, scope mismatch, stale source, unsupported launch choice, busy lane or unavailable persistence preserves existing data and confirmed behavior. |

The Site freezes its native configuration identity at startup. Clients send no
paths, raw TOML, alternate device selector or arbitrary file write. Shared values
live in a bounded sibling private preference record; the old TOML remains an
explicit one-time import source. Desktop Main and renderer import only their
allowed unclaimed keys. Reset claims prevent later imports from restoring old
values. See [scope and persistence](../../../crates/workbench-protocol/docs/user-preferences.md#scope-persistence-and-migration).

| Transition          | Required behavior                                                                                                        |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Hidden → open       | Read a complete snapshot and freeze its source.                                                                          |
| Draft → edit / draw | Keep edits local and revoke old drawing authority.                                                                       |
| Draft → save        | Require a current successful draw, freeze a nonempty patch and consume submission before dispatch.                       |
| Submit → commit     | Lock the shared record, compare source, validate the complete candidate and atomically publish; preserve omitted keys.   |
| Commit → effective  | Publish only the confirmed values; do not reinterpret a disk receipt as Host application acknowledgment.                 |
| External update     | Publish runtime values and refresh clean views; preserve active drafts and their original source with a conflict notice. |
| Refused / unknown   | Preserve the draft and last confirmed values; show the distinct outcome and offer a fresh read.                          |
| Cancel / close      | Discard only unsubmitted edits; hiding after submission cannot roll back a commit.                                       |

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
