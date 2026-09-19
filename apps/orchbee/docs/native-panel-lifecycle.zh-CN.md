# 本地面板生命周期

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

在首个 `Starting` 相位，叶子几何观测仍执行 `sync_pane_set`——记录、panel 索引、最新
观测几何与分歧的期限预算全部写入——但普通收敛工作延后：`schedule_pane_reconcile`
不入队 `ReconcilePanes`，观测也不放行普通 `Resize`，从而在 `reconcile_pending` 持续
可见欠账的同时保留启动 poll-key 分配器的队列空/lease 空守卫。Ready 安装后，下一次
普通 tick 或观测以同一 `PaneRef`、同一已铸期限与最新已记几何交付该延后工作——相同
membership 绝不续期该期限，`last_admitted_geometry` 仅在 `Resize` 实际入队成功时
记录。setup 自有的 Acquire/Resize/专用 Poll/FrameApplied/ReadyCandidate 全程保持
原序，`Starting` 之外的相位照原样放行普通工作。

信箱准入按命令作用域分划：input 与 resize 仅放行逻辑焦点 pane；poll 放行期望集合中每条精确
非 departing incarnation。携带已退 incarnation 到达的命令或 worker 事件——重绑前已排队或
迟到结算——在精确 `PaneRef` 处被丢弃或拒绝，绝不迁移到焦点 pane，也不重放到重新绑定的
附着上。`FrameApplied` 只提交到 stage 该帧的记录本身；就绪仅由焦点 pane 决定，后台 pane 独立
挂接与渲染。共享 HostViewers 容量上限 8 对所有可见 pane 生效，只读 pane 绝不获取控制或
resize。

在 `Attached` 与 `SetupConfigured` 之间，pane 记录没有已结算的控制器 mode。普通轮询轮换只
放行已挂接、已投影、非 departing 且 mode 已解析的记录——仍在配置中的 leaf 被排除在外，其
自身 setup 管线对它的轮询不受影响。本地焦点移动——无论按方向还是按直接面板身份规划——
适用同一边界：未配置的 leaf 在规划阶段即被拒绝，获准的移动安装该 leaf 自己被授予的
mode，绝不虚构 `ReadOnly`。`ReadOnly` 配置的 leaf 是一等的已结算 mode：它显示内容、加入
普通轮询轮换、获得焦点，但绝不获取输入控制或 resize。

会话全局 `ReadyCandidate` 的发布权属于拥有 Ready 相位的 setup：目标仍持焦点的 `Startup`
与操作 `Lease` setup。`Reconcile` 持有的 setup 是后台收敛——它只完成自身记录的
attach/configure/draw/feedback 链，然后经纯记录尾返回，即使其目标在 feedback 结算窗口内
获得焦点也不发布全局候选，因此已 `Ready` 的会话绝不被推入第二次 Ready。在该窗口之前、
之中或之后的焦点移动都不会改写已记录的 incarnation、controller 或 sequence，已接受的
输入仍路由到其原本的精确 `PaneRef`。

`NativeClientPaneMembership` 是集合的有界见证。在两个 pane、两个 anchor 与每 pane 两个
incarnation 上，它强制：活记录始终属于当前 anchor（`LiveRecordsShareCurrentAnchor`）；departing
记录不携带帧工作（`DepartingCarriesNoFrameWork`）；accepted 帧总是该记录自身 stage 的帧
（`AcceptedFrameIsOwnStaged`）且只落在活的已挂接已投影记录上
（`AcceptedImpliesAttachedLiveProjected`）；host viewer 始终等于其记录的精确 incarnation
（`ViewerIsExactLiveIncarnation`）并在共享预算内（`ViewerBudget`）；已交付 input 总是到达其
标记的精确挂接 incarnation 且该 pane 当时持有焦点（`InputsDeliverOnlyToTaggedLiveFocus`）。
在处置与重现的弱公平下，离场 pane 最终结算（`DepartedEventuallyDisposesOrReappears`）。

### 收敛到期后的显式刷新

未附着且标为 `ConvergenceExpired` 的 pane，在重复几何观测和自动 publication drain 下保持
驻留。用户刷新是独立且绑定 generation 的意图：仅获准请求并不产生另一轮挂接资格。只有该
flight 成功 `Append` 安装当前权威快照之后，才可为当前 anchor 中可附着、非 departing、
未附着且已到期的 pane 开启一个新的有界收敛 episode。内部 workspace 或 context 恢复不算
用户刷新。

refresh flight 将来源与 generation 一起保存。准入前拒绝不产生 flight；明确失败、退休、
过期或被替代的结果不能重开 episode。成功安装使用现有 `LocalOperationClock` 在应答事务中
的观测点铸造唯一预算。活 episode 绝不延长，重复结果不能再铸预算；再次到期后仍为终态，
直到新的已接受证据到达。其他故障、健康附着、控制授权、已排队输入、帧身份，以及等待精确
处置证明的 departing viewer 均保持不变。

丢失 viewer 时也清除该附着的已接受几何，包括后台 pane；无附着记录不能保留失效 viewer 的
已接受 resize。pane 身份、观测到的叶子几何与 `ViewerLost` 故障保留，不自动重新附着。

这个来源/generation/安装/到期转移组由确定性 state 与应答事务测试覆盖。
`NativeClientPaneMembership` 抽象掉时间和刷新原因，其有界模型结果不证明这项恢复策略。

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
| 信箱准入分划——input/resize 走逻辑焦点、poll 走精确 incarnation（`terminal_session/mailbox.rs` 的 `admits`）                                                              | `Admitted`、`EnqueueFocusControl`、`EnqueuePoll`、`EnqueueStaleCommand`、`DequeueAdmit`、`DropStaleQueueCommand`                                                                 |
| `run_input`/`run_resize`/`run_poll` 中的执行器精确 incarnation 栅栏（`terminal_session/connection/native.rs`）                                                           | `ExecuteControl` 交付门、`ExecutePoll` stage 门、`ExpireInflight`                                                                                                                |
| 逐记录 `stage_drawn_frame`/`mark_feedback_accepted`/`invalidate_projection_frame`                                                                                        | `SettleFrameAck`、`AcceptedFrameIsOwnStaged`、`AcceptedImpliesAttachedLiveProjected`                                                                                             |
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
`MaxFrame = 2`、`MaxReject = 4`、`MaxViewers = 2` 与 `MaxLog = 3`。对抗性入队命令与 worker
事件按构造可携带任意陈旧或交叉接线的 incarnation 标记，因此拒绝路径是可达见证而非假定守卫。
以固定的 TLA+ tools 1.7.4、单 worker 运行
`node scripts/tla/run-models.mjs --models native-client-pane-membership --jobs 1`；唯一的
`default` 配置必须完成且队列清空、全部 22 个注册动作覆盖为正。

| 配置      | 有穷角色                                     | 生成 / 去重 / 深度 | 模型/配置 SHA-256 |
| --------- | -------------------------------------------- | ------------------ | ----------------- |
| `default` | 两 pane、两 anchor、两 incarnation，对抗栅栏 | —                  | —                 |

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
