# 本地面板生命周期

## 已绑定和未绑定成员

规范 Panel 保留主成员及有序子成员，即使部分成员没有终端绑定。Server 运行时成员集合只包含
已绑定成员，并记录主成员身份；已绑定子成员不能替代未绑定主成员。注册和结构操作保留未绑定
成员。旧的单成员读取路径仍遵循原有严格契约。

Open 使用现有已绑定成员。仅包含未绑定成员的布局仍视为已占用，返回 terminal-runtime-unavailable，
不启动终端，也不退役权威。显式 Tab Create 或 Split 可以创建首个已绑定终端，并正常安装 standalone 引用。

Close 根据规范拓扑确定范围，而不是根据已绑定终端数量。Panel Close 保留未绑定兄弟面板。
完全未绑定的 Panel 或 Tab 使用保留的 Close 发布流程，无需 Worker 销毁；只在规范发布时移除。
范围冲突、修订额度、重放、发布恢复和跨位置移动栅栏仍然适用。

## 创建 Panel 终端

私有 `panel.create-terminal` 操作指定一个规范 Context／Tab／Panel。封闭的 `target` 可以是
`child { label }`，用于追加一个已绑定子项；或 `unbound { presentationId }`，用于启动指定的既有
主项／子项，并保留其 ID、标签及覆盖值。目标缺失或已经绑定时在启动前拒绝，不回退到主项；
子项容量为 16。操作不创建 Panel 或分割节点。

Server 通过类型化放置方式与分屏共享终端创建流程。分屏分配五个身份，子项创建分配一个，
绑定既有成员不分配新身份。操作同时要求导航与创建权限。修订、配置档案／启动参数／覆盖项、
Workspace 目录、待处理 Close 及请求账本检查先于规范状态和运行时的预备提交及派发。
待处理／最终回执只携带真实身份；预构造有界结果及结算凭证保留重放和未知结果语义。
首个 standalone 终端可以使用活动 Worker，无需伪造来源绑定。创建不改变 Client 成员选择，
也不发送输入。既有布局导出／应用及成员控件是预期消费方；私有载体支持本身不代表用户能力完成。

## 范围与归属

本地 Client 对每个被接受的操作最多放行一次结构性面板变更——拆分、移动或关闭选中叶子——经由本地
carrier 传输，并通过 UI 信箱结算其类型化结果。本规范管辖
[native Tab Session](native-tab-session.zh-CN.md) 的面板操作扩展，以及为每个可见叶子持有一条
附着 incarnation 的 pane 成员集合。维护时请与
`terminal_session/state/panel.rs`、`terminal_session/mailbox/panel.rs`、
`terminal_session/ui/panel.rs`、`terminal_session/state.rs` 与
`terminal_session/state/membership.rs` 中的 pane 记录与调和机制、
`terminal_session/workbench_projection.rs` 中的投影验证器、
`terminal_session/connection/native/panel.rs` 中的本地执行器以及 local-client workbench 面板
模块保持一致。

Client 绝不合并无法关联的面板结果。放行要求 Session 稳定、存在选中叶子、结构变更槽空闲且前向
截止时间未过期。面板操作在放行时一次性绑定其请求身份、附着叶子键、基线投影修订、期望附着与总
截止时间；此后不得被静默重编。

## 状态与迁移契约

