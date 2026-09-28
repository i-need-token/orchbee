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

`NativeClientPanelLifecycle` 强制：至多一个附着且始终等于选中叶子（`SingleAttachment`、
`AttachedIsSelected`）；选中是已知缓存叶子当且仅当存在缓存叶子（`SelectedIsKnownLeaf`、
`DesiredIsSelection`）；挂起的面板在途排斥所有其他结构变更与刷新，并冻结基线投影
（`StructuralExclusion`、`OtherStructuralExcludesPanel`、`PanelFlightFreezesCache`）；只有当前
操作的结果可被放置或消费（`OnlyCurrentResultPlaced`、`DepositCorrelatesCurrent`）；已提交修订
总是基线后继（`CommittedRevisionIsSuccessor`）且保留的提交绝不丢弃（`CommittedFactIsRetained`、
`RecoveryRetainsCommit`）；非法嫁接绝不应用（`InvalidGraftNeverApplies`）；无叶子的取代候选总是
经由成员关系修复，绝不分歧（`LeaflessRepairNotDivergence`、`LeaflessSupersedingRepairs`）；附着
新叶子要求旧叶子已释放（`AttachRequiresReleasedOld`、`PendingMembershipOwnsOld`、
`InstalledMembershipReleasedOld`）；成员关系调和指名挂起的面板请求
（`MembershipCorrelationNamesRequest`）；关闭回执在其发布安装前保持选中
（`CloseReceiptKeepsSelection`、`CloseAcceptKeepsSelection`）；提交至多一次且模糊结果后绝不重放
（`SubmissionIsAtMostOnce`、`NoReplayAfterAmbiguous`）；请求身份绝不回退
（`RequestCursorNeverRewinds`）；在过期与消费的弱公平下挂起阶段最终结算或退休
（`PendingEventuallySettlesOrRetires`）。驻留的附着不持有附着且保留有效提交
（`ParkedHasNoAttachment`、`ParkRetainsValidCommit`）；挂起的已拒绝/不可用应答绝不退休
（`AttachUnavailableNeverRetires`、`RejectedAttachNeverRetires`）且总会被消费
（`AttachReplyEventuallyConsumed`）；驻留的 Session 只能经由新一轮成员关系安装或退休离开——
绝不自动重新附着或重放（`ParkedSettlesViaMembership`）；被拒绝的过期输入保留阶段、
请求/序号关联、提交事实、附着、选中与缓存修订（`StaleRejectionPreservesState`）。这些主张仍是
单 pane 特化：它们管辖的面板在途仍作用于单个叶子，而围绕其外的活 pane 附着集合由下文管辖。

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

`NativeClientPaneMembership` 是集合的有界见证。在两个 pane、两个 anchor 与每 pane 两个
incarnation 上，它强制：活记录始终属于当前 anchor（`LiveRecordsShareCurrentAnchor`）；departing
记录不携带 pending draw、已 stage 或已 accept 的帧工作（`DepartingCarriesNoFrameWork`）；
已收未绘的帧只存在于活的已挂接已投影记录上（`PendingFrameIsLiveAttachedProjected`）；
accepted 帧总是该记录自身完成绘制的帧（`AcceptedFrameIsOwnStaged`）且只落在活的已挂接
已投影记录上（`AcceptedImpliesAttachedLiveProjected`）；host viewer 始终等于其记录的精确
incarnation（`ViewerIsExactLiveIncarnation`）并在共享预算内（`ViewerBudget`）；已交付
input 总是到达其标记的精确挂接 incarnation，且该命令入队时该 pane 持有焦点
（`InputsDeliverOnlyToTaggedLiveFocus`）。在处置与重现的弱公平下，离场 pane 最终结算
（`DepartedEventuallyDisposesOrReappears`）。

## 源码对应关系

