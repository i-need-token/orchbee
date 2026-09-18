# Native line editor

## Scope and ownership

The terminal session's rename and workspace-setup drafts share one pure single-line editor core
at [`terminal_session/state/line_editor.rs`](../src/terminal_session/state/line_editor.rs). It
covers the editing vocabulary the keyboard standard assigns to draft fields: insert (typed input
and atomic paste share one path), Backspace, Delete, Left, Right, Home/Ctrl+A, End/Ctrl+E, Ctrl+W
and Ctrl+U. Selection, undo history, Ctrl+K, and Up/Down remapping are outside this core; consumers
leave unbound keys ignored rather than forwarding them.

The editor is pure state — no I/O, clock, session mutation, terminal read, or ANSI parser.
Character admission stays with the typed-input boundary and each field's policy; the core never
filters, normalizes, or rewrites supplied text. Prefill keeps protocol-legal bytes exact, including
scalars the chrome projection cannot display, so a draft's original value and cancel path stay
faithful. Rendering is a pure consumer projection owned by
[`terminal_session/state/field_window.rs`](../src/terminal_session/state/field_window.rs); the core
does not own display layout.

## State and editing contract

`LineEditor` holds `text` (exact UTF-8), `cursor` (a byte index that is always an
extended-grapheme boundary, possibly `text.len()`), and a `LengthLimit`. The limit is `Scalars`
for workbench labels (512 scalars) or `Bytes` for filesystem paths (4096 bytes) and is checked
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

A draft field never sanitizes or window-slices the raw text directly. `field_window(editor,
field_width)` projects the complete raw text once through `terminal_text`'s ANSI-aware
projection — complete escape sequences are stripped as whole units, forbidden scalars are dropped —
and keeps the raw-to-visible byte-boundary map from that same parse. The window is then chosen over
the *visible* text by whole grapheme clusters so a raw cursor inside a stripped sequence lands on a
display boundary and no mid-sequence raw slice can re-enter the parser. The returned window is the
drawable text plus the cursor column strictly inside the field width; a zero-width field yields
`None` rather than a fake position, and a cluster wider than the remaining space is never split.

Both renderers draw that window inside the modal's padded inner area and place the terminal cursor
at the field row plus the cursor column; the typed `ModalPresentation` receipt attests the complete
text fit, and a field that cannot be presented cancels the draft instead of swallowing keys.

## Correctness evidence

Deterministic unit tests cover five-scalar family emoji, ZWJ and regional-indicator merges,
combining-mark insertion and deletion, Unicode word deletion over whitespace, punctuation, emoji,
and mixed Chinese/English, exact scalar and byte limit boundaries, atomic rejection, byte-exact
prefill, and cursor-at-end. Projection witnesses cover whole-sequence stripping with raw cursors
inside the removed span, narrow windows with cursor motion, ordinary Chinese, emoji, and combining
marks, and zero-width fields. Draft-level tests cover prefill-at-end editing, navigation, deletion,
word-backspace, atomic paste, picker inertness, and fault recovery; renderer and session journeys
witness the drawn window, the placed cursor, and exact raw submission.
