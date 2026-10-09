# 原生鼠标输入

## 范围与归属

本参考治理原生 Client 的指针行为：宿主捕获生命周期、最后一个已绘制帧的 hit map、各绘制
区域接受的手势词表、向 pane 内程序的透传编码，以及备用屏幕的滚轮回退。它与
`tui_pointer.rs`、`terminal_input/mouse.rs`、`terminal_input/decoder.rs`、
`terminal_input/dispatcher.rs`、`terminal_lifecycle.rs`、`tui_render.rs` 及其
`tabs`/`contexts` 绘制器、`tui_shell.rs`、`client_view.rs` 与 `client_view/bindings.rs`、
`terminal_session/ui.rs`、`invocation.rs`、`terminal_link_activation.rs`、`localization.rs`
共同维护。仓库标准
[`docs/tui/mouse-input.md`](../../../docs/zh-CN/tui/mouse-input.md) 拥有该映射必须满足的
规则；[原生终端环境](native-terminal-environment.md) 拥有 arming 与恢复所依托的
mode-lease 机制。[原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)拥有未跟踪
pane 的选择及其复制生命周期。

Windows [分隔线调整](native-divider-resize.zh-CN.md) 拥有可撤销 split 预览及一次仅松手提交的共享比例修改。

Windows [动作菜单](native-action-menus.zh-CN.md) 在普通聚焦之前消费属于 Client 的右键按下。
菜单捕获不能替换已有真实手势或 wheel 流。

滚动指示器跳转、中键表面、pixel 格式报告、选择复制
键位、三击选行和 Ctrl 悬停链接装饰仍属于独立决策。

## 侧栏导航输入边界

Windows 上，`Ctrl+B Shift+B` 显式进入侧栏键盘导航，并在有效几何允许呈现时
显示侧栏。`Ctrl+B b` 保持切换显隐的含义。`Picker` 拥有可见树、展开状态、
稳定行焦点及滚动窗口；活动终端上下文保持独立。`InputDispatcher` 拥有输入
租约及包边界保护。不增加服务端、线协议或持久状态。

shell 的视口同步在派发已读入的包之前及改变布局的动作之后结算有效侧栏可见性。
视口采样缺失时，结算使用上次尺寸。隐藏几何先退出 Picker 导航，然后由分发器协调
捕获待处理片段。真正关闭紧凑侧栏时，所有 backdrop Press 变体也在同一本地转换中
退出导航。帮助保留底层侧栏投影，关闭时返回该表面。

派发在返回绘制之前协调最后一条指针报告，即使后面没有字节。单独的 backdrop
滚轮 Press 因此在 PublishFrame 之前退休，下一新包无需额外重绘即可放行。
轨迹同时覆盖此包边界及带尾部的情况。

shell 回归轨迹通过真实同步边界测试 resize 与 backdrop Press 紧随 Enter 或跨读取
CSI，断言成功新帧放行后续输入之前没有 Selection、没有终端字节。这才是模型
RetireSidebar 事件的生产细化；测试预先手动让 Picker 观察到隐藏不能证明该顺序。

| 事件／输入                 | 守卫与可观察结果                                                                                                                                            |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 进入导航                   | 清除竞争的本地模式，显示侧栏并请求 Picker 进入；捕获开启包的尾部。                                                                                          |
| 导航／展开                 | Picker 的类型化按键决策只改变本地焦点、展开及窗口；不产生 Selection 请求或终端输入。                                                                        |
| F1 帮助                    | 用既有快捷键帮助覆盖树而不退出导航，包括紧凑浮层；F1、Escape 或外部按下关闭帮助并回到侧栏，PageUp/PageDown 翻阅注册表派生的条目，被覆盖的树不响应导航按键。 |
| 显式激活 Context           | 重新验证聚焦 Context，调用既有侧栏 Selection 路径，退出导航并捕获决策包尾部。                                                                               |
| 打开对象菜单               | 精确且仍存活的行必须存在于最后成功绘制的命中图，且属于当前机器和 Site；以该图的 Site/revision 证明包装菜单目标。开启不改变 Picker 焦点或终端上下文。        |
| 取消菜单                   | 既有菜单捕获保护决策包及已轮询输入直到新帧；Picker 调和保留的焦点身份。                                                                                     |
| Escape／侧栏隐藏／竞争捕获 | 无激活地退出导航；捕获待完成片段和退出包直到新命中图发布。                                                                                                  |
| 导航内未绑定按键或粘贴     | 无动作地消费；显式输入拥有者之外保持原有终端字节路径。                                                                                                      |

