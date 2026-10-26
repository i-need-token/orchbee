# 原生鼠标输入

## 范围与归属

本参考治理原生 Client 的指针行为：宿主捕获生命周期、最后一个已绘制帧的 hit map、各绘制
区域接受的手势词表、向 pane 内程序的透传编码，以及备用屏幕的滚轮回退。它与
`tui_pointer.rs`、`terminal_input/mouse.rs`、`terminal_input/decoder.rs`、
`terminal_input/dispatcher.rs`、`terminal_lifecycle.rs`、`tui_render.rs` 及其
`tabs`/`contexts` 绘制器、`tui_shell.rs`、`client_view.rs` 与 `client_view/bindings.rs`、
`workbench_client/ui.rs`、`invocation.rs`、`terminal_link_activation.rs`、`localization.rs`
共同维护。仓库标准
[`docs/tui/mouse-input.md`](../../../docs/zh-CN/tui/mouse-input.md) 拥有该映射必须满足的
规则；[原生终端环境](native-terminal-environment.md) 拥有 arming 与恢复所依托的
mode-lease 机制。[原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)拥有未跟踪
pane 的选择及其复制生命周期。

Windows [分隔线调整](native-divider-resize.zh-CN.md) 拥有可撤销 split 预览及一次仅松手提交的共享比例修改。

