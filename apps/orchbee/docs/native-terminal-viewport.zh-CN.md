# 原生终端历史视口

## 范围与归属

本参考规范原生 Client 的保留历史视口：以 `Ctrl+B PageUp`、primary 屏幕向上滚轮或滚动条进入的按窗格滚动回看模式、其
demand 双腿、行窗口 read/acknowledge 握手、导航意图、恢复，以及呈现面如何生成与围栏。
与 `terminal_session/terminal_viewport.rs`、`terminal_session/mailbox.rs`、
`terminal_session/connection/native.rs`、`terminal_session/ui.rs`、`terminal_input/dispatcher.rs`、
`client_view.rs`、`client_view/bindings.rs`、`tui_pointer.rs`、`tui_render.rs` 及其
`tui_render/viewport.rs` 画笔、`localization.rs`、`formal/tla/TerminalClientViewport.tla` 共同维护。
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

## 状态机

每个窗格拥有独立的 viewport 核心（`TerminalViewport`，每 `PaneRef` 一个；隐藏或后台
窗格不受焦点窗格模式影响）。无 viewport 状态即 `Live`；核心存在时其 `mode` 为：

| 模式         | 含义                                                                  |
| ------------ | --------------------------------------------------------------------- |
| `Entering`   | 历史+投影 demand 双腿已受理；等待 `HistoryChanged` 事实与首个行窗口   |
| `Viewing`    | 已安装 committed 窗口；read 在保留历史中导航                          |
| `Recovering` | 恢复 demand（`FollowingBottom`+`Active`）已受理或在途；合法新帧结束它 |

`Live` 即无核心。分离窗格无核心；viewer 丢失的窗格由 worker 的 demand 路径
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
的失焦／键盘轨迹、输出增长和恢复。`TerminalScrollbar.tla` 检查捕获围栏、延迟 install/ACK
后的最新意图保留、facts 后读取及公平恢复完成。两个身份／generation 和小范围是有限证据，
不是无界证明或物理宿主截图。已有 `TerminalClientViewport` 保留 worker／lease／行验证权威；
绝对意图细化 `PressJump`，零偏移细化 `PressExit`。

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

| 生产面                                      | 模型对应                                                                                                                           |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `terminal_session/terminal_viewport.rs`     | `TerminalViewport` 核心：`Mode`、`facts`、`window`、`ack`、`read_flight`、`committed`、`pending_nav`、`pending_commands`、期限     |
| `terminal_session/mailbox.rs`               | `QueuedCommand::Viewport`、`ViewportStep`、`WorkerEvent::ViewportOutcome`                                                          |
| `terminal_session/connection/native.rs`     | `run_viewport` 执行器：demand 腿、read、ack、恢复（租约围栏）                                                                      |
| `terminal_session/ui.rs`                    | `enter_history` 闸门、`history_key`、`history_wheel`、kind-53 分支、`KeyedProjection.viewport`/`history_sequence`/`read_watermark` |
| `terminal_input/dispatcher.rs`              | `history_key` 端口路由（先于普通 pending 字节）；wheel 移交                                                                        |
| `client_view.rs`、`client_view/bindings.rs` | `ClientAction::EnterHistory`、`Ctrl+B PageUp` 绑定、help 优先级                                                                    |
| `tui_pointer.rs`                            | `PointerProbe.viewport_non_live`、非 Live wheel 滚动与非滚轮事件消费                                                               |
| `tui_render.rs`、`tui_render/viewport.rs`   | 共享样式单元格画笔、指示、位置/提示末行、generation/列围栏                                                                         |
| `localization.rs`                           | 历史提示、指示、位置与提示字符串（en/zh）                                                                                          |
| `formal/tla/TerminalClientViewport.tla`     | 覆盖阻塞、水位、暂存受理、有类型结果、事件、静默、界限与恢复的状态机模型；注册于 `scripts/tla/models.json`                         |

## 验证

