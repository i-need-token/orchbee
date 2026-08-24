# 终端正确性证据

本目录负责 `@orchbee/terminal` 中有关并发、独立排序工作和客户端输入修复的有界形式化证据。这些模型只抽象客户端拥有的投影终端状态；Terminal Host 及其 PTY/Ghostty 权威不在本目录范围内。

## 渲染器初始化退役

`tla/TerminalRendererInitialization.tla` 将首次 WebGPU 建立和恢复建模为依次进行的适配器、设备、管线、验证、准备与后端发布阶段。外部 Promise 挂起时，`freeze()` 与 `dispose()` 可以独立发生。每个完成栅栏要么推进仍存活的初始化，要么取消初始化且不发布 WebGPU 或 Canvas 后备所有权。

| 渲染器声明                       | 模型属性                                                       | TypeScript 细化                                                                 |
| -------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| 已退役的初始化不能发布 WebGPU    | `PublicationRequiresLive`                                      | `WebGPURenderer.initializeGPU` 在每次 await 后及获取/配置上下文前检查退役状态。 |
| 已退役的失败不能激活 Canvas 后备 | `FallbackRequiresLive`                                         | `activateFallback` 拒绝已冻结或已处置的渲染器，包括生命周期退役后才完成的失败。 |
| 已取消初始化不保留后端           | `CancelledOwnsNothing`、`OwnershipMatchesPhase`                | 退役栅栏释放部分创建的 GPU 资源、清除当前所有权并销毁取得的设备。               |
| 处置后不保留渲染器后端所有权     | `DisposedOwnsNothing`、`BackendMatchesPublication`             | `dispose()` 释放 GPU 资源、销毁当前设备并清除 WebGPU 与 Canvas 上下文引用。     |
| 冻结只保留已经发布的表面         | `FreezePreservesPresentedBackend`、`BackendMatchesPublication` | 已完成的渲染器保留最后绘制的画布；飞行中的初始化在冻结后只能取消。              |

模型把浏览器和 GPU Promise 的完成视为独立排序的环境动作。它只证明上述有界安全不变量；不声称适配器、设备或驱动操作最终完成，不声称物理 GPU 内存被同步回收，也不声称所有浏览器实现相同的上下文保留行为。

## 运行时呈现配置

`tla/TerminalPresentationConfiguration.tla` 对两个彼此独立完成的呈现配置请求建模。字体准备与提交资格是分离的状态：请求被取代或对象被处置时会撤销资格，但不会假定已经启动的异步工作随即停止。该过时工作以后仍可能完成或失败，但其操作只能得到已取消的最终结果，无法发布。只有最新、具备资格且已就绪的请求才能在一次提交中发布全部呈现字段。

| SU-14 声明             | 模型属性                                          | TypeScript 细化                                                                                         |
| ---------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| 过时的请求永远不会提交 | `NoStaleCommit`                                   | `ProjectedTerminal.updatePresentationConfiguration` 在字体准备之后和渲染器准备/提交之前检查其请求纪元。 |
| 配置发布完成           | `NoPartialPublication`                            | `WebGPURenderer.commitPresentationConfiguration` 交换一个完整的准备值。                                 |
| 已撤销工作无法发布     | `CancelledCannotPublish`、`EligibilityConsistent` | 被取代或处置的字体工作可能稍后才结束，但纪元与处置检查会在渲染器变更前返回类型化取消结果。              |
| 取消保留之前的配置     | `CancellationPreservesPublication`                | 过时的完成和处置会使提交的配置和所有已发布的字段保持不变。                                              |
| 取代保留先前的配置     | `SupersessionPreservesPublication`                | 启动较新的请求会撤销旧请求的提交资格，而不更改活动呈现。                                                |
| 准备失败保留先前的配置 | `FailurePreservesPublication`                     | 字体准备失败会在渲染器变更之前返回类型化失败。                                                          |
| 提交前不发生任何变更   | `PublicationChangesOnlyOnCommit`                  | 校验、字体准备、取消和渲染器准备均不改变已提交、已发布的呈现状态。                                      |
| 最新完成的请求提交     | `LatestReadyEventuallyResolves`                   | 使用公平提交调度程序，除非较新的请求或处置取消资格，否则就绪请求会提交。                                |

