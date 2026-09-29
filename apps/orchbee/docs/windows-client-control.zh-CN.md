# Windows Client 视图控制

本契约拥有原生 CLI 对运行中 Windows TUI Client 的发现与导航。TUI 拥有选择和缩放状态；Server 路由有界请求并保留观测。规范 Workbench 快照、修订和终端控制权不变。路由请求不激活操作系统窗口。

普通 Workspace/Tab focus 共用 [Client 选择记忆](native-context-selection.zh-CN.md)：返回时恢复现存的上次 Tab/Panel；显式 Panel 或 Agent 导航覆盖记忆。历史由所选 Client 私有持有，重启丢弃。

## 身份与发布

经认证的 Client Session 可发布一个交互视图。Client 身份是已接纳的非零 Session generation，编码为 32 位小写十六进制，作用域为 Site。身份由 Server 分配，generation 在一次 Server 生命周期内不重复。普通 CLI 连接不发布视图。同步不接受调用者指定的 Client 身份，不能确认其他 Session 的操作。

视图报告已观测 Workbench 修订、阶段（`starting`、`ready`、`busy`、`unavailable`）及必填可空的选择。选择包含 TerminalContext 身份、必填可空的 Tab／Panel 身份、显式缩放标记和活动 Tab 的 Panel 数（没有 Tab 时为零，有 Tab 时为正）。有 Panel 必须有 Tab；缩放必须有 Panel。这些是 Client 观测，不是新增的规范 Workbench 字段。

每个 UI 样本携带非零规范 u64 文本 heartbeat 和视图修订。heartbeat 随 UI 采样推进；报告的视图改变时视图修订推进。合并样本可以跳号。相同修订要求视图内容相同；更旧的修订或 heartbeat 冲突。没有新 UI heartbeat 的重复网络同步不延长新鲜期。新 UI 样本到达 Server 后五秒内视图为 current，之后为 stale，直到新样本到达。

## 控制与确认

控制命令包含操作身份、精确 Client 身份、预期视图修订、最低已观测 Workbench 修订，以及一种意图：

- Context：选择精确 TerminalContext（CLI 将 Workspace 解析为其 context）。
- Tab：选择所属 context 中的精确 Tab。
- Panel：选择所属 Tab／context 中的精确 Panel。
- Zoom：将精确选中 Panel 所在 Tab 的缩放设为显式布尔值。
- ToggleZoom：UI 所有者激活目标后，从观测到的状态切换该 Tab 一次。
- Layout：读取精确目标几何，不选择或调整尺寸；[布局所有者](windows-client-layout.zh-CN.md)定义其观测与预算。

toggle 由 TUI 解析，并为这次已投递操作保留解析结果。相同操作身份和完整命令返回保留的观测，即使 Client 已断开。命令数据改变会冲突。回执过期后的缺失不能证明原命令没有效果。

| 转移                | 守卫及观测                                                                                                                                                          |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Submit              | 已登记且 current 的 Client、精确视图修订、可用容量、该 Client 无活动命令；保留 `queued`                                                                             |
| Synchronize         | 仅连接所有者发布样本；精确排队前置条件允许一次投递，并原子记录 `delivered`                                                                                          |
| 排队超时            | 五秒未投递产生 `rejected: expired`；没有投递任何操作                                                                                                                |
| Client 确认         | 精确已投递操作及所有者；保留 `applied`、动作前的 `rejected`、带原状态的 `zoom-toggled`、单 Panel zoom-on/toggle 的 `zoom-unchanged` 或可能部分执行后的 `incomplete` |
| 投递前断开          | 移除运行中视图；排队工作变为未执行的拒绝                                                                                                                            |
| 投递后断开          | 移除运行中视图；未确认工作变为 incomplete，不声称最终视图                                                                                                           |
| 调用者超时或 Ctrl+C | 仅停止 CLI 等待；查询保留的操作身份                                                                                                                                 |
| 完成后保留          | 完成观测保留 60 秒；只能回收已过期的完成记录                                                                                                                        |

在既有 Client Session 接纳上限内最多有 32 个运行中视图，每个 Client 一条活动命令，共 256 条保留控制记录。容量拒绝绝不逐出 pending 或未过期证据。已投递工作在确认或断开前持续有所有者。Server 执行 Client 命令不产生终端输入、Git 或全局焦点副作用。

