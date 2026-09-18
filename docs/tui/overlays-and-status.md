# TUI Overlays and Status

## Authority and scope

This standard owns surface selection, overlay composition, input capture, confirmations, the status
line, hints, transient notices, and pending indication for Orchbee-authored terminal interfaces.
Geometry follows [`layout.md`](layout.md#overlays); colors and hierarchy follow
[`visual-foundations.md`](visual-foundations.md); key ownership follows
[`keyboard-input.md`](keyboard-input.md#input-ownership).

The interruption budget in
[`frontend/dialogs-and-forms.md`](../frontend/dialogs-and-forms.md#interruption-budget) applies
unchanged: start with no surface and no question, ask only what the outcome requires, and never
steal focus for background status.

## Surface selection

| Surface               | Use when                                                                                 | Avoid when                                                            |
| --------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Status notice         | A result, state change, or refusal can be stated in one line and needs no response       | The person must choose or type                                        |
| Status-line editor    | One short value is edited in place in the status row                                     | The value is long, multi-field, or needs surrounding context to judge |
| Non-capturing overlay | Reference content such as help or a navigator preview while typing continues to the pane | The content must be acted on before the pane can continue             |
| Capturing overlay     | A choice, picker, or multi-value draft needs exclusive keyboard ownership                | One line of status or a status-line editor suffices                   |
| Confirmation          | A brief decision precedes an effect that is not readily reversible                       | The effect is reversible, or the surface merely reports information   |

Any surface that consumes every key is capturing regardless of its size or name. A surface never
changes capture kind while open.

## Overlay composition

An overlay is one bordered region with these slots in order:

| Slot    | Contract                                                                                  |
| ------- | ----------------------------------------------------------------------------------------- |
| Header  | One localized title in the frame's top border or first row; optional one-line description |
| Content | The task representation: list, editor, prompt, text, or progress; scrolls when too tall   |
| Footer  | The exit binding always, then the primary binding, then at most two secondary bindings    |
| Actions | Optional row of selectable actions when keys alone would be ambiguous                     |

- One overlay is open at a time. A subtask replaces the current content and footer under the same
  owner; it never opens a second overlay above the first.
- The header states what the overlay is about, not the action name alone. A confirmation header
  names the object being affected.
- A non-capturing overlay states in its footer that typing reaches the pane.
- An overlay whose content depends on remote state shows the content's age or a refresh binding; it
  never presents stale content as current without indication.
- Closing an overlay returns focus and layout to the state before it opened. Overlays do not
  reorder panes or change selection as a side effect of closing.

## Confirmations

- A confirmation is required before an effect that destroys retained state, ends a process, or
  affects other attached Clients. It names the consequence and its scope in one sentence.
- The confirm key is one exact key; `Esc` cancels; the default outcome is cancel. No other key is
  interpreted, and the decision packet is captured as described in
  [`keyboard-input.md`](keyboard-input.md#editing-conventions).
- A confirmation is not used for reversible effects, routine edits, or information. Prefer direct
  execution with a notice.
- A confirmation that cannot be drawn at the current geometry is refused with a status notice; the
  effect does not run.

## Status line

The status line is one row, always visible, and never covered. Its segments, from left to right:

| Segment         | Contract                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------- |
| Input indicator | The armed prefix, active mode, capture kind, or released pointer capture; empty in the ordinary layer |
| Context         | The current location such as workspace and tab; optional detail dropped first when width is short     |
| Notice          | The most recent transient or persistent notice; replaces context detail while shown                   |
| Hints           | The current owner's key hints, right-aligned, shortened or dropped through tiers                      |

- The input indicator is mandatory whenever any owner other than passthrough holds the keyboard.
  It is the last segment to be dropped.
- Segments collapse in the order documented in [`layout.md`](layout.md#size-tiers). A partial
  segment is never drawn; it is dropped or replaced by its shorter documented form.
- Status text is sanitized like every external string before painting.

## Notices

| Kind       | Lifetime                                                             | Presentation                         |
| ---------- | -------------------------------------------------------------------- | ------------------------------------ |
| Transient  | A documented duration, then removed by a scheduled redraw            | `text-secondary`                     |
| Persistent | Until the next key event that changes state or an explicit dismissal | `status-*` role matching its meaning |

- At most one notice is visible. A newer notice replaces an older one; a persistent error is not
  replaced by a transient notice.
- Expiry schedules a redraw; a notice never lingers because the interface was idle.
- A notice states the outcome and, when the person can act, the binding that acts. It never
  contains raw error text; failures map to localized messages with stable codes.
- Notices never block input and never move layout.

## Pending and progress

- An operation that has not completed within a documented short delay shows a pending indicator in
  the affected region or the status line. Before that delay nothing changes.
- A pending indicator is a fixed-width glyph sequence plus a localized verb. It never blocks
  drawing or input for other regions.
- An operation with a deadline shows the deadline's effect on completion: success, a definite
  failure, or an unknown outcome with the recovery binding. Unknown is presented as unknown, never
  as success or failure.
- Progress that can continue in the background does not open an overlay.

## Empty states

- A content region with nothing to show paints one localized statement of why and the binding that
  creates or loads content. It never paints an unlabeled blank area.
- An empty state uses `text-secondary` and never a status role.

## Verification

| Change                | Required check                                                                                                                                  |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Overlay addition      | Rendered at every required geometry in [`verification.md`](verification.md#render-evidence); footer shows the exit binding; capture kind tested |
| Confirmation addition | Tests prove only the exact key confirms, `Esc` cancels, and the refused-geometry path does not run the effect                                   |
| Status segment change | Tests cover every tier and prove the input indicator survives to the smallest tier                                                              |
| Notice change         | A test with injected time proves expiry schedules a redraw and a persistent error is not replaced                                               |
| Pending indicator     | A test proves nothing is drawn before the delay and the indicator has constant width across frames                                              |
