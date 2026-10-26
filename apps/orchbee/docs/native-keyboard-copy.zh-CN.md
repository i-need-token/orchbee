# 原生键盘复制模式

## 范围与所有权

导航、选择及搜索在所有平台共享。剪贴板交付要求宿主剪贴板能力：不支持的宿主隐藏复制
控件及提示；显式复制请求报告本地化的不支持结果并保留选择。参见
[TUI 平台边界](tui-platforms.zh-CN.md)。

`Ctrl+B U` 为准确的焦点 pane 进入键盘复制模式。`[keys] copy_mode`
通过现有解析后的绑定注册表替换或禁用入口。大写入口不替换 `Ctrl+B [`、`Ctrl+B ]` 的上下文切换。

`WorkbenchClientUi` 拥有易失的 episode、pane incarnation 和集成。确定性 `terminal_copy_mode` 核心
拥有光标、搜索草稿、选区和待定意图。`ClientView` 仅投影该 owner 是否活动。
`TerminalViewport` 保持历史所有权；Terminal Host 仍是选区端点与导出文本权威。
现有效果队列和剪贴板 writer 拥有系统提交。这些内容均不存入 checkpoint。

`terminal_content` 拥有独立只读连接上的准确内容来源与有界读取。链接接管和
`terminal_copy_query` 都消费它；URL／插件／浏览器路由仍由链接 owner 管理。
复制查询没有终端输入、控制者或变更能力。不新增依赖或公开协议。

## 入口与内容表面

| 表面                                     | 入口与导航                                                                                                   |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 主屏幕、实时                             | 复制专属入口允许已知非空 frame，即使零 scrollback。`Tail(0)` 进入历史，等待接纳的 `RowsApplied` 后使用坐标。 |
| 主屏幕、已提交历史                       | 复用准确的已提交窗口；不再提交历史入口。                                                                     |
| 备用屏幕、实时                           | 使用准确的当前 live frame，不发历史 demand。垂直／翻页与 `g`／`G` 留在当前屏幕；没有历史时不伪造行。         |
| 缺 frame、几何不可用或历史正在进入／恢复 | 用本地化提示拒绝；不发 pane 输入，也不从旧绘制推断位置。                                                     |

普通 PageUp／滚轮历史入口与到末尾退出仍由[原生终端历史视口](native-terminal-viewport.zh-CN.md)
管理。复制导航增加绝对 `Newest` 意图（`Tail(0)`），继续留在历史。
向下复制导航必要时使用该意图，不走普通到末尾退出路径。

只有准确屏幕、Host epoch、generation、revision、列数与接纳表面一致，且绝对行落入该表面时，
才采用可见应用光标；否则从左下单元格开始。移动归一化到完整的显示 glyph。

复制模式从完整接纳的行窗口或备用屏幕 frame 取得坐标，不依赖普通 live crop。本地显示为持续存在的
外层底栏预留空间；Minimal 布局还为首行鼠标工具栏留位。它裁剪已服务的行列，使实际光标字形完整
显示在底栏上方，包括宽字形的第二格。提交搜索及键盘或工具栏重复搜索时，匹配的已服务文本与光标
一起显露；不会用底栏单元格替代被遮挡的结果。此显示不改变物理终端尺寸、不移动已提交历史书签，
也不改变普通输入法坐标。Minimal 之外的布局由鼠标工具栏替换 Tab 条。pane 内不绘制普通历史
badge 和底栏；外层状态区显示持续模式标识、搜索草稿或告警，以及由
owner 排序的键盘提示。dispatch、底栏按键及其标签来自同一组局部绑定定义。底栏按顺序保留
能完整容纳的最长“按键加动作标签”前缀，并把当前返回路径放在首位；它不会截断一项，也不会
只画按键而缺少本地化动作。光标与选区绘制采用同一本地裁剪；指针命中携带其源原点，转换回原始
接纳坐标。读取及 Host 选区命令保留完整来源围栏。裁剪变化会撤销针对上次绘制矩形和原点的拖动。

## 鼠标控件

