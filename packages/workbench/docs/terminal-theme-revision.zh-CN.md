# 终端主题修订

## 领域契约

一个工作台组合为其连接的 Terminal Host 纪元协调一个完整的运行时主题修订。该事务会更改主机拥有的终端颜色状态和匹配的客户端拥有的呈现，而不使任何一方对另一方具有权威性。

| 领域   | 合同                                                                                                                                               |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 仅限客户端的默认颜色覆盖可能与主机调色板、光标、投影像素、覆盖层和应用程序表面不一致。                                                             |
| 输入   | 工作台拥有的操作 ID、最后观察到的主机呈现版本以及一个完整的规范化呈现快照。                                                                        |
| 输出   | 一个已完成的修订、一个键入的过时/失败/取消结果或对当前已完成快照的有意无操作。                                                                     |
| 非目标 | 字体、填充、不透明度、光标形状/闪烁、VT 内容修订、保留的首选项布局、新的设置界面以及浏览器端 VT 或调色板权限。                                     |
| Owner  | Workbench 拥有 intent、supersession 与发布资格；Terminal Host 拥有 revision 分配与终端颜色提交；投影客户端（projected client）拥有准备与原子呈现。 |

`TerminalThemeSnapshot` 已完成：

- `terminal` 包含 `colorScheme`、默认前景/背景、可空光标 RGB 以及正好 256 个调色板 RGB 值。空光标具有明确的含义“使用终端默认前景”；它没有丢失数据。
- `overlays` 包含光标重音、选择前景/背景、搜索匹配前景/背景和活动搜索匹配前景/背景。
- `application` 为应用程序表面选择完整的 `light` 或 `dark` 设计令牌映射。设计标记对于映射的具体界面颜色仍然具有权威性。

所有 RGB 值都是无符号 24 位整数。请求准入后，完整的快照是不可变的。 `defaultColorOverride` 不是本次交易的授权或输入。

## 修订与操作标识

呈现版本 `0` 是一个主机纪元的启动基线，并且没有操作 ID。工作台请求提供唯一的操作 ID 和 `expectedRevision`；仅当该期望仍指定当前主机修订版时，主机才会序列化准入并分配下一个正修订版。revision 是提交屏障，因此仅更改覆盖或应用程序表面的事务仍然会推进主机修订，而不会产生终端颜色差异。较新的请求始终会取代活动操作，即使其所需的快照等于当前可见的快照：主机可能已经提交了较旧的、尚未呈现的操作，因此较新的意图必须通过其自己的修订来恢复主机和​​呈现对齐。

主机保留当前操作ID、修订版和完整的终端快照。当前操作和终端快照的精确重播将返回 `already-applied`，而不会发生其他突变。对不同数据重复使用操作 ID 是不可重试的冲突。基于通过比较和设置进行的一次修订竞争的两种不同操作：一种可能提交，另一种则变得过时。

## 转换模型

| 目前状态                        | 事件                     | 结果和命令                                                                               |
| ------------------------------- | ------------------------ | ---------------------------------------------------------------------------------------- |
| 无活动请求（no active request） | 请求更改的完整快照       | 不取代任何东西；准备客户端快照并请求针对上次观察到的主机修订进行主机提交。               |
| 请求活跃                        | 较新的请求               | 撤销旧操作的发表资格；保留最后呈现的快照。已经开始的工作可以落定（settle），但无法发布。 |
| 主机待定                        | 确切的主机提交           | 保留返回的Host状态；仅在相同的最新操作可供客户端使用后才发布。                           |
| 主机待定                        | 陈旧的结果               | 保留当前呈现，采用返回的主机状态，并允许对相同所需操作进行显式变基。                     |
| 主机待定                        | 已知的可重试预提交失败   | 保留两个权威（authorities）并允许针对相同的观察到的修订进行显式重试。                    |
| Host 待定                       | 结果未知                 | 保留当前呈现，并在重试前读取/协调 Host 状态；禁止盲目重放。                              |
| 客户端待定                      | 准备失败                 | 保留当前呈现。可重试失败可以再次准备同一不可变快照；客户端绝不回滚 Host 提交。           |
| Host 已提交且客户端已就绪       | 最新匹配操作             | 在 Host 分配的修订上原子发布终端像素、光标、覆盖层和应用表面。                           |
| 隐藏或暂停                      | 最新的匹配操作已准备就绪 | 保留它且不暴露部分呈现；恢复（resume）时只发布最新的合格完整 revision。                  |

过时、失败、取消、隐藏或暂停路径绝不会只写入 Host 或客户端字段集的一部分。当客户端准备（client preparation）尚未完成时，Host 提交可能领先于可见呈现；在确切的最新操作可发布前，先前完整呈现保持可见。

## 结果和重试

| 结果                   | 含义                                             | 重试规则                                                       |
| ---------------------- | ------------------------------------------------ | -------------------------------------------------------------- |
| `completed`            | 主机操作和完整的客户端呈现只需一次修订。         | 无需重试。                                                     |
| `cancelled/superseded` | 较新的工作台操作取消了发布资格。                 | 切勿重试旧意图。                                               |
| `stale`                | 在此比较和设置之前，另一个操作更改了主机修订版。 | 采用当前的主机状态，然后显式地重新设置仍然当前所需操作的基础。 |
| 已知的可重试失败       | 指定所有者未提交。                               | 只重试失败的所有者；保留已经准备或提交的对等方。               |
| 已知的不可重试的故障   | 候选者或操作身份无效或冲突。                     | 使用新的 ID 开始纠正操作。                                     |
| Host 结果未知          | 传输丢失导致无法判断 Host 是否已提交。           | 先协调保留的 Host 操作/修订/快照；再在本地完成或执行变基。     |