| 生产接口                                                                                                                                                                 | 模型对应                                                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PanelMarker::barrier`、放行窗口（`terminal_session/state/panel.rs`）                                                                                                    | `SamplePanel`、`RefusePanelAdmission`、`RefusePanelAfterLockExpiry`、`RefuseMoveNoNeighbor`                                                                                      |
| `try_enqueue_panel_for_using` 放行与结构排斥                                                                                                                             | `AdmitPanel`、`StructuralExclusion`、`OtherStructuralExcludesPanel`                                                                                                              |
| 信箱在途排队/飞行阶段（`terminal_session/mailbox/panel.rs`）                                                                                                             | `stage = "queued"/"in-flight"`、`SubmitPanel`、`ExpirePanelFlight`                                                                                                               |
| `run_panel` 结果分类（明确的预提交否决/未变更、关闭回执、已提交、未知）                                                                                                  | `Receive*` 动作、`ReceiveAmbiguousOutcome`、`NoReplayAfterAmbiguous`                                                                                                             |
| `place_panel_result` 身份/序号/截止时间关联                                                                                                                              | `PlaceResult`、`OnlyCurrentResultPlaced`、`DepositCorrelatesCurrent`、`RejectStaleResultDeposit`                                                                                 |
| `panel_reply_current` 过期请求/序号/目标拒绝                                                                                                                             | 所有 `Consume*` 动作的 `ResultCurrent` 门控、`RejectStaleResultDeposit`、`RejectStaleMembershipReply`、`RejectStaleRefreshConsume`、`StaleRejectionPreservesState`               |
| `prepare_panel`、`prepare_panel_candidate` 精确与取代验证                                                                                                                | `ReadCommitSnapshot*`、`ConsumeCommittedMerge`、`ConsumeCommittedUnavailable`、`ConsumeCommittedDivergent`                                                                       |
| `prepare_panel` Tab 不可用快照分类（更旧/分歧、Tab 消失、更新的规范 Tab）                                                                                                | `ReadTabUnavailableSnapshot`、`ConsumeTabUnavailableMembership`/`Divergent`/`NotApplied`/`Refused`                                                                               |
| `validate_split_panel`、`validate_moved_panel` 精确 `r+1` 嫁接与放宽的取代                                                                                               | `ReceiveCommitted` 有效性、`ConsumeInvalidGraftCommit`、`ReadCommitSnapshotExact`/`Superseding`                                                                                  |
| `merge_projection` 要求先前选中叶子                                                                                                                                      | `ConsumeCommittedMerge` 与 `ConsumeCommittedLeaflessRepair` 的分划                                                                                                               |
| `apply_prepared_panel`、`retain_prepared_panel_membership` 提交保留                                                                                                      | `commitHeld`/`commitExpected`、`EnterMembership`、`RecoveryRetainsCommit`                                                                                                        |
| `stage_panel_membership_cleanup`、`prepare_panel_membership_install` 先释放后安装次序                                                                                    | `ReleaseOldAttachment`、`InstallMembership*`、`AttachRequiresReleasedOld`                                                                                                        |
| `retain_pending_panel`、`PanelProjectionUnavailable` 恢复                                                                                                                | `ConsumeCommittedUnavailable`、`stage = "recovery"`、`AdmitRefresh`、`ApplyRefresh*`                                                                                             |
| `apply_panel_event` UI 结算（含无附着与 Empty 安装）                                                                                                                     | `InstallMembershipAttach`、`InstallMembershipSelect`、`InstallMembershipEmpty`、`CompleteAttach`                                                                                 |
| `setup_target` 附着结果分类与 `settle_without_attachment`（`SessionPhaseKind::RetryableAttachUnavailable`）                                                              | `OfferAttachReply`、`CompleteAttach`、`ParkAttachUnavailable`、`FailAttach`、`ExpireAttach`、`ParkedHasNoAttachment`、`RejectedAttachNeverRetires`、`ParkedSettlesViaMembership` |
| 关闭 `Accepted` 回执与后续发布/移除                                                                                                                                      | `ReceiveCloseAccepted`、`ConsumeCloseAccepted`、`PeerCloseLeaf`、`CloseReceiptKeepsSelection`                                                                                    |
| `PaneKey`/`PaneRef`/`PaneRecord`、`sync_pane_set`、`depart_all_panes`、`drop_anchor_panes`、`settle_pane_detached`（`terminal_session/state.rs`、`state/membership.rs`） | `rec[p]`、`MintPane`、`VanishLeaf`、`ReappearLeaf`、`SettleDetachProof`、`AdvanceAnchorEpoch`、`LiveRecordsShareCurrentAnchor`、`DepartingCarriesNoFrameWork`                    |
| 信箱准入分划——input 在入队时绑定逻辑焦点，resize/poll 绑定期望集合中的精确 incarnation（`terminal_session/mailbox.rs` 的 `admits`）                                      | `Admitted`、`EnqueueFocusControl`、`EnqueuePaneResize`、`EnqueuePoll`、`EnqueueStaleCommand`、`DequeueAdmit`、`DropStaleQueueCommand`                                            |
| `run_input`/`run_resize`/`run_poll` 中的执行器精确 incarnation 栅栏（`terminal_session/connection/native.rs`）——dispatch 保留冻结的 `PaneRef`，绝不复查焦点              | `ExecuteControl` 交付门、`ExecutePoll` pending 存放、`ExpireInflight`                                                                                                            |
| poll 结果在绘制通道前的因果结算（`publish_poll_result_before`/`take_causal_before`，离场时 `CausalSettlement::NoFeedback`）                                              | `rec[p].pending` 存放、`PendingFrameIsLiveAttachedProjected`、离场清 `pending`                                                                                                   |
| 逐记录 `stage_drawn_frame`/`mark_feedback_accepted`/`invalidate_projection_frame`                                                                                        | `DrawPendingFrame`、`SettleFrameAck`、`AcceptedFrameIsOwnStaged`、`AcceptedImpliesAttachedLiveProjected`                                                                         |
| `settle_pane_fault` 已知拒绝驻留；`settle_attached_event`/`record_setup_configuration` 精确 `PaneRef` 结算                                                               | `SettlePaneFault`、`RejectStalePaneFault`、`RejectStaleDetachProof`、`RejectStaleFrameAck`                                                                                       |
| `plan_panel_focus_local` 同 Tab 焦点移动                                                                                                                                 | `MoveFocusLocal`                                                                                                                                                                 |
| 共享 `host_viewers` 容量准入（上限 8）                                                                                                                                   | `ViewerBudget`、`AttachPane` 容量门                                                                                                                                              |

模型抽象掉字节级传输、具体 OS/PTY 行为、渲染、worker 接合、真实终端身份分配以及移动方向/几何。
现有真实测试覆盖具体接缝：`terminal_session/connection/native/tests/panel.rs` 中的本地执行器
脚本化 carrier 运行、`local_client/tests/workbench/panel_split.rs`、`panel_move.rs` 与
`panel_close.rs` local-client workbench 适配器、
`terminal_session/workbench_projection/tests/panel_expectation.rs`、`panel_move.rs` 与
`panel_close.rs` 投影验证器，以及 `tests/cli_mutation.rs` 中的端到端 CLI 变更流程。

## 有穷模型证据

`NativeClientPanelLifecycle` 是有界设计见证，不是对无界系统的证明。它覆盖一个 Tab、三个叶子
`{p, q, n}`、每次运行一个面板操作、`MaxQueue = 2`、`MaxRequest = 4`、`MaxRevision = 4`、
`MaxTime = 5`、`Budget = 2`、`MaxPeer = 2`、`MaxRefresh = 2`。Tab 存活期间对端叶子移除不受限制，
且不假设其保留操作的源叶子——取代叶子反例按构造可达。公平性仅为弱公平：截止过期、结果存放、
结果消费、成员关系解决、附着应答的提供与解决以及刷新应用在持续启用时必须最终触发；当某阶段
合法地等待有界截止时间时不强制推进。

以固定的 TLA+ tools 1.7.4、每配置一个 worker 运行
`node scripts/tla/run-models.mjs --models native-client-panel-lifecycle --jobs 1`。下列五个配置
完成安全性与最终时序检查，队列清空，族内全部 71 个必需动作覆盖为正。

| 配置               | 有穷角色                                    | 生成 / 去重 / 深度 | 模型/配置 SHA-256 |
| ------------------ | ------------------------------------------- | ------------------ | ----------------- |
| `split-attached`   | 拆分，已附着，单叶 Tab                      | —                  | —                 |
| `move-attached`    | 移动，已附着，双叶（未变更/Tab 不可用路径） | —                  | —                 |
| `close-attached`   | 关闭，已附着，双叶（回执 + 发布）           | —                  | —                 |
| `split-unattached` | 拆分，无附着来源（直接安装路径）            | —                  | —                 |
| `move-singleton`   | 移动，已附着，单叶（无邻居拒绝）            | —                  | —                 |

`NativeClientPaneMembership` 是集合的有界设计见证，不是对无界系统的证明。它覆盖两个 pane
`{p0, p1}`、两个选中 anchor（一次代次切换）、每 pane 两个挂接 incarnation、`MaxQueue = 2`、
`MaxFrame = 2` 与 `MaxLog = 3`。对抗性入队命令携带严格已退或从未持有的
incarnation 标记——incarnation 只增不减，伪造标记永不可能撞上尚未铸造的附着——拒绝路径
因此是可达见证而非假定守卫。以固定的 TLA+ tools 1.7.4 运行
`node scripts/tla/run-models.mjs --models native-client-pane-membership --jobs 1`；登记条目
使用八个 TLC worker 与每配置 1800000ms 超时，每次运行的实际资源（堆、主机）记入该次
运行自身证据而非在此固定——这只是执行资源设置，不改变任何变量、守卫、界、公平子句或
受检性质，也不构成完成证据。两个配置
都必须完成且队列清空、族内全部 23 个注册动作覆盖为正。`viewer-contention` 配置将共享
viewer 预算收紧到 pane 数之下，使 `ViewerBudget` 作为真实约束而非空洞约束被演练。
ServerBusy 时序、逐步 lease 让步、截止值计算与非 `Detached` 结果分类不建模——这些主张由
确定性 reducer 与脚本化 carrier 测试承担。

| 配置                | 有穷角色                                             | 生成 / 去重 / 深度 | 模型/配置 SHA-256 |
| ------------------- | ---------------------------------------------------- | ------------------ | ----------------- |
| `default`           | 两 pane、两 anchor、两 incarnation，`MaxViewers = 2` | —                  | —                 |
| `viewer-contention` | 同界 `MaxViewers = 1`——真实共享预算争用              | —                  | —                 |

已交付 input 见证以摘要而非序列保存。模型曾携带 `appliedInputs`——记录每次已交付控制输入的
ghost 日志——其读者只有 `MaxLog` 长度界、`ExecuteControl` 内的 `Len < MaxLog` 追加守卫与逐条
`tag = delivered` 安全检查；条目的 pane 字段、次序与重复均无消费者。模型因此以
`(appliedCount, allTaggedValid)` 存储抽象
`alpha(X, L) = (X, Len(L), forall l in L: l.tag = l.delivered)`：`Init` 将空日志映射为 `(0, TRUE)`；其余 22 个不改日志的动作与
`alpha` 平凡交换；唯一写入者 `ExecuteControl` 的守卫改用 `appliedCount < MaxLog`，并把每次
交付以实际的 `inflight.incarnation = viewer[p]` 比较折入被赋值的合取——写作显式括号的
`allTaggedValid' = (allTaggedValid /\ (...))`，绝不赋常量 `TRUE`。括号是承重的而非风格
问题：相等比较的结合优先级高于 `/\`，不加括号的写法会被解析为
`(allTaggedValid' = allTaggedValid) /\ comparison`，使比较成为额外的使能守卫——比较为 FALSE
时整个动作不产生后继（并不会落入 `ELSE` 分支），错配交付因此永远不可能被记录。饱和路径
（`appliedCount = MaxLog`）仍消费命令但不记录，与序列版跳过追加完全一致，
两种表示在饱和后的盲区相同。公平性不变：每个弱公平守卫只读保留状态，且任何改变日志的步
已同时改变摘要，使能性与非 stuttering 在商下保持。`InputsDeliverOnlyToTaggedLiveFocus` 现在
恰为 `allTaggedValid`；其余不变量与 liveness 性质从不读日志。该对应在可达状态上是精确互
模拟而非单向安全过近似：每个摘要状态都有具体升点，且每个商步可由任一升点实现——守卫只
读 `X` 与 count。

控制命令的 kind 同样是商而非载荷模型。模型读 kind 的全部位置只问成员性（
`in ControlKinds`）、`"poll"` 相等或 `Idle` 哨兵——没有任何位置区分 `input` 与
`resize`，故模型只携带单一 `control` 标签且 `CommandKinds = {"control", "poll"}`。映射
`beta(input) = beta(resize) = control`、`beta(poll) = poll`、
`beta(idle) = idle` 保持
每个变量字段（pane、incarnation、`focusAdmitted`）、每个动作的使能、全部七项弱公平守卫
与非 stuttering 关系——没有一个动作只翻转子标签——且每个商步可升回 `input` 或 `resize`
的同名具体步。真实 Rust `Input` 与 `Resize` 命令仍完全独立；本模型不声称验证其各自
载荷、几何或字节编码。

三个 `RejectStale*` 动作保留原名与各自的 stale 存在量词守卫，但不再更新状态：每个写作
`IF (\E d : G(d)) THEN UNCHANGED X ELSE FALSE`，是真实守卫下的 stuttering 后继，
仍计入 generated coverage。模型曾携带以 `MaxReject` 为界的 `rejections` 计数；其读者
只有三个 `RejectStale*` 守卫与 `ExecutePoll` 的失败分支——这是验证专用的 stale 见证
拒绝计数，并非产品具有的界。投影 `alpha(X, r) = X` 将其移除。提升方向上，新 trace 映射
回旧 trace：`Init` 之后先发射至多 `MaxReject` 次 ghost 拒绝使计数饱和；此后新模型的
`RejectStale*` stutter 对应旧 spec `[][Next]_vars` 始终允许的 stuttering 步——
`UNCHANGED vars` 自环，包括旧 `RejectStale*` 动作已被饱和计数禁用的状态。新的失败
poll 对应旧的
`ExpireInflight` 步（X 更新——清空 `inflight`、保持 `frameCursor` 与 `rec` 不变——完全
相同），成功 poll 与其余全部 X 动作原样。公平性未加强：`ExpireInflight` 本为弱公平，
持续非空的 `inflight` 仍会被清空，`Idle` 槽打断弱公平推理所依赖的连续使能链。九个业务
不变量与 liveness 性质只读 `X` 且不变；`TypeOK` 保留除被移除 ghost 计数分量外的全部
合取；模型不再声称产品所没有的拒绝次数上限。这是
可达状态上的双向 fair-trace 投影，不是任意提升或一步强互模拟。

生成的 Rust 状态模型（`terminal_session/state/tests/generated.rs`）对真实 reducer 演练同一
契约：其 `ReincarnationFence` 对抗类为一个逻辑 pane 铸造第二个 incarnation，并验证携带已退
附着标记的工作在精确 `PaneRef` 处被拒；`state/tests.rs` 中的
`reincarnated_pane_fences_late_work_from_the_retired_attachment_incarnation` 与
`same_anchor_panes_fence_cross_pane_and_fabricated_incarnation_refs` 直接对生产结算 API 固定
错 incarnation 与错 pane 负向对照。

### 负向对照

临时变异体（不进入仓库）确认所检查的性质确实承重：

- 从放置/消费中移除结果关联身份后，过期请求/序号的存放可被消费——
  `DepositCorrelatesCurrent`/`OnlyCurrentResultPlaced` 给出反例。
- 删除过期输入守卫使不匹配的存放或应答改写受保护状态时违反 `StaleRejectionPreservesState`。
- 将无叶子取代的成员关系修复替换为权威分歧——先前已发布的行为——在对端于 `r+2` 合法关闭
  选中叶子而 Tab 存活的场景下违反 `LeaflessSupersedingRepairs` 并给出反例。
- 未先释放旧附着即安装或附着修复后的成员关系违反 `AttachRequiresReleasedOld`。
- 将可重试驻留改写为退休违反 `RejectedAttachNeverRetires`；从驻留阶段重放或自动重新附着
  违反 `ParkedSettlesViaMembership`。
- 对已收未绘的 poll 帧直接确认——抽掉完整绘制门使 `SettleFrameAck` 从 `pending` 提交
  `accepted`——违反 `AcceptedFrameIsOwnStaged`：被确认的帧从未在该记录上经绘制 stage。
- 让 `ExecuteControl` 把一次交付记到兄弟 pane 的 viewer 上——错配投递变异体——是
  `InputsDeliverOnlyToTaggedLiveFocus` 的既定对照：对 `p0` 的首次合法交付即把
  `inflight.incarnation = viewer[p1]`（`1 = 0`）经带括号的合取把 `allTaggedValid` 折为 `FALSE`，同时
  count 照常自增且真实准入门保留。早先未加括号的表达式下该变异体不可能浮现违例：
  FALSE 比较充当额外守卫而产生无后继，而非写出 FALSE 标志。

上述结果仅确立上文描述的有穷面板操作接口。已知边界如实披露而非默默声称：`commitValid = FALSE`
与非规范候选是错误权威的防御性输入，不是正常的诚实对端轨迹；`SomeLeaf` 使用非确定性
`CHOOSE`，对选中钳制做过近似；快照可经由请求游标被重复读取，而生产只读取一次；关闭回执的
效果延迟到后续发布或对端步骤；`transport` 在抽象模型中没有生产者。成员关系向量验证、数值
截止精度、真实 carrier 行为与多操作流水线仍在该有界设计见证之外——它是所建模接口的证据，
不是对无界生产系统的证明。

另有两个被建模的输入是安全过近似，不是正常的 wire 历史。可达性分三层：抽象
`Receive*`/`Consume*` 动作出现在聚合见证中，只证明所建模接口能消费该形状；本地 adapter
只接受严格的按意图结果子集；而诚实 Server 发出的集合还要更小。`ReceiveTabUnavailable`
是 reducer/权威故障输入：诚实权威只在 `revision == expected == baseline` 且 Tab 在该基线
中本就缺失时才发出 `Rejected{TabUnavailable}`。此类请求所指的 Tab 在其自身基线中已缺失，
因此良构的 Client 规划无法生成它；若对端在该基线之后才移除 Tab，修订已经前进，Server 会
在检查 Tab 之前返回 `RevisionMismatch`。模型动作则可在任意 `serverRev` 下触发且不约束
`serverTab`。`ReceiveUnchangedForeign` 对 adapter 不可达——真实 adapter
校验 `Unchanged.revision == expected`，不一致时以无法归类的结果呈现，绝不作为结果
交付——而模型仍将其送入 reducer 的分歧裁决。两者都有意保留在模型中：它们演练生产分支
所编码的防御性裁决，且 71 个动作的聚合覆盖仅证明每个抽象动作存在见证，而非每个动作都能
由诚实 Server 到达。正常的已提交路径——包括经由对端合法关闭造成的取代叶子丢失——仍是
诚实历史，有界安全性质建立在这一保守包络之上。