窗格动作菜单打开同一个复制轮次。工具栏的复制、搜索和帮助采用共用 action 样式；选择采用
稳定的布尔值 `[ ]` 选择／`[x]` 选择；上一个、下一个和关闭采用固定三格动作。搜索草稿显示
查找与返回，再显示静态方向值及有文字标签的反转动作。禁用控件保留原几何但不能激活。
控件只在同一已绘制令牌及矩形上匹配的普通左键释放时生效。键盘输入、失焦、释放捕获、
新内容或几何变化撤销待定点击。决策数据包及迟到释放不能变成窗格输入。

在已服务内容上普通左键按下，以归一化后的完整字形为选择锚点；已在选择时，点击和拖动扩展已有选区。清除结束已有选择后才能设置新锚点。拖动通过现有 Host 选择确认合并移动意图；释放保留选区，等待显式复制。原轮次以及精确的屏幕、generation、revision 和行窗口约束每个坐标。不增加独立剪贴板 writer 或终端输入路径。

已服务内容内滚轮每格累计三行，每次处理量受已绘制视口限制。反向滚动丢弃旧积压并保留第一个反向滚轮。来源／几何变化、键盘输入、查询或导出取消未处理工作。内容外滚轮无效。搜索、选择、读取和剪贴板失败保留原有 native owner 行为。

## 输入所有权与绑定

独占编辑器、确认、popup 或 machine 表面优先。随后复制模式在 prefix、board、普通历史、
配置直接绑定及终端透传之前拥有整个解码输入包。进入时取消原 pointer gesture、resize 模式、
帮助和已启用 prefix。模式不嵌套。含进入或退出动作的数据包仍被捕获；
尾随字节、release、未知序列及非法／超限 paste 均不进入 pane。pane 任务与输出轮询继续。

| 输入              | 意图                                       |
| ----------------- | ------------------------------------------ |
| 方向键、`h j k l` | 移动一个 glyph 或一行                      |
| `w b e`、`W B E`  | 用现有复制策略按词或空白分隔的大词移动     |
| `{ }`             | 用现有复制策略移到前／后段落               |
| PageUp/PageDown   | 移动一页视口                               |
| `g G`             | 最旧／最新可用内容；备用屏幕为当前首／末行 |
| `/ ?`             | 正向／反向字面搜索                         |
| `n N`             | 重复上次搜索方向或反方向                   |
| `v`、Space        | 开始或取消选区                             |
| `y`、Enter        | 显式请求选区复制                           |
| F1                | 显示来自该模式注册表的帮助                 |
| `q`、Esc          | 取消并回普通输入；Esc 也从搜索草稿退出     |

编辑搜索时，包括 `q` 在内的可打印字符用于查询。Enter 提交；Ctrl+G 返回复制导航而不退出
模式；Ctrl+R 反转草稿当前的搜索方向但不提交。
草稿遵循现有行内编辑约定、整 grapheme 删除与原子 paste，限制为 512 个 Unicode scalar，
超限明确提示拒绝。查询含大写字符时区分大小写，否则不区分。
空搜索不读取内容；结果沿用 `terminal_copy::search` 的循环策略。

搜索命令按现有 canonical key identity 分类：原始回车和具名 Enter 都提交，Ctrl+G 返回，
Ctrl+R 反转方向；Ctrl+H 和 Backspace 删除整个 grapheme。可打印输入保留原 Unicode scalar
和大小写。拒绝 paste 或编辑时保留草稿和光标，在退出提示旁显示告警。重复 `n` 沿最后提交
的 `/` 或 `?` 方向；重复 `N` 始终沿其反方向，不改变记住的基础方向。

query、export 或剪贴板 flight 期间，不兼容的新动作报告 Busy，不排队或重放。
viewport 或 selection update 待定时，光标移动表示为唯一最新 desired endpoint。
复制不能绑定旧端点：请求端点和 Host gesture 未结算时报告 Busy。Esc 始终可用。

## 状态与转换

核心使用有类型的 intent 与 effect；transport、时钟、线程、剪贴板与 UI 句柄留在 adapter。
episode／query／export identity 递增且不复用；耗尽拒绝新工作。
wrapper 保存准确 `PaneRef`、活动屏幕种类与内容 fence。

