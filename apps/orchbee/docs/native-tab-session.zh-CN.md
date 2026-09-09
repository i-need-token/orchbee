# 原生 Tab 会话

状态：这是原生 Client Tab 会话已接受的实现与正确性契约。
原生 Move 运行时已有确定性集成及 Linux cohort 进程证据。这仍是未发布的本地开发链路；
原生 Close 已有下文限定的 Linux 进程证据；打包和生产恢复仍属于独立工作。

[原生上下文选择](native-context-selection.zh-CN.md) 拥有 Workspace 导航和当前上下文投影扩展。
其中的位置身份、Empty 选择及保留 provenance 规则扩展下文的独立启动与同上下文 Tab 工作流。
[原生 Workspace 设置](native-workspace-setup.zh-CN.md) 拥有分步创建和首目录输入，
复用该 Session 的结构准入、carrier 和成员结算。

## 已接受的 Close 细化与本地集成

[规范终端 Tab Close](../../orchbee-server/docs/terminal-tab-close.zh-CN.md) 拥有已接受的
Server/Worker 生命周期、线协议/状态/Resume 及原生移除契约。原生 `Ctrl+B x` 以活动上下文的 Tab
为目标，以两种语言确认执行将对所有已挂接 Client 结束，并要求独立生命周期授权。Closed
要求已知销毁、目标所有者清理及规范移除。最后移除留下可 Refresh/手动 `c` 的可用 Empty
Workbench，不替代 shell，也不退出 Client。

该记录规定活动/非活动移除、按旧序号选择存活成员、Empty Create 来源、已知 Close 保留，以及
精确/更晚 CreateSnapshot、Switch、Move 和 Refresh 结算。更晚 snapshot 可移除 ID，但须保留
存活不可变身份；精确 Create/Move 提交修订仍有更严格期望。共同成员实现和已登记的移除/屏障
模型细化这些规则。早期不含移除的模型切片仍是条件性证据；精确 UI 消费、清理及 worker
退出事实仍为必需。

## Close 控制与恢复契约

原生状态所有者保留 Close 已知事实，使其独立于终端选择和投影。
Linux 原生输入、确认绘制、Close/Read/Resume 执行及显式仅状态恢复已集成。Closed 成员安装现由
共同精确清理消费者完成，且仅在 exchange 与 reconciliation 引用均释放后修剪记录。协议适配器
校验线协议关联；原生所有者校验精确操作身份、不回退的尝试代数、已保留的释放/Closed 事实以及
UI flight 身份。下文的 Linux 多 Client Close 到 Empty 进程用例补充了这些检查。

已提交变更尚无确定结果时，因截止或显式停用等原因退休，必须保留本地提交不确定性及用于
状态恢复的精确身份。仅排队但未提交的取消，或确定的类型化 precommit 结果，不虚构提交
不确定性。每次 join 的 worker 都必须有该精确 carrier 的返回证据；全新恢复 carrier
不能借用旧 worker 已完成的返回事实。停用保留已观察到的当前 worker 返回事实。

原生停用完成后，会使已显示会话失效一次，即使是在下一次 UI tick 之前，通过未变化的几何
观察发现 mailbox 已关闭。期间若已成功绘制，也会消费该通知；未变化的停用 tick 不会持续
重绘。停用会同时清除 navigator 的临时 Refresh 显示和待处理代数。真实 Poll 不可用回归
覆盖 Ready 屏幕与已排队 Refresh 的屏幕、精确 worker join 和
下一次重绘。它证明停用可见性，不证明 peer 移除后的恢复或独立的多 Client Close 到 Empty
进程尾段。

| 边界               | 必需的原生转换                                                                                                                            |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 初始 Close         | 捕获精确选中的 Site/context/Tab 和规范基准。先渲染本地化的所有已连接 Client 警告，再启用确认；合并输入中的未显示确认不得提交。            |
| Resume             | 要求精确的当前已显示可恢复操作/尝试、全新且已渲染的确认，以及准入时重新校验。Unknown 永不可恢复。                                         |
| 取消或本地拒绝     | 保持选择、附件、已知事实、请求游标和 mailbox 位置。                                                                                       |
| 已接受命令         | 保留规范主操作和接受修订；别名可能确认更早的已接受基准。接受不代表移除。                                                                  |
| 命令或控制回复丢失 | 保留已提交身份和已知事实；停用结果不明的 carrier，不重放变更。结果不明提交后的早期 NotClosing 读取不能下定论，因为接受可能仍在等待。      |
| Read 观察          | Denied/unavailable/查询失败不能变为 Host outcome unknown。已知接受后出现 NotClosing、操作改变或矛盾的 Closed 修订属于分歧；保留已知事实。 |
| Closed 观察        | 在任何可失败的 Snapshot 或 UI 交接前保留精确目标、规范操作、尝试和不可变移除修订。导出成员关系所有者的精确目标/最低修订预期。             |

成功的终端绘制报告精确的不透明确认及状态 token。状态的语义消费不代表该状态已经显示。
每次 Resume，包括原始 carrier 上的 Resume，都要求精确当前确认及其可恢复状态已显示。
完整 Resume 模态提示覆盖普通状态行时，自身必须渲染该状态。缺少状态会撤销保留 Resume
确认的已显示资格，但保留意图；渲染器假设不能替代这一状态所有者检查。
每次成功绘制还报告缺席或不完整可见性：缺少完整确认会撤销其已显示资格，同时保留意图和
事实。稍后完整绘制可重新启用同一个当前意图。因此，宽窗口完整绘制后再于窄窗口裁剪绘制，
不能授权确认。绘制失败不产生成功回执，遵循现有失败/停用处理。观察到真实 viewport 或
geometry 改变时，在分派已读取的输入包前撤销确认和状态的显示资格；同一循环迭代中的
调整尺寸后确认不能使用旧绘制。shell 在分派前通过 `invalidate_close_display` 报告任何
外层 WindowDimensions 改变，即使侧栏改变使终端 geometry 恰好不变；`observe_geometry`
另外使真实终端 geometry 改变失效。外层及终端尺寸都相等时，不得每轮无条件撤销。
意图和事实保持保留，完整绘制后才能重新启用。确认提示拥有的输入在确认
或取消后仍捕获同一输入包的剩余字节，防止尾随字节进入 shell。包括 Enter 在内的未识别
提示输入都不能确认。

渲染器在最终模态内容矩形内，按 Unicode 单元格宽度及 ratatui 的实际换行行数测量清洗后的
目标、警告和操作。即使换行器会省略某个字符簇，比矩形更宽的字符簇仍代表内容不完整。
内容不完整时显示本地化的调整尺寸/取消提示，不发送确认 token。完整模态和退化提示都在
绘制终端单元格前隐藏旧终端光标。状态 token 仅描述最终帧中仍完整可见的精确文字；提示
替换、裁剪或后续覆盖不能取得资格。完整 Resume 模态提供自身的精确状态 token。

确定的 Close 或 Resume 拒绝显示“Close request was not applied” / “关闭请求未执行”。
存在保留的 Close 状态时，这条本地提示使用次级提示区域，并保留该状态。拒绝本身不授予
任何显示资格；后续 Resume 确认仍受正常的精确可见性检查约束。

真实会话 Close 交互语料将原始输入包送入 NativeUi、成功的 TestBackend 绘制、因果 mailbox
及脚本线协议对端。它覆盖两种语言的 Close 确认、合并输入中的未显示确认、取消及输入包
尾部捕获、不完整重绘、清洗后的 512 标量 Unicode 目标，以及同一循环中从 100 列缩为
70 列、但终端宽度仍为 68 列的调整尺寸。Resume 覆盖被裁剪和被提示替换的状态，以及完整
模态的两个 token。这些检查证明输入/绘制/准入边界；脚本对端不证明 Host 销毁或组合的
多 Client 进程见证。

最多保留 256 个以目标为键的 Close 记录。达到容量后，在请求或队列分配前以本地 Busy
拒绝新记录。未解决结果或 worker/UI 交接仍引用记录时，记录可长于当前成员关系存活。
只有在成员关系已消费且所有交接引用释放后，才能裁剪已结算且已移除的记录。单调性针对
所有者仍保留的事实，不声称允许裁剪后仍有无界登记表。在 worker 消费 A 的确认前结算 A
并准入 B，必须保留两个身份；迟到的 A 交接不能替换 B 的 flight。

已知自身接受后，禁止新的目标 Input、Resize 和普通 Poll，同时保留精确附件清理身份。
已经准入的因果工作如实结算；未发送的排队工作不能被确认为已交付。其他目标只能在精确
屏障结算后恢复可用。每次独立准入的命令、Read 或 Resume 捕获一个固定的 50 秒
transport/deposit 截止时间。更早的因果截止时间仍然更早。及时放置类型化结果后该次
exchange 预算结束；UI 语义消费没有超时。只有前一个精确 key 已结算，后续 Read 才可获得
新预算。Busy、进展和 UI 延迟都不能续期已准入的 exchange，也不能续期现有 Create/Switch
的 45 秒前向和 50 秒总预算。

一个全局轮询调度器服务于保留的 Closing 记录及下述有界 disposed 类。只有一个精确 Read
flight，实际准入之间至少间隔 250 ms；不创建逐记录计时器或线程。选择改变不能清除其他
目标的可寻址性。现有导航器 Refresh 意图还请求对保留的未结算记录进行一轮有界手动读取，
包括已移除目标。最多 256 个目标 ID 仅是将来意图：在各次准入前不分配请求、队列位置或
时钟。重复请求合并。在活动 carrier 上，该轮读取在现有 Snapshot Refresh 语义结算后
开始；各次读取在后续准入前释放 lease。已停放记录仍可通过现有导航器行选择；已移除目标
的观察在全局状态/提示区域用保留的目标标签显示。缺席目标的过时状态不能授权 Resume。

reason 为 target-cleanup-pending 或 preparing 且 resumable=false 的
DisposedAwaitingPublication 仍可自动观察。活动发布和代数耗尽后的停放可能共用此线协议
形状。按精确规范操作/尝试保留 disposed_auto_admissions，初始为零，上限为 32。
每次因该状态准入的自动 Read，与 mailbox 准入原子地消耗一个额度；后续 NotSubmitted、
Busy 或失败仍然计费。准入前拒绝不计费；首次揭示释放状态的 Read 不倒计费。
同代数状态变化、手动读取、选择、停用、join 和重连都不续额。只有严格更高且已校验的尝试
才可获得新额度。其他 disposed 原因或 resumable=true 要求手动观察；Closing 保持普通
自动策略。额度耗尽只产生本地化的自动观察已暂停提示。保持 Server 状态、代数、已知事实
和 Resume 资格；手动 Refresh 仍可用，但不补充额度。

正常轨迹为 Closing、disposed(preparing,false)、一次自动 Read，然后 Closed。在精确
类型化消费后，成员关系所有者校验修订不低于 Closed.revision 且目标缺席的 Snapshot，
结算精确旧清理，并安装存活项或 Empty。因此，成功的正常释放/发布无需手动操作。
有限观察额度、transport 或 Server continuation 耗尽时，不由此推出最终 Close 保证。

carrier 停用会停止自动调度。只有显式用户状态 Refresh 才能在精确旧 worker join 后重新
连接，并且仅使用保留的可信本地 endpoint/cohort 和 Site/target。不匹配时为 unavailable，
不接管身份。此仅状态阶段不得激活 Site、Open、Create、Start、Attach 或重放变更。
跨 join 和重连保留已知事实；永不复活旧 Viewer。协调/Attach 只能通过成员关系所有者已
校验的 Closed/Snapshot 交接在此阶段之外开始。重启持久化和 authority 改变恢复仍被排除。

在新 carrier 上成功的精确 Read 已语义结算并显示同一保留 authority/Site/target/operation
的最新可恢复停放状态后，全新用户 Resume 意图可离开仅状态恢复，进入一次显式控制。
新的确认必须实际渲染，准入时重新校验状态、代数和显示 token。Read 失败、Unknown、
NotClosing、旧 token 或未显示确认都不能开启此控制阶段。准入一次全新 Resume 请求和固定
exchange 预算；保留 Server 的生命周期 grant 校验。不重放旧变更，也不执行激活、Open、
Create、Start、Attach 或 Viewer 恢复。回复丢失会再次停用此 carrier 并保留新身份。
精确结果结算后回到状态观察；再次 Resume 需要最新可恢复状态和全新已绘制确认。
此转换不授予通用 Session 或 authority 改变恢复。

Linux 原生恢复实现在精确停用任务完成 join 后保留原本地 endpoint/cohort，以及 Workbench
请求、attachment/Refresh generation 与 command/UI-wait 游标。新 mailbox 复制这些分配器
位置，不复制旧 queue、flight、event 或 Viewer。新的显式状态 Refresh 创建一个新 carrier，只准入 Read 和另经确认的 Resume
exchange，绝不执行 bootstrap。旧停用附件继续不可用。它已完成结算的停用原因不能再次
停用成功的新状态观察；新 mailbox 的失败仍会停用新 carrier。结果不明提交之后的早期
NotClosing 会结算该次 Read，但保留原提交 exchange 与不可用观察。只有经过验证的规范
Available 状态能够消解该不确定性。外来目标或状态回复保留原身份并停用新 carrier，不采纳回复。

真实 Unix socket 测试覆盖两次连续变更回复丢失与两次独立恢复 join、Resume 前的新状态与
确认绘制、恢复到 Closed 而不执行附件 setup、早期 NotClosing 后显式 Read 消解不确定性，
以及外来目标拒绝。这些测试细化仅状态恢复，不证明独立的 Closed 成员交接、记录修剪、
进程终止或其他操作系统上的恢复。

首次建立 Available Closed 且完成语义结算的精确 Read 拥有不可变 Close 对账来源：原 flight
key、规范操作、尝试、Closed 修订以及捕获的 binding/附件关联。后续 Closed 观察不能替换
该来源。独立 Snapshot marker 将其与新请求 ID 和独立固定 50 秒传输/放置截止时间配对。
通用成员结果验证两个身份。Snapshot 失败保留未消费来源；原始缺失或不可用结果均不证明清理。

普通 live 来源在准入前 Full 或 Busy 时保留一个合并的初始 Snapshot 意图，不分配 ID 或
截止时间。成功准入才用掉这次初始机会；之后失败需要新的显式 `w`。仅状态 Closed 则需要
当前精确状态实际绘制与新的显式 `w`，在 Snapshot 准入前进入该来源的
MembershipRecoveryPending。Full 保留该窄阶段并要求另一次显式 `w`，不授予普通命令权限。
只有经过验证的候选、精确清理与原子成员安装才允许附件连接到幸存目标。预验证 Close 消费
token 保留在独占状态安装计划内部，与成员一起无失败提交。记录修剪等待移除、消费和所有
精确 worker/UI/交接引用释放。私有 Closed 附件证明绑定不可变目标 binding 与精确旧附件代；
一旦真实 Detach 已交接，只有其实际关联结果才能完成该次清理。

原生生产端与通用成员消费端现已通过现有有界命令队列和单一 executor 传递该 marker。
及时放置的 Snapshot 仍可在迟到 UI tick 上消费。真实旧 UI-wait 序号在任何变更闭包运行前
被判为 stale；把当前序号与外来关联拼接属于协议违例，不能安装成员或虚构 worker 确认。
放置或取走 UI 回复均不释放 worker 的 marker：只有精确 lease 完成或已结束 worker 的
join 才释放该引用。较早 Snapshot 的确认不能释放较晚重试已消费的记录。仅收到确认的
tick 在修剪移除保留的 Closed 状态时请求重绘。

`mailbox::tests::close_reconciliation` 与 `state::tests::tab_close::reconciliation` 覆盖 Full
准入、不可变来源、手动重试、旧/当前关联、精确清理证明及消费/修剪屏障，包括非活动目标
移除时保留幸存目标的 Ready 证明。`ui::membership_tests` 还检查仅收到确认时的重绘，以及
Snapshot 在零时刻放置、UI 在 75 秒消费，随后 Gone 在唯一新捕获的对账预算的 0、44.999、
45、49.999 和 50 秒发生的边界。
`tests::close_recovery::close_membership` 中三条 Unix socket 轨迹覆盖 live Close → Closed
Read → 全新 Snapshot → Gone → Empty、已绘制的仅状态 Closed 加新显式 `w` → Empty，以及
Snapshot Denied 后手动以全新 ID 重试 → Empty。这些见证不证明 peer 恢复、Server/Host
进程终止、幸存相邻目标附件、从 Empty 手动 Create 或其他操作系统。

有限控制证据为 NativeClientCloseControl 的 command/status 与 Resume/status
配置，以及针对两个合法目标的 NativeClientCloseHandoff。控制切片有一个目标和一个未完成
exchange、规范/别名操作代表、尝试 1..3、最多一次 Close、两次 Resume 控制和有界 Read
准入。命令配置允许两次 Read 且不允许 Resume；Resume 配置允许三次 Read 和两次 Resume。
两个配置都最多探索两次打开的确认意图和一次显式重连。
其符号 exchange 时间为 before/cutoff/transport-budget-ended；disposed 额度采用 0..2。交接切片有
两个合法目标、一个迟到 worker 确认、一个可能后续 flight、有界手动目标意图和一个 Read
slot；不包含状态正文、Resume、确认或时钟的乘积。两个切片都不建模 Host 清理、Tab 排列
或附件 setup。

必需的细化包括真实八槽因果 mailbox 存储、256 记录边界、32 次读取的 31/32/33 计数、
受检的代数/身份边界、跨失败和 join 的同代数不续额、两个 locale 的原始按键/绘制确认，
以及 A 已结算/B 已准入/A 迟到轨迹。模型公平性仅涵盖本地到期、锁释放、精确 UI 消费和
已结束 worker 的 join；不假设网络、生命周期、重连或 OS 成功公平性。组合 Linux 见证必须
由有授权的只读 Viewer 关闭 shell 及抵抗退出的后代，同时让另一终端存活，让两个 Client
看到移除，然后到达 Empty 并手动执行全新 Create。这些义务不授权手动抢焦点原生窗口测试。

### 控制与交接模型边界

控制模型跨未完成工作、停用和 join 保留精确当前确认/提交 key。无需保留更早已完成 key
集合：每次新准入严格推进游标，提交会离开 queued，且同一个 key 不能重新进入 queued。
规范操作知识与接受回执分离；接受回复丢失后的 available Read 可建立操作知识，但不能
虚构其接受修订。及时类型化放置映射到预算已结束的时钟类别，其中包含任意晚的 UI 消费。
它不启动另一个截止时间。更早因果时间保持独立。

| 原生义务                               | 有限断言                                                                                                                                                               | 必需的具体细化                                                                          |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Close/Resume 前实际当前绘制            | `ConfirmedAdmissionRequiresDraw`、`ResumePromptRequiresDisplayedStatus`、`GeometryChangeRevokesDrawBeforeInput`、`RecoveryResumeRequiresFreshReadAndDraw`              | 真实成功绘制回执、同包未显示确认、调整尺寸加确认的分派顺序，以及两个 locale。           |
| 一次精确变更提交及保留的规范事实       | `NoUnconfirmedMutation`、`NoMutationReplay`、`EachSubmissionIsNew`、`SameKeyNeverReentersQueue`、`OnlyCommandResponseEstablishesReceipt`、`KnownReceiptNeverRegresses` | 字节适配器关联、受检游标/代数边界、精确 mailbox key 及结果不明 worker 退出交接。        |
| 固定 exchange 预算及无时限语义消费     | `ExactResultFence`、`PendingUiIsUntimed`、`TransportAgeNeverRenewsForSameKey`、`EarlierDeadlineNeverRenews`、`PendingEventuallySettlesOrRetires`                       | 真实八槽因果预留、截止相等、锁竞争、迟到 UI 消费，以及未改变的 Create/Switch 时钟。     |
| 有界 disposed 观察且不虚构生命周期状态 | `QuotaChargeIsAdmission`、`SameAttemptDoesNotRenewQuota`、`HigherAttemptAloneRenewsQuota`、`ReadAdmissionConsumesInterval`                                             | 真实 31/32/33 计数和 250 ms 全局准入间隔；失败/手动/join 不续额。                       |
| 两个合法目标、保留地址和受保护裁剪     | `ACompletionCannotReplaceB`、`ReadCompletionPreservesOtherTarget`、`ManualPassIncludesRemovedUnsettledTargets`、`PruningRequiresAllFences`                             | A 已结算/B 已准入/A 迟到的 mailbox 和 join 轨迹；256 记录拒绝及成员关系已消费后的裁剪。 |

控制模型从已校验类型化适配器结果开始。其拒绝过时/外来事件的动作是防御性无操作或
carrier 停用；它们不建模第二个具体包存储，也不证明字节过滤。独立交接模型包含两个合法
保留目标、接受/释放/Closed 的事实等级、精确后续 flight 和旧 worker 引用。其手动意图及
一个轮询 slot 保持已移除目标可寻址，而不把状态正文、Resume 代数、确认或时钟与该双目标
图相乘。两个模型都不证明 Server/Host 清理、附件 setup 或最终 Close 成功。

