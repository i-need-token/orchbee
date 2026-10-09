# 原生 panel 关闭确认

## 范围与归属

原生 Client 将 `Ctrl+B Shift+X`（关闭当前选中的 panel 叶）视为破坏性意图，必须先经确认才能到达
workbench 载波。本记录约束 [native Tab Session](native-tab-session.zh-CN.md) 与
[native panel 生命周期](native-panel-lifecycle.zh-CN.md) 的 panel 关闭确认扩展。维护时请同步
`terminal_session/state/panel_close.rs`、`terminal_session/ui/panel_close.rs`、
`tui_render/panel_close.rs`、`localization/panel_close.rs`，以及
`terminal_session/ui/tab_close.rs` 中的回执字段。

panel 关闭不是 Tab 关闭。确认后的效果就是针对被冻结叶走既有 `PanelRequestKind::Close`
操作；确认层绝不把它改写成 `CloseOperation` 或任何 Tab 级意图。

冻结目标包含终端在 Tab 内从一开始计数的位置，以及 Tab 在当前位置内从一开始计数的位置。
提示分两行显示这两个位置和清理后的标签，因此标签重复时仍能识别受影响的终端。
位置只是显示事实，不是效果身份；关闭仍由原身份、revision、完整 fit、绘制 token 与取消守卫
授权。

## 状态与迁移契约

| 迁移                | 守卫与结果                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 武装 panel 关闭确认 | 稳定 Session（`Ready`、`SelectedWithoutAttachment` 或 `RetryableAttachUnavailable`）、无待决刷新、无结构变更、无 rename/setup 草稿、无已武装的 Tab 关闭确认、无已武装的 panel 关闭确认时，冻结当前选中叶：site、terminal context、Tab id、panel id、panel 与 Tab 标签及其从一开始计数的位置、叶 binding、该叶是否为 Tab 的最后一片叶、基线 revision。提示已武装但尚未 shown。 |
| 绘制提示            | 渲染器在统一内容区内以该 surface 的首选有限尺寸绘制本地化覆盖层，指名被冻结的叶及其影响——关闭 Tab 的最后一片叶会为所有已连接 Client 终结该 Tab；关闭其他叶只终结该叶中的终端。绘制报告类型化结果：内容完整放得下时回执为携带已武装 token 的 `Presented`，放不下时为 `NotPresented`。渲染绝不改 Session 状态，只产出供 UI 在绘制后结算的回执。                                 |
| 回执确认绘制        | 只有携带确切已武装 token 的 `Presented` 回执才把确认标记为 shown；`NotPresented`、缺失或不匹配的 token 使其保持未 shown。resize 会使 shown 失效，直到下一次真实绘制。                                                                                                                                                                                                         |
| 不可呈现绘制结算    | 确认武装期间收到 `NotPresented` 回执即取消该确认并发布有界本地化状态提示：放不下的几何结束输入捕获，而不是无限吞键。resize 后仍放得下的空间重新绘制并正常继续。                                                                                                                                                                                                               |
| 确认关闭            | 已绘制的确认只认精确小写 `y`。确认时按存活 Session 复核冻结目标——同一 site、context、revision、选中 Tab、选中 panel id、叶 binding 与最后叶性质——再走普通 `PanelRequestKind::Close` 的计划与准入。任何漂移按 stale 拒绝；效果绝不随焦点移动落到另一片叶上。                                                                                                                   |
| 取消关闭            | 仅 `Esc` 取消；其余按键一律作为非决定吞掉。被放弃或失效的提示默认结果为取消。                                                                                                                                                                                                                                                                                                 |
| 决定包独占          | 确认武装期间独占键盘输入。携带决定字节的整个输入包被完整捕获，`y` 或 `Esc` 之后同包尾部字节绝不泄漏到 shell 或前缀层；其后的独立输入包正常派发。                                                                                                                                                                                                                              |
| 漂移失效            | 刷新、选叶变化、退役或任何结构漂移都会使已武装确认变为 stale。stale 会被自动结算——取消已武装捕获并发布有界本地化提示——绝不留下无限吞键的 armed-but-unshown 状态。确认路径在准入时仍会复核冻结事实作为纵深防御。                                                                                                                                                               |
| Session 退役        | Session 退役与载波退役都会直接丢弃已武装确认；退役的 Session 上不会残留继续吞键的提示。                                                                                                                                                                                                                                                                                       |

