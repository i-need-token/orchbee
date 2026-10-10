# 原生 Workspace 关闭

## 范围与用户结果

Windows TUI 通过 `Ctrl+B D`（`Shift+D`）关闭当前 Workspace，再按确切小写 `y` 确认；
`Esc` 取消。完整确认框显示 Workspace 名称，说明它及其关联 worktree Workspace 的全部
终端都会结束、影响每个 Client，并说明保留 Git 目录。该动作不能关闭 Standalone。这是
[`workspace close --group --confirm`](windows-worktrees.md#workspace-group-closure) 的原生投影：
普通和 linked Workspace 是单成员范围；已知 main-checkout Workspace 关闭捕获的关联组，
源最后关闭。Project 记录、Git checkout 和分支均保留。

该动作不增加 Close 协议、不删除目录、不恢复失败 Close 尝试、不接管活着的孤儿进程，
也不承诺回滚。Server 既有保留组及 child Close 所有者仍是权威。
`workspace close-status --group --request <operation-id>` 观察保留操作；可恢复失败仍通过
既有显式 CLI Resume 处理。

## 输入与归属

| 事实                                          | 所有者与校验                                                                                                   |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| 当前 Site、Project、Workspace、context 和导航 | 校验后的 Workbench snapshot，投影为 `NativeContextTabs`；ID 选择范围，label 仅用于显示                         |
| 确认                                          | `WorkspaceCloseState`：冻结来源缓存、确切 desired pane 和递增 display token                                    |
| 完整绘制与几何                                | 纯 renderer 返回 typed receipt；UI 绘制后仅认可当前提供的 token                                                |
| 请求身份、队列、lease 与期限                  | Command mailbox；单个有界 Refresh flight 将关闭与其它结构工作串行化                                            |
| 组成员与终端生命周期                          | Server 保留 operation／member／child 身份；Worker／Host 拥有实际处置                                           |
| 提交与观察证据                                | Native carrier 在后续 read／UI wait 可失败之前把 typed evidence 存入 mailbox；UI 与已 join worker 的交接保留它 |
| 选择与 Viewer 清理                            | 既有 context-removal 与 membership 所有者消费新 snapshot 和确切 attachment 身份                                |

不可信按键由既有确认 packet 所有者解码。带修饰键、大写 `Y`、Enter、粘贴和尾随字节
不能确认，也不能到达终端。缺少 draw receipt 不构成许可。来源 revision、成员或选择
变化使确认失效。传输交付前，新 snapshot 必须投影为完整冻结来源缓存；不可用或变化
输入产生确定拒绝。该检查与 Server 准入是两个进程边界：组成员按既有 CLI 组关闭语义
在 Server 准入时原子捕获；TUI 不声称增加 read／Start 事务。

## 转移模型

`WorkspaceCloseOperation` 拥有一次 group Start 和原始 `OperationDeadlines`。
`WorkspaceCloseEvidence` 区分确定未发出／拒绝、结果未知和已观察组结果。已接受组结果
包含不可变成员顺序与每个已知 child Close 身份。回执描述生命周期事实，独立于 UI 交付。

| 状态／事件               | 守卫与输出                                                                 |
| ------------------------ | -------------------------------------------------------------------------- |
| 打开                     | 当前 Workspace 且结构所有者安静；冻结来源并提供一个确认框                  |
| 绘制                     | 准入几何下内容完整；仅认可其 token                                         |
| Esc、来源过期或几何拒绝  | 取消确认，不分配／提交 Close 请求                                          |
| 确切 `y`                 | 相同完整绘制来源；原子分配并排队一个操作，关闭确认框                       |
| 执行                     | 活 lease 和原 forward cutoff；新来源证明先于一次 Start                     |
| 确定未发出或确定准入拒绝 | 保留证据；不声称已接受关闭，也不自动重试变更                               |
| 已接受或观察到 Pending   | 每个可失败边界前保留关联回执；有界只读轮询同一操作                         |
| Closed                   | 保留已知 Closed，再读新 snapshot；既有 membership 清理可落在 Standalone    |
| Failed、观察不可用或超时 | 保留全部已知 member／child 事实；显示状态和原身份的可执行查询命令          |
| 传输丢失或 worker 退出   | 经 mailbox／join 交接保留回执或未知请求身份；不重放 Start，也不推断 Closed |
| 提交后 Client 退出       | 仅结束本地等待；Server 拥有的关闭可继续                                    |

原生等待按 50 ms 轮询，使用既有 50 秒总预算和其中 5 秒清理保留，并在原 forward
cutoff 停止；读操作不延长期限。超时是观察限制，不是取消 Server 保留操作。Pending
满 200 ms 后由注入的单调时钟启用受影响区域的本地化状态，不打开进度模态；最终回执可立即显示。仅实际 Closed 回执报告关闭成功。
Canonical 消失可修复选择，但不能把不可用观察提升为 Closed。其它存活 Workspace 的
Client 保留自己的选择；处于已移除 context 的 Client 沿用 peer-removal 契约。新 Close
不能覆盖尚未解决操作的恢复身份。

## 正确性与证据

所属实现测试覆盖绘制准确确认后才执行一次 Start、来源变化无效果、保留已知回执、Pending/Unknown 分类、规范清理、组关闭及保留检出路径。

在 `src/terminal_session/state/workspace_close.rs`、`src/terminal_session/mailbox/tests/close_reconciliation.rs`、`src/terminal_session/connection/native/tests/workspace_close.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。
