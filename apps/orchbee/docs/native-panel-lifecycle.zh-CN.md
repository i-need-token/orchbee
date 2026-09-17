# 本地面板生命周期

## 范围与归属

本地 Client 对每个被接受的操作最多放行一次结构性面板变更——拆分、移动或关闭选中叶子——经由本地
carrier 传输，并通过 UI 信箱结算其类型化结果。本规范管辖
[native Tab Session](native-tab-session.zh-CN.md) 的面板操作扩展。维护时请与
`terminal_session/state/panel.rs`、`terminal_session/mailbox/panel.rs`、
`terminal_session/ui/panel.rs`、`terminal_session/workbench_projection.rs` 中的投影验证器、
`terminal_session/connection/native/panel.rs` 中的本地执行器以及 local-client workbench 面板
模块保持一致。

Client 绝不合并无法关联的面板结果。放行要求 Session 稳定、存在选中叶子、结构变更槽空闲且前向
截止时间未过期。面板操作在放行时一次性绑定其请求身份、附着叶子键、基线投影修订、期望附着与总
截止时间；此后不得被静默重编。

## 状态与迁移契约

| 迁移                     | 守卫与结果                                                                                                                                              |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 采样面板意图             | 稳定的 Session 在有界放行窗口内采样用户意图；采样本身不改变 Session 状态。                                                                               |
| 拒绝面板放行             | 锁竞争超过截止、已排队/在途/待结算的结构变更、挂起的刷新或切换栅栏、信箱饱和、身份耗尽或无期望叶子都会拒绝放行并保留全部 Session 事实。无邻居的移动是明确的空操作拒绝。 |
| 放行面板操作             | 放行分配下一个请求身份与租约序号，将基线修订/面板集合与期望叶子冻结进操作记录，并在提交前进入排队阶段。                                                    |
| 提交面板操作             | 排队操作在截止前最多提交一次；到达 carrier 的提交被恰好记录一次。                                                                                         |
| 接收面板结果             | carrier 产生未提交、预提交否决、未变更、Tab 不可用、关闭已受理，或带提交后权威快照的已提交事实。                                                           |
| 模糊结果退休             | 无法归类结果的已提交操作使 Session 以 unknown 退休；可能已产生效果的操作绝不自动重放。                                                                     |
| 读取提交后快照           | 已提交的拆分/移动以新身份重读权威快照。精确的 `r+1` 候选必须匹配提交的嫁接；更晚的取代修订仅做规范验证。                                                   |
| 存放类型化结果           | worker 将结果绑定到操作请求、序号与截止时间存放；与当前操作身份不匹配的存放永不被放置。                                                                     |
| 消费类型化结果           | UI 仅在请求、序号、基线修订与期望叶子仍与挂起操作匹配时消费已放置结果。                                                                                    |
| 合并已提交投影           | 候选仍包含操作选中叶子的已提交结果按候选修订并入投影缓存；该叶子保持选中与附着。                                                                           |
| 修复取代成员关系         | 已提交结果的更晚候选在 Tab 存活时移除了选中叶子——包括对端合法关闭——进入成员关系调和：绝不以分歧退休。                                                     |
| 不可用时进入恢复         | 候选不可用的已提交结果驻留在面板恢复中；已提交事实保留到后续刷新应用或成员关系替换。                                                                       |
| 非法权威分歧             | 非法嫁接、外来未变更移动、Tab 不可用矛盾或分歧候选使 Session 以权威分歧退休。                                                                              |
| 暂存成员关系清理         | 成员关系调和先暂存释放精确旧附着；清理失败或截止过期以保留已提交事实的方式退休。                                                                           |
| 安装修复后成员关系       | 仅在旧附着释放后方可安装新成员关系：Tab 消失时安装 Empty，否则选择修复后的叶子；可附着来源随后重新附着。                                                    |
| 放行至多有界刷新         | 无结构在途时，有界刷新重读权威快照；面板恢复期间它是唯一能解决保留提交的途径。                                                                             |
| 应用刷新结果             | 规范刷新执行合并、修复缺失的选中叶子或安装 Empty 成员关系；不可用读驻留，分歧读退休。成员关系挂起期间刷新结果被栅栏。                                       |
| 退休并关闭 Session       | UI 退休冻结挂起记录并保留已提交事实；Exit 关闭 Session。                                                                                                 |

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
（`PendingEventuallySettlesOrRetires`）。

## 源码对应关系

