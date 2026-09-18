# Native panel close confirmation

## Scope and ownership

The native Client treats `Ctrl+B X` (close the selected panel leaf) as a destructive intent
that must be confirmed before it can reach the workbench carrier. This reference governs the
panel-close confirmation extension of the [native Tab Session](native-tab-session.md) and the
[native panel lifecycle](native-panel-lifecycle.md). Maintain it with
`terminal_session/state/panel_close.rs`, `terminal_session/ui/panel_close.rs`,
`tui_render/panel_close.rs`, `localization/panel_close.rs`, and the receipt fields in
`terminal_session/ui/tab_close.rs`.

A panel close is not a Tab close. The confirmed effect is exactly the existing
`PanelRequestKind::Close` operation planned against the frozen leaf; the confirmation layer
never re-shapes it into a `CloseOperation` or any Tab-level intent.

## State and transition contract

| Transition                     | Guard and result |
| ------------------------------ | ---------------- |
| Arm panel-close confirmation   | A stable Session (`Ready`, `SelectedWithoutAttachment`, or `RetryableAttachUnavailable`) with no pending refresh, no structural mutation, no rename/setup draft, no armed Tab-close confirmation, and no armed panel-close confirmation captures the selected leaf: site, terminal context, Tab id, panel id, panel and Tab labels, leaf binding, whether the leaf is the Tab's last one, and the baseline revision. The prompt is armed but not yet shown. |
| Draw the prompt                | The renderer draws the localized overlay naming the frozen leaf and its impact — closing the Tab's last leaf ends the Tab for every attached Client; closing any other leaf ends only that leaf's terminal — and reports the armed token in the draw receipt. Content that cannot fit draws only the resize hint and reports no token. |
| Acknowledge the draw           | Only a receipt carrying the exact armed token marks the confirmation shown; a missing, stale, or mismatched token leaves it unshown. Resize invalidates the shown flag until the next honest draw. |
| Confirm the close              | A drawn confirmation decides only on the exact lowercase `y`. Confirmation re-validates the frozen target against the live Session — same site, context, revision, selected Tab, selected panel id, and leaf binding — then plans and admits the ordinary `PanelRequestKind::Close` operation. Any drift refuses as stale; the effect can never follow a moved focus onto a different leaf. |
| Cancel the close               | Only `Esc` cancels; every other key is a swallowed non-decision. The default outcome of an abandoned or invalidated prompt is cancel. |
| Own the decision packet        | While a confirmation is armed it owns keyboard input. The whole input packet carrying the decision byte is captured, so bytes trailing `y` or `Esc` in the same packet never reach the shell or the prefix layer; later independent packets dispatch normally. |
| Invalidate on drift            | Refresh, leaf selection change, retirement, or any structural drift invalidates or blocks the armed token; the confirm path re-checks the frozen facts at admission and refuses stale ones. |

## Invariants and temporal rules

- `OneConfirmationOwner`: at most one modal confirmation owns input — Tab close, panel close,
  rename draft, or setup draft; opening any of them while another lives refuses as busy.
- `NoUndrawnExecution`: without a shown acknowledgement bound to the armed token, no plan and
  no submission exists — undrawn, too-small, or invalidated geometry cannot produce an effect.
- `FrozenTargetOrRefuse`: the admitted operation always names the frozen leaf and baseline; a
  moved focus or drifted revision refuses instead of redirecting the close.
- `ExactDecisionKeys`: only `y` confirms and only `Esc` cancels; all other input is captured
  as non-decision while armed, including `n`, `N`, and `Y`.
- `PacketTailCaptured`: the decision packet is consumed whole; a cancelled or confirmed prompt
  never leaks trailing bytes to the shell.
- `PanelStaysPanel`: the confirmed effect is `PanelIntent::Close` on the frozen leaf via the
  existing panel submission path — never a Tab close, never a different leaf.
- `CancelIsDefault`: absent or refused decisions leave the panel untouched.

## Source correspondence

| Production surface | Model correspondence |
| ------------------ | -------------------- |
| `terminal_session/state/panel_close.rs` | Armed/shown state, frozen `PanelCloseSelection`, token issuance, `confirmed_panel_selection`, `plan_panel_close_confirmation`, `begin_panel_close_exchange` |
| `terminal_session/ui/panel_close.rs` | `NativePanelCloseConfirmationView`, `request_panel_close_confirmation`, `confirm_panel_close`, `cancel_panel_close`, `capture_panel_confirmation_key` |
| `terminal_session/ui/tab_close.rs` | `NativeDrawReceipt.panel_prompt`, `capture_confirmation_key` routing, exact-key tightening, `after_close_draw`/`invalidate_close_display` fan-out |
| `tui_render/panel_close.rs` | Overlay draw, fit gate, token-bearing receipt |
| `localization/panel_close.rs` | `en-US`/`zh-CN` title, impact warning, action and resize-hint strings |
| `terminal_session/ui.rs` | `ClientAction::ClosePanel` arm to `request_panel_close_confirmation` |