TLC 配置检查两个有序请求以及全部十个已发布字段的标识。字段值被抽象为请求纪元，因为具体值与测量由确定性校验和渲染器测试负责。模型让过时的字体工作保持在途，直至独立的完成或失败转换记录其取消结果。TLC 只对具备资格的提交采用弱公平性；字体准备完成、请求到达和处置仍各自独立排序，且不假定公平。

## 输入修复

`tla/TerminalInputRepair.tla` 对原生输入法组合生命周期、本地文本/预编辑发布资格、结算、瞬态输入呈现、按事件区分的单记录去重、修复后果与处置建模。结算守卫与普通关联记录彼此分离，因此修复可以清除关联记录，同时保留仍可能发出结束事件或组合来源 `beforeinput` 的原生工作。新的组合可在旧组合结束前取代已取消的工作；独立按键、焦点和空闲 `beforeinput` 的推进不等待旧组合结束。运行时使用浏览器可提供的事实（`isComposing`、`insertCompositionText` 和 `insertFromComposition`）阻止延迟到达的组合来源输入。浏览器事件不携带原生组合标识，因此新的 `compositionstart` 会取得下一个结束事件的所有权。

| IN-02 声明                                   | 模型属性                                                                                                                                                | TypeScript 细化                                                                          |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| 一次组合最多发布一次文本                     | `AtMostOneCompositionPublication`                                                                                                                       | 两种组合事件顺序都会在 `ProjectedTerminal` 调用同步端口前发布状态。                      |
| 每个主机输入尝试都带有其附件修订版本         | `HostInputAttemptUsesCurrentCapabilityRevision`, `CapabilityRevisionStampChangesOnlyOnInputAttempt`                                                     | `ProjectedTerminal` 使用构建时提供的正修订版来标记键、文本、粘贴、鼠标、焦点和修复命令。 |
| 每次发布都有明确的合格或空闲权限             | `CompositionPublicationRequiresEligibleAuthority`, `EveryPublicationHasAnExplicitOrigin`                                                                | 历史快照依据前一阶段和具名发布操作检查每次计数器递增。                                   |
| 不合格的更新无法发布预编辑                   | `IneligibleUpdateCannotPublishPreedit`                                                                                                                  | `terminalCompositionUpdateIsEligible` 控制每个渲染器预编辑更新。                         |
| 修复保留结算同时清除相关性                   | `RepairRetainsSettlementGuard`, `RepairClearsTransientInput`                                                                                            | 修复清除文本区域/预编辑和普通记录，但保留或创建单独的结算保护。                          |
| 重复数据删除记录由其配对事件消耗             | `DedupCorrelationIsSingleUse`                                                                                                                           | 显式的输入前键、输入前组合、输入结束前、粘贴和鼠标转换会消耗记录。                       |
| 无状态输入永远不会成为持有所有权             | `StatelessPublicationDoesNotChangeHeldOwnership`                                                                                                        | 合成编辑键和滚轮按钮报告绕过持久键和指针按钮分类帐。                                     |
| 仅在接受同步发送后才提交主机所有权           | `HostOwnershipChangesOnlyAtPublication`, `FailedSendPreservesPublishedOwnership`                                                                        | 主机发布的账本只有在同步`port.send`返回后才会改变； throw 报告失败而不提交该更改。       |
| 每个主机输入发布都等待焦点对齐               | `InputPublicationRequiresFocusConvergence`、`TextPublicationRequiresFocusConvergence`、`CurrentStagedContinuationHasAuthority`                          | 键、鼠标、无状态、合成和文本发布共享焦点协调命令路径。                                   |
| 失败的按下不能产生孤立的释放                 | `FailedPressRollsBackLocalOwnership`, `OrdinaryReleaseRequiresPublishedOwnership`                                                                       | 物理按键或指针按钮按下失败会回滚本地暂存和关联；释放未持有的输入时不执行任何操作。       |
| 修复释放按键，然后释放鼠标，然后释放可选焦点 | `CleanupReleaseRequiresPublishedOwnership`、`RepairConsequenceOrder`、`FocusOutRequiresPriorOwnership`、`FocusOutIsConditional`                         | 清理快照主机发布的所有权，并在每个事务中尝试一次每个有序结果。                           |
| 生命周期修复绕过停滞的审核                   | `LifecycleInvalidationDetachesQueuedGeneration`、`CancelledReviewDoesNotBlockCurrentGeneration`、`RepairEventuallyClosesAdmission`                      | 生命周期在清理前使调度器生成失效，且绝不等待粘贴审核结算。                               |
| 审核拒绝不能阻塞当前生成                     | `CancelledReviewResolutionIsInert`、`CancelledReviewDoesNotBlockCurrentGeneration`                                                                      | 当前 DOM 拒绝会被报告；失效的完成或拒绝无法发布粘贴或排队输入。                          |
| 重置会协调保留的本地焦点                     | `ResetNeverPublishesFocusOut`、`RepairReopenReconcilesFocus`                                                                                            | Reset 保留 DOM 焦点；若失效生成从未发布该焦点，则启动新的 focus-in 事务。                |
| 已失焦对象无需 focus-out 即可完成处置        | `UnfocusedDisposeRequiresNoFocusOut`、`DisposalEventuallyCompletes`                                                                                     | 完成 blur 后可直接处置，无需中间重新聚焦/恢复，也不会重复发送 focus-out。                |
| 修复/处置拒绝同步输入重入                    | `InputEventsRequireOpenAdmission`, `RepairAndDisposeCloseAdmissionFirst`                                                                                | `ProjectedTerminal` 在清除状态之前关闭其准入门，并仅在清除回调解除后重新打开它。         |
| 发送后的延续不能超过其权限                   | `CurrentStagedContinuationHasAuthority`、`RepairInvalidatesCurrentContinuation`、`CurrentContinuationRequiresAuthority`、`StaleContinuationOnlyCancels` | 适配器在同步发送后清除文本区域或预编辑之前检查修复纪元和分阶段合成状态。                 |
| 待处理的 focus-out 可跨越中间阶段            | `PendingFocusOutIsNotErased`、`PendingFocusOutEventuallySettles`                                                                                        | 组合结算不能打断释放顺序，也不能抹除必要的 focus-out 尝试。                              |
| 已处理的输入不接受稍后的事件                 | `DisposedAcceptsNothing`                                                                                                                                | 处置会在一次有序清理尝试之前关闭准入，然后删除侦听器和终端拥有的 DOM。                   |