| 生产接口                                                                                              | 模型对应                                                                                                 |
| ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `PanelMarker::barrier`、放行窗口（`terminal_session/state/panel.rs`）                                    | `SamplePanel`、`RefusePanelAdmission`、`RefusePanelAfterLockExpiry`、`RefuseMoveNoNeighbor`                 |
| `try_enqueue_panel_for_using` 放行与结构排斥                                                            | `AdmitPanel`、`StructuralExclusion`、`OtherStructuralExcludesPanel`                                       |
| 信箱在途排队/飞行阶段（`terminal_session/mailbox/panel.rs`）                                             | `stage = "queued"/"in-flight"`、`SubmitPanel`、`ExpirePanelFlight`                                        |
| `run_panel` 结果分类（明确的预提交否决/未变更、关闭回执、已提交、未知）                                   | `Receive*` 动作、`ReceiveAmbiguousOutcome`、`NoReplayAfterAmbiguous`                                       |
| `place_panel_result` 身份/序号/截止时间关联                                                             | `PlaceResult`、`OnlyCurrentResultPlaced`、`DepositCorrelatesCurrent`、`RejectStaleResultDeposit`           |
| `panel_reply_current` 过期请求/序号/目标拒绝                                                            | 所有 `Consume*` 动作的 `ResultCurrent` 门控                                                               |
| `prepare_panel`、`prepare_panel_candidate` 精确与取代验证                                                | `ReadCommitSnapshot*`、`ConsumeCommittedMerge`、`ConsumeCommittedUnavailable`、`ConsumeCommittedDivergent`  |
| `validate_split_panel`、`validate_moved_panel` 精确 `r+1` 嫁接与放宽的取代                                | `ReceiveCommitted` 有效性、`ConsumeInvalidGraftCommit`、`ReadCommitSnapshotExact`/`Superseding`            |
| `merge_projection` 要求先前选中叶子                                                                     | `ConsumeCommittedMerge` 与 `ConsumeCommittedLeaflessRepair` 的分划                                        |
| `apply_prepared_panel`、`retain_prepared_panel_membership` 提交保留                                     | `commitHeld`/`commitExpected`、`EnterMembership`、`RecoveryRetainsCommit`                                  |
| `stage_panel_membership_cleanup`、`prepare_panel_membership_install` 先释放后安装次序                    | `ReleaseOldAttachment`、`InstallMembership*`、`AttachRequiresReleasedOld`                                  |
| `retain_pending_panel`、`PanelProjectionUnavailable` 恢复                                               | `ConsumeCommittedUnavailable`、`stage = "recovery"`、`AdmitRefresh`、`ApplyRefresh*`                       |
| `apply_panel_event` UI 结算（含无附着与 Empty 安装）                                                    | `InstallMembershipAttach`、`InstallMembershipSelect`、`InstallMembershipEmpty`、`CompleteAttach`           |
| 关闭 `Accepted` 回执与后续发布/移除                                                                     | `ReceiveCloseAccepted`、`ConsumeCloseAccepted`、`PeerCloseLeaf`、`CloseReceiptKeepsSelection`              |

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
结果消费、成员关系解决、附着解决与刷新应用在持续启用时必须最终触发；当某阶段合法地等待有界
截止时间时不强制推进。

以固定的 TLA+ tools 1.7.4、每配置一个 worker 运行
`node scripts/tla/run-models.mjs --models native-client-panel-lifecycle --jobs 1`。下列五个配置
完成安全性与最终时序检查，队列清空，族内全部 68 个必需动作覆盖为正。

| 配置               | 有穷角色                                        | 生成 / 去重 / 深度 | 模型/配置 SHA-256 |
| ------------------ | ----------------------------------------------- | ------------------ | ----------------- |
| `split-attached`   | 拆分，已附着，单叶 Tab                          | —                  | —                 |
| `move-attached`    | 移动，已附着，双叶（未变更/Tab 不可用路径）     | —                  | —                 |
| `close-attached`   | 关闭，已附着，双叶（回执 + 发布）               | —                  | —                 |
| `split-unattached` | 拆分，无附着来源（直接安装路径）                | —                  | —                 |
| `move-singleton`   | 移动，已附着，单叶（无邻居拒绝）                | —                  | —                 |

### 负向对照

临时变异体（不进入仓库）确认所检查的性质确实承重：

- 从放置/消费中移除结果关联身份后，过期请求/序号的存放可被消费——
  `DepositCorrelatesCurrent`/`OnlyCurrentResultPlaced` 给出反例。
- 将无叶子取代的成员关系修复替换为权威分歧——先前已发布的行为——在对端于 `r+2` 合法关闭
  选中叶子而 Tab 存活的场景下违反 `LeaflessSupersedingRepairs` 并给出反例。
- 未先释放旧附着即安装或附着修复后的成员关系违反 `AttachRequiresReleasedOld`。

上述结果仅确立上文描述的有穷面板操作接口——不涵盖成员关系向量验证、数值截止、真实 carrier
行为或多操作流水线。
