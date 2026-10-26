# 原生 panel 关闭确认

## 范围与归属

原生 Client 将 `Ctrl+B Shift+X`（关闭当前选中的 panel 叶）视为破坏性意图，必须先经确认才能到达
workbench 载波。本记录约束 [native Tab Session](workbench-client-runtime.zh-CN.md) 与
[native panel 生命周期](native-panel-lifecycle.zh-CN.md) 的 panel 关闭确认扩展。维护时请同步
`workbench_client/state/panel_close.rs`、`workbench_client/ui/panel_close.rs`、
`tui_render/panel_close.rs`、`localization/panel_close.rs`，以及
`workbench_client/ui/tab_close.rs` 中的回执字段。

panel 关闭不是 Tab 关闭。确认后的效果就是针对被冻结叶走既有 `PanelRequestKind::Close`
操作；确认层绝不把它改写成 `CloseOperation` 或任何 Tab 级意图。

冻结目标包含终端在 Tab 内从一开始计数的位置，以及 Tab 在当前位置内从一开始计数的位置。
提示分两行显示这两个位置和清理后的标签，因此标签重复时仍能识别受影响的终端。
位置只是显示事实，不是效果身份；关闭仍由原身份、revision、完整 fit、绘制 token 与取消守卫
授权。

## 状态与迁移契约

| 迁移                | 守卫与结果                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 武装 panel 关闭确认 | 稳定 Session（`Ready`、`SelectedWithoutAttachment` 或 `RetryableAttachUnavailable`）、无待决刷新、无结构变更、无 rename/setup 草稿、无已武装的 Tab 关闭确认、无已武装的 panel 关闭确认时，冻结当前选中叶：session、terminal context、Tab id、panel id、panel 与 Tab 标签及其从一开始计数的位置、叶 binding、该叶是否为 Tab 的最后一片叶、基线 revision。提示已武装但尚未 shown。 |
| 绘制提示            | 渲染器在统一内容区内以该 surface 的首选有限尺寸绘制本地化覆盖层，指名被冻结的叶及其影响——关闭 Tab 的最后一片叶会为所有已连接 Client 终结该 Tab；关闭其他叶只终结该叶中的终端。绘制报告类型化结果：内容完整放得下时回执为携带已武装 token 的 `Presented`，放不下时为 `NotPresented`。渲染绝不改 Session 状态，只产出供 UI 在绘制后结算的回执。                                    |
| 回执确认绘制        | 只有携带确切已武装 token 的 `Presented` 回执才把确认标记为 shown，并准入该 presentation incarnation。缺失或不匹配的回执只撤销精确绘制权限，不推断 surface 缺席的原因。                                                                                                                                                                                                           |
| 不可呈现绘制结算    | `NotPresented` 只表示渲染器确实评估了提示且几何阻止呈现。首次成功呈现之前，新提示被拒绝并移除，同时外层 recovery owner 保持本地化尺寸恢复提示可见。成功呈现之后，相同结果挂起既有提示：保留其 token 与冻结目标，撤销确认和鼠标权限，仍允许本地 `Esc` 取消；只有同一 token 再次完整呈现后才恢复。它不会被报告为用户取消。                                                         |
| 确认关闭            | 已绘制的确认只认无修饰的 `Enter`。确认时按存活 Session 复核冻结目标——同一 session、context、revision、选中 Tab、选中 panel id、叶 binding 与最后叶性质——再走普通 `PanelRequestKind::Close` 的计划与准入。任何漂移按 stale 拒绝；效果绝不随焦点移动落到另一片叶上。                                                                                                               |
| 取消关闭            | 本地 `Esc` 或在外侧蒙层上匹配的普通左键点击会取消；其余按键一律作为非决定吞掉。冻结前提过期也会移除提示并发布有界通知；仅缺失绘制证明绝不会隐式取消或形成肯定决定。                                                                                                                                                                                                              |
| 决定包独占          | 确认武装期间独占键盘输入。携带决定字节的整个输入包被完整捕获，`Enter` 或 `Esc` 之后同包尾部字节绝不泄漏到 shell 或前缀层；其后的独立输入包正常派发。                                                                                                                                                                                                                             |
| 漂移失效            | 刷新、选叶变化、退役或任何结构漂移都会使已武装确认变为 stale。stale 会被自动结算——取消已武装捕获并发布有界本地化提示——绝不留下无限吞键的 armed-but-unshown 状态。确认路径在准入时仍会复核冻结事实作为纵深防御。                                                                                                                                                                  |
| Session 退役        | Session 退役与载波退役都会直接丢弃已武装确认；退役的 Session 上不会残留继续吞键的提示。                                                                                                                                                                                                                                                                                          |