`AbsentStatusDrawUnarmsResume` 独立于模态渲染检查缺少状态的情形。
`JoinedCarrierHasCurrentReturn`、`WorkerReturnHasCurrentSource` 和
`FreshCarrierCannotBorrowOldReturn` 在一次重连两侧把 join 绑定到真实当前 worker 身份；
停用保留已完成的当前 worker 证据。`SubmittedRetirementRetainsRecovery` 和
`NoInventedSubmissionUncertainty` 区分已交付但无确定结果的变更、仅排队的取消，以及确定
类型化结果。必需覆盖包括提交后截止和显式停用、第二个 carrier 返回前停用、拒绝旧 worker
完成，以及当前恢复 worker 返回后的 join。

2026-09-08 的登记三配置运行通过了安全性、时序检查及所有必需动作覆盖 marker，使用固定的
TLC 1.7.4、每配置一个 worker、`--jobs 1` 和未改变的 180 秒限制。
模块加实际配置的 SHA-256 如下：

| 配置                                         | 生成 / 独立 / 深度       | SHA-256                                                            |
| -------------------------------------------- | ------------------------ | ------------------------------------------------------------------ |
| `native-client-close-control/command-status` | 623,726 / 121,398 / 29   | `9dbfcc7b02378d2d41b7024eabf85ba8a9c6c22aa9523f51308ec3ab591639f5` |
| `native-client-close-control/resume-status`  | 3,215,267 / 574,031 / 40 | `41eaed7818966f5bf599e48b979b27468f7c9baa8c50bdb1fea5871af240fa59` |
| `native-client-close-handoff/default`        | 257,104 / 39,662 / 24    | `60fbfb6b8f4cec742c114ef6fc1de0e03fb1a2cce8b1b191655ab4428ed4e52e` |

```sh
node scripts/tla/run-models.mjs --models native-client-close-control,native-client-close-handoff --jobs 1
```

此有限模型本身不证明运行时或组合进程行为；这些声明依赖上文及下文独立的可执行证据。
更早超时或诊断运行不能替代本次登记运行。

## 规范成员实现边界

共享 membership 实现替代下文较早的 append-only 假设。纯校验要求 Site/context 相同、revision
不回退、同 revision 完全相等。更高 revision 可新增、移除、重排并重命名 Tab label；每个存活
Tab 保持 Panel/Presentation identity、Presentation label 和 Worker/terminal binding。
在该 Site/context 内，当前与候选的 Tab、Panel、Presentation 或 Terminal identity 只要有任一重叠，
就必须属于同一个完整不可变 member tuple。只修改 Tab ID 不能把身份重新分配伪装成移除加新增。
Worker ID 本身不是 member identity：一个 Worker 可以拥有多个 terminal。允许真正移除并使用全新
identity 新增。这里只比较相邻两个 snapshot，不维护所有历史已移除 identity 的 registry。
精确 Create 与 Move commit revision 保留原有严格期望。更晚 revision 可以缺少已提交目标，
由可选的已校验 target index 表达；绝不抹去其历史 commit。投影所有的 `ClosedTabExpectation`
检查精确 Site/context/Tab、最低 Closed revision 以及目标缺失；它只证明 membership。
该 expectation 已在 `workbench_projection/closed.rs` 实现，identity 字段私有，仅提供只读访问器。
其纯验证器复用规范 candidate 校验，再检查精确 scope、最低 revision 和目标缺失；测试拒绝错误
scope、旧 revision 和目标仍存在。Close state 所有者在接入消费者时另行提供保留的 attachment
identity 及清理证明。

Live Empty 保留已校验的规范 Site/context/revision 和空 Tab vector，没有 provenance entry、
active/desired key、viewer/controller、projection、frame 或 Ready 事实。Cold bootstrap 仍要求
非空投影。`MailboxMembership::AwaitingBootstrap` 与 `Live { desired: None }` 不同；Empty 允许
手动 Create 和 Refresh，以及 Close 所有、不需要 attachment 的 status 与 retained-record 观察。
它绝不自动创建替代 shell。

Mailbox 判别状态已独立于 state 安装实现：live Empty mailbox 保留 attachment cursor，允许
Refresh 准入与执行，拒绝旧 attachment 标签命令，也拒绝第二次 Bootstrap event。
AwaitingBootstrap 仍阻止普通准入与执行。确定性 mailbox 测试覆盖这些边界；state 所有者通过
下文的共同原子 Refresh 安装提供 live Empty 转移。

State 保存 `desired: Option<AttachmentKey>` 并公开可选引用。既有非空 setup、cleanup、frame、
feedback 和 mutation 路径显式要求其精确 key；option 缺失时绝不制造 key。既有非空 Client
测试继续作为此表示变化的回归边界。下文的成员安装已接入 Empty 的产生及可选 Refresh settlement。

第一个实际安装消费者是来自真实无 viewer 稳定来源的 Refresh，以及 Empty Refresh。
`state/membership.rs` 在回复 mutex 外准备规范 membership、存活 provenance 和 preferred-ID/
旧 ordinal/最后存活成员选择。在该 mutex 内，`mailbox/membership.rs` 准备可选 desired key
及 checked cursor；state 准备匹配的 cache/phase plan。两份 plan 独占借用各自 owner，直到
commit 或 drop。两次不可失败 commit 移动已准备值，中间没有校验、分配或 I/O。此入口不能
移除 attached target：它必须经过待接入的精确 cleanup continuation。非活动替换可保留相同
key 及 Ready/frame/geometry 事实。

实际 UI Refresh 回复可以安装 Empty，无 key 地结算相同 revision 的 Empty 刷新或 definite
Unavailable，然后以 cursor 后继和规范 binding 安装 peer 的真正全新成员。选择改变时在锁前
采样唯一的新 45s/50s budget；checked deadline 失败或达到 forward cutoff 会阻止安装。
选择不变时不采样新 attachment budget。及时 typed placement 可晚消费；一个 UI witness 在
75s 和 150s 后消费结果，未续期其 transport budget。该 witness 驱动真实 UI admission、
mailbox flight/result/reply 和 reducer 路径，检查 desired key/cursor 一致及无排队的
Open/Create/Start；它不执行返回的 peer Attach。

Plan 测试独立覆盖 drop、第二份 plan 拒绝、generation 耗尽、reservation 复用和 cold-empty
拒绝。带 attachment 的 Refresh 续接现在把精确结果 generation、旧 attachment 和已验证候选
暂存于 `ReconcilingMembership`。Cache、selection 和 attachment 事实保持冻结，基于 Ready
的准入停止。现有 executor 在存储的 total deadline 内向旧 viewer 发送精确 Detach；只有匹配的
Detached 响应能产生共同清理 event。UI 复核待处理结果和旧 key，准备两个 owner plan，随后
安装 Empty 或 checked fresh successor。放弃准备时，待处理候选仍由 state 持有；其有界副本
在回复 mutex 外准备。

实际 UI 测试覆盖精确与错误 generation 清理，以及 44.999s、45s、49.999s 和 50s 的消费。
从 forward cutoff 起，已知清理只允许退役，不能安装候选或发起新 Attach。Scripted carrier
证据执行 Snapshot、精确 Detach、Empty 结算或后继 Attach 直至真实 Ready；意外 viewer 响应
绝不安装。从 Empty 手动 Create 及其他 Create/Move/Switch/Close 来源消费者另有下文描述的
来源特定细化。本 Refresh 片不构造或消费 Close Gone proof，也不改变 Switch supersession。

Create 现在携带不可变的 `CreateOriginKind::{Ready, Empty}` 和真实的
`CreateAnchor::{Ready(ReadyAttachment), Empty}`。Empty 准入预留下一个 mailbox generation，
捕获当前规范 scope/revision 与 geometry，不虚构 attachment。Definite precommit refusal
恢复 Empty，不发送 Resize、Detach 或发布 Ready。Empty 来源的已知 commit 即使后续工作在
发布前退役，也保留精确 operation 和 provenance。State、实际 UI 及 scripted carrier witness
覆盖此来源/refusal 边界，以及预留 generation 已消耗后的下一次 peer 选择。两种来源的已提交
Create 成功路径都使用同一个共同 membership 消费者。

实际 UI Create Snapshot handler 现在于回复 mutex 外准备规范/commit 校验，在不可 Clone 的
准备结果中保留精确 baseline 与 committed operation。回复事务重新核对 baseline 和当前
operation，再暂存精确旧清理或安装 Empty 来源候选。Executor 在任何旧 Detach 前取得此
Snapshot。旧 `CreateOldDetached` 发布事件已移除，只有共同双 plan 安装能发布选择。
存活 C 复用预留代次；更晚缺席则选择 checked fresh fallback 或 Empty。Typed Snapshot
Unavailable 冻结旧 attachment/projection 事实且不保留 Ready 权限。显式 Refresh recovery
重新校验已保留 commit，并且只采样一次新的 45s/50s 对账预算；及时 typed placement 允许
晚 UI 消费，但不会续期已经完成的 transport 预算。

Scripted carrier 执行 Started 和不可挂接 Create 的候选校验及精确旧 Detach；执行 Empty
来源 Started 的 Snapshot 直至真实 Ready，Create 期间没有 Detach；并执行显式恢复的晚 UI
结算、精确清理与同一预留代次。实际 UI 测试覆盖预留 C、fresh 回退到旧 A、Empty，以及新准入、
refusal 和 pending commit 均保留先前已安装 receipt。Generated mailbox lifecycle fixture
同样使用 candidate-first 暂存和共同安装。这些是本地细化 witness，不证明 Close Gone 消费，
也不证明组合的双 Client Create 场景。

Candidate-first Create 续程只保留一个 `settled_create` receipt，表示最近一次成功安装
membership 的 Create。共同 state-plan commit 在规范安装的同一原子事务中替换此历史 receipt。
准入、入队、definite refusal 或已知但尚未安装的新 commit 都不能替换它：pending B 和 settled A
可以共存。旧 worker/UI reference 仍绑定各自精确 operation。已创建 Tab 后续缺席不会抹去历史
commit；此 receipt 不代表当前 attachment，也不能作为 retirement 形状的 anchor。这是单个
保留 receipt，不是全部 Create operation 的历史 registry。

单一 membership owner 准备已校验 candidate、重映射 provenance，并依次按 preferred ID、历史
ordinal、最后存活成员、Empty 选择目标。Empty origin 选择第一个存活成员。仅涉及非活动成员的
替换保留精确 attachment、projection、geometry 和 cursor。其他 candidate 在既有 deadline owner
约束下等待精确 attachment cleanup，并冻结旧状态。Snapshot 缺失绝不是 cleanup evidence。
只有 Close 能构造不透明、精确 generation 的 `DefinitiveClosedAttachment`；它不能覆盖已 handed-off
的 Detach。

现有 UI-reply mutex 拥有原子安装。不可 Clone 的 state plan 和预备 mailbox desired plan
（`Preserve`、`ReuseReserved`、checked `Fresh` 或 `Empty`）必须在任一 commit 前完成校验。
随后两个不会失败的 commit 安装规范 cache/provenance/selection 与可选 desired key/cursor，
期间不再校验、分配或执行 I/O。放弃任一 plan 均不改变两个 owner。新的 post-transport
reconciliation 在获取该 mutex 前采样一次 45s forward/50s total budget；Create/Switch 保留原
deadline。及时 typed placement 结束 transport budget；延迟 UI consume 不能更新存储的 budget。

本记录释放 pure projection、Empty、mailbox sum 和共享 prepare/stage/install 边界。重排后的
Create 与 Switch 执行仍依赖各自修正后的 owner model 和细化审查。Typed Snapshot-Unavailable
的 blanket effect fence 仅适用于 Create；Move 保持既有 Ready effects，Refresh 保持既有 typed
settlement。Switch 的 definite-Unavailable restoration 在 owner model 与 stable-anchor 证明
被接受前仍是显式保留决策。每个 consumer 集成时在对应 owner 旁添加实现证据；此边界本身不证明
完整 Close 交付。

## Close 模型范围与细化

第一片具体投影实现位于 `workbench_projection.rs`：规范刷新接受更高修订号的删除，
`ValidatedCreatedProjection` / `ValidatedMovedProjection` 保留已验证候选及可选目标索引。
在共同成员续接接入之前，现有 Create 消费者仍要求该索引存在。本片不声称完成 attachment
清理或 Empty 安装。四个确定性成员回归覆盖 0/1/255/256 成员边界的删除、后续 Create/Move
目标缺失、每种保留结构身份域，以及共享同一 Worker 的真正全新终端。生成标签历史现在允许
删除，新增身份独立于当前长度分配，并在 64 条、每条 38 步的历史中比较独立的分域身份映射。
其范围是相邻投影，而非全生命周期 ID 复用。

`NativeClientMembershipRemoval.tla/.cfg` 拥有跨 Refresh、关联 Create 投影、Move 结果/恢复、
Switch 清理及自身已知 Closed 的有限移除组合。五种入口按初始 Ready 可控制、Ready 只读、
无挂接选中及可重试状态划分为十九个配置；Create 仅从两种 Ready 模式开始，Refresh 另有 Empty。
此划分不改变转换或支持的初始状态并集。每个配置使用一个 TLC worker，限制为 180 秒，
不使用状态约束。

Tab 名称是明确的角色代表，不是 TLC 对称性归约。A 最初为活动项。Refresh、Move 及自身 Close
使用 `[A]`、`[A,B]`、`[B,A]`、`[A,B,C]`、`[B,A,C]` 和 `[B,C,A]`；Refresh 另有 `[]`。
Switch 使用活动 A、期望 B，覆盖两种双成员顺序及全部六种三成员顺序。其租约可以是旧
A/代次 0，或已挂接 B/代次 1；替换 setup 使用代次 2。Create 使用 `[A,B]` 和 `[B,A]`，
活动 A、新建 C。这些情况覆盖空/单成员及全部活动序号，不证明任意排列。

成员移除切片在保留 C 的已知提交时，把它已预留的代次记为 1。存活 C 保留 1；回退目标消耗
checked 后继 2。其他新选择消耗初始游标的 checked 后继，Empty 保留已消耗游标但没有 key。
精确清理后，安装把选中 Tab 与代次一起原子发布；Attach 使用该 key，不再分配一次。
单次恢复位于安装前，并保留预留代次。具体准入拒绝及 `u64` 耗尽需要身份/邮箱证据。

模型准入一次结构操作和一次显式恢复，key 为 1..2、代次为 0..2、一次成员移除发布，
以及初始 Empty Refresh 观察到的一次对端 Create。规范修订域为 1..4；不可信的相对前驱 0
触发低修订拒绝。同修订及精确 Create/Move 提交候选仍须完全一致。更晚移除保留历史提交事实
及存活成员来源。不可变元组/绑定/Presentation 标签验证抽象为有效/无效边界；具体值及
0..256 成员历史需要投影器证据。不建模所有对端已移除 ID 的保留注册表。本地选择/恢复遵循
已接受缓存；自身已知 Closed 另按保留的提交修订对目标设栅栏。

携带移除的候选在替换缓存或选择前等待精确历史挂接结算。`CompleteAlreadyGoneCleanup`
表示独立的、精确目标的明确清理观察；单纯快照缺席不能充当其实现证据。非活动移除保留
Viewer/controller/代次及历史 Ready。活动/期望项移除选择历史序号处的存活成员，否则末个
存活成员，否则 Empty。Empty Refresh 通过新 Attach/重试观察对端 Create，不发出原生
Create 或 Start。新 Ready 抽象为已完成绘制/反馈证明；完整尺寸、租约及首帧顺序仍归既有
Switch 模型和确定性适配器轨迹负责。

符号时间为 0..8。传输准入捕获自身截止时间；及时类型化放置结束此预算，UI 消费没有合成
超时。既有 Create/Switch 前向及总预算不变。新的传输后协调在暂存验证候选、开始旧清理前，
一次性捕获前向 `+2` 和最后清理 `+1`；它们表达已接受的前向 45s、总计 50s 顺序，
不是 50s 再加 5s。传输、前向及预留期到期均有明确待处理阶段/截止时间守卫。弱公平仅适用于
推进此有限时钟、已启用的到期动作及精确 UI 消费。所检查的进展主张为结算、显式不可用恢复
或退役；成功 Snapshot/Attach、成功 Close、OS 调度及实际耗时均没有公平性或计时定理。
已知 Close 跨原生 Client 工作线程退出和 UI 退役保留；单个抽象非零尝试不证明线协议代次。

持有存活 UI 栅栏时，防御性过时输入直接投影为空转。`StaleResultCannotSettleCurrentFlight`
性质声明这一细化边界，不是缓冲存储或传输过滤的独立证据。这些机制由
`NativeClientRefreshFence` 及具体字节/邮箱测试负责。仍允许重复过时观察。

`NativeClientCreateBarrier` 单独细化一次从 Ready 或类型化 Empty 开始的手动 Create，队列
容量为二、尺寸值为 0..1。准入捕获来源/尺寸；Empty 拒绝恢复 Empty，仅 Started 允许新挂接
setup。保留提交后，它会在任何旧查看器清理前验证并暂存精确或更晚的规范候选。Ready 随后要求
精确 Detach 结果或不透明的精确目标已知 Closed 证明，才能安装候选；Empty 不 Detach 虚构旧
挂接。模型包含一次更晚移除返回成员：保留的 Create 事实及代次 1 仍是历史事实，冻结序号以代次
2 选择存活后备成员，或留下 Empty。它保留全部原 Ready FIFO/排空/失败/清理动作，不证明重复
Create 重入、任意移除或 ProjectionUnavailable 恢复。这些移除/恢复组合仍归
`NativeClientMembershipRemoval` 所有。未改变的 RefreshFence 及 MoveBarrier 模型仍是各自计时/
FIFO/setup 行为的无移除条件切片；旧保留性质不证明移除。TabSwitch 单独检查下文所述的候选、
精确清理及 successor 安装边界。成员模型族显露这些清理/结果边界，不把所有内部队列和时钟
组合成一个乘积。

| 主张 / 模型边界                                      | 必需运行时细化及可执行证据                                                                                                        | 形式化性质                                                                                                                                                                                |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 精确/更晚修订、不可变存活成员及历史 Create/Move 结果 | 严格投影器 corpus，加独立按身份生成的 create/rename/move/remove 历史，含缩减/重放及 0/1/255/256 边界。                            | `SameRevisionIsExact`, `SurvivingFactsAreImmutable`, `HistoricalCommitNeverForgotten`, `KnownClosedAbsentAtCommitOrLater`                                                                 |
| 冻结旧租约、序号选择、不恢复缺席 ID                  | 状态及 NativeUi 确定性轨迹，覆盖每个屏障的非活动/活动/期望项移除、精确 gone 清理、歧义、选中但不可用及 Empty。                    | `CandidateWaitsForExactCleanup`, `NoReplacementSelectionBeforeCleanup`, `NeighborUsesHistoricalOrdinal`, `NoRemovedIdResurrection`, `InactiveRemovalPreservesExactAttachment`             |
| 已知 Closed/尝试、精确 UI 结果及退役交接             | 原生工作线程/邮箱/UI 轨迹，覆盖状态接收、快照失败、类型化放置、延迟消费、过时 key、清理和工作线程退出切点。                       | `KnownClosedNeverForgotten`, `WorkerExitPreservesKnownClosed`, `PendingResultFencesExecutor`, `StaleResultCannotSettleCurrentFlight`                                                      |
| Empty 手动 Create 及 Empty 对端 Create Refresh       | 捕获尺寸及类型化来源状态测试；模拟载体断言无虚构 Detach、无重复 Open/Create/Start，并按类型化终端结果挂接。                       | `EmptyHasNoAttachmentOrReady`, `NoAutomaticCreateOrStart`, `EmptyHasNoFabricatedReadyOrAttachment`, `EmptyOriginNeverDetachesImaginaryViewer`, `CapturedGeometryIsImmutableDuringBarrier` |
| 新挂接证明及集成可观察结果                           | 既有 setup 细化，加受控多 Client 进程轨迹，覆盖活动/非活动 Close、最后 Close → Empty → 手动 Create，及历史 Create/Move 目标移除。 | `AtMostOneViewerAndController`, `FreshSelectionUsesFreshKey`, `FreshReadyRequiresProof`, `CreatedReadyRequiresDrawAndFeedback`                                                            |

`TypeOK` 提供结构状态检查。`ExistingSetupBudgetNeverRenews`、
`ReconciliationEventuallySettles` 及 Create 的 `BarrierEventuallySettles` 映射到确定性时钟轨迹，
覆盖 45s/50s 边界、checked-add 拒绝、静默 I/O、清理预留期及延迟 UI 消费。其公平性假设
不证明外部完成成功。

这些工件先于依赖的原生运行时改动。其有界检查不证明运行时细化、字节/JSON 验证、真实 PTY
行为、重启接管、远程传输或完整 Close 交付；这些声明仍须由上方独立的可执行运行时及进程证据建立。

## 已接受的 Move 集成与身份规则

