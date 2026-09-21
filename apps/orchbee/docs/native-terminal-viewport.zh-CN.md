# 原生终端历史视口

## 范围与归属

本参考规范原生 Client 的保留历史视口：以 `Ctrl+B PageUp` 进入的按窗格滚动回看模式、其
demand 双腿、行窗口 read/acknowledge 握手、导航意图、恢复，以及呈现面如何生成与围栏。
与 `terminal_session/terminal_viewport.rs`、`terminal_session/mailbox.rs`、
`terminal_session/connection/native.rs`、`terminal_session/ui.rs`、`terminal_input/dispatcher.rs`、
`client_view.rs`、`client_view/bindings.rs`、`tui_pointer.rs`、`tui_render.rs` 及其
`tui_render/viewport.rs` 画笔、`localization.rs`、`formal/tla/TerminalClientViewport.tla` 共同维护。
协议契约归 `crates/terminal-protocol`（`row_window.rs`、`history.rs`、`row_access.rs`）
与 `crates/client-server-protocol/src/viewer.rs`（`TerminalViewerAction` 面）所有；
本文档消费该契约，绝不扩展它。

本切片刻意不实现历史上的复制/搜索/选择、样式渲染、剪贴板集成或持久化——这些是另行
接受的决策。

## 状态机

每个窗格拥有独立的 viewport 核心（`TerminalViewport`，每 `PaneRef` 一个；隐藏或后台
窗格不受焦点窗格模式影响）。无 viewport 状态即 `Live`；核心存在时其 `mode` 为：

| 模式          | 含义                                                                                        |
| ------------- | ------------------------------------------------------------------------------------------ |
| `Entering`    | 历史+投影 demand 双腿已受理；等待 `HistoryChanged` 事实与首个行窗口                             |
| `Viewing`     | 已安装 committed 窗口；read 在保留历史中导航                                                  |
| `Recovering`  | 恢复 demand（`FollowingBottom`+`Active`）已受理或在途；合法新帧结束它                          |

`Live` 即无核心。分离窗格无核心；viewer 丢失的窗格由 worker 的 demand 路径
（`settle_pane_viewer_lost`）停靠，不由核心判定。

## 入口可用性

`Ctrl+B PageUp` 请求焦点窗格进入历史。除非该窗格最近一帧 live 帧满足以下全部条件，
请求一律拒绝——显示诚实的本地化 `HistoryUnavailable` 提示，且线上发出零条 `Enter`
命令：已知帧存在、`scrollback_rows > 0`、窗格不在备用屏幕、且帧至少携带一条可读行。
屏幕自身行绝不与保留历史混为一谈：`total_rows` 只来自服务过的窗口，不取自
`scrollback_rows`。

## 命令与 demand 双腿

`QueuedCommand::Viewport` 每窗格携带一个逻辑 `ViewportStep`，由 `run_viewport` 依序执行，
每条腿前重新核对 incarnation+attachment 租约：

- `Enter` — `SetHistoryDemand(ViewingHistory)` 再 `SetProjectionDemand(Paused)`。第一腿被拒
  或不可用 → 发布 `ViewportOutcome::Enter{applied:false}` 且不走第二腿。第二腿失败或结果
  未知 → 执行一次复位腿（`SetHistoryDemand(FollowingBottom)`）；复位失败或不可用 →
  窗格判错。完全受理 → 发布 `ViewportOutcome::Enter{applied:true}`。
- `Read{origin,row_count,revision,generation,request_sequence,maximum_bytes}` — 租约约束的
  `ReadViewportRows` 动作。`Record` 载荷发布为
  `ViewportOutcome::Window{request_sequence,record}`；`RowsRejected{reason}` 发布带类型理由的
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
- `Committed{delta}` read 每步界于 ±4096；更大的移动拆分为残量限界步，一次只发一步。
  `Tail{rows_before}` read 仅服务首次入场与绝对跳转。

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
`PaneRef` 的单个 `Input` 命令转发——绝不在两条 demand 腿之间插入，拒绝后绝不重放。
`Recovering` 窗格直接转发该字节（恢复已受理）。非 Live 窗格本地消费历史导航键
（`PageUp`、`PageDown`、`Home`、`End`、`Esc`）与鼠标滚轮；Live 窗格保留完整的应用输入
与鼠标跟踪。`Ctrl+B PageUp` 进入历史；help 打开时 `PageUp` 仍归 help。

凡以非 Live 窗格为目标的非滚轮指针事件一律由 client 消费——落在焦点窗格上的按下、把
焦点移入该窗格的按下、排队中的 pending-focus 重放、手势延续，以及手势中途 Live→非 Live
转换后才结束的拖拽。它们都不编码应用鼠标字节；非 Live 手势在释放时终结。滚轮保持自身
契约：在保留窗口内本地滚动，落在窗格其他位置则被消费且不关闭覆盖层。

