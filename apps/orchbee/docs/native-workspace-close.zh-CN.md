# 原生 Workspace 关闭

## 范围与用户结果

Windows TUI 通过 `Ctrl+B D`（`Shift+D`）仅关闭当前 Workspace，再按无修饰的 `Enter` 确认；
`Esc` 取消。完整确认框显示 Workspace 名称，说明其终端会对所有 Client 结束，同时保留
Git 分支和检出目录。Project 分组、共享目录及 worktree 关系均不会扩大关闭范围。
未分组的 Workspace 是有效目标。历史兼容工作区没有规范 Workspace ID；其 Tab 需逐个关闭。
该工作区的整体关闭入口禁用，并提供本地化说明。

Workspace 上下文菜单的删除针对所指向的 Workspace，不会先切换过去。确认、取消和完成均
保留无关的当前 Context、选中 pane 和终端 attachment；当前 shell 继续运行。关闭当前
Workspace 时使用规范移除：先选择首个存活的规范 Workspace，包括空 Workspace，再选非空历史
内容，否则显示显式的无工作区新建入口。主动关闭绝不重建 Workspace 或终端。从历史内容打开
Close 可以指定某个 Workspace 为目标。

可见的取消和关闭按钮支持鼠标左键点击。按下与释放必须仍指向同一个已完整绘制的确认；
在按钮外释放、修饰键或过期绘制均不能授权关闭。键盘与鼠标使用同一个提交门禁。Esc 与 Enter 提示以次级文字放在对应按钮内。
在外侧蒙层上匹配的普通左键点击复用精确的已绘制 X 目标取消，不会选中底层终端或向其发送输入。

显式 Project 全部关闭兼容入口针对确切 Project ID。确认框显示 Project 名称及
Workspace 数量，说明全部成员 Workspace 及其终端会对所有 Client 关闭。Server 在准入时
捕获 Project 成员。Project 记录保留，原本为空的 Project 也可以执行。解散 Project 是
独立的组织操作，保留 Workspace 和终端。

普通关闭使用 `WorkbenchAction::WorkspaceClose`；仅显式 Project 全部关闭使用 group
Start。两条路径都不删除 Git 数据，也不自动恢复失败尝试。通过
`workspace close-status --workspace <workspace-id>` 或
`project close-status --request <operation-id>` 查询保留状态。符合条件的恢复使用显式
CLI Resume；Pending 和 Unknown 仅允许观察。

## 侧栏条件准入

侧栏 Workspace 删除使用新增 `WorkspaceCloseIfIdle`，绝不是 Client 声称“空闲”后再发送普通关闭。
既有 Server 准入门禁内，权限、Site／revision、重放指纹和容量检查先于完整捕获成员及 primary／
child 绑定证明。只有明确从未创建或为空的终端范围才无提示准入。存在运行时、待定、不可用或未知
范围，不能因为 Client 未见附着 pane 或进程似乎退出就被当作空闲。接受时在释放门禁前安装同一
Close reservation／围栏，因此并发创建或分屏不能占用已接受范围。

Project 删除对全部捕获成员范围使用 `ProjectDissolveIfIdle`，保留所有 Workspace 和资源，绝不调用
全部关闭。明确且匹配来源修订号的 `confirmation-required` 作为无效果保留，并以新显示 token 打开
新的审核决策。警告说明成员与终端都会保留。只有完整成功绘制及显式普通 Enter 才可提交新的无条件
请求 ID。其他拒绝、过期、缺失、传输或未知证据绝不打开此升级或重试效果。普通显式确认的关闭及
Project 解散保留既有兼容语义与不同指纹。

Workspace 升级状态说明需要确认，不会在新决策下方显示普通拒绝／重试指令。这一明确无效果状态
没有已准入的 Close 操作 ID，也不显示 close-status 恢复命令。普通拒绝与未知结果保留各自独立的
反馈和恢复规则。

## 输入与归属

