# 原生键盘复制模式

## 范围与所有权

Windows 上，`Ctrl+B U` 为准确的焦点 pane 进入键盘复制模式。`[keys] copy_mode`
通过现有解析后的绑定注册表替换或禁用入口。大写入口不替换 `Ctrl+B [`、`Ctrl+B ]` 的上下文切换。

`NativeUi` 拥有易失的 episode、pane incarnation 和集成。确定性 `terminal_copy_mode` 核心
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
才采用可见应用光标；否则从左下单元格开始。此起点投影采用本地 Herdr 修订
`e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/client/shell/copy_mode.rs`；
这不是完整 Herdr 兼容性声明。移动归一化到完整的显示 glyph。

复制模式绘制所有被服务的内容行，不在 pane 内绘制普通历史 badge 和底栏；
外层状态区显示持续模式标识、提示和搜索草稿。在模式标识、搜索草稿和告警之外的剩余宽度内，
提示按实测显示宽度选取能完整容纳的最宽本地化动作集（完整集，再简集），否则只保留持续退出
提示；提示段从不绘制半截。光标、选区、读取坐标与 painter 使用相同内容几何。

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

编辑搜索时，包括 `q` 在内的可打印字符用于查询。Enter 提交。
草稿遵循现有行内编辑约定、整 grapheme 删除与原子 paste，限制为 512 个 Unicode scalar，
超限明确提示拒绝。查询含大写字符时区分大小写，否则不区分。
空搜索不读取内容；结果沿用 `terminal_copy::search` 的循环策略。

搜索命令按现有 canonical key identity 分类：原始回车和具名 Enter 都提交；Ctrl+H 和
Backspace 删除整个 grapheme。可打印输入保留原 Unicode scalar 和大小写。拒绝 paste 或编辑
时保留草稿和光标，在退出提示旁显示告警。重复 `n` 沿最后提交的 `/` 或 `?` 方向；
重复 `N` 始终沿其反方向，不改变记住的基础方向。

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

## 正确性证据

| Claim                                        | Owner 与 witness                                                                                               | 限制                                                                 |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| 独占输入、无 packet-tail 泄漏                | 模式绑定／dispatcher ownership tests；`KeyboardCopyMode` 输入与 tail actions                                   | 有界解码包；现有 decoder 契约                                        |
| 坐标与 selection update 使用接纳表面         | Copy core／NativeUi viewport traces；`KeyboardCopyMode` commit／move invariants                                | 几何、glyph、准确 frame tests；仍信任 Host pins                      |
| 旧 query／effect completion 不影响新 episode | Correlated core／adapter traces；`KeyboardCopyMode` query／delivery actions                                    | merge model 两个 episode、export 与 source version                   |
| 至多一次效果提交与匹配成功退出               | Effect queue／selection traces；`KeyboardCopyMode` attempt 与 success invariants；既有 selection／effect model | 既有 sink／OS 信任；无剪贴板读取或无界投递声明                       |
| 有界完整内容策略                             | Shared-reader、motion／search、query 与 overflow tests                                                         | 复用现有 Unicode policy；无新的 universal Lean claim                 |
| 键盘、输出、剪贴板与取消集成                 | 受控 Windows trace 与普通仓库 gate                                                                             | 自动 inherited-terminal／ConPTY 证据；真人 Windows Terminal 验收独立 |

确定性 tests 包括可重放且可缩减的生成状态序列、旧与重排 completion、两种表面、零历史、
mouse-tracking 程序、几何边界、clear／resize／eviction、显式重试与匹配成功。
渲染覆盖支持的尺寸 tier、双语、native 与 RGB theme。
没有新的文本转换声明无界正确性；有限 model 将文本抽象为 source identity，不证明 Unicode 语义。

模型保留准确的 query completion ID 和 pending/ready selection 握手；旧 query ID 不改变当前 query，复制接纳需等待 Host 端点确认。source tuple 抽象完整的 `ContentSource`：Host identity/screen/generation 表示 incarnation 和使旧状态失效的几何变化；revision 表示固定的 revision/columns/total-row 事实，committed surface 表示已接纳的 anchor/window 反馈。具体字段相等和重基行为仍由 adapter/core 证据承担。输入和外部结果观察使用一个可翻转的 witness bit，代替各自独立的单调历史计数；这允许任意重复观察，而不组合无关历史状态。退役清除待定 export intent。

运行 `node scripts/tla/run-models.mjs --models keyboard-copy-mode` 检查已登记的有界 copy owner。
既有受影响 model family 仍是 change-triggered 义务。
迭代时运行针对性原生 tests，合入前对冻结的最终输入运行完整 `pnpm verify`。

运行 `node scripts/tla/run-models.mjs --models keyboard-copy-mode-selection-barrier` 检查独立的
per-pane cleanup／admission 维度。模型覆盖旧 gesture flight、准确最后 cleanup 入队、FIFO
执行、迟到响应、pane 失效与退役，与 text／query／export episode 组合分开。其入队顺序不变量
映射到两个 pane incarnation 的真实 driver／mailbox FIFO trace，包括 Cancel 前保留的
CancelExport。接纳需要最后 cleanup 的 FIFO 入队，或终止该 boundary 的准确已知拒绝；
执行需要该 cleanup 结算，或该已知拒绝。Typed proof 状态区分已接纳、已结算与被拒尝试，
不保留已消费历史。不承诺清理成功或 transport fairness。`KeyboardCopyMode` 的
`SelectionSettled` 表示已确认的本地 gesture 意图，包括合法
Host None；`ExportFailure` 包括 Host 未启动 export 的 End。方向和完整 glyph 规则仍由确定性
core／adapter 证据承担，不形成无界文本声明。