| 事件                                                                           | Guard 与结果                                                                                                                         |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| Enter                                                                          | 准确 pane 可读，安装一个 episode 与对应表面，或提示拒绝。                                                                            |
| 主屏幕窗口安装                                                                 | 绘制可更新，但 selection／query 坐标必须等待准确接纳的 `RowsApplied`。                                                               |
| 光标移动                                                                       | 校验整 glyph 目标；必要时导航，然后只在已提交目标表面更新选区。                                                                      |
| 仅 revision 的输出                                                             | 主屏幕有效 committed anchor 按原 local offset 重基光标；备用屏幕保留 local cell。使旧 query matches 失效，不把旧绝对行当永久身份。   |
| Query                                                                          | 捕获 episode／query、pane／member、screen、Host epoch、generation、revision、columns、total 与 committed surface；接纳一次有界读取。 |
| Query completion                                                               | 只采用匹配当前 token 与完整 source fence 的结果；drift／failure 提示并保留模式，不自动重试。                                         |
| Start selection                                                                | 取消旧 selection effects，然后使用语义 Host gesture adapter，不受程序 mouse tracking 限制。                                          |
| Copy                                                                           | 结束已结算 gesture 并导出，或在先前失败后显式重导出活动且已结算选区。                                                                |
| 有效 export                                                                    | 将完整已确认 UTF-8 文本及 pane／generation／export／episode identity 入队；拒绝成为匹配失败，不伪装等待成功。                        |
| Clipboard outcome                                                              | 只有匹配 `Delivered` 退出。Failed／Unknown 保留模式供稍后显式请求；program、pointer、旧 episode 与 cancelled 结果不使模式退出。      |
| Esc、pane／context change、reload、retirement                                  | 取消旧 local／query／selection 工作一次。主屏幕恢复 following-bottom；备用屏幕只清 local mode 与选区。                               |
| Screen switch、generation／geometry change、clear 或 endpoint／anchor eviction | 取消旧状态并提示，不猜测新坐标。                                                                                                     |

主屏幕 revision 刷新时，同一 incarnation 的准确窗口等待 `RowsApplied` 期间保留 episode。
此时状态显示待定，隐藏语义光标，坐标、query 和 export 按键被捕获而不积压工作；
搜索之外的 `q` 和 `Esc` 仍可退出。确认接纳后按新行重基光标；generation、screen、geometry 或 anchor
失效仍取消 episode。
搜索草稿仍允许本地编辑和有效粘贴；提交与其他终端效果一同等待。

主屏幕恢复前取消 Host 选区，保持普通 mailbox 的顺序。
取消能移除尚未派发效果，不能撤回已开始的系统调用。
每个仍有效且成功入队的 export token 至多提交到系统一次，永不自动重放。
Unknown 不证明成功；稍后的用户 `y`／Enter 是新的 export 和尝试。

每个 projection 的 selection driver 在最后一条准确 cleanup 命令前阻止 viewport 接纳。
进入时显示加载并独占输入，等待之前的 gesture 结算；接纳 Enter 或采用 committed surface
前重新核对 pane、attachment、Host epoch、screen、generation 与 geometry。仅 revision 的输出
可刷新仍有效的 source。退出立即清本地 episode 并暂存恢复，同一 per-pane fence 也阻止普通
history／input 路径提前接纳恢复。通常最后 cleanup 命令真正入队才释放 fence，FIFO worker
先执行它再执行 viewport 命令。准确 current prior／cleanup flight 明确被拒时，终止剩余
cleanup、报告失败，并将该 boundary 作为被拒尝试释放；待定 viewport intent 可继续，
不重放 cleanup，也不报告成功。模型将该拒绝回执与最后 cleanup 的接纳、结算分别表示。
其他 incarnation 的拒绝不能释放新 boundary。Generation 变化或成员移除丢弃旧工作；
另一 pane 独立；退役不接纳新命令。