| 事实                                         | 所有者与校验                                                                                                                                                        |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session、Workspace 身份和可选 Project 元数据 | 已验证的 Workbench snapshot 投影为 `NativeContextTabs`；分组变化不改变 Context 身份                                                                                 |
| 确认                                         | `WorkspaceCloseState`：带类型的 Workspace/Context 或 Project 目标、包含来源 Context 和 desired pane 的冻结完整 snapshot、名称、Project 成员数量和递增 display token |
| 完整绘制与几何                               | 纯 renderer 返回 `ModalPresentation`；`tui_presentation` 与 `WorkspaceCloseState` 区分首次绘制阻塞和准入后的挂起，确切 shown-token 证明仍单独保留                   |
| 请求身份、队列、lease 和期限                 | Command mailbox；单个有界 Refresh flight 将关闭与结构工作串行化                                                                                                     |
| 捕获成员与终端生命周期                       | Server 保留范围、成员及 child 身份；Worker/Host 拥有处置                                                                                                            |
| 提交与观察证据                               | Carrier 在下次 read 或 UI wait 前保留带类型的准入及观察；mailbox 和已 join worker 的交接保留这些事实                                                                |
| 选择与 Viewer 清理                           | 既有 context-removal 与 membership 所有者消费规范 snapshot 和确切 attachment 身份                                                                                   |

既有确认 packet 所有者解码按键。带修饰键、`y`/`Y`、粘贴和尾随字节均不能确认，
也不能到达终端。缺失或不完整的绘制证据不能提交。首次完整呈现之前，经过评估的几何阻塞会
拒绝新提示，并保留本地化放大提示。成功呈现之后，相同阻塞会挂起既有提示并保留其 token、
冻结来源与焦点：`Enter` 和过期鼠标释放不生效，本地 `Esc` 仍可取消，同一 token 随后完整绘制
时恢复。缺失回执只撤销 shown 权限，不判定几何，也不丢弃提示。来源 revision、成员或选择
变化会使确认失效。指定 Workspace 必须仍指向已捕获的确切终端 Context；即使当前 Context 和
revision 未变化，目标消失或身份替换仍会使确认失效。交付前，新 snapshot 必须等于
冻结投影，包括后台成员；变化或不可用的输入拒绝提交。
该读取与 Server 准入仍是独立边界：Project 成员在 Start 时原子捕获，不声称存在跨越
此前 Client 读取的事务。

## 转移模型

`WorkspaceCloseOperation` 保留带类型目标、来源投影、请求身份和原始 `OperationDeadlines`。
其 Workspace 目标携带自身 Context 身份；提交或回执验证不会借用当前终端的身份。
`WorkspaceCloseEvidence` 区分未提交/拒绝、未知、Workspace 准入、已观察的 Workspace
状态和已观察的 Project 结果。即使首次状态读取尚未成功，普通准入也保留 Server 规范
`close_request_id`、context 和 accepted revision。Project 证据保留冻结成员顺序和
每个已知 child Close 身份。

| 状态/事件                         | 守卫与输出                                                                                    |
| --------------------------------- | --------------------------------------------------------------------------------------------- |
| 打开                              | 结构所有者空闲；冻结当前或指定 Workspace，或显式指定的 Project，不移动焦点                    |
| 绘制                              | 准入几何下内容完整；仅认可其 token                                                            |
| 首次绘制前几何阻塞                | 拒绝并移除新提示，不分配 Close 请求，也不报告为用户取消                                       |
| 成功绘制后几何阻塞                | 挂起同一 token、冻结来源与焦点；撤销提交和鼠标权限，直到同一 token 再次完整呈现               |
| 缺失或被覆盖的绘制回执            | 只撤销确切 shown 权限；保留 presentation phase 与冻结提示                                     |
| Esc 或来源过期                    | 取消，不分配或提交 Close 请求                                                                 |
| 无修饰 `Enter` 或有效关闭按钮点击 | 同一个已完整绘制的来源；原子分配并排队一个带类型操作                                          |
| 执行                              | 活 lease 和原始 forward cutoff；新来源证明先于一次普通命令或 Project Start                    |
| 确定未提交或准入拒绝              | 保留证据，不声称已准入，也不自动重试                                                          |
| Workspace 已接受或已观察 Pending  | 首次读取前保留准入；用只读请求轮询同一个规范操作                                              |
| Project 已接受或已观察 Pending    | 只读轮询前保留组回执和已知 child 身份                                                         |
| Closed                            | 读取规范 snapshot 前保留 Closed；保留存活的来源，来源被移除时使用精确清理和存活工作区选择顺序 |
| Failed、读取不可用或超时          | 保留准入和全部已知观察；显示该范围的可执行状态命令                                            |
| Worker 退出或传输丢失             | 经 mailbox/join 交接保留回执或未知提交身份；不重放效果，也不推断 Closed                       |
| 提交后 Client 退出                | 结束本地等待；Server 拥有的操作可以继续                                                       |

