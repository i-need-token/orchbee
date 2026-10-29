# TUI Overlays and Status

## Authority and scope

This standard owns surface selection, overlay composition, input capture, interaction lifecycle,
confirmations, the status line, hints, pointer tooltips, transient notices, and asynchronous outcome
presentation for Orchbee-authored terminal interfaces.
Geometry follows [`layout.md`](layout.md#overlays); colors and hierarchy follow
[`visual-foundations.md`](visual-foundations.md); key ownership follows
[`keyboard-input.md`](keyboard-input.md#input-ownership); reusable overlay shells and controls follow
[`components.md`](components.md).

These rules are required for new and materially changed TUI behavior; their presence does not claim
every current Client conforms. Runtime adoption requires source and the evidence in
[`verification.md`](verification.md#contract-review). Record any staged gap with its owner and
completion condition.

The interruption budget in
[`frontend/dialogs-and-forms.md`](../frontend/dialogs-and-forms.md#interruption-budget) applies
unchanged: start with no surface and no question, ask only what the outcome requires, and never
steal focus for background status.

## Surface selection

| Surface               | Use when                                                                           | Avoid when                                                            |
| --------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Status notice         | A result, state change, or refusal can be stated in one line and needs no response | The person must choose or type                                        |
| Status-line editor    | One short value is edited in place in the status row                               | The value is long, multi-field, or needs surrounding context to judge |
| Non-capturing overlay | A passive preview while typing continues to the pane                               | The content must be acted on before the pane can continue             |
| Capturing overlay     | A choice, picker, draft, or reference surface whose keys must not reach the pane   | One line of status or a status-line editor suffices                   |
| Confirmation          | A brief decision precedes an effect that is not readily reversible                 | The effect is reversible, or the surface merely reports information   |
| Pointer tooltip       | A resting pointer asks what a glyph, bracket control, or cut name in chrome means  | The words are already painted, or the information is needed to act    |

Any surface that consumes every key is capturing regardless of its size or name. A surface never
changes capture kind while open.

## Overlay composition

An overlay is one bordered region with the following reusable slots. Required slots are always
present; optional slots are omitted without reordering the remaining slots or recreating them
inside content.

Context menus use a compact composition instead: a rectangular border whose top edge names the
root menu's target, action rows and any owner-required reason row. Separators and `text-secondary`
group headings divide the rows; a heading is never selectable. Context menus omit the header close
control, footer hints and position counter. A submenu cascades beside its parent as an untitled
frame of the same composition. The parent stays painted but inert: its opening row keeps the
selection fill without the navigation marker, and the child holds the candidate. The visible menu
retains exclusive input ownership. Esc closes a root menu; in a child, Left, Esc or a click on the
parent frame returns to the parent. An outside click dismisses the whole cascade through the exact
admitted menu surface. General modal slots below continue to govern forms, pickers, confirmations
and global action discovery.

| Slot       | Requirement | Contract                                                                                                                  |
| ---------- | ----------- | ------------------------------------------------------------------------------------------------------------------------- |
| Header     | Required    | One localized task title; an optional one-line description names its target or consequence                                |
| Navigation | Optional    | Return path, breadcrumb, or tabs for movement within the same task                                                        |
| Query      | Optional    | Search or filter editor; query text is draft state, not a committed domain value                                          |
| Status     | Optional    | Validation, freshness, pending, failure, or recovery information for this task                                            |
| Content    | Required    | List, editor, prompt, reference text, or progress; the only slot that scrolls by default                                  |
| Actions    | Optional    | Selectable actions when footer bindings alone would be ambiguous                                                          |
| Footer     | Required    | Exit or return first, primary binding second, then at most two secondary bindings; decision buttons may carry these hints |

- One overlay is open at a time. A child task replaces the shell's task slots under the same capture
  owner; it never opens a second overlay above the first. A cascaded context submenu is the one
  exception: its parent stays painted beneath it as inert presentation, and only the child owns
  input.
- A capturing overlay may admit the configured global prefix for close or switch actions through
  its established action owner. Modal tasks and confirmations keep first refusal; other keys and
  paste remain with the overlay and never reach the covered pane.
- The header states what the overlay is about, not the action name alone. A confirmation header
  names the object being affected.
- A non-capturing overlay states in its footer that typing reaches the pane.
- An overlay whose content depends on remote state shows the content's age or a refresh binding; it
  never presents stale content as current without indication.
- Header, navigation, query, status, actions, and footer remain fixed while content scrolls. If the
  fixed slots leave less than the content minimum in [`layout.md`](layout.md#overlays), refuse a
  newly requested overlay; suspend an existing one instead of hiding its return or primary control.

### Frame chrome

- An overlay uses one neutral single-line `frame-unfocused` border, with no outer ring, shadow, or
  doubled frame. The overlay already owns input through exclusive occlusion; its heading and its
  terminal caret or provisional marker identify that ownership, so the border never repeats a focus
  accent.
- The header title sits on the top-left edge as `─ Title ` in heading text. The close control sits
  on the top-right edge as `─ [x] ─`, with one blank cell beside each bracket and one rule cell
  before the corner. Other title controls, such as page controls, precede the close control with one
  blank cell between controls. The title truncates before any title control loses a cell.
- One blank row separates the top edge from the first slot when height permits. It is the first space
  removed under height pressure, before any content, status, or footer row.
- Footer hints separate whole items with a spaced vertical rule, as in `Key label  │  Key label`.
  Keys use `hint-key` and labels use `hint-label`. Persistent chrome such as the status line keeps
  its middle-dot separator.
- A query slot is one row followed by a divider. At rest with an empty query, it shows the activation
  key and a localized prompt in secondary text, such as `/ to search`. A query retained at rest shows
  `Label: query` in secondary text. While editing, it shows the localized label, a colon, and the
  draft, such as `Search: draft`, with the terminal caret following the draft and no bracket
  boundary. A query that receives every typed character keeps its editing presentation. Form fields
  keep their labeled bracket boundaries.

### Content hierarchy

- Paint the task title once as the frame heading described in [Frame chrome](#frame-chrome).
  Supporting target context and description use secondary text; the value, exact decision target,
  and consequence needed to act use primary text. Do not make all prose bold or accent-colored.
- Align body text and field groups to declared content tracks. Put form field labels directly above
  their boundaries; a compact editor may keep its label inline. Put field help directly below.
  Separate the header, field groups, and footer with a
  blank row when space permits; reduce those gaps before removing useful content. A field's active
  boundary and caret remain visible, independently of row navigation or selection.
- Keep validation and recovery beside the affected task or field. A status role colors its glyph or
  status word, while explanatory prose keeps its reading hierarchy. Disabled controls retain their
  text-disabled role and reason; neither an error nor selection turns supporting metadata into a
  heading.
- Size the surface for readable task content and complete required controls. Optional help never
  widens a form. Wrap prose and shorten supporting context through the owning layout policy before
  sacrificing the target, field, exit, or primary action. Refuse or suspend an unusable task rather
  than paint a partial required control.
- Show each binding once in the footer. When an action has a button, put its formatted key inside
  that button's shared shortcut slot and omit the duplicate key-and-verb hint. The fixed button row
  follows any secondary editing or navigation hints; hints drop only as whole records. Button labels
  retain their primary, secondary, destructive, or disabled role while shortcut text stays subdued.

## Lifecycle and return

- Opening an overlay records the invoking region and focus identity. Entering a child task also
  records the parent's focus, selection, query or filter, viewport anchor, draft, and reversible
  preview. A visible return action exists whenever that parent state exists, except that a
  cascaded context submenu uses its Left/Esc return bindings and its painted parent frame without
  additional chrome.
- Returning from a child restores the recorded parent state. Closing the root restores the invoker
  when it still exists; otherwise focus moves to the nearest surviving item in the same region,
  then that region's documented default. Neither path reorders panes or changes domain selection as
  a side effect; a selection that already crossed its commit boundary remains committed.
- `Esc` takes the next local back step derived from the current surface state. That step may dismiss
  completed feedback, leave editing, clear a provisional selection, revert a preview, return from a
  child task, or close the root; the surface model defines which one is current rather than applying
  a universal cancellation order. Each step restores that layer's entry state. `Esc`, Back, and
  closing a surface never cancel running work; cancellation is a separate, explicitly named domain
  action.
- Dismissal discards only the current layer's uncommitted UI state. It does not roll back an
  immediate setting or another operation that already crossed its declared commit boundary. Hiding
  or closing after an accepted submission neither undoes the submission nor cancels the work it
  started.
- Replacing or closing a surface releases capture before focus restoration. The decoded input
  packet containing that decision remains owned and is consumed through its end, so trailing bytes
  never reach the new owner. Only a subsequent, separately decoded input event is resolved by the
  new owner; it is never delivered to the dismissed surface.
- A visible popup may admit dismissal from its outside backdrop. Its feature owner may withhold
  that route to retain an active draft, child task, required decision or pending operation while
  keeping an explicit close or return path. An admitted dismissal reuses its exact painted close
  action, respects the current capture owner, and consumes the complete gesture and packet tail.
  Body padding and wheel reports do not dismiss or fall through. An admitted dismissal has the
  same draft and accepted-work policy as the close action.
  A compact context menu without a close control instead uses its exact successfully painted
  surface, target and token proof for that same local dismissal; it invents no invisible X target.
- A surface that stops fitting after it opened is suspended, not implicitly dismissed. Preserve its
  parent state, draft, required decision, accepted submission, pending work, and recovery. The
  too-small state exposes only a currently valid return, exit, or recovery binding and never
  accepts a hidden submit or confirmation; restoring sufficient geometry resumes the same surface
  instance. A required decision follows the parking or close-refusal policy of its owning action.

## Preview and commitment

- Typing a query changes only the visible result set. Moving focus or selection may preview an item
  only when the preview is reversible; neither operation activates the item or writes domain state.
- Every editor, picker, and setting declares its commit boundary through its domain owner. Immediate
  settings commit at their control; an atomic staged draft exposes one outcome-named primary action.
  Do not infer a page-wide Save action from the presence of an overlay.
- Activation accepts the focused choice only after current validation succeeds and crosses the
  surface's declared boundary. The owner decides whether that acceptance updates a parent draft or
  commits domain state. Back or `Esc` restores only an active, unaccepted reversible preview. Once
  accepted, later return or `Esc` preserves the edit in its parent draft or domain and dismisses
  only the current uncommitted layer.
- Asynchronous validation and preview results bind to the exact target and draft revision that
  produced them. A result for an older draft cannot enable commitment or replace current feedback.

## Confirmations

- A confirmation is required before an effect that destroys retained state, ends a process, or
  affects other attached Clients. It names the consequence and its scope in one sentence.
- Unmodified `Enter` confirms; `Esc` cancels; the default outcome is cancel. Their subordinate key
  hints appear inside the corresponding button brackets rather than a duplicate footer row. No
  other key is interpreted, and the decision packet is captured as described in
  [`keyboard-input.md`](keyboard-input.md#editing-and-drafts).
- A confirmation is not used for reversible effects, routine edits, or information. Prefer direct
  execution with a notice.
- A confirmation that cannot be drawn when requested is refused with a status notice; the effect
  does not run. A confirmation already awaiting a decision follows the suspension contract instead.

## Status line

The status line is a floating one-row chip on the host's last row, end-aligned one cell in from
the right edge of the content beside any docked side panel. It is shown for an input owner,
meaningful notice, recovery, persistent state, or explicit mode such as released pointer capture.
Content reaches the last row: the status reserves no row, opening and closing it never changes
terminal geometry, and closing restores the covered cells. The chip occludes only its own painted
cells, including pointer input and the terminal cursor beneath them; the rest of the row remains
live content. While visible it stays above other overlays. Its segments, from start to end:

| Segment         | Contract                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------- |
| Input indicator | The armed prefix, active mode, capture kind, or released pointer capture; empty in the ordinary layer |
| Context         | The current location such as workspace and tab; optional detail dropped first when width is short     |
| Notice          | The most recent transient or persistent notice; replaces context detail while shown                   |
| Hints           | The current owner's key hints after the lead, dropped as whole records by priority                    |

- The input indicator is mandatory whenever any owner other than passthrough holds the keyboard.
  It is the last segment to be dropped.
- Ordinary passthrough floats nothing at rest. The one exception teaches the entry gesture: until
  the person first uses the prefix in the Client process, the chip names it and its purpose.
- In prefix mode, the current effective cancel binding comes first, followed by effective hints for
  help, side-panel focus when a side panel exists, the action menu when available, tab creation,
  splits, the Agent board, and detach. Width drops trailing hints as whole records.
- A persistent state that leaves the focused content unusable, such as a missing directory, a lost
  connection, or an ended program, shows one or two words with its severity marker, followed by
  the binding that fixes it when one exists. Its full explanation belongs to the affected surface.
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

## Pointer tooltips

A pointer tooltip states in words what one painted chrome target shows only as glyphs, a bracket
control, or a cut name. It supplements discovery: everything it says stays reachable without a
pointer through help, menus, or the binding table.

- Content is a legend, a full name, or an action. A legend names each glyph the target shows, in
  the glyph's own role, followed by its word; items stand two cells apart with no separator. A cut
  name appears whole before its legend. An action states its label, then the key that does the same
  to the same target now, taken from the effective binding table and never hard-coded. A target
  already explained in words gets none; a tooltip never names the current marker, the row kind, or
  help.
- The box is the context-menu frame without a heading, marker, or arrow: `surface-overlay` inside a
  `frame-unfocused` border with one padding cell each side, so it is its text width plus four cells
  by its line count plus two rows. Text is `text-primary`, keys use `hint-key`, and the box looks
  the same in every theme without an inverted fill.
- Text takes at most 40 cells by two lines and wraps at spaces; wide characters may break between
  characters. Whatever is left out ends the last line with the ellipsis, a path keeps both ends, an
  action key stays whole while its label truncates, and a cut legend ends with a `text-secondary`
  ellipsis.
- The box sits wholly below the target's last row, else wholly above its first row, else it does
  not open. It starts at the pointer's column and shifts left to stay on screen. It never covers its
  target, the pointer, or the floating status, and it stays put while the pointer moves within its
  target.
- It opens after the pointer rests 300 ms, counted from a real button-free movement report; content
  moving beneath a still pointer never opens one. While one is open, or within 400 ms after the
  pointer left one, the next target's opens at once. It never times out, and its content follows its
  target.
- Leaving the target closes it. So does any key, which still reaches its owner unchanged; `Esc` is
  not consumed. A press or wheel notch closes it and keeps that target quiet until the pointer leaves
  and returns. Focus loss, released capture, lost any-motion reporting, a resize, a menu, overlay,
  help, prefix, mode, drag, or rename starting, and the target moving, disappearing, or changing what
  it does also close it.
- The box is not a target. Its cells occlude what lies beneath like the status chip: moving onto it
  closes it, a press there reaches nothing, and hover resolves beneath after the next frame.
- Keyboard focus never opens one. Without any-motion reporting or native console motion it never
  opens, and nothing substitutes for it, such as a long press.
- Only the sidebar, the Tab strip, and pane titles offer tooltips, under their owners' documented
  content. Menus, overlays, Settings, links, and program output never do.

## Pending, completion, and attention

- An admitted foreground intent receives local acceptance feedback at its initiating control or
  intended destination on the next available frame. It does not wait for background preparation,
  completion, or the progress-indicator delay. The acceptance projection must not invent a canonical
  resource; admission alone does not establish commitment, readiness, or success.
- An operation that has not completed within a documented short delay additionally shows a pending
  indicator in the affected region or the status line. This delay suppresses unnecessary progress
  animation, not acceptance feedback. Local acceptance, progress, first useful content, and
  authoritative completion remain separate observations.
- A pending indicator is a fixed-width glyph sequence plus a localized verb. It never blocks
  drawing or input for other regions.
- A surface request carries the surface instance, target identity, and request generation. Only a
  completion that still matches all three may change selection, preview, pending state, or visible
  feedback. Replacing or closing the surface invalidates its presentation ownership without
  cancelling domain work. The effect owner may still settle authoritative domain state, but the
  completion cannot reopen the surface or mutate a replacement surface instance.
- An operation with a deadline shows the deadline's effect on completion: success, a definite
  failure, or an unknown outcome with the recovery binding. Unknown is presented as unknown, never
  as success or failure.
- Failure feedback stays beside the affected task or control, preserves retryable non-secret draft
  state, and provides a localized recovery action. Raw errors remain outside product copy.
- Attention is ordered as: a decision currently captured from the person, recovery for the current
  task, other current-task status, then background or unread state. Lower-priority feedback never
  displaces a required decision or persistent failure.
- Progress and completion that can continue in the background do not open an overlay, move focus or
  selection, replace the current draft, or switch context. They update their owning region or an
  unread/status indication until the person navigates to them.

## Empty states

- A content region with nothing to show paints one localized statement of why and the binding that
  creates or loads content. It never paints an unlabeled blank area.
- An empty state uses `text-secondary` and never a status role.

## Verification

| Change                  | Required check                                                                                                                                                                                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Overlay addition        | Rendered at every required geometry in [`verification.md`](verification.md#render-evidence); fixed slots and footer order remain visible; new-size refusal, open-size suspension and resumption, and capture kind are tested                                  |
| Child or dismissal flow | Tests restore focus, selection, query, viewport anchor, draft, and preview; each `Esc` unwinds one layer and leaves running work unchanged                                                                                                                    |
| Preview or commit flow  | Tests distinguish focus, unaccepted reversible preview, validation, activation, immediate commit, staged commit, and cancellation; cancellation reverts only the active unaccepted preview and preserves an accepted parent draft or domain value             |
| Confirmation addition   | Tests prove only the exact key confirms, `Esc` cancels, and the refused-geometry path does not run the effect                                                                                                                                                 |
| Status segment change   | Tests cover every tier and prove the input indicator survives to the smallest tier                                                                                                                                                                            |
| Notice change           | A test with injected time proves expiry schedules a redraw and a persistent error is not replaced                                                                                                                                                             |
| Pointer tooltip change  | Tests with injected time prove the rest from real motion only, adjacent opening, key dismissal with the key still delivered, press quieting, and closing when a published frame moves the target; placement tests cover below, above, left shift, and refusal |
| Asynchronous operation  | Tests prove local acceptance before background completion, delayed progress, active-generation completion, stale-result rejection, cancellation/ownership retirement, definite failure, unknown recovery, and that background completion does not focus       |