模型为有符号导航导入 `Integers`，并将所有已受理命令表示为三元组。无参数的 Enter、Recover
和 Input 使用 `<<kind, 0, 0>>`，Read 与 Ack 保留序号和 generation。这一一对应的编码保持
队列长度、顺序、受理与执行，避免 TLC 在字符串与元组之间进行相等比较，不改变生产命令
的表示。注册配置检查完整可达图、全部声明的不变式、时序性质、公平条款及必需动作覆盖；
有限域界仍是证据限制。

### 观察量投影

模型保留 34 个状态变量，投影掉最近动作标签 `action`、最近发送增量见证
`emitted_delta` 和只写的输入标记 `delivered`；三者均不参与迁移守卫。
命名 `Next` 动作和全部 26 个弱公平义务保持不变。
每个公平动作都改变物理命令/结果队列、暂存槽、模式、事件待处理标记或期限，因此删除
这三个见证保持其启用性和非 stutter 步。仅改变见证的非公平动作投影为原已允许的 stutter。
在这些读者与生产者条件下，每条原受支持行为均投影到保留状态的规范。这是经过检查的
表示推导，不是经 kernel 检查的普遍定理。

`EmitLeavesStagedRead`、`CommittedReadGuard`、`BoundedDelta` 和 `InstallGates` 现在检查
对应命名迁移的边，不再保留诊断状态。Committed 发送的有符号增量为
`nav_delta - nav_delta'`，等于原 `chunk`；Tail 与 Init 产生零，其余迁移不产生增量。
配置保留这些 19 个断言名，分为 11 个状态不变量与 8 个时序性质。`PaneIncarnationFence`
检查既有身份主张，`OutcomeProductionOwned` 检查下文说明的生产者边界，因此当前时序性质
总数为十个。常量、动作覆盖和公平性不变，没有用状态约束缩窄探索。
新增见证读者或增量生产者时必须
重新核对本映射。

现有 20 状态反例的重放检查这段已记录前缀上的物理投影与转换后的断言。驱动器推进
固定记录，不证明原无限公平规范。五个从实际源码派生的故障分别检查非法提交提升、
错误暂存序号、清掉已提交锚点、超界 Committed 增量以及安装后未释放 flight。
这些定向检查补充注册的完整状态图检查与原生核心测试，不替代两者，也不意味着独立审查。

`Lost` 表示此 incarnation 的 Client 核心已经不存在。迟到的历史事件被消费，但不改变
Client 事实、已接受事件序号或暂存命令；受理要求仍拥有核心。这对应
`NativeUi::clear_pane_projection`、`apply_poll_result` 中的精确键核对以及
`apply_viewport_outcome` 中的核心存在性核对。宿主状态和已经受理的队列继续独立表示。
已知可达前缀后依次移除、锚点驱逐、事件投递与尝试受理，可区分缺失围栏和受支持迁移；
UI 测试也在投影移除后投递迟到事实与结果。

丢失核心投影仅在此 incarnation 进入 `Lost` 后，把已消失的 17 个核心字段归到重置值：
事实、窗口/ack、flight、导航、refresh、期限、重试标记、恢复完成、绘制 generation 与
暂存。其余 17 个字段保留模式、已签发 read 水位、已受理队列、结果、已接受事件
见证以及宿主状态。这项核心重置仅适用于 Lost。Lost 模式的守卫不会读取已消失字段来生成新核心或
线上命令；结果与事件消费保持保留状态。参考中的丢失核心期限只能执行有限次 tick，
投影为 stutter。其他公平步骤仍改变保留状态或保持禁用，`EnteredResolves` 只依赖模式。
在此 incarnation 终态和读者闭包下，归零保持受支持的安全与进展观察。这是经过检查的
表示推导，不是经 kernel 检查的普遍证明。

单步边界检查器对 20 个已记录状态及其移除后的对应状态，核对参考 `Next`、全部 11 个
状态断言和 8 个安全动作性质。其 357 个不同状态是局部证据，不是完整公平规范。
三个负向对照分别检出清掉已受理线上命令、归零活动核心以及丢失已签发 read 水位。
注册的完整状态图检查保留全部常量、动作覆盖和 26 个公平义务，不增加探索约束。