## 呈现

合法文本行窗口以纯文本（无单元格样式）绘制，受窗格内容几何约束。旧 generation 或旧
列数的数据绝不绘制：surface 访问器每次绘制都以当前事实复核已安装窗口的 generation 与
列数。`Entering` 在历史徽标下保留最近已验证 live 帧；`Viewing` 无当前窗口 → 清屏加徽标；
`Recovering` → 清屏加恢复指示。叶区末行左侧显示本地化位置读数（当前已验证窗口的
`start_row`/`total_rows`——已过期或陈旧窗口不显示），右侧常驻本地化返回实时提示；
窄窗格按字素边界降级，提示保持优先。`Esc` 返回实时输出。

## 不变式

- `OneReadFlight`：至多一个在途 read；宿主 pending 绝不被二次占用。
- `AckAfterInstall`：`RowsApplied` 只对已安装窗口、匹配序号、精确一次。
- `CommittedOnlyAfterAck`：`Committed` read 要求已受理的应答。
- `NoReadWhileAck`：应答待受理或在途期间不发出 read。
- `SurfaceCurrent`：绘制的行永远匹配当前 generation 与列数。
- `RecoverBeforeInput`：普通输入仅在恢复 demand 受理后入队。
- `EveryEventAcked`：每个合法 `HistoryChanged` 精确应答一次。
- `MonotoneEvents`：接受的事件序号严格递增。
- `BoundedDelta`：每个发出的 `Committed` 步都在 ±4096 内。
- `HonestModes`：`Live` 模式绝不消费历史键或滚轮；非 Live 绝不转发应用键或跟踪。
- `PaneIncarnationFence`：每个线上动作与事件都解析精确 `PaneRef` 与 attachment 租约；
  死 incarnation 的结果丢弃。

## 源码对应

| 生产面                                     | 模型对应                                                                                                                  |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| `terminal_session/terminal_viewport.rs`    | `TerminalViewport` 核心：`Mode`、`facts`、`window`、`ack`、`read_flight`、`committed`、`pending_nav`、`pending_commands`、期限 |
| `terminal_session/mailbox.rs`              | `QueuedCommand::Viewport`、`ViewportStep`、`WorkerEvent::ViewportOutcome`                                                    |
| `terminal_session/connection/native.rs`    | `run_viewport` 执行器：demand 腿、read、ack、恢复（租约围栏）                                                                |
| `terminal_session/ui.rs`                   | `enter_history` 闸门、`history_key`、`history_wheel`、kind-53 分支、`KeyedProjection.viewport`/`history_sequence`/`read_watermark` |
| `terminal_input/dispatcher.rs`             | `history_key` 端口路由（先于普通 pending 字节）；wheel 移交                                                                 |
| `client_view.rs`、`client_view/bindings.rs` | `ClientAction::EnterHistory`、`Ctrl+B PageUp` 绑定、help 优先级                                                              |
| `tui_pointer.rs`                           | `PointerProbe.viewport_non_live`、非 Live wheel 滚动与非滚轮事件消费                                                          |
| `tui_render.rs`、`tui_render/viewport.rs`   | 纯文本窗口画笔、指示、位置/提示末行、generation/列围栏                                                                       |
| `localization.rs`                          | 历史提示、指示、位置与提示字符串（en/zh）                                                                                    |
| `formal/tla/TerminalClientViewport.tla`    | 覆盖阻塞、水位、事件、静默、界限与恢复的状态机模型；注册于 `scripts/tla/models.json`                                          |

## 验证

| 面                       | 证据                                                                                                                                          |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 核心迁移                  | `terminal_viewport/tests` — 入场、导航合并、±4096 残量步、安装闸门、ack 提升、带类型拒绝、恢复、期限                                              |
| 对抗序列                  | 确定性 trace 测试 — 流浪/外来/陈旧窗口、带类型 stale 重试、减半、ack 拒绝、底部退出、绝对跳转                                                      |
| 输入边界                  | `client_view` 测试 — `Ctrl+B PageUp` → `EnterHistory`、help 优先级；dispatcher 消费经端口缺省路径验证                                              |
| 呈现                      | surface 围栏归核心所有（`surface()` 每次复核 generation/列数）；画笔覆盖随 shell 渲染路径                                                          |
| 结果通知接缝               | `ui/viewport_tests` — `publish_notice_before` → `take_event_before` → `apply_viewport_outcome` 回归保持协议开放并推进 Entering 核心               |
| 模型                      | `formal/tla/TerminalClientViewport.tla` + `.cfg` 注册于 `scripts/tla/models.json`；`ConsumeRecovered`/`ExecuteAck`/`ResetCore` 机械修正已映射，TLC 仍由门禁负责人待跑 |