## TUI 执行

一条 Windows 控制通道拥有独立的已接纳连接及有界最新样本交接。它不阻塞终端载体，也不在自身线程修改 UI。UI 在本地输入之后、绘制之前消费语义命令，检查已发布视图前置条件，将规范成员关系刷新至命令最低修订，再使用既有 context、Tab、Panel 选择所有者。异步附件及刷新沿用原有所有者和 deadline。

本地导航或模态意图中断后续远程步骤。已接纳的原生工作不描述为已撤销；此类步骤之后的中断为 incomplete。目标缺失、视图过期、不支持状态及模态／忙碌接纳均有明确结果。几何变化在绘制前重新观测 viewport 尺寸并使指针命中图失效。Applied 确认要求已观测到目标选择／缩放且成功绘制。绘制失败或连接丢失不能伪造 applied 成功。viewport 小到无法显示内容时视为 busy。对单 Panel Tab 开启或切换缩放，在选中并绘制目标 Panel 后返回 `zoom-unchanged`，不伪造 zoom 标记；该结果要求已观测单 Panel 目标、`zoomed: false` 及请求的最低修订。成功绘制的观测固定保留到确认，后续本地导航不能改写回执。

TUI 退出时控制通道停止、唤醒阻塞 I/O 并 join。Server Session 清理移除发布并结算保留请求；受管 Site 停止会 join 这些 handler。Client 视图／控制状态易失，不跨 Server 重启保留。

## CLI 路由与线上边界

显式 Client 选择器选择该精确运行中 Client。省略时，经校验的调用者 Terminal／Worker 上下文选择唯一聚焦于该绑定的视图；没有此上下文时要求恰好一个 current 视图。缺失、无效、过期或歧义上下文均明确失败；不猜测最近或全局焦点。目标解析继续使用规范 Workbench 身份。

`workbench.client-view` 包含 Synchronize、List、Get、Submit、Read 动作，响应为 `workbench.client-view.result`。请求携带协议版本、交换请求身份及 Site 身份。Synchronize 携带必填可空 completion，响应携带必填可空的下一条 control。操作身份与交换身份分离。frame kind 25／26 沿用 OBWP 版本 1；分配前请求体限 16 KiB，结果限 64 KiB。Rust 与 TypeScript 独立校验封闭变体、必填可空字段、身份／序列边界及跨字段关系。

该边界在精确 cohort 接纳范围内增量添加。旧 cohort 不会默默重新解释新消息。回滚 cohort 只丢弃易失视图／控制观测，不改变规范 Workspace 或终端状态。新增 Mac/Linux 控制运行时和 Desktop CLI 暴露不在本次 Windows 实现范围内。

## Client 视口观察

Windows Panel list/get 指定精确 Client 后，通过 `viewports` 读取该 Client 已安装的主终端
视口，每批有序包含 1–16 个唯一 Panel ID。`viewport-read` 返回规范修订号、每个 Panel
可空的主终端身份及观察：unavailable（`not-attached` 或 `frame-unavailable`）、pending
（`entering`、`awaiting-window`、`recovering`）或 observed（`live` 或 `history`）。
查询不会为非活动或未绑定 Panel 创建 viewer，仅当前主终端实例参与。

已观察来源保留 Host epoch、attachment、generation、revision、列数、屏幕、总行数和起始行。
滚动计数使用规范十进制 uint64 字符串；`viewportRows` 是 Client 单元格高度，不是可能缩短的
历史响应行数。`maxOffsetFromBottom = max(totalRows - viewportRows, 0)`，
`offsetFromBottom = maxOffsetFromBottom - min(startRow, maxOffsetFromBottom)`。
实时主屏从 scrollback 起始，备用屏的 scrollback 为零。历史读取与绘制器相同的、经过代际及
几何检查的已安装窗口。待恢复或缺失数据绝不伪造位于底部的位置。