Project 行及展开控件按下请求分发器先取得粘贴租约，再聚焦并展开精确 Project；租约
写入失败时 Picker 保持不变，绝不激活终端。行菜单按钮、
右键及键盘菜单共用身份验证；打开菜单不会先聚焦或激活所属行。展开、行及菜单
命中矩形互不重叠。其他机器不能通过静默切换当前端点打开菜单。菜单命令可用性
及完整来源证明复验仍归既有原生菜单拥有者。

确定性证据归既有指针与派发器测试：同名异身份、删除或替换行、其他机器或 Site、
未绘制目标、键盘与指针菜单准入、捕获优先级、模式退出及拆分决策包。轨迹断言
Picker 请求、菜单目标、终端字节及返回后的输入归属。宿主图片只证明呈现；
受控输入轨迹不声称覆盖物理鼠标。

`formal/tla/SidebarInputOwnership.tla` 注册为 `sidebar-input-ownership`，检查按键或
粘贴片段、所有者退出与成功帧以不同顺序到达时的输入边界。`owner` 抽象分发优先级；
`menuReturn` 表示菜单下保留的 Picker 导航，不是第二份焦点存储。`packetOwned` 与
`paintPending` 对应分发器的包级和退出保护。`fragment` 对应解码器来源标记；
`bornOwned` 与 `leaked` 只是规范观察量，不是运行时状态。

| 抽象事件                                             | 生产细化与确定性证据                                                                                                                        |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| EnterSidebar / SidebarKey                            | 显式前缀动作与 Picker 按键分发；`sidebar_mode_owns_only_explicit_navigation_and_fences_exit_packets` 轨迹对照捕获输入和保持不变的终端输入。 |
| OpenMenu / CloseMenu                                 | 共享已绘制目标解析器与既有菜单捕获／结算；键盘和行菜单轨迹保留 Picker 焦点并归还粘贴租约。                                                  |
| RetireSidebar / CoveringCapture / EndCoveringCapture | Picker 退出导航，分发器协调竞争所有者并把待处理解码片段标记为捕获；退出轨迹包含跨读取按键、外部隐藏、重绘与之后的终端字节。                 |
| PublishFrame / PacketBoundary                        | 成功发布命中图才清除退出捕获；同步只在该发布之后清除包级捕获。                                                                              |
| BeginFragment / FinishFragment                       | 既有按键／粘贴解码器跨读取保留包来源；捕获期间开始的片段不能在退出之后成为终端输入。                                                        |

模型限定一个符号片段和四种所有者，允许重复进入、菜单关闭、退出及不同顺序的绘制。
它检查输入不泄漏，不证明树排序、规范成员关系、菜单命令效果或物理宿主行为；这些仍由
Picker 测试、NativeActionMenus 与受控分发器／宿主证据负责。不假设公平性或无条件进展：
宿主必须提供下一次读取与成功帧，退出后的输入才会释放。紧凑 F1／帮助返回轨迹另行检查
真实侧栏表面投影。

## 侧栏宽度输入

Picker 拥有侧栏偏好宽度；`SidebarSurface` 把同一个偏好传给布局、终端几何及绘制。
几何与边界仍由[壳层布局](tui-shell-layout.zh-CN.md)治理。`DrawnSidebarWidth` 记录
视口、包含偏好的完整表面及派生侧栏。只有成功帧中露出的右边框可以准入调整；
键盘导航使用同一绘制来源，不要求鼠标捕获。

`PointerState` 拥有一个可撤销的宽度拖动。移动只改变预览；普通左键松手重新验证
完整绘制来源，先移除手势，再比较原偏好并提交一次。Escape、Shift、缺失命中图、
宿主缩放、侧栏隐藏或竞争捕获均取消，不改变偏好。预览共用分隔线合成器的图像
遮盖与干净重绘路径。无操作保留偏好，即使视口限制了有效宽度。焦点、展开及终端
选择保持不变。

