# TUI Input System and Interaction Vocabulary Extension

| Field              | Value                                                                                                                                                                                                                                                                                                             |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Draft proposed for product-owner acceptance. Nothing here is accepted yet; every new binding, mode, gesture rule, and wire request is marked provisional and takes effect only through the same acceptance path as the parent contract                                                                            |
| Owner              | Product command experience and the maintainers of the Rust `orchbee` Client TUI input path, the Server semantic-command boundary, and the terminal protocol                                                                                                                                                       |
| Parent design      | [`native-command-surface.md`](native-command-surface.md) owns the accepted TUI Client contract and the `Ctrl+B` initial map; [`native-runtime-topology.md`](../platform/native-runtime-topology.md) owns the shared Workbench document and the Client View boundary                                               |
| Decision authority | The product owner decides the second-level input structure, the provisional key tables, each wire admission request, and the mouse-capture policy. Astra's research verdict already excludes floating panes, plugin systems, configurable keymap files, tmux session semantics, and client-side tiling from scope |
| Exit condition     | An accepted slice of this vocabulary lands in the owning app documentation, input implementation, and tests; this proposal is then retired or reduced to the still-unaccepted remainder                                                                                                                           |

Simplified Chinese review translation:
[tui-input-vocabulary.zh-CN.md](tui-input-vocabulary.zh-CN.md).
English remains canonical.

Windows keyboard resizing now follows the accepted [native keyboard contract](../../apps/orchbee/docs/native-keyboard-resize.md). The broader PANEL vocabulary below remains proposed; its design inventory does not replace current app authority.

## Purpose

The current TUI input system is a single `Ctrl+B` prefix layer over a raw byte stream. It works
today, but the admitted Workbench surface keeps growing: `panel.split`, `panel.close`, and
`panel.move` are already accepted Server mutations with no shortcut, and scrollback, navigator
deepening, and session/attachment management are next. This proposal inventories the existing
input surfaces, argues that a single flat prefix layer cannot carry the next generation of
actions, compares four second-level structures, and drafts an extended vocabulary plus a
mouse-to-semantic-intent mapping for acceptance.

## Current input inventory

The native Client reads the inherited TTY as a raw byte stream. `InputDispatcher` walks each byte
through, in order, an exclusive modal editor decoder, a close-confirmation capture, and the
`ClientView` prefix state machine; every byte that reaches none of them is forwarded to the
terminal in its original order. `ClientView` is a pure decision function: `decide` maps one byte to
at most one `ClientAction`, `commit` applies the transition. `ClientAction::backend_action` splits
actions that need the session port from purely local ones.