`compositionPublicationCount` 刻意使用受检查域 `0..2`；因此“至多一次”是转换不变式，而不是 `TypeOK` 的推论。前一阶段、前一结算、前一发布、前一焦点、前一准入及前一已暂存延续的快照，使发布、保留结算、焦点所有权、准入和过时延续等不变式能够观察刚发生的转换。它们取代了那些可能因赋值而使属性在代数上不可达的错误标志。模型将一个抽象的发送前延续与其当前权限分离。修复开始时，当前延续变为过时；此后完成时只能丢弃。准入重新开放后新暂存的延续彼此独立。

`capabilityRevision` 从 `1` 开始，并由 `MaxCapabilityRevision` 限定上界；受检查配置允许降级到修订 `2`，再升级到修订 `3`。替换是推进修订的唯一转换，权限必须交替；普通、排队、组合、焦点、无状态或修复输入的每次尝试都会记录当前修订。降级会原子地使延续/审核工作失效，并清除本地及已发布的组合、文本区域、按键、鼠标和焦点所有权。只读状态不允许尝试 Host 输入。升级只改变权限与修订，因此降级前的工作无法重放。Host 侧变更、目标附件修复和过时输入拒绝仍由 `TerminalInputBackpressure.tla` 独立检查。

显式修复阶段对瞬态清除、按键释放、鼠标释放、条件式 focus-out 与重新开放准入建模。`localKeyPressed`、`localMousePressed` 和 `localFocused` 表示已准入的目标所有权；`keyPressed`、`mousePressed` 和 `focused` 表示已经发布给 Host 的所有权。`reviewStalled` 与 `queuedOwnershipInput` 属于当前调度器生成。生命周期失效会清除二者，只把单独建模的已取消审核保留为惰性工作，并从 Host 已发布的所有权开始清理。因此，排队的释放不能掩盖所需释放，排队的按下不能产生孤立释放，清理进度也不依赖审核完成或公平性。无状态发送对合成编辑键与滚轮报告建模，但不改变持有所有权。只有同步发送成功完成才更新 Host 已发布所有权。物理按下失败会回滚本地暂存及其关联；释放或 focus-out 失败则保留先前的 Host 所有权。即使清理发送失败，修复仍会推进，因为运行时在每个清理事务中对每项后果最多报告并尝试一次；模型不声称失败后仍完成交付或清除所有权。

