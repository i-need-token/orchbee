# 原生终端历史视口

## 范围与归属

本参考规范原生 Client 的保留历史视口：以 `Ctrl+B PageUp`、primary 屏幕向上滚轮或滚动条进入的按窗格滚动回看模式、其
demand 双腿、行窗口 read/acknowledge 握手、导航意图、恢复，以及呈现面如何生成与围栏。
与 `terminal_session/terminal_viewport.rs`、`terminal_session/mailbox.rs`、
`terminal_session/connection/native.rs`、`terminal_session/ui.rs`、`terminal_input/dispatcher.rs`、
`client_view.rs`、`client_view/bindings.rs`、`tui_pointer.rs`、`tui_render.rs` 及其
`tui_render/viewport.rs` 画笔、`localization.rs`、`formal/tla/TerminalViewportCoordination.tla` 共同维护。
协议契约归 `crates/terminal-protocol`（`row_window.rs`、`history.rs`、`row_access.rs`）
与 `crates/client-server-protocol/src/viewer.rs`（`TerminalViewerAction` 面）所有；
本文档消费该契约，绝不扩展它。

历史保留捕获的单元格颜色、属性及宽字符几何。选择与剪贴板集成遵循
[原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)。搜索与 copy mode、图形显示
和持久化仍是独立义务。

同一应用内核心也通过 `cli/terminal_stream/history.rs` 服务 Windows CLI 控制流。每个
已附着的 control 调用拥有一个核心、读取水位及严格历史事件序号。CLI 的 demand/行交换
使用相同协议；完整 ANSI 历史输出刷新后才发送 RowsApplied。适配器的 NDJSON 契约归
[Windows 终端流](windows-terminal-streams.zh-CN.md)所有；TUI 通知文案仍由 TUI 拥有。