### 结果生产者归属

[原生连接](../src/terminal_session/connection/native.rs)的 `run_ready_queue` 拥有唯一活动命令 lease。其 `run_viewport` 执行
`viewer_for_ready_lease`、解码 Read 响应，并在结束 lease 前调用
[mailbox](../src/terminal_session/mailbox.rs) 的 `publish_notice_before`。[窗口解码器](../src/terminal_session/terminal_viewport/window.rs)的 `decode_viewport_window`
校验 kind-46 响应、角色、correlation 存在性、编码、payload 边界和 epoch。核心的
[`TerminalViewport::window`](../src/terminal_session/terminal_viewport.rs) 随后检查 attachment 和行路由序号。合法解码但行序号外来的
窗口触发恢复；未经请求或格式错误的信封不会独立发布 `ViewportOutcome`。

因此，`HostFaultWindow` 只损坏 FIFO 任一槽中已有、尚未消费窗口的序号。它保留槽位置、
generation、宿主 pin 和全部队列长度。这表示错误响应 payload，不会修改生产中的不可变
窗口。故障选择可以移回 `ExecuteRead`：中间的受理与执行闸门只检查 FIFO 长度／类型，
读取序号的消费者尚未运行。零表示已签发域之外的外来序号，不是解码器接受的实际线上零值。

独立的故障结果生产者不符合该 worker。它可以令 FIFO 长期在一个与两个结果之间切换，
使执行仅间歇启用。在弱公平下，满命令队列便可令暂存 Recover 一直饥饿，核心始终处于
Entering。这是模型环境修正，与下述表示投影分开。新增的 `OutcomeProductionOwned`
要求每次结果 FIFO 增长，都通过 `ExecuteEnter`、`ExecuteRead`、`ExecuteAck` 或
`ExecuteRecover` 消费一个已受理命令。全部原安全／进展性质和公平义务仍然必需；生产代码不变。

准备好固定版本 TLC jar 后运行
`node --test apps/orchbee/formal/terminal-client-viewport.test.mjs`，或与模型 runner 一样
提供 `ORCHBEE_TLA2TOOLS_JAR`。包内回归到达重新入场状态：四个旧 Input 排在 Enter 前，
Recover 暂存在满队列之后。可达终止见证禁用全部原公平动作，因此无限 stutter 延续满足公平性。它重新引入独立
生产者以在原性质下复现原无限饥饿循环，并检查新增所有权断言拒绝该生产者。这些定向
检查补充完整模型运行，不替代其交互覆盖或完整公平图。
另外两条轨迹分别损坏已发布与阻塞中的窗口槽，在消费错误窗口直到普通恢复期间，
保持 FIFO 长度／位置和已服务的宿主 pin。

### 表示归一化

完整检查使用原边界：`MaxSeq = 3`、`MaxEvent = 3`、`MaxGen = 2`、
`MaxDelta = 5`、`DeltaBound = 4`、`MaxQueue = 5`、`MaxDeadline = 2`。保留全部 40 个必需
动作、11 个状态不变量、10 个时序性质以及 26 个弱公平义务。原有 9 个性质
（8 个安全动作性质和 `EnteredResolves`）全部保留；`OutcomeProductionOwned` 是唯一新增性质。
常量、8 个 TLC worker 和 3,600,000 ms 超时均不变。没有状态约束、对称集或 TLC `VIEW`。

专门运行此模型并使用 8192 MiB 堆上限时，执行
`node scripts/tla/run-models.mjs --models native-terminal-viewport --jobs 1 --max-heap-mib 8192`。
这一受支持的执行资源设置不改变任何模型边界或性质；结果记录实际分配的堆大小。
验收要求探索队列为空、时序检查完成、全部必需动作覆盖通过，且运行器退出码为零。
即使所有必需动作均已到达，超时或未完成的运行仍然是失败。