## 正确性证据

[`../formal/tla/TerminalThemeRevision.tla`](../formal/tla/TerminalThemeRevision.tla) 检查两个独立排序的操作、主机/客户端准备、主机比较和设置、精确重播、操作冲突、已知和未知故障、协调、重试、取代和隐藏/暂停呈现。两个抽象 snapshot token 区分同操作精确 replay 与不同数据 conflict，而不复制具体 RGB 载荷。

| SU-09 声明                                  | TLA+ 属性                                                                                                                                              | 确定性细化                                                                                                                                  |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Host 默认值、调色板与光标一起提交（commit） | `HostStateIdentityIsCoherent`, `HostCommitIsAtomic`, `HostChangesOnlyOnCommit`                                                                         | Terminal Host 保留回复，预检完整的物理更新，然后提交一项操作/修订；其声明映射的本机测试拒绝部分失败。                                       |
| 像素、覆盖层、光标和应用程序表面一起发布    | `PresentationIsAtomic`, `PresentationChangesOnlyOnPresent`                                                                                             | 只有在主机/客户端完全达成一致后，Reducer 才会发布一份完整的快照； Web 屏障在更改应用程序令牌主题之前同步提交每个准备好的投影终端。          |
| 被取代/陈旧/失败的路径保留呈现              | `NonPresentationActionsPreservePresentation`, `SupersessionPreservesPresentation`                                                                      | 非最新匹配成功的操作相关事件不返回发布命令。                                                                                                |
| 精确 replay 幂等；不同数据复用 ID 会冲突    | `HostReplayIsExactAndIdempotent`, `HostConflictIsDifferentAndNonMutating`                                                                              | Host request 测试比较完整 terminal snapshot，并返回 already-applied 或不可重试 conflict。                                                   |
| 未知结果保留观察并先协调                    | `UnknownOutcomePreservesObservation`, `ReconciliationAdoptsActualHost`, `ReconciliationClassifiesHostIdentity`, `UnknownOutcomeRequiresReconciliation` | reducer 测试保留此前观察到的 Host 状态、采用 reconcile 结果，并按 operation 加 snapshot identity 选择 committed、stale/rebase 或 conflict。 |
| 持续 ready 的当前修订在可见时成功           | `EligibleReadyEventuallyResolves`                                                                                                                      | 以 `Present` 表示的成功发布分支为条件；类型化 presentation failure 与 retry 属于 reducer/controller 证据。                                  |

TLA+ `Present` 操作抽象出成功的 `publish-theme` 命令及随后的精确 `presentation-committed` 结果；
确定性 reducer 保持这两个 adapter 步骤与类型化 presentation-failure/retry 分支显式。弱公平性仅用于该
成功抽象内持续启用的 `Present`。Host 响应、客户端准备、协调、网络可用性或失败发布转为成功均不假设公平性。

检查边界是两个 operation、两个 Host revision 与两个不可变 snapshot token。模型把实际 Host 的
operation/revision/snapshot 与客户端最后观察到的 Host 状态分开。未知结果以非确定性方式表示提交前
丢失或 Host 已提交但回复丢失；只有 reconciliation 会把实际状态复制到观察。RGB 值与 256 项 palette
由 snapshot token 抽象，因为 schema 和确定性 contract 测试拥有其具体验证。Host 细化已实现并映射到 [Terminal Host 运行时合约](../../../apps/terminal-host/docs/runtime-model.md#correctness-invariants) 中的 `TH-018`。投影终端测试通过独立的分阶段打包存储、渲染器覆盖预检、精确的帧修订匹配、同步存储/渲染器提交和恢复最新行为来完善发布操作。 Web控制器测试涵盖多参与者应用屏障和故障保存。桌面 `terminal-parity/SU-09-theme` 提供有头（headed）运行时跟踪。有界的 TLA+ 模型和那些实现见证是互补的；两者都没有声称浏览器或 GPU 调度超出了其规定的边界。

## 实施细化

当前的确切终端协议注册 `ApplyTerminalTheme` 和 `TerminalThemeResult`，在恢复库存中携带完整的当前主机状态，并使用其捕获的主机主题修订标记每个 OBTF/OBPB 投影。 Terminal Host 在提交该修订之前以原子方式更新延迟的启动准备或启动的运行时。成功的仅语义主题提交也会使每个读取附件无效，因此即使终端内容没有发生更改，它也会收到修订标记的快照。

client reducer 把 `prepare-client-theme` 排在 `apply-host-theme` 之前。因此，渲染器预检失败会使主机和可见应用程序状态保持不变。主机成功后，`ProjectedTerminal` 将匹配帧确认到私有存储中，并保持先前​​存储、覆盖、光标和应用程序表面可见。它拒绝高于目标的分阶段修订，在隐藏、暂停或呈现保留历史记录时等待，并且仅当实时呈现可以恢复时才使确切目标符合条件。 `canCommitThemePresentation` 为最终同步预检； `commitThemePresentation` 交换存储并准备好覆盖层/光标，无需等待。

Web 设置所有者等待每个已安装终端的准备提交。如果任何参与者失败、被取代或失去资格，它将中止所有准备好的句柄并保留先前的应用程序主题。否则，它会提交每个终端并在同一 JavaScript 任务中更改 `data-theme`。 Desktop Main 转发应用/协调命令，将完整恢复的主机主题保留在其生命周期引导程序中，并清楚地报告未知的传输结果，以便客户端在重试之前进行协调。失去应用程序屏障的主机提交不会回滚；其帧（frame）保持隔离，直到 replacement 或 disposal 恢复对齐。
