# Native line editor

## Scope and ownership

The Workbench Client's rename, workspace-setup and Settings drafts share one pure single-line editor core
at [`terminal_text/line_editor.rs`](../src/terminal_text/line_editor.rs). It
covers the editing vocabulary the keyboard standard assigns to draft fields: insert (typed input
and atomic paste share one path), Backspace, Delete, Left, Right, Home/Ctrl+A, End/Ctrl+E, Ctrl+W
and Ctrl+U. Selection, undo history, Ctrl+K, and Up/Down remapping are outside this core; consumers
leave unbound keys ignored rather than forwarding them.

The editor is pure state — no I/O, clock, Client mutation, terminal read, or ANSI parser.
Character admission stays with the typed-input boundary and each field's policy; the core never
filters, normalizes, or rewrites supplied text. Prefill keeps protocol-legal bytes exact, including
scalars the chrome projection cannot display, so a draft's original value and cancel path stay
faithful. Rendering remains a consumer projection: `terminal_text` owns the shared sanitized text
and cursor mapping, while rename/workspace fields and Settings own their respective display windows.
The editor core does not own display layout.

## State and editing contract

`LineEditor` holds `text` (exact UTF-8), `cursor` (a byte index that is always an
extended-grapheme boundary, possibly `text.len()`), and a `LengthLimit`. The limit is `Scalars`
for workbench labels (512 scalars) or `Bytes` for filesystem paths (4096 bytes). Settings uses
[field-specific byte limits](../src/native_settings/rows.rs), up to 32 KiB. Each limit is checked
against the exact resulting text — never against grapheme clusters. An over-limit insertion or
paste returns `LengthLimitReached` and leaves text and cursor unchanged; the owning draft turns
that rejection into its visible input fault.

Grapheme movement and deletion use `unicode-segmentation`, already locked at 1.13.3; no
hand-written UAX #29 or second width table enters the build. Every mutation re-segments the draft
— bounded by the length limits — instead of assuming `cursor += inserted.len()` survives cluster
merges. Insertion intends the end of the inserted span and deletion intends the span start; when an
edit merges that intent inside a new cluster (a ZWJ joining neighbors, two regional indicators
becoming one flag), the cursor snaps forward to the cluster's end. `Ctrl+W` deletes from the last
Unicode word boundary before the cursor, keeping earlier whitespace runs; punctuation like `foo.bar`
stays one word under UAX #29 and makes no readline-compat promise.

## Field window contract

Every draft field projects the complete raw text once through `terminal_text`'s ANSI-aware
projection — complete escape sequences are stripped as whole units, forbidden scalars are dropped —
and keeps the raw-to-visible byte map from that same parse. `TerminalTextProjection::cursor_boundary`
snaps the mapped cursor to the visible grapheme boundary at or before it: stripping controls can
merge neighboring clusters. No raw prefix or mid-sequence slice is parsed independently.

For rename and workspace setup,
[`field_window(editor, field_width)`](../src/workbench_client/state/field_window.rs) chooses a window
over the _visible_ text by whole grapheme clusters. The returned window is the
drawable text plus the cursor column strictly inside the field width; a zero-width field yields
`None` rather than a fake position, and a cluster wider than the remaining space is never split.

The rename and workspace renderers draw that window inside the modal's padded inner area and place
the terminal cursor at the field row plus the cursor column. Their typed `ModalPresentation`
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
and mixed Chinese/English, exact scalar and byte limit boundaries, atomic rejection, byte-exact
prefill, and cursor-at-end. Projection witnesses cover whole-sequence stripping with raw cursors
inside the removed span, narrow windows with cursor motion, ordinary Chinese, emoji, and combining
marks, and zero-width fields. Draft-level tests cover prefill-at-end editing, navigation, deletion,
word-backspace, atomic paste, picker inertness, and fault recovery; renderer and session journeys
witness the drawn window, the placed cursor, and exact raw submission.
The [Settings renderer witnesses](../src/tui_render/settings/tests.rs) check control/ANSI removal
that joins combining marks or emoji variation selectors, normal wide glyphs and cursor-at-end.
The [Settings input trace](../src/terminal_input/dispatcher/tests/settings.rs) sends literal paste
and Ctrl+W through the decoder and verifies word deletion without saving or pane forwarding.