## 不变量与时序规则

- `OneConfirmationOwner`：任意时刻至多一个模态确认持有输入——Tab 关闭、panel 关闭、
  rename 草稿或 setup 草稿；其一存活时再开其余一律按 busy 拒绝。
- `NoUndrawnExecution`：没有与已武装 token 绑定的 shown 回执，就没有计划与提交——
  未绘制、过小或已失效的几何都无法产生效果。
- `FrozenTargetOrRefuse`：被准入的操作始终指名被冻结的叶与基线；焦点移动或 revision
  漂移一律拒绝，而不是把关闭重定向到别处。
- `StaleSettlesCancelled`：冻结身份与存活状态不再匹配的确认会被自动取消并发布
  有界本地化提示；绝不残留 armed-but-stale 状态继续吞键。
- `PresentedOrCancelled`：绘制结果只有 `Presented`（携带已武装 token）或
  `NotPresented`；武装期间收到 `NotPresented` 即取消并提示，而不是无限等待。
- `RetiredMeansGone`：Session 或载波退役直接移除已武装确认。
- `RenderNeverMutates`：渲染器只返回类型化结果；所有 Session 变更都发生在 UI 层的
  绘制后结算中。
- `ExactDecisionKeys`：仅 `y` 确认、仅 `Esc` 取消；武装期间其余输入（含 `n`、`N`、`Y`）
  全部按非决定捕获。
- `PacketTailCaptured`：决定包整体被消费；取消或确认后的提示绝不向 shell 泄漏尾部字节。
- `PanelStaysPanel`：确认效果是经既有 panel 提交通道对被冻结叶的 `PanelIntent::Close`——
  绝不是 Tab 关闭，也绝不是另一片叶。
- `CancelIsDefault`：缺席或被拒绝的决定使 panel 保持原样。

## 源码对应

| 生产面                                  | 模型对应                                                                                                                                                                      |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal_session/state/panel_close.rs` | 武装/shown 状态、冻结的 `PanelCloseSelection`、token 签发、`confirmed_panel_selection`、`plan_panel_close_confirmation`、`begin_panel_close_exchange`、存活匹配与不可呈现结算 |
| `terminal_session/state/tab_close.rs`   | `settle_stale_close_confirmations`、不可呈现结算、载波退役取消                                                                                                                |
| `terminal_session/ui/panel_close.rs`    | `NativePanelCloseConfirmationView`、`request_panel_close_confirmation`、`confirm_panel_close`、`cancel_panel_close`、`capture_panel_confirmation_key`                         |
| `terminal_session/ui/tab_close.rs`      | `NativeDrawReceipt` 类型化 `ModalPresentation` 字段、`capture_confirmation_key` 路由与 stale 结算、精确键收敛、`after_close_draw`/`invalidate_close_display` 分发             |
| `terminal_session/ui.rs`                | `NativeUi::tick` 变更后 stale 结算、`NativeSessionNotice::ConfirmationCancelled`                                                                                              |
| `tui_render/panel_close.rs`             | 内容区覆盖层绘制（首选有限尺寸）、fit 门、类型化 `Presented`/`NotPresented` 回执                                                                                              |
| `localization/panel_close.rs`           | `en-US`/`zh-CN` 标题、影响警示、操作与放大提示字符串                                                                                                                          |
| `terminal_session/ui.rs`                | `ClientAction::ClosePanel` 接入 `request_panel_close_confirmation`                                                                                                            |