仅当 Full 或 Compact 布局可显示当前隐藏的侧栏，且没有覆盖输入所有者时，状态栏
绘制恢复控件；它调用既有显式侧栏聚焦动作。Minimal 与 TooSmall 不绘制不可用的
恢复控件。

既有指针／分发器轨迹覆盖精确绘制来源准入、绝对预览与松手、取消、过期偏好、
鼠标捕获已释放时的键盘调整、包片段及新帧放行。壳层合成轨迹覆盖预览图像与终端
几何一致性；几何向量及双语言宽度绘制仍归布局拥有者。

`SidebarInputOwnership.ResizeSidebar` 对应共用宽度决策保护：保留当前键盘所有者，
捕获决策包与待处理片段，成功绘制后才放行后续读取。既有 `DividerDrag` 模型同时
提供宽度手势生命周期：其来源 token 抽象视口、表面及偏好；本地松手／CAS 细化为
Release 紧接 SettleKnown，无操作则对应 ReleaseNoChange。不增加异步待确认所有者。
来源丢失、捕获、Escape、Shift 及重复松手使用既有取消事件。这是该模型的同步
子集；几何、精确目标相等与本地 CAS 由宽度核心及指针轨迹检查，不由符号 token 证明。

真实 shell 布局回调返回同步是否观察到变化。分发器在处理 Press 或 Motion 后立即
根据该结果废弃旧命中图及手势，包括同一次读取内不改变尺寸层级的宽度变化和仅高度
变化。缓存的视口分类不能代替该观测。真实 shell 轨迹在同包松手之前经过此边界，
之后只有新绘制及新按下才可准入另一次拖动。

宽度预览结束也退休其命中图，即使 Escape、Shift 或无变化松手保留偏好：其状态提示
遮住了规范 Menu 控件。既有重绘保护捕获结束决策及片段，直到规范帧成功绘制；同包
按下不能激活这些不可见控件。预览绘制轨迹检查重绘前拒绝 Menu 按下、重绘后准入，
同时保持 Picker 焦点不变。

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

| 绘制表面                    | 层               | 目标                           | 接受的手势                                                                                  |
| --------------------------- | ---------------- | ------------------------------ | ------------------------------------------------------------------------------------------- |
| Navigator context 行        | `Overlay`        | `Context(location)`            | Press 选择该 context（`SelectContextAt`）                                                   |
| Navigator tab 行            | `Overlay`        | `Tab(tab_id)`                  | Press 选择该 Tab（`SelectTab`）                                                             |
| 控件之外的 overlay 本体     | `OverlaySurface` | `OverlaySurface`               | 吞掉——绝不到达下层                                                                          |
| 捕获型 overlay 之外         | —                | backdrop                       | Press 关闭该 overlay；其下任何区域——包括未被遮盖的 tab 条或 pane 等前景构件——都看不到该事件 |
| 紧凑 tab-strip 标记         | `Foreground`     | `Tab(tab_id)`                  | Press 选择该 Tab（`SelectTab`）                                                             |
| Pane 叶视口（content 矩形） | `Foreground`     | `Pane{tab,panel}` + 已绘制来源 | 精确 `Ctrl`+主键点击打开已绘制链接；其他 Press 先聚焦，再使用程序透传或 Client 选择         |
| 视口之外的 pane 叶标题行    | `Foreground`     | `Pane{tab,panel}`              | Press 只聚焦；pane 内程序看不到任何内容                                                     |
| Windows 可见 pane 分隔线    | `Foreground`     | `Divider(source)`              | 无修饰左键按下开始预览，松手一次应用，Esc 或 Shift 取消                                     |
| 其余一切（状态、空隙）      | —                | dead space                     | 忽略                                                                                        |

pane 区域上的 press 按*被绘制的那个 Tab* 解析：当绘制时的 Tab 不再处于 active 的
stale-map 边界退化为普通 Tab 选择，而不是聚焦一个不可达的 panel。

