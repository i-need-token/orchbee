# 原生鼠标输入

## 范围与归属

本参考治理原生 Client 的指针行为：宿主捕获生命周期、最后一个已绘制帧的 hit map、各绘制
区域接受的手势词表、向 pane 内程序的透传编码，以及备用屏幕的滚轮回退。它与
`tui_pointer.rs`、`terminal_input/mouse.rs`、`terminal_input/decoder.rs`、
`terminal_input/dispatcher.rs`、`terminal_lifecycle.rs`、`tui_render.rs` 及其
`tabs`/`contexts` 绘制器、`tui_shell.rs`、`client_view.rs` 与 `client_view/bindings.rs`、
`terminal_session/ui.rs`、`invocation.rs`、`localization.rs` 共同维护。仓库标准
[`docs/tui/mouse-input.md`](../../../docs/zh-CN/tui/mouse-input.md) 拥有该映射必须满足的
规则；[原生终端环境](native-terminal-environment.md) 拥有 arming 与恢复所依托的
mode-lease 机制。[原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)拥有未跟踪
pane 的选择及其复制生命周期。

Windows [分隔线调整](native-divider-resize.zh-CN.md) 拥有可撤销 split 预览及一次仅松手提交的共享比例修改。

滚动指示器跳转、右键上下文菜单、中键表面、pixel 格式报告、选择复制
键位和三击选行仍属于独立决策。

## 捕获策略

- 启动通过一对 `ModeLease` arm button-event tracking（`?1002h`）与 SGR 编码（`?1006h`）；
  `orchbee tui --mouse off` 跳过 arming。Any-motion tracking（`1003`）永不开启。与其他
  已租用模式一样，该对在每条退出路径上恢复——正常返回、失败、owner 线程 panic 以及每个
  终止信号。
- `Ctrl+B m` 在运行时释放并重新 arm 捕获。释放只在模式写成功后生效，因此持久状态指示
  （`mouse released · Ctrl+B m restores` / `鼠标已释放 · Ctrl+B m 恢复`）与宿主实际上报
  状态绝不不一致。released 期间，每个已解码指针事件在探测或命中解析之前即被忽略，所有
  在途手势与滚轮流取消；重新 arm 不复活它们。
- `--mouse` 在 `tui` 准入（`off` 与 `on` 两个值）。Windows `terminal attach` 和
  `agent attach` 也按独立的[流契约](windows-terminal-streams.zh-CN.md)受理该选项。
  裸 `orchbee` 拒绝该选项且仍默认捕获开启；其他路由拒绝；`tui` 继续拒绝 `--format`。两种声明写法均可——
  `--mouse off|on` 与 `--mouse=off|on`——封闭词表以外的值、以 dash 开头的值、重复
  出现一律拒绝。
- `Shift` 修饰的报告从不被认领：宿主保留 `Shift` 用于原生选择。pane 程序透传期间收到的
  `Shift` 报告被丢弃，而该手势继续存活，使随后不带修饰的 release 仍到达 pane；Client
  选择期间收到该报告则取消 Client 手势且不复制。

## 区域映射

渲染器把绘制的每个交互区域记入暂存的 `HitMapBuilder`；shell 只在该帧绘制完成后发布
map。区域携带稳定身份——`WorkbenchTabId`、`NativeContextLocation`、`WorkbenchPanelId`——
点击时按新探测解析为位置，因此绘制与点击之间的重排不会误路由。层从前到后解析，同层内
区域绝不重叠：