此只读操作可在 Client Busy（包括历史浏览）时执行，不放宽导航或变更准入。
Starting/unavailable 视图不能产生成功的视口观察；精确视图修订、单个待处理操作及规范成员
检查仍然必需。UI 不执行 attachment、输入、resize、导航、历史读取、确认或绘制副作用。
每批最多保留 12 KiB，Server 准入前预留该额度。沿用现有帧种类及 16 KiB 请求／64 KiB 结果
限制。Rust、TypeScript 和 schema/语料保持一致；增量契约要求匹配 cohort。
状态易失，回滚不迁移持久数据。定向证据覆盖边界、错误成员/路由、待处理/恢复状态、Busy
只读与变更的准入区别，以及真实 Windows 双 Client 历史观察且焦点、视口不变。

## 模型与证据

已登记的 `ClientViewControl` TLA 模型抽象所有者路由、发布版本约束、一次投递、UI 效果、绘制、中断及回执回收。精确时长、容量及序列化边界由维护的可执行测试负责。Server registry 是接纳 generation、Client 样本、单调时间、排队／已投递命令及完成观测的确定性状态所有者。认证载体是适配器。TUI controller 拥有有序语义步骤；连接通道拥有传输及 join；CLI 拥有选择、单次提交及有界等待。任何所有者均不写其他所有者的私有状态。

证据必须覆盖所有者隔离、两个 Client、过期／冻结 UI、修订变化、不重放投递、重复／冲突身份、容量／过期、部分执行、人工输入干扰、断开和停止。生成式有状态轨迹保留缩减反例。共享 Rust／TypeScript／schema 夹具和 frame 上限建立有界契约一致性。受控 Windows ConPTY 轨迹必须展示 CLI 选择到达指定 Client 且成功绘制，另一 Client 不变。Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 focus／zoom handler 是选定行为参考；独立 Client 路由属于 Orchbee 架构。用户已推迟 TLA 执行，必须如实报告模型资产及未执行检查。同作者检查不代表独立复核。

### 指定目标的缩放切换

`ToggleZoom` 是精确目标意图。UI 激活目标 Tab 后，只采样一次该 Tab 原有的缩放状态，并在 Panel 焦点切换和后续循环中保留此决定。`ZoomToggled` 携带 `previouslyZoomed` 及成功绘制后的观察，缩放值必须与原值相反。它不能确认普通的 `Zoom` 意图。单 Panel 目标返回 `ZoomUnchanged`，不声称发生了缩放切换。导航中断仍返回未完成，状态查询不会再次执行切换。

布局读取使用同一个单次投递通道，但无需绘制确认，直接以 `layout-read` 完成。专用帧类型 27/28 承载其较大的有界载荷。`ClientViewControl` 还抽象只读纯度以及待处理／结果内存预留；registry 测试验证精确载荷计费。

### Panel 方向聚焦

进入上下文或 Tab 后，精确 Panel 导航可能遇到目标查看器仍在附着或完成初始化。它在原有操作 deadline 内等待已有归属方，不重新附着，也不延长 deadline。成员丢失、绑定明确不可用或带故障且未存活的查看器会明确失败。本地打断仍停止后续步骤，并保留已发生部分效果的事实。

`panel focus [source] --direction left|right|up|down` 在任何导航之前，按指定 Client 的未缩放单元格几何解析相邻面板。它沿用 [Panel 布局](windows-client-layout.zh-CN.md) 的 Snapshot、成员关系、可见性、版本和排序规则。来源可以位于未激活或未访问的 Tab。来源不可用或隐藏时明确失败，不能据此认定没有邻居。

没有邻居时返回 `direction-unchanged`，携带来源 `layoutRevision` 和未改变的 Client 视图，不选择来源上下文或 Tab，不改变焦点或缩放，不附着、不调整终端大小，也不请求绘制。来源版本必须达到命令要求的最低版本；当前选择的其他上下文不必推进到该版本。来源 Tab 已选中时，其布局版本等于采样视图版本。

找到邻居后，将其一次性固定为精确的目标 Panel。现有的有序选择和绘制确认执行该固定目标，回执保留原始方向意图。后续 tick、视口变化、状态查询和重放都不会再次执行方向移动。`direction-focused` 携带目标 Panel 和观测视图；目标必须区别于来源，并匹配要求的上下文、Tab 和最低版本。即使找到的邻居已经获得焦点，也在绘制后返回此结果。保留缩放状态。本地打断和成员丢失沿用已有 rejected/incomplete 语义，不激活操作系统窗口。