五项表示选择避免因不可区分的历史值而成倍增加状态：

- `delivered` 除布尔类型检查外没有读者。`ExecuteInput` 仍准确移除队首命令；
  `InputAppendedUnderCover` 仍检查实际队列受理。删除该标记保留这一公平动作的非 stutter 队列变化。
- `host_committed` 用 0 或 1 记录存在性。唯一决策读者 `HostEvict` 判断是否为零。
  成功的 `ExecuteAck` 仍创建锚点，驱逐仍清除锚点。准确 read 身份继续保留在命令、结果、
  已安装窗口和 `host_read_watermark` 中。
- 待处理事件将 `none` 和 `evicted` 锚点均表示为 `evicted`。唯一决策读者判断
  `evt_anchor /= "committed"`，因此两个原始分支的恢复行为均保留。协议仍区分这两个标签。
- `FactsArrive` 之后，`evt_gen` 和 `evt_anchor` 取初始值。唯一读者只消费待处理事件；
  `HostEmitEvent` 在下次消费前覆盖两者。`evt_seq`、`last_evt_seq` 和 `observed_seq`
  均保留，包括 incarnation 丢失之后。
- Recover 受理且 `mode = "recovering"` 后，`retry_used`、`halve_used`、`win_gen`、
  `committed` 和 `need_facts` 均归零。下文说明这些字段的受保护读者与 reset 边界。

对于符合生产结果归属模型内的前四项选择，投影删除输入标记，将正的宿主锚点序号映射为 1，将待处理事件的两个缺失锚点标签映射为
`evicted`，并将已消费事件的 payload 映射为初始值。其他字段完全相同。每个原命名步骤
均投影为同名步骤：消费在清除前读取原 payload，锚点接受／驱逐保持存在性，其余读写
均与投影可交换。反向地，归一化步骤可以恢复任一缺失锚点标签、已接受确认的序号以及
已丢弃的 payload／标记历史。这保持受支持的安全观察。每个公平动作仍改变未被投影的
队列、暂存槽、模式、待处理标记或期限，因此保持其启用性和非 stutter 发生。
`EnteredResolves` 观察未改变的模式。这是经过检查的带动作标签表示推导，不是经 kernel
检查的普遍证明。

单步参考检查在两边保留相同的修正后故障生产者，并枚举事件消费、发送、确认和输入的边界上下文，包括全部原锚点标签及
序号／generation 边界。正向探索比较投影后全部 11 个状态断言与全部 40 个命名动作谓词的真值。
反向探索从每个投影后的 fixture 出发，检查每个归一化后继都存在从该原 fixture 出发的同名参考步骤，
恢复参考历史，并确保后继能准确投影回来。负向对照分别擦除宿主锚点存在性、待处理事件 generation
或事件水位；反向对照允许没有锚点时驱逐。每项都必须被拒绝。
这些有限局部检查支持读写推导；注册的完整公平状态图检查提供有界安全和活性证据。
新增这些字段的读者或公平动作时必须重新审查投影。运行时行为不变。

### 仅恢复阶段归一化

第五项投影只在 `mode = "recovering"` 时改变上述五个字段；其他字段完全相同。
Entering 或 Viewing 中尚未受理的暂存 Recover 保留原值。进入 Recovering 只能通过
`RecoverAdmitted`：由 `AdmitStaged` 受理 Recover，或由 `KeyTyped` 受理 Recover 时调用。
这些调用设置规范值；其他分支保留字段。这是对核心保留数据的表示归一化，不是在生产
`TerminalViewport::command_admitted` 中增加一次 reset。

准确的决策读者为：

- `retry_used`：`ReadTypedRetry`；`halve_used`：`ReadLimit`。两者都要求活动模式和
  `~RecoveryPending`。在普通 read 结束后清除任一标记会错误地更新每意图预算；
  因此它们一直保留到受理进入 Recovering。