Windows 快捷键 Help 分别描述实际配置后的 PageUp 与 PageDown 绑定：Help 关闭时 PageUp
进入保留历史，Help 打开时向前翻页；PageDown 在 Help 打开时向后翻页。这仅改变发现文案。
历史可用性、Esc 返回实时终端、复制／导出及普通 Help 的非模态输入行为均保持不变。
捕获输入的 [Workbench 导航器](native-context-selection.zh-CN.md#windows-捕获输入的导航器)
拥有输入期间，将这些键用于本地行翻页。

## 自动 Agent 尾部展示

通用裁剪偏好可借用同一核心显示保留实时输入的尾部窗口。
准入要求精确当前 Agent 观察、无应用鼠标追踪的主屏幕、可用保留行及可证明为空的尾部。
提示标记、光标行、带样式的背景／下划线及图形阻止裁剪。
返回窗口必须匹配实时 attachment、epoch、generation、revision、几何与预期起始行，才可绘制。
过期、待处理或失败读取保持精确实时网格；未变的失败目标不重试。

自动尾部持有保留行租约，但不消费普通终端键，也不显示手动历史标记。
显式滚动、复制模式或准入的选择将核心移交手动历史。
活动选择与导出阻止自动导航。关闭偏好或失去资格时通过已有 demand 所有者恢复。
不调整 PTY 尺寸或删除历史。核心模型覆盖 demand/read/ack/recovery；
目标准入及光标／复制坐标属于实现义务，由针对性的尾部与绘制测试覆盖。

## 状态机

每个窗格拥有独立的 viewport 核心（`TerminalViewport`，每 `PaneRef` 一个；隐藏或后台
窗格不受焦点窗格模式影响）。无 viewport 状态表示精确实时网格；自动尾部也可持有核心并保留实时输入；核心存在时其 `mode` 为：

| 模式         | 含义                                                                  |
| ------------ | --------------------------------------------------------------------- |
| `Entering`   | 历史+投影 demand 双腿已受理；等待 `HistoryChanged` 事实与首个行窗口   |
| `Viewing`    | 已安装 committed 窗口；read 在保留历史中导航                          |
| `Recovering` | 恢复 demand（`FollowingBottom`+`Active`）已受理或在途；合法新帧结束它 |

手动 `Live` 表示无手动历史核心；自动尾部由其目标区分。分离窗格无核心；viewer 丢失的窗格由 worker 的 demand 路径
（`settle_pane_viewer_lost`）停靠，不由核心判定。

## 入口可用性

`Ctrl+B PageUp` 请求焦点窗格进入历史。除非该窗格最近一帧 live 帧满足以下全部条件，
请求一律拒绝——显示诚实的本地化 `HistoryUnavailable` 提示，且线上发出零条 `Enter`
命令：已知帧存在、`scrollback_rows > 0`、窗格不在备用屏幕、且帧至少携带一条可读行。
屏幕自身行绝不与保留历史混为一谈：历史窗口的 `total_rows` 只来自它服务过的 route。
Live 滚动条观察已验证帧的 `scrollback_rows + geometry.rows`，并检查加法溢出；
绝不以该和替代未知历史窗口的总量。

焦点窗格未启用 tracking 的 primary 屏幕内容区内，非 Shift 滚轮从尾部向前回看三行，
键盘入口仍回看一页。NativeUi 在受理 Enter 前复核精确焦点、当前帧与保留行。Live 状态
向下滚动、历史缺失/为空、帧缺失或备用屏幕均无效果，不显示不可用提示。后续 notch
沿用核心的带符号三行导航，包括进入尚未完成时的意图合并及到达实时尾部时的恢复。
入口拒绝保留 Live，不重放输入。

## 命令与 demand 双腿

`QueuedCommand::Viewport` 每窗格携带一个逻辑 `ViewportStep`，由 `run_viewport` 依序执行，
每条腿前重新核对 incarnation+attachment 租约：

- `Enter` — `SetHistoryDemand(ViewingHistory)` 再 `SetProjectionDemand(Paused)`。第一腿被拒
  或不可用 → 发布 `ViewportOutcome::Enter{applied:false}` 且不走第二腿。第二腿失败或结果
  未知 → 执行一次复位腿（`SetHistoryDemand(FollowingBottom)`）；复位失败或不可用 →
  窗格判错。完全受理 → 发布 `ViewportOutcome::Enter{applied:true}`。
- `Read{origin,row_count,revision,generation,request_sequence,maximum_bytes}` — 租约约束的
  `ReadViewportRows` 动作。worker 校验完整的 kind-46 Response 封装（当前协议、必需的
  correlation、非 optional 的 row-window 编码、请求的载荷上限及封装/数据体 epoch 一致），
  再严格解码 render OBRW 及内嵌 OBPB/OBKG。`terminal_viewport/window.rs` 生成一个不可变样式窗口。
  Windows CLI 流保留其中捕获的默认色；TUI 默认标记使用[原生主题角色](native-themes.zh-CN.md)。
  按精确指针／长度让重复借用字符串别名共享 `Arc<str>`。复制的唯一字符串
  字节也受请求载荷预算约束，在分配前检查；本地耗尽转为 `ResourceExhausted` 恢复，释放
  Host 已服务窗口的钉，不针对该 pending 钉重试。Host 返回的 `LimitExceeded` 仍沿用既有
  有界行数减半。该预算加固定单元格存储限定保留内存。`ViewportOutcome::Window` 发布装箱的类型化窗口；原始
  传输字节不进入 UI 行解码器。两个交互消费者均请求 render；拒绝 text 响应，不静默丢样式。
  CLI 历史使用相同的应用内解码器。畸形/异源响应在安装行前失败；`RowsRejected{reason}` 发布带类型理由的
  `ViewportOutcome::ReadRejected`；不透明宿主拒绝发布同一结果的宿主形态；`Unavailable`
  → 窗格判错。
- `Ack{request_sequence,generation,revision}` — `RowsApplied`。受理 → 发布
  `ViewportOutcome::Ack{request_sequence,accepted:true}`；拒绝 → `accepted:false`。
- `Recover` — `SetHistoryDemand(FollowingBottom)` 再 `SetProjectionDemand(Active)`。任一腿
  失败 → 窗格判错。成功 → 发布 `ViewportOutcome::Recovered`。

每个结果都经 fire-and-forget `WorkerEvent::ViewportOutcome` 通知回程：
`publish_notice_before` 不设 wait key、不期待回复；UI 的 `take_event_before`
与窗格 detach/failure 通知同等受理，再由 `apply_viewport_outcome` 按精确
`PaneRef` 派发到 core。

每一步都经 `viewer_once_ready`/`viewer_for_ready_lease` 围栏：窗格必须仍是同一
incarnation 与活动 attachment 的规范记录，且结果的 attachment 与（租约）generation
匹配。死 viewer 的动作绝不执行。

## 事实入口（kind 53）

`HistoryChanged` 事件经唯一 producer 进入：`TerminalProjection::receive_for_geometry`
校验完整 envelope、角色、epoch、编码与精确 attachment 身份，对合法 kind-53 记录返回唯一的
`TerminalViewerAction::HistoryObserved` 应答。`ui.rs` 的 poll-result 分支消费该返回动作——
绝不自行铸造——并在消费该事件的绘制完成后经普通 `CausalSettlement::Feedback` 通道精确落地
一次，Live 与非 Live 态皆然。同一已校验事件再解码一次供 viewport 核心的事实入口消费，
受每 incarnation 严格序号单调约束；重复或回退以协议故障退役载体。kind-53 记录绝不重跑
帧钩子。

## Read 与 acknowledge 纪律

- 每窗格至多一个在途 read。宿主持有 pending read 钉：pending 期间的第二个 read 会被拒，
  因此核心绝不发出第二个。
- read 请求序号属于 attachment，不属于单次会话。宿主的
  `last_row_request_sequence` 在 `Enter`/`Recover` 循环间存活，因此 client 侧水位归
  `KeyedProjection` 所有、跨 viewport 会话保留：重新进入的历史会话严格高于最后发出的
  序号续接——已发出、已受理或已取消的皆然——只有真正的新 attachment incarnation 才
  归零。kind-53 事件序号是独立域。耗尽是终态故障，绝不回绕。
- 铸造与受理是两个独立步骤。核心在 `pending_commands` 中至多暂存一个未受理步骤；
  `next_read_sequence` 在铸造时即被消耗，因此暂存步骤若随后被恢复或 resize 替换会留下
  真实空号——已发出的序号绝不重发。read 在途与应答生命周期仅在步骤被受理
  （`command_admitted`）时绑定；已受理命令不可撤销、按序执行，恢复在自身被受理时才清除
  在途状态，而不是在决定时。每条已执行命令恰好发布一个有类型的结果；至多一个结果已发布
  等待、第二个已算出的结果在其后等待——已发布的结果绝不阻塞下一条命令的执行。
- 至多一个在途 `RowsApplied` 应答——仅在窗口通过全部安装闸门后发出。read 在途、或应答
  待受理或在途期间，不发出新 read；导航意图改为合并。
- 窗口安装闸门依序：解码合法；attachment 与 host epoch 匹配；匹配当前活动 read 序号；
  匹配当前事实 generation；匹配当前窗格列数。序号不匹配、或无在途 flight 时收到窗口，
  无法证明它清结了哪个 pending → 进入恢复。被丢弃的窗口并不自动意味着还会有事件：
  若持有的事实在该 read 发出时的 generation 之后已经推进，该次 kind-53 转换本身就是
  钉的清结，下一个 read 在新 generation 下继续。只有当事实自发出以来未曾变动时，核心
  才进入有界等待——generation 值只与发出时的 generation 判等比较，绝不按数值排序。
- 安装窗口不等于 committed。只有精确受理的 `RowsApplied` 才把 pending 锚点提升为
  committed。应答被拒 → 锚点丢失并带本地化提示进入恢复——绝不以陈旧绝对偏移伪造位置。
  先前已提交的锚点在尺寸变化后保留，独立于新窗口待受理或在途的应答。尺寸变化丢弃新窗口
  及其未受理应答；收到新事实后，可以刷新旧提交锚点，无需接受已丢弃的窗口。迟到应答
  无法匹配已丢弃的窗口。
- 本地行意图以正数表示移向更旧行。进入时将意图加到 `Tail{rows_before}`；committed 导航
  则对每段意图取反，因为 Host 将 wire `delta` 加到 committed **起始行**。在途与剩余意图
  保持本地符号。`Committed{delta}` read 每步界于 ±4096；更大的移动拆分为残量限界步，一次只发一步。
  `Tail{rows_before}` read 仅服务首次入场与绝对跳转。
- [键盘复制模式](native-keyboard-copy.zh-CN.md) 增加内部 `Newest` 绝对跳转：`Tail{0}`
  在行应答接纳后保持 Viewing。其 primary 入场允许可读的零历史 frame 在 Tail{0} 进入；普通
  PageUp／wheel 入场与尾部恢复规则不变。Copy 使用 committed-only window accessor，输入归属
  先于普通历史。alternate 表面使用 live frame，不申请历史 demand。Viewport model 的
  `PressJump` 抽象两种绝对目标；具体目标与 copy cursor／selection gating 由 core／native traces
  和 copy coordination model 检查。

## 事实期限与恢复

单一有界预算（3 秒）覆盖每个等待片段：入场事实、一次 stale-generation 或
stale-revision 重试、resize 后事实。超时 → 排队 `Recover` 并显示恢复提示。stale
重试与 `LimitExceeded` 减半预算是按导航意图计的：当 nav 调用发现既无待决也无在途意图
时重置，resize 时也重置——并入进行中的意图则共享其预算。

一旦恢复被决定——已进入 `Recovering` 或已排队 `Recover`——迟到的事实、窗口、read
结果与应答结果一律被吸收：不安装窗口、不再铸造 `Read`/`Ack`/`Recover` 命令；合法的
kind-53 事件仍经其自有通道精确应答一次。`Recovering`
仅当恢复 demand 已应用 _且_ 之后到达合法帧时才结束——恢复 demand 之前已在途的帧不能
提前结束它。committed 锚点丢失（`anchor_status = Evicted`，或 committed 期间为 none）
同样进入恢复。

## 输入排序

焦点窗格非 Live 时的普通键盘输入先排队 `Recover`；仅当恢复命令被受理后，该字节才以同一
`PaneRef` 的单个 `Input` 命令转发——绝不在两条 demand 腿之间插入，拒绝后绝不重放。若
线路只容得下 Recover，则恢复照常受理而该字节丢弃；若毫无余量，则 Recover 保持暂存而该
字节丢弃——被丢弃的字节绝不重放。`Recovering` 窗格直接转发该字节（恢复已受理）。非 Live 窗格本地消费历史导航键
（`PageUp`、`PageDown`、`Home`、`End`、`Esc`）与鼠标滚轮；Live 窗格保留完整的应用输入
与鼠标跟踪，未启用 tracking 的 primary 屏幕滚轮进入历史除外。`Ctrl+B PageUp` 进入历史；
help 打开时 `PageUp` 仍归 help。

凡以非 Live 窗格为目标的非滚轮指针事件一律由 Client 消费——落在焦点窗格上的按下、把
焦点移入该窗格的按下、排队中的 pending-focus 重放、手势延续，以及手势中途 Live→非 Live
转换后才结束的拖拽。内容上的普通主键手势针对已提交历史窗口，遵循
[原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)；其他事件在 release 时于本地
终结。它们都不编码应用鼠标字节。滚轮保持自身契约：在保留窗口内本地滚动，落在窗格其他
位置则被消费且不关闭覆盖层。

## 呈现

已验证的 render 窗口与实时输出共用样式单元格画笔，受窗格内容几何约束。颜色使用窗口
捕获的默认值，不取 TUI 主题或更新的实时帧。样式属性和宽字符／占位格沿用实时绘制行为；
控制字符经过净化，被裁切的宽字符绘为空白。历史不显示光标。图形资源在解码时完整验证，
但暂不显示，不能据此关闭图形历史义务。旧 generation 或旧
列数的数据绝不绘制：surface 访问器每次绘制都以当前事实复核已安装窗口的 generation 与
列数。`Entering` 在历史徽标下保留最近已验证 live 帧；`Viewing` 无当前窗口 → 清屏加徽标；
`Recovering` → 清屏加恢复指示。叶区末行左侧显示本地化位置读数（当前已验证窗口的
`start_row`/`total_rows`——已过期或陈旧窗口不显示），右侧常驻本地化返回实时提示；
窄窗格按字素边界降级，提示保持优先。`Esc` 返回实时输出。徽标矩形和整个末行先重置底层
单元格，再绘制各自语义样式，避免子程序颜色、隐藏／反色属性及历史文字残留到提示区。
跨过徽标左边界的宽字符绘为空白，不让半个字形覆盖徽标。

## 滚动条交互

每个窗格在终端内容右侧固定预留一列 chrome，与历史是否可用无关。绘制、原生终端尺寸和
布局查询共享这份几何；分屏分隔线仍是独立目标。只有一列宽的窗格保留内容、省略滚动条列。
轨道不足两行时不显示滑块，也不发布鼠标目标。

`ScrollbarSnapshot` 观察准确的 attachment、epoch、generation、列数、视口行数、总行数和
起始行。Live 要求可读的主屏幕帧；历史要求当前有效的已验证窗口。加载、恢复、备用屏幕、
空历史和超过协议 `u32` tail 寻址范围的历史不显示滑块，也不接纳新按下。不猜测数量。
较短的返回窗口以实际行数计算滑块，同时保留物理窗格几何；读取上限缩减会取消旧抓取。
整数映射四舍五入到最近的可表示行，至少保留一格滑块和一格移动空间，使用焦点／文字及
边框主题角色，字形为 `┃`/`│`，ASCII 替代为 `#`/`|`。滑块跟随已验证的显示窗口，
不将待处理拖动意图乐观绘制为当前位置。
滑块强调遵循与窗格标题相同的 shell 输入归属。

| 事件／守卫                                                                                     | 转移与结果                                                                                                             |
| ---------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 滑块上无修饰左键按下                                                                           | 本地聚焦准确且已附着的 `PaneRef`；捕获几何、范围和抓取行，不跳转。                                                     |
| 轨道上无修饰左键按下                                                                           | 聚焦该窗格；将滑块中心移到按下行并限制在已知范围内，保留捕获以便拖动。                                                 |
| 移动／匹配的释放                                                                               | 按按下时范围与抓取偏移映射；钳制轨道外坐标并合并相同偏移。释放应用最终位置并结束捕获。                                 |
| Live 中的正偏移                                                                                | 以该 tail 偏移进入历史。facts 仍控制首次读取；已接纳的按下在首个窗口待定时提供已知范围。                               |
| 历史中的正偏移                                                                                 | `ViewportNav::OffsetFromBottom` 通过现有单读请求／ACK 守卫替换待处理绝对意图。安装旧请求只清除匹配偏移，保留更新目标。 |
| 偏移为零                                                                                       | 排队普通 Recover；只有恢复 demand 完成后再到达的新帧才删除 core 并恢复 Live 跟随。                                     |
| 总行数、generation、attachment 或几何变化                                                      | 新鲜来源检查或绘制时取消后续拖动意图。输出增长要求重新按下；旧抓取不映射到新滑块。已接纳工作保留原窗格和普通完成规则。 |
| resize、窗格切换／关闭／重建身份、区域被覆盖、释放捕获、宿主失焦、键盘输入或带修饰键的指针事件 | 取消捕获；迟到的移动／释放不能启动另一窗格的手势，也不能到达应用鼠标上报。                                             |

指针只拥有捕获。NativeUi 在每个意图前重新检查准确焦点、读取就绪、选择准入、复制模式排除
和来源范围。待定窗口可保留已有捕获，但不能授权新捕获。facts 总量变化在替代窗口绘制前就
拒绝后续请求。历史中的实时输出刷新已提交锚点，不发出滚动条跳转。取消捕获不撤销先前
已接纳的导航；该意图仍通过 core 的验证和 ACK 完成。选择／复制归属和应用
鼠标上报保持原样：chrome 事件不进入程序。键盘等价入口仍为 `Ctrl+B PageUp`、PageUp/PageDown、
Home、End 和 Esc。

比较参考为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/ui/scrollbar.rs` 和
`src/client/shell/mouse.rs`：右侧轨道、抓取偏移及有界滚动。这不是源码等价移植；Orchbee
保留自己的 read/ACK 权威与取消策略。

证据：`terminal_scrollbar/tests` 覆盖整数边界；`tui_pointer/tests/scrollbar` 覆盖手势／取消
及所有鼠标追踪模式；`ui/viewport_tests/scrollbar` 覆盖组合主题／语言绘制、原生准入、解码后
的失焦／键盘轨迹、输出增长和恢复。 保留的视口核心仅检查验证章节中的协调性质；滚动条几何与手势仍由实现测试提供证据。

## 不变式

- `OneReadFlight`：至多一个已受理的在途 read；宿主 pending 绝不被二次占用。
- `SingleStagedStep`：暂存槽位至多存放一个未受理步骤。
- `OrderedNonreusedReads`：已铸造的 read 序号绝不重发；线路上尚存的任何 read 序号——
  暂存、已受理或已产出——都不超过当前在途 flight。错误窗口 payload 使用外来序号符号
  `sq = 0`，不参与两两唯一性比较；它仍占用某个已执行 read 的原结果槽。
- `AckAfterInstall`：`RowsApplied` 只对已安装窗口、匹配序号、精确一次。
- `CommittedOnlyAfterAck`：只有精确受理的应答才提升锚点，`Committed` read 使用先前
  已受理的锚点。模型的 `CommittedNeedsAck` 在从未提交到已提交的迁移边上检查
  `AckAccepted`，不要求当前窗口的应答在尺寸变化后持续存在。
- `NoReadWhileAck`：应答待受理或在途期间不发出 read。
- `SurfaceCurrent`：绘制的行永远匹配当前 generation 与列数。模型只把已绘制 generation
  约束为空或当前值，本身不证明正向绘制——绘制证据来自产品测试。
- `RecoverBeforeInput`：每个新受理到线路上的普通输入都已在覆盖之下——面板处于
  `Recovering`，或其前方已有受理的 `Recover`。以受理边时序断言表达；已执行完毕的
  恢复无需重复见证。
- `EveryEventAcked`：每个合法 `HistoryChanged` 精确应答一次。
- `MonotoneEvents`：接受的事件序号严格递增。
- `BoundedDelta`：每个发出的 `Committed` 步都在 ±4096 内。
- `HonestModes`：Live 保留普通应用键和 tracking；符合条件的 primary 屏幕向上滚轮
  可进入历史。非 Live 导航留在本地，普通输入先恢复实时。
- `PaneIncarnationFence`：每个线上动作与事件都解析精确 `PaneRef` 与 attachment 租约；
  死 incarnation 的事件和结果丢弃。受检动作性质让本 incarnation 的 `Lost` 保持终态；
  重新附着归属于新的 incarnation。

## 源码对应

`TerminalViewportCoordination.tla/.cfg` 抽象一个 pane、两次进入/读取尝试、两个匿名输入命令和容量为二的 FIFO。`Enter`、`Read`、`Recover` 对应 viewport 暂存命令；`Admit`、`Execute` 对应 mailbox/worker FIFO；`Consume` 对应结果消费；`Detach` 使 pane epoch 失效。故障注入只能破坏已有窗口响应，不能制造独立结果生产者。

检查范围为有界队列所有权、单一读取 flight、当前 epoch 的显示、恢复时保留已接纳命令，以及在接纳、执行、消费弱公平假设下进入/恢复最终收敛。这是更小的协议抽象，不是所有 viewport 操作的等价证明。实际行边界、导航、期限、滚动条手势和代际/序号校验由 `terminal_session/terminal_viewport/tests.rs`、`window.rs` 及 projection/connection 测试负责。

## 验证

运行 `node scripts/tla/run-models.mjs --models native-terminal-viewport` 检查有界协议，并运行原生 Cargo 测试验证实现。`scripts/tla/model-mutations.test.mjs` 注入独立结果生产者，要求得到 `OutcomeProductionOwned` 反例。完整 `pnpm test:tla` 门禁执行此负向控制及所有保留配置，不复用成功结果缓存。

模型不证明实际时钟延迟、载荷校验、行几何或所有 UI 调度。超时、未清空状态队列或动作覆盖不足均属于检查失败。