`focused` 与 `needsFocusOut` 区分仍聚焦的修复和已经失焦的修复。处置可以从已完成的焦点修复阶段开始，无需先重新开放输入，这与 focus-out 回调中请求的同步处置一致。单独的待处置路径会在较早的释放回调中立即关闭准入，把任何必要的 focus-out 加入已持有的清理，并仅在清理后完成处置。若 reset 持有事务时焦点修复到达，reset 后果会变为条件式 focus-out；仅执行 reset 则保留焦点。DOM 焦点被保留、但失效生成未发布焦点时，重新开放 reset 修复会在新生成中启动焦点协调。弱公平性只适用于已经启动的同步发送完成与修复后果阶段、建立准入关闭、有序清理尝试推进、待处理 focus-out 结算及处置完成；它不假设发送成功、浏览器结算到达、已取消审核完成或过时延续运行。关联集合为每类来源抽象一个可替换记录。具体文本标识与严格的 99/100 毫秒边界由纯 TypeScript 测试负责。测试会严格解析并重放固定成熟跟踪中的 record/check 字段；该跟踪的版本见 [`../docs/input-repair-conformance-v0.0.1.json`](../docs/input-repair-conformance-v0.0.1.json)。有界面 `terminal-parity/IN-02-composition` 见证使用合成的 `CompositionEvent`、`KeyboardEvent` 和 `InputEvent` 实例驱动真实 `ProjectedTerminal` DOM 与 Canvas 适配器。它在应用层浏览器适配器检查组合修复、Shift 标点的原生文本优先及后备，并检查完整 Canvas 帧；它不运行原生操作系统 IME 候选窗口 UI。

## 行读取飞行协调

`tla/TerminalRowFlightCoordination.tla` 对独立排序的视口、搜索和通用缓冲区行读取通道建模。每个通道最多接纳一个请求，三个通道可以共存，按请求序列关联的响应只会结算其所属通道。中止只作用于搜索或通用缓冲区工作；处置会关闭所有通道。检查边界允许每个通道连续接纳两个请求，因此除首次接纳外也覆盖结算后的复用。

| 行读取飞行声明                 | 模型属性                                                            | TypeScript 细化                                                            |
| ------------------------------ | ------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 每种用途保持单飞行             | `AtMostOneFlightPerLane`、`AtMostThreeFlights`                      | `ProjectedTerminal` 分别保存一个视口、搜索和通用缓冲区飞行，总上限为三个。 |
| 独立排序的响应不能串到其他通道 | `InFlightSequencesAreDistinct`、`CrossLaneSettlementIsolation`      | `applyRows` 先按解码后的请求序列选择精确通道，再结算或应用结果。           |
| 中止不能取消其他用途           | `CrossLaneSettlementIsolation`、`SettledFlightHasExactlyOneOutcome` | 每个辅助中止监听器捕获其通道和请求序列；视口意图绝不会取消它。             |
| 处置与正常完成都会推进         | `DisposalClosesEveryLane`、`EveryAdmittedFlightEventuallySettles`   | 处置会结算辅助调用方并丢弃视口权限；否则由关联响应完成结算。               |

弱公平性只应用于已准入的关联响应。模型不假设 Host 响应期限、传输交付或三个通道之间的特定顺序；具体的过时生成、字节上限和渲染行为仍由 TypeScript 测试覆盖。

## 选择手势过时取消

`tla/TerminalSelectionGestureCoordination.tla` 对指针手势建模：其串行 Host 命令可能在稍后的更新、结束或取消仍未发送时变为过时。客户端拥有活动指针、捕获、自动滚动、一个在途手势命令及其有界待处理队列；Terminal Host 拥有选择内容、手势校验以及生成/修订权威。缺省值 `none` 表示没有命令占用被建模的通道。过时响应是预期的取消结果，其他拒绝仍是可见的类型化失败。