- `win_gen`：`AckEmit`，要求 Entering 或 Viewing。`InstallGates` 仍在每条安装边观察原 generation。
- `committed`：`EmitCommitted`、`ApplyCurrentFacts` 和 `ClientResize`。首尾两者要求
  Entering 或 Viewing；恢复期间收到 facts 时，先进入 `RecoveryPending` 的位置不变分支，
  不执行两个依赖 committed 的决策。
- `need_facts`：`ReadUnblocked`、`InstallWindow`、`StaleWindow`、`ClientResize` 和 `SeqExhaust`。
  调用者或闸门均排除 Recovering。恢复期间 facts 消费可以写入零，保持规范值。

`Init`、`ResetCore` 和归一化后的 `RecoverAdmitted` 将全部五个字段归零。其他写者准确列举如下：

| 字段         | 其他写者                                                                           |
| ------------ | ---------------------------------------------------------------------------------- |
| `retry_used` | `ScrollDelta`、`PressJump`、`ReadTypedRetry`、`ClientResize`                       |
| `halve_used` | `ScrollDelta`、`PressJump`、`ReadLimit`、`ClientResize`                            |
| `win_gen`    | `ApplyCurrentFacts`、`InstallWindow`、`AckRejected`、`ClientResize`                |
| `committed`  | `ApplyCurrentFacts`、`AckAccepted`、`AckRejected`                                  |
| `need_facts` | `PressEnter`、`ApplyCurrentFacts`、`StaleWindow`、`ReadTypedRetry`、`ClientResize` |

这些活动处理器均无法在 Recovering 中恢复字段。唯一出口为 `ConsumeEnterRejected` 和
`FrameArrive` 到 Live，或 `FactsArrive`、`ExecuteEnter`、`ExecuteRecover`、`PaneDetach`
到 Lost；每个出口都直接或通过 `LoseCore` 调用 `ResetCore`，之后新 episode 才能再次观察它们。
`recover_done`、facts generation、窗口序号、acknowledgement 状态、全部线上队列、宿主 pin
和两个水位均保留。生产 `surface`、`position` 和 `committed_window` 同样不会暴露恢复中的窗口。

此投影的正向／反向边界检查使用已经修正结果归属、并应用前四项归一化的模型作为参考。
它比较全部 40 个动作谓词、11 个状态断言和 9 个安全动作断言，反向步骤恢复五个字段的
参考历史。在 Recovering 之外也擦除字段的提前 reset 对照必须失败。每个公平步骤仍改变
未投影的队列、暂存槽、模式、事件标记或期限；`EnteredResolves` 观察未改变的模式。
26 个公平义务均不变。这些有限检查和读写推导支持投影；仍必须检查完整公平图。
新增恢复期间的读者或不经过 reset 的出口时必须重新审查此投影。

| 面           | 证据                                                                                                                                                                        |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 核心迁移     | `terminal_viewport/tests` — 入场、导航合并、±4096 残量步、安装闸门、ack 提升、带类型拒绝、恢复、期限                                                                        |
| 对抗序列     | 确定性 trace 测试 — 流浪/外来/陈旧窗口、带类型 stale 重试、减半、ack 拒绝、底部退出、绝对跳转                                                                               |
| 输入边界     | `client_view` 测试 — `Ctrl+B PageUp` → `EnterHistory`、help 优先级；dispatcher 消费经端口缺省路径验证                                                                       |
| 呈现         | surface 围栏归核心所有（`surface()` 每次复核 generation/列数）；画笔覆盖随 shell 渲染路径                                                                                   |
| 结果通知接缝 | `ui/viewport_tests` — `publish_notice_before` → `take_event_before` → `apply_viewport_outcome` 回归保持协议开放；状态中性的 applied Enter 使 Entering 核心保持不变          |
| 模型         | `formal/tla/TerminalClientViewport.tla` + `.cfg` 注册于 `scripts/tla/models.json`；合并检查运行完整有界规范和必需动作覆盖，包括暂存 flight、输入覆盖与丢失 incarnation 围栏 |