Windows [Tab 排列](native-tab-groups.zh-CN.md#指针排列与菜单导航) 拥有按下／拖拽／释放、
所选成员、分组边界与静态反馈。InputDispatcher 对提交和取消均捕获释放输入包内的
按键、粘贴及待解码片段。

Windows [动作菜单](native-action-menus.zh-CN.md) 在普通聚焦之前消费属于 Client 的右键按下。
菜单捕获不能替换已有真实手势或 wheel 流。

中键表面、pixel 格式报告、选择复制
键位、三击选行和 Ctrl 悬停链接装饰仍属于独立决策。

## 各界面的鼠标控件

鼠标与键盘进入相同的 native 状态所有者。可见方括号控件和完整行具有准确命中矩形；禁用、截断或被覆盖的单元格不能激活底层目标。本地导航可以在按下时执行。原生表单、菜单、设置、复制、调整大小和 Agent 看板按钮等待匹配的普通左键释放，令牌和矩形必须仍与绘制时相同。键盘输入、失焦、释放捕获和绘制授权变化撤销该次按下。决策拥有数据包尾部及迟到释放，随后新输入回到保留的终端焦点。

确认、rename/setup/worktree 表单、Settings、Machine picker 与插件 popup 为外侧蒙层关闭
登记精确的已绘制 X 目标。普通左键按下及匹配释放必须始终位于同一个完整绘制表面的外侧；
进入正文、新 token、命中图失效或失焦均取消该意图。蒙层上的滚轮及修饰报告不生效。
关闭沿用既有 X 所有者，保留已接受提交与恢复；底层 pane、控件和 shell 均不会收到该手势
或决定包尾部。既有菜单、Help、navigator 与 Agent 看板蒙层路径保持其本地返回策略。

`tui_composition::project_capture` 是独占绘制、输入和通知准入的唯一权威。Machine 捕获覆盖原生
独占表面；原有原生顺序覆盖 Copy，Copy 再覆盖 Resize。Claim 表达保留的捕获意图，不代表表面
已经绘制。被选所有者的 context 缺失或变旧时，输入仍被消费，不能暴露更低层所有者。即使 Resize
成为下一个所有者，退出 Copy 仍保留其物理数据包围栏。

Client 快捷键帮助仍不捕获输入。`client_chrome_overlay_allowed` 允许它覆盖保留的 Copy 或 Resize
进行绘制和鼠标路由，但在 Machine 或原生独占捕获之下拒绝准入。帮助鼠标控件先于保留模式运行；
普通按键和粘贴路由保持该模式的既有契约。关闭帮助恢复父级，不释放父级输入租约。

独立获准的更高捕获覆盖 chrome 手势时，dispatcher 会立即将 Divider、侧栏宽度、Tab 拖动或
Tab 新建的 arm 退役为既有的一次性释放清理。选择和 modal 交互保留各自所有者。只要存在独占
所有者，帧绘制器也会独立抑制这些预览，包括在输入协调与绘制之间安装的捕获。仍在加载的插件
popup 允许其精确已绘制关闭目标；其正文、按键和粘贴的就绪性仍由插件所有者管理。

撤销后，中央 generic 所有者以及现有分隔条、侧栏宽度、Tab 新建、Tab 拖动和已绘制 modal
所有者只保留类型化主键释放清理。该清理在捕获、宿主焦点和命中图守卫之前运行，也涵盖释放捕获后
已经排队的报告。准确的迟到释放只消费一次并围住其整个物理数据包，且不重新命中或产生效果；新的
非滚轮按下会退役债务，期间的指针报告不能借债务重新命中，孤立释放不会创建债务。pane 透传、链接、
选择及其清理所有者不受本契约改变。

对于已绘制 modal 控件，已取消的已绘制按下记录与 dispatcher 中对应的按钮释放债务（包括 action
menu 配对的数据包债务）会在同一个所有者前置步骤中结算。更新的 Settings 或 Machine 表面不能隐藏
该清理，也不能让之后的释放再次建立数据包围栏。仍然活动且在释放时提交的 modal 控件或复制表面拖动
会绕过数据包清理，直到其自身所有者处理精确释放；已取消的清理绝不重新命中该目标，也不执行其效果。

| 界面                        | 鼠标流程                                                                                     |
| --------------------------- | -------------------------------------------------------------------------------------------- |
| 机器／Workspace／Agent 搜索 | 方括号分类切换视图；完整行选择准确身份。Agent 筛选、搜索、清除／完成和 X 只作用于选择器。    |
| Workspace 导航器            | X 关闭；每列有独立滚动区域。滚动只改变本地焦点，不激活 context 或 Tab。                      |
| 快捷键帮助                  | X 和前／后页控件；文档内滚轮每格翻一页。关闭恢复底层复制、调整大小或侧栏模式。               |
| 原生确认及重命名            | X、取消及确认／提交共享键盘所有者。完整警告和操作行未绘制时，破坏性确认不可用。              |
| Workspace／Worktree 表单    | 可点击字段、准确选项行、取消／提交及阶段对应的状态／隐藏控件；已接受操作保留现有完成所有者。 |
| 设置                        | 可点击行、选项箭头、子项确认／取消、页面返回和标题 X；只有保存写入。                         |
| Agent 看板                  | 整行选择、打开、刷新和 X；打开定位到选中的稳定 Agent 身份。                                  |
| 复制和调整大小模式          | 从窗格动作菜单进入；模式工具栏、复制拖动／滚动及调整方向使用既有 native 所有者。             |

滚轮仅属于指针下已绘制的列表；标题、按钮、边框留白和背景均无效。列表每格归一化为三行／选项，限制累计工作，每次最多结算一个已绘制视口；Worktree 容量按双行选项计算。反向丢弃旧方向工作，保留第一个反向滚轮。所有者／来源／几何变化取消积压。待定列表移动先于后续键盘决策结算，产生的新原生草稿令牌仍须成功绘制。滚动路径不执行选项，也不改变设置值。

释放捕获会将鼠标输入交还宿主。恢复仍使用现有键盘捕获切换；本地控件无法收回宿主未发送的鼠标事件。

完整帮助页放不下时，调整窗口提示仍保留已绘制的 X 按钮。关闭仍捕获决策数据包并恢复底层模式；也可扩大窗口恢复保留的帮助页。页面拒绝呈现时不能留下只有键盘退出路径的不可见鼠标所有者。

已读入的数据包先解析待定输入帧，再处理解码器的空闲超时。即使绘制或原生状态处理超过 50 ms 等待时间，也不能把拆开的鼠标报告变成单独的 Escape 加可打印坐标，无论当前所有者是终端还是原生表单。只有未读到输入的轮询才使这些等待过期；真正空闲时的单独 Escape 仍通过既有所有者取消或转发。shell 循环及 modal／dispatcher 轨迹覆盖跨繁忙帧的就绪 SGR／X10 片段，并保留空闲 Escape 的验证。

## 侧栏导航输入边界

Windows 上，`Ctrl+B Tab` 在侧栏键盘导航与终端输入之间切换，保留 `Ctrl+B Shift+B` 别名。进入导航时，在有效几何允许呈现时
显示侧栏。`Ctrl+B b` 保持切换显隐的含义。`Picker` 拥有可见树、展开状态、
稳定行焦点及滚动窗口；活动终端上下文保持独立。`InputDispatcher` 拥有输入
租约及包边界保护。不增加服务端、线协议或持久状态。

shell 的视口同步在派发已读入的包之前及改变布局的动作之后结算有效侧栏可见性。
视口采样缺失时，结算使用上次尺寸。隐藏几何先退出 Picker 导航，然后由分发器协调
捕获待处理片段。普通 backdrop 按钮按下真正关闭紧凑侧栏时，也在同一本地转换中
退出导航。背景滚轮无效。帮助保留底层侧栏投影，关闭时返回该表面。

派发在返回绘制之前协调最后一条指针报告，即使后面没有字节。普通背景按钮按下在
PublishFrame 之前退休；外部滚轮保留导航。轨迹覆盖决策包边界、保留的尾部及发布后的新输入。

shell 回归轨迹通过真实同步边界测试 resize 与 backdrop Press 紧随 Enter 或跨读取
CSI，断言成功新帧放行后续输入之前没有 Selection、没有终端字节。这才是模型
RetireSidebar 事件的生产细化；测试预先手动让 Picker 观察到隐藏不能证明该顺序。

| 事件／输入                 | 守卫与可观察结果                                                                                                                                            |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 进入导航                   | 清除竞争的本地模式，显示侧栏并请求 Picker 进入；捕获开启包的尾部。                                                                                          |
| 导航／展开                 | Picker 的类型化按键决策只改变本地焦点、展开及窗口；不产生 Selection 请求或终端输入。                                                                        |
| F1 帮助                    | 用既有快捷键帮助覆盖树而不退出导航，包括紧凑浮层；F1、Escape 或外部按下关闭帮助并回到侧栏，PageUp/PageDown 翻阅注册表派生的条目，被覆盖的树不响应导航按键。 |
| 显式激活 Context           | 重新验证聚焦 Context，调用既有侧栏 Selection 路径，退出导航并捕获决策包尾部。                                                                               |
| 打开对象菜单               | 精确且仍存活的行必须存在于最后成功绘制的命中图，且属于当前机器和 Session；以该图的 Session/revision 证明包装菜单目标。开启不改变 Picker 焦点或终端上下文。  |
| 取消菜单                   | 既有菜单捕获保护决策包及已轮询输入直到新帧；Picker 调和保留的焦点身份。                                                                                     |
| Escape／侧栏隐藏／竞争捕获 | 无激活地退出导航；捕获待完成片段和退出包直到新命中图发布。                                                                                                  |
| 导航内未绑定按键或粘贴     | 无动作地消费；显式输入拥有者之外保持原有终端字节路径。                                                                                                      |

Project 行及展开控件按下请求分发器先取得粘贴租约，再聚焦并展开精确 Project；租约
写入失败时 Picker 保持不变，绝不激活终端。行菜单按钮、
右键及键盘菜单共用身份验证；打开菜单不会先聚焦或激活所属行。展开、行及菜单
命中矩形互不重叠。其他机器不能通过静默切换当前端点打开菜单。菜单命令可用性
及完整来源证明复验仍归既有原生菜单拥有者。

确定性证据归既有指针与派发器测试：同名异身份、删除或替换行、其他机器或 Session、
未绘制目标、键盘与指针菜单准入、捕获优先级、模式退出及拆分决策包。轨迹断言
Picker 请求、菜单目标、终端字节及返回后的输入归属。宿主图片只证明呈现；
受控输入轨迹不声称覆盖物理鼠标。

| 抽象事件                                             | 生产细化与确定性证据                                                                                                                        |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| EnterSidebar / SidebarKey                            | 显式前缀动作与 Picker 按键分发；`sidebar_mode_owns_only_explicit_navigation_and_fences_exit_packets` 轨迹对照捕获输入和保持不变的终端输入。 |
| OpenMenu / CloseMenu                                 | 共享已绘制目标解析器与既有菜单捕获／结算；键盘和行菜单轨迹保留 Picker 焦点并归还粘贴租约。                                                  |
| RetireSidebar / CoveringCapture / EndCoveringCapture | Picker 退出导航，分发器协调竞争所有者并把待处理解码片段标记为捕获；退出轨迹包含跨读取按键、外部隐藏、重绘与之后的终端字节。                 |
| PublishFrame / PacketBoundary                        | 成功发布命中图才清除退出捕获；同步只在该发布之后清除包级捕获。                                                                              |
| BeginFragment / FinishFragment                       | 既有按键／粘贴解码器跨读取保留包来源；捕获期间开始的片段不能在退出之后成为终端输入。                                                        |

## 侧栏宽度输入

Picker 拥有侧栏偏好宽度；`SidebarSurface` 把同一个偏好传给布局、终端几何及绘制。
几何与边界仍由[壳层布局](tui-shell-layout.zh-CN.md)治理。`DrawnSidebarWidth` 记录
视口、包含偏好的完整表面及派生侧栏。只有成功帧中露出的右边框可以准入调整；
键盘导航使用同一绘制来源，不要求鼠标捕获。

`PointerState` 拥有一个可撤销的宽度拖动。移动会在下一帧前投影实际侧栏宽度、行布局、
边框命中及停靠终端几何；按住期间偏好不变。Compact 覆盖层只改变自身宽度，不缩小
其下方终端。不可变规范来源与最后成功草稿帧凭据分离，使第二次按住移动可跨过自身
首帧存活，同时仍取消外部过期几何。普通左键松手重新验证
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

真实 shell 布局回调返回同步是否观察到外部宿主或侧栏所有者变化。自身草稿投影也同步
终端尺寸，但不把自身报告为外部变化。分发器在处理 Press 或 Motion 后立即
根据该结果废弃旧命中图及手势，包括同一次读取内不改变尺寸层级的宽度变化和仅高度
变化。缓存的视口分类不能代替该观测。真实 shell 轨迹在同包松手之前经过此边界，
之后只有新绘制及新按下才可准入另一次拖动。

宽度草稿结束也退休其命中图，即使 Escape、Shift 或无变化松手保留偏好：其状态提示
遮住了规范 Menu 控件。既有重绘保护捕获结束决策及片段，直到规范帧成功绘制；同包
按下不能激活这些不可见控件。预览绘制轨迹检查重绘前拒绝 Menu 按下、重绘后准入，
同时保持 Picker 焦点不变。

## 捕获策略

- 启动通过一个 `ModeLease` arm button-event tracking（`?1002h`）与 SGR 编码（`?1006h`）；
  `orchbee tui --mouse off` 跳过 arming。基线完成后，dispatcher 为保留的外层端点发出一次 `1003`
  DECRQM 查询与 DA1 屏障。只有 reset 报告（`2`）后接该屏障，才准入全局任意移动跟踪。不支持或
  固定模式的报告保持 button-only；矛盾的 set 报告、缺少回复或 300 ms 截止均结算为 unknown。查询在首帧视口绘制后才启动。
  打开菜单和重新 arm 捕获都不重试。同一租约在每条退出路径上禁用 `1003`、`1006` 和 `1002`——正常返回、失败、owner 线程 panic 以及每个
  终止信号。
- `Ctrl+B m` 在运行时释放并重新 arm 捕获。释放只在模式写成功后生效，因此持久状态指示
  （`mouse released · Ctrl+b m restores` / `鼠标已释放 · Ctrl+b m 恢复`）与宿主实际上报
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

Windows 下，捕获已启用时收到的实际无修饰、无按钮控制台移动，也可准入悬停呈现，不改变 VT 查询裁决
或捕获模式。侧栏、Tab、按钮和菜单命中表共用该观察能力。粘贴序列不能准入它，已释放捕获保持释放。
既不提供已准入 VT 移动、也不提供控制台移动的宿主，保留方括号动作与键盘导航，不声称具有悬停能力。

## 区域映射

渲染器把绘制的每个交互区域记入暂存的 `HitMapBuilder`；shell 只在该帧绘制完成后发布
map。区域携带稳定身份——`WorkbenchTabId`、`NativeContextLocation`、`WorkbenchPanelId`——
点击时按新探测解析为位置，因此绘制与点击之间的重排不会误路由。层从前到后解析；同层内
第一个匹配区域优先。具体控件先于包含它的表面区域记录：

中央交互 projection 现已作为迁移基础提供。feature 只能在 renderer 准入并绘制精确几何后记录
`InteractionSpec`。记录组合一份封闭的 `InteractionTarget`（携带 owner 的完整语义凭据）、命中
层、精确命中矩形、完整绘制矩形、显式主键契约（`ActivateOnRelease`、`SelectOnPress` 或
`Specialized`）及 availability。fingerprint 还会规范化该绘制矩形内最前方的语义 winner；因此
新增、移除或替换 child 会改变其包含行的有效暴露形状，即使 press cell 本身未被覆盖，矩形外
目标则不会。较小的子项命中矩形可以引用完整绘制矩形。目标身份与手势策略都不能从文本、
颜色、位置或宽泛的整行目标推断；子 action 必须先于包含它的整行记录。

`PointerState` 只把 `Rest`、`Hovered` 或 `Pressed` 投影到下一个候选 frame；它不写键盘焦点、
current 或 checked 状态、availability、pending 状态或任何领域模型。动作菜单的指针适配器
还会把真实可用行移动报告给所有者，由其只更新[临时菜单候选](native-action-menus.md#interaction)。
其他集合保留临时选择。`Hovered` 要求全局 `AnyMotion` 准入、菜单范围 `MenuMotion` 请求完成，
或独立观察到 Windows 控制台移动，且必须收到真实的无按钮移动报告。button-only 可观测性、释放捕获、宿主失焦及可观测性降级
都会清掉保留位置，因此恢复不能合成 hover。dispatcher 只有在能力准入和类型化模式切换都成功
完成后，才发布全局 `AnyMotion`；菜单范围可观测性则要求其自身的切换完成。不确定的部分切换会通过恢复边界退出，
而不发布回退状态。捕获释放期间收到的回复只结算准入；下一次捕获使用该结果，不再查询。迟到及
重复回复不能重新开启已结算准入。全局尚未准入时，动作菜单保留带按钮回退的菜单范围请求；
退出后恢复全局级别，绝不隐式重新启用已释放捕获。
模式支持准入与语义交付证据仍是两个维度；[终端环境参考](native-terminal-environment.zh-CN.md)
拥有探测与宿主见证。

启用的 `ActivateOnRelease` 目标在无修饰左键 press 时 arm，且只有 release 仍解析为同一个类型化
目标、完整绘制矩形、契约和 availability 时才激活；其层、精确命中范围及有效暴露形状也必须
一致。移出目标、替换、遮挡、禁用、结构失效或归属丢失都会永久取消 arm；同一 fingerprint
的纯视觉重绘不会取消。已取消 arm 会消费迟到的 release，且不能改投目标；已经观测到的静止
位置仍可在下一份成功 map 上重新对齐 hover。`SelectOnPress` 只供明确拥有临时导航的 feature
使用；`Specialized` 则把 press 交给 feature 专用的拖动或选择状态机。

shell 在候选 draw 前快照 projection，且只在该 frame 成功 flush 后发布候选 map。发布会按静止的
已观测位置重新验证；若 projection 因此变化，shell 只安排一次后续 turn 重绘，后续 map 携带
新 revision，不会再次自行调度。失败或部分 draw 不发布候选，并在 loop 致命退出前使先前指针
权威失效。只有当一个 packet 全部由无键 motion 构成、每个解码 item 都未改变 projection，且
decoder、timer、布局与 owner transition 均无其它变化时，才可跳过重绘。

Settings、动作菜单、侧边栏、Agent board、Help、Navigator、Machine Picker、Presentation
选择器、Tab 创建及全局 Menu 和 Sidebar Restore 按钮都使用该中央协议。通用控件在匹配释放
时激活。侧边栏 Menu/Create 保留专用菜单打开 owner，Agent-board 行只用 `SelectOnPress`
进行临时导航。Windows Tab/group 排列、分隔线、滚动条、Copy 选择及 pane 输入保留专用
手势 owner。禁用目标保持可见并消费输入，不会穿透到包含它的行或 pane。

Help 控件与滚动携带精确打开 episode 和成功呈现的页；完整拒绝只准入关闭控件。Help 仍不
捕获键盘输入与粘贴。Navigator 行和关闭携带打开身份与 catalog 来源。Machine Picker 使用
精确打开身份、模式、有序 catalog 及真实筛选／查询依据；renderer 只向中央 map 暂存目标，
不拥有局部矩形缓存。异步替换、重新打开或来源变化不能激活旧候选。全局 chrome action
使用稳定动作身份，在释放时复验实时准入；Sidebar Restore 还要求侧边栏可用且当前隐藏。

Copy、Resize 与原生表单发布包含 availability 和精确几何的显式专用 modal-control fingerprint。
只有 modal 形状的 raw region 不会获得隐式 Enabled 权威。共享控件应用 hover／pressed
背景时，保留 focus、current、checked、临时导航、状态及 operation 提示。已取消的 Tab 点击
也会退休 pressed 呈现；移回不会重臂，之后跨阈值的拖动仍遵循专用排列契约。

| 绘制表面                    | 层               | 目标                                                      | 接受的手势                                                                               |
| --------------------------- | ---------------- | --------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Navigator context 行        | `Overlay`        | Windows `NavigatorRow(Row)`；其它平台 `Context(location)` | 匹配主键释放选择精确绘制的 context                                                       |
| Navigator tab 行            | `Overlay`        | Windows `NavigatorRow(Row)`；其它平台 `Tab(tab_id)`       | 匹配主键释放选择精确绘制的 Tab                                                           |
| 控件之外的 overlay 本体     | `OverlaySurface` | `OverlaySurface`                                          | 吞掉——绝不到达下层                                                                       |
| 捕获型 overlay 之外         | —                | backdrop                                                  | 普通按钮按下关闭，滚轮无效。底层任何区域，包括未遮盖的 tab 构件或 pane，都看不到该事件。 |
| 紧凑 tab-strip 标记         | `Foreground`     | 带显式交互元数据的 `Tab(tab_id)`                          | Windows 按下为点击／拖拽建立手势，释放时激活或一次提交；其它平台在匹配释放时选择         |
| Pane 叶视口（content 矩形） | `Foreground`     | `Pane{tab,panel}` + 已绘制来源                            | 精确 `Ctrl`+主键点击打开已绘制链接；其他 Press 先聚焦，再使用程序透传或 Client 选择      |
| 未绑定的 pane 叶            | `Foreground`     | `Pane{tab,panel}`                                         | Press 只聚焦；没有程序收到该事件                                                         |
| Windows 可见 pane 分隔线    | `Foreground`     | `Divider(source)`                                         | 无修饰左键按下开始预览，松手一次应用，Esc 或 Shift 取消                                  |
| 其余一切（状态、空隙）      | —                | dead space                                                | 忽略                                                                                     |

pane 区域上的 press 按*被绘制的那个 Tab* 解析：当绘制时的 Tab 不再处于 active 的
stale-map 边界退化为普通 Tab 选择，而不是聚焦一个不可达的 panel。

Windows 导航器为每个完整绘制行使用 `NavigatorRow(episode, source, target)` 凭据，关闭使用
精确 `DrawSource`。只有该精确行上无修饰主键的匹配释放才可请求激活，
键盘 Enter 还要求已绘焦点。[导航器契约](native-context-selection.zh-CN.md#windows-捕获输入的导航器)
拥有准入与取消。背景按下仅关闭而不穿透，滚轮及现有 Shift 宿主绕过除外；滚轮仅滚动已绘制的列而不激活，行上的
修饰键／右键按下不激活。已消费的行／背景决策在成功新帧后仍拥有对应释放事件，
仅释放、宿主焦点丢失或新的非 Shift 合格按下可清除此归属，单纯绘制退休不可清除。
进入时通过清理所有者取消现有手势，不复制／导出选择，也不刷新排队的 pane 鼠标输入。

## 手势生命周期

窗格右侧独立 chrome 列在 Foreground 层记录 `Scrollbar{pane,content,source}`，与内容和分屏
分隔线不重叠。无修饰左键按下在轨道上跳转或抓取滑块；移动和释放即使越过另一窗格也由
Client 所有。`Scrollbar` 手势保留准确窗格身份和绘制几何，在历史范围、generation、几何、
焦点、捕获、键盘输入或宿主焦点变化时取消。未知／加载中的窗口不暴露新目标。
[viewport 契约](native-terminal-viewport.zh-CN.md#滚动条交互)拥有有界导航、实时输出取消和
恢复。选择／复制模式及程序鼠标上报保留现有优先级；滚动条事件不编码为终端输入。

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

Modal 输入捕获让其已绘制控件独占指针归属；手势不能到达底层 pane。普通 overlay 在外部按钮按下时关闭，modal 表面则吞掉它。已绘制列表外的滚轮无效。

使用当前 painted-release arm 的 modal 控件绑定到精确的已绘制目标及矩形。移出该矩形会永久
取消本次 arm；移回矩形内或重新绘制都不会重新 arm。仅视觉变化且目标与矩形相同时，只要指针
从未移出，就会保留原 arm。激活仍要求匹配的 release；被取消的交互必须重新 press。这是当前
modal 控件的不变式，不表示所有 native chrome 目标都已经从 press 激活迁移完成。

## Pane 透传

落在 pane 内容区的 fresh 事件按照精确的已绘制 `PaneRef`、视口和最新已验证 frame
解析；`Shift` 始终属于宿主。程序路由与编码器使用同一个 tracking 判定：

| Tracking | 程序可上报的事件类别                                         |
| -------- | ------------------------------------------------------------ |
| None     | 无。可以进入 Client 选择／历史，或独立的 DEC 1007 回退。     |
| X10      | 仅左键、中键或右键按下。                                     |
| Normal   | 可编码的按下及已捕获的释放；不报告移动。                     |
| Button   | 可编码的按下、有键按住的移动及已捕获的释放；不报告无键移动。 |
| Any      | Button 的类别，外加无键移动。                                |

fresh 准入比线协议的可上报范围更窄。可上报的非滚轮按键 press 可以开始 `Pane` 手势；
可上报的滚轮 press 是一次性事件。没有该 press-time owner 的 release 或有键移动仍是无操作的
Client 侧事件，绝不重新定向给指针下的 pane。无键 `Any` 移动是唯一无状态的程序事件：它只
到达精确且已经聚焦、处于 live 和 controlling 状态的 pane，且必须位于已绘制内容矩形内；它
不开始手势、不改变焦点，也不会迁移到同级或替换后的实例。当前启动捕获仍为 button-event
`1002`；本路由自身不会开启全局 any-motion 捕获。

一旦准入，`Pane` 手势保留其精确 pane 实例、已绘制视口原点和按键，直到匹配的 release。
即使新 frame 改变 tracking，已接受的 motion 与 release 仍属于原 owner。当前 tracking gate
仍决定是否产生字节：新模式不报告的事件类别会被静默消费，但其匹配 release 仍会清除手势。
这把手势清理与 fresh Client 路由分开，防止无主尾部变成选择、历史操作或另一个 pane 的输入。

未启用 tracking 时，只有启用 DEC 1007 的备用屏幕 pane 上的垂直滚轮 press 会进入下文的
方向键回退。启用 tracking 时不会使用该回退；活动模式也只能取得其实际上报的类别。例如
X10 滚轮仍属于 Client，可以驱动主屏幕的保留历史，且不会产生非法程序报告。未跟踪鼠标的
pane 绝不接收选择字节；其普通主键手势遵循
[原生终端选择与效果](native-terminal-selection-effects.zh-CN.md)。

被转发的事件按已绘制内容原点转换为 pane-local 坐标，并由唯一 tracking × format 编码器
处理。已捕获的同 cell motion 会去重；视口外的非 release 事件只有在 motion tracking 且有键
按住时才上报，并夹到边缘 cell。legacy `Default` 拒绝超过 222 的坐标而不回绕；
`Utf8` 把坐标编码为 codepoint；SGR 使用 `M`/`m` 结尾；URXVT 使用十进制形式。
仅有 cell 坐标的输入在 pane 请求 `SgrPixels`（`?1016`）时使用 SGR cell，与 Herdr
的 cell 回退一致；它不虚构 pixel 位置，也不在宿主捕获中启用 `?1016`。

字节投递复用既有 `forward` 路径，包括其 `Controlling` 检查、只读拒绝与确认语义。
有 attachment 的 pane 可以获得本地读取焦点而不拥有输入权限；未绑定叶的 press 只聚焦而不发送
pane 输入。mode 仍未知的 content press 在原始 pending 期限内等待。上报 pane 一旦明确为
`ReadOnly`，其属于程序的 press 会被丢弃并报告输入说明，不隐式 acquire。读取焦点绝不
代替应用输入权限，历史也绝不把已捕获的程序手势变成 Client 选择。

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

## 滚轮回退

在程序不上报 tracking 且开启 DEC 1007 的备用屏幕 pane 上，滚轮 notch 编码为方向键按下——每个 notch 三
次，使用该 pane 的 application-cursor 形式（`ESC O A` 对 `ESC [ A`）。per-pane 流为突发
设界：方向反转清掉旧方向的排队余量，并接纳新方向的第一个 notch，包括两个方向同处一个
输入包的情况；每次 flush 至多发出一屏方向键，发现队列为空
的 tick 关闭该流，使下一个 notch 重新开始。每次 flush 都按最新帧复核转发条件；关闭
DEC 1007、离开备用屏幕、开启 tracking 或进入历史会丢弃排队的方向键。DEC 1007 关闭时，
备用屏幕滚轮事件不发送应用输入。回归见证覆盖立即反转、多屏积压下的反转、新方向的
application-cursor 编码、有界排空，以及已退休尾部不再恢复。在未启用 tracking 的 primary 屏幕内容区，向上滚动一个
notch 从实时尾部向前回看三行；后续每个 notch 本地移动三行。实时尾部向下滚动、历史为空
或帧不可用时均无效果、不提示。滚轮绝不改变焦点或从标题/边框滚动内容；包括历史中，
Shift 始终属于宿主。进入、合并导航与恢复沿用[保留视口](native-terminal-viewport.zh-CN.md)所有者。

## 不变式与时序规则

- `LastDrawnOnly`：事件只按最近一次完整成功 draw 的 map 解析；布局变化与下一次发布之间
  的每个指针事件被丢弃而绝不误路由。
- `PublishAfterDraw`：暂存 map 仅当其所属帧的绘制完成才成为权威——失败的 draw 不发布
  任何内容，且由于宿主可能已有部分候选内容，会退休先前 map。
- `ExplicitInteraction`：通用 hover 与激活要求已绘制的 `InteractionSpec` 元数据；label、颜色、
  region variant 与坐标都不能发明契约。
- `ReleaseFingerprint`：通用激活要求同一目标、绘制矩形、契约、availability 及权威命中；
  frame 序号本身既不会取消，也不会重新 arm。
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

| 生产表面                                                                                                 | 模型对应                                                                                                                                                                          |
| -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tui_pointer.rs`、`tui_pointer/interaction.rs`                                                           | `PointerState` 捕获/map/手势生命周期、类型化交互元数据与 projection、稳定 release fingerprint、`FrameHitMap`/`HitMapBuilder` 暂存区域、specialized `Gesture` owner、`WheelStream` |
| `terminal_input/mouse.rs`                                                                                | `MouseEvent` 解码（`decode_sgr`、`decode_x10`）、`route_pane_pointer`、`encode_pane_report`、`encode_cursor_arrows`                                                               |
| `terminal_input/decoder.rs`                                                                              | 类型化 `DecodedKind::Mouse`/`Ignored` item；X10 三字节 body 原子消费、超时与中止；鼠标形态的畸形/中止/越界运行解码为 `Ignored`                                                    |
| `terminal_input/dispatcher.rs`                                                                           | raw 透传前的显式 `Mouse` 与 `Ignored` 臂；通用指针 outcome 与整包不变 motion 重绘裁决；pending 字节及 decoder deadline 保持顺序；结构变化使 map 失效                              |
| `terminal_lifecycle.rs`                                                                                  | 基线 Button/SGR 捕获、一次性类型化 `1003` 准入、已结算 motion 可观测性及每条退出路径的日志式恢复                                                                                  |
| `tui_shell.rs`                                                                                           | 启动 arming、每个候选使用不可变 pointer projection、成功才发布、一次后续 turn 对账重绘、失败 draw 使权威失效、capture 回调接线                                                    |
| `tui_render/settings.rs`、`action_menu.rs`、`agent_board.rs`、`agent_sidebar.rs`、`workspace_sidebar.rs` | Settings、已打开的动作菜单、Agent board 与可见侧边栏的精确类型化交互                                                                                                              |
| `tui_render.rs`、`help`、`navigator`、`tabs`、`contexts`、`presentation_selector`、`machine_picker`      | 精确 authored chrome 交互与来源凭据；pane 叶区域保留专用输入契约                                                                                                                  |
| `client_view.rs`、`client_view/bindings.rs`                                                              | `Ctrl+B m` → `TogglePointerCapture`，`SelectContextAt`/`FocusPanel`/`SelectTab` disposition，released 指示与指针 notice                                                           |
| `workbench_client/ui.rs`、`state/`                                                                       | `pointer_probe` 新鲜事实、`focus_panel`/`select_context_at` 准入、稳定 id 访问器（`tab_id`、`panel_id`、`location`）                                                              |
| `terminal_link_activation.rs`                                                                            | 精确来源围栏、OSC 8／纯文本解析、有序 handler 交接、单效果浏览器 fallback、有界通道与 join 关闭                                                                                   |
| `workbench_client.rs`（test 接缝）                                                                       | 由真实 projection 帧与 controller 状态支撑的单 pane `Legacy` probe——connected-PTY 见证后端                                                                                        |
| `terminal_input.rs`                                                                                      | `PointerProbe`/`PointerTab`/`PointerFrameFacts` 端口表面与 `forward_or_report` 投递                                                                                               |
| `invocation.rs`、`cli/`                                                                                  | `tui --mouse off\|on` 准入、help 文本与三种 shell 补全                                                                                                                            |
| `crates/terminal-protocol`                                                                               | `TerminalInteraction` 可信 Rust API，暴露已验证帧的 tracking/format/几何/cursor 模式事实                                                                                          |

## 验证

以下 Unix PTY 测试维护共享契约，不构成 Windows 运行证据。Windows cell 降级由编码器、
指针及 attach 回归，以及[终端流](windows-terminal-streams.zh-CN.md)记录的打包
control/attach 字节实测覆盖。

| 表面                    | 证据                                                                                                                                                                                                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 解码                    | `terminal_input/mouse/tests` SGR/X10 表——press/release/motion/wheel/修饰、跨 chunk 原子性、畸形拒绝且不退化为按键；`terminal_input/decoder/tests`——合法 SGR/X10 item、X10 分片喂入、超时 drain、ESC/C1 中止、畸形 body、越界运行、与按键/粘贴交错——所有鼠标形态畸形字节均为 `Ignored`            |
| 编码与归属              | `route_pane_pointer` 的 fresh 事件矩阵，加上 `encode_pane_report` 对每个 tracking × format 单元、已捕获清理、release 结尾、去重、夹取、legacy 上界与 SGR cell 降级的覆盖                                                                                                                         |
| Hit map                 | `tui_pointer/tests` 层序、边缘/外侧 cell、overlay 捕获与 backdrop 关闭——包括落在未遮盖 tab 条与 pane 上的外点 press 既不选择也不转发、stale-map 丢弃                                                                                                                                             |
| 交互 projection         | `tui_pointer/tests/interaction` 覆盖 hover 进入／重复／离开、disabled 与 specialized 目标、press/release、移出后永久取消、重绘连续性、fingerprint／遮挡变化、transport 丢失、子项优先级及一次有界静止位置后续重绘                                                                                |
| 通用接入                | Renderer/dispatcher 测试覆盖 Settings、菜单、侧边栏、Board、Help、Navigator、Machine Picker、Presentation、Tab 创建及全局 chrome：精确来源／几何、禁用消费、陈旧拒绝、hover 与已提交选择隔离、菜单候选更新、释放取消及禁止改投                                                                   |
| 手势与 pending focus    | `tui_pointer/tests` press/release 顺序、队列上界与溢出取消、超时、orphan、只读、目标移动拒绝、settled 无帧等待与按事实准入                                                                                                                                                                       |
| dispatcher 端口写       | `terminal_input/dispatcher/tests`——畸形 SGR/X10 与中止/超时鼠标运行产生零转发字节，而普通按键、粘贴与无关未知 CSI 按序透传；连续无键 motion 不能让到期 Escape 饥饿，且只有全部不变 motion 的 packet 才跳过重绘                                                                                   |
| 滚轮流                  | `tui_pointer/tests` 每 notch 三次、每次一屏上限、反转清零、排空后关闭                                                                                                                                                                                                                            |
| 捕获生命周期            | `terminal_lifecycle/tests` 租约配对与恢复；dispatcher `Ctrl+B m` 切换测试；状态指示                                                                                                                                                                                                              |
| 链接点击                | recording pointer port 覆盖精确修饰键、同 cell 释放、取消、既有手势 owner 与忙碌准入；共享 OSC 8 宽字符和折行 HTTP(S) 解析器保留其 CLI 集成 fixture；路由测试用 recording browser port 覆盖优先级、受理后不明、确定无 handler fallback 与单效果；受控 Windows 集成覆盖新鲜 viewer 和原生插件传输 |
| CLI                     | `invocation/tests` `tui --mouse` 值准入、裸调用/其他族拒绝、`--format` 拒绝；`cli/completion` 脚本                                                                                                                                                                                               |
| 受控 Windows Host       | `apps/terminal-host` 的 `test-conpty-orchbee-pointer` 通过固定 ConPTY/Ghostty 启动 Orchbee child，覆盖真实 inherited Reader、reply decoder、一次性模式准入、无能力回复的菜单 motion lease、返回按钮策略、语义 hover 区分及模式恢复；不证明物理指针／字体／配色行为                               |
| 真实 PTY（Legacy 接缝） | `tui_shell/tests/connected::real_tui_routes_pointer_reports_to_the_reporting_pane_and_restores_modes`——记录的 pane-local SGR 字节、pane-local 平移、只读丢弃、SGR cell 降级、滚轮转方向键、捕获切换、模式恢复                                                                                    |
| 真实 PTY（公开 Native） | `tests/local_startup_process/native_pointer.rs`——真实 PTY 上 spawn 的 `orchbee` 二进制：捕获 arm、X10 与 SGR pane-local 报告、未聚焦 pane 点击聚焦与排队放行、只读拒绝、滚轮转方向键、`Ctrl+B m`、`--mouse off`、退出恢复，以及零像素宿主几何下 cell 寻址报告仍可用、`?1016` 使用 SGR cell 降级  |

## 共享右键策略

[Panel 输入策略](native-panel-input.zh-CN.md) 在焦点或透传之前裁决新的右键按下。默认 `orchbee` 将其保留给[动作菜单](native-action-menus.zh-CN.md)；`panel` 仅允许启用鼠标上报的终端内容内无修饰键按下，并遵循现有控制权与就绪检查。排队右键在任何字节发出前复核策略。已准入手势捕获按钮与精确 Pane 实例，其他按钮不能注入按下或释放它，旧格式通用释放映射到归属按钮。改变策略作用于下一个手势，捕获/焦点/实例取消规则继续生效。打开和取消菜单保持焦点，并捕获匹配的 release。

## Presentation 选择器

会话就绪或已选择但未附着时，包含多个 Presentation 的 Panel 在无边框单面板的顶栏末端，或在分屏与放大视图中叶边线标题行末端展示 `[<] [position/count] [>]`。控件准入前为 Tab 或叶标题保留十二个单元。窄顶栏和窄标题省略选择器；标题保留标签／数量回退，无边框 Panel 的顶栏保留不可交互的位置计数，有效键盘路径在所有可呈现层级仍可用。每个箭头有三个单元的命中区域。数量和空格不增加自身的命中目标。顶栏选择器与 Tab、创建和操作入口互不重叠；标题行箭头在命中上优先于其下方的分隔线，且从不覆盖终端内容。无边框终端内容紧接顶栏下方。不增加标题行或主机鼠标模式。

无修饰左键按下为绘制时的精确目标 Presentation 建立手势，只有匹配释放才选择它。按钮按规范成员顺序循环，与当前有效的 Previous/Next Presentation 绑定等价（默认 `Ctrl+B (`／`Ctrl+B )`）。已绑定目标包含当前 `PaneRef` 和目的 Presentation ID。空目标包含结构 Pane key 与精确选中的 Presentation ID；新鲜探针必须仍确认该成员未绑定。空 Panel 内容区的按下只聚焦，不开始终端手势。新鲜探针必须仍在当前 desired anchor 下保留同一实例或空选择；UI 随后在活动 Tab 内解析 Panel／成员，才提交既有 Switch 事务。已删除／替换成员、旧 anchor、模态输入、捕获 overlay 和已释放鼠标捕获均不能改投手势。终端只读模式不禁止 Client 本地导航；选择从不转发终端字节，也不绕过子项仅 binding 附着。成功点击后旧命中图失效，直到重绘。

证据覆盖按钮边界、不响应的数量／空格、不重叠的分屏矩形、单面板／分屏／放大渲染、窄窗口及 Minimal 降级、修饰键／非左键报告、捕获／overlay／模态守卫、旧实例／anchor、成员移除及实际 UI 准入精确子项 Switch。组合 TestBackend 截图证明 cell composition；物理宿主配色、字体、鼠标硬件、延迟及多 Client 视觉行为需要各自的原生证据。
