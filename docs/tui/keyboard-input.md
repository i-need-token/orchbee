# TUI Keyboard Input

## Authority and scope

This standard owns key decoding, the binding model, input ownership, focus and activation semantics,
editing and draft conventions, return behavior, and shortcut discovery for Orchbee-authored
terminal interfaces. Concrete bindings, mode names, captions, and the prefix chord belong to the
owning application's guide; this standard defines the shape they must satisfy. Host
keyboard-protocol negotiation follows
[`terminal-environment.md`](terminal-environment.md); pointer input follows
[`mouse-input.md`](mouse-input.md); control state and composition follow
[`components.md`](components.md).

The frontend contract in [`frontend/keyboard-shortcuts.md`](../frontend/keyboard-shortcuts.md)
governs browser and Desktop bindings. Choose the TUI binding and its GUI counterpart together under
[`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md): preserve the action's semantic scope, localized
name, mnemonic, commit boundary, and cancellation result while adapting its modifier, prefix, or
mode through the rules below.

This is the required contract for new and materially changed TUI keyboard behavior. It does not
assert that current Clients already conform; runtime adoption is established only by source and
evidence under [`verification.md`](verification.md#contract-review). Record a staged gap with its
owner and completion condition. Until adoption, application-owned references remain the descriptive
authority for current bindings and input behavior.

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
  event. `ESC` followed by a printable or UTF-8 character is that character with `Alt`; `ESC`
  followed by another C0 control byte is a lone `Esc` and then that control's own key, never an
  `Alt` chord, so `Esc` followed quickly by a prefix such as `Ctrl+B` delivers both.
- Bracketed paste is always enabled. A paste arrives as one atomic text event, is never matched
  against bindings, and is delivered to exactly one owner in its original order.
- Reject malformed, over-bound, or timed-out pastes as whole input frames. Discard their payload
  and drain silently through the terminator matching the opener; only subsequent bytes resume
  ordinary input.
- Multi-byte UTF-8 fragments and split escape sequences are reassembled before dispatch. A
  truncated sequence at a read boundary waits for the next read within a bounded window.

## Binding and intent model

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

- A binding resolves to a scoped semantic intent, not directly to a renderer mutation. Keyboard and
  pointer routes for one committed operation use the same intent identity, target, availability
  check, commit boundary, and outcome.
- The effective binding is the binding that can execute in the current owner and state after user
  overrides, capability gates, and shadowing. Dispatch, footer hints, and shortcut help derive from
  this effective model; a displayed default never claims that a shadowed key will work.
- Registered product actions use their action definitions. Component-local editors, pickers, and
  modes may own structured keymaps, and an explicit command surface may own structured command
  entries. They contribute to discovery through their owner; no rule assumes every command comes
  from one global registry.

## Input ownership

Every key or paste event has exactly one owner, resolved through these four layers:

1. An exclusive capture, such as the topmost modal task, line editor, or confirmation.
2. The current named input owner or mode, including an explicitly focused, visibly indicated
   Orchbee chrome surface or control.
3. The armed prefix layer, when the higher owner admits it.
4. Unmodified passthrough to the focused terminal pane.

Ownership rules:

- A focused Orchbee surface is a named owner only while its focus target was painted and its focus
  indicator is visible. If the target becomes non-drawable, hidden, or occluded, the surface moves
  focus to its documented painted fallback or vacates ownership before accepting another key. A
  disabled target follows its component's declared focus policy: it either remains focusable so
  the current owner can expose the localized reason, or is skipped and uses the same deterministic
  fallback. It never activates. A remembered target never authorizes input through stale geometry.
- A named owner may bind bare navigation, `Enter`, `Space`, `Tab`, or editing keys only for its
  visible interaction contract. Moving focus into chrome is explicit; hover alone never establishes
  keyboard ownership.
- Outside an exclusive capture or named owner, the Client binds no unmodified printable key, no
  bare `Enter`, `Tab`, `Esc`, or `Backspace`, and no `Ctrl+C`, `Ctrl+D`, or `Ctrl+Z`. The prefix
  chord is the only default unconditional interception after higher owners decline it. When no
  capture owns input, pressing the prefix twice forwards the literal chord; a capturing owner may
  consume it under its own contract. Explicit user configuration may add safe direct chords after
  higher ownership; the same printable, editing, and signal-key exclusions apply.
- An armed prefix has no timeout. `Esc` and host focus loss dismiss it. A paste delivered to the
  terminal also dismisses the interrupted prefix before subsequent typed input; paste owned by a
  capturing surface retains that surface's contract. Host capability replies never dismiss it.
  An unbound key after the prefix dismisses the
  prefix, forwards nothing, and shows a transient notice naming the effective help binding.
- A named mode owns one domain for its duration. Its name and return route stay visible; every key
  resolves to at most one intent; unbound keys are ignored, never forwarded. Modes do not nest:
  entering another mode replaces the current mode through a documented transition.
- An exclusive capture such as a modal task or confirmation consumes every key, including the
  prefix chord, until it submits, cancels, or yields through its documented local return step. Only
  an exclusive capture may accept unmodified printable text without first taking visible named
  ownership.
- A capturing overlay may admit the configured prefix for actions that close it or switch to
  another surface, but those actions still resolve through the normal binding owner. Ordinary keys
  and paste remain with the overlay, and this route never bypasses an exclusive capture above it.
- Bytes that no higher owner consumes reach the focused terminal unchanged and in order. The Client
  never reorders, merges, or drops passthrough input. In particular, `Esc` remains pane input when
  a terminal pane owns the keyboard; a named owner resolves `Esc` through its own local return
  contract.
- Ownership is a pure decision over the last presented interaction state and one event. Network
  progress and background work do not silently move ownership. A completed frame may establish a
  new owner for later events; it never reroutes an event already being dispatched.

## Focus, selection, and activation

- Keyboard focus, pointer hover, provisional selection, persistent current context, and activation
  are independent. Exactly one painted Orchbee region displays keyboard focus when chrome owns the
  keyboard; a focused terminal pane displays its own focus through the pane contract.
- Focus movement does not activate a target or change persistent current context. A collection
  owner declares whether arrows move focus, provisional selection, or both, and how the combined
  state is rendered.
- `Enter` and `Space` do not have one universal row behavior. Each focused control declares its
  primary intent: immediate action, navigation, provisional selection, explicit acceptance,
  disclosure, editing, or another reviewed component contract. The matching pointer route invokes
  that same intent rather than inferring behavior from row shape.
- `Tab`, arrows, page keys, and component mnemonics move only among targets in the last presented
  focus map. Hidden, non-drawable, occluded, and targets omitted for width or capability are
  skipped. A disabled target participates only when its component declares it focusable; while
  focused it retains disabled treatment, dispatches no activation, and exposes the same localized
  unavailable reason through the component or current help owner. Navigation order follows reading
  order unless the owner documents a spatial relationship.
- Opening a temporary surface records the prior semantic focus target, not only its cell. Closing
  it returns focus to that target if it remains valid; otherwise the owner uses its documented
  painted fallback. Focus never returns to an occluded or stale target.

## Editing and drafts

Every inline text editor honors the applicable conventions below. An editor may add bindings; it
does not silently rebind a convention it claims to support.

| Key                            | Single-line behavior                                            |
| ------------------------------ | --------------------------------------------------------------- |
| `Enter`                        | Submit when the draft is valid; otherwise explain in the footer |
| `Esc`                          | Invoke the editor's current local return step                   |
| `Backspace`                    | Delete one grapheme before the cursor                           |
| `Delete`                       | Delete one grapheme after the cursor                            |
| `← →`                          | Move one grapheme                                               |
| `Home` `End` `Ctrl+A` `Ctrl+E` | Move to the start or end                                        |
| `Ctrl+W`                       | Delete the word before the cursor                               |
| `Ctrl+U`                       | Clear the draft explicitly                                      |
| Paste                          | Insert atomically at the cursor                                 |

- An editor declares whether its value is a persistent composer draft, a staged edit, or an
  immediate control. Moving focus, opening help or a picker, parking an Orchbee-authored blocking
  task, and visiting a temporary overlay preserve a persistent composer draft. Only successful
  submission, explicit clear, or the owner's documented cancel action removes it.
- Cancelling a staged edit restores the value from the start of that editing boundary. Merely
  moving focus or dismissing a child popup does not commit or discard it.
- A multiline editor declares separate newline and submit bindings. Its footer shows the effective
  pair; `Enter` never changes meaning because the terminal became narrower.
- Setting commit policy follows
  [`settings.md`](settings.md#draft-preview-and-commit) and the feature's common contract; keyboard
  input never infers apply timing from a control's shape. Within an atomic editor, keyboard and
  pointer edits update the same draft, and only its explicit `Save` crosses the write boundary.
  Return is layer-specific: child return restores its recorded parent navigation and draft, retains
  accepted parent edits, and reverses only the child's unaccepted edit or preview to its entry
  projection. While the root draft is dirty, its original saved basis is retained for conflict
  comparison with the newest confirmed effective state. Closing the root before submission discards
  its draft and preview, then projects that newest state. An immediate control commits on activation
  only when its owner declares that policy and it is outside an enclosing atomic draft.
- Drafts have a documented maximum length in Unicode scalars. Input beyond it is rejected with a
  visible notice; it is never silently truncated. Paste uses the same limit atomically.
- An editor that edits an existing value prefills it and places the cursor at its documented
  selection or end position. Reopening a preserved composer restores its draft, cursor, and
  selection when those states remain valid.
- Composition through an input method must work: the editor never consumes a binding during
  composition and never redraws in a way that hides the composition candidate row.
- A destructive confirmation accepts only its exact confirm action and its local return action. It
  captures the whole input packet containing the decision so trailing bytes never reach a terminal;
  its default outcome is cancellation without the destructive effect.

## Local return and cancellation

- The active owner exposes one current local return step as structured data: semantic intent,
  effective key, localized label, state effect, draft effect, and focus destination. The handler,
  footer, help, and pointer close or back control consume that same step; they never promise
  different outcomes.
- `Esc` performs at most the current owner's one reversible step. Depending on that owner, the
  step may dismiss a child search or dropdown, leave an editor while preserving a composer or
  restoring a staged value, clear a provisional choice, close a temporary surface, park an
  Orchbee-authored blocking task while leaving it pending, or leave a named mode. These are local
  possibilities, not a global priority list shared by every surface.
- When a named Orchbee owner has no local return step, it follows its documented focus fallback.
  Default chrome `Esc` never cancels running work, terminates an Agent, or clears an unrelated
  draft. Leaving a product surface or the Client is a separately owned return or quit action and is
  never inferred from the absence of a local step. Cancellation of running work is a separately
  named semantic action with its own scope and confirmation policy.
- When a terminal pane owns input, `Esc` is forwarded to its program. Blocking prompts and cards
  presented by an Agent inside that pane retain the Agent's input semantics; Orchbee does not
  reinterpret them as Client-owned tasks.
- A pointer close, back, or backdrop gesture maps to the same local return intent only when it has
  the same state, draft, and focus effects. Otherwise it uses a separately labeled action. A
  geometry miss never counts as consent or cancellation.

## Discovery

- The active owner supplies structured effective hints from the same state and resolver used for
  dispatch. Action definitions, component-local keymaps, widget state, and explicit command entries
  may all contribute; duplicate handwritten labels are not a source of binding truth.
- The status line or footer shows the most useful actions for the current owner through the tiers in
  [`layout.md`](layout.md#size-tiers). The current return or focus-recovery route and the route to
  full help remain discoverable under compact truncation. A pending confirmation replaces ordinary
  hints with its exact next step.
- Full shortcut help identifies the active scope, presents effective keys first, and may retain
  inactive or disabled actions for learning only when their unavailable state and localized reason
  are explicit. Shadowed keys are not shown as usable in the active scope.
- Prefix help lists the effective bindings owned by that prefix. Mode help shows that mode's
  effective bindings and its current return step. Component-local help remains with the component
  instead of being forced into an unrelated global registry.
- Concrete product bindings, captions, and business actions stay in the owning application guide.
  Generate documentation from structured owners when practical, and verify explicit entries
  against the same intent and effective-binding tests.

## Verification

| Change            | Required check                                                                                                                                                                                                                                                                                              |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decoder change    | Tests cover every C0 normalization, uppercase implies `Shift`, `Ctrl+]`, lone `ESC`, split sequences, IME composition, and atomic or malformed paste                                                                                                                                                        |
| Binding or intent | Tests prove exact-modifier matching, Apple and non-Apple labels, override and shadow resolution, and keyboard/pointer agreement on target, availability, and commit                                                                                                                                         |
| Ownership change  | A table-driven test proves all four layers, visible focused-chrome admission and fallback, stale-target refusal, disabled focus policy, prefix behavior, pane `Esc`, and ordered passthrough                                                                                                                |
| Focusable control | Tests cover painted-target order, unconditional hidden-target relocation, both declared disabled-focus policies with reason discovery and no activation, focus versus selection, resize or occlusion fallback, and semantic focus return                                                                    |
| Editor or draft   | Tests cover editing keys, length and paste bounds, IME, focus and overlay preservation, submit, explicit clear, staged cancel, cursor restoration, and multiline hints                                                                                                                                      |
| Settings control  | Tests prove the declared policy: key and pointer share one draft; only explicit `Save` writes; child return retains accepted parent edits and restores its entry; root close discards local work and shows the newest confirmed state; immediate controls commit only when declared outside an atomic draft |
| Return behavior   | Tests derive the hint and handler from the same local step and cover its exact state, draft, work-cancellation, pointer, pane-passthrough, and focus-return effects                                                                                                                                         |
| Confirmation      | A test proves trailing bytes in the decision packet do not reach the terminal and cancellation performs no destructive effect                                                                                                                                                                               |
