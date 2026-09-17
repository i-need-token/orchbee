# TUI Scrollback 视口

| 字段     | 值                                                                                                                                                                                                                                                                               |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 提案评审中；下文 wire 语义已对照 `crates/terminal-protocol`、`packages/terminal-protocol/references/protocol.md` 与 `apps/terminal-host` 核实；Client-local 视口模型将 `packages/terminal` projected-terminal 已验证的行为移植到 TUI；键位、剪贴板途径与呈现细节保持 provisional |
| 负责人   | TUI 视口体验，以及 Rust `orchbee` Client、OBCS/OBCN relay 词汇与 `packages/terminal` 的维护者                                                                                                                                                                                    |
| 现行权威 | `crates/terminal-protocol` 与 `apps/terminal-host/docs/runtime-model.md` 拥有 wire 与 Host 语义；`apps/orchbee/README.md` 拥有 TUI 不变量                                                                                                                                        |
| 父设计   | [`native-command-surface.md`](native-command-surface.zh-CN.md) 拥有 TUI-as-peer-Client contract；[`native-runtime-topology.md`](../platform/native-runtime-topology.zh-CN.md) 拥有 Client/Server/Worker/Host relay 拓扑                                                          |
| 决策权威 | 产品决策人通过 `m3-input-design` 裁决键位、剪贴板途径与呈现细节；本提案只决定视口模型及其对既有 wire 语义的使用                                                                                                                                                                  |
| 退出条件 | TUI 用户可以在不改 wire 协议、不改共享状态的前提下上翻已保留的 terminal 历史并复制历史文本；当实现面写入所属 package 指南后删除本提案                                                                                                                                            |

英文规范原文：
[tui-scrollback-viewport.md](tui-scrollback-viewport.md)。
如有冲突，以英文为准。

## 目的与范围

当前 TUI 只渲染活动 terminal 帧：`TerminalProjection` 保存最新的
`TerminalTextFrame`，由 `tui_render/terminal_cells.rs` 绘制。用户无法上翻已保留的输出，
也无法复制历史文本。本提案定义 Client-local 的 scrollback 视口，仅使用既有 wire 语义
弥合这一差距。

范围内：

- 数据面：TUI 如何通过既有 attachment `read` capability、`HistoryDemand`、`ReadRows`、
  `RowsApplied`、selection 与 selection export 获取历史行。
- Client-local 视口状态机、其 offset 与锚定语义、新帧到达时的行为、自动回到底部规则，
  以及 session 切换或 retirement 时的失效语义。
- provisional 交互键位、选择与复制、界限与失败行为，以及与并行 `m3-terminal-read`
  实现共用的语义层边界。
- 将既有 wire 语义送达 TUI 所需的内部 relay 增补。

范围外：