两种新完成结果沿用普通有界帧 25/26。Rust、TypeScript 和 JSON Schema 拒绝未知变体和字段；语义校验器拒绝意图不符的方向结果、以来源充当目标、视图不符以及过期的来源版本。Server 保留原始操作，不自行解析几何。TLA 抽象增加没有副作用和绘制的无邻居完成，执行仍延期。维护的控制器和注册表见证覆盖无邻居纯度、固定目标、绘制顺序、打断和回执重放。Windows ConPTY 证据覆盖精确 Client 隔离及非活动来源行为。固定版本 Herdr 的方向聚焦处理器提供无邻居行为参考；Orchbee 保留自己的单元格布局和认证 Client 路由。

`panel resize [panel] --direction left|right|up|down [--amount <number>]` 调整一个符合条件的分割边界，保留焦点和缩放。`layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` 设置精确二进制分割路径。两者都路由到一个 Windows TUI，并保留 `layout-adjusted` 回执。当前标签页的变化等待绘制，非活动标签页只保存、不导航。有限比例限制为 0.1–0.9；有限 resize 数量转为绝对值且最多 0.5，默认 0.05。比例归该 TUI 会话所有，重启后不恢复。参见[布局契约](windows-client-layout.zh-CN.md)。

## Agent 导航

Windows `agent focus <target> [--client <id>]` 将规范 Session 或终端 Agent 解析为一个可导航主 Panel。一次 `agent-focus` 控制在 Client 导航和完成期间保留精确 Agent 与 Presentation 身份。Client 复核自己的观察；物理目标还要求匹配 Host 帧。显式聚焦成功后，将所选 Tab 的当前已知版本标记已读，包括目标已经聚焦及外层失焦的情况。普通绘制和只读 `agent attention` 保持既有规则。`--request-id`、`--detach`、默认 30000 ms 总期限及保留状态恢复不变。Starting 或 Unavailable Client 在 Agent 读取前拒绝；Busy 可以进入一次性模式退出前置检查，但不丢弃已提交操作。历史模式等待恢复及新的实时帧；目标丢失／替换会拒绝或报告导航未完整完成，不改选目标。`orchbee.client.control` v5 携带此操作，不激活操作系统窗口。[Agent 导航契约](native-agent-navigation.zh-CN.md) 负责身份、确认及最终验收。

## Client 相对 Workbench 查询

Windows `workspace list/get`、`tab list/get` 和 `panel list/get` 接受可选
`--client <id>`。省略时保留规范的无界面投影；提供时，由准确指定的当前 Ready Client
提供全局 `focused` 标记；Panel 读取还根据上文只读视口契约接纳 Busy。Workspace 行还包含 `activeTabId`，包括非活动工作区记住的
选择。null 表示空上下文。焦点指在该 TUI 中被选择，不表示操作系统窗口焦点。
缺失、不符合准入条件的忙碌、过期或有歧义的事实明确失败，不替换为其他 Client。

只读 `Selections` 意图携带 1–128 个不重复的 TerminalContext ID。TUI 根据单个新鲜
规范快照解析当前选择或私有导航记忆：保留仍存在的记忆 Tab，否则使用正常返回上下文时
相同的首个 Tab 回退规则。不访问该上下文，也不更新记忆。`NavigationRead` 保留请求
顺序、规范修订号和 Client 观察。载荷限制为 16 KiB，复用现有保留控制操作、64 KiB
结果封装和 Server 容量核算。

CLI 先解析查询目标，在单个已准入连接上分批读取，检查准确的请求/上下文身份和成员关系，
并在输出完整结果前确认规范修订号和 Client 视图修订号未改变。统一五秒总期限和信号所有者
覆盖所有阶段。不产生部分成功、自动重放、导航、已看过确认、终端附着或输入效果。
指定 Client 的结果包含 Client/操作/视图来源；未指定时不伪造焦点或活动字段。
取消会关闭连接并退出130；不可用或变化的观察保留类型化失败。