| 绘制表面                    | 层               | 目标                | 接受的手势                                                                                  |
| --------------------------- | ---------------- | ------------------- | ------------------------------------------------------------------------------------------- |
| Navigator context 行        | `Overlay`        | `Context(location)` | Press 选择该 context（`SelectContextAt`）                                                   |
| Navigator tab 行            | `Overlay`        | `Tab(tab_id)`       | Press 选择该 Tab（`SelectTab`）                                                             |
| 控件之外的 overlay 本体     | `OverlaySurface` | `OverlaySurface`    | 吞掉——绝不到达下层                                                                          |
| 捕获型 overlay 之外         | —                | backdrop            | Press 关闭该 overlay；其下任何区域——包括未被遮盖的 tab 条或 pane 等前景构件——都看不到该事件 |
| 紧凑 tab-strip 标记         | `Foreground`     | `Tab(tab_id)`       | Press 选择该 Tab（`SelectTab`）                                                             |
| Pane 叶视口（content 矩形） | `Foreground`     | `Pane{tab,panel}`   | Press 先聚焦，再由该 pane 的上报契约决定程序透传或 Client 选择                              |
| 视口之外的 pane 叶标题行    | `Foreground`     | `Pane{tab,panel}`   | Press 只聚焦；pane 内程序看不到任何内容                                                     |
| Windows 可见 pane 分隔线    | `Foreground`     | `Divider(source)`   | 无修饰左键按下开始预览，松手一次应用，Esc 或 Shift 取消                                     |
| 其余一切（状态、空隙）      | —                | dead space          | 忽略                                                                                        |

pane 区域上的 press 按*被绘制的那个 Tab* 解析：当绘制时的 Tab 不再处于 active 的
stale-map 边界退化为普通 Tab 选择，而不是聚焦一个不可达的 panel。

## 手势生命周期