| 迁移                | 守卫与结果                                                                                                                                                              |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 采样面板意图        | 稳定的 Session 在有界放行窗口内采样用户意图；采样本身不改变 Session 状态。                                                                                              |
| 拒绝面板放行        | 锁竞争超过截止、已排队/在途/待结算的结构变更、挂起的刷新或切换栅栏、信箱饱和、身份耗尽或无期望叶子都会拒绝放行并保留全部 Session 事实。无邻居的移动是明确的空操作拒绝。 |
| 放行面板操作        | 放行分配下一个请求身份与租约序号，将基线修订/面板集合与期望叶子冻结进操作记录，并在提交前进入排队阶段。                                                                 |
| 提交面板操作        | 排队操作在截止前最多提交一次；到达 carrier 的提交被恰好记录一次。                                                                                                       |
| 接收面板结果        | carrier 产生未提交、预提交否决、未变更、Tab 不可用、关闭已受理，或带提交后权威快照的已提交事实。                                                                        |
| 模糊结果退休        | 无法归类结果的已提交操作使 Session 以 unknown 退休；可能已产生效果的操作绝不自动重放。                                                                                  |
| 读取提交后快照      | 已提交的拆分/移动以新身份重读权威快照。精确的 `r+1` 候选必须匹配提交的嫁接；更晚的取代修订仅做规范验证。                                                                |
| 读取 Tab 不可用快照 | Tab 不可用结果重读权威：更旧修订或同修订矛盾判为分歧，Tab 缺失安装 Empty 成员关系，更新的规范 Tab 候选经由常规候选路径接受。                                            |
| 存放类型化结果      | worker 将结果绑定到操作请求、序号与截止时间存放；与当前操作身份不匹配的存放永不被放置。                                                                                 |
| 拒绝过期输入        | 携带不匹配请求、序号或代次的存放或成员关系/刷新应答被原地拒绝；它保留阶段、关联身份、提交事实、附着、选中与缓存修订。                                                   |
| 消费类型化结果      | UI 仅在请求、序号、基线修订与期望叶子仍与挂起操作匹配时消费已放置结果。                                                                                                 |
| 合并已提交投影      | 候选仍包含操作选中叶子的已提交结果按候选修订并入投影缓存；该叶子保持选中与附着。                                                                                        |
| 修复取代成员关系    | 已提交结果的更晚候选在 Tab 存活时移除了选中叶子——包括对端合法关闭——进入成员关系调和：绝不以分歧退休。                                                                   |
| 不可用时进入恢复    | 候选不可用的已提交结果驻留在面板恢复中；已提交事实保留到后续刷新应用或成员关系替换。                                                                                    |
| 非法权威分歧        | 非法嫁接、外来未变更移动、Tab 不可用矛盾或分歧候选使 Session 以权威分歧退休。                                                                                           |
| 暂存成员关系清理    | 成员关系调和先暂存释放精确旧附着；清理失败或截止过期以保留已提交事实的方式退休。                                                                                        |
| 安装修复后成员关系  | 仅在旧附着释放后方可安装新成员关系：Tab 消失时安装 Empty，否则选择修复后的叶子；可附着来源随后重新附着。                                                                |
| 提供附着应答        | 挂起的安装附着在成员关系截止前收到 carrier 应答——已附着、已拒绝、不可用或协议违例。                                                                                     |
| 驻留可重试附着      | 已拒绝或不可用的附着应答将 Session 驻留在稳定的可重试阶段：无查看器附着，保留的提交保持有效，操作绝不重放也不自行重新附着。协议应答与截止过期仍然退休。                 |
| 放行至多有界刷新    | 无结构在途时，有界刷新重读权威快照；面板恢复期间它是唯一能解决保留提交的途径。                                                                                          |
| 应用刷新结果        | 规范刷新执行合并、修复缺失的选中叶子或安装 Empty 成员关系；不可用读驻留，分歧读退休。成员关系挂起期间刷新结果被栅栏。                                                   |
| 退休并关闭 Session  | UI 退休冻结挂起记录并保留已提交事实；Exit 关闭 Session。                                                                                                                |

关闭 `Accepted` 仅是回执：它本身不移动选中或附着。选中仅在后续规范发布移除该叶子时变化——经由
合并、成员关系修复或 Empty 安装——且附着只能在旧附着释放后改变。

## 不变量与时序规则

单操作实现契约要求每个操作只有一个选中 attachment；请求/serial/基线关联精确；面板 flight 冻结基线期间保持结构排他；保留每个已知提交事实。无效 graft 不得应用。合法的后续叶移除应修复成员关系，不得虚构分歧。

安装替代者前先释放旧 attachment。拒绝或不可用的 attach 进入停放，不自动重放或重新 attach；无效协议回复或期限到达保留文档规定的退役边界。Close 接纳本身在发布前不改变选择。期限与受控完成应使待处理工作结算或退役。过期输入保留阶段、关联、提交、attachment、选择和缓存修订。state、mailbox、connection、projection 套件针对生产代码执行这些要求。

具体操作轨迹不声明 TLC 穷举进展。下方保留的 pane 集合核心独立检查共享身份和清理协议。

## Pane 成员集合

在一个选中 Tab 内，Session 为每个可见叶子持有一条 pane 记录，而非单个附着。逻辑 pane 是
`PaneKey {anchor, WorkbenchPanelId}`；其物理附着是 `PaneRef {pane, incarnation}`，在每次签发
该记录的挂接目标时铸造。`sync_pane_set` 在一个绝对截止时间内调和观测到的叶子集合：缺失的叶子
铸造记录，重新出现的当前 anchor departing 记录原地愈合，消失的叶子离场——departing 记录在
等待仅由自身 `PaneRef` 结算的处置证明（`settle_pane_detached`）期间清空投影与帧簿记。
anchor 级拆除（`drop_anchor_panes`）经由 `PreparedDesiredInstall` 原子替换整个代次，而同 Tab
焦点移动是本地的，绝不令记录离场或重铸。