| 事件                 | 守卫                       | 客户端转换与可观察结果                                                     |
| -------------------- | -------------------------- | -------------------------------------------------------------------------- |
| 指针开始             | 没有先前手势工作           | 捕获指针并发布 `start`。                                                   |
| 指针移动、结束或取消 | 本地手势处于活动状态       | 立即发布或保留最新的有界未发送端点；结束和取消会终止本地活动。             |
| 当前 Host 接纳       | 生成和修订仍匹配           | 结算在途命令；只有接纳的 `end` 才能请求自动复制。                          |
| 生成或修订改变       | 命令可能仍在途             | 使该命令可以收到过时拒绝；生成改变还会终止 Host 手势状态。                 |
| 过时拒绝             | 串行手势在途命令已过时     | 终止完整本地手势、丢弃未发送端点、保留 Host 选择、不复制且不发出功能失败。 |
| 其他拒绝             | 串行手势在途命令仍为当前   | 保留现有可见失败行为，不自动重放。                                         |
| 新指针开始           | 过时取消后没有遗留手势工作 | 使用当前生成和修订开始新的独立手势。                                       |

| 选择手势声明             | 模型属性                                                           | TypeScript 细化                                                                                      |
| ------------------------ | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| 过时取消不留下旧手势工作 | `StaleBarrierClearsGestureWork`、`StaleBarrierRecordsCancellation` | `ProjectedTerminal.notifyBrokerFailure` 清除在途命令与队列、取消计划发送和自动滚动，并释放指针捕获。 |
| 过时取消保留 Host 选择   | `StaleBarrierPreservesSelection`                                   | 修复既不发送选择端点也不发送清除命令；最近一次 Host 发布的选择会继续投影。                           |
| 过时取消不会复制         | `StaleBarrierDoesNotCopy`                                          | 已终止的在途操作以后无法被解释为已接纳的 `end`。                                                     |
| 过时取消保持静默         | `StaleBarrierDoesNotSurfaceFailure`                                | 只有在途 `stale-generation` 或 `stale-revision` 选择失败会被吸收。                                   |
| 后续手势独立开始         | `RestartGesture` 与 `AcceptSelection` 的必需操作覆盖               | 确定性指针轨迹会取消过时工作，然后正常完成新手势。                                                   |
| 其他选择失败仍然可见     | `RejectSelection` 的必需操作覆盖                                   | 非过时选择失败的现有 `onError` 交付保持不变。                                                        |

TLC 检查一个当前指针手势、一个合并后的待处理端点、两个生成、两个修订，以及有界的发布、选择、失败和过时取消计数。具体队列边界与指针/捕获效果由确定性 `ProjectedTerminal` 测试检查。模型假设选择失败会结算当前串行手势命令，因为协议不会按请求序列关联选择手势。并发的程序化选择命令、传输期限、Host 文本语义、剪贴板交付与自动重试不在范围内；其现有契约保持不变。

## 原生输入结算

`tla/TerminalNativeInputSettlement.tla` 负责短暂的原生文本结算窗口，该窗口与 `TerminalInputRepair.tla` 中的输入法组合结算分开。模型探索两个连续获准的请求，区分未修饰与由 Shift 生成的可打印后备请求，并独立排列文本区域值变更、重复入队尝试、后备合并、input 事件延迟、计时器触发、组合开始/结束、修复/重新开放与处置。它还把后续 Host 定向输入表示为发布转换；当前原生请求得到最终结果之前，该转换保持禁用。