原生 Move 投影器、邮箱、适配器、状态、按键及 UI 消费者实现下方已接受规则。
确定性证据与完整 cohort 进程验收分开记录。[Server 归属及证据](../../orchbee-server/docs/workbench-authority.zh-CN.md#已接受的规范-tab-移动模型)
规定从引用导出的引导组合、纯提交和发布边界。

Move 仅以活动选中 Tab 为目标，从 Ready、SelectedWithoutAttachment 或
RetryableAttachUnavailable 准入，要求活动载体及精确缓存/选择身份。`Ctrl+B <` 和 `Ctrl+B >`
向前/后移动；在边界处高层操作提交 `last`/`first` 实现循环。线上 previous/next 从不循环。
Move 准入不分配挂接代次；活动目标在验证投影中仍存活时，结算保留查看器/控制器、终端投影及历史 Ready 证明。普通帧失效仍不为 Create
增加新鲜帧前提。

Tab ID 和 `AttachmentKey { generation, tab_id }` 是权威。索引由一份已接受缓存向量派生，
仅可在该向量冻结时保留。缓存替换按精确 ID 准备来源状态，为仅快照新增 ID 赋 BindingOnly，恰好
一次解析各阶段要求的活动/期望/已提交 ID，再原子安装缓存/来源状态并派生选中索引。仅经验证的
关联 Create 覆盖创建者来源状态。单独保留的序号不能决定身份、清理、反馈、重放、来源或渲染。
对于 `[A,B,C] -> [B,A,C]`，活动 A 从索引 0 移至 1，且
`[Started(A),Unavailable(B),BindingOnly(C)]` 变为
`[Unavailable(B),Started(A),BindingOnly(C)]`；挂接身份不变。

普通 Refresh/Move 安装仅限匹配的稳定阶段或精确显式恢复。Starting、Creating、
CreateCommittedNeedDetach、Switching 和 Retired 禁止普通替换。关联 CreateSnapshot 在暂存旧清理前
验证保留提交。显式 Create ProjectionUnavailable 恢复验证同样的事实，并为候选清理与安装采样
新的操作预算。Started 可在该预算内继续 setup；非 Started 安装时不执行 Attach。
冷 Bootstrap 拥有初始安装。这些例外绝不准入第二个结构在途操作，也不允许
普通 refresh 冒充 CreateSnapshot。

一个准入时的 `structural_mutation_pending` Move 在途操作涵盖排队标记、命令 I/O、SnapshotRead、
类型化事件及 UI 结算。在同一个邮箱/规范状态锁下，Create/Switch/Refresh/Move 已排队、活动、在途或
等待 UI 时拒绝 Move。一旦准入，在精确 UI 结算或退役前拒绝新结构标记；显式恢复继续阻挡依赖顺序的
操作。先行 Switch/Refresh 同样在 setup/cleanup/UI 结算期间冻结缓存。仅 FIFO 不够：不得在
Move(B, first) 后构造 Switch(B, old index 1)。Input/Resize/Poll/因果反馈保留现有代次/Tab ID
FIFO 规则；Move 等待语义结算时后续效果不得执行。`n`、`p` 和数字使用最新已应用顺序；
Switching 时 n/p 以期望 Tab ID 和冻结向量为基准。

使用现有注入单调时钟及单一请求游标。锁前采样并只计算一次 `sampled_at.checked_add(50s)`；
溢出无修改拒绝。非阻塞锁/重新采样规则在 `now >= deadline` 拒绝，包括等值及跨过期的竞争，
不延长截止时间。原子检查阶段、精确目标、队列容量、互斥及身份可用性，再捕获请求 ID、基线、
上下文、Tab、方向、截止时间和在途操作。任一初始时间/队列/目标/身份拒绝保留游标、标记、缓存、
挂接、选择、投影和导航器。不允许第二个游标。

| 转换                                    | 守卫及结果                                                                                          | 结构栅栏 / 效果                                                                                                   |
| --------------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| 排空后提交                              | 更早 FIFO 和因果反馈如实完成；截止前提交                                                            | 同一个 50 秒预算覆盖排空、Move 和验证 Snapshot；现有更早截止时间仍更早                                            |
| 明确提交前                              | action 相容的 Denied/Rejected/Unavailable，或已证明本地未提交过期；TabUnavailable 读取一次 Snapshot | 精确 UI 结算显示通知并清除在途操作；经验证的移除使用共用成员关系结算                                              |
| 无变化                                  | `Unchanged.revision = baseline`                                                                     | 无 Snapshot；精确 UI 结算清除在途操作，不改变身份/缓存                                                            |
| 已知提交                                | 精确上下文/Tab 且 `TabMoved.revision = checked(baseline + 1)`                                       | 保留提交；执行时从同一游标分配一个 Snapshot ID；耗尽即退役，不重放或回退游标                                      |
| Snapshot 可用                           | 修订至少已提交；在提交修订处为完整基线的精确预期置换；更高修订使用共享严格验证                      | 有界类型化事件；保留目标或进入共用清理；精确原子安装释放栅栏                                                      |
| 提交后 Snapshot 明确 denied/unavailable | Move 已知提交，规范投影不可用                                                                       | 进入 MoveProjectionUnavailable，保留旧历史缓存、先前稳定阶段、查看器/控制器/代次及 Ready 证明                     |
| 显式恢复                                | `Ctrl+B w` 准入一个 Refresh，使用其现有独立 50 秒传输/投递分界                                      | 至少已知提交修订，该修订处要求精确置换，否则允许后续历史；重映射并恢复先前阶段；再次明确 unavailable 保持恢复状态 |
| 损坏/歧义交换                           | 关联/种类/outcome 错误、不可能修订、损坏投影或可能移交后歧义                                        | 退役混合载体，不换新 ID 重放；已知提交仍已知                                                                      |
| Exit/退役                               | 精确现有载体生命周期                                                                                | 释放等待执行器；退役不是成功完成                                                                                  |

类型化投递自身必须早于 `now >= deadline`，使用锁外采样、非阻塞锁及精确事件键/截止时间再验证。
若不能投递安全完整的类型化事件，退役未完成传输在途操作。投递后传输预算结束：UI 语义结算无
人为墙钟上限，栅栏保持至精确消费或载体/Exit 退役。晚消费不能使已投递结果过期。UI 循环进展仍为
环境假设，不是新增公平性保证。Move 传输没有 Detach 预留、重置 Snapshot 预算或 Move 重试。
目标仍存活时无需挂接清理/setup；后续投影移除目标时则进入下述共用成员关系续程。

MoveProjectionUnavailable 在恢复成功前阻挡选择/Create/Move。目标身份未变，Input/Poll 及有控制权
Resize 可继续。恢复仅提交 Snapshot，绝不提交 Open、Create、Start 或 Move。目标存活时保留挂接；
经验证的移除使用共用精确清理及可选后继 setup。身份耗尽或损坏/歧义恢复退役。
新启动仍加入保留 Open 引用/outcome，再选当前快照 `tabs[0]`：将 B 移到引导 A 前后，新 Client
选择 B，已有 Client 保留活动 ID。失败/unavailable/unknown 锚结果在 Snapshot 前失败；Started
锚配不可挂接的新首项时，在该首项 AttachBinding 失败。不得回退选择或扫描。其他活动 Client 仅经
显式 `Ctrl+B w` 更新，无推送或终端切换。

Move 成员关系消费者已实现共用清理和安装细化。`NativeClientMoveBarrier` 继续负责无移除的传输/FIFO/
提交保留切片；已检查 `NativeClientMembershipRemoval` 的 move 起点负责后续活动目标移除、
精确清理、回退及 Empty。其确定性 UI/载体证据与最终原生组合集成分开记录。精确提交修订
仍要求完整预期置换；只有更高修订才可移除已移动目标。保留不可变存活身份及创建者来源，再按共用
历史序号选择邻居或 Empty。原始缺席绝不证明 Gone。

任一可失败成员关系续程前先保留精确已知 Move 提交。已挂接移除冻结旧缓存/key，直到共用所有者
结算精确清理；传输后仅有一次 45 秒 forward 加 5 秒清理预留预算。无查看器替换直接使用相同的
独占状态/邮箱安装计划。两计划原子安装缓存、来源、可选 desired key 及预验证 Move 完成事实；
失败或丢弃计划不改变成员关系和游标，已知提交仍保留。Fresh 替换使用共享代次游标；Empty 保留
游标。精确 Move 在途栅栏持续至清理结算。替换/Empty 安装后，Move 后排队的旧 key Input 和 Resize 在查询
控制器或分配序号前丢弃。先验证当前 lease，仅当实际挂接及 desired key 都不同于排队 key 时丢弃；
desired 推进后，更早的 pre-Switch Input 和 Resize 仍通过其拥有的旧控制器排空。Move 目标存活时同样继续
通过同一控制器发送。

明确 `TabUnavailable` 在原 Move 传输预算内请求一次 Snapshot，不虚构提交。除普通规范验证外，
其可信拒绝修订是该顺序 Snapshot 的下界，不提供置换预期。Available 成员关系使用共用续程。
类型化 Unavailable 保留普通拒绝通知和旧稳定核心，清除精确在途操作，并允许之后普通显式 Refresh；
不创建已提交恢复或自动重试。已知提交后的 Unavailable 则保留既有 MoveProjectionUnavailable
结构栅栏及普通 Ready 效果。显式恢复把原 Move 预期带入共用 Refresh 清理/安装，绝不重放 Move。
具体证据必须区分 Snapshot 修订低于/等于/高于保留的拒绝修订、无查看器安装、Empty/真实后继 Ready 前的
已挂接清理、排队过时 Input、精确回复关联、45/50 秒边界、晚类型化 UI 消费及保留 A 与更新 B。
拒绝修订下界细化可达顺序模型读取：`ReceiveMoveTabUnavailable` 在移除后发生，
`ReadAvailableSnapshot` 观察不递减 Server 修订。这不是单独检查的拒绝下界不变量；具体线上边界
测试负责该防御检查。
Snapshot 恰为拒绝修订却包含被拒目标时，与已观察缺席矛盾，应按 authority divergence 退役，
不保留提交或安装成员关系。更高修订仍使用普通规范验证；不新增历史移除 ID 注册表。
实际 Server 在目标查询前检查预期修订，因此有效原生基线后的普通对等移除通常返回
RevisionMismatch。TabUnavailable 覆盖 action 相容回复的防御处理，并非主要对等移除竞争。
同修订缺席一致性是具体回复细化，不是新增已检查模型属性。

`state/move_tab.rs` 在可失败续程前保留精确提交并验证拒绝修订事实。`ui/move_membership.rs`
消费实际 Move 结果，既有共用成员关系所有者结算 Detached，并在回复互斥锁内提交两份安装计划。
`ui/move_membership_tests.rs` 覆盖晚类型化结果消费、精确旧清理、45/50 秒边界、错误代次、
无查看器直接安装及普通拒绝恢复。`connection/native/tests/tab_move.rs` 使用真实载体驱动
Move/Snapshot/Detach 到 Empty，或后继 Attach/Resize/Poll/帧反馈/Ready；随后一次 Snapshot
作为 FIFO 证据，确认旧 Input 和 Resize 未发送请求。它还检查仍拥有旧控制器的 pre-Switch Input/Resize，以及
已提交恢复仅发送 Snapshot。运行 `cargo test --locked -p orchbee --bin orchbee terminal_session::`
检查这些状态和消费者。完整原生二进制测试还需既有 Server、Worker 和 terminal-host 进程二进制。
这些检查不构造 Gone 证明，也不建立完整 Close 集成。
生产入口是 `TerminalIntent::Standalone`。遗留 `Create`/`Attach`、其执行器和渲染器，以及已连接的
遗留 TUI 入口只为测试编译。既有进程见证保留渲染、输入、Resize 和终端模式恢复断言；它不证明
生产启动路径。仅由测试使用的 Close 检查辅助函数和重导出采用同样的明确边界。Create reducer
测试在清理前准备候选，并使用共用安装计划。退休用例覆盖保留提交、已验证且待清理的候选、
清理前确定的投影不可用，以及已安装选择。不存在单独的先 Detach 后 Snapshot 状态或成功消费者。
Reducer 测试不声称证明载体或 mailbox 回执；这些义务仍由原生执行器与 NativeUi 成员关系测试承担。

必需所属模型扩展为 `NativeClientRefreshFence` 中按 ID/来源重映射和结构在途/恢复、
`NativeClientCreateBarrier` 中关联投影前移动创建成员，以及 `NativeTabSwitch` 中暂存候选后的
精确清理与 fresh successor 安装。保留现有 action 义务。确定性测试另外负责严格 0..256 身份/结构及精确相同/create/Move
修订检查、覆盖前复用创建 ID、不可变元组/绑定/Presentation 标签拒绝、三个稳定阶段、可缩减/重放的
生成 create/rename/move 历史、历史 Ready、过时 UI、准入竞争和恢复。假时钟/载体测试负责 checked-add
溢出、截止减一/等值/加一、锁竞争、更早截止时间、共享预算、初始及提交后 ID 耗尽、限时投递及过期后
UI 消费。受控 PTY 测试负责活动 PID/标记连续性、可用输入/Resize 及第二 Client 刷新；worker/UI
轨迹负责精确查看器/控制器/代次保留。内部退役到完成
仅属 Site/状态证据。一次性收窄 TLC 不支持 JSON、真实时间、无界置换、OS/PTY 或恢复成功声明。

## 投影细化证据

严格投影器现已按不可变 Tab 身份接受更高修订的 create/rename/reorder 历史。
`workbench_projection/tests/reorder.rs` 检查 0 至 256 的每个缓存长度、选定反转/旋转历史、
结构 ID 唯一性、不可变成员拒绝及后续移动后的创建成员查找。`label_evolution.rs` 将生成历史
与独立的排序成员关系预言机比较。精确提交前缀及同修订完全相等检查仍为必需。
这些检查建立纯投影边界；Move 命令运行时集成及 cohort 进程验收的独立证据见下文。

已检查的 `NativeClientCreateBarrier` 和 `NativeClientRefreshFence` 细化建模投影前移动创建
成员及按 ID/来源/索引重映射。Create 使用二至三个成员的历史；Refresh 枚举一至三个成员的缓存。
两者都采用选定置换，并抽象来源与有效性。它们不证明具体 256 成员置换、schema 验证、
不可变标签/绑定、线上关联或 OS/PTY 行为。
`NativeTabSwitch` 保留 Tab ID/代次清理抽象，没有缓存向量或序号；具体缓存替换需要状态证据。

状态替换现先按 Tab ID 准备来源，并解析阶段要求的选中及已提交身份，再安装缓存。
`state/tests/reorder.rs` 检查全部六类历史来源、三个稳定阶段、非首项选择、帧失效后的历史 Ready、
已移动的关联 CreateSnapshot 与显式恢复，以及覆盖来源前拒绝复用创建 ID。Started 恢复用例
检查移动后的绑定/索引及新 setup 预算。Create/Switch setup 和清理期间仍禁止普通替换。

`state/tests/reorder_history.rs` 在每个稳定阶段使用四个固定种子，每条 101 步 create/rename/move，
与独立规范快照及按身份键控的来源历史比较。语料从六个成员开始，每步至多增加一个；覆盖双向
移动、无变化移动及活动索引变化。它记录种子/阶段/覆盖并缩减失败操作片段及值；
`ORCHBEE_NATIVE_REORDER_REPLAY` 接受输出的逗号分隔 `c`、`r:index:value` 和 `m:from:to`
轨迹。仅快照创建检查 BindingOnly；关联创建者覆盖另有确定性证据。在 Unix 上使用
`cargo test --locked -p orchbee --bin orchbee terminal_session::` 运行这些检查。这些证据不建立
Move 邮箱准入、传输截止时间、精确 Move 提交置换或活动 PTY 连续性；下方独立的 Move 集成及进程检查负责这些边界。

## 范围与归属

本文档规定原生 Client 如何使用规范 OBWP `tab.create` 和 `tab.move` 契约，以及
`Ctrl+B c`、`<`、`>`、`n`、`p`、`1` 至 `9` 和 `w` 操作。它保留以下现有约束：一个顺序处理的已准入
Client 载体、一个 OBCS 查看器、至多一个兼管输入和尺寸调整的控制器、有界的命令/事件存储，
以及 TUI 恢复边界。

Server 所有的 Workbench 快照仍是 Tab 顺序、标签、Panel 与 Presentation 成员关系及终端绑定的
唯一事实来源。单次原生调用只拥有自身缓存的严格投影、本地活动 Tab ID、挂接代次、导航器状态、
尺寸观测、终端投影和临时状态。本地选择绝不写入或推进 Workbench 修订号。

以下内容仍不在范围内：固定/分组、Split 或子 Presentation
支持、持久化、推送/增量订阅、远程传输、重连、隐藏载体、载体多路复用，以及
在一个 Client 内组合多个查看器。

## 纯函数严格投影

在 `terminal_session/workbench_projection.rs` 中，用以下仅限 Unix 的纯函数投影替换
`select_standalone_binding`：

```rust
struct NativeContextTabs {
    site_id: SiteId,
    revision: WorkbenchRevision,
    tabs: Vec<NativeSimpleTab>,
    location: NativeContextLocation,
    navigation: Arc<[NativeContext]>,
}

struct NativeSimpleTab {
    tab_id: WorkbenchTabId,
    tab_label: WorkbenchLabel,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
}

struct NativeTerminalBinding {
    worker_id: WorkerId,
    terminal_id: TerminalId,
}

struct CreatedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    requested_label: WorkbenchLabel,
}

fn project_native_standalone_tabs(
    snapshot: &Snapshot,
) -> Result<NativeContextTabs, NativeProjectionError>;
```

启动投影器要求恰好存在一个 ID 等于 `navigation.standaloneTerminalContextId` 的 Terminal Context。
当前上下文投影器改为验证已选择的 canonical 位置。其他 Project/Workspace 的导航元数据保留，
非活动上下文的 Tab 布局不在这里验证。活动上下文不得包含分组，并可包含恰好
0 至 256 个有序 Tab。每个 Tab 都必须未分组且未固定，拥有一个 Panel 根节点，没有子
Presentation，并拥有一个已绑定的主 Presentation。只要存在一个不支持的成员，就拒绝整个
投影；投影器绝不筛选、重排或选择第一个可挂接成员。两个标签均原样保留，包括重复值和空白。

另外两个纯验证器负责替换缓存：

```rust
fn validate_append_only_refresh(
    current: &NativeContextTabs,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence>;

fn validate_created_tab(
    current: &NativeContextTabs,
    expected: &CreatedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<(NativeContextTabs, usize), NativeProjectionDivergence>;
```

已接受的 Move 细化替换旧 `validate_append_only_refresh` 前缀策略：要求 Site/上下文相同，
拒绝更低修订；相同修订要求完整投影精确相等，包括顺序和标签。更高修订时，每个旧 Tab ID 必须在
任意位置恰好出现一次，保持精确 Panel/Presentation 身份、结构、绑定、terminal-state 约束和
`Presentation.label`；只有 `Tab.label` 和顺序可不同。新成员通过完整 schema/严格投影器及结构
身份唯一性。移除、重复 ID 或其他不可变成员变化均为分歧。最多 256 个 Tab 只需精确扫描，不需存储映射。

`CreatedTabExpectation` 归投影层所有，只携带纯验证所需的六项规范事实。会话状态会嵌入或构造
它；请求身份、终端来源状态、挂接代次和邮箱类型绝不进入投影器。`validate_created_tab` 在投影
恢复期间保留原始创建前基线。返回的创建 ID 必须在来源覆盖或挂接分配前确认不在该基线中。其已提交
修订必须等于准入基线的 checked 后继，候选修订至少等于该提交。在任意位置恰好一次找到返回 Tab；
上下文/Tab/Panel/Presentation ID 等于结果，`Presentation.label` 等于请求创建标签。恰在提交
修订处，要求一个新成员、完整基线作为精确不变前缀，创建成员追加在后且两个标签均等于请求标签。
更高修订使用共享严格更高修订规则：旧/创建 Tab 标签和顺序可反映后续 rename/move，并允许严格的
后续成员。不再假定旧长度索引。结果无绑定；仅此已验证关联快照提供绑定，先前为该 ID 保留的任何
绑定必须精确不变。按 ID 准备来源状态，全部检查后才覆盖，再原子安装。已知创建来源在 Worker 退出
后仍为历史事实。仅此快照导出的绑定可进入 `AttachBinding`。

`MovedTabExpectation` 保留提交修订、上下文、Tab ID 及提交的线上方向。
`validate_moved_tab` 验证已准入基线经溢出检查的后继修订，并拒绝不可能的已提交无变化操作。
在该修订处，它要求两个标签均不变的精确完整向量置换；更高修订由共享严格身份验证器接受后续
受支持历史。`workbench_projection/tests/moved_tab.rs` 语料检查首/中/末位置、全部四个线上方向、
单成员及 255/256 边界、错误提交和不可变成员分歧。Move 结果及显式恢复状态转换使用此验证器；
其邮箱、传输及真实 UI 证据记录于下方。

纯投影器接受空的独立上下文，从而完整覆盖 0..256 一致性。冷启动和每次活动会话刷新各自额外
要求当前状态对应的活动成员不变量。

## 启动

冷启动流程仍严格如下：

1. 测量完整、真实的终端尺寸；
2. 提交一次 `terminal.open` ensure/join，并等待其最终 `Opened` 结果；
3. 在同一载体上提交一次 `SnapshotRead`；
4. 验证完整的 `NativeContextTabs` 投影，并要求存在 `tabs[0]`；
5. 设置 `active_tab_id = tabs[0].tab_id`，分配初始挂接代次，并发送从快照精确取得的
   `AttachBinding`；
6. 执行 Acquire，按条件执行 Resize，接收新的完整投影并绘制，发送匹配的 `FrameApplied`，
   且必须收到 `FeedbackAccepted` 后才能发布 Ready。

Open 返回的创建引用或终端身份均不参与选择。启动不重试快照，也没有后备路径，并沿用现有覆盖
整个启动过程的截止时间。

原生邮箱在 Open 前就已存在，此时没有期望挂接。唯一的请求游标提供 Open 和 SnapshotRead 的
身份。工作线程准备经过严格验证且非空的投影，然后把一个 Bootstrap 事件放入现有 `EventCell`。
在该事件的精确 UI 结算时，UI 检查原始启动截止时间，并原子分配首个挂接代次、安装缓存与
Starting 选择、发布期望状态镜像，随后放入 `BootstrapApplied(BootstrapTarget)`。该目标只包含
已验证的首个 Tab 及其来自快照的绑定，不携带新的 Switch/Create 截止时间。只有该回复才能授权
AttachBinding。初始化前不取出命令，也不引入临时 Tab 身份、额外通道或独立请求游标。

投影无效或为空、事件放入或 UI 结算过期，以及代次耗尽，均不会安装缓存与期望挂接。已经发出的
请求身份保持已消耗。重复或不再拥有当前等待的过时启动回复，不能再次分配代次或挂接。
确定性邮箱测试覆盖这些边界及等待 UI 时关闭的情况；脚本化载体测试覆盖 Open、SnapshotRead
与 Attach 失败及精确的身份顺序。三个 Tab 模型都从启动完成后开始，不证明这些冷启动实现声明。

## Client 状态与有界邮箱

`TerminalSession` 拥有仅限 Unix 的类型化状态。`client_view.rs` 始终参与编译，只包含与协议无关
的按键、浮层和提示决策；不得向其中加入 Workbench、OBCS、Ratatui、Unix 或 TTY 类型。渲染器
与会话仍使用 `cfg(unix)`。Cargo 的协议依赖继续位于 `target.'cfg(unix)'.dependencies` 下，
在不扩大依赖图的前提下，维持 Windows 上帮助、版本、用法及 TUI 不可用情形的可编译性。

会话状态是显式和类型。以下名称用于指导实现，而不是第二份线协议契约：

```rust
enum SessionPhase {
    Starting,
    Ready(ReadyAttachment),
    Creating(CreatingBarrier),
    CreateCommittedNeedDetach(CommittedTabCreate),
    ProjectionUnavailableBeforeCleanup(CommittedTabCreate),
    Switching(SwitchSetup),
    SelectedWithoutAttachment(SelectedTerminalState),
    RetryableAttachUnavailable(SelectedTab),
    Retired(RetiredState),
}

enum ControllerMode { Controlling(ControllerId), ReadOnly }

enum CreateTerminalProvenance {
    Started,
    Pending,
    Unavailable(WorkbenchTerminalUnavailableReason),
    Failed(WorkbenchTerminalFailureReason),
    OutcomeUnknown,
    BindingOnly,
}
```

`BindingOnly` 表示 Client 已得知一项规范绑定，但创建方不知道终端结果。Started 和 BindingOnly
Tab 可以挂接。Pending/unavailable/failed/outcome-unknown 结果按精确 Tab ID 保留，成为该 Client
会话稳定且不可挂接的来源状态。刷新不会清除这项认知。冷启动投影与每次刷新均按精确 Tab ID
合并来源状态：原样保留所有已有条目；对快照中尚无已知记录的每个 ID 分配 `BindingOnly`；完成
新建成员验证后，为刚返回的 ID 分配其已关联的 Create 结果来源状态，而不使用快照默认值。对于
原本可挂接的 Tab，切换期间明确的 Attach 失败则可通过以后显式选择同一 Tab 来重试。

普通 `active_tab_id` 只存在于有缓存支撑的状态中，并且必须指向最近一次已验证投影中的成员。
`desired_tab_id` 可以指向已经从该缓存复制出的 Switch 目标。尚未投影的 Create 结果留在不可变
已提交操作中；它不会填充 `desired_tab_id` 或公开的 `committed_target`。新建成员验证成功后，会
改为冻结一个包含规范顺序、来源状态、选中身份与代次的私有候选。只有精确旧清理完成后，一个
原子转换才会把该候选安装为有缓存支撑的 `active_tab_id` 与 `desired_tab_id`；绝不会把过时缓存
视作包含返回的 Tab。

用一个 `Arc<CommandMailbox>` 替换原始 `SyncSender<Command>`。其中包含互斥锁、条件变量、八条
记录的 FIFO、关闭标志、经过检查且不复用的挂接代次游标、已发布的期望代次/Tab、现有以操作
系统随机值为种子的 `Identities(u128)` 游标，以及一个预留的因果反馈槽。这个唯一预留槽有界，
且不是另一个载体；它使屏障前 Poll 引起的反馈能在 FIFO 头部屏障前完成结算。仍保留一个有界
事件槽。唯一的 I/O 所有者消费全部记录，并在已准入载体上顺序执行每一次 OBCS/OBWP 交换。

```rust
enum QueuedCommand {
    Input(Tagged<ControlInput>),
    Resize(Tagged<Geometry>),
    Poll(Tagged<()>),
    Switch(SwitchMarker),
    Create(CreateMarker),
    Refresh(RefreshMarker),
}

struct SwitchMarker {
    generation: u64,
    tab_id: WorkbenchTabId,
    binding: NativeTerminalBinding,
    provenance: CreateTerminalProvenance,
    forward_cutoff: Instant,
    setup_deadline: Instant,
}

struct CreateMarker {
    reserved_generation: u64,
    request_id: WorkbenchRequestId,
    site_id: SiteId,
    expected_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    label: WorkbenchLabel,
    geometry: WorkbenchTerminalGeometry,
}

struct RefreshMarker { refresh_generation: u64 }
```

每个会话恰好只有一个请求身份分配器。把现有游标从连接工作线程的所有权移入此邮箱；绝不克隆
其种子或计数器。界面层与工作线程只在检查并提交一个下一 ID 所需的时间内持有同一互斥锁，且
绝不跨 I/O 持锁。Create 准入先检查阶段与容量，在不修改状态的情况下计算下一个代次和请求
ID，然后在同一临界区中追加完整标记并提交两个游标。因队列/代次而拒绝发生在提交 ID 之前，
不会消耗 ID。一旦成功追加，即使之后遇到提交前拒绝，这两个值也已消耗。身份溢出不会追加内容，
也不消耗代次；由于今后的 OBCS 或 OBWP 请求都将无法关联，它会关闭邮箱，并把载体退役为一次
明确的本地交付前失败，而不是报告可重试的忙状态。

Switch 准入只持锁一次，并在此期间检查容量与 `checked_add`，复制精确的目标 ID/绑定及按目标
保留的来源状态，追加标记，推进分配游标和期望代次，并将目标发布为活动目标。Create 准入同样
预留并消耗一个未来挂接代次，但在已提交 Snapshot 候选通过验证、精确旧目标完成清理且候选原子
安装前不会发布它。提交前拒绝后，
已消耗的代次绝不复用。因队列或代次拒绝时，不追加任何内容，也不改变缓存、活动/期望 Tab、
代次、挂接或投影；界面层可以用本地化的“本地繁忙”文案替换临时提示。

界面层始终记录最近一次测得的有效尺寸，即使查看器处于 Ready ReadOnly，或在 Creating 或
Switching 期间暂停了终端 Resize 准入。该观测只改变 Client 本地布局数据，不授权 OBCS
Resize。会话单独跟踪最近一次已准入/已应用的控制器尺寸。Create 标记在其原子准入时捕获观测
尺寸；之后成功 Acquire 时，先用当时最新测量尺寸执行 Resize，后续观测则使用下文的建立追赶
规则。提交前拒绝会恢复旧控制器；如果测量值已变化，则在恢复轮询前准入一次追赶 Resize。
只读查看器永远不发送 Resize。

## 本地选择

选择只根据最近一次应用的严格缓存解析。`n` 和 `p` 使用本地活动目标的索引，并按 Tab 数量在
两个方向循环。`1` 至 `9` 是从一开始的精确索引；索引不存在时属于本地空操作，并显示越界提示。
Tab 数量为零时没有可选目标。

| 当前阶段和已解析目标                                                            | 精确结果                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 其他条件允许选择、目标不同，且一次刷新已经排队、在途或等待 UI 结算              | 在本地拒绝切换：标记、代次、选择、挂接、投影和导航器均不改变。报告本地繁忙。此守卫先于下方目标不同的各行。                                                                                                                                        |
| 其他条件允许选择、目标相同，且一次刷新已经排队、在途或等待 UI 结算              | 空操作，包括 RetryableAttachUnavailable：标记、代次、选择、挂接、投影和导航器均不改变。此守卫先于下方所有目标相同的各行。                                                                                                                         |
| Ready，目标不同                                                                 | 原子准入一个 Switch 标记。成功后立即更新活动/期望目标，清除可见投影/反馈，进入 Switching，并关闭导航器。                                                                                                                                          |
| Ready，目标是活动 Tab                                                           | 标记、代次、挂接、投影和导航器均不改变。                                                                                                                                                                                                          |
| Switching，目标不同于当前期望 Tab                                               | 容量/代次允许时准入另一个标记；它取代先前的建立流程。n/p 从当前期望 Tab 开始解析。                                                                                                                                                                |
| Switching，目标等于当前期望 Tab                                                 | 空操作；不生成重复标记或代次。                                                                                                                                                                                                                    |
| RetryableAttachUnavailable，可挂接目标等于活动目标                              | 显式选择会准入新的标记与代次；由于清理未留下查看器，执行器跳过 Detach。不存在自动重试。                                                                                                                                                           |
| RetryableAttachUnavailable，目标不同                                            | 原子准入标记/代次；由于不存在查看器而跳过 Detach，并按照该目标保留的来源状态分派，与 SelectedWithoutAttachment 完全相同。                                                                                                                         |
| SelectedWithoutAttachment，目标不同                                             | 原子准入同一种标记/代次转换。由于不存在查看器，执行器跳过 Detach，然后依据 **目标的**保留来源状态分派：Started/BindingOnly 执行正常 Attach 建立流程；Pending/unavailable/failed/outcome-unknown 不执行 Attach，结算为 SelectedWithoutAttachment。 |
| SelectedWithoutAttachment，目标是已选中的不可挂接 Tab                           | 空操作；没有标记或代次。                                                                                                                                                                                                                          |
| Creating、CreateCommittedNeedDetach、ProjectionUnavailable、Starting 或 Retired | 拒绝有副作用的选择。只有下文明确列出的恢复操作仍可使用。                                                                                                                                                                                          |
| 队列/代次已满                                                                   | 保持选择与挂接状态；报告本地繁忙。                                                                                                                                                                                                                |

对只含一个 Tab 的缓存，n/p 都解析到同一成员：Ready 是空操作，
RetryableAttachUnavailable 仅在没有待结算刷新时才是显式重试。有待结算刷新时，所有其他条件
允许选择的阶段都先应用同目标空操作守卫。队列已满或越界操作不会关闭已经打开的导航器。
Panel 按键仍不可用。

## Create 是排空 FIFO 的屏障

`Ctrl+B c` 只允许从状态一致且已结算的 Ready 准入：载体存活；活动、期望、挂接和投影代次都
指向同一个缓存 Tab；这个挂接代次在建立期间已接受第一个完整帧反馈；没有正在进行的
Create/Switch；并且没有已排队、在途或等待 UI 语义结算的 Refresh。普通投影更新和提交前恢复
不会清除 Ready 所代表的建立完成事实。后续暂存帧、NeedSnapshot、ProjectionBlocked 或已接受
的 Resize 可以使当前帧证明失效，但不会撤销 Create 准入资格。旧目标的在途或已排队 Poll 不会
阻止准入，因为该屏障负责排空其绘制和因果结算。这不会放宽首次发布 Ready 所需的完整帧、尺寸
或反馈要求。普通 Input 继续使用 Ready/Controlling 权限；Create 成功准入时，与标记一起原子
关闭后续副作用准入，先前已准入的副作用仍按 FIFO 顺序排空。
准入不会延长较早请求自身的截止时间，也不保证 Create 一定提交：标记前的请求失败或到期时，
载体可能在分派标记前退役。
拟定标签是十进制的 `cached.tabs.len() + 1`；达到 256 个 Tab 的上限时，该值为 `257`，
由唯一一次提交的命令让 Server 容量策略返回类型化结果。允许重复标签。成功准入会关闭导航器并
进入 Creating。Exit 仍可用。帮助/侧栏浮层的变化仍是本地行为，不会准入终端或 Workbench
副作用。

Creating 拒绝后续 Create、切换、刷新、用户输入和有副作用的 Resize。Resize 观测仍会更新最新
测量尺寸。它停止调度新的旧目标 Poll。UI 继续排空有界事件通道，并可在界面显示 Creating 的
同时，合并并绘制一个已经接受的旧代次 Poll 结果。

连接工作线程提交 `tab.create` 前，会排空标记之前的每一条 FIFO 记录：

1. 已接受的 Input/Resize 继续遵循现有有界 ServerBusy 策略，直到得到如实的明确或未知结算，
   即使其他本地状态发生变化；
2. 标记前的 Poll 结果会送到 UI；有效帧必须实际绘制后才发送其 `FrameApplied`，NeedSnapshot
   只在对应的无帧重绘后发出；
3. 因果结算占用预留槽，先于屏障处理。如果它携带反馈，工作线程发送该反馈并要求精确的
   `FeedbackAccepted`；ProjectionBlocked 则必须在无帧重绘后才结算，不发送线上反馈；
4. 只有在旧投影在途请求和反馈均已结清后，工作线程才能提交那一条已捕获的 OBWP 命令。

该排空过程必不可少，因为提交前结果会保留同一个查看器。丢弃其在途请求并假定之后会 Detach，
可能使保留的查看器一直阻塞。UI 绝不确认尚未绘制的帧。

第一次提交 `tab.create` 使用唯一的已捕获请求 ID。不存在使用新 ID 的自动重试、后备 OBCS
Create/Open/Start、取消声明，也不会根据新近可见的 Tab 作推断。在明确的提交前拒绝之后，用户
再次按下 `c` 属于另一次独立操作。当一个 Create 仍处于其屏障或已提交投影恢复过程内时，不准入
后续 `c`。待其完成结算且选择重新处于 Ready 后，用户之后按下的 `c` 才是使用新请求 ID 的新
操作。

Create 准入时固定一个从锁前时钟采样起 50 秒的单调总屏障截止时间，并固定一个比它早五秒的清理截止点。
标记前排空、唯一一次 OBWP 交换、已提交 SnapshotRead/候选暂存、精确旧清理和 Started 建立流程
共享这个绝对截止时间；任何进展都不会使其续期。前向工作到清理截止点时停止。已知查看器可以
使用最后的预留时间执行精确 Detach 及下文规定的关联清理结算；如果到最终期限屏障仍未到达 Ready、
SelectedWithoutAttachment、RetryableAttachUnavailable、ProjectionUnavailable 或明确的提交前
恢复状态，载体就会退役。这是操作结算的界限；它不假定 Worker 或 Host 会取得进展。

结果转换如下：

| 结果                                                                                                                                             | 转换                                                                                                                                                                                                                                                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 顶层明确 denied/rejected/unavailable，或已证明未提交                                                                                             | 保留完全相同的旧查看器/控制器、活动 ID、投影和有效反馈；以原子方式回到 Ready，并带有下文定义的事件驱动提交前提示；如果处于控制状态且有需要，发出一次追赶 Resize，然后恢复 Poll。不要自动刷新或重试。                                                                             |
| `tab-create-pending`                                                                                                                             | 保留已提交元组/修订号/来源状态及预留代次 1。在旧查看器清理前读取并验证新 Snapshot，冻结精确规范候选，然后完成精确旧清理并原子安装候选。安装前持续显示 Creating；本 cohort 永远不挂接新建 Tab。保留请求身份；不要提交新身份。                                                     |
| `tab-created/started`                                                                                                                            | 保留已提交事实及预留代次 1；读取并验证新 Snapshot；冻结其精确候选；只有之后才完成精确旧清理并原子安装有缓存支撑的活动选择。若返回成员仍存在，AttachBinding 使用其 Snapshot 派生绑定及代次 1。若更晚 Snapshot 已移除它，冻结序号以代次 2 选择存活候选，或选择 Empty；不得复活它。 |
| `tab-created/unavailable`、`/failed` 或 `/outcome-unknown`                                                                                       | 保留稳定不可挂接来源状态及代次 1；读取并验证新 Snapshot；冻结其精确候选；只有之后才完成精确旧清理并原子安装匹配的“已选择但未挂接”状态。已移除的返回成员仍是历史事实，存活后备项使用代次 2。不要发送 Attach。                                                                     |
| OBWP 可能已经交付，随后发生写入/读取超时或失败；结果格式错误/被截断/类型错误/关联错误/与操作不兼容；已提交上下文/修订号错误；或 started 尺寸错误 | 命令结果未知。使混合载体退役，并丢弃未发送的命令。不得推断、重试、刷新、在有歧义的载体上执行 detach，或选择未经过有效关联的返回身份。                                                                                                                                            |

每个按操作区分的已提交结果都必须携带已捕获上下文，且修订号必须精确等于
`expected_revision + 1`；发生溢出时，已提交结果无效。`started.geometry` 必须等于已捕获尺寸。
适配器会先验证这些字段，然后状态机才会把结果视为已提交。

适配器一旦返回已验证的提交结果，工作线程就已经知道该事实，包括尚未发布邮箱事件的时刻。
原生执行器立即用一个仅由工作线程拥有的待确认值保留完整且不可变的 `CreateOperation` 和提交
结果，然后才执行其他可能失败的栅栏、截止时间或发布步骤。它绝不覆盖先前的待确认值。只有消费
精确的 `CommitRetained` 回复才会清除该值；发布、UI 取走事件或放入回复都不够。发生类型化失败
或协作关闭时，该值通过既有原生工作线程的 join handle 返回。它不是最近一次提交的缓存，也
不是额外的邮箱、事件槽或因果槽。

`CreateCommittedKnown` 的放入与 UI 保留使用原有总截止时间。这只保留不可变事实，不选择新
目标，也不授权前向工作。工作线程必须先消费 `CommitRetained`，才能执行 SnapshotRead、暂存候选
或精确清理旧查看器。
总截止时间之前放入的回复仍可在之后消费：消费会清除待确认值，因为状态已经拥有这些事实，
但 `now >= total` 仍禁止 SnapshotRead 或 Detach。总截止时间之前，如果确认晚于前向截止点，也
不能开始 SnapshotRead，因为前向截止点前尚未暂存候选。最终清理预留仅适用于已经有验证候选的
情况；执行器可以在预留内开始精确旧 Detach 或完成精确旧清理，而在预留内完成只会退役，不安装
候选、也不挂接。它绝不续期原有 45 秒前向截止或 50 秒总截止。

原生 UI 退役先关闭准入、记录退役请求，并清除可见投影和待绘制内容。它保留当前状态和邮箱，直到
所属 `TerminalSession` 关闭 socket 并在不持有邮箱锁的情况下 join 工作线程。随后使用邮箱的
首次失败原因和返回的待确认值完成状态退役，再释放邮箱。每个可变产品入口都必须在返回前完成
这个顺序；绘制、操作或只读视图不能与其中的私有中间状态交错。Drop 使用相同的关闭和 join
顺序。UI 错误路径不能在最终结算前丢弃 Creating 屏障。

状态最终结算器只接受与完整 Creating 操作精确匹配的待确认值，或与已经保留的操作及提交结果
完全相同的重复值。提交构造检查已捕获修订号、上下文、尺寸和预留代次；识别已保留的重复值时
比较完整元组和来源状态。新建成员关系和绑定仍由 Snapshot 校验负责。它保留
旧的诊断活动身份，不发布可操作的已提交目标。过时、未来或冲突的事实不会被注入；协议不匹配
也不能替换更早的退役原因。重复结算保留首因和已保留事实，已完成的较早 Create 不能为后来的
操作提供诊断结果。

这个交接覆盖预期的类型化错误和协作关闭，包括适配器返回到事件发布之间发生的关闭。意外的
工作线程 panic 会终止所属失败域：既有退出 guard 关闭邮箱，所有者执行 join，但发生栈展开的
线程不承诺返回其待确认栈值。不引入通用 panic 恢复。Create 模型把结果取得和保留抽象为同一
步骤；确定性的工作线程/UI 测试负责证明这个细化在发布、取走、回复放入、消费、截止点相等及
关闭边界上的行为。

得到有效的已提交结果后，同一份固定前向预算先允许一次 SnapshotRead。有效的精确或更晚响应
必须通过 `validate_created_tab`、保留已提交的历史 Create 事实，并冻结完整规范候选：顺序、
修订号、按身份对齐的来源状态、选中 ID 和代次。该暂存步骤之前不会发布返回 ID 或任何替换选择。
返回成员使用已经预留的代次 1。若有效的更晚 Snapshot 已将它移除，其缺席绝不擦除 Create 事实
或回退游标；冻结历史序号以代次 2 选择存活后备成员，或选择 Empty。

Ready 来源的暂存随后只授权清理精确旧挂接。经过精确关联的 OBCS `Detached` 可证明清理完成。
不透明的已保留 known-Closed 事实仅在匹配精确旧 Tab/绑定/代次、所有者清理已完成且尚未交付
Detach 时，才能证明相同结果。Snapshot 中的原始缺席绝不是该证明。明确的 `ServerBusy` 仅在
操作截止时间内重试。明确的非忙拒绝/不可用、总截止到期或任何已交付 Detach 歧义都会使载体
退役。Empty 来源的暂存跳过清理，因为不存在旧查看器。

精确清理后，一个 UI 转换会原子安装已暂存缓存、来源状态、活动/期望身份、派生索引和代次。
只有该转换可以发布替换选择，或授权 Started 挂接。若精确 Detach 在最终清理预留中完成，Client
会退役而不安装候选。这会保留已知 Create 提交及旧诊断选择，不会把命令结果重新归为未知。
重复退役保留这些事实及首个原因；它们均不授权退役后的输入、选择、刷新或恢复。

候选暂存前明确的 Snapshot denied/unavailable 会进入 `ProjectionUnavailable`。它保留已提交
元组、修订号、终端结果及预留代次 1，同时保留精确旧已验证缓存、查看器/控制器、活动选择及
投影/反馈。这些旧事实仅供诊断：结构栅栏仍然生效，因此 Input、Resize、普通 Poll、选择、
Create 与 Move 均不可用。`Ctrl+B w` 可以准入下文的显式恢复；退出仍可用。有效恢复会先暂存
候选，再清理并安装。有效但不一致的快照属于权威状态分歧，会使载体退役；格式错误、关联错误
或传输歧义响应也同样处理，且都不授权猜测 Detach。

通过后续 Refresh 恢复 Started 时，不复用原 Create 截止时间或已经结束的 Refresh 传输截止时间。
纯投影与已创建成员校验通过后，UI 在取得邮箱锁前采样共享操作时钟。恢复准入将
`total = sampled_at.checked_add(50s)` 与 `forward_cutoff = total - 5s` 同挂接续行一起原子提交。
这份新的保守建立预算覆盖 Attach、Acquire、Resize、Poll、真实绘制、反馈及精确清理，进展不会
使预算续期。它使用保留且尚未执行过挂接的 Create 挂接代次，每次交换仍从唯一游标分配新的请求
身份。它不会重新提交 Create，也不会准入用户 Switch 标记。如果截止时间运算溢出，则将该次
精确的待结算 Refresh 消费并清除为刷新失败，保留 ProjectionUnavailable、恢复前缓存、已提交
事实，并将所有身份/代次游标保留在 SnapshotRead 之后的值，不发送 Attach，并保持载体可复用。
已发出的 Refresh 请求身份仍已消耗。已知不可挂接来源状态的恢复不需要
建立截止时间。确定性轨迹覆盖 UI 在前两个截止时间均已过后才结算、恢复成功、固定建立预算到期、
溢出时的状态保留以及新的交换身份；独立的 Refresh 与 Switch 模型不声明覆盖这些状态的组合。

## Switch 建立与取代

共享 Switch 安装与交接基础、Snapshot-first executor 及迟到丢弃后的 anchor 失效均已集成，
并由 state、mailbox 和实际 UI 测试覆盖。这些证据证明下述单 Client Switch 运行时；多 Client
PTY peer 移除恢复见证仍属独立工作。

运行时把 Switch 调用 handoff 映射到既有 mailbox mutex 下的唯一 receipt。准许同步 carrier 调用
前，一并核验精确活动 lease、原始 marker、cleanup token 与旧挂接、共享 request cursor 及原截止点。
该 receipt 证明调用授权，不证明传输提交或成功；IPC 从不持有 mutex。新 marker 不能抹掉已授权
调用。相容响应把同一 receipt 保留为已回答，直到精确 UI 语义提交与 reply 放入一并释放它。因此，
无论新 marker 在 UI 消费前到来，还是在 UI 提交后、worker 读取 reply 前到来，都保留如实结算。
当前 Busy 在 gate 下释放并复核；已被取代的无副作用响应保留 receipt 直到类型化结算，不清除
viewer。歧义调用在退役期间保留授权证据。唯一共享 membership record 拥有候选和清理政策；
该 receipt 不增加候选、截止时间、身份计数器或清理 owner。

旧 Poll 的返回可能在 marker 准入后、放入前变为 stale。Mailbox 把该真实丢弃，以及已取走却放弃
的 Feedback 结算，保留为精确 marker／源挂接事实。成功的取代准入把事实传给匹配的继承 anchor；
准入被拒绝时保留事实。当前 Snapshot 的 UI 结算在准备恢复或安装前消费它。只有相同源 key 的
证明失效。调用前跳过、没有 frame 结果的 Busy，或空 stale 回调都不制造丢弃事实。该事件 receipt
在 queued marker 成为 active 后继续保留，不分配请求、代次或预算。

运行时成员 owner 区分已准入的 Switch 意图与已安装的挂接键。意图准入消耗一个代次。
在既有 FIFO 与精确旧在途请求 drain 后，一次 SnapshotRead 使用共享 identity cursor，以及标记
原有的 45 秒前向／50 秒总预算。精确类型化结果必须在前向截止点前放入。有效 Available 候选
保持暂存，等待旧挂接清理。每次非空安装，包括目标仍存活与后备选择，都消耗 checked successor，
并原子发布 cache、selection、desired/work key、cursor 和挂接许可。Empty 保留已消耗 cursor，
不拥有 key、viewer、controller，也不伪造 Ready 证明。成员模型拥有候选校验与 ordinal 选择；
Switch 模型接收该有效选择，检查具体 key 转换、清理与建立续程。
意图不能安装 projection、接受目标 setup configuration、发布 Ready，或结算为未挂接 setup。
这些路径必须持有已安装候选。旧的直接 Switch-old-Detached 事件及其意图转 Attach 续程已移除。

及时的明确 Unavailable 结果只能恢复 owner 持有、源自真实稳定状态的 anchor。恢复 Ready 要求
精确相同的 viewer/controller，以及在标记前如实结算后仍有效的完整帧、draw/feedback 和当前
geometry 证明。已接受 Resize 清帧、最新 geometry 未匹配、无法保留证明的旧 Poll/causal 放弃，
或 viewer 清理／替换，都会使该证明失效。Anchor 随这些事实更新；准入时 clone 不能回滚它们。
已记录的 selected-without-attachment 或 retryable 来源，只能在仍没有 viewer/controller 时恢复。
尚未完成的 Switch 不能创造 stable anchor；特别是第一个标记尚未 Ready 的 viewer 清理后，第二个
标记没有任何 anchor。当前 Unavailable 没有有效 anchor 时退役，不发送修复 Poll 或建立操作。
已被取代的结果仅结算其旧 flight。精确恢复可以把 desired 返回旧的已分配 key，但 allocation cursor
绝不回退，意图代次仍被消耗。及时放入 Unavailable 后其 transport 预算结束，因此精确 UI 结算可以
稍后发生；它不启动前向工作，也没有人为添加的放入后截止时间。

Gone 只能在 Detach handoff 前结算计划中的精确旧清理，要求 actual Close owner 的 exact
known-Closed proof、所有更早工作已结算，且没有 outstanding wire request。Snapshot 缺席不足以
证明 Gone。Gone 不创建 pending Detach，也不能替代已经 handoff 的请求响应；该请求仍要求原始
精确响应或如实按歧义退役。安装前若前向预算到期，只允许在原总预留时间内精确清理旧挂接后退役；
没有 viewer 时直接退役。安装后保留下文既有的未尝试、已尝试建立及清理过期区别。候选处理不会
延长任何一个截止时间。
当前 Switch executor 始终为旧 viewer 取得精确 Detach 响应，尚未消费 Close owner 的 Gone
证明作为快捷路径。

`NativeTabSwitch` 独立于 generation cursor 限定两次准入意图。四个配置变体按初始 controlling／
read-only 模式，以及四／二代次上界分区。每个配置保留所有准入目标、同目标重试和候选／后备
选择，不使用 state constraint 或固定 Tab 名称捷径。四允许两次完整操作（intent 1／install 2／intent 3／
install 4）；二额外检查 successor 耗尽。达到两次操作的探索上界时，两个代次配置都不会伪造产品
拒绝。原有 51 个 required action 全部保留，并加入候选、Gone、精确 anchor 恢复、无效 proof 及
第二标记无 anchor 退役见证。另有两个带 guard 的 action 见证要求精确意图集合 `{1, 3}`、保留的
已安装代次 2 的 Ready anchor、从 cursor 3 安装到代次 4，以及第二个代次的完整 Ready proof。
它们不增加状态或改变转换并集，并为两次完成的切换提供见证；仅分配 cursor 4 不满足该证据义务。
该模型为每个配置声明 600,000 ms 验证超时，并使用两个 TLC worker。此前两个 180,000 ms 检查
均未完成，其中一次按首次目标类别分区仍未充分缩小探索空间。这些超时仍是失败证据；观察到的
状态数和更大的固定上限都不保证完成。该验证资源上限不改变任何运行时截止时间、有限上界、
被检查属性或公平性假设。
单调分配与精确恢复替代过时的无条件 cursor/desired 等式。唯一新增
公平性是及时 Unavailable 结果在存活 UI 上的精确结算；不假定 Snapshot 成功、anchor 必然可恢复
或 Host 完成。共享 identity 字节、不可变 snapshot tuple、真实旧 flight proof 更新、数值截止时间
与运行时恢复仍属于确定性 adapter/core 证据，不是该有限模型的结论。

工作线程把当前 Switch 标记作为一次建立操作处理：

1. 结算先前已接受的 Input/Resize；
2. 在顺序载体上结算任何已经交付的 Ready 代次 Poll/Feedback 响应，再读取、放入并验证当前候选，
   按上述规则精确清理旧挂接并原子安装 fresh key；Detach 只会清除尚未发送／已暂存，或已得到
   精确响应的旧投影在途请求；
3. 使用已安装候选的有效 Site/Worker/Terminal 绑定执行精确 AttachBinding；
4. 根据返回的 AttachmentId/HostEpoch 构建新的 `TerminalProjection`；
5. 执行 Acquire；控制器繁忙时转为 ReadOnly，而 Acquired 使用最新真实尺寸和新的控制器序号开始
   Resize；
6. 持续 Poll，直到新投影接受一个完整快照帧。Empty 时重复。NeedSnapshot 在无帧重绘后反馈，
   然后继续轮询。ProjectionBlocked 保持可见的 Switching/blocked 状态，并要求修订号更高的后续
   完整快照；
7. 暂存一个精确代次/Tab 帧，将它绘制，发送匹配的 FrameApplied，要求收到
   FeedbackAccepted，然后才能发布 Ready Controlling 或 Ready ReadOnly。

对于控制建立流程，每一次已接受 Resize 都会把其精确尺寸与最新测量尺寸分开记录。帧门只接受
尺寸等于最近一次已接受 Resize 的新鲜完整帧。在发布 Ready 前的最后时刻，UI 持有邮箱互斥锁并
再次比较最新测量值。如果不同，Ready 仍不可用，Ready 之后的 Input/Poll 准入仍关闭；工作线程
会发送一次携带最新测量值和全新控制器序号的追赶 Resize。然后它重新开始完整帧、绘制及
FrameApplied/FeedbackAccepted 门。因此，在 Acquire、Resize、Poll、暂存帧、绘制或反馈阶段
发生的尺寸变化，要么成为首次 Resize 值，要么触发这个有界追赶循环。已经绘制的旧尺寸帧确实
曾经可见，因此会得到如实反馈；但它不能授权 Ready。ReadOnly 建立流程按最新本地视口记录并
渲染，但不发送 Resize。共享建立截止时间限制持续的物理尺寸调整。

每个已准入 Switch 标记都捕获自己的 50 秒单调绝对截止时间，以及比其早五秒的前向截止点；
任何进展都不会使其续期。如果存在查看器，工作线程可以在最终预留时间内开始并完成精确 Detach。当前期望
目标失败后完成精确清理，会进入 RetryableAttachUnavailable。因被新请求取代而进行的精确清理
则直接转向最新准入的标记，绝不把过时目标发布为 Retryable 或 Ready。在较旧代次正在清理时
准入的新标记保留其独立捕获的时间预算；精确清理会继续转向它，但如果它自己的时间预算已经过期，
则禁止 Attach，并把可挂接目标结算为 RetryableAttachUnavailable，把已知不可挂接目标结算为
SelectedWithoutAttachment。清理未得到 Detached 或交付存在歧义时，载体会退役。这样可以在
不声称 Host 最终取得进展的情况下，限制反复的 Empty/NeedSnapshot/ProjectionBlocked 和
ServerBusy 序列。初始启动继续使用原有覆盖整个启动过程的时间上限。

对于仍持有前一个查看器的 Switch 候选，精确旧 Detach 及其关联的 UI 结算使用该 Switch 标记的总
截止时间；即使交换期间或 UI 消费结果之前跨过前向截止点，也同样如此。在前向截止点之前结算
可以返回挂接续程。从前向截止点开始到总截止时间之前，结算则只原子清除精确的旧挂接后退役，
不安装候选或发送 Attach。候选已经安装后，当前建立清理保留下文的 RetryableAttachUnavailable
或已知 SelectedWithoutAttachment 结算。被新请求取代时，只清除旧挂接并继续最新标记。在总截止时间之前放入的回复可以
稍后消费，但消费它不能授权任何截止点之后的前向工作。

当前已经安装的 Switch 候选没有查看器且尚未尝试 Attach 时，可能在进入建立流程前耗尽前向预算。
这包括已安装挂接续程的回复提前放入，但工作线程在截止点之后才取走的情况。尚未安装的标记或
候选不能使用此稳定结算，而是遵循上述安装前退役规则。当 `now < total` 时，同一个精确的未尝试过期结算直接发布
RetryableAttachUnavailable 或已知的 SelectedWithoutAttachment，不执行 Attach；过时标记则
直接跳向较新的标记。工作线程在首次 Attach 之前重新检查这个边界；较早的时钟采样或已放入的
续程都不能授权迟到的请求。这个独立的未尝试边界不声称 Detach 成功，也不延长已经尝试的建立流程：Attach
拒绝/不可用以及连续 Attach Busy 仍使用仅限前向预算的建立结算。

精确清理包括把关联于当前代次的结算放入现有 `EventCell`。当 `now < total` 时，UI 在邮箱锁下
原子发布 RetryableAttachUnavailable 或 SelectedWithoutAttachment，并放入匹配的 `Reply`。
当 `now >= total` 时，尚未完成的结算到期，使载体退役，不发布稳定状态。超时与 UI 结算通过同一
把锁串行化。一旦回复已放入，旧截止时间就不能使已完成的结算退役或撤销它。工作线程可以稍后
消费该精确回复，然后在任何后续效果前重新检查期望代次、已排队标记及其各自的截止时间。回复
在消费前持续占用该槽，防止槽被复用。如果在 UI 结算前有更新的标记取代了此次清理，旧结果只会
清除精确的旧挂接，随后转向最新标记，不发布过时状态。复用该槽时，只能使精确匹配的已过期
`UiWait` 键失效；迟到的旧回复不能清除或覆盖
更新的事件、回复或状态。不存在第三个槽。

预留时间不会延长 Attach、Acquire、Resize、Poll、Feedback、SnapshotRead，也不会延长普通的
建立流程与语义 UI 等待。Create 提交前恢复、授权旧清理的候选验证，以及允许候选安装的任何
普通清理完成，仍必须在前向截止点前完成。较早已交付的交换可以在其原始截止时间内结束，但其结果
不能授权截止点之后的新前向工作。模拟时钟轨迹覆盖清理事件放入、消费和回复的每个边界，包括
被新请求取代和精确等于截止时间的情况；Switch 模型把这些清理步骤抽象为一个结算转换。

每次得到明确的 ServerBusy，以及其他每个会阻塞的 Detach/Attach/Acquire/Resize/Poll/Feedback
结果后，工作线程都重新加载邮箱的期望代次。标记前已经接受的 Input/Resize 必须继续得到如实
结算。即使被新请求取代，旧查看器的 Detach 仍是前置条件。对于已经被取代的
Attach/Acquire/Resize/Poll/Feedback，停止重试：Attach 前直接跳过；得到任何 Attached 结果后，
必须先精确 Detach 再处理更新的标记。如果请求可能已交付但结果未知，则使载体退役，而不是猜测
或继续执行。

同一个载体绝不允许 Detach 越过已经提交的 Poll 或 Feedback。对于已经交付的旧 Ready Poll，
先读取精确响应；明确的 Busy 会停止重试；精确帧可以不经绘制直接丢弃，因为紧随其后的 Detach
会清除它的反馈在途请求；存在歧义则使载体退役。对于已经交付的 FrameApplied/NeedSnapshot，
在收到精确响应前保留不可变的请求事实；被新请求取代时，明确的 Accepted/Busy/rejection 会在
不重试的情况下完成结算，随后执行 Detach；存在歧义则使载体退役。只有已暂存但从未交付的
Poll/feedback 操作才可直接放弃。脚本化载体测试分别在交付前、Poll 交付后、帧送达后、绘制后
和 Feedback 交付后切入取代；Switch TLA 对应建模建立代次的帧/反馈，包括 Feedback 交付后的
取代。

只要 Attach 之后发生任何明确的建立失败，已 Attached 查看器也必须得到精确的 Detached 结果。查看器尚未
存在时的 Attach rejection/unavailable 不需要清理。除控制器繁忙外的 Acquire 失败，以及 Attach
之后的 Resize 失败、Poll 失败、反馈失败、截止时间到期或被新请求取代，都要求清理。因此最多只会
保留一个查看器/控制器。

每个命令/事件都受精确挂接代次和 Tab ID 限定。过时事件、投影、反馈已接受报告或 Ready 报告均
被丢弃。如果某个帧在之后的切换准入前已成功绘制，那么它确实曾经可见；Client 会调度一次清屏
重绘，绝不声称过时像素从未发布。保证是：被取代的代次不能发布 Ready、接受标记之后的输入，或
保留其查看器/控制器。如果 FrameApplied 已发送，它可以在强制 Detach 前结算；若尚未发送，则由
Detach 清除在途请求。

明确的建立失败会保持所选 Tab 处于已选状态，并让其可挂接来源状态可重试。不会选择其他 Tab、
旧绑定、Open、Create、Start 或终端生命周期后备路径。

## Ctrl+B w 导航器与刷新

`Ctrl+B w` 是精确的开关。Workbench 导航器打开时，它会关闭且不提交任何内容。关闭且没有待
处理刷新时，它立即显示在缓存投影之上，并尝试恰好一个
`RefreshSnapshot { refresh_generation }`。在稳定的 Ready、RetryableAttachUnavailable、
SelectedWithoutAttachment 或 ProjectionUnavailable 状态中打开时可以准入；Starting、
Creating、Switching、Retired、邮箱已满或刷新代次耗尽时显示本地繁忙，且不发送任何内容。如果
一次已准入刷新尚未被 UI 消费，再次打开只显示该待处理代次和缓存内容；它不会再排入一个
SnapshotRead，也不推进代次。在 FIFO 标记、在途交换和未消费结果三个阶段合计最多只有一个待
处理刷新。

在 UI 消费该唯一结果前，目标不同的 n/p/数字选择会在本地被拒绝，Create 仍不可准入；目标相同
的选择保持普通空操作。因此在 Switching 期间不会应用刷新结果，每个已准入 Switch 都从完全
结算的缓存复制其目标与绑定。确定性归约器轨迹覆盖：刷新已排队、刷新在途、导航器关闭时结果已
排队、UI 消费成功/失败/分歧结果、拒绝选择时精确保留核心状态、RetryableAttachUnavailable 在
三个待结算阶段的同目标空操作，以及完成结算后再准入选择或显式重试。独立
的有界刷新模型与切换模型有意不声称覆盖这些状态的组合。

邮箱已满或打开被拒绝时，仍显示缓存行和本地繁忙状态；缓存、活动/期望 Tab、挂接代次、投影和
最近一次已准入刷新代次均不改变。关闭导航器、用快捷键帮助/侧栏替换它，或成功选择/已准入
Create，只会隐藏它，绝不会取消已准入的 SnapshotRead。前缀之外的 Escape 仍是终端输入，不会
关闭浮层。同一 Tab 空操作、越界选择和准入拒绝都会让导航器保持打开。

导航器仅供查看。它可见时，普通终端字节批次继续通过正常的 Ready/Controlling 准入；ReadOnly
和屏障状态沿用各自现有的拒绝规则。ClientView 绝不把原始 Escape 字节当作 UI 事件，也绝不拆分
方向键、Alt 或其他终端序列来实现关闭。

Refresh 共享载体与 FIFO。它不得放弃当前代次的投影在途请求：同一条预留因果反馈规则会在
SnapshotRead 前，将任何先行 Poll 一直排空到绘制和 FeedbackAccepted。在刷新标记后准入的
Input/Resize 会排在该读取操作之后等待。工作线程发出带刷新代次标签的类型化结果，随后在邮箱
条件变量上等待 UI 结算精确的待处理代次。之后接受的 Input/Resize 仍留在 FIFO 中；结果等待
语义验证时，它们不能执行。UI 以原子方式应用成功/失败结果，或在发生分歧时使载体退役，随后
清除待处理代次并通知条件变量；执行器只能在此之后继续。Exit 或载体退役也会唤醒它，并丢弃未
发送记录。成帧、关联、类型错误或传输歧义会在发出类型化结果前被检测到，并立即使载体退役。
因此 Refresh 屏障跨越读取前的因果反馈排空、线路读取和 UI 语义结算。

`RefreshMarker` 有意不携带请求 ID。因果在途请求排空完成、它到达 FIFO 头部后，工作线程锁住
同一个邮箱身份游标，恰好分配一个 SnapshotRead ID，解锁，然后执行 I/O。本地身份耗尽发生在
线路交付前：工作线程为该待处理刷新代次发出一个精确的本地失败，关闭邮箱，并使这个已无法使用
的载体退役；它绝不臆造第二个游标或请求 ID。刷新传输预算是在准入时固定的一个 50 秒单调截止
时间，由先行在途请求排空与 SnapshotRead 共享。到期时可能使已排队/在途交换退役。类型化结果
一旦进入有界 UI 事件槽，传输截止时间就已结束；关闭导航器不能取消它，正常事件循环消费会在
没有合成超时的情况下将它精确结算一次。

UI 对唯一待处理代次恰好消费一次。它会防御性地丢弃代次不同于待处理/最近已准入代次的任何
类型化完成结果，且不改变缓存、活动/期望 Tab、投影、导航器状态或已提交创建的恢复状态。这样的
过时完成结果是对抗性测试切入点，不是排队并发读取的许可。线路/成帧/关联失败已经破坏唯一载体；
即使相关逻辑代次后来证明为过时，也会使它退役。

当前明确 denied/unavailable 结果保留缓存、活动 ID 和终端投影，并设置刷新失败；显式恢复期间
明确不可用保留该恢复状态。已接受的 Move 细化应用严格投影器和以 ID 为权威的替换规则。更低修订、
相同修订有差异、旧 ID 缺失/重复、不可变元组/绑定/Presentation 标签改变、Site/上下文不匹配或
阶段所需活动身份丢失会退役。更高修订下支持的重排和 Tab 标签改变有效。成功按 ID 准备来源状态，
原子替换缓存/来源状态，保留活动 ID 并派生其新索引。ProjectionUnavailable 额外验证保留 create
事实。MoveProjectionUnavailable 要求已知提交修订处的精确已提交置换，或允许的后续历史，才能
恢复保留的稳定阶段，不产生挂接效果。不声明读取间推送或新鲜度。

导航器为每个规范有序成员显示一行：

```text
[*N] <safe-label>   active
[ N] <safe-label>   inactive
```

`N` 是从一开始的十进制位置，且 `]` 后恰好有一个空格。始终可见的紧凑条使用
`[*N <safe-label>]` 和 `[ N <safe-label>]`。成功选择或已准入 Create 会关闭导航器。
`Ctrl+B w` 将其关闭时，其标题和每一行都消失；紧凑条与 Ready 状态保持不变。关闭期间，在途
刷新会在后台静默继续。

## 渲染与本地化可观测状态

Client 数据中的规范标签永远不会被规范化、去除首尾空白、去重或截断。Presentation 通过将每个
Unicode 控制标量（包括 C0、DEL 和 C1）以及 U+2028/U+2029 替换为 U+FFFD，得到临时的安全
单行字符串。Ratatui 在单行有界 `Rect` 中布局该安全字符串；按终端单元格计算的缓冲区裁剪绝不
写出该区域，也不会拆开宽字符单元格。当紧凑条宽度足以容纳活动标记前缀 `[*N ` 时，横向滚动会
完整露出该前缀，同时保持规范顺序。更窄的 Rect 只渲染经安全裁剪后的前导单元格，不会溢出，也
不会出现控制字节。导航器行会纵向滚动，以保持活动行可见。PTY 见证使用足够宽、能显示完整活动
标记的视口。

`FixedViewportBackend::draw` 是最终的信任边界守卫，因为它调用 `Print(cell.symbol())`。
调用 `Print` 前，它对每个单元格符号应用相同的禁用标量替换。Workbench 标签中的控制符/换行/
C1 字节序列绝不可能成为终端命令。渲染器/后端测试覆盖：重复标签、非空但仅含空白的标签、
C0/ESC/CR/LF/DEL/C1、U+2028/U+2029、组合字符与双宽字符、单列裁剪、许多 Tab 后的活动标记，
以及渲染后仍精确保留的规范数据。

稳定的英文与中文状态字符串如下。普通 Move 准入拒绝保留现有 notice，不会重新获取邮箱锁；
本地正忙一行适用于其他结构准入路径。

| 语义状态                   | en-US                                             | zh-CN                                 |
| -------------------------- | ------------------------------------------------- | ------------------------------------- |
| 创建中                     | `Creating Tab…`                                   | `正在创建 Tab…`                       |
| Create 提交前明确拒绝      | `Tab not created · previous Tab unchanged`        | `未创建 Tab · 原 Tab 未更改`          |
| 刷新中                     | `Refreshing Tabs…`                                | `正在刷新 Tab…`                       |
| 刷新成功                   | `Tabs refreshed · revision {R}`                   | `Tab 已刷新 · 修订 {R}`               |
| 刷新失败                   | `Tab refresh failed · showing revision {R}`       | `Tab 刷新失败 · 正在显示修订 {R}`     |
| 本地准入拒绝               | `Client busy · no command sent`                   | `Client 正忙 · 未发送命令`            |
| 数字选择越界               | `No Tab at position {N}`                          | `第 {N} 个 Tab 不存在`                |
| 切换中                     | `Tab {N} · Connecting…`                           | `Tab {N} · 正在连接…`                 |
| 首帧前的 NeedSnapshot      | `Tab {N} · Connecting · waiting for full frame`   | `Tab {N} · 正在连接 · 等待完整帧`     |
| 首帧前的 ProjectionBlocked | `Tab {N} · Connecting · display blocked`          | `Tab {N} · 正在连接 · 显示已阻塞`     |
| Ready 控制模式             | `Tab {N} · Ready · controlling`                   | `Tab {N} · 已就绪 · 可控制`           |
| Ready 只读模式             | `Tab {N} · Ready · view only`                     | `Tab {N} · 已就绪 · 仅查看`           |
| 已提交快照不可用           | `Created Tab committed · refresh required`        | `已提交创建的 Tab · 需要刷新`         |
| 已知终端等待创建           | `Tab {N} · terminal creation pending · no replay` | `Tab {N} · 终端创建等待中 · 不会重放` |
| 已知终端不可用             | `Tab {N} · terminal unavailable`                  | `Tab {N} · 终端不可用`                |
| 已知终端创建失败           | `Tab {N} · terminal creation failed`              | `Tab {N} · 终端创建失败`              |
| 已知终端结果未知           | `Tab {N} · terminal result unknown · no replay`   | `Tab {N} · 终端结果未知 · 不会重放`   |

刷新状态在打开的导航器内渲染。关闭时仍保留，但在视觉上静默。因此，普通状态栏提供的 PTY
证据是一个活动紧凑标记与精确 Ready 字符串的合取；绝不要求绘制临时的 Creating/Refreshing
状态。

明确的 Create 提交前结算永远不创建定时器驱动阶段。UI 消费类型化结果时，会把原挂接直接精确
恢复到 Ready，并把本地化提示存入 `Ready.notice`；该提示会替代普通 Ready 页脚，直到下一个已
解码的 `Ctrl+B` Client 操作以原子方式清除或替换它。普通终端字节、Poll/帧通信和实际 Resize
观测都不会清除它。Pending 已提交目标保持 SelectedWithoutAttachment，并显示上面的精确
Pending 状态。`Ctrl+B w` 可以刷新规范成员关系，但不会重放 Create、挂接该 Tab，或清除其保留
的 Pending 来源状态。

## Rename 传输适配器

私有 `WorkbenchTabRenameRequest(WorkbenchCommand)` 仅接受 `TabRename`，并保留完整命令，包括请求身份、预期 revision、目标和精确 label。
`LocalClient::submit_workbench_tab_rename(request, deadline)` 使用既有单次 exchange。
它要求命令结果具有原请求身份：`Renamed` 必须恰好为 `checked(expectedRevision + 1)`，
`Unchanged` 必须恰好为 `expectedRevision`，包括最大 revision。
`Renamed` 仅携带 revision，没有可验证的目标或 label 回显。

兼容的提交前拒绝原因恰好为 `site-mismatch`、`revision-mismatch`、
`terminal-context-unavailable`、`tab-unavailable`、`resource-limit`、`revision-exhausted` 和
`request-identity-conflict`；保留其报告的 revision。`denied`、`request-ledger-full` 和
`service-unavailable` 也兼容。其他结果或原因、关联不匹配、畸形响应和不可能的成功 revision
均退役承载连接，并以原请求身份返回 `OutcomeUnknown`。编码失败及任何可能写入前的失败为
`DefinitelyNotSubmitted`；可能交接后的不确定完成会退役连接，且不重试。

deadline 始终为调用方、单次 exchange 和既有启动 deadline 中最早者。
没有 SnapshotRead、Start、Attach、选择、label 修改或请求身份分配。
此确定性分类器映射到既有 `LocalClientSubmission` 的 exchange 和无效结果类别，
不新增状态、时钟或重试语义。适配器测试通过解码检查完整 typed 请求得到保留，覆盖兼容与不兼容结果、
revision 边界，以及写入前与交接后的 deadline 失败。抽象模型不证明 label 字节或 revision 算术。

原生 Rename 命令及 mailbox/UI 流程消费此传输基础。适配器本身不拥有编辑器激活、
成员安装或附件清理。

## Rename 投影证据

`workbench_projection::renamed` 根据捕获的规范基线和已知 Rename revision 验证快照。
精确 context/Tab 必须存在于基线，请求 label 必须不同于其旧 label，提交 revision 必须为
`checked(baseline + 1)`。在该精确 revision，完整投影向量必须等于仅将目标 Tab label
替换为精确请求文本后的基线。其他 label 变化、重排、新增、移除或不可变成员变化均属分歧。

更晚 revision 使用共享的严格规范成员规则。被重命名的 Tab 可以具有后来修改的 label，
也可以缺席；验证结果携带候选投影和可选目标索引。两种情况都不会使历史提交失效。
期望和基线均为借用且保持不变。同 label 的 `Unchanged` 结果不属于此已知提交验证器的范围。

这只是纯投影证据，不证明终端销毁、附件清理或 UI 安装。针对性测试覆盖精确 label 保留、
revision 溢出、提交时禁止的变化、后续演进/移除/Empty，以及重叠身份拒绝。
它不新增状态机或模型声明。原生 Rename 消费者在共享成员安装前使用此证据，
并保留共同清理 owner 的独立证明要求。

## Rename Unicode 草稿缓冲

`state::rename_text::RenameText` 为原生 Rename 草稿保存精确的可编辑 label 文本。
它复制已验证的规范 label，提供文本借用，在 `MAXIMUM_LABEL_LENGTH` 范围内原子追加完整
UTF-8 文本，移除末尾一个 Unicode 标量，并可清空为 Empty。超限追加保留完整旧文本；
追加空文本和清空 Empty 均为无操作。缓冲只使用一个 String，按需统计标量，不维护第二个计数器。

编辑期间允许 Empty。生成 label 调用既有 `WorkbenchLabel::parse`，因此 Empty 不能提交，
仅含空白的文本仍然有效。空白、规范化形式、组合标量、补充平面标量和 schema 允许的控制字符
均精确保留；显示净化不会修改缓冲。移除一个标量不是字素删除或物理 Backspace 契约。
返回的 label 独立拥有文本，不受后续缓冲编辑影响。

测试覆盖精确文本、Empty、511/512/513 标量边界、原子拒绝、标量删除和独立返回的 label。
此确定性有界 String 变换不引入时钟或状态机模型。它不解析原始输入，不建立草稿身份或绘制资格，
不分配请求、不准入命令，也不执行 I/O。state-owned RenameDraft 与实际输入 dispatcher
消费此缓冲，命令准入仍由 mailbox transaction 拥有。

## 原生 Rename 消费流程

Rename 提交消费者使用 state 和 mailbox 中具体共享的 Move/Rename pending mutation。
在既有 mailbox 锁内，它重新规划精确可见草稿，准备 request 和 serial 候选，仅在所有拒绝
条件通过后安装 operation、marker 和 cursor。typed 结果关联与 flight 阶段在 state/成员
联合提交前校验。worker 仅使用一次存储的请求；已知提交先于 Snapshot 保留，并跨传输退役
保持。持久输入 dispatcher 与实际 renderer 消费这些域方法。

私有 Rename operation 拥有一个不可变 typed request、捕获的 draft intent 和原始 deadline。
结果 preparation 独立检查未变文本/基线 revision 与已变文本/checked 后继语义，再调用严格
Rename projector。投影校验失败或 Snapshot 不可用时，仍可保留有效已知提交。该 operation
preparation 由实际 mailbox/executor/UI 流程消费；preparation 本身不分配身份、不提交命令、不安装
成员关系，也不新建 flight 或 cleanup owner。

Rename 的 last-settled 回执遵循 Move 的最新 UI 已消费 known commit，而非 Create 的最新成功
成员安装。B 准入、排队、拒绝、precommit 和 no-op 均保留 A。worker 已知 B 与 state A 共存，
直到精确 B UI 消费在任何可能失败的成员 continuation 前保留 B。投影不可用保留 B、recovery B
和历史缓存。A 的 worker 引用保留至精确 ACK；单 executor 在该 ACK 待处理时不能执行 B。
因此 UI 已消费 known B 的替换不会留下合法未 ACK 的 A 引用。不引入永久旧请求 ledger。

及时 typed 放置后，先纯校验候选，再考虑成员预算。需要附件转换的有效候选在纯校验后、
reply 锁前仅采样一次，forward cutoff 为 45 秒、total cutoff 为 50 秒。共同 owner 在清理、
设置和延迟 UI 消费中保存该精确 pair，绝不续期。保留当前 key 时不分配新成员预算。

state 拥有的 Rename 草稿消费 `RenameText`，捕获完整 canonical 基线及精确选择的附件 generation，
并暴露 opaque 绘制 token。它不分配命令身份或 deadline。精确成功绘制赋予提交资格；缺席 token
撤销资格。state 在返回提交 intent 前独立复核权威，并在草稿存在时拒绝结构准入。
显式取消和退役销毁草稿。输入、mailbox 和 UI 消费者将这些 state 操作接到键盘入口与
精确 token 提交，而不允许 decoder 构造请求。
`state/tests/rename_draft.rs` 检验可见及已撤销 token、精确未变文本 intent、原子文本上限错误、
显式修正、不完整输入、独立基线/generation 失效、取消、退役及双向 state 准入。
实际 mailbox 测试覆盖容量、cursor/serial 耗尽、固定 cutoff 拒绝、精确 ACK 与 worker 丢失证据保留。

`Ctrl+B ,` 在存活且稳定的阶段为当前上下文中精确选中的 Tab 打开 state-owned 草稿。
捕获规范基线、Site/context/Tab 和附件 generation，不分配请求或 deadline。
Empty、结构/恢复 flight 和 Close/Resume 确认期间拒绝进入；草稿与 Close 确认互斥。
已应用的权威或选择变化使草稿失效；Enter 原子重验捕获元组，绝不把文本改投其他目标。

草稿使用有界 `RenameText` 操作，保留 schema 有效的精确文本。Enter 必须已有其精确不透明
token 的首次成功绘制资格。缺失/失败绘制、隐藏、视口失效或草稿替换撤销资格；
后续文本编辑不要求每个标量重新绘制。显示净化与返回的精确 label 分离。
输入错误仅由显式解码的 Backspace 或 Ctrl+U 清除，包括 Empty；两者仍执行 pop 或 clear。
成功键入/粘贴、忽略序列、失败编辑和 Enter 均不清错。本地化错误提示指明这两个修正键。
取消销毁草稿及其错误。

编辑器由既有单一输入 reader 和终端模式 owner 处理。ESC 等待 50ms，CSI/SS3 上限为
32 字节，碎片化 UTF-8 最多保留三个前缀字节。括号粘贴在 label 剩余容量内原子追加，
并跨读取识别六字节终止符。负载控制字符不提交、不取消、不清错，也不执行前缀动作。
超限或无效粘贴一直排空到终止符，不接受部分前缀。粘贴模式启用失败拒绝进入。
持久输入解析器跨读取/tick 保持这些状态。

取消/提交保留已识别分帧的排空状态；完整结束后才允许 50ms 静默 guard 恢复普通输入，
每个丢弃字节重置该 guard。在相等边界，先到期 guard，再处理新采样输入。
已识别分帧可跨任意间隔保持隔离；ESC 分类及静默交接到期后才补齐的起始符视为新输入。
不保证任意迟到的未分帧字节来源可证明；没有额外恢复手势、第二个 reader 或传输预算续期。

Enter 在锁前捕获一次固定 50 秒预算。阶段、绘制 token、完整有效输入、捕获权威、容量、
cursor 和互斥性全部验证后才原子准入请求/marker。既有结构变更槽使用具体 Move/Rename
枚举，与 Close flight 双向互斥。编辑、取消和拒绝不消耗请求身份；相同文本仍提交一次由
Server 决定。纯 Rename 保留附件及 Ready 事实。

`Unchanged` 要求基线 revision 和相同请求文本，不读取快照。`Renamed` 要求不同文本及
checked 后继；先保留精确操作与已知 revision，再以同一预算读取一次快照。结果没有目标回显。
精确 revision 要求仅 label 变化的投影；更晚严格成员快照可以覆盖或移除目标。
确定的命令提交前结果在 UI 精确结算后清 flight；只有提交后投影不可用保留恢复 fence 和
历史缓存，同时同目标 effects 可继续可用。显式 Refresh 使用自己的固定预算，绝不重放 Rename。

及时放置精确 typed 结果即结束传输预算，UI 消费不限时。区分放置、UI 消费、executor 确认、
worker 返回和 join。已知提交与不确定提交均保留精确操作 key；A 消费后准入 B，迟到 A 确认
不能清除 B。旧 worker 返回不能证明当前 worker 返回。
单 executor 在交接 B 或将 B 完成为确定未提交之前，均须等待 A 的确认；B 的 UI 结算不得覆盖
待处理的 A 确认。排队或确定未提交后的退役不得虚构已提交不确定性。

验证后候选通过共享独占 state/mailbox 事务安装。快照缺席不是清理证明；共享安装器在附件事实
仍存在时拒绝替换活动目标，Rename 消费者必须使用共同清理 continuation 及其存储的
deadline owner。Rename 不得绕过该守卫，也不得仅凭成员关系获得新 Ready。

Rename 生命周期模型必须先于依赖运行时。其有界抽象覆盖精确草稿资格、两次 Rename 准入、
一个竞争结构 flight、原始 exchange cutoff/typed 放置、带 key 的历史事实、一个迟到回执和
当前 worker join。输入字节、标量算术、八槽存储、具体 label 和完整成员向量属于可执行细化义务。
共享清理/设置证明来自外部 owner；公平性仅覆盖可用精确完成的本地消费和既有本地到期，
不强制产生 Host 清理或 Attach 成功。确定性 parser 测试及实际 NativeUi/socket 测试细化输入、
operation 和成员接缝。Linux cohort 进程测试
`native_rename_visible_editor_updates_canonical_label_and_preserves_live_shell` 还驱动实际可见
编辑器，在提交及显式 Refresh 后观察已重命名的规范 Tab，并验证同一 shell PID、环境标记及
可用输入输出。该进程见证不独立证明未重新附着，也不证明其他操作系统。
既有 Move 模型回执不是 Rename 证据。

成功 Rename entry 通过 `ClientView.commit` 关闭已打开的 navigator；拒绝保留原 overlay。
只有完整本地化编辑器实际绘制后才确认初始 token。同一 renderer receipt 携带相互独立的
可选 Close 与 Rename token；即使终端列数未变，外层 viewport 变化也在输入前撤销 Rename 资格。
既有 terminal mode lease 在 entry 前启用 bracketed paste，并在已识别 drain 和 quiet handoff
期间保持该模式；启用失败不会创建草稿或命令。

`terminal_session/tests/rename_interaction.rs` 通过本地 scripted socket 驱动持久 dispatcher、
实际 renderer、NativeUi、mailbox 和 executor。它覆盖双语言、未显示时 Enter、sticky 输入错误、
精确最大标量粘贴、取消 drain、viewport 失效、navigator 关闭、权威 no-op、变更投影、显式恢复、
畸形已知投影、歧义 exchange 与提交后 Snapshot 丢失。后续目标移除通过共同 Detach 后进入
Empty 或 successor Attach；明确不可用的 successor 不会成为 Ready。
这些测试拒绝未请求的 Start、Close 或重复 Rename 请求。

### Rename 生命周期模型边界与证据

`NativeClientRenameLifecycle` 包含一个固定 Site/context、两个目标代表、最多三次草稿打开和
两次 Rename 准入。四个请求身份包含同一 cursor 分配的 Snapshot/Refresh；每次 Rename
最多建模一次恢复准入。相同/不同文本、有效/无效/不完整输入，以及截止前/已到期/预算已结束
时钟类别代替具体字符串与时间。Ready 和 selected-without-attachment 配置覆盖两类 effects；
RetryableAttachUnavailable 映射到后者，因为该纯命令不尝试 Attach。
一个旧 worker 与一个当前 worker 是不同代表，不建模重连。
竞争的状态观察、结构工作和 Close 确认是不同类别：状态观察可与未提交草稿并存，确认则不能。
仅基线变化与仅选择变化独立检验两个捕获权威守卫。选择变化表示外部 owner 已应用的
非 Ready 状态转换，不是 Rename 产生的新附件证明。

| 所属义务                   | 模型断言或转换                                                                                                                   | 仍需的具体细化                                                                           |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| 精确可见草稿和显式错误修正 | `AdmissionRequiresExactDraft`、`OnlyExplicitCorrectionClearsFault`、`CapturedOperationNeverRetargets`                            | 实际绘制回执、过期基线/选择、Backspace/Ctrl+U 和碎片输入。                               |
| 单 flight 和单次命令交接   | `StructuralFlightsExclusive`、`NoRepeatHandoff`、`LocalDraftEditsDoNotAllocate`                                                  | 真实八槽/因果存储、cursor 无消耗拒绝、与 Move/Close/Refresh/Create/Switch 双向互斥。     |
| 已知提交和固定预算         | `CurrentCommitFacts`、`NoopUsesSameLabel`、`KnownCommitNeverRegresses`、`ExistingCutoffNeverRenews`                              | 精确仅 label 变化/后续投影、checked revision，以及覆盖 Snapshot 和放置的原始 50 秒预算。 |
| 不限时精确消费和迟到确认   | `PendingUiIsUntimed`、`TypedResultNamesCurrent`、`AcknowledgmentPreservesCurrentFlight`, `PendingAcknowledgmentIsNotOverwritten` | A 消费/B 准入/迟到 A 确认、精确 UI/worker key，且不替换 flight。                         |
| 退役及外部成员证据         | `NoInventedUncertainty`、`JoinedHasCurrentReturn`、`MembershipInstallNeedsExactOwner`、`AvailableOwnerResultIsUntimed`           | 可能交接与确定未提交、当前 worker 返回、精确清理/Empty 交接及 deadline owner。           |

成员完成输入表示 owner 已产生的精确结果。该结果可用即结束 owner 等待预算，本地消费
可以更晚发生。公平性不强制产生该结果；没有结果时，既有本地 owner 到期可以退役等待。
模型绝不从移除目标的快照或 owner 成员结果构造新 Ready。

固定 TLC 1.7.4 已通过两个注册配置的安全性、时序检查和全部 42 项必需语义动作覆盖，
使用一个 worker、`--jobs 1` 和未改变的 180 秒超时。两个完整图的深度均为 37，排队状态为零。
这些是有界模型回执。

| 配置                          | 生成 / 不同状态数   | 模块加有效配置 SHA-256                                             |
| ----------------------------- | ------------------- | ------------------------------------------------------------------ |
| `ready`                       | 2,140,506 / 318,440 | `a146d4ef78797c84f6750ad7dc6b661a4132f9c77d9cf33436e02358e29555b1` |
| `selected-without-attachment` | 2,031,640 / 304,420 | `0de583638a9b4ee9c894d410609f74a8b5a3dffc99897b73ab33278878f0a454` |

```sh
node scripts/tla/run-models.mjs --models native-client-rename-lifecycle --jobs 1
```

## 有副作用的适配器契约

在快照提交入口旁添加不透明的 `WorkbenchTabCreateRequest(WorkbenchCommand)`。它的纯
`TryFrom<WorkbenchCommand>` 只接受 `TabCreate`，并将其余每个已取得所有权的命令原样作为
转换错误返回；构造过程不修改状态，也不执行 I/O。唯一有副作用的入口是
`LocalClient::submit_workbench_tab_create(request, caller_deadline: Instant)`。不存在通用的
Workbench 命令提交方法，也不存在本地臆造的“不支持操作”线协议结果。交付前，该方法编码已经
验证的请求。交付后，它要求结果为 `workbench.command.result`、请求 ID 精确匹配，并且结果与
操作兼容。对于 `tab.create`，`denied` 和封闭枚举
`WorkbenchCommandUnavailableReason` 的每个值都兼容。兼容的 `rejected` 原因恰好是
`site-mismatch`、`revision-mismatch`、`terminal-context-unavailable`、`resource-limit`、
`identity-exhausted`、`revision-exhausted` 和 `request-identity-conflict`；
`project-unavailable` 与 `workspace-unavailable` 和该操作不兼容。Project/workspace 结果变体与
`unchanged` 也不兼容；只有 TabCreatePending/TabCreated 已提交变体匹配 `tab.create`。

对于已提交的 Tab 结果，适配器还要求已捕获上下文 ID、精确等于 `expected + 1` 的修订号；对于
Started，还要求精确的请求尺寸。它返回明确的 `DefinitelyNotSubmitted`、`DefinitePrecommit`、
`Committed` 或 `OutcomeUnknown` 分类。编码失败或未写入失败属于明确未提交。只要任何写入可能
已经开始，写入/读取/截止时间错误、格式错误/重复/截断响应、非预期响应类型、关联不匹配、操作
不匹配、错误的已提交字段或错误的 Started 尺寸都会使载体退役，并归类为 OutcomeUnknown。
状态机绝不会把它们转换成安全重试。

为终端查看器、终端控制和 SnapshotRead 添加内部的调用方截止时间形式。每一种都复用
`exchange_once`，并将其普通单次交换截止时间限制为调用方提供的截止时间与任何现有启动上限中
较早者；现有普通方法保留当前的自有预算行为。`next_operation_deadline` 读取现有注入的单调
时钟，绝不单独调用 `Instant::now`。Switch/Create 只捕获一次
`total = sampled_at.checked_add(50s)` 与 `forward_cutoff = total - 5s`：每次前向交换/重试都使用固定的
截止点，精确清理 Detach 及其关联的 UI 结算则使用固定的总截止时间。两者都不续期。Refresh 传入其唯一固定的
50 秒传输截止时间；启动保留现有启动上限。

上述截止时间以取得邮箱锁之前的时钟采样为起点。私有且可克隆的 `LocalOperationClock`
封装 `Arc<dyn MonotonicClock>`；私有时钟 trait 要求 `Debug + Send + Sync`。在把
`LocalClient` 移入工作线程前克隆同一句柄，使 UI 准入与 I/O 共享同一个注入的时钟源。
只有测试步进时钟的接收端需要 `Mutex`；生产时钟不加锁。UI 准入先采样再加锁，计算
`total = sampled_at.checked_add(50s)`，并将固定的截止时间与标记、请求身份、代次一起原子提交。
溢出会在任何状态变更前拒绝。因此，成功准入获得的保守预算最多为 50 秒，其中包含等待锁的时间；
不在持有邮箱锁时持有时钟锁或执行 I/O。私有的 `WorkbenchTabCreatePrecommit` 与
`WorkbenchTabCreateCommit` 载荷枚举仅包含上述兼容的提交前与已提交变体，不接纳无关命令结果。
这些表示方式不增加线协议结果，也不改变模型中的抽象时钟或结果分类。

受截止时间约束的邮箱事件操作在互斥锁外采样共享时钟，并使用非阻塞方式取得锁。发生竞争后，
它们先重新采样再尝试，并在取得的锁下重新校验精确事件键与固定截止时间。范围受限的
`yield_now` 重试处理互斥锁竞争；队列、事件和 Busy 等待继续使用条件变量。不能先在锁前采样，
再阻塞取锁，并在普通锁等待跨过截止时间后仍授权结算。注入时钟测试在持锁期间跨过期限，断言
结算回调从未运行。最终采样后的调度暂停及内核停顿仍属于环境限制；普通锁竞争不能使时间预算
续期或延长。

快照读取仍然没有副作用，但格式/类型/关联/传输失败也会使共享载体退役。明确的 denied/unavailable
可以让它继续存活。所有 ID/绑定都通过经过检查的精确文本一致性在 OBWP 与 OBCS 之间转换；两个
协议 crate 均不依赖另一个。

## 声明/证据矩阵

下方矩阵概括当前 Client 证据。上方 Move 专属记录及下方细化检查扩展原有的 Create/选择/Refresh
模型；仅有 append-only 检查不能建立重排保留声明。

| 声明                                                                                   | 最低层级证据                                                                                      | 补充见证                                              | 限制                                               |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | -------------------------------------------------- |
| 严格的 0..256 投影与保留身份的刷新                                                     | 穷尽的分区/单元用例和从语料库派生的固定样例                                                       | 生成的 create/rename/move 历史及精确提交置换检查      | 更高修订允许 Tab 重排及改名；不可变成员事实保留。  |
| 选择的循环/边界/同目标/重试与原子拒绝                                                  | 纯选择表测试                                                                                      | 使用稳定种子、语义缩减和重放文本生成的邮箱轨迹        | 只报告有限的生成轨迹规模，并非普适证明。           |
| Create 在提交前排空旧 Poll/Feedback，提交前拒绝会恢复                                  | `NativeClientCreateBarrier.tla` 加模拟载体轨迹                                                    | 确定性的旧帧/绘制/反馈检查点轨迹                      | TLA 抽象字节、时钟、尺寸和渲染。                   |
| 提交后先验证/暂存 Snapshot 候选，再精确清理旧目标并原子选择；只有存活的 Started 才挂接 | Create 模型已检查；实际 UI 双 plan 安装、candidate-first carrier 和 generated mailbox traces      | 每个阻塞响应处的模拟载体                              | 载体退役清理不等同于声称 Detach 成功或候选已安装。 |
| 快速切换的清理/代次/首帧/尺寸门                                                        | `NativeTabSwitch.tla` 加确定性核心轨迹                                                            | 真实 PTY 活动标记与 Ready 检查点                      | 该模型与 Create/刷新分离，并抽象真实像素。         |
| 刷新的单一待处理代次栅栏/身份/活动项保留与导航器开关                                   | `NativeClientRefreshFence.tla` 加确定性过时事件测试                                               | 双 Client/进程刷新见证                                | 不声明推送或读取之间的新鲜度。                     |
| Move 保留所选终端及精确已知提交                                                        | `NativeClientMoveBarrier.tla`、邮箱/状态、worker 及真实 NativeUi/外层退役轨迹                     | 两项 Linux cohort Move 场景                           | PTY shell 连续性本身不证明查看器未重新附着。       |
| 整个建立流程终止                                                                       | 在 Empty/NeedSnapshot/Blocked/Busy 和清理预留边界使用模拟单调时钟                                 | PTY 使用具名的 Ready/unavailable 检查点，绝不休眠等待 | 不证明 Host 最终会成功响应。                       |
| 安全且精确的标签渲染                                                                   | 纯净化器/布局/后端字节测试                                                                        | PTY 数字标签与活动标记                                | 不包含操作系统字体/像素保真度。                    |
| 平台依赖边界                                                                           | Windows 目标上针对帮助/版本/不可用情形的 `check/test` 加 Cargo 元数据断言；Unix 构建/测试覆盖 TUI | Linux 真实 PTY                                        | 不声明 Windows TUI。                               |

有状态生成测试使用包内的确定性生成器，而不增加新的运行时依赖。测试会记录稳定种子语料、最大
轨迹长度、操作/结果覆盖计数器和规范序列化轨迹。发生失败时，测试框架先移除操作片段，再缩小
索引、队列占用、代次、结果和尺寸值；每个最小化轨迹都可直接重放，并成为具名的确定性回归测试。
覆盖断言要求涵盖 Create/提交前/每一种已提交类别、满容量准入、Poll 前反馈、刷新过时/当前结果、
每个阻塞边界的切换取代、控制器繁忙、NeedSnapshot、Blocked、超时、清理和载体退役。一个具名的
重复 Create 覆盖类别必须在同一 Session 内执行至少两次用户操作。它覆盖：从 Ready 到第二次
`c`、新的已检查身份和已消耗代次、保留每个 Tab 的来源状态、拒绝复用第一个已提交元组，以及
明确提交前拒绝之后的重新进入。

Move 邮箱在 `MoveOperation` 中捕获一个请求 ID、不变的 attachment key、
baseline、方向和 deadline。结构栅栏覆盖排队、执行和等待 UI 三个阶段，只在精确类型化 UI
settlement 被存入时释放。及时存入结果后，UI 消费不再受 deadline 限制。准入对 deadline、
容量、身份和目标失败均不消耗身份或 attachment generation。确定性邮箱测试覆盖这些边界，
包括锁竞争以及 worker 消费第一次确认前第二次 Move 已排队的情况。

`state/move_tab.rs` 在不变的稳定 attachment phase 之上保留待处理操作、已提交恢复及精确
已结算提交记录。`MoveProjectionUnavailable` 是显式显示/状态种类；其恢复记录阻止 Create、
Switch 和 Move，同时保留 Ready 输入、Poll、Resize、controller 及历史帧状态。原始
`Ctrl+B w` 准入具有新预算的纯 Snapshot 恢复，验证原始 Move 期望，并按选中 ID 重映射，
不分配 attachment generation 或建立目标。外层 worker join 同时保留 Create 和 Move 的已知提交交接。

六个聚焦状态测试覆盖控制及只读历史 Ready、两种未挂接 phase、非首位选择、明确拒绝、单标签
无变化、损坏及更高 revision 恢复。组合规范状态/邮箱轨迹结算 Move A，在 worker 读取 A 确认前
排队 B，然后退役，仅识别 A 的精确提交且不替换 B。真实 NativeUi/carrier 测试驱动原始 Move 和
`w` 按键、渲染重排后的活动位置、在显式不可用期间执行输入/Poll/Resize、拒绝结构动作，恢复时
仅允许 SnapshotRead。九个 worker transport 测试另行检查截止投递、无 deadline 的 UI 结算、
精确请求分配、保留查看器权限及跨传输/退役切点的已知提交。
真实 NativeUi 竞争回归在准入过期后继续持有邮箱锁，要求拒绝在解锁前返回，并保留 notice、
查看器和身份游标。因此普通 Move 拒绝不会再次获取锁。独立外层 TerminalSession 测试中断
提交后的 Snapshot 读取、join worker，并验证最终结算后保留的精确 Move 提交及首个退役原因。

在运行时代码 `f2467e8e` 上，原生二进制测试通过 311 项、忽略 1 项；完整 `local_startup_process`
目标在 WSL Linux 上通过全部 23 项。其两项 Move 场景覆盖三个 Tab 的六次移动、双向及边界循环、
不同 shell PID、新输出和 Resize、独立的另一 Client 选择与显式刷新，以及重新打开后选择规范首项
并再次访问其余终端。有序完整帧观察器要求恰好一个活动标记及其精确 Ready 页脚。
这些进程检查建立可观察的 shell 连续性；worker 和 UI 轨迹另行建立附着保留声明。
它们不证明其他操作系统、安装、重启恢复或 Close。

## 模型范围

下列范围记录已检查的 Create/选择/Refresh 基线及其 Move 细化。现有 action 义务仍必须满足；
每个模型保留其声明的有限边界。

`NativeClientCreateBarrier.tla/.cfg` 检查从已稳定 Ready 锚点或上文类型化 Empty 来源出发的任意一次 Create 屏障：有界
FIFO 排空、旧在途请求的因果反馈、副作用准入冻结、提交前恢复、保留提交后再读取 Snapshot、
验证候选暂存后才精确清理旧挂接、原子选择安装，以及仅 Started 才 Attach 的抽象。一次更晚响应
可以移动或移除返回成员。返回成员保留历史代次 1；存活的冻结序号后备项使用代次 2，Empty 则不
安装活动成员。明确的 Snapshot 不可用会在结构栅栏后保留精确旧核心。模型有意只包含一对
Old/New，并不证明连续 Create 的重新进入、任意成员移除、显式 ProjectionUnavailable 恢复或逐
操作字段重置；上述具名的至少两次操作生成/重放轨迹和 `NativeClientMembershipRemoval` 负责
这些声明。
`NativeTabSwitch.tla/.cfg` 检查暂存候选安装、分离的意图与 successor 分配、definite Unavailable
时有条件的精确 stable-anchor 恢复、handoff 前 Gone、显式的 Detach 请求/结果顺序、查看器/控制器唯一性、被取代挂接的
清理、Attach 后失败/截止时间清理、歧义退役、代次栅栏、首帧反馈和最终尺寸追赶。
`NativeClientRefreshFence.tla/.cfg` 检查导航器/打开操作的复合准入、最多一个待处理刷新、重新
打开但不读取、拒绝时完整保留核心状态、不复用刷新代次、精确应用待处理结果、只追加缓存替换、
保留有缓存支撑的活动项、防御性隐藏过时完成结果，以及一条抽象的已接受 Input/Resize 记录；该
记录在 UI 语义结算期间保持排队，且只能在其后执行。该抽象控制检查执行器栅栏；确定性归约器与
脚本化载体测试会区分 Input 与 Resize，并覆盖其具体 FIFO 结果。

这些模型不声明 JSON/关联验证、精确标签或像素值、真实时间、渲染器安全、操作系统调度或 OBCS
Busy 重试时序；这些边界分别由确定性适配器、归约器、模拟时钟、渲染器和 PTY 测试负责。每个
TLC 结果只适用于其具名的 Create、切换或刷新抽象；不会将任何一个用于另外两个屏障。

修正后的 CreateBarrier Ready/Empty 模型族保留 `MaxQueue = 2`、尺寸 `0..1`，以及一次 Create；
候选是精确修订 2，或一次更晚修订 3 的重排/移除。候选代次 2 是预备后继，只有
`InstallValidatedCandidate` 才消费它。安装前退役保留已提交的游标 1；Empty 同样保留游标 1，
且没有活动代次。known-Closed Gone 动作以外部已验证的精确旧挂接能力凭证作为输入；本模型
不获取该证明，也不建模独立的 Host 身份。成员缺失并不提供这种证明，在途 Detach 也不能走此
捷径。规范 Move/移除的交错发生在暂存之前；之后检查候选不可变性，但此有界模型没有更晚的
规范写入动作。安装动作属于未结算集合并具有弱公平性；截止时间
公平性只保证真实的稳定结果，不保证清理或 Attach 成功。显式恢复和可挂接后备项的组合仍由
`NativeClientMembershipRemoval` 与运行时细化负责。

TLA+ 1.7.4 使用一个 worker 检查了两个修正后的配置，队列剩余状态均为零，模型族合计覆盖全部
53 个要求的动作名称。所有原有失败和到期动作均保留；`InstallValidatedCandidate` 替代过时的
`CommitEmptyWithoutOldDetach` 动作。

| CreateBarrier 变体 | 有效模型/配置 SHA-256                                              | 生成状态 | 独立状态 | 深度 |
| ------------------ | ------------------------------------------------------------------ | -------- | -------- | ---- |
| Ready              | `da47bdb693b77398139b8825de235f9c0f4a60f2489693a5153c03619d8b2318` | 20,872   | 6,784    | 27   |
| Empty              | `a277a6e320a5239fa0161559bea4329ecb66eac0f4fd947b04b98b2368de6c90` | 3,704    | 1,267    | 23   |

Create 将旧查看器在预留期内的清理完成建模为退役，同时保留已知提交、已暂存候选及旧诊断选择；
该路径不能安装或发布替换选择。Switch 将在途旧 Detach 跨过前向截止点建模为同一精确请求进入清理，
随后按当前或被取代状态结算。弱公平作用于前向到期动作的并集：Busy 阶段变化不能重启或无限
规避同一个固定截止时间。精确的 UI 放入、消费、回复放入时的等于截止点，以及迟到回复消费，
仍由确定性的邮箱及脚本化载体证据负责。
前向预算到期后的当前安装前清理会退役；精确的被取代清理可以把控制权交给较新标记。
Switch 模型记录当前工作代次是否已经尝试 Attach。新标记重置这一事实，Attach Busy 则保留它。
已经尝试 Attach 后到期会退役，不会使用尚未尝试时的入口结算；要求覆盖的动作包含这一区分。
Refresh
活性属性假定存活的 UI 事件循环具备弱公平性：类型化结果一旦占用事件槽，传输截止时间就已结束，
UI 语义结算没有墙上时钟界限。Exit 或退役会唤醒等待中的执行器；如果两者都没有发生，且不存在
UI 公平性，本设计不作最终结算声明。

`NativeClientMoveBarrier.tla/.cfg` 单独检查从 Ready、SelectedWithoutAttachment 或
RetryableAttachUnavailable 出发的一次用户 Move，活动 A 最初位于三个成员的首位、中间或末位；
Ready 单成员配置覆盖无操作。其转换在邮箱准入前采样经过溢出检查的截止时间，准入拒绝
保留核心状态，同时捕获请求身份与基线，排空更早的排队/Poll/绘制/Feedback 工作，且最多
提交一次 Move。明确提交这一事实在内部一直保留，跨越提交后 Snapshot 身份耗尽、损坏或
含糊的交换、投递失败、退役及退出；该抽象不要求中间的 UI 提交确认。精确提交版本必须
对应预期排列；一次后续规范 Move 允许取得更高版本。类型化结果必须在固定截止时间前
投递，但精确 UI 消费可发生在此后，并一直持有结构栅栏直到消费。成功应用按 ID 重映射
来源和活动索引，不改变存活 Viewer/controller 或历史 Ready 事实。明确的投影不可用
保留这些事实，并在结构操作中仅准入显式恢复。恢复具有重新采样的预算和新代次，共享
请求游标。其他 Create/Switch/Refresh 在途被抽象为互斥且冻结缓存的操作；其内部行为
及关联 CreateSnapshot 例外仍归另外三个原生模型负责。

有限边界为一次用户 Move、最多两次恢复准入、三个请求 ID、容量为二的队列、0..8 的时钟值
及值为二的预算。符号时钟分区刻画溢出检查、相等即过期及单一不续期的传输截止时间，不
测量真实的 50 秒调度。时间、邮箱释放、传输过期和 UI 消费的弱公平性建立已准入待处理
操作最终结算或退役，不保证恢复最终成功。邮箱竞争抽象为持有/空闲的准入与投递切点；
精确的非阻塞锁调用、字节/关联拒绝、分配、工作线程 join 交接、按键解码、渲染及 OS/PTY
行为需要实现证据。这是已接受 Move 批次的有界模型证据；运行时细化及 Linux cohort 进程验收
具有上方的独立证据。它不是无界并发或排列定理。
一次 Move 的边界不覆盖重复 Move 重入，也不覆盖 UI 结算 Move A 后、工作线程消费 A 的
确认前准入 Move B 的竞争。必需的确定性状态/适配器轨迹须检查：A 的工作线程完成前，B
不能执行或替换保留的提交事实；在此期间退役须识别已结算的 A，同时保留 B 及任何更新事实。
Ready 建模有控制权的挂接；只读 Ready 还需要确定性证据。

## 模型验证

### Close 细化检查

2026-09-08，变更后的 Create 两个配置及成员移除十九个配置分别运行并通过，使用固定的
TLA+ tools 1.7.4、每个配置一个 worker。全部安全性和时序检查完成，状态队列为空；
Create 族覆盖全部 49 个必需动作，成员移除族覆盖全部 48 个必需动作。
下列摘要对应 UTF-8、LF 的模型源和每个变体的有效配置。上方有限范围、细化边界及公平性
限制仍然适用；这些结果不是运行时或完整 Close 交付证据。两个族可一起重跑：

```sh
node scripts/tla/run-models.mjs --models native-client-create-barrier,native-client-membership-removal --jobs 1
```

| 配置                       | 生成状态 / 不同状态 / 深度 | 模型/配置 SHA-256                                                  |
| -------------------------- | -------------------------- | ------------------------------------------------------------------ |
| `create-barrier/ready`     | 13,592 / 4,054 / 26        | `4662ebe9d98dbe674b4f6eb9095adba16d3681dd55a1f31390f440f92311d652` |
| `create-barrier/empty`     | 3,519 / 1,162 / 23         | `f075eeccb739782ad04732ebf20a59f4edb6196872a65491dcd21a3fba263ff1` |
| `refresh-ready-control`    | 421,752 / 144,062 / 24     | `3cc839e14828c3e39963f531c2488db043154cb3737f924d601b8580720777aa` |
| `refresh-ready-readonly`   | 421,752 / 144,062 / 24     | `5cf22dfcbe74e082552598ba222ed5536212b2a7904d4327dd53753438a5d955` |
| `refresh-selected`         | 376,000 / 131,462 / 22     | `7bd0ed28386f95694b40ce8e5ae64d03f58729b7e34c9228af6d08a655b471d9` |
| `refresh-retryable`        | 374,040 / 127,542 / 22     | `45ee4f719fe1de3297b88e06bd57b28ece6da82f3cca1a2528a07c715c11eb0c` |
| `refresh-empty`            | 40,618 / 13,283 / 22       | `e35c4e16cce57e798c0426d06dd3f9621fac5c5aaa4dd661a101e6fb4d6b71a8` |
| `create-ready-control`     | 2,697,042 / 836,248 / 30   | `a99536a3e2885b07eeeebb827b92b2ec65af481483d7dec456760e1d756c2e7f` |
| `create-ready-readonly`    | 2,697,042 / 836,248 / 30   | `e3a9e79e6e6916b1ef055cccffa7514393156b4b9f11cdcd10638377d54a23f3` |
| `move-ready-control`       | 739,636 / 251,162 / 29     | `1c329f1d59c0c60efbb56284cbca691b4bd5640ac74ddac2e4bf17aed21fb2dd` |
| `move-ready-readonly`      | 739,636 / 251,162 / 29     | `61a1ef1c96f11e3bc601730d5baab56d7c14f002943202b6d2496ceb7a90043e` |
| `move-selected`            | 661,250 / 231,002 / 27     | `ec253efcfded19d107f8a41fa51a419af6bb86e45ff6c5bd624aa3234556b0b3` |
| `move-retryable`           | 656,938 / 222,378 / 27     | `2e2f9627805ccafa4d5a17ce5dbaf8e15b0f5fecb17ab77eca8d12d6a71e0269` |
| `switch-ready-control`     | 1,738,552 / 533,328 / 25   | `7333286df772274308ca7d5c3291170787f269ba6dca2e501cdc14b13bae08c7` |
| `switch-ready-readonly`    | 1,738,552 / 533,328 / 25   | `cc065cd7889955163660be460e73cc845a0ecb1f5ef6c688f94b8888fd4f6388` |
| `switch-selected`          | 759,482 / 237,978 / 22     | `d1553ee025912b097283d4c53bc93e558d0c2c894519ccc9c5fcbc2d606eb077` |
| `switch-retryable`         | 779,378 / 232,500 / 22     | `88cbb7206ed8884723f8df0093d62acfe5bb7006b546904c843bc92ca631fc6d` |
| `own-close-ready-control`  | 1,115,736 / 346,322 / 29   | `8985add91e8339c7b78c6d0bdefaa8f19377fdce4f77edc7b57298af447b219a` |
| `own-close-ready-readonly` | 1,115,736 / 346,322 / 29   | `4cb64ac9656b21ded2b10cd67016786f91ef6a9ce68012701097deab13fcbfe4` |
| `own-close-selected`       | 945,468 / 306,758 / 27     | `8b08a06211580c24d31e5e79374980a821374bd7d49822e7fbdefdc12b30dd74` |
| `own-close-retryable`      | 925,868 / 285,198 / 27     | `5ae9e77eb7f67c286b842776709681bcb3fcebbdac8d31a62b4e1c2a9302e0ea` |

### Switch 成员安装证据

确定性测试通过实际顺序 executor 与脚本化 carrier 覆盖先 Snapshot 后清理、目标存活／后备／
Empty 安装、新 successor key 和原始截止时间。独立的生产 `NativeUi` 测试覆盖同锁 cleanup
receipt、取代及过期。旧 Poll 返回与已取走 Feedback 两个切点都要求精确丢弃事实；禁用任一
producer 都会使其回归测试失败。没有 frame 的 Busy 保留恢复资格，准入测试覆盖事件／cell 的
真实丢弃、缺失回调、拒绝时保留、精确 lineage 传递与 queued-to-active 保留。脚本化 UI driver
不证明终端渲染或真实 PTY 进程；下方有限模型结果保持不变。
实际 worker 还会把及时返回的 Unavailable Snapshot 保留到原总截止时间之后的 UI 结算；
若在放入后使用有截止点的 reply 等待，该确定性回归就会失败。

使用固定的 TLA+ tools 1.7.4 运行
`node scripts/tla/run-models.mjs --models native-tab-switch --jobs 1`。
下列四个配置均完成安全及最终时序检查，队列为空，整个配置族的全部 77 个 required action 均有
正生成覆盖，包括两次完成切换的见证。每个配置包含三个 Tab、其中两个可挂接、两次准入意图、
`MaxQueue = 3` 和 `MaxGeometry = 1`；`two-switch` 使用 `MaxGeneration = 4`，`exhaustion`
使用二。固定验证超时为每配置 600,000 ms，使用两个 worker。这些结果仅支持上文描述的有限
Switch 接口，不证明成员向量验证、运行时 anchor proof 保留、数值截止时间或真实 carrier/UI 行为。

| 配置                  | 生成状态 / 不同状态 / 深度 | 模型/配置 SHA-256                                                  |
| --------------------- | -------------------------- | ------------------------------------------------------------------ |
| `two-switch-control`  | 8,790,448 / 2,220,844 / 69 | `946b0ac5cab403701cb53254f28087375cc6bd57869b1046cb968cfcea26682c` |
| `two-switch-readonly` | 2,306,238 / 567,200 / 65   | `ad32026f23e6c5d2b783f954d4ecf6382093408cad40f5066d049ff71b149660` |
| `exhaustion-control`  | 355,697 / 97,340 / 44      | `6186c9818a558457bd0399becc76d699f16fe2fab099f06482b79a8328b43b87` |
| `exhaustion-readonly` | 112,009 / 30,832 / 40      | `06e0c19c36637188962a091f6b5d6db1a815d2b1bda3271f06be8c0ce5752ebc` |

### 较早的无移除证据

从仓库根目录运行已注册的 Client 检查：

```sh
node scripts/tla/run-models.mjs --models native-client-create-barrier,native-client-refresh-fence,native-tab-switch --jobs 1
```

下列 Move 前配置使用固定的 TLA+ tools 1.7.4、每个模型一个 worker 完成检查，时序性质检查已结束，
状态队列均为空，要求覆盖的 116 个语义动作均生成了后继状态。模型/配置摘要标识本次
注册检查的确切输入；前文列出的范围和公平性限制仍然适用。这些哈希/计数是基线证据，不是 Move
证据；依赖集成前，模型输入变更要求另外报告检查及哈希。

| 模型                        | 有限边界                                                                         | 生成状态 / 不同状态 / 深度 | 模型/配置 SHA-256                                                  |
| --------------------------- | -------------------------------------------------------------------------------- | -------------------------- | ------------------------------------------------------------------ |
| `NativeClientCreateBarrier` | `MaxQueue = 2`；一对 Old/New                                                     | 2,717 / 842 / 24           | `38629543117f6ebdb1db4b84f01ec563a197fe2d8ad4f3c25b80b65c3e005368` |
| `NativeClientRefreshFence`  | `MaxQueue = 1`、`MaxRefreshGeneration = 2`                                       | 56,530 / 14,710 / 20       | `01a82fe7e298c1cae9561e7c3a30d234874fb15f5df4c7b33e99199a5cfc31f2` |
| `NativeTabSwitch`           | 三个 Tab，其中两个可附加；`MaxGeneration = 2`、`MaxQueue = 3`、`MaxGeometry = 1` | 1,904,382 / 425,800 / 61   | `510b243c0d5164dd6d5e885cb276ff2258aecd0c521634334d2d2381a32aaa1a` |

使用 TLA+ tools 1.7.4 运行
`node scripts/tla/run-models.mjs --models native-client-move-barrier --jobs 1`。
四个配置完成时序检查，状态队列为空，整个配置族覆盖了全部 41 个要求的语义动作：

| 配置                 | 生成状态 / 不同状态 / 深度 | 模型/配置 SHA-256                                                  |
| -------------------- | -------------------------- | ------------------------------------------------------------------ |
| `ready`              | 4,710,774 / 777,516 / 31   | `90d94e5d928e0808097f268774e8a7fa2c36e22f1d4ed94198dd8e7fe1ac8dbd` |
| `selected-no-attach` | 470,349 / 113,340 / 28     | `eb5fe3b64033cfbceb44fceaaa688cc31817b314aa4ebabf5553ebf3cf390b6a` |
| `retryable`          | 470,349 / 113,340 / 28     | `144f9a7b682c4ada75a2089dac158aa8ac945071f1ff5e06f72ecf860be0dd3d` |
| `singleton`          | 1,056,290 / 147,508 / 20   | `4699f37d9ce6bcb6d030a29f13b6837c7c938fab2b1ce817d2ab809d7b3c73f3` |

## 退役对等客户端的成员恢复

此入口用于非 Close 发起方的原生 Client：其精确当前普通 Poll 返回了
`TerminalRuntimeUnavailable`。入口保留该已关联请求与 attachment key、不可变绑定、规范
Site/context 基线和历史序号。过期或 setup Poll、传输不确定、畸形回复及其他失败都不能建立
此来源。激活要求精确旧任务成功 join、旧传输已关闭，并已结算所有返回事实。panic 或缺失返回
证据不能建立资格。任何保留的 Close/Read 记录或 flight，或者未决结构变更、提交、setup 或
cleanup 交接，都会排除此入口。既有 Close 状态恢复保留优先级及其全部事实。

完成退役后的显式 `Ctrl+B w` 可通过保留的本地来源启动一个仅 Snapshot 的连接。不会自动重连。
同一个任务 owner 与可处理竞争的 shutdown handle 管理该连接。取消后才建立的连接会被关闭。
不透明的退役 mailbox seed 保留 identity、attachment、Refresh、command 和 UI-wait 分配进度；
新队列和 flight 为空，新 mailbox 从 `Live { desired: None }` 开始。请求 ID 跨连接继续推进；
旧选中 key 和新连接 generation 都不能替代保留的分配 cursor 或规范 authority 检查。

精确 join 禁止**新的旧本地提交和 Viewer 复活**，但不禁止先前已交接的远端工作完成。
Server Drop 可能只排入异步 Detach；本地退役既不是 Detached，也不是 Host Gone。新连接不继承
Viewer 或 controller lease。旧远端引用仍可能存在并占用容量；此入口不声称其最终释放，也不
允许新连接接管或释放它。这是进入公共成员安装的独立退役入口，不是放宽存活连接的清理证据。

每个显式尝试捕获一次 checked deadline：forward 为 45 秒，total 为 50 秒。这个新来源策略从
用户动作开始，覆盖连接、Snapshot 放置、验证、UI 安装及邻居 setup。它不改变普通 Refresh 从
候选阶段开始的预算，也不改变独立的 Close 状态预算。收到结果或等待 UI 后都不会续期。
只有新尝试 setup 的精确清理可使用最后预留时间。停放后再次尝试要求前一 carrier 已关闭并
join，且用户执行另一次显式动作。

安装前唯一请求是 SnapshotRead。完整候选必须保留 Site/context、非递减 revision、相同
revision 下的精确成员，以及所有保留 identity 的不可变元组。原不可用 Tab 必须缺失。
目标仍在、Snapshot 不可用或候选无效时，保留旧规范状态与来源并停放，不执行 Attach。
缺失属于成员知识，不是清理证据。此观察入口不包含 bootstrap、Open、activation、Create、
Start 或 Close。

在 reply mutex 外准备候选，然后检查精确退役来源、基线、flight 和 forward deadline，在该
mutex 下原子提交 state 与 mailbox 计划。按钳制后的历史序号选择邻居或 Empty。邻居仅在
安装时推进实际已消耗的 attachment cursor 一次；Empty 保留它。保留的 provenance 遵循公共
projector。邻居在新连接上使用剩余 setup 预算，不会自动 Start；Empty 接受 Refresh，绝不
自动创建 terminal。安装前到期保留旧规范状态。安装后失败保留已提交候选和 cursor，并如实
退役。过期结果不能安装进另一个 flight。

`NativeClientRetiredMembershipRecovery` 拥有已检查的独立有限入口模型；十九个
`NativeClientMembershipRemoval` 配置保留其既有终止 Retired 与 Detached/Gone 含义。
边界为旧 A 和至多两个邻居 B/C、Empty 与有序候选、两次显式尝试、先前 attachment cursor
为 1、已消耗的 2 或已耗尽的 3 且最大为 3；独立 request/command/Refresh/UI-wait 分配器
最大为 5 并覆盖耗尽，以及独立保留或释放的旧远端引用。
验证是由真实规范 projector 精化的 typed 外部输入。要求检查来源与冲突 guards、单 carrier、
join 后无旧本地提交、不伪造 cleanup/Close 事实、目标缺失、原子安装、过期结果、固定 deadline、
cursor 保留，以及不自动重试或 Create。进度仅假设本地 deadline 与 carrier 终止公平性；
它承诺存活结果或如实停放退役，不承诺远端清理或恢复成功。

使用固定版本 TLA+ tools 1.7.4 和一个 worker 运行
`node scripts/tla/run-models.mjs --models native-client-retired-membership-recovery --jobs 1`。
下列十二个配置均完成时序检查，状态队列为空。十二个配置合计以生成后继覆盖全部 42 个要求的动作。
序号／分配器分区覆盖全部 252 个原始初始状态，不使用对称假设或状态约束。
此前未分区检查的 180 秒超时不能作为通过结果。
进度仅适用于已准入的活跃尝试，并依赖本地时钟、到期、关闭和 join 公平性；不承诺初始退役
完成、用户准入或重试、远端引用释放或恢复成功。Poll 关联、精确 join、typed 验证和 setup
仍是抽象边界，需要完成下文所述运行时精化。

| 配置                   | 生成状态 / 不同状态 / 深度 | 模型/配置 SHA-256                                                  |
| ---------------------- | -------------------------- | ------------------------------------------------------------------ |
| `singleton-normal`     | 3,483,814 / 753,054 / 35   | `2d899c099f0863e9552669fd18d90117b3acec6247c7df4691bcede7c7a1b9b9` |
| `singleton-near-limit` | 4,905,728 / 1,056,996 / 31 | `75aad168a14705a3b05e616813d23b46197fe3589547c333d04b98353765ac25` |
| `singleton-exhausted`  | 1,333,389 / 287,118 / 26   | `b44fb3da620e88734ab0e53f5252edc31211f128747c0523a4bb26750cc6cfd4` |
| `first-normal`         | 3,483,814 / 753,054 / 35   | `0d0e7dc31d4e82791958d3f524e5a93580f33eb94b34fb9d438037c7ae743748` |
| `first-near-limit`     | 4,905,728 / 1,056,996 / 31 | `2d2b70e0c3a363da009907c818f5a1c013346b3aeb0495c450764e11eb982816` |
| `first-exhausted`      | 1,333,389 / 287,118 / 26   | `5fc6b9a602c6d05e17eec2b3e54e584ea70a35764ee936e9ff66253606ea685c` |
| `middle-normal`        | 3,483,814 / 753,054 / 35   | `4c53573ce23a1c0c89d3d2585ba1e7fa5316786ab42f54bb7c4bd0aa4a4b2d9b` |
| `middle-near-limit`    | 4,905,728 / 1,056,996 / 31 | `833d8bdc641cfeedb9bf062fd7bec1b46fe93ca608930c50375ddc4ff0f7baff` |
| `middle-exhausted`     | 1,333,389 / 287,118 / 26   | `b43fc09461924ce9dff66d4b5a8cd0c41a9001c94e4b01b32e50d87f05f36c09` |
| `last-normal`          | 3,483,814 / 753,054 / 35   | `47e22a7ca5ee7f94561421d7a7169b301279e7e83440183e658a82b48ca25d2b` |
| `last-near-limit`      | 4,905,728 / 1,056,996 / 31 | `e04240842241f1401a17b686639c2740e2fd9edd2ee05a01cac522698cf3db62` |
| `last-exhausted`       | 1,333,389 / 287,118 / 26   | `1b85549eb06c352e37318fe6656dfc3b6a0d96011df498e68e3fe51e547f94ac` |

运行时精化复用现有 task owner、重连来源和 shutdown fence。私有 state origin 与 UI attempt
精化模型中的资格与安装动作；普通 Poll executor 仅通过正常精确 join 返回关联证据。
现有 mailbox seed、Snapshot flight、公共 membership transaction 与 setup executor 保留各自所有权。

`terminal_session/tests/retired_membership.rs` 和
`terminal_session/ui/retired_membership_tests.rs` 中的确定性测试覆盖 Snapshot-first Empty 与
邻居 setup 成功、停放后显式重试、错误 ID／transport／其他 unavailable 拒绝、setup Poll 拒绝、
保留事实冲突、缺失返回、过期回复、connect 与 Snapshot 期间取消、原始 45 s forward／50 s total
边界检查，以及 setup 拒绝后的候选／cursor 保留。`mailbox/tests/tab_close.rs` 检查五种分配器
cursor 均跨退休保留、耗尽不能回绕，以及恢复开始时没有 live work 或 bootstrap。在 Linux 运行
`cargo test --locked -p orchbee --bin orchbee retired_`。

Linux PTY Close witness 先观察新的 Disconnected 帧，再执行一次显式 Refresh，随后验证两个
Client 的 Empty 状态、精确移除后的完整公开 Snapshot 相等、原 shell 退出，以及有界 500 ms
新观察期间没有自动 Create。同一用例随后由原只读 Client 手动按 `c`，验证同一 Site/context
内的新 Tab/Panel/Presentation/Terminal 身份和真实新 shell，并将 peer 刷新到只读模式。
普通退出保留新 shell 及原 Server/Worker/Host 进程。后续裸 Open 返回相同的规范 Snapshot、
shell PID 和 shell 局部标记。这不改变 Site 仍为 Empty 时裸 Open 可独立引导创建的已接受行为。
退休 finalization 在 tick 返回并绘制之前同步关闭并 join task。
准备本地 Server／Worker／Host cohort 后，运行
`cargo test --locked -p orchbee --all-targets --all-features`。
这些检查不证明远端引用清理、重启接管或通用恢复。