测试覆盖数量边界与重复 ID、响应关联、无效成员与修订号、空/非活动/重排/已删除选择、
独立 Client、无界面兼容、语法、schema 和双语输出。真实 Windows 验收需比较查询前后的
两个 Client 和规范状态。Agent 汇总、Panel 进程/标题和 Client 滚动由各自观察所有者提供；
Workspace worktree 及上报元数据仍属于后续关联。

## 精确 Panel Presentation 观察

标量被动 `presentation` 意图读取一个 Panel 的当前原生成员，不改变焦点或输入所有权。
`presentation-read` 携带规范修订和 Panel ID，以及必须存在但可为 null 的成员：
Presentation ID、精确绑定、现有本地附着代次及窗格实例编号和已安装视口观察。
载荷限制为 4 KiB；Server 接入前预留此容量，并在保留回执前验证 Panel、最低修订
及 Ready/Busy 完成视图。没有本地成员时返回 null；过时绑定不得借用其他成员的帧。
此操作不能以已应用导航完成。

[当前 Panel 契约](native-current-panel.zh-CN.md#进程信息的精确当前-presentation)
负责 Windows process-info/current 组合及前后成员栅栏。Client control v6 接纳此意图；
v1–v5 保持封闭。已有主终端 `viewports` 读取保留原有语义。

未绑定原生成员把被动选择扩展为严格的另一种形状，仅包含 `presentationId` 和非零
`selectionGeneration`，不含挂接、绑定或 viewport；null 仍表示没有本地成员。发布前验证原生
稳定状态和规范身份。Control v7 暴露这一形状；既有已绑定／null 回执仍为 v6，旧版本拒绝未绑定
形状。两种形式都不确认 attention，也不改变焦点。

## 显式聚焦的已查看确认

Workspace、Tab、直接 Panel、方向 Panel 或 Agent 聚焦成功后，会确认所选 Tab 中每个
Presentation 当前已知的 attention 版本，包括隐藏兄弟，以及外层终端失焦时已选中的目标。
已有 Client attention 所有者在匹配的 Ready 绘制完成后执行一次；空 Context 无需确认。
不会为未来观察或其他 Tab 排队确认。

控制器使用一次解析的方向目标；无邻居时不导航、不绘制、不确认。只读查询、缩放和调整
尺寸不获得这项显式副作用。完成绘制前解码到人的输入意图或到达期限时，不确认已查看；
保留的 rejected／incomplete 结果继续描述已经发生的导航。Agent 聚焦保留精确主体／成员／
物理画面校验，不激活操作系统窗口，也不发送终端输入。

此行为参照固定 Herdr `e0507237` 的 Pane 聚焦及 Workspace／Tab 切换确认。弹层打开时的 Workspace／Tab 导航仍是独立的待补项。定向控制器与 attention 测试覆盖外层失焦、同一目标、当前版本范围、方向解析／
无邻居、人类输入、期限及重放；完整 Windows 验收仍留到最后。

## Panel 聚焦模式退出

直接 Panel 聚焦和已解析出邻居的方向聚焦，可以进入 Busy Client 执行一次可取消模式的
退出预检。取消草稿或 shell 界面前，控制通道读取一次最新规范 Snapshot，Client 校验 Site、
最低版本以及精确 Context／Tab／Panel 成员关系。目标缺失、拒绝、过期或不可用时，保持
模式不变。方向几何在退出模式前仅解析一次；没有邻居时返回未变，不绘制或确认已查看。

Panel 和 Agent 聚焦共享现有 Client 负责的退出序列：仅一次取消未提交的对话框和 shell
模式，由 viewport 所有者请求历史恢复，等待恢复和新的实时帧，然后导航，并在匹配的
Ready 绘制后确认已查看。稳定的未绑定选择可以进入，无需创建 attachment。已提交的变更、
待处理刷新、关闭结算、退役和过小视口均在取消前拒绝。新模式、人工输入或到期会打断请求；
保留的 incomplete 结果报告此前已发生的模式或导航效果。不得丢弃历史缓冲或已提交操作
来制造就绪状态。

Workspace／Tab 聚焦不继承 Panel 的模式取消行为。不新增协议字段、公开 schema、操作系统
激活、终端输入或持久化状态。定向控制器、规范预检、Server 准入和 Windows 通道关闭测试
覆盖这些边界；完整 Windows 验收仍属于最终回归。