| 原生输入结算声明                   | 模型属性                                                                                                | TypeScript 细化                                                          |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| 每个获准请求恰有一个最终结果       | `EveryAdmittedRequestSettlesOnce`、`PublicationIsExactlyOnce`                                           | `#queueNativeInputChange` 持有一个待处理请求，并在计时器发布前将其清除。 |
| Shift 可打印候选项保留文本后备     | `ShiftedRequestsAreAdmittedFallbacks`、必需的 `QueueShiftedFallback` 覆盖                               | `shouldUseNativeTextInput` 把 Shift 标点和数字准入同一结算状态机。       |
| 已变化的原生文本优先于布局文本后备 | `NativePublicationRequiresChangedValue`、`FallbackPublicationRequiresUnchangedValue`                    | 计时器先把结算时的文本区域值与捕获的基线比较，再选择原生文本或后备文本。 |
| 重复的 keydown 只会丰富当前的请求  | `PendingIdentityIsCurrent`, `FallbackPublicationRequiresUnchangedValue`                                 | 当第一个非空回退被合并时，挂起的请求保留其基线和计时器。                 |
| 一个独立的输入事件等待定时器       | `DeferredInputDoesNotPublishEarly`                                                                      | 当独立文本插入有待处理的本机更改时，`#consumeInputValue` 返回。          |
| 稍后的主机定向输入不能超过结算     | `FollowingInputCannotOvertakeSettlement`                                                                | `#sendInput` 在发布后续命令之前同步解决待处理的本机请求。                |
| 组合、修复和处置会撤销计时器权限   | `CompositionOwnsNoSettlementTimer`、`ClosedAdmissionHasNoTimer`、`InvalidationCancelsPendingSettlement` | 组合开始/结束、瞬态修复和表面处置会调用 `#cancelNativeInputChange`。     |
| 未取消的计时器最终确定下来         | `PendingTimerEventuallySettles`                                                                         | 弱公平仅适用于已经布防的当前定时器；环境事件可能会先取消它。             |

模型通过事件顺序表示时间，从不假定慢速机器会恰好在 30 毫秒触发。它把具体 Unicode 字符串抽象为 textarea 生成，并把浏览器准入分区抽象为未修饰或由 Shift 生成的后备动作。确定性的 TypeScript 测试负责具体标点/数字、字母、空格以及 Control/Meta/Alt 守卫分区。操作系统输入法拥有最终 Unicode 映射。发送失败和焦点收敛仍由 `TerminalInputRepair.tla` 与 TypeScript 测试负责。独立的零延迟 `#nativeInputTextForNextKey` 标记也不在该模型内；单元测试覆盖标记所在浏览器任务窗口中的 input-before-keydown，因此没有关于这两个浏览器事件之间任意延迟的形式化声明。

从仓库根目录运行有界模型：

```sh
pnpm --filter @orchbee/terminal test:tla
```

共享仓库运行器让渲染器初始化、呈现配置、行读取飞行、选择手势和原生输入结算模型各执行一次，并让输入修复模型覆盖九个固定语义场景：组合、按键、鼠标、焦点、审核按键、审核鼠标、审核焦点、审核组合和生命周期。`review-composition` 组合停滞审核、排队焦点、reset 失效、刻意失败的首次新生成焦点协调、被阻止的组合发布尝试、成功的协调重试及后续组合发布。`InputPublicationReady` 只守卫 Host 定向发布；组合开始、更新、取消、结束结算和丢弃过时延续仍是本地调度器操作，不要求 Host 焦点收敛。

[`../../../scripts/tla/models.json`](../../../scripts/tla/models.json) 是执行清单。运行器以 SHA-256 固定 `tla2tools 1.7.4`，每次运行使用一个 worker、零指纹多项式、显式超时、Action 覆盖率，以及根目录下已忽略 `.tools` 中的唯一元数据目录。变体配置与模型副本仅为临时文件，并在 `finally` 中删除。每项完成的检查都会发出一条 `TLA_RESULT` 记录，其中包含模型/配置摘要、持续时间、生成状态数、不同状态数、剩余队列及深度。稳定的具名覆盖条目必须存在，并在已注册变体中具有非零计数；源码行号不是证据。可以把 `ORCHBEE_TLA2TOOLS_JAR` 指向已配置且摘要匹配的 jar，也可以让运行器配置仓库根目录下已忽略的副本。

清单有意记录边界和可执行覆盖范围，而不是过时的状态计数。当前计数属于生成它们的确切模型/配置摘要的 `TLA_RESULT` 输出。通过这些有界检查只是声明的抽象和公平性假设的证据；它不是浏览器调度、本机 IME 行为、传输交付或无限生产系统的证明。