| 状态        | 含义与出口                                                                                                                                                                                 |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Idle`      | 无指针 owner。命中区域上的 press 打开一个手势或直接应用导航。                                                                                                                              |
| `Divider`   | Windows：一条精确已绘制 split 拥有可撤销预览线。松手交接一次有变化比例，取消则丢弃。提交由共用局部调整所有者拥有，不属于 pointer 手势。                                                    |
| `Pane`      | 已聚焦的上报 pane 拥有指针直至 release。motion 与 release 按 press 时绘制的视口原点编码为 pane-local——拖出区域的 drag 仍属于它。release 在手势中途绝不丢弃。                               |
| `Selection` | 已聚焦且未跟踪鼠标的 pane 为其精确 attachment 和投影 generation 拥有一个 Host 选择手势。motion 在 pane 内合并；release 留在最新 motion 之后，并且只有 Host 接受的活动 end 才能请求复制。   |
| `Pending`   | 落在未聚焦 pane 上的 press 触发了 `FocusPanel`；该 press 与随后的输入排队（FIFO，上界 16，motion 合并为最新 cell），直至切换就绪。溢出在任何新输入被接受之前取消并报告有界的输入丢弃说明。 |

就绪判定对新鲜探测做出，绝不比较缓存序号或代次差值。排队 press 等待其精确记录的 pane
实例仍有 attachment、成为 desired focus、mode 已明确，并具有可用的当前帧。读取就绪允许在
不跟踪鼠标的 `ReadOnly` pane 上进行 Client 选择。应用输入独立要求 `input_ready`、精确 leaf
的 `Controlling` mode 和实时视口；历史中的跟踪 pane 在本地消费队列。实例消失或被替换，或
焦点被改到别处时，取消而不重放。原始两秒期限到达后取消，并报告输入丢弃说明。Start 尚未
准入时，已松开的点击可以被同一已绘制 pane 上的新 press 替换，但未松开的重复 press 不能
替换它、重置期限或绕过队列上限。同代本地 focus 复用已安装 attachment 时以同样方式准入。

Modal 输入捕获（draft 或已 arm 的确认）吞掉指针事件而不产生手势；modal 自身的外点规则不
变——普通 overlay 在外点 press 时关闭，modal 表面则吞掉它。

## Pane 透传

- 当当前帧报告任一 tracking 模式时，pane 程序拥有其 content 视口内的指针事件，外加下文
  的备用屏幕滚轮回退。帧事实——tracking、format、cell 几何、备用屏幕、DEC 1007、application-cursor
  模式——来自最新已验证帧的可信 `TerminalInteraction`，每个事件重新探测。
- 未跟踪鼠标的 pane 绝不接收选择字节。其普通主键手势遵循
  [原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)；`Shift` 继续绕过 Client
  归属。
- 被转发的事件按绘制的 content 原点平移为 pane-local 坐标，并由唯一 owner 按该 pane
  请求的 tracking × format 重新编码：X10 只报 press；Normal 报除 motion 外的一切；
  Button 在有按住时上报（press 自身计为已按住）；Any 全报；release 始终上报。同 cell
  motion 去重；视口外的非 release 事件仅在 motion tracking 且有按住时上报并夹到边缘
  cell；legacy `Default` 格式拒绝超过 222 的坐标而不回绕；`Utf8` 把坐标编码为
  codepoint；SGR 使用 `M`/`m` 结尾；URXVT 使用其十进制形式。
- 仅有 cell 坐标的输入，即使 pane 请求 `SgrPixels`（`?1016`），也按 SGR cell 编码，
  与 Herdr 的 cell 降级一致。tracking、修饰键、release 按钮身份、夹取和 motion 去重
  保持普通 SGR 规则。这不提供精确像素输入，也不推测 cell 中心；宿主捕获仍以 cell
  为单位，绝不开启 `?1016`。
- 字节投递复用既有 `forward` 路径，包括其 `Controlling` 检查、只读拒绝与确认语义。
  有 attachment 的 pane 可以获得本地读取焦点，不依赖输入权限；title 行 press 只聚焦而不
  发送 pane 输入。mode 仍未知的 content press 在原始 pending 期限内等待。跟踪 pane 一旦
  明确为 `ReadOnly`，其属于程序的 press 就会被丢弃并报告输入说明，不隐式 acquire。读取焦点
  绝不代替应用输入权限，历史也绝不把属于程序的跟踪手势改为 Client 选择。

## 滚轮回退

在程序不上报 tracking 且开启 DEC 1007 的备用屏幕 pane 上，滚轮 notch 编码为方向键按下——每个 notch 三
次，使用该 pane 的 application-cursor 形式（`ESC O A` 对 `ESC [ A`）。per-pane 流为突发
设界：方向反转清掉排队余量并丢弃该 notch，每次 flush 至多发出一屏方向键，发现队列为空
的 tick 关闭该流，使下一个 notch 重新开始。每次 flush 都按最新帧复核转发条件；关闭
DEC 1007、离开备用屏幕、开启 tracking 或进入历史会丢弃排队的方向键。DEC 1007 关闭时，
备用屏幕滚轮事件不发送应用输入。在未启用 tracking 的 primary 屏幕内容区，向上滚动一个
notch 从实时尾部向前回看三行；后续每个 notch 本地移动三行。实时尾部向下滚动、历史为空
或帧不可用时均无效果、不提示。滚轮绝不改变焦点或从标题/边框滚动内容；包括历史中，
Shift 始终属于宿主。进入、合并导航与恢复沿用[保留视口](native-terminal-viewport.zh-CN.md)所有者。

## 不变式与时序规则

- `LastDrawnOnly`：事件只按最近一次完整成功 draw 的 map 解析；布局变化与下一次发布之间
  的每个指针事件被丢弃而绝不误路由。
- `PublishAfterDraw`：暂存 map 仅当其所属帧的绘制完成才成为权威——失败的 draw 不发布
  任何内容。
- `StableIdentity`：区域携带 `WorkbenchTabId`/`NativeContextLocation`/`WorkbenchPanelId`
  并在点击时重新解析；重排之后不残留缓存下标。
- `OneGestureOwner`：press、drag、release 按 press 命中的区域解析；捕获释放、modal 捕获
  或外部布局变化整体取消手势。
- `ReleaseNeverCrossesPress`：pending-focus 输入按 FIFO 放行——release 绝不先于其 press
  被投递，取消丢弃整个队列而不重排。
- `FocusIsClientLocal`：press 引起的焦点变化先于任何到达 pane 的字节生效。
- `NeverFabricate`：cell 事件经 SGR 降级后仍使用 cell 地址。不从 cell 或未知宿主
  像素几何推算精确像素位置。
- `PendingKeysFirst`：dispatcher 先 flush 已接受的 pending 键字节再处理指针事件，鼠标
  报告绝不插到更早的键盘输入之前；畸形或中止的鼠标形态序列（`DecodedKind::Ignored`）被
  原子消费——绝不退化为按键，其原始字节也绝不转发给 pane。无关的未知 CSI 保留兼容透传
  （`Nondecision`）。
- `CaptureHonesty`：released 指示只在模式写成功后出现，`Ctrl+B m` 路径与启动及恢复路径
  共用同一 lease。

## 源码对应

| 生产表面                                    | 模型对应                                                                                                                                                                        |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tui_pointer.rs`                            | `PointerState` 捕获/map/手势生命周期、`FrameHitMap`/`HitMapBuilder` 暂存区域、`Gesture`（`Idle`/`Pending`/`Pane`/`Selection`/`Divider`）、`WheelStream`                         |
| `terminal_input/mouse.rs`                   | `MouseEvent` 解码（`decode_sgr`、`decode_x10`）、`pane_owns_pointer`、`encode_pane_report`、`encode_cursor_arrows`                                                              |
| `terminal_input/decoder.rs`                 | 类型化 `DecodedKind::Mouse`/`Ignored` item；X10 三字节 body 原子消费、超时与中止；鼠标形态的畸形/中止/越界运行解码为 `Ignored`                                                  |
| `terminal_input/dispatcher.rs`              | 位于 raw 透传通配之前的显式 `Mouse` 与 `Ignored` 臂——`Ignored` 被消费并丢弃、绝不入 pending；先 flush pending 字节；`RegionsChanged` 置 map 为 stale；外部变化 `invalidate_map` |
| `terminal_lifecycle.rs`                     | `set_pointer_capture` ModeLease 对（`?1002`/`?1006`），enter 时 arm、每条退出路径恢复                                                                                           |
| `tui_shell.rs`                              | 启动 arming、每次 draw 暂存 `HitMapBuilder`、成功才发布、`set_pointer_capture` 回调接线                                                                                         |
| `tui_render.rs`、`tabs`、`contexts`         | 按绘制矩形记录 tab 标记、navigator 行、overlay 本体与 pane 叶的区域                                                                                                             |
| `client_view.rs`、`client_view/bindings.rs` | `Ctrl+B m` → `TogglePointerCapture`，`SelectContextAt`/`FocusPanel`/`SelectTab` disposition，released 指示与指针 notice                                                         |
| `terminal_session/ui.rs`、`state/`          | `pointer_probe` 新鲜事实、`focus_panel`/`select_context_at` 准入、稳定 id 访问器（`tab_id`、`panel_id`、`location`）                                                            |
| `terminal_session.rs`（test 接缝）          | 由真实 projection 帧与 controller 状态支撑的单 pane `Legacy` probe——connected-PTY 见证后端                                                                                      |
| `terminal_input.rs`                         | `PointerProbe`/`PointerTab`/`PointerFrameFacts` 端口表面与 `forward_or_report` 投递                                                                                             |
| `invocation.rs`、`cli/`                     | `tui --mouse off\|on` 准入、help 文本与三种 shell 补全                                                                                                                          |
| `crates/terminal-protocol`                  | `TerminalInteraction` 可信 Rust API，暴露已验证帧的 tracking/format/几何/cursor 模式事实                                                                                        |

## 验证

以下 Unix PTY 测试维护共享契约，不构成 Windows 运行证据。Windows cell 降级由编码器、
指针及 attach 回归，以及[终端流](windows-terminal-streams.zh-CN.md)记录的打包
control/attach 字节实测覆盖。

| 表面                    | 证据                                                                                                                                                                                                                                                                                            |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 解码                    | `terminal_input/mouse/tests` SGR/X10 表——press/release/motion/wheel/修饰、跨 chunk 原子性、畸形拒绝且不退化为按键；`terminal_input/decoder/tests`——合法 SGR/X10 item、X10 分片喂入、超时 drain、ESC/C1 中止、畸形 body、越界运行、与按键/粘贴交错——所有鼠标形态畸形字节均为 `Ignored`           |
| 编码契约                | `encode_pane_report` 覆盖每个 tracking × format 单元的表行、release 结尾、去重、夹取、legacy 上界与 SGR cell 降级                                                                                                                                                                               |
| Hit map                 | `tui_pointer/tests` 层序、边缘/外侧 cell、overlay 捕获与 backdrop 关闭——包括落在未遮盖 tab 条与 pane 上的外点 press 既不选择也不转发、stale-map 丢弃                                                                                                                                            |
| 手势与 pending focus    | `tui_pointer/tests` press/release 顺序、队列上界与溢出取消、超时、orphan、只读、目标移动拒绝、settled 无帧等待与按事实准入                                                                                                                                                                      |
| dispatcher 端口写       | `terminal_input/dispatcher/tests`——畸形 SGR/X10 与中止/超时鼠标运行产生零转发字节，而普通按键、粘贴与无关未知 CSI 按序透传                                                                                                                                                                      |
| 滚轮流                  | `tui_pointer/tests` 每 notch 三次、每次一屏上限、反转清零、排空后关闭                                                                                                                                                                                                                           |
| 捕获生命周期            | `terminal_lifecycle/tests` 租约配对与恢复；dispatcher `Ctrl+B m` 切换测试；状态指示                                                                                                                                                                                                             |
| CLI                     | `invocation/tests` `tui --mouse` 值准入、裸调用/其他族拒绝、`--format` 拒绝；`cli/completion` 脚本                                                                                                                                                                                              |
| 真实 PTY（Legacy 接缝） | `tui_shell/connected_tests::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes`——记录的 pane-local SGR 字节、pane-local 平移、只读丢弃、SGR cell 降级、滚轮转方向键、捕获切换、模式恢复                                                                                   |
| 真实 PTY（公开 Native） | `tests/local_startup_process/native_pointer.rs`——真实 PTY 上 spawn 的 `orchbee` 二进制：捕获 arm、X10 与 SGR pane-local 报告、未聚焦 pane 点击聚焦与排队放行、只读拒绝、滚轮转方向键、`Ctrl+B m`、`--mouse off`、退出恢复，以及零像素宿主几何下 cell 寻址报告仍可用、`?1016` 使用 SGR cell 降级 |

## 共享右键策略

[Panel 输入策略](native-panel-input.zh-CN.md) 在焦点或透传之前裁决新的右键按下。默认 `orchbee` 将其保留；`panel` 仅允许启用鼠标上报的终端内容内无修饰键按下，并遵循现有控制权与就绪检查。排队右键在任何字节发出前复核策略。已准入手势捕获按钮与精确 Pane 实例，其他按钮不能注入按下或释放它，旧格式通用释放映射到归属按钮。改变策略作用于下一个手势，捕获/焦点/实例取消规则继续生效。本功能不新增上下文菜单。

## Presentation 选择器

会话就绪或已选择但未附着时，包含多个 Presentation 的 Panel 在现有标题栏展示 `[<] [position/count] [>]`：单面板／放大使用外框标题，分屏使用叶节点标题。控件为标题／状态保留十二个单元；窄标题栏和 Minimal 层级保留既有标签／数量及键盘路径。每个箭头有三个单元的命中区域。数量和空格不响应；分屏 Pane 命中区域排除整个选择器条，因此控件不与终端内容或聚焦区域重叠。不增加标题行或主机鼠标模式。

仅无修饰键的左键按下选择绘制时的精确目标 Presentation。按钮按规范成员顺序循环，与 `Ctrl+B (`／`Ctrl+B )` 等价。已绑定目标包含当前 `PaneRef` 和目的 Presentation ID。空目标包含结构 Pane key 与精确选中的 Presentation ID；新鲜探针必须仍确认该成员未绑定。空 Panel 内容区的按下只聚焦，不开始终端手势。新鲜探针必须仍在当前 desired anchor 下保留同一实例或空选择；UI 随后在活动 Tab 内解析 Panel／成员，才提交既有 Switch 事务。已删除／替换成员、旧 anchor、模态输入、捕获 overlay 和已释放鼠标捕获均不能改投手势。终端只读模式不禁止 Client 本地导航；选择从不转发终端字节，也不绕过子项仅 binding 附着。成功点击后旧命中图失效，直到重绘。

证据覆盖按钮边界、不响应的数量／空格、不重叠的分屏矩形、单面板／分屏／放大渲染、窄窗口及 Minimal 降级、修饰键／非左键报告、捕获／overlay／模态守卫、旧实例／anchor、成员移除及实际 UI 准入精确子项 Switch。真实主机视觉／多 Client 验收仍随最终 Windows 全量门禁统一执行。
