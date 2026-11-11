# Native line editor

## Scope and ownership

Every Workbench Client single-line field shares one pure editor core at
[`terminal_text/line_editor.rs`](../crates/workbench-client/src/terminal_text/line_editor.rs): Tab and
Panel rename, workspace setup, worktree launch, Settings text values and search, Action-menu and
Command Palette queries, the Machine picker's workspace and Agent queries, the Agent board's start
name, the shortcut-help filter, and copy-mode search. It covers the editing vocabulary the
[keyboard standard](../../../docs/tui/keyboard-input.md#editing-and-drafts) assigns to draft
fields: insert (typed input and atomic paste share one path), Backspace, Delete, Left, Right,
Home/Ctrl+A, End/Ctrl+E, and the shared readline keys `LineKey` names: `Ctrl+U` and `Ctrl+K` kill to
the start or end, `Ctrl+W` and `Alt+D` kill the word before or after the cursor, `Alt+B` and `Alt+F`
move by word, and `Ctrl+Y` yanks the most recent kill. The only selection is the whole prefilled
value of a name field, which opens selected: an insertion (typing, paste or yank) replaces it,
Backspace, Delete or a kill clears it (a kill keeps it for `Ctrl+Y`), and Left, Home or `Alt+B` end
it with the cursor at the start, Right, End or `Alt+F` at the end. Only Tab, Panel, Tab group,
Project and Workspace name drafts select their prefill; queries, paths and other fields never do.
Partial selection, undo history, and Up/Down remapping are outside this core; consumers leave
unbound keys ignored rather than forwarding them.

`LineKey::for_key` is the one map from a decoded host key to a shared key, on the exact normalized
modifier set; the modal decoder emits the same keys from legacy bytes (`0x15`, `0x0b`, `0x17`,
`0x19`, and `ESC b`/`f`/`d`). They are fixed editing conventions, not `[keys]` actions, and stay
out of the Action registry. A field that already gives one of these keys another meaning keeps it:

- The Machine picker's `Ctrl+Y` copies the selected row's ID in both queries, so yank is not
  available there.
- Action-menu and Command Palette queries keep Up/Down, Home/End, Left and Right for menu
  navigation, and the shortcut-help filter keeps Up/Down, Home/End and the page keys for paging;
  there only the shared readline keys and Backspace edit the query, and the help filter also takes
  Left, Right and Delete.
- Choice stages of workspace setup and worktree launch consume no text keys.

Keys reach the Panel only when no field owns input.

Browser and Desktop inputs in the GUI keep native platform editing. Browsers do not provide this
readline set (Windows has none of it; macOS text fields provide part of it natively), and
intercepting it would take Control chords from the page and Alt chords from AltGr layouts, so the
host difference is recorded rather than emulated; see the
[Web keyboard shortcuts](../../web/docs/keyboard-shortcuts.md#text-field-editing).

The editor is pure state — no I/O, clock, Client mutation, terminal read, or ANSI parser.
Character admission stays with the typed-input boundary and each field's policy; the core never
filters, normalizes, or rewrites supplied text. Prefill keeps protocol-legal bytes exact, including
scalars the chrome projection cannot display, so a draft's original value and cancel path stay
faithful. Rendering remains a consumer projection: `terminal_text` owns the shared sanitized text
and cursor mapping, while rename/workspace fields and Settings own their respective display windows.
The editor core does not own display layout.

## State and editing contract

`LineEditor` holds `text` (exact UTF-8), `cursor` (a byte index that is always an
extended-grapheme boundary, possibly `text.len()`), a `LengthLimit`, and a one-entry in-memory kill
buffer. Consecutive kills extend the buffer (backward kills prepend, forward kills append); any
other edit or movement ends the chain, and the buffer never touches the system clipboard. It is
per editor, so a draft rebuilt for a new stage starts empty. A yank inserts through the atomic
insert path and is rejected by the length limit like a paste. The limit is `Scalars`
for workbench labels (512 scalars) or `Bytes` for filesystem paths (4096 bytes). Settings uses
[field-specific byte limits](../crates/workbench-client/src/native_settings/rows.rs), up to 32 KiB. Each limit is checked
against the exact resulting text — never against grapheme clusters. An over-limit insertion or
paste returns `LengthLimitReached` and leaves text and cursor unchanged; the owning draft turns
that rejection into its visible input fault.

Grapheme movement and deletion use `unicode-segmentation`, already locked at 1.13.3; no
hand-written UAX #29 or second width table enters the build. Every mutation re-segments the draft
— bounded by the length limits — instead of assuming `cursor += inserted.len()` survives cluster
merges. Insertion intends the end of the inserted span and deletion intends the span start; when an
edit merges that intent inside a new cluster (a ZWJ joining neighbors, two regional indicators
becoming one flag), the cursor snaps forward to the cluster's end. Word keys use UAX #29 words
(`unicode_word_indices`): `Ctrl+W` and `Alt+B` go to the start of the last word before the cursor,
`Alt+D` and `Alt+F` to the end of the first word after it, skipping whitespace, punctuation and
emoji runs that contain no word; each Han ideograph is its own word. The result snaps to a grapheme
boundary. Punctuation like `foo.bar` stays one word under UAX #29 and makes no readline-compat
promise beyond the key set.

## Field window contract

Every draft field projects the complete raw text once through `terminal_text`'s ANSI-aware
projection — complete escape sequences are stripped as whole units, forbidden scalars are dropped —
and keeps the raw-to-visible byte map from that same parse. `TerminalTextProjection::cursor_boundary`
snaps the mapped cursor to the visible grapheme boundary at or before it: stripping controls can
merge neighboring clusters. No raw prefix or mid-sequence slice is parsed independently.

For rename and workspace setup,
[`field_window(editor, field_width)`](../crates/workbench-client/src/workbench_state/field_window.rs) chooses a window
over the _visible_ text by whole grapheme clusters. The returned window is the
drawable text plus the cursor column strictly inside the field width; a zero-width field yields
`None` rather than a fake position, and a cluster wider than the remaining space is never split.

The rename and workspace renderers draw that window inside the modal's padded inner area, a
selected value on the `surface-selection` fill over its own cells, and place the terminal cursor at
the field row plus the cursor column. Their typed `ModalPresentation`
receipt attests that the complete surface, field window and exact cursor projection were painted.
An evaluated geometry block refuses a newly opened draft that has never painted. Once a draft has
painted, the same block suspends that incarnation while preserving its token, raw text, grapheme
cursor and target; ordinary edits and submission remain inert until a complete paint of the same
token resumes it, while local `Esc` may still cancel. A missing or covered receipt revokes only the
current draw proof and neither classifies geometry nor discards editor state. Settings retains its
Paragraph scroll policy and owns its own presentation lifecycle, but takes its text and cursor
boundary from the same whole-text projection; painting never rewrites the raw draft.

## Correctness evidence

Deterministic unit tests cover five-scalar family emoji, ZWJ and regional-indicator merges,
combining-mark insertion and deletion, Unicode word deletion over whitespace, punctuation, emoji,
and mixed Chinese/English, kill chaining and yank, word movement over combining, CJK and emoji
text, atomic yank rejection, the exact-modifier key map, exact scalar and byte limit boundaries,
atomic rejection, byte-exact prefill, cursor-at-end, and how each key family replaces, clears or
ends a selected prefill. The modal decoder table covers the
legacy byte forms of the shared keys. Projection witnesses cover whole-sequence stripping with raw cursors
inside the removed span, narrow windows with cursor motion, ordinary Chinese, emoji, and combining
marks, and zero-width fields. Draft-level tests cover editing after a prefill, navigation, deletion,
word-backspace, atomic paste, picker inertness, and fault recovery; renderer and session journeys
witness the drawn window, the placed cursor, and exact raw submission.
The [Settings renderer witnesses](../crates/workbench-client/src/tui_render/settings/tests.rs) check that the inline field's cursor lands on whole graphemes after control/ANSI removal
joins combining marks or emoji variation selectors, normal wide glyphs and cursor-at-end.
The [Settings input trace](../crates/workbench-client/src/terminal_input/dispatcher/tests/settings.rs) sends literal paste
and Ctrl+W through the decoder and verifies word deletion without applying or pane forwarding.