信箱准入按命令作用域分划：input 仅在入队时其 pane 为逻辑焦点才放行；resize 与 poll 放行
期望集合中每条精确非 departing incarnation。状态所有者只为可见集合中已附着且持有控制权的 pane 请求尺寸：未缩放时为所有叶子，缩放时仅为焦点叶子。只读和隐藏 pane 不调整尺寸。被放行的命令保留其入队时捕获的 `PaneRef`——
dispatch 按该精确引用栅栏且不复查焦点，因此已接受的输入在焦点移动后仍落在原 pane，绝不
迁移到新焦点 pane。携带已退 incarnation 到达的命令或 worker 事件——重绑前已排队或
迟到结算——在精确 `PaneRef` 处被丢弃或拒绝，绝不重放到重新绑定的附着上。poll 结果作为
pending draw 等待，直到精确活记录上的一次完整绘制将其 stage；`FrameApplied` 只提交到
stage 该帧的记录本身——pane 已离场的 pending draw 以无反馈结算，绝不伪造确认——就绪
仅由焦点 pane 决定，后台 pane 独立挂接与渲染。共享 HostViewers 容量上限 8 对所有可见
pane 生效，只读 pane 绝不获取控制或 resize。

精确存活 pane 收到类型化 `TerminalRuntimeUnavailable` 应答时，在本地处置该 viewer。
对焦点 pane，仅结构所有者空闲的 `Ready` 且无刷新或仅有既存 publication 刷新时允许此行为；
setup、变更、显式刷新、已武装的 panel-close 确认或恢复仍保留既有退役边界。既有 `ViewerLost` 故障令精确选中项驻留，
不重新挂接。存活兄弟 pane 继续 poll，也可结算此前准入的 controlling resize。
该精确焦点故障仍在时，既有一秒刷新调度器重新武装 publication debt，包括过早的未变更或
拒绝读取之后。权威成员移除随后走既有兄弟清理与成员安装，选择存活叶或进入 `Empty`。
丢失时不重放终端请求，也不报告请求成功；其他拒绝原因和传输/协议故障保留既有处理。

## 源码对应关系

| 运行时所有者                                  | 证据义务                                                       |
| --------------------------------------------- | -------------------------------------------------------------- |
| `terminal_session/state/panel.rs` 与成员状态  | 接纳排他、准确结果关联、保留提交、过期输入保留、合法后续叶修复 |
| `terminal_session/mailbox/panel.rs`           | FIFO 交接、最多一次提交和准确的不明结果                        |
| `terminal_session/connection/native/panel.rs` | 实际结果校验、类型化 attach 失败及有界清理                     |
| `terminal_session/workbench_projection`       | 准确下一修订及后续发布校验、选择和 Empty 安装                  |
| `terminal_session/state/tests.rs` 及生成测试  | Pane incarnation 隔离、替换和错误 pane 负向控制                |

`PaneIncarnationFencing` 仅抽象最后一项身份/清理边界。实际几何、载荷、操作回执和完整面板生命周期组合由这些实现套件负责。

## 有穷模型证据

`PaneIncarnationFencing.tla/.cfg` 检查两个逻辑 pane、每个 pane 最多两个 incarnation、两个选择 anchor、一个排队命令和一个执行 flight。`Mint`/`Depart`/`Cleanup` 对应成员替换与退役；`Admit`/`Start`/`Complete` 保留捕获的 pane、incarnation 和 anchor；`Draw` 只消费当前帧。清理、启动和完成采用弱公平。模型检查帧/效果属于当前身份、迟到清理保留后继实例，以及离开中的成员最终收敛。

`terminal_session/state/tests/generated.rs` 用生产 reducer 执行 `ReincarnationFence`。`state/tests.rs` 的 `reincarnated_pane_fences_late_work_from_the_retired_attachment_incarnation` 与 `same_anchor_panes_fence_cross_pane_and_fabricated_incarnation_refs` 覆盖错误 incarnation 和错误 pane 的效果。Mailbox、connection、workbench-projection 测试负责面板操作结果、回执关联、成员修复、绘制确认和失败处理。这些详细组合由实现测试支撑，不属于 TLC 穷举声明。

维护中的变异控制删除完成身份检查，必须触发 `FramesAreCurrent` 或 `EffectsUseCurrentIncarnation`。运行 `node scripts/tla/run-models.mjs --models native-client-pane-membership` 及所属 Cargo 测试。