## 不变量与时序规则

- `OneConfirmationOwner`：任意时刻至多一个模态确认持有输入——Tab 关闭、panel 关闭、
  rename 草稿或 setup 草稿；其一存活时再开其余一律按 busy 拒绝。
- `NoUndrawnExecution`：没有与已武装 token 绑定的 shown 回执，就没有计划与提交——
  未绘制、过小或已失效的几何都无法产生效果。
- `FrozenTargetOrRefuse`：被准入的操作始终指名被冻结的叶与基线；焦点移动或 revision
  漂移一律拒绝，而不是把关闭重定向到别处。
- `StaleSettlesCancelled`：冻结身份与存活状态不再匹配的确认会被自动取消并发布
  有界本地化提示；绝不残留 armed-but-stale 状态继续吞键。
- `RefuseOrSuspendOnGeometry`：几何阻塞会拒绝从未完成首次绘制的 incarnation；准入后则挂起，
  并保留同一 token 与冻结目标，直到随后完整绘制或明确的本地取消。
- `MissingReceiptIsNotGeometry`：缺失回执只撤销 shown 权限；它不判定几何、不取消提示，
  也不丢弃其生命周期 phase。
- `RetiredMeansGone`：Session 或载波退役直接移除已武装确认。
- `RenderNeverMutates`：渲染器只返回类型化结果；所有 Session 变更都发生在 UI 层的
  绘制后结算中。
- `ExactDecisionKeys`：仅无修饰的 `Enter` 确认；`Esc` 或已绘制外侧蒙层关闭动作取消；武装期间其余输入（含 `y`、`n`、`N`、`Y`）
  全部按非决定捕获。
- `PacketTailCaptured`：决定包整体被消费；取消或确认后的提示绝不向 shell 泄漏尾部字节。
- `PanelStaysPanel`：确认效果是经既有 panel 提交通道对被冻结叶的 `PanelIntent::Close`——
  绝不是 Tab 关闭，也绝不是另一片叶。
- `CancelIsDefault`：缺席或被拒绝的决定使 panel 保持原样。

## 源码对应

| 生产面                                  | 模型对应                                                                                                                                                                      |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `workbench_client/state/panel_close.rs` | 武装/shown 状态、冻结的 `PanelCloseSelection`、token 签发、`confirmed_panel_selection`、`plan_panel_close_confirmation`、`begin_panel_close_exchange`、存活匹配与不可呈现结算 |
| `workbench_client/state/tab_close.rs`   | 跨确认的 `settle_stale_close_confirmations` 路由与载波退役取消                                                                                                                |
| `workbench_client/ui/panel_close.rs`    | `PanelCloseConfirmationView`、`request_panel_close_confirmation`、`confirm_panel_close`、`cancel_panel_close`、`capture_panel_confirmation_key`                               |
| `workbench_client/ui/tab_close.rs`      | `DrawReceipt` 类型化 `ModalPresentation` 字段、`capture_confirmation_key` 路由与 stale 结算、精确键收敛、`after_close_draw`/`invalidate_close_display` 分发                   |
| `workbench_client/ui.rs`                | `WorkbenchClientUi::tick` 变更后 stale 结算、`WorkbenchClientNotice::ConfirmationCancelled`                                                                                   |
| `tui_presentation.rs`                   | 共享 pending/admitted/suspended 呈现 phase，以及 owner 层 refused/suspended 几何结算                                                                                          |
| `tui_render/panel_close.rs`             | 内容区覆盖层绘制（首选有限尺寸）、fit 门、类型化 `Presented`/`NotPresented` 回执                                                                                              |
| `localization/panel_close.rs`           | `en-US`/`zh-CN` 标题、影响警示、原始操作标签与按键提示标签                                                                                                                    |
| `workbench_client/ui.rs`                | `ClientAction::ClosePanel` 接入 `request_panel_close_confirmation`                                                                                                            |

<!-- documentation-pairing: english-sha256=8c20af93356ec08839d5cc682c65a65860f21de6506806d40601faa7df871995 zh-cn-sha256=abdec9bcfde1a620c7d08f37da94e1188c590c3501983edbcb73b5c382c564e0 -->