Start 或 Update 确认在鼠标 gesture 阈值前可以合法返回无 Host selection。这只结算本地选择
意图，不关闭它。End 后无活动 Host selection 且未启动 export 时报告 NoSelection 并回 Ready，
不能等待从未开始的 export。所有本地提交端点，包括进入、同窗口 page／end 导航和 revision
重基，都对齐到完整显示 glyph。

Host Gesture Cancel 保留已经结算的选区。因此 copy episode 只在语义 Start 后拥有选区；
进入和退出期间未执行此 Start 时保留更早的 pointer 选区，本 episode 不能导出它。清 copy-owned 选区沿用现有 wire：
在接纳的非零几何内有效 cell 做单击 Start，以 None 替换选区，然后 Cancel 复位替换 gesture。
活动 export 在替换之前取消。整个两／三命令链都受 fence 限制，直到最后的 Cancel 入队。
缺失或过期 ownership 明确失败／失效，不推断清理成功；普通 pointer Cancel 语义不变。
Export 已从活动状态取走后，本地已排队的取消仍有明确归属。进入、退出、清选区或
generation 变化替换过期 queued gesture 时，保留该取消于 cleanup 链首并纳入入队 fence；
已经在 flight 中的 export 取消保持原有顺序。Barrier 模型的 cleanup plan 抽象这个保留义务，
包括单独的 CancelExport。

## 读取与生命周期上限

Motion 复用 CLI 完整行 collector：行端点读一行，段落读有界上下文，词移动从 64 行按方向扩展。
组装上下文最多 65,536 行；搜索读取此限制内的完整 retained buffer。
每次读取同样受行数限制，累计预算为 1,048,576 glyph 和 8 MiB row payload。
共用 reader 不声明额外累计行计数。query job 还校验现有 4096 字节搜索限制；行内草稿更小。

Buffer point 保留现有 `u32` 行表示。total 超过 `u32::MAX + 1`、route／point 不可表示、
identity 耗尽或预算超限都明确拒绝，不截断。query 保持一个准确 revision；
持续变化的内容可能使查询失败，明确提示，不拼接混合 revision 快照。

query lane 只接纳一个 job，拥有 wake stream 与线程；停止接纳、唤醒等待、join 并关闭独立连接。
临时 viewer 预分配 Detach identity，只尝试释放一次；只有确定释放与最终 canonical member fence
后才能发布成功。未知释放会丢弃独立连接，不能产生成功。
现有剪贴板 writer 保留自身 shutdown 契约，不声称任意 Win32 调用都在截止时间内完成。

标签、上下文或创建流程替换投影时，取消旧 episode 并 join 其 query lane，同时保留
Client 会话的重连来源。此后的键盘或工具栏搜索能重新打开精确读取。
只有 Client 会话退休才关闭该来源；移动光标或重试查询不需要修复连接配置。

## 正确性证据

所属实现测试覆盖输入包所有权、已接纳几何、拒绝过期查询/效果、释放前最后移动、效果顺序、最多一次提交和匹配的成功退出。

在 `src/terminal_copy_mode/tests.rs`、`src/workbench_client/terminal_selection/tests.rs`、
`src/workbench_client/ui/copy_mode/tests/mouse.rs`、`src/tui_render/copy_mode/tests.rs` 及相邻的
状态、carrier、输入和渲染套件中维护实际证据。鼠标可见性套件在 120×40、80×24、Compact 和
Minimal 尺寸下，以两种语言和原生／RGB 主题组合完整 shell。受控搜索完成检查主屏与备用屏幕的
可见匹配字形及底栏上方的光标，覆盖键盘／工具栏搜索与重复搜索，以及宽单元格选择／拖动的坐标
转换。这些本地裁剪检查不执行 query transport。流程具有多种结果时，使用受控完成／时钟顺序
及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。

<!-- documentation-pairing: english-sha256=e6f328f44c8d1585897e242b827904e9f2393bebf6c88d4331aee826289e3a3e zh-cn-sha256=c29181a28c317ce8be39fbee0c35677a23b8f4bb26c2563e4a49adbb556e2b06 -->