- 不新增或修改 wire 协议；本文使用的每条消息都已存在。
- 不实现代码。
- 不做完整的 scrollback 搜索 UI 设计；搜索保留为后续切片（见
  [保留决策与缺口](#保留决策与缺口)）。
- 不修改 `apps/`、`crates/`、仓库级 `docs/`、其他 proposals 或 `.oxlintrc.json`。

## 已核实的 wire 语义

本节所有消息 kind、role 与界限均经源码核实。OBTP 是 Client 到 Terminal Host 的 wire；
其中任何内容都不变。

### Attachment capability 与 projection demand

- viewer 通过 `AttachTerminal` 请求 `AttachmentCapabilityState { read: true, input: false, resize: false }`
  且不申请 resize lease；Host 以 `AttachmentCapabilities` 应答授予的不可变 capability。
  现有 TUI viewer attach 请求的正是该授予，scrollback 不需要新 capability。
- `SetProjectionDemand { demand: Active | Paused }` 控制 Host 是否推送 frame 事件；
  `ProjectionDemandAccepted` 回显已接受的 demand，`ProjectionBlocked` 报告 projection
  捕获失败。viewing history 期间暂停 demand 是停止 live-frame 搅动的机制。

### History demand、事件与观测

- `SetHistoryDemand { demand: FollowingBottom | ViewingHistory }` 选择 attachment 的
  history 模式；`HistoryDemandAccepted` 回显它。切到 `ViewingHistory` 会布防 history
  跟踪并把 attachment 标脏，使下一个 `HistoryChanged` 发布当前状态；切回
  `FollowingBottom` 会释放 pending 与 committed anchor。
- `HistoryChanged` 是事件，携带 `attachment_id`、正值 `generation`、`revision`、正值
  attachment-local `sequence`、`total_rows`，以及取值 `{ Valid, None, Evicted }` 的
  `anchor_status`。`Valid` 表示 committed anchor 存活；`None` 表示不存在 committed
  anchor；`Evicted` 表示 retained-row 逐出、screen 替换或 pin 丢失将其销毁。
- `HistoryObserved` 通过回显事件的 `sequence`、`revision` 与 `generation` 确认一个事件。
  事件在等待确认期间会合并；陈旧或重复的确认是幂等 no-op，绝不清除更新的 flight。
- mutation、generation 变化（resize）、screen 变化与逐出都会把 viewing attachment
  标脏，并以相应的 anchor status 重新发布 `HistoryChanged`。

### Row window 与 anchor

`ReadRows` 按 attachment、`detail`（`Render` 或 `Text`）、`purpose`（`Viewport`、`Search`
或 `Buffer`）、正值 `request_sequence`、精确 `generation`、`revision`、
`revision_policy`（`Exact` 或 `CurrentView`）、`row_count`、`maximum_bytes` 以及一个
origin 界定：

- `Absolute { row }` 命名一个已保留的绝对行。
- `Oldest { offset }` 命名相对最旧已保留行的偏移。
- `Tail { rows_before }` 命名所请求视口之下保留多少行。
- `Committed { delta }` 对 attachment 最后 applied 的 anchor 施加有界有符号 delta（`±4096`）。

策略规则（已在 `row_access.rs` 与 Host runtime model 中核实）：

- `Exact` 要求所请求 revision 等于当前 Host revision，对每种 purpose 与 origin 有效。
- `CurrentView` 仅对 origin 为 `Tail` 或 `Committed` 的 `Viewport` 请求有效。其 revision
  是观测游标而非准入栅栏：Host 在持有 terminal mutex 时按当时视图解析 origin，并返回
  捕获到的当前 revision、generation 与总行数。持续的 PTY 输出因此不会饿死交互式读取。
- 两种策略都要求精确的当前 generation，因此 resize 或 screen-generation 转换不会
  重新解释旧请求。
- 每个 attachment、每条 purpose lane 至多一个请求在飞；`Viewport`、`Search` 与
  `Buffer` lane 相互独立。
- Host 只解析一次 origin，返回一个连续的 `RowsSnapshot` row-window 记录
  （`PayloadEncoding::RowWindow`）。响应只是 pending anchor；匹配的 `RowsApplied` 请求
  把它提升为 committed anchor，`Committed` origin 与 selection gesture 据此解析。
- resize 放弃 generation-pinned 的 pending 捕获；committed anchor 跟随有效的 Ghostty
  reflow pin。screen 替换、detach 或逐出会使 anchor 失效，并相应地重新发布
  `HistoryChanged`。

`RowsSnapshot` 头部报告视口所需的字段：detail、columns、host epoch、attachment、
request sequence、捕获的 revision、捕获的 generation、已保留总行数、绝对首行、
连续行数与 body 布局。`Render` detail 连接一个 OBPB projection 快照加 OBKG 图形；
`Text` detail 存储纯文本行与可选逻辑映射，无样式。

### Selection 与 export

- `SelectionGesture` 动作（`Start`、`Update`、`End`、`Cancel`）携带 attachment 的精确
  generation 与视口相对行/列端点，但不带 revision。Host 在持有 terminal mutex 时于处理
  当下解析每个点：attachment 在 viewing history 时对着 committed tracked anchor 解析，
  否则对着活动视口解析。因此 client 只提供表面坐标；它从不自行换算历史行。
- `SelectionCommand`（`Clear`、`SelectAll`、`SetRange`、`SelectOutput`）携带精确的
  generation 与 revision。发布的 `SelectionChanged` 端点是绝对行/列对；状态为
  `Active`、`None` 或 `Evicted`，其中 `Evicted` 表示已保留行已消失。
- `SelectionExport` 只产出纯文本：`SelectionExportStarted`，然后至多一个在飞的
  `SelectionExportChunk` 事件（至多 1 MiB 加 32 字节头），由
  `SelectionExportChunkApplied` 配速，以 `SelectionExportFinished` 或带原因
  `ClipboardFailed`、`Disposed`、`Replaced`、`User` 的 `SelectionExportCancel` 结束。
  导出总量上限 16 MiB。wire 不执行任何平台剪贴板效果；写剪贴板是 Client 的本地职责。

## Relay 面与所需增补

wire 已完备，但当前内部 relay 无法把它送达 TUI。已核实的事实：

- OBCS `TerminalViewerAction` 与 OBCN `TerminalViewerAction` 目前只携带 `Attach`、
  `AttachBinding`（OBCS）、`Poll`、`FrameApplied`、`NeedSnapshot` 与 `Detach`。没有动作
  能到达 history、row-access、selection 或 export 控制。
- Worker 的 `terminal_viewer_operation.rs` 把每个 viewer 动作映射为一次关联 OBTP
  `exchange()`；`request_id` 复用为 OBTP `CorrelationId`。同一模式可以承载本提案
  所需的每个控制。
- Worker 的 `HostViewers` 只把 frame 记录（kind 41/42）与 `ProjectionBlocked`
  （kind 49）路由进每个 viewer 的一个记录槽。第二个未交付记录会使该 viewer 退役。
  `HistoryChanged`（kind 53）、`SelectionChanged`（kind 62）、`SelectionExportChunk`
  （kind 65）与 `SelectionExportFinished`（kind 67 事件）目前会被当作意外事件拒绝。
- `TerminalViewerOutcome::Record(Vec<u8>)` 在两跳上都已返回不透明记录字节，因此
  poll 交付的事件记录与按请求关联的响应记录不需要新的 outcome 形状。
- `MAXIMUM_VIEWER_RECORD_BYTES` 在两跳上都是 64 字节 + 36 MiB；row window 合法可达
  44 MiB，因此 client 必须把 `maximum_bytes` 限在 relay 记录上限之下。

所需增补（内部 relay 词汇，仅增量——OBTP 本身不动）：

| 层级                                | 增补                                                                                                                                                                                                                    |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OBCS 与 OBCN `TerminalViewerAction` | 镜像 OBTP 控制的新 typed 变体：`SetHistoryDemand`、`HistoryObserved`、`ReadRows`、`RowsApplied`、`SelectionGesture`、`SelectionCommand`、`SelectionExport`、`SelectionExportChunkApplied`、`SelectionExportCancel`      |
| Worker viewer 路由                  | 每个 viewer 一个有界 FIFO（建议深度至少 8）存放非 frame 事件记录——`HistoryChanged`、`SelectionChanged`、`SelectionExportChunk`、`SelectionExportFinished`——与单个 frame 槽分离；溢出即退役该 viewer，与既有饱和策略一致 |
| Worker exchange 映射                | 每个新动作执行一次关联 OBTP `exchange()`，并经既有 outcome 词汇返回响应记录（或 `FeedbackAccepted` 式确认）                                                                                                             |
| Server local-terminal service       | 新动作与 outcome 的透明透传，与既有 viewer relay 一致                                                                                                                                                                   |
| TUI client                          | `LocalClient::submit_terminal_viewer` 为新动作的管道，外加 `Render` detail window 的有界 Rust OBRW 解码器（OBRW 头加内部 OBPB 快照，映射到渲染器已绘制的 cell 类型；OBKG placement 可在首个切片中跳过）                 |

## 与 `terminal read` 的共享语义边界

`m3-terminal-read` 在同一批实现一次性 `orchbee terminal read` CLI，基于相同的 wire
原语。边界在 purpose lane 与生命周期，而非另一种协议。

| 关注点           | 共享 wire/transport 层（两个消费者）                                                          | TUI scrollback 视口（本提案）                                                                  | `terminal read` CLI（兄弟任务）                                                         |
| ---------------- | --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Attachment       | `read` capability attach；`AttachTerminal`/`DetachTerminal`；capability/epoch 校验            | 复用既有的长生命周期 viewer attachment                                                         | 可持有短生命周期 read attachment，输出后 detach                                         |
| Demand           | `SetHistoryDemand`/`HistoryDemandAccepted` 语义                                               | 会话 viewing 期间保持 `ViewingHistory`                                                         | 仅读取期间 `ViewingHistory`，随后恢复                                                   |
| Row access       | `ReadRows`/`RowsSnapshot`/`RowsApplied`；generation/revision/request-sequence 身份；lane 隔离 | `purpose: Viewport`、`CurrentView` 策略、`Tail`/`Committed` origin、`Render` detail、单 flight | `purpose: Buffer`、`Exact` 策略、`Absolute`/`Oldest` origin、`Text` detail、分页 flight |
| Events           | `HistoryChanged`/`HistoryObserved` 精确确认                                                   | 确认每个事件；anchor status 驱动 refresh 或 recover                                            | 确认读取期间收到的事件                                                                  |
| Selection/export | Gesture、command、`SelectionChanged`、分块纯文本导出                                          | 通过 committed anchor 驱动复制                                                                 | 可用 export 或 `Text` 读取产出输出                                                      |
| 身份规则         | Attachment、host epoch、generation、revision、request sequence、event sequence                | 按 flight 强制；陈旧 window 丢弃                                                               | 按页强制；陈旧页重试或上报                                                              |
| 界限             | 协商的 `rowWindowBytes`、relay 记录上限、协议硬界限                                           | 视口大小 window；overscan 缓存                                                                 | 按字节预算分页并自适应缩小                                                              |

TUI 特有的内容不会渗入 CLI：viewport phase、scroll offset、overscan 缓存、键鼠意图、
selection gesture 与剪贴板途径都是 Client-local。反之，CLI 的一次性快照也不驱动 TUI
视口。两个消费者都不新增独立的读协议；二者都扩展上文所述的同一 relay 词汇。

## 数据面

### 进入 scrollback 视口

进入是非模态的：第一次上翻意图把视口移入历史，而不劫持键盘输入。前置条件：
primary screen（`TerminalTextFrame.alternate_screen == false`）、`total_rows` 超过
可见行数、无 `disposed` 状态。

```text
User scroll-up intent
  → Client: SetHistoryDemand { demand: ViewingHistory }
  → Client: SetProjectionDemand { demand: Paused }
  → Host:   HistoryDemandAccepted          (echo)
  → Host:   ProjectionDemandAccepted       (echo)
  → Host:   HistoryChanged { anchor_status: None|Valid, generation, revision, sequence, total_rows }
  → Client: HistoryObserved { sequence, revision, generation }
  → Client: ReadRows { purpose: Viewport, detail: Render, revision_policy: CurrentView,
                       origin: Tail { rows_before: n }, row_count: visible + overscan,
                       generation, revision, request_sequence, maximum_bytes }
  → Host:   RowsSnapshot record            (pending anchor)
  → Client: install window as the rendered view
  → Client: RowsApplied { request_sequence, generation, revision }
  → Host:   RowsApplied echo               (pending anchor promoted to committed anchor)
```

进入时的 `anchor_status: None` 是预期而非错误：在第一次 `RowsApplied` 之前不存在
committed anchor。视口进入请求使用 `Tail { rows_before }`，因为 client 只知道想把
window 放在底部之上多远——还不知道绝对行号。

### viewing 期间滚动

- 行级与页级 delta 在本地解析为新的 `rows_before` 值，然后在 `Viewport` lane 上发出一次
  `ReadRows { origin: Committed { delta }, revision_policy: CurrentView }` flight。
  `CurrentView` 让 Host 在 mutex 下对着 committed anchor 解析，因此推进 `revision` 的
  PTY 输出不会使读取变陈旧。
- 跳转（`Home`、拖到某位置）解析目标 `rows_before` 并发出
  `ReadRows { origin: Tail { rows_before }, revision_policy: CurrentView }`。
- flight 未决期间到达的请求累积为一个 pending delta，在 lane 空出时派发——wire 允许
  每条 lane 至多一个在飞。
- 每个被接受的 `RowsSnapshot` 替换缓存 window，更新 `start_row`、`rows_before`、
  `revision`、`generation` 与 `total_rows`，并在 client 安装渲染后用 `RowsApplied`
  确认。
- viewing 期间每个 `HistoryChanged` 都以 `HistoryObserved` 确认；`Valid` anchor status
  触发 `Committed { delta: 0 }` 刷新 flight，其他 status 触发恢复（见下）。

### 选择与复制

```text
User drag / keyboard selection intent
  → Client: SelectionGesture { action: Start|Update|End|Cancel, generation,
                               viewport-relative point }
  → Host:   SelectionChanged event { state: Active { start, end, rectangle, preview } }
  → Client: (copy intent) SelectionExport { format: Plain, sequence }
  → Host:   SelectionExportStarted
  → Host:   SelectionExportChunk event { sequence, chunk_sequence, final, bytes }   (≤ 1 MiB each)
  → Client: SelectionExportChunkApplied { sequence, chunk_sequence }                (paces next chunk)
  → Host:   SelectionExportFinished event
  → Client: writes assembled plain text to the local clipboard transport
```

因为 Host 对着 committed anchor 解析 gesture 点，对历史行的选择不需要 client 侧行
换算。`SelectionCommand`（`Clear`、`SelectAll`、`SetRange`、`SelectOutput`）覆盖
程序化选择与取消。

### 返回 live 输出

以下任一——滚动到 `rows_before == 0`、`End`、terminal input、evicted anchor、detach
或 terminal 切换——都会退出视口：

```text
Client: SetHistoryDemand { demand: FollowingBottom }
Client: SetProjectionDemand { demand: Active }
  → Host:   both accepted echoes
  → Host:   next live snapshot frame (fresh base; deltas cannot follow a pause)
  → Client: discard history window and cache; render the live frame
```

进入期间 TUI 让最后的 live 帧停留在底层，并在重新激活后的第一帧恢复 live 渲染——
与 web client 使用的 `recovering → live on next snapshot` 规则相同。

## Client-local 视口状态

### 状态

| 状态         | 含义                                                                              |
| ------------ | --------------------------------------------------------------------------------- |
| `live`       | following bottom：渲染 live 帧 projection；未安装 history window                  |
| `entering`   | 首个 `Viewport` lane `ReadRows` flight 在飞；冻结的最后 live 帧停留在屏上         |
| `viewing`    | 已提交 history window 被渲染；至多一个 `Viewport` lane refresh/scroll flight 在飞 |
| `recovering` | 已请求回到底部；等待下一个 live 快照帧以丢弃 history 状态                         |
| `disposed`   | attachment、terminal 或 session 已消失；全部视口状态被丢弃                        |

### Offset 与锚定模型

- 主表示：`start_row`，第一个可见行的绝对已保留行索引，取自 applied `RowsSnapshot`
  头部。绝对行跨 revision 稳定；已保留第 0 行是最旧已保留行，0 与最旧已保留索引之间
  的差值标记被逐出的历史。
- 导航表示：`rows_before = total_rows − (start_row + visible_rows)`，即可见窗口之下
  的已保留行数。滚动意图修改 `rows_before`；`Tail` origin 直接消费它。
- 稳定性权威：Host 拥有的 committed tracked anchor。`RowsApplied` 钉住 applied
  window；`Committed { delta }` origin 与 selection gesture 在 terminal mutex 下据此
  解析。client 绝不从帧状态重新推导锚定。
- 每个视口会话携带的身份：`attachment_id`、`host_epoch`、`generation`、`revision`、
  `total_rows`、`start_row`、`rows_before`、`request_sequence` 计数器，以及最近观测的
  `HistoryChanged` sequence。

### 状态转移表

| 状态                   | 事件                                                                | 下一状态     | 动作                                                                                                      |
| ---------------------- | ------------------------------------------------------------------- | ------------ | --------------------------------------------------------------------------------------------------------- |
| `live`                 | 首个 live 帧 / 正常帧                                               | `live`       | 照旧应用 projection                                                                                       |
| `live`                 | 上滚意图；alternate screen、无 scrollback，或 `total_rows` ≤ 可见行 | `live`       | 显示本地化 "no scrollback" 状态提示                                                                       |
| `live`                 | 上滚意图；primary screen 且有 scrollback                            | `entering`   | 发送 `SetHistoryDemand(ViewingHistory)`、`SetProjectionDemand(Paused)` 与一次 `Tail` `ReadRows` flight    |
| `entering`             | `HistoryDemandAccepted` / `ProjectionDemandAccepted`                | `entering`   | 记录接受；仍在等 row flight                                                                               |
| `entering`             | `HistoryChanged`                                                    | `entering`   | `HistoryObserved`；首个 window 落地前 `anchor_status` 仅供参考                                            |
| `entering`             | 匹配 flight 的 `RowsSnapshot`                                       | `viewing`    | 安装 window、渲染它、发送 `RowsApplied`                                                                   |
| `entering`             | Typed failure、陈旧或畸形 window、发送失败                          | `recovering` | fail closed：恢复 `FollowingBottom` + `Active` 并给出状态提示                                             |
| `viewing`              | 行/页滚动意图                                                       | `viewing`    | 累积或派发 `Committed { delta }` `CurrentView` flight                                                     |
| `viewing`              | `Home` / 跳转意图                                                   | `viewing`    | 朝目标发 `Tail { rows_before }` `CurrentView` flight                                                      |
| `viewing`              | `End` 或滚到 `rows_before == 0`                                     | `recovering` | `SetHistoryDemand(FollowingBottom)` + `SetProjectionDemand(Active)`                                       |
| `viewing`              | 匹配 flight 的 `RowsSnapshot`                                       | `viewing`    | 替换 window；安装后 `RowsApplied`                                                                         |
| `viewing`              | 陈旧 `RowsSnapshot`（generation/revision/request sequence 不匹配）  | `viewing`    | 丢弃；若无 pending 则发 `Committed { delta: 0 }` 刷新                                                     |
| `viewing`              | `HistoryChanged` 且 `anchor_status: Valid`                          | `viewing`    | `HistoryObserved`；刷新 flight `Committed { delta: 0 }`                                                   |
| `viewing`              | `HistoryChanged` 且 `anchor_status: Evicted` 或 `None`              | `recovering` | `HistoryObserved`；anchor 身份已失，回到底部                                                              |
| `viewing`              | 暂停期间到达 live 帧记录                                            | `recovering` | 在 paused demand 下属意外；按失同步处理并恢复                                                             |
| `viewing`              | `SelectionChanged` / export 事件                                    | `viewing`    | 更新或退役 selection lane；与 viewport phase 正交                                                         |
| `viewing`              | terminal input 意图                                                 | `recovering` | 先吸回底部；input 经 controller 独立继续                                                                  |
| `viewing`              | 视口 resize                                                         | `viewing`    | 丢弃已渲染 window，显示 re-anchoring 占位；随后的 `HistoryChanged` 决定 refresh（`Valid`）或 `recovering` |
| `viewing`              | detach、attach 失败、transport 丢失                                 | `disposed`   | anchor 与 window 随 attachment 消亡                                                                       |
| `entering` / `viewing` | terminal retirement 或 session/attachment 切换                      | `disposed`   | 全部视口状态随旧 attachment 丢弃；新 attachment 从 `live` 开始                                            |
| `recovering`           | 下一个 live 快照帧                                                  | `live`       | 丢弃 history store 与计数器；恢复 projection                                                              |
| `recovering`           | 快照到达前的上滚意图                                                | `recovering` | 忽略（或至多缓冲一个意图）直到 `live`                                                                     |
| `recovering`           | `HistoryChanged`、detach 或 transport 丢失                          | `disposed`   | 能确认的仍确认；attachment 反正终结会话                                                                   |
| `disposed`             | 任意事件                                                            | `disposed`   | 终态；没有新的 `live` attach 不再进入                                                                     |

### viewing 期间的稳定性

- 暂停的 projection demand 意味着 viewing 期间没有 live 帧到达；历史内容由
  committed anchor 钉住，新输出落地时不会移动。
- `Valid` 的 `HistoryChanged` 携带新的 `revision`、`generation` 与 `total_rows`；
  client 更新元数据、确认，并通过 `Committed { delta: 0 }` 刷新内容——视口停留在
  相同的逻辑行上。
- 返回 `revision` 不早于（`>=`）最近事件的 `CurrentView` window 已经反映该变化，可取代
  pending 的 refresh 或 recover 转移。
- 新输出绝不自动滚动已打开的历史视口；只有无效 anchor 或用户自身意图才会移动它。

### 自动回到底部

- 到达 `rows_before == 0`（滚过最后已保留行）、`End` 或任何 terminal input 都转移到
  `recovering`，并恢复 `FollowingBottom` demand 加 `Active` projection。
- 在第一个 live 快照帧上，client 丢弃 history window、缓存、selection lane 与计数器，
  恢复正常 projection。
- 用户有意 viewing history 时，回到底部绝不跟随输出。

### 失效

- Retirement、detach、attachment 替换、terminal 或 session 切换：`disposed`；每个
  视口计数器都以 attachment 为键并随之消亡。
- Generation 变化（resize）：Host 放弃 generation-pinned 的 pending 捕获；committed
  anchor 仅在其 Ghostty reflow pin 存活时存续，由 `anchor_status` 报告。client 等待
  决定性的 `HistoryChanged`，而非抢先恢复。
- 逐出（`scrollback_bytes` 收缩或预算压力）、screen 替换或 pin 丢失：
  `anchor_status: Evicted` → `recovering`。`UpdateTerminalConfiguration` 响应另向
  收缩方携带 `history_rows_evicted`，只读 viewer 看不到它；`HistoryChanged` 是 viewer
  的逐出信号。
- `RowsSnapshot` 上陈旧的 generation/revision/request sequence：丢弃该记录、保留
  视口、经 `Committed { delta: 0 }` 刷新；绝不渲染身份未验证的 window。
- `entering`/`viewing` 期间 attach 失败或 transport 丢失：viewer attachment 已消失——
  视口 `disposed`；session 层决定重新 attach 或呈现失败。

## 交互（provisional）

以下所有键位均为 **provisional**：`m3-input-design` 拥有 TUI 键位词汇，并由产品决策人
签署。模型立场是非模态滚动——scrollback 从不捕获输入；terminal input 会把视口吸回
底部。

| 意图               | provisional 键位                                    | 说明                                                                  |
| ------------------ | --------------------------------------------------- | --------------------------------------------------------------------- |
| 进入 scrollback    | 鼠标滚轮上滚，或 `Ctrl+B PageUp` / `Ctrl+B Up`      | 非模态；terminal 应用不占用指针时无需模式键                           |
| 上/下一行          | 鼠标滚轮，或 `Ctrl+B Up` / `Ctrl+B Down`            | 映射为 ±1 的 `Committed { delta }` flight                             |
| 上/下一页          | `Ctrl+B PageUp` / `Ctrl+B PageDown`                 | 每页 `visible_rows − 1`，与 terminal 惯例一致                         |
| 跳到顶/底          | `Ctrl+B Home` / `Ctrl+B End`                        | 顶 = 最旧已保留行；End 返回 `live`                                    |
| 退出 scrollback    | `Ctrl+B End`、滚到底部，或键入 terminal input       | 吸回底部；非模态模型下不需要独立退出键                                |
| 选择开始/移动/结束 | 视口上鼠标拖拽                                      | `SelectionGesture` `Start`/`Update`/`End`；键盘驱动选择是后续模态切片 |
| 取消选择           | `Ctrl+B Escape` 或开始新选择                        | `SelectionGesture` `Cancel` 或 `SelectionCommand` `Clear`             |
| 复制选择           | `Ctrl+B y`                                          | `SelectionExport` 纯文本 → 本地剪贴板（途径保留见下）                 |
| 状态指示           | 面板边框徽标如 `History 1,240/5,000` 加逐出省略标记 | 呈现细节；保留                                                        |

本提案有意不设计的可选后续项：模态 copy-mode 光标、scrollback 搜索 UI、prompt
导航跳转与 scrollbar 交互。`purpose: Search` row lane 已为后续搜索切片备好。

## 选择与复制

存在两条剪贴板途径：

| 途径                                               | 方向                                             | 适配                                                                                   |
| -------------------------------------------------- | ------------------------------------------------ | -------------------------------------------------------------------------------------- |
| OSC 52（`TerminalClipboardWriteRequest`，kind 56） | terminal *应用*请求 Client 写剪贴板              | 方向不对：它只承载应用意图，不授予平台效果                                             |
| TUI 进程本地剪贴板写入                             | Client 组装 `SelectionExport` 字节并写系统剪贴板 | 方向正确；把复制语义（`SelectionChanged` → export → 剪贴板）留在 wire 指定的 Client 上 |

推荐路径：用户选择留在 wire 上（`SelectionGesture`/`SelectionCommand` →
`SelectionChanged`）；复制是一次 `SelectionExport`，按块排入有界本地缓冲（≤ 16 MiB），
由 TUI 写入系统剪贴板。剩余的保留选择是剪贴板 transport 本身——向宿主 terminal
发射 OSC 52（可跨 SSH、无依赖）还是原生剪贴板集成——由产品决策人裁决，不在此定。

复制绝不改变共享 Workbench 状态：它只触碰 attachment 级 selection 与一次
Client-local 剪贴板写入。

## 界限与失败行为

| 界限                                | 值                                      | 视口规则                                                              |
| ----------------------------------- | --------------------------------------- | --------------------------------------------------------------------- |
| `scrollback_bytes` 默认 / 零 / 上限 | 10,000,000 / 禁用 / 1,000,000,000       | 零意味着 `total_rows` 等于屏幕行数；进入时显示 "no scrollback"        |
| Row window                          | ≤ 44 MiB 且 ≤ 4096 行，112 字节 OBRW 头 | 视口请求远小于此；请求自身携带更紧的 `maximum_bytes`                  |
| 协商 `rowWindowBytes`               | Worker 已协商到协议上限                 | 仍把每次请求的 `maximum_bytes`（建议 ≤ 4 MiB）限在 relay 记录上限之下 |
| Relay 记录                          | 64 B + 36 MiB                           | 合法的 44 MiB window 无法穿越 relay；视口与页大小远低于它             |
| `Committed` delta                   | ±4096 行                                | 更大的累积 delta 拆成多次 flight 或经 `Tail` 解析                     |
| Frame cells / rows / columns        | 262,144 / 4096 / 4096                   | 视口缓存镜像 frame 有界几何                                           |
| Selection preview / export / chunk  | 4 KiB / 16 MiB / 1 MiB                  | 复制按块排干，溢出或 dispose 时取消                                   |
| 视口缓存                            | 建议 ≤ 8 MiB 解码 cell，16 行 overscan  | 超过缓存预算的 window 被拒绝并使视口恢复                              |
| History 事件确认                    | 至多一个在飞 `HistoryChanged`           | 总是 `HistoryObserved`；陈旧确认为 no-op                              |

失败矩阵——fail closed，尽可能保住 live 视图：

| 状况                                                                                       | 行为                                                                                                              |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| 无已保留 scrollback（`total_rows` ≤ 可见行、alternate screen，或 `scrollback_bytes` 为 0） | 保持 `live`；本地化状态提示；不发送 demand 变化                                                                   |
| viewing 期间最旧行被逐出                                                                   | `HistoryChanged` `Evicted` → `recovering`；仅在 window 仍可信时把历史顶标记（`start_row` > 最旧索引）渲染为省略行 |
| generation/revision/sequence 陈旧的 window                                                 | 不渲染直接丢弃；经 `Committed { delta: 0 }` 刷新；反复陈旧止于 `recovering`                                       |
| 畸形或不匹配 window                                                                        | 同样丢弃并恢复；身份无法验证的 window 绝不绘制                                                                    |
| 任一 history 控制的 `TypedFailure`                                                         | `recovering`；经既有 notice 路径呈现该 typed failure                                                              |
| Attach 失败或 transport 丢失                                                               | `disposed`；按 live projection 丢失的 session 重连策略处理                                                        |
| Row window 超出协商/relay 预算                                                             | 缩小 `maximum_bytes` 与 `row_count`；若单屏 window 仍失败则 `recovering`                                          |
| Terminal retirement 或 session 切换                                                        | `disposed`；替换 attachment 以空 history 状态从 `live` 开始                                                       |
| viewing 期间 resize                                                                        | 丢弃已渲染 window（其列几何已陈旧）；下一个 `HistoryChanged` 解析为刷新或 `recovering`                            |

## 共享文档一致性

已接受的 TUI Client contract 已把"active location、active Tab、focus、layout
projection、viewport、scroll、search 与 selection presentation"划归每个 Client 自己。
Scrollback 恰好落在该边界内：

- 权威内容在 Terminal Host 中：VT 状态、已保留行、逐出、anchor、revision/generation
  身份。
- scrollback 视口是通过 attachment `read` capability 对该内容的只读投影——它不能
  改变 terminal、Workbench 或其他 Client。
- `start_row`、`rows_before`、缓存、pending 意图、gesture 与剪贴板状态都是
  Client-local 呈现状态；两个 Client 合法地查看同一 terminal 的不同行。
- 共享 Workbench 文档绝不应携带视口位置、scroll offset、selection、本地复制状态、
  搜索游标或 scrollbar 呈现——那会把一个 Client 的瞬时视图泄漏给所有其他 Client。

## 实现面

供评审者规划后续切片的非 wire 工作项：

1. Relay 词汇：[Relay 面与所需增补](#relay-面与所需增补) 列出的 OBCS/OBCN
   `TerminalViewerAction` 与 Worker 路由增补，与 `m3-terminal-read` 共享。
2. 有界 Rust OBRW `Render` detail 解码器，产出渲染器既有的 cell 行。
3. `TerminalProjection` 旁的 TUI-local 视口模块：状态机、window 缓存、flight 簿记，
   以及 provisional 意图的 input/dispatch 面。
4. 导出 selection 字节的本地剪贴板 transport。

## 保留决策与缺口

留给各自 owner 裁决、此处不定：

- 到系统剪贴板的 transport（OSC 52 发射 vs 原生集成）。
- scrollback 搜索是否并入本切片；建议：延后——wire 已有 `purpose: Search`，但 UI
  需要 `m3-input-design` 先定词汇。
- 呈现细节：状态徽标配文、history/live 分隔、逐出行标记、scrollbar。
- OBCS/OBCN 动作变体的最终形状及任何版本化成本，归 relay-protocol 维护者。
- 未发现 OBTP wire 缺口：该视口需要的每个语义——history demand、事件、row window、
  anchor、selection 与 export——都已在 wire 上。

## 验证

```sh
pnpm docs:confirm-pairing -- proposals/cli/tui-scrollback-viewport.md
```