Windows 导航器将上面两个旧导航目标替换为每个完整绘制行的
`NavigatorRow(episode, source, target)` 凭据。只有无修饰主键点击该精确行才可请求激活，
键盘 Enter 还要求已绘焦点。[导航器契约](native-context-selection.zh-CN.md#windows-捕获输入的导航器)
拥有准入与取消。背景按下仅关闭而不穿透，滚轮及现有 Shift 宿主绕过除外；滚轮以及行上的
修饰键／右键按下被消费且不激活。已消费的行／背景决策在成功新帧后仍拥有对应释放事件，
仅释放、宿主焦点丢失或新的非 Shift 合格按下可清除此归属，单纯绘制退休不可清除。
进入时通过清理所有者取消现有手势，不复制／导出选择，也不刷新排队的 pane 鼠标输入。

## 手势生命周期

| 状态        | 含义与出口                                                                                                                                                                                 |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Idle`      | 无指针 owner。命中区域上的 press 打开一个手势或直接应用导航。                                                                                                                              |
| `Divider`   | Windows：一条精确已绘制 split 拥有可撤销预览线。松手交接一次有变化比例，取消则丢弃。提交由共用局部调整所有者拥有，不属于 pointer 手势。                                                    |
| `Pane`      | 已聚焦的上报 pane 拥有指针直至 release。motion 与 release 按 press 时绘制的视口原点编码为 pane-local——拖出区域的 drag 仍属于它。release 在手势中途绝不丢弃。                               |
| `Selection` | 已聚焦且未跟踪鼠标的 pane 为其精确 attachment 和投影 generation 拥有一个 Host 选择手势。motion 在 pane 内合并；release 留在最新 motion 之后，并且只有 Host 接受的活动 end 才能请求复制。   |
| `Pending`   | 落在未聚焦 pane 上的 press 触发了 `FocusPanel`；该 press 与随后的输入排队（FIFO，上界 16，motion 合并为最新 cell），直至切换就绪。溢出在任何新输入被接受之前取消并报告有界的输入丢弃说明。 |
| `Link`      | 精确 `Ctrl`+主键 press 拥有一个已绘制 cell，直到同一 cell 的匹配 release。motion 或来源失效会取消；修饰键变化不会把它转交给 pane 或选择 owner。                                            |

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

## 链接激活

精确 `Ctrl`+主键且没有 `Shift` 或 `Alt` 的 press，会在程序鼠标 tracking、聚焦或选择准入
之前取得 pane content cell。这个显式修饰键形成独立手势 owner：已经由 `Pane`、`Selection`
或 `Pending` 拥有的普通手势不会被之后的 Ctrl 变化夺走；`Link` 手势则消费之后的修饰键变化，
直到匹配 release。它不聚焦 pane，也不请求终端控制权，因此只读 Client 也能打开链接。motion、
在不同 cell 释放、释放捕获、pane incarnation 被替换、视口变化或 hit map 失效都会取消，而且
不会把任何事件重放给 pane。

renderer 从同一次绘制的 frame 或保留历史 window 记录链接来源。非捕获式通知卡成功绘制后，
已发布 frame 会抑制所有终端链接来源。若这个 frame 替换了进行中的链接来源，手势会变成已取消
tombstone：匹配 release 会被吞掉，且不产生外部效果。来源把精确 pane／成员 binding、本地
attachment 与 sequence 来源证明，以及 host epoch、内容 generation／revision、
列数、总行数和已绘制视口的绝对起点组合在一起。实时行从 `scrollback_rows` 开始；保留历史行
使用已验证 row window 的 `start_row`。release 只把已绘制 cell 转换一次为终端绝对行列。新的
只读 viewer 可以使用不同 attachment 和 sequence，但在读取行前后及效果交接前，都必须匹配
成员 binding、host epoch、generation、revision、列数和总行数；不匹配不产生效果。

每个临时 viewer 必须在下一次读取或外部效果前确认其唯一一次 detach。detach 结果不确定时不
重试；通道会丢弃该连接并拒绝本次点击。

解析先读取点击行的 render detail，保留 OSC 8 元数据并采用共享的宽字符尾 cell 回到头 cell
规则。没有显式 hyperlink 时，使用与 `terminal link-resolve` 相同的有界小写
`http://`／`https://` 解析器，包括可见视口外经过验证的软折行上下文。一个私有且会 join 的
通道最多接纳一次点击；忙时明确拒绝，不排队也不重试。按顺序排列的已启用 Windows 插件链接
handler 优先。匹配 handler 在单次提交前立即重新验证。handler 启动、失败或结果未知均为这次
点击的最终结果。只有确定无 handler 时，才可通过 Windows shell 打开一次小写 HTTP(S) URL。
Shell 失败可见且不重放。关闭会中断自有等待并 join 通道；它不宣称能取消已经受理的插件
action 或 Windows 拥有的浏览器进程。本次 button-event capture 切片不包含 Ctrl 悬停装饰。

### 链接工作流与正确性

`PointerState` 在释放前拥有 `Link` 手势；`terminal_link_activation::Driver` 拥有 single-flight
准入位与提示；它会 join 的通道从作业被接纳起一直拥有作业，直到一个终局结果。Workbench
Snapshot 与终端 Host 仍是成员 binding 与内容的规范来源，Windows 插件 registry 仍是 handler
顺序的规范来源。输入包括一份精确已绘制来源与 cell、新鲜 Snapshot／viewer 记录、registry
条目，以及插件或 shell 结果。事实缺失或畸形表示拒绝，绝不推断为“确定无 handler”。输出为
`Opened`、`Handled`、`NoLink`、`Busy`、`Failed` 或 `Unknown`；只有插件提交和 Windows shell
启动属于外部效果。

| 状态／事件                                              | 守卫                                     | 下一状态与命令                                                  |
| ------------------------------------------------------- | ---------------------------------------- | --------------------------------------------------------------- |
| `Idle` + 精确 Ctrl+主键按下                             | 已发布 pane／content；没有既有 owner     | `Link`；保留有效来源与 cell，否则成为已取消 tombstone；无效果   |
| `Link` + 移动、Shift/Alt、成员／来源／map／capture 漂移 | press owner 继续消费报告                 | 已取消 `Link`；吞掉匹配 release；无效果                         |
| `Link` + 同 cell 匹配 release                           | 来源与精确 pane 仍是当前值               | single-flight `Resolving`；无 pane 输入、聚焦或控制权请求       |
| `Resolving` + 新鲜读取                                  | 成员与内容围栏两次匹配                   | 固定插件预检、确定无 handler 的浏览器资格，或终局 no-link／失败 |
| 已选择 handler + registry 重读                          | 同一条目、handler、action、URL 与上下文  | 至多一次插件提交；其 started／failed／unknown 结果为终局        |
| 确定无 handler + 小写 HTTP(S)                           | 期限有效，且交接时完整注册目录身份未变化 | 至多一次 `ShellExecute`；成功／失败为终局                       |
| 忙碌准入或关闭                                          | 既有作业拥有通道                         | 不排队／重试地拒绝，或停止等待并 join 自有通道                  |

[`NativeTerminalLinkActivation`](../formal/tla/NativeTerminalLinkActivation.tla) 已登记在
`scripts/tla/models.json`，它在有界范围检查手势归属、过期／registry 拒绝、每次点击至多一个
效果、插件／浏览器资格及 join 通道关闭。`LinkGesture`、`PaintedLinkSource`、`ReadSession`、
插件 `Preflight`／`Invocation`、`EffectHandoff` 与 `Driver` 是实现细化点。TLC 检查声明的有限
状态图；它不证明 Win32 行为、插件执行、终端协议实现，也不证明能取消其他 owner 已经受理的
效果。确定性 Rust 测试与受控 Windows 集成覆盖这些边界。

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
- `OneLinkEffect`：一次匹配的链接 release 最多准入一次插件提交或一次浏览器启动；handler
  失败、结果不明或 registry 漂移绝不落入另一个效果。
- `PaintedLinkFence`：链接坐标与内容围栏来自同一次完整绘制；新的 viewer 必须在外部效果前
  证明捕获的成员与内容版本。
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
| `tui_pointer.rs`                            | `PointerState` 捕获/map/手势生命周期、`FrameHitMap`/`HitMapBuilder` 暂存区域、`Gesture`（`Idle`/`Pending`/`Pane`/`Selection`/`Divider`/`Link`）、`WheelStream`                  |
| `terminal_input/mouse.rs`                   | `MouseEvent` 解码（`decode_sgr`、`decode_x10`）、`pane_owns_pointer`、`encode_pane_report`、`encode_cursor_arrows`                                                              |
| `terminal_input/decoder.rs`                 | 类型化 `DecodedKind::Mouse`/`Ignored` item；X10 三字节 body 原子消费、超时与中止；鼠标形态的畸形/中止/越界运行解码为 `Ignored`                                                  |
| `terminal_input/dispatcher.rs`              | 位于 raw 透传通配之前的显式 `Mouse` 与 `Ignored` 臂——`Ignored` 被消费并丢弃、绝不入 pending；先 flush pending 字节；`RegionsChanged` 置 map 为 stale；外部变化 `invalidate_map` |
| `terminal_lifecycle.rs`                     | `set_pointer_capture` ModeLease 对（`?1002`/`?1006`），enter 时 arm、每条退出路径恢复                                                                                           |
| `tui_shell.rs`                              | 启动 arming、每次 draw 暂存 `HitMapBuilder`、成功才发布、`set_pointer_capture` 回调接线                                                                                         |
| `tui_render.rs`、`tabs`、`contexts`         | 按绘制矩形记录 tab 标记、navigator 行、overlay 本体与 pane 叶的区域                                                                                                             |
| `client_view.rs`、`client_view/bindings.rs` | `Ctrl+B m` → `TogglePointerCapture`，`SelectContextAt`/`FocusPanel`/`SelectTab` disposition，released 指示与指针 notice                                                         |
| `terminal_session/ui.rs`、`state/`          | `pointer_probe` 新鲜事实、`focus_panel`/`select_context_at` 准入、稳定 id 访问器（`tab_id`、`panel_id`、`location`）                                                            |
| `terminal_link_activation.rs`               | 精确来源围栏、OSC 8／纯文本解析、有序 handler 交接、单效果浏览器 fallback、有界通道与 join 关闭                                                                                 |
| `terminal_session.rs`（test 接缝）          | 由真实 projection 帧与 controller 状态支撑的单 pane `Legacy` probe——connected-PTY 见证后端                                                                                      |
| `terminal_input.rs`                         | `PointerProbe`/`PointerTab`/`PointerFrameFacts` 端口表面与 `forward_or_report` 投递                                                                                             |
| `invocation.rs`、`cli/`                     | `tui --mouse off\|on` 准入、help 文本与三种 shell 补全                                                                                                                          |
| `crates/terminal-protocol`                  | `TerminalInteraction` 可信 Rust API，暴露已验证帧的 tracking/format/几何/cursor 模式事实                                                                                        |

## 验证

以下 Unix PTY 测试维护共享契约，不构成 Windows 运行证据。Windows cell 降级由编码器、
指针及 attach 回归，以及[终端流](windows-terminal-streams.zh-CN.md)记录的打包
control/attach 字节实测覆盖。

| 表面                    | 证据                                                                                                                                                                                                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 解码                    | `terminal_input/mouse/tests` SGR/X10 表——press/release/motion/wheel/修饰、跨 chunk 原子性、畸形拒绝且不退化为按键；`terminal_input/decoder/tests`——合法 SGR/X10 item、X10 分片喂入、超时 drain、ESC/C1 中止、畸形 body、越界运行、与按键/粘贴交错——所有鼠标形态畸形字节均为 `Ignored`            |
| 编码契约                | `encode_pane_report` 覆盖每个 tracking × format 单元的表行、release 结尾、去重、夹取、legacy 上界与 SGR cell 降级                                                                                                                                                                                |
| Hit map                 | `tui_pointer/tests` 层序、边缘/外侧 cell、overlay 捕获与 backdrop 关闭——包括落在未遮盖 tab 条与 pane 上的外点 press 既不选择也不转发、stale-map 丢弃                                                                                                                                             |
| 手势与 pending focus    | `tui_pointer/tests` press/release 顺序、队列上界与溢出取消、超时、orphan、只读、目标移动拒绝、settled 无帧等待与按事实准入                                                                                                                                                                       |
| dispatcher 端口写       | `terminal_input/dispatcher/tests`——畸形 SGR/X10 与中止/超时鼠标运行产生零转发字节，而普通按键、粘贴与无关未知 CSI 按序透传                                                                                                                                                                       |
| 滚轮流                  | `tui_pointer/tests` 每 notch 三次、每次一屏上限、反转清零、排空后关闭                                                                                                                                                                                                                            |
| 捕获生命周期            | `terminal_lifecycle/tests` 租约配对与恢复；dispatcher `Ctrl+B m` 切换测试；状态指示                                                                                                                                                                                                              |
| 链接点击                | recording pointer port 覆盖精确修饰键、同 cell 释放、取消、既有手势 owner 与忙碌准入；共享 OSC 8 宽字符和折行 HTTP(S) 解析器保留其 CLI 集成 fixture；路由测试用 recording browser port 覆盖优先级、受理后不明、确定无 handler fallback 与单效果；受控 Windows 集成覆盖新鲜 viewer 和原生插件传输 |
| CLI                     | `invocation/tests` `tui --mouse` 值准入、裸调用/其他族拒绝、`--format` 拒绝；`cli/completion` 脚本                                                                                                                                                                                               |
| 真实 PTY（Legacy 接缝） | `tui_shell/connected_tests::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes`——记录的 pane-local SGR 字节、pane-local 平移、只读丢弃、SGR cell 降级、滚轮转方向键、捕获切换、模式恢复                                                                                    |
| 真实 PTY（公开 Native） | `tests/local_startup_process/native_pointer.rs`——真实 PTY 上 spawn 的 `orchbee` 二进制：捕获 arm、X10 与 SGR pane-local 报告、未聚焦 pane 点击聚焦与排队放行、只读拒绝、滚轮转方向键、`Ctrl+B m`、`--mouse off`、退出恢复，以及零像素宿主几何下 cell 寻址报告仍可用、`?1016` 使用 SGR cell 降级  |

## 共享右键策略

[Panel 输入策略](native-panel-input.zh-CN.md) 在焦点或透传之前裁决新的右键按下。默认 `orchbee` 将其保留给[动作菜单](native-action-menus.zh-CN.md)；`panel` 仅允许启用鼠标上报的终端内容内无修饰键按下，并遵循现有控制权与就绪检查。排队右键在任何字节发出前复核策略。已准入手势捕获按钮与精确 Pane 实例，其他按钮不能注入按下或释放它，旧格式通用释放映射到归属按钮。改变策略作用于下一个手势，捕获/焦点/实例取消规则继续生效。打开和取消菜单保持焦点，并捕获匹配的 release。

## Presentation 选择器

会话就绪或已选择但未附着时，包含多个 Presentation 的 Panel 在现有标题栏展示 `[<] [position/count] [>]`：单面板／放大使用外框标题，分屏使用叶节点标题。控件为标题／状态保留十二个单元；窄标题栏和 Minimal 层级保留既有标签／数量及键盘路径。每个箭头有三个单元的命中区域。数量和空格不响应；分屏 Pane 命中区域排除整个选择器条，因此控件不与终端内容或聚焦区域重叠。不增加标题行或主机鼠标模式。

仅无修饰键的左键按下选择绘制时的精确目标 Presentation。按钮按规范成员顺序循环，与 `Ctrl+B (`／`Ctrl+B )` 等价。已绑定目标包含当前 `PaneRef` 和目的 Presentation ID。空目标包含结构 Pane key 与精确选中的 Presentation ID；新鲜探针必须仍确认该成员未绑定。空 Panel 内容区的按下只聚焦，不开始终端手势。新鲜探针必须仍在当前 desired anchor 下保留同一实例或空选择；UI 随后在活动 Tab 内解析 Panel／成员，才提交既有 Switch 事务。已删除／替换成员、旧 anchor、模态输入、捕获 overlay 和已释放鼠标捕获均不能改投手势。终端只读模式不禁止 Client 本地导航；选择从不转发终端字节，也不绕过子项仅 binding 附着。成功点击后旧命中图失效，直到重绘。

证据覆盖按钮边界、不响应的数量／空格、不重叠的分屏矩形、单面板／分屏／放大渲染、窄窗口及 Minimal 降级、修饰键／非左键报告、捕获／overlay／模态守卫、旧实例／anchor、成员移除及实际 UI 准入精确子项 Switch。真实主机视觉／多 Client 验收仍随最终 Windows 全量门禁统一执行。
