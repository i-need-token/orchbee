# TUI Keyboard Input

## Authority and scope

This standard owns key decoding, the binding model, input ownership layers, editing conventions,
and shortcut discovery for Orchbee-authored terminal interfaces. Concrete bindings, mode names, and
the prefix chord belong to the owning application's guide; this standard defines the shape they must
satisfy. Host keyboard-protocol negotiation follows
[`terminal-environment.md`](terminal-environment.md); pointer input follows
[`mouse-input.md`](mouse-input.md).

The frontend contract in [`frontend/keyboard-shortcuts.md`](../frontend/keyboard-shortcuts.md)
governs browser and Desktop bindings. Where both apply to the same semantic action, the action name
and its localized label are shared; the chord may differ.

## Decoding

- Decode the raw host byte stream into typed key events before any owner sees it. Owners match
  typed events, never bytes.
- Request keyboard-protocol disambiguation with the flags that keep input-method composition
  working: disambiguate escape codes, report event types, and report alternate keys. Do not request
  all keys as escape codes by default; enable it only with evidence that composition still works on
  every supported host.
- Act on press and repeat events only. Release events are consumed and never dispatched.
- Normalize before matching: a raw C0 control character with no modifier is the matching `Ctrl`
  chord; an uppercase letter implies `Shift`; legacy `Ctrl+5` is `Ctrl+]`. Raw `ESC` (`0x1b`) and
  `DEL` (`0x7f`) are never normalized into `Ctrl` chords.
- Resolve a lone `ESC` against an `Alt` prefix with a bounded wait only when the host does not
  disambiguate. The wait never exceeds one input poll interval and never delays a disambiguated
  event.
- Bracketed paste is always enabled. A paste arrives as one atomic text event, is never matched
  against bindings, and is delivered to exactly one owner in its original order.
- Multi-byte UTF-8 fragments and split escape sequences are reassembled before dispatch. A
  truncated sequence at a read boundary waits for the next read within a bounded window.

## Binding model

One canonical binding is a key code plus a complete modifier set. Matching and display both derive
from that value; no surface stores a separate label string.

| Concern    | Rule                                                                                                              |
| ---------- | ----------------------------------------------------------------------------------------------------------------- |
| Match      | The normalized event equals the normalized binding; a superset of modifiers does not match                        |
| Alternates | One action may list several bindings; the first is the displayed hint, order carries no dispatch priority         |
| Label      | Modifiers in the order `Ctrl`, `Alt`, `Shift`, `Super`, joined to the key with `+`                                |
| Key names  | `Enter`, `Esc`, `Tab`, `Space`, `Backspace`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`, `F1`…`F12`, `↑ ↓ ← →` |
| Platform   | On Apple hosts display `Alt` as `⌥`; the client operating system decides, not the server                          |
| Chord      | A prefix followed by a key is displayed as two labels separated by one space                                      |
| Fallback   | Arrow glyphs use `Up`, `Down`, `Left`, `Right` under simplified glyphs                                            |

Letters display in lowercase unless `Shift` is part of the binding. A digit range displays as
`1..9`, never as nine entries.

## Input ownership

Every key event has exactly one owner, resolved in this order:

1. An exclusive capture such as a line editor or a confirmation.
2. The active named mode.
3. The armed prefix layer.
4. The unmodified passthrough to the focused terminal pane.

Ownership rules:

- Bytes that no owner consumes reach the focused terminal unchanged and in order. The Client never
  reorders, merges, or drops passthrough input.
- Outside an exclusive capture or a mode, the Client binds no unmodified printable key, no bare
  `Enter`, `Tab`, `Esc`, or `Backspace`, and no `Ctrl+C`, `Ctrl+D`, or `Ctrl+Z`. The prefix chord
  is the only default unconditional interception, and pressing it twice forwards the literal chord.
  Explicit user configuration may add safe direct chords after capture and mode ownership;
  the same printable/editing/signal-key exclusions apply and discovery must show the actual bindings.
- An armed prefix has no timeout. `Esc` dismisses it. An unbound key after the prefix dismisses the
  prefix, forwards nothing, and shows a transient notice naming the help binding.
- A named mode rebinds the keyboard for one domain. Entering a mode is visible in the status line
  for its whole duration; `Esc` always returns to the ordinary layer; every key resolves to at most
  one intent; keys the mode does not bind are ignored, never forwarded.
- Modes do not nest. Entering a mode from another mode replaces it.
- An exclusive capture consumes every key, including the prefix chord, until it submits or cancels.
  Only an exclusive capture may bind unmodified printable keys.
- Ownership is a pure decision over the current input state and one event. Time, host, and network
  state do not change who owns a key.

## Editing conventions

Every inline text editor honors this set. An editor may add bindings; it never rebinds these.

| Key                            | Behavior                                                        |
| ------------------------------ | --------------------------------------------------------------- |
| `Enter`                        | Submit when the draft is valid; otherwise explain in the footer |
| `Esc`                          | Cancel and restore the previous value                           |
| `Backspace`                    | Delete one grapheme before the cursor                           |
| `Delete`                       | Delete one grapheme after the cursor                            |
| `← →`                          | Move one grapheme                                               |
| `Home` `End` `Ctrl+A` `Ctrl+E` | Move to the start or end                                        |
| `Ctrl+W`                       | Delete the word before the cursor                               |
| `Ctrl+U`                       | Clear the draft                                                 |
| Paste                          | Insert atomically at the cursor                                 |

- Drafts have a documented maximum length in Unicode scalars. Input beyond it is rejected with a
  visible notice; it is never silently truncated.
- The editor prefills the current value and places the cursor at its end.
- Composition through an input method must work: the editor never consumes an event during
  composition and never redraws in a way that hides the composition candidate row.
- A destructive confirmation accepts only its exact confirm key and `Esc`. It captures the whole
  input packet that contains the decision so trailing bytes never reach the terminal, and its
  default outcome is cancel.

## Discovery

- The prefix layer defaults to `?` for shortcut help; user overrides must be reflected in hints. Help lists every binding of the current owner from
  the binding registry; it is never a hand-maintained string list.
- Inside a mode, help shows that mode's bindings first and the exit binding last.
- The status line shows the current owner's most useful hints through the tiers in
  [`layout.md`](layout.md#size-tiers); hints use the binding label formatter above.
- A disabled action stays in help with its localized reason.
- Concrete bindings are documented in the owning application guide, generated from the same
  registry when practical, and never repeated in this standard.

## Verification

| Change                | Required check                                                                                                  |
| --------------------- | --------------------------------------------------------------------------------------------------------------- |
| Decoder change        | Tests cover every C0 normalization, uppercase implies `Shift`, `Ctrl+]`, lone `ESC`, split sequences, and paste |
| Binding addition      | Tests prove match, non-match with extra modifiers, and the displayed label on Apple and non-Apple hosts         |
| Ownership change      | A table-driven test proves precedence for every owner pair and that unconsumed bytes pass through in order      |
| Mode addition         | Tests prove the indicator is visible, `Esc` exits, unbound keys are ignored, and modes do not nest              |
| Editor addition       | Tests cover every editing key, the length bound, prefill, and atomic paste                                      |
| Confirmation addition | A test proves trailing bytes in the decision packet do not reach the terminal                                   |
