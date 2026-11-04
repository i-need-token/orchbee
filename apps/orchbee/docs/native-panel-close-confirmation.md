# Native panel close confirmation

## Scope and ownership

The native Client treats `Ctrl+B Shift+X` (close the selected panel leaf) as a destructive intent
that must be confirmed before it can reach the workbench carrier, unless the Server finds no program
running in its terminals ([idle close](native-tab-close-sets.md#idle-close)). This reference governs
the panel-close confirmation extension of the [native Tab Session](workbench-client-runtime.md) and
the [native panel lifecycle](native-panel-lifecycle.md). Maintain it with
`workbench_state/panel_close.rs`, `workbench_client/ui/panel_close.rs`, `tui_render/panel_close.rs`,
`localization/panel_close.rs`, and the receipt fields in `workbench_client/ui/tab_close.rs`.

A panel close is not a Tab close. The confirmed effect is exactly the existing
`PanelRequestKind::Close` operation planned against the frozen leaf; the confirmation layer
never re-shapes it into a `CloseOperation` or any Tab-level intent.

The frozen target includes its one-based terminal position within the Tab and the Tab's
one-based position in the current location. The prompt presents both positions with their
sanitized labels on separate lines, so duplicate labels still identify the affected terminal.
These positions are display facts, not effect identities; the original identity, revision,
complete-fit, draw-token and cancellation guards continue to authorize the close.

## State and transition contract

| Transition                    | Guard and result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Arm panel-close confirmation  | A stable Session (`Ready`, `SelectedWithoutAttachment`, or `RetryableAttachUnavailable`) with no pending refresh, no structural mutation, no rename/setup draft, no armed Tab-close confirmation, and no armed panel-close confirmation captures the selected leaf: session, terminal context, Tab id, panel id, panel and Tab labels and one-based positions, leaf binding, whether the leaf is the Tab's last one, and the baseline revision. The prompt is armed but not yet shown.                                                                                                                                                                          |
| Ask the Server                | A Controlling Client sends `PanelRequestKind::CloseIfIdle` for the frozen leaf as soon as the prompt is armed, and the prompt stays hidden and captures no input while it is pending, so keys typed meanwhile queue for the terminal behind the request. Accepted, it applies as the ordinary close and drops the prompt; `confirmation-required` leaves the prompt to draw; any other refusal drops it with the not-applied notice. A ReadOnly Client, or one that cannot send the request, draws the prompt at once.                                                                                                                                          |
| Draw the prompt               | The renderer draws the localized dialog centered in the whole window at its class size, naming the frozen leaf and its impact — closing the Tab's last leaf ends the Tab for every attached Client; closing any other leaf names the running work and retained output it ends in that leaf's Presentations, as [close impact](native-tab-close-sets.md#close-impact) counts them at this draw. The draw reports a typed outcome: `Presented` with the armed token when the complete content fits, or `NotPresented` when the offered area cannot present it. Rendering never mutates Session state; it only produces the receipt the UI settles after the draw. |
| Acknowledge the draw          | Only a `Presented` receipt carrying the exact armed token marks the confirmation shown and admits that presentation incarnation. A missing or mismatched receipt revokes exact-draw authority without deciding why the surface was absent.                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Settle an un-presentable draw | `NotPresented` means only that this renderer evaluated the prompt and geometry blocked it. Before any successful presentation, the new prompt is refused and removed, and the status line says it cannot open there. After a successful presentation, the same result suspends the existing prompt: retain its token and frozen target, revoke confirm and pointer authority, allow local `Esc` cancellation, and resume only when the same token is completely presented again. It is not reported as user cancellation.                                                                                                                                       |
| Confirm the close             | A drawn confirmation decides only on the unmodified `Enter`. Confirmation re-validates the frozen target against the live Session — same session, context, revision, selected Tab, selected panel id, leaf binding, and last-leaf nature — then plans and admits the ordinary `PanelRequestKind::Close` operation. Any drift refuses as stale; the effect can never follow a moved focus onto a different leaf.                                                                                                                                                                                                                                                 |
| Cancel the close              | Local `Esc` or a matching plain-left click on the outside backdrop cancels; every other key is a swallowed non-decision. A stale frozen premise also removes the prompt with a bounded notice, while missing draw proof alone never becomes an implicit cancellation or affirmative decision.                                                                                                                                                                                                                                                                                                                                                                   |
| Own the decision packet       | While a confirmation is armed and not awaiting the Server, it owns keyboard input. The whole input packet carrying the decision byte is captured, so bytes trailing `Enter` or `Esc` in the same packet never reach the shell or the prefix layer; later independent packets dispatch normally.                                                                                                                                                                                                                                                                                                                                                                 |
| Invalidate on drift           | Refresh, leaf selection change, retirement, or any structural drift makes the armed confirmation stale. Staleness is settled automatically — the armed capture is cancelled and a bounded localized notice is posted — never left armed-but-unshown indefinitely. The confirm path also re-checks the frozen facts at admission as defense in depth.                                                                                                                                                                                                                                                                                                            |
| Retire the Session            | Session retirement and carrier retirement drop any armed confirmation outright; no prompt survives a retired Session to keep swallowing keys.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

## Invariants and temporal rules

- `OneConfirmationOwner`: at most one modal confirmation owns input — Tab close, panel close,
  rename draft, or setup draft; opening any of them while another lives refuses as busy.
- `NoUndrawnExecution`: without a shown acknowledgement bound to the armed token, no ordinary
  close is planned or submitted — undrawn, too-small, or invalidated geometry cannot produce an
  effect. The only undrawn submission is the conditional close, which the Server admits only when
  nothing runs in the leaf.
- `FrozenTargetOrRefuse`: the admitted operation always names the frozen leaf and baseline; a
  moved focus or drifted revision refuses instead of redirecting the close.
- `StaleSettlesCancelled`: a confirmation whose frozen identity no longer matches live state is
  cancelled automatically and posts a bounded localized notice; it never lingers armed-but-stale
  swallowing keys.
- `RefuseOrSuspendOnGeometry`: a geometry block refuses an incarnation that never completed its
  first paint; after admission it suspends and preserves the same token and frozen target until a
  later complete paint or explicit local cancellation.
- `MissingReceiptIsNotGeometry`: an absent receipt revokes shown authority only; it does not
  classify geometry, cancel the prompt, or discard its lifecycle phase.
- `RetiredMeansGone`: session or carrier retirement removes the armed confirmation outright.
- `RenderNeverMutates`: the renderer only returns the typed outcome; all Session mutation
  happens in the post-draw settlement inside the UI layer.
- `ExactDecisionKeys`: only unmodified `Enter` confirms; `Esc` or the painted outside-backdrop dismissal cancels; all other input is captured
  as non-decision while armed, including `y`, `n`, `N`, and `Y`.
- `PacketTailCaptured`: the decision packet is consumed whole; a cancelled or confirmed prompt
  never leaks trailing bytes to the shell.
- `PanelStaysPanel`: the confirmed effect is `PanelIntent::Close`, and the unasked one
  `PanelIntent::CloseIfIdle`, on the frozen leaf via the existing panel submission path — never a
  Tab close, never a different leaf.
- `CancelIsDefault`: absent or refused decisions leave the panel untouched.
- `ImpactNeverUnderstates`: the warning counts a terminal as ended only from a launch failure or
  a Host settlement on its exact binding. Confirmation does not re-validate the count; the frozen
  revision and leaf binding keep the executed close from exceeding the last drawn warning.

## Source correspondence

| Production surface                   | Model correspondence                                                                                                                                                                                                                                          |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `workbench_state/panel_close.rs`     | Armed/shown state, frozen `PanelCloseSelection`, token issuance, `confirmed_panel_selection`, `plan_panel_close_confirmation`, `begin_panel_close_exchange`, `plan_panel_close_if_idle`, `begin_panel_close_if_idle`, live-match and not-presented settlement |
| `workbench_state/close_impact.rs`    | Running and ended terminal count over the frozen leaf's Presentations                                                                                                                                                                                         |
| `workbench_state/tab_close.rs`       | Cross-confirmation `settle_stale_close_confirmations` routing and carrier-retirement cancellation                                                                                                                                                             |
| `workbench_client/ui/idle_close.rs`  | `begin_idle_close`: the conditional close of the armed prompt, or the prompt drawn at once                                                                                                                                                                    |
| `workbench_client/ui/panel_close.rs` | `PanelCloseConfirmationView`, `request_panel_close_confirmation`, `confirm_panel_close`, `cancel_panel_close`, `capture_panel_confirmation_key`                                                                                                               |
| `workbench_client/ui/tab_close.rs`   | `DrawReceipt` typed `ModalPresentation` fields, `capture_confirmation_key` routing and stale settlement, exact-key tightening, `after_close_draw`/`invalidate_close_display` fan-out                                                                          |
| `workbench_client/ui.rs`             | `WorkbenchClientUi::tick` post-mutation stale settlement, `WorkbenchClientNotice::ConfirmationCancelled`                                                                                                                                                      |
| `tui_presentation.rs`                | Shared pending/admitted/suspended presentation phase and owner-level refused/suspended geometry settlement                                                                                                                                                    |
| `tui_render/panel_close.rs`          | Content-area overlay draw at preferred finite size, fit gate, typed `Presented`/`NotPresented` receipt                                                                                                                                                        |
| `localization/panel_close.rs`        | `en-US`/`zh-CN` title, impact warning, raw action labels and key-hint labels                                                                                                                                                                                  |
| `workbench_client/ui.rs`             | `ClientAction::ClosePanel` arm to `request_panel_close_confirmation`                                                                                                                                                                                          |