The implemented prefix map binds 33 byte values. The product-facing actions documented in the
[`apps/orchbee` guide](../../apps/orchbee/README.md#native-tab-interactions) number 17; the full
implemented map adds the sidebar toggle, shortcut help, literal-prefix forwarding, and prefix
dismissal from the accepted initial contract:

| Key                   | Behavior                                                              | Semantics          |
| --------------------- | --------------------------------------------------------------------- | ------------------ |
| `c`                   | `tab.create` in the active context                                    | Workbench mutation |
| `n` / `p`             | Select next / previous Tab, switching the display attachment          | Client-local       |
| `1` … `9`             | Select the ordered Tab, switching the display attachment              | Client-local       |
| `[` / `]`             | Select previous / next standalone or Workspace context                | Client-local       |
| `a`                   | `workspace.create` through the staged setup wizard                    | Workbench mutation |
| `e`                   | `workspace.primary-directory.accept` for an Empty Workspace           | Workbench mutation |
| `r` / `R`             | `workspace.rename` / `project.rename` through the rename editor       | Workbench mutation |
| `<` / `>`             | `tab.move` previous / next                                            | Workbench mutation |
| `,`                   | `tab.rename` through the rename editor                                | Workbench mutation |
| `x`                   | `tab.close` after the drawn confirmation                              | Workbench mutation |
| `w`                   | Toggle the navigator overlay; refresh performs a Server snapshot read | Client-local       |
| `h` / `j` / `k` / `l` | Move local focus across the split tree                                | Client-local       |
| `q` / `d`             | Close this Client Session; retained execution continues               | Client-local       |
| `b`                   | Toggle the business sidebar (docked or overlay by viewport)           | Client-local       |
| `?`                   | Open the shortcut-help overlay                                        | Client-local       |
| `Ctrl+B`              | Forward one literal `0x02` byte to the terminal                       | Client-local       |
| `Escape`              | Dismiss the pending prefix                                            | Client-local       |

Four other input surfaces exist beside the prefix map:

- The modal line editors own the keyboard while a rename draft (512 Unicode scalars) or a
  workspace-setup stage (project selection, label, or a 4096-byte directory path) is open. The
  decoder bounds UTF-8 fragmentation, CSI/SS3 sequences, and bracketed paste; Enter submits,
  Escape cancels, Backspace removes one scalar, and Ctrl+U clears the draft.
- The close confirmation captures a complete input packet so that trailing bytes cannot leak
  into the shell; only its exact confirmation keys act.
- The navigator, shortcut-help, and sidebar overlays do not capture input. Ordinary bytes keep
  flowing to the terminal while they are open, and only the prefix layer still interprets keys.
- Terminal modes (raw mode, alternate screen, bracketed paste `?2004`) are owned by the session
  lifecycle through private `crossterm::Command` sequences with restoration steps, not by the
  crossterm event stream.

## Capacity argument

The flat prefix layer occupies 33 positions and its mnemonic space is nearly exhausted:

| Key space         | Bound                               | Free                                |
| ----------------- | ----------------------------------- | ----------------------------------- |
| Lowercase letters | 15: `a b c d e h j k l n p q r w x` | 11: `f g i m o s t u v y z`         |
| Uppercase letters | 1: `R`                              | 25                                  |
| Digits            | 9: `1`–`9`                          | 1: `0`                              |
| Punctuation       | 6: `[` `]` `<` `>` `,` `?`          | about 26 printable positions        |
| Control bytes     | 2: `Ctrl+B`, `Escape`               | effectively none safe for shortcuts |

The already-admitted demand alone exceeds what remains memorable:

- `panel.split` needs four directional variants; `panel.close` and `panel.move` need keys.
- Scrollback entry/exit, a search editor, and selection export form a second domain.
- Navigator deepening needs movement, activation, and filtering once it captures input.
- Session and attachment management (status surface, control-grant request) needs keys.
- `panel.resize`, if admitted on the wire, needs step grow/shrink bindings.

Fifteen or more new bindings against eleven free letters would force punctuation with no mnemonic
value, and a flat map near fifty entries is past reliable recall. The vocabulary needs a second
level; the question is which shape carries it.

## Second-level structure selection

Every key, whatever structure dispatches it, resolves to exactly one of three intents: a
`workbench.command` mutation on the shared document, an attachment-scoped terminal-protocol
effect such as `MouseInput`, `SetHistoryDemand`, or `SelectionGesture`, or a purely Client-local
view or session transition. The second level is therefore a Client-local input concern only —
the wire and the Server contract never see it.

- **(a) Second prefix under `Ctrl+B` (tmux-style extension).** A first-level key arms a named
  sub-prefix, a second key fires the action. Keeps every sequence at two keystrokes and adds no
  visible state, but each sub-prefix is another magic letter to memorize, a dangling armed prefix
  needs its own timeout/dismiss rules, and grouping is invisible until help is open.
- **(b) Explicit mode family (Zellij-style).** A prefix key enters a named mode with a visible
  indicator; inside the mode the whole keyboard is rebound to that domain until Escape. Modes
  reuse the same letters per domain, absorb large low-frequency domains cheaply, and make
  repeated operations efficient. The cost is modal risk: a user who forgets the mode is lost
  unless the indicator is always visible and Escape always exits.
- **(c) Command palette / `:` command line.** One binding opens a line editor that takes a typed
  command using the CLI noun/verb grammar, e.g. `panel split right`. Discoverable, unbounded in
  verbs, and the only natural home for parameterized actions like resize amounts — but too slow
  for spatial, high-frequency operations.
- **(d) Hybrid.** The flat prefix keeps high-frequency one-shot actions; a small reserved set of
  named modes carries low-frequency domains; `:` carries parameterized and rare commands and
  doubles as the discovery path into both.

Recommendation: **(d), the hybrid.** The split follows action frequency, which is what the
existing prefix already does well; modes give domains like panel management and scrollback a
whole second keyboard without consuming the flat namespace; and `:` reuses the accepted CLI
grammar, so the command language the TUI already promises stays consistent. A pure (b) design
would push even `c` and `n` behind a mode wall, and a pure (c) design makes split and focus
operations typing-bound. Option (a) is the weakest: it adds invisible state and second-order
mnemonics without adding capacity where the pressure actually is.

Mode mechanics follow the existing exclusive-input pattern: a mode is one more input owner beside
the modal editors and the confirmation capture, entered and exited as Client-local view
transitions. While any owner holds the keyboard, no bytes pass through to the terminal; a
persistent indicator names the active mode; Escape always returns to the ordinary layer; and
every key still resolves to at most one typed intent per input event.

## Proposed vocabulary extension

All bindings below are **provisional** — a draft for acceptance, not committed behavior.
Convention: new uppercase bindings open named modes; new lowercase and punctuation bindings stay
one-shot actions. `R` predates this convention and stays as it is.

Top-level additions after `Ctrl+B`:

| Binding | Action                                                                    | Semantics          | Status      |
| ------- | ------------------------------------------------------------------------- | ------------------ | ----------- |
| `-`     | `panel.split` placing the new panel below                                 | Workbench mutation | Provisional |
| `_`     | `panel.split` placing the new panel above                                 | Workbench mutation | Provisional |
| `\`     | `panel.split` placing the new panel left                                  | Workbench mutation | Provisional |
| `\|`    | `panel.split` placing the new panel right                                 | Workbench mutation | Provisional |
| `X`     | `panel.close` on the focused leaf, with the same drawn-confirmation style | Workbench mutation | Provisional |
| `z`     | Toggle a Client-local zoom of the focused panel                           | Client-local       | Provisional |
| `P`     | Enter the `PANEL` mode                                                    | Client-local       | Provisional |
| `U`     | Enter the `SCROLLBACK` mode                                               | Client-local       | Provisional |
| `:`     | Open the command line                                                     | Client-local       | Provisional |
| `i`     | Open a session/attachment status overlay                                  | Client-local       | Provisional |
| `t`     | Request the controller grant when the Client is view-only                 | Client-local       | Provisional |

The split glyph bindings read as the line they draw: `-` splits into rows and `|` into columns;
the unshifted glyph places the new panel at the natural end (below, right) and its shifted
variant mirrors it (above, left). `X` narrows `x` from the Tab to the focused Panel.
`u`-family letters were kept free, so `U` can read as up into history.

`PANEL` mode draft map (mode-local namespace; Escape exits):

| Key             | Action                                      | Semantics          | Status            |
| --------------- | ------------------------------------------- | ------------------ | ----------------- |
| `h`/`j`/`k`/`l` | `panel.move` the focused panel to that side | Workbench mutation | Provisional       |
| `x`             | `panel.close` on the focused leaf           | Workbench mutation | Provisional       |
| `z`             | Toggle Client-local zoom                    | Client-local       | Provisional       |
| `+` / `-`       | Grow / shrink the focused panel one step    | Workbench mutation | Blocked: wire gap |
| `=`             | Reset the focused split to equal shares     | Workbench mutation | Blocked: wire gap |
| `?`             | Mode-specific shortcut help                 | Client-local       | Provisional       |
| `Escape`        | Exit the mode                               | Client-local       | Provisional       |

`SCROLLBACK` mode draft map. Entering declares `ViewingHistory` demand on the attachment; leaving
returns `FollowingBottom`. All viewing keys are Client-local drives over existing wire reads:

| Key                    | Action                                              | Semantics         | Status      |
| ---------------------- | --------------------------------------------------- | ----------------- | ----------- |
| `j`/`k` or `Down`/`Up` | Scroll one row                                      | Attachment-scoped | Provisional |
| `d`/`u` or page keys   | Scroll one viewport                                 | Attachment-scoped | Provisional |
| `g` / `G`              | Jump to oldest / newest                             | Attachment-scoped | Provisional |
| `/`                    | Open the search line editor over `Search` row reads | Client-local      | Provisional |
| `n` / `N`              | Next / previous search match                        | Client-local      | Provisional |
| `y`                    | Export the active selection                         | Attachment-scoped | Provisional |
| `q` / `Escape`         | Exit the mode                                       | Client-local      | Provisional |

The `:` command line reuses the modal line-editor machinery and the CLI noun/verb grammar:
`panel split right`, `panel resize right 10`, `tab rename`, `help panel`. Enter submits at most
one typed intent; Escape cancels. It is the designated home for any parameterized action,
including resize once the wire admits it.

Navigator deepening is a separate decision because the overlay currently never captures input.
If admitted as a capturing surface, its draft map is `j`/`k` or arrows to move, `h`/`l` to switch
context grouping, `Enter` to activate, `/` to filter, `x` to request Close on the marked entry,
and `Escape` to close — all Client-local, with activation resolving to the same selection intents
as the top-level select keys.

Help discovery stays on `?` at the top level, becomes context-sensitive inside modes, and is
reachable through `:` as `help`.

## Mouse-to-semantic-intent mapping

Capability findings, verified against the locked `crossterm 0.29.0` dependency:

- `EnableMouseCapture`/`DisableMouseCapture`, `EnableFocusChange`, `MouseEvent`, and the
  `event::read`/`poll` stream exist but are gated behind the `events` and `bracketed-paste`
  cargo features, which `apps/orchbee` disables through `default-features = false`. Enabling
  them pulls `mio` and `signal-hook-mio` into the tree.
- The established Orchbee pattern already emits private DECSET sequences directly (`?2004` for
  bracketed paste) under the terminal-mode lifecycle with restoration steps. Mouse capture can
  follow the same pattern with `?1002` button-event tracking plus `?1006` SGR encoding, with no
  new dependency and no move to the event stream.
- SGR mouse reports then arrive in the same raw byte stream as `ESC [ < b ; x ; y M/m`, which
  the input layer can decode beside the existing CSI handling. `?1003` any-motion tracking is
  not needed for click, drag, and wheel.

Draft gesture mapping. The hit regions come from the Client-local layout projection:

| Gesture                    | Surface         | Semantic intent                                                                  | Semantics          | Status            |
| -------------------------- | --------------- | -------------------------------------------------------------------------------- | ------------------ | ----------------- |
| Left click                 | Tab strip entry | Select that Tab                                                                  | Client-local       | Provisional       |
| Left click                 | Panel viewport  | Focus that panel; forward as `MouseInput` only while the inner app reports mouse | Client-local       | Provisional       |
| Left drag                  | Split boundary  | One `panel.resize` intent on release                                             | Workbench mutation | Blocked: wire gap |
| Left click / activate      | Navigator entry | Resolve to the same selection intent as the select keys                          | Client-local       | Provisional       |
| Wheel up / down            | Panel viewport  | Enter or move within `SCROLLBACK` demand; at bottom return to `FollowingBottom`  | Attachment-scoped  | Provisional       |
| Left drag                  | Panel text      | `SelectionGesture` start/update, then `SelectionExport`                          | Attachment-scoped  | Provisional       |
| Modified drag (e.g. Shift) | Anywhere        | Leave to the outer terminal's own bypass for native selection                    | Client-local       | Provisional       |

Selection versus mouse-reporting passthrough is the one real policy decision. When an app inside
the terminal enables its own mouse reporting, drags in its viewport belong to that app; when it
does not, the same gesture is Orchbee's own text selection. The Client cannot make that choice
today because the inner terminal's mouse mode is not projected to it. Recommendation: project
the terminal's mouse-reporting flag as a Client-visible terminal fact (wire gap below); until
then the conservative interim is click-to-focus only, drag selects Orchbee text, and nothing is
forwarded as `MouseInput`. This keeps every gesture deterministic and leaves app mouse support
to a later admitted slice.

## Wire gaps and decision requests

Existing wire already covers `MouseInput`, `FocusInput`, `SetHistoryDemand`, `ReadRows`
(`Buffer`, `Search`, `Viewport`), `SelectionGesture`/`SelectionCommand`/`SelectionChanged`, and
`SelectionExport`. The gaps:

| Gap                                       | Needed for                             | Decision requested                                                                |
| ----------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------- |
| `panel.resize` `WorkbenchAction`          | Boundary-drag and keyboard step resize | Admit a resize mutation — shape options: direction plus delta, or new slot ratios |
| Inner-terminal mouse-mode projection      | Selection-versus-passthrough routing   | Expose the terminal's mouse-reporting flag as a Client-visible terminal fact      |
| Clipboard hand-off for exported selection | `y` and drag-selection delivery        | Confirm OSC52 or platform clipboard as the Client-local delivery path             |

## Non-goals

- Floating panes, plugin systems, configurable keymap files, tmux session semantics, and
  client-side tiling are excluded by the standing research verdict.
- No code changes in this proposal; implementation lands in its own slices after acceptance.
- No new Server semantics are designed here beyond the explicitly listed wire-gap requests.
- No adoption of the crossterm event stream or new dependencies; the raw-byte input path and
  private mode sequences stay.
- No second prefix byte or public configurable chord; `Ctrl+B` remains the only chord.

## Effect on accepted contracts

The `Ctrl+B` initial map in `native-command-surface.md` is an accepted _initial_ contract. This
proposal extends it additively: every existing binding keeps its behavior, and the invariants
below are preserved or generalized, not weakened:

- One chord, one pending layer: `Ctrl+B` is still the only top-level entry; `Ctrl+B` `Ctrl+B`
  still sends a literal `0x02`; Escape still dismisses a pending prefix.
- Ordered passthrough: bytes owned by no input surface keep reaching the terminal in order,
  unmodified.
- At most one typed intent per input event, now including mode keys, mode-local keys, `:` line
  submissions, and mouse gestures.
- Exclusive input ownership with a visible indicator: modal editors, the confirmation capture,
  named modes, the command line, and any capture-capable overlay are the same kind of owner;
  Escape always returns to the ordinary layer.
- Unknown input stays inert: an unbound key produces a notice and no side effect.
- Modes, overlays, and palettes are Client-local input machinery; nothing about them ever
  appears on the wire, and every Server-semantic outcome still resolves to exactly one
  `workbench.command` with a fresh request identity under the at-most-once mutation rule.

This proposal does not change `native-tab-session.md`'s runtime input bounds, the confirmation
draw requirements, or any wire schema. Each accepted slice updates the owning app documentation
in the same change.

## Decision requests

- **D1 — Structure:** accept the hybrid (d), or redirect to a different second-level shape.
- **D2 — Key tables:** accept, adjust, or strike each provisional binding above.
- **D3 — `panel.resize` wire:** admit it and choose its shape; otherwise boundary-drag and
  resize keys stay unbound.
- **D4 — Mouse-mode projection:** expose the inner terminal's mouse flag, or keep the interim
  focus-only rule.
- **D5 — Mouse capture:** enable `?1002`+`?1006` at session start with lifecycle restoration.
- **D6 — Navigator capture:** keep non-capturing, or admit the capturing draft map.
- **D7 — Zoom:** admit the Client-local zoom toggle or drop `z`.