每次观察必须匹配其操作、Workspace/context 或 Project 范围。尝试代数、已知成员、
child 身份、处置和 Closed 结果不能回退。已观察到的 OutcomeUnknown 不能改变状态或尝试代数。
信息更少的 worker 结果不能抹去准入。
迟到完成不能替换新操作，也不能选择先前的 Context。关闭后台 Workspace 或其他 Project
时保留当前存活 Context 的焦点。Begin 和 finish 根据冻结 snapshot 验证目标；规范移除后
仍保留有效的 Closed 回执，不要求已被移除的目标继续存在。

确认和执行中的操作阻止成员变化。保留的未解决证据继续阻止新的结构效果，但已经结束的
flight 允许只读规范成员修复。即使最新 Close read 不可用，snapshot 也能证明 Context
已移除并修复选择；它不能将保留状态提升为 Closed。新 Close 不能替换未解决的恢复身份。

轮询间隔为 50 ms，使用既有 50 秒总预算及其中 5 秒清理保留。读取从不延长原始 cutoff；
超时限制观察，而不是取消 Server 工作。满 200 ms 后，注入的时钟启用本地化 pending
状态，不打开进度模态。最终回执可以立即显示。

## 正确性与证据

经过验证的移除安装会在自动落到的规范 Workspace 保留已知 Closed 提示。此私有显示凭据
通过现有移除许可绑定请求、刷新 generation 与精确落点，不授予导航或终端输入权限。
之后手动切换 Context 会使其失效，返回时也不会恢复。Pending 与 unknown 结果保留现有守卫。

状态、carrier 和 UI 测试覆盖单个 Workspace 范围及未分组成员、显式 Project 成员捕获与
空 Project、确切绘制确认、绑定 token 的鼠标控件、packet 尾部捕获、来源漂移、准入/read/publication 丢失、
单调证据、规范修复和焦点保留。后台关闭证据覆盖从 Workspace 和 Standalone 发起时的
取消与发布、保留 pane/终端绑定和 attachment 身份，以及不清理或重新连接来源。即使
revision 相同，目标 Context 缺失或被替换也会在提交前被拒绝；来源或过期目标的回执不能
覆盖有效准入证据。Framed peer 断言普通 Close 从不发送 group Start，且
两条路径都不重放变更。Snapshot fixture 保留 Project 元数据和共享检出目录。

在 `src/terminal_session/state/tests/workspace_close.rs`、
`src/terminal_session/connection/native/tests/workspace_close.rs`、
`src/terminal_session/ui/tests/workspace_close.rs` 和 local-client Close adapter 套件中维护
证据。渲染证据覆盖两种 locale、terminal-native 与 RGB 主题，以及 Standard、Compact、
Minimal 和被拒绝的几何。适用的仓库 gate 仍是权威；定向开发使用
`cargo test --locked -p orchbee --bin orchbee workspace_close` 和 Project 全部关闭 carrier 测试。
这些受控实现轨迹不证明全部交错、物理终端外观或未测试的平台。共享身份与提交协议仍由
已注册模型负责。
