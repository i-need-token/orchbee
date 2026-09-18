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
faithful. Rendering remains a pure projection; the chrome-safe display of stored text is a separate
consumer concern.

## State and editing contract

`LineEditor` holds `text` (exact UTF-8), `cursor` (a byte index that is always an
extended-grapheme boundary, possibly `text.len()`), `scroll` (the byte index where the visible
window starts, always a grapheme boundary at or before the cursor), and a `LengthLimit`. The limit
is `Scalars` for workbench labels (512 scalars) or `Bytes` for filesystem paths (4096 bytes) and
is checked against the exact resulting text — never against grapheme clusters. An over-limit
insertion or paste returns `LengthLimitReached` and leaves text, cursor, and scroll unchanged.

Grapheme movement and deletion use `unicode-segmentation`, already locked at 1.13.3; no
hand-written UAX #29 or second width table enters the build. Every mutation re-segments the draft
— bounded by the length limits — instead of assuming `cursor += inserted.len()` survives cluster
merges. Insertion intends the end of the inserted span and deletion intends the span start; when an
edit merges that intent inside a new cluster (a ZWJ joining neighbors, two regional indicators
becoming one flag), the cursor snaps forward to the cluster's end. `Ctrl+W` deletes from the last
Unicode word boundary before the cursor, keeping earlier whitespace runs; punctuation like `foo.bar`
stays one word under UAX #29 and makes no readline-compat promise.

## Viewport contract

Display width is measured with the project's existing `Line::width`, so combining marks cost zero
cells and wide clusters cost two. `pin_scroll(field_width)` advances `scroll` by whole grapheme
boundaries until the cursor column — the width of `text[scroll..cursor]`, not the whole prefix —
is narrower than the field. `viewport(field_width)` returns `None` for a zero-width field rather
than a fake cursor position, and otherwise yields the drawable window plus cursor column; a cluster
wider than the remaining space is never split. The projection attests that the window and cursor
are drawable — not that the whole draft is on screen, and draw tokens built on it carry the same
honest scope.

## Correctness evidence

Deterministic unit tests cover five-scalar family emoji, ZWJ and regional-indicator merges,
combining-mark insertion and deletion, Unicode word deletion over whitespace, punctuation, emoji,
and mixed Chinese/English, exact scalar and byte limit boundaries, atomic rejection, byte-exact
prefill, cursor-at-end, zero-width fields, whole-cluster scrolling, unsplittable clusters, and
repeated mixed operations preserving invariants. These witnesses establish the pure model only —
wiring the three drafts, modal input decoding, and field rendering is a separate consumer
integration, and a usable editor surface is not claimed from this core alone.
