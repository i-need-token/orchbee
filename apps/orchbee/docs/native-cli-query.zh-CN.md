# 原生 CLI 查询

## 规范语法边界

Panel 命令族使用位置 Panel 或 `--current`，拒绝旧 `--panel` 别名。Agent 目标额外接受 `--terminal` 与 `@terminal:`，精确定位子终端或主终端。启动、集成及 doctor 使用 `--provider antigravity`；`antigravity-cli` 保留为私有适配器身份。CLI 事件与过滤器输入及 v4 回执使用 `panel.*`；历史 v2/v3 Schema 保留 `pane.*`。Terminal 与 Agent 读取使用 `--source`／`--lines`；下方保留的旧 read/history 回执说明仅描述历史 Schema。来源等待默认 30000 毫秒，`--forever` 显式选择无限等待。

## 会话与后台服务

一个 **Orchbee 会话（Session）** 是包含 Workspace、Tab、Panel、终端和 Agent 的独立工作环境。
后台服务拥有共享状态和进程，CLI 与 TUI Client 连接到它。退出 Client 后，其中的程序继续运行。
首次使用默认会话，用 Workspace 组织日常工作；Windows 命名会话通过独立的运行时端点和
持久化状态，管理不同的终端程序集合。

`--session <name>` 选择该工作环境；`session list/attach/stop/delete` 管理命名会话，
`server start/diagnose/stop/config/reload-config` 管理所选会话的后台服务。
**Agent 会话** 标识一个 Agent 对话／运行时记录，通过 `--agent-session <id>` 或普通 Agent 目标选择。
协议中的 **Client Session** 标识一次已准入的连接，不代表另一套工作环境。

用 `orchbee server -h` 查看服务命令，用 `orchbee server stop -h` 查看需确认的关闭操作及恢复选项。
在 PowerShell 中，从可执行文件所在目录使用 `.\orchbee.exe server -h`；未加入 `PATH` 时，
也可通过完整路径调用。

| 旧拼写                     | 规范拼写                         |
| -------------------------- | -------------------------------- |
| `--site <name>`            | `--session <name>`               |
| `ORCHBEE_SITE`             | `ORCHBEE_SESSION`                |
| `site <command>`           | `server <command>`               |
| `agent ... --session <id>` | `agent ... --agent-session <id>` |

CLI 只接受规范拼写。`ORCHBEE_SITE` 保留为低于 `ORCHBEE_SESSION` 的旧环境变量兼容入口，
使现有配置继续指向原会话。现有运行时名称、scope 哈希、清单、检查点和 JSON schema 保持原样；
`siteId` 等机器字段以及 `orchbee.site.start` 等 schema 标识保留既有协议含义。
本次术语调整不会使 Electron Desktop 运行时成为原生会话服务的消费者。

## 独立 Agent 状态的 Schema 版本

当前输出将实际 Agent 状态与 Client 已读确认分开。Windows Agent List 和 Get 现使用合并 Agent 投影；内嵌目录版本为 148。其他平台保留当前规范 Agent 文档。后文可能介绍历史形状的引入；下表规定当前输出版本。

| 命令                                        | 版本                     | 适用变体                                       |
| ------------------------------------------- | ------------------------ | ---------------------------------------------- |
| `agent explain`                             | 4/5                      | 物理观测／清单诊断                             |
| `agent get`                                 | Windows 19；其他平台 1/2 | 统一投影／规范文档                             |
| `agent list`                                | Windows 2；其他平台 1    | 合并投影／规范文档                             |
| `agent wait; agent prompt`                  | 7/8/9/10                 | 规范／带 Client 的规范／终端／带 Client 的终端 |
| `agent view`                                | 2                        | 来源所有的过滤回执                             |
| `events stream; events wait`                | 3                        | 物理 Agent 事件词汇                            |
| `panel report-agent; panel report-metadata` | 3/4                      | 主项／显式成员                                 |
| `panel current`                             | 9                        | Windows 当前成员                               |
| `panel get; panel list`                     | 10/11                    | Linux／Windows                                 |
| `tab get; tab list`                         | 7/5                      | Get／List                                      |
| `workspace get; workspace list`             | 8                        | 规范 Agent 聚合                                |

## 当前 CLI 发现与平台契约

补全将本地 `--session` 选择与 Agent 会话选择器分开。Windows 上，get/read/prompt/send-keys/wait/focus 在目标前提供静态 `@agent:<kind>` 提供方别名；不会查询运行时状态，也不会编造序号候选项。在 PowerShell 中，提供方别名应从引号开始（例如 `'@agent:c`）；补全会插入完整且安全地用单引号括起的句柄，因为未加引号的 `@agent` 会被解析为 splatting 语法。Windows POSIX Shell 适配器使用与 PowerShell、Elvish 相同的静态语法筛选远端 `--machine` 路径，覆盖嵌套命令族和根级数据命令。输入已知选项与操作数后，仍可发现后缀 `--help`；不支持的路由、重复选项及已终止的选项不会重新获得候选项。

Snapshot v7 与 Tab Get v7 接纳规范的可选分隔比例 `ratio`（0.1–0.9）；省略表示 0.5。Snapshot v6 与 Tab Get v5/v6 保留原来的封闭树。Windows Agent Doctor v4 覆盖全部 20 个提供方；v1–v3 保留 14 行上限。

目标帮助明确 ID／选择器优先级：显示名称形似 ID 或已识别的 `@kind:` 选择器时，应使用资源的实际 ID 作为目标。创建和重命名保留字面显示名称。`panel resize --amount` 表示分隔比例的调整步长（默认 0.05，绝对值最大 0.5）。

Agent 启动帮助说明 `--provider antigravity` 对应集成／诊断的 `--provider antigravity`。启动目标 Panel 不存在时，提示明确指出目标错误并引导运行 `orchbee panel list`。本地发布清单读取失败时，提示检查 `ORCHBEE_UPDATE_MANIFEST` 中的路径和读取权限；机器原因仍为 manifest 阶段的 `download`。

Launch Profile list/get 现输出 v2，接纳 wire 契约中的 POSIX、驱动器绝对路径和 verbatim
驱动器可执行路径；v1 保留仅 POSIX 的模式。Windows 诊断输出 v3，覆盖默认与命名 Session
scope；v2 保留原 native scope。集成列表输出 v3，或在带 `--outdated-only` 时输出 v4，
边界覆盖全部 20 个提供方；v1/v2 保留原有边界。通用失败 v3 通过 `stage: plugin` 携带封闭的
本地插件准备原因；v1/v2 保留既有形状。Session 清单与启动环境失败包含对应运行时阶段，
不再缺少详情或误归类为 internal-failure。客户端选择诊断明确指向客户端，并说明适用的
`--client` 处理方式。这些投影不编造缺失的观察。

事件流 Schema 在 `$defs/filter` 暴露结构化筛选输入，插件 runtime 在 `$defs/context` 暴露
上下文输入。帮助包含有效 JSON 示例与 Schema 查找提示。Schema 描述结构输入；CLI 解码
还执行语义组合、Unicode scalar 解码和 UTF-8 字节边界。人类表格测得宽度超过 80 个单元格
时使用带标签的记录。字段块始终分隔标签和值，保留完整 ID 供复制；文本布局不改变 JSON 投影。

## Workspace 与可选 Project

Project 列表遵循 Navigation 独立的 `projectOrder`，省略或空值时使用规范 ID 顺序。ID／名称
查找始终独立于显示位置；Workspace 列表继续使用其独立的 `workspaceOrder`。

Navigation 的 `directories` 和 `workspaces` 为全局数组；`projects` 只含 ID/名称。Workspace 的 `projectId` 必填可空。Project list/get v2 由成员 Workspace 派生目录；Workspace list/get v7 全局解析并明确显示未分组。`api snapshot` v6、Worktree query/list v2、Workspace close-status v2 和目录 v143 发布这些形状。语义事件 stream/wait 封装为 v2：Workspace 数据仅在嵌套 Workspace 上携带分组，Worktree 操作元数据可空。

`workspace set-project <workspace> --project <project>` 移动分组；`workspace create-project <workspace> --label <label>` 原子创建 Project 并移入；`workspace ungroup <workspace>` 取消分组；`project dissolve <project>` 取消全部成员分组并删除 Project。它们只改变组织元数据，保留精确 ID、cwd、PTY、目录和顺序。`workspace move`/`move-block` 继续单独重排。Project 可组织多个仓库。普通关闭只影响选中 Workspace；批量关闭使用下文显式 Project 命令。

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` 在一个输入请求中粘贴命令，再发送 Enter。命令须为一个引号字符串；保持原始文本，目标只接受完整 ID 或 `@terminal:<suffix>`。回执确认输入提交，不保证 Shell 已就绪、命令完成或退出状态；部分或未知结果不得重放。 [命令提交契约](native-terminal-run.zh-CN.md).

状态：原生 `orchbee` 命令行的当前实现与正确性契约，覆盖查询、Workbench 修改、
终端观察与输入、Agent 名称及本地 provider 接入。查询观察已有 Session；Workbench 修改
遵循下文 mutation 契约。只有显式的 `server start` 和 TUI 启动路径可以激活 Session。
Agent 状态来自 Server 的 canonical Agent 模型，不从终端文字推断；名称由 Server
管理，`agent integration` 只管理调用用户的本地 provider 配置。每个命令的效果、
预算和资源释放边界见相应章节。命令无需 TTY；交互式 TUI 有独立的终端准入要求。
旧 Desktop TypeScript CLI 已退役。

归属：本记录拥有 `src/invocation.rs` 与 `src/cli/**` 实现的公共语法、结果信封、失败矩阵、
预算、交换状态机与输出副作用。文字与实现必须在同一变更中收敛。

## 清空终端输出

Windows `terminal clear <terminal>` 在已有 Session 中清空一个精确绑定的主终端或子终端。
保留光标所在的完整软折行提示行，删除此前历史及其他输出，清除选区并将视口返回活动屏幕。
备用屏幕会被拒绝，内容不变。终端模式、标题、工作目录和子进程保持不变。
该行为遵循固定版本 Herdr 的 `pane.clear` 实现，不通过注入终端输入模拟。

公开终端控制动作 `clear` 指定一个已准入的只读 viewer。Server 核对当前 Session 暴露、
精确物理绑定和 Worker 代次，然后分配独立的临时输入 attachment 标识。
Worker 使用该 viewer 的精确 Host 目标，附加仅输入权限且不申请尺寸租约，
提交一次带 `preservePrompt: true` 的 Host `ClearTerminal`，验证 attachment/terminal
以及正数代次／修订确认，再分离。已有控制权和终端尺寸不变。
省略 `preservePrompt` 时，旧 Host 清屏操作保留默认行为。

CLI 发现和提交共用一个可取消的五秒预算。Host 操作使用三秒效果预算和独立的一秒清理预算。
提交前失败不会清空内容；提交后确认丢失或畸形则结果未知，绝不重放。
已确认清空但清理未确认时保留已知效果并退出 8。失败 Host 通道会关闭，
由连接释放临时 attachment，避免累积状态不明的 attachment。
CLI 在最后一个消费者结束后于所有结果路径释放 viewer。
取消保留已提交回执并退出 130。清空不启动 Session、不聚焦 Client、不接管控制权、
不发送 PTY 输入、不重置进程，也不触发浏览器效果。

封闭的 `orchbee.terminal.clear` v1 携带精确目标、操作标识和类型化结果；
已知清空包括新内容修订／代次以及清理确认。成功退出 0、类型化拒绝 3、不可用 4、效果未知 8。
已有控制编解码器拥有新增的清空请求／结果；Host 协议对等端拥有可选的保留提示行字段。
定向契约、Host 运行时和 Windows 管道测试验证这些边界；部署后 Host/TUI 验收和全仓验证
仍按用户持续授权留至最后阶段。

操作标识是请求关联标识，不是持久保留的幂等回执；不提供状态查询或自动重试。

## 读取终端选区

Windows `terminal selection-read <terminal> --anchor ROW:COL --cursor ROW:COL`
从已有终端读取包含两个端点的非矩形范围。坐标以零开始，行指完整保留屏幕
缓冲区的物理行，与 Client 视口无关；行是无符号 32 位、列是无符号 16 位整数。
反向端点先排序。选区最多跨 4096 行，使用现有单次 8 MiB 行窗口预算。
无效坐标、重复或其他命令选项、过大跨度均在发现 Session 前拒绝。

可选 `--content-revision <u64>` 固定预期内容修订；
`--generation <positive-u64>` 额外固定其几何／存储 generation。
省略时捕获新帧。所有行读取仍固定同一帧的修订、generation、attachment、
Host epoch、尺寸及保留行总数。过期或已回收数据整体失败，不重试或输出部分文本。
超出帧的行／列会拒绝，不会改选邻近单元格。

命令持有一次可取消的五秒既有 Session 总预算和一个临时只读 viewer，最后消费者
结束后在所有结果路径释放。不取得控制者、不改变选区／视口、不发送输入、
不写剪贴板、不确认用户已查看、不启动 Session，也不导航 Client。
单元格字节映射保留完整宽字和组合字形；硬行尾填充被移除，相邻且经验证的
软换行直接连接。结果包含原始端点、精确终端 binding 与捕获的内容修订／
generation／Host epoch。`orchbee.terminal.selection` v1 为封闭 JSON 回执；
文本经现有 CLI 展示所有者转义控制字符。已有 viewer／read-history 类型失败
保留 stale-revision、stale-generation 和 history-evicted 原因。取消退出 130；
不重放，也不引入持久状态。

证据覆盖反向／单单元格／空白选区、宽字／组合字映射、软／硬换行、尺寸／跨度
边界、解析／帮助／补全／schema admission，以及实际 Windows CLI 管道中的成功
读取、过期预期内容、错误行路由、Host 类型拒绝及确定的 viewer 释放。
外部 scrollback 编辑及最终 Host／模拟器验收仍为独立义务；复制移动／搜索见下节。

## 解析终端链接

Windows `terminal link-resolve <terminal> --cursor ROW:COL` 解析显式原生视口
中的单元格，不打开浏览器，也不调用插件。视口坐标为零起点的无符号 16 位；
可选 `--offset-from-bottom <u64>` 默认零，视口首个物理行为捕获的 scrollback
行数减去 offset。Offset 超出保留历史或光标超出捕获尺寸均失败。这个 CLI
拥有的视口独立于 TUI Client 的焦点和滚动，不改变任何 Client 视口。精确绑定
的主／子终端目标、可选完整 u64 `--content-revision` 与正 u64 `--generation`
沿用选区／复制的既有 Session 只读五秒 viewer 和完整帧／Host／绑定围栏。

解析器先严格读取一行 render，检查被点击完整字形的 OSC 8 目标，包括宽字尾。
显式超链接返回完整 URI 与空 regions，遵循参考实现将帧拥有的 OSC 8 高亮和
服务端解析的纯文本区域分开的规则。没有 OSC 8 时，只按需读取完整映射文本行，
以证明跨已验证软换行的空白分隔 token，包括请求视口之外的 token 端点。
缺少邻接上下文时有限扩展；硬行边界与 Unicode 空白结束 token，已验证宽字头
占位不增加文本。纯文本识别遵循固定 Herdr：小写 http／https 前缀、裁剪尾部
引号／标点、保留平衡的圆／方／花括号，点击最终 URL 之外返回无链接。
参考实现的共享 8192 单元格探查预算计算选中的物理格、宽字头占位／尾格及双向
边界探查；预算耗尽返回无链接，不返回截断 URL。原生编码字节／
字形／65536 扫描行限制仍明确失败，不返回未证明的空结果。可见的包含端点区域
保留完整宽字／组合字，并裁剪到请求视口。

封闭 `orchbee.terminal.link-resolve` v1 包含完整 URL 或 null、
`plain-text`／`explicit`／`none` 类型、可见区域、视口起点／offset 与捕获的
内容修订／generation／Host／终端绑定。空结果是参考实现有界 token 策略下的
成功无链接结果，包括其探查预算耗尽的情况。
错误／重复／外来参数在运行时发现前拒绝；过期／回收／误路由／不完整记录整体
失败，不重试，也不返回部分区域。本操作不取得 controller、不发输入、不确认
用户 attention、不改选区／剪贴板／焦点、不启动 Session、不写持久状态。
Schema／帮助／限定范围 Windows 补全与机器只读转发共用类型化请求。TUI 悬停／
点击接线及物理 Host／模拟器验收是另外明确界定的消费路径。

## 激活终端链接

Windows `terminal link-activate <terminal> --cursor ROW:COL` 接受与 resolve 相同的
offset、修订和 generation 参数及显式原生视口。先解析完整点击 URI／URL，再选择
已启用、可用的 Windows 插件处理器。插件 ID 按字典序，处理器保留 manifest 顺序；
处理器与引用动作都必须支持 Windows。非法正则处理器不可用；匹配使用已有的
线性时间正则引擎，编译后 pattern 至多 1 MiB，处理器之间检查期限／取消。
没有链接或匹配处理器返回 `handled=false`，不调用插件。

匹配后收集精确点击的主／子终端显示上下文，包括规范 Panel／Tab／Workspace
及已证实的 cwd／Agent／worktree 信息。无关 caller 环境或 Client 焦点不选择此
来源；未知事实保持缺省。工作流固定 `invocationSource=link_click`、完整点击 URL、
handler ID 和操作关联。适用已有插件上下文上限：URL 4096 UTF-8 字节、合计
context 16 KiB、request 32 KiB；匹配 URL 无法准入时在提交前失败，不截断。
收集上下文后严格重读相同修订的 render 行以保护点击内容，并在单次现有插件调用
前复核选中的 catalog entry 未改变。这些是提交前观察，不承诺 catalog／内容原子
事务或提交后的内容锁。

Server 独自解析当前动作可执行文件、插件根 cwd 与保护环境；已有 Worker Job
拥有启动、输出日志和取消。显示数据不能提供 argv、目录策略或 Worker 权限。
本命令不打开浏览器、不发终端输入、不导航／聚焦、不改剪贴板、不启动 Session。
一个既有 viewer 与全部预检／提交共用五秒和取消。匹配的效果路径在最终内容／成员
围栏后、提交插件前，必须取得一次相关联的 Detach 确认；Detach 被拒或结果不明时不
提交插件，也不重试。只读及无效果结果保留尽力 Detach 与会话 EOF 清理。已准入的
插件 Job 由现有 Job 所有者管理，可在临时 viewer 结束后继续。

封闭 `orchbee.terminal.link-activate` v1 保留已解析链接、视口、内容／绑定回执，
并增加激活状态和操作身份。`handled=true` 必须有相关联且已知的 Started 回答，
表示 Job 准入，不代表命令成功。已知拒绝退出 3；提交后回答丢失或未知／错误结果
退出 8，保留操作身份且不重试；取消退出 130。没有处理器是成功无操作。
已有插件日志可查看已准入 Job；原只读回执不变。双语帮助／schema／限定补全与
机器转发共用类型化命令。定向证据覆盖顺序／平台匹配、上下文限制、精确目标
上下文、提交前过期、启动／拒绝／丢失与实际 Windows 管道 viewer 释放；
部署后的 Host／TUI 验收仍保留。

## 终端复制移动与搜索

Windows `terminal copy-motion <terminal> --cursor ROW:COL --motion <motion>`
计算新的屏幕缓冲区光标，不改变 Client 选区也不发送输入。移动名为
`line-end`、`first-non-blank`、`next-word-start`、`previous-word-start`、
`next-word-end`、`next-big-word-start`、`previous-big-word-start`、
`next-big-word-end`、`previous-paragraph`、`next-paragraph`。
两类词均跨越已验证软换行，硬行插入空白分隔。普通词区分空白、tmux ASCII
标点（下划线除外）及其他字符；大词仅按空白分隔。结果保留完整字形头部，
宽字尾与组合字均不拆开。行末是裁剪硬行尾填充后的最后字符；首非空白是首个
非空白字形。依冻结参照，Kitty Unicode placeholder 单元格在复制移动／搜索中视为空白。段落移动找指定方向最近的空白物理行，保留列，依冻结参照最多
检查 999 行。没有目标则返回原光标；缓冲区外坐标失败。

Windows `terminal copy-search <terminal> --cursor ROW:COL --query <literal>`
`--search-direction forward|backward --content-revision <u64>` 搜索完整保留逻辑文本。
查询原样保留，最多 4096 UTF-8 字节，按字面量与 Unicode 智能大小写匹配：
任意大写字符使其区分大小写。空查询在内容围栏通过后返回零匹配。
可选 `--previous ROW:COL-ROW:COL` 是排序后的包含端点前次匹配；向前起点是
其末端，向后是其始端；省略时使用传入光标。下一项必须起于向前起点之后，
或终于向后起点之前；没有则环绕到首项／末项。匹配边界必须对应完整字形。
硬逻辑行末裁剪填充；已验证宽字 spacer head 不增加文本或词边界。
结果保留精确总数及全局当前位置，并返回最多 1024 项、以当前匹配居中的窗口
及其局部索引。零匹配的当前索引为 null；u64 索引／计数使用十进制字符串。

两命令复用 selection-read 的既有 Session、五秒取消、精确终端／帧修订／generation／
Host／几何及 viewer 释放所有者。移动可选 `--content-revision`，搜索必须指定；
二者另接受 `--generation <positive-u64>`。每次行请求有递增独立序号，使用既有
逐窗口字节／单元格限制；相邻窗口的软换行连续标记必须一致。
完整准入扫描完成后才输出：最多 65536 行、累计 8 MiB 编码行窗口／逻辑文本及 1048576
个保留字形。超限或过期／回收／错误路由整体失败，不输出部分匹配、不重试，
也不捏造无操作成功。行移动只读一行；段落与词移动先读 64 行，按需扩展：段落在最近空白行或 999 行距离处停止；
只有词边界得到证实才返回，向前继续未结束的词末，向后继续软换行延续的词首。
相邻批次保留同一帧及全局字节／字形预算。搜索覆盖完整保留缓冲区。

`orchbee.terminal.copy-motion` 和 `orchbee.terminal.copy-search` v1 为独立封闭回执，
保留原始请求、精确终端绑定与内容身份。默认文本本地化标签、转义查询／控制字符。
确定性的 `terminal_copy` 域拥有单元格／词／逻辑行决策；只有 CLI 读取适配器拥有
发现、交换、期限与清理。证据覆盖全部十种移动、Unicode／完整字形、行／窗口边界、
字面量／智能大小写／空搜索、前次起点、环绕、超过 1024 项的精确总数、效果前参数
限制及实际 Windows CLI 成功／过期／错误回包／释放轨迹；最终部署 Host／模拟器验收独立保留。

## Workbench 状态

裸 `status` 读取一份 Workbench 快照并报告可达性和资源计数。只有显式提供 `--session` 时，
Text 才显示所选 Session 名称。当前 Client 的包版本始终显示。Windows `status server` 报告 Server 的实际构建元数据。

## Workbench 拓扑查询

Windows `workspace list/get` 返回 `number`、`tabCount` 和 `panelCount`；
`tab list/get` 返回 `number` 和 `panelCount`。Workspace 编号是完整规范导航顺序中
从 1 开始的位置，在 `--project` 筛选之前确定。Tab 编号是所属终端上下文中
从 1 开始的位置，与手动或自动标签无关。重排改变编号，但不改变身份。
空上下文的 Tab 和 Panel 数量均为零。Panel 是 Tab 分割树中的一个叶节点；
子 presentation 和终端绑定都不会增加 Panel 数量。

人类文本中的 `project list`、`workspace list` 与 `tab list` 输出 `HANDLE` 列：精确身份的稳定 `@<kind>:0x<hex>` 形式，仅省略前导零。
筛选以及其他成员的创建／删除不改变句柄。精确身份被删除后返回 not-found；
紧凑身份保留所有有效数字，最多 32 位。`workspace list` 使用本地化的 `HANDLE`（句柄）、`NAME`（名称）、
`PROJECT`（项目）列；`tab list` 使用 `HANDLE`（句柄）、`NAME`（名称）、`PANELS`（面板）、
`WORKSPACE`（工作区）列。目标歧义时，文本诊断以这些句柄加带引号的标签列出至多八个候选
及真实总数；JSON 保留标识符形式的 `candidates` 与 `totalCount` 详情。项目和工作区所属列显示规范名称；独立 Tab
显示本地化的独立上下文名称。名称单元格带引号、转义控制字符并限制宽度，使每张表
不超过 80 个终端单元。这些列表不会把 Workspace 主目录当作进程 CWD，也不会用内部
上下文 ID 充当名称。

`panel list` 包含 `workspaceId`（独立上下文为 null），以及主 presentation 的
`label`、可空 `customLabel` 和 `displayLabel`。这些值与 `panel get` 返回的
规范值相同，并非推断的进程或 Agent 标题。JSON 原样保留标签；文本输出转义标签，
并限制列表中的显示宽度。

这些投影消费单个已验证 Workbench 快照，保留其来源修订号、仅连接已有 Session 的准入、
有界输出和无 TUI 时的可用性。不执行导航或变更；运行时观察见下文。

人类文本中的 `workspace list` 和 `tab list` 只呈现快照 Text 表格，不执行可选的
`Agent`、`worktree`、`metadata` 或 `viewport` 关联。指定 `--client <id>` 时，Text 仍会核验
该精确就绪 Client 的当前选择及成员关系，但不增加文本列。下文所述关联和最终一致性检查仍适用于
这两个列表的 JSON，以及其他既有使用这些关联的查询。

在 JSON 中，Windows 在这六个 list/get 查询中接受可选 `--client <id>`，用于补充全局焦点
和 Client 在各 Workspace 记住的活动 Tab。
[Client 查询契约](windows-client-control.zh-CN.md#client-相对-workbench-查询)定义这一有界只读关联、
来源记录与一致性检查。Schema 版本为 `orchbee.workspace.list/get` 6、`orchbee.tab.list` 4、
`orchbee.tab.get` 5 和 `orchbee.panel.list/get` 9；使用 `api schema` 发现完整内嵌目录。
保留的选择读取控制使用 `orchbee.client.control` v2，视口读取使用 v3，其他控制保留 v1。
`orchbee.cli.failure.client-query` v1 标识指定 Client 的读取失败，既有目标和准入诊断保持完整。
进程和 worktree 观察使用下文所述的各自所有者；上报元数据仍由独立所有者负责，拓扑字段不代表已实现它。

验证覆盖全局重排与筛选后不重新编号、空树与嵌套树、子 presentation 和未绑定叶节点、
列表/详情一致、自动标签、文本转义，以及真实 Windows 查询文档的 schema 校验。`WorkbenchQuery`
组合见证检查两种语言下只含表格的 Text 输出，并确认 JSON 路径不会进入该快捷分支。

## Workbench Agent 汇总

Windows Workspace/Tab/Panel 的 list/get 及 Panel current 查询新增根级 `agents` 观察，包含规范 Agent
修订号、List 请求 ID 和匹配的活动会话摘要。各行包含 `agentSessionIds` 和可空
`agentStatus`。仅可导航的主 Presentation 绑定参与，子 presentation 和已结束会话不参与。
一个 Session 即使有多个匹配绑定，也只在每行计算一次；多个匹配 Session 均保留。
只有完整且成功的规范列表才能证明绑定为空，不能把读取失败或截断当作没有 Agent。

每页最多100个摘要，组合查询最多接受512个活动 Session。所有页必须修订号相同且 ID
不重复；重复游标、带后续页的空页、超限和修订号漂移均失败，不重放、不输出部分结果。
CLI 复用单个已准入连接、信号所有者和五秒总期限，输出前验证原 Workbench 修订号。
Agent 观察与 Workbench 快照保留各自来源，不声称分布式原子快照。不扫描进程或推测文本，
不产生启动、输入、导航和已读确认效果。

指定 `--client` 时，每个 attention 批次最多100个匹配 Session，复用既有
[经认证的 Client 关联契约](native-client-attention.zh-CN.md#合并规范-agent-读取)。
存在匹配 Session 时，后续完整 Agent 列表用于输出，通过事件连续性、generation 和 sequence 核实各 Client
投影。新出现的 Session 明确标为 missing，不假定已看过。最初没有匹配时，以首次完整列表作为观察。Workbench 和 Client 视图
修订号必须保持不变。根级结果保留规范摘要以及可选的逐 Session Client 投影。

聚合优先级沿用规范的注意／错误／结果／运行／空闲／未知顺序：blocked、error、done、working、idle、unknown。Idle 保持 idle，succeeded 映射为 done，failed/canceled/interrupted 映射为 error，与 Client 已读确认无关。stale/disconnected 观察贡献 unknown，并在会话事实中保留明确状态。没有匹配 Session 时为 unknown。未指定 Client 时不伪造 seen。文本本地化聚合状态并转义显示数据，JSON 不随语言变化。

`orchbee.cli.failure.workbench-query` v2 使用 `stage: workbench-query` 和封闭的 reason
表达有界组合失败：agent-capacity、agent-page-invalid、agent-revision-changed、
workbench-changed、client-changed 或 metadata-changed。既有 Agent 和准入错误保留原有 schema 与诊断，
指定 Client 的路由错误沿用 Client 查询契约。验证覆盖优先级与未确定状态、主/子成员关系、
重复/混合页、过期确认、schema/文本、无界面查询及真实 Windows 双 Client 观察。
Panel 进程/标题字段由下述终端观察所有者提供；Client 滚动和 Workspace worktree 观察
由下文各自所有者提供。Workspace token 由下述独立元数据所有者提供。

## Workspace 元数据上报与查询

Windows `workspace report-metadata <workspace> --source <id> --token <key=value>`
提交一次规范化补丁。重复的 `--token` / `--clear-token` 按参数顺序生效；可选 `--seq`
约束各来源的新鲜度，`--ttl-ms` 使本次设置的值过期。[Workspace 元数据契约](windows-workspace-metadata.zh-CN.md)
负责边界、旧序号无操作、过期、未知结果回执及独立 Server 修订。Workspace list/get v7
包含 `tokens` 和根级 `metadataSource`，在原五秒总期限内分批读取并最终复核修订。
读取不会激活 Session 或执行导航。

## Workspace worktree 观察

Windows Workspace list/get 将每个权威主 Directory 交给既有的
[Windows Worktree 只读查询](windows-worktrees.zh-CN.md#按来源发现-worktree)。
返回可空 `worktree` 和必需的 `worktreeObservation`。已观察到的汇总包含
`workerId`、`repositoryRoot`、`commonDirectory`、`repositoryName`、`checkoutPath` 和
`linked`；Worker ID 与 Git 公共目录共同标识仓库分组。名称取自观察到的仓库根目录的
最后一个非空路径部分，不使用 Workspace 标签或调用 CLI 的目录。

| 输入或结果                                               | 发布的观察                                           | 效果                           |
| -------------------------------------------------------- | ---------------------------------------------------- | ------------------------------ |
| 没有主 Directory                                         | `no-directory`，worktree 为 null                     | 不请求 Git                     |
| 有效的来源发现                                           | `observed`，精确查询来源和汇总                       | Worker 只读 Git 查询           |
| Git／Worker 不可用、不支持、启动／I/O／大小失败          | `unavailable`，请求 ID、类型化原因，worktree 为 null | 不回退、不重放、不覆盖信任策略 |
| 拒绝访问、来源缺失／改变、无效结果、传输丢失、超时或取消 | 整条命令失败，不返回部分成功                         | 既有查询清理                   |

每次请求指定精确 Workspace 和 `trustRepository: false`。成功应答必须与原始 Session、
Workbench 修订、主 Directory 路径及 Worker 一致，且来源检出项必须包含该 Workspace。
全部行、Agent／Client 关联及最终权威／Client 一致性检查共用一个已准入连接、信号所有者
和原有五秒总期限。查询不激活 Session、不改变关联、不导航、不启动终端、不写 Git 配置。
Git 清理仍归既有 Worker 读取任务所有。外部文件系统变化不受栅栏约束：这些是依次进行的
实时观察，不是原子 Git 快照或 Herdr 的缓存关联。`unavailable` 绝不表示“不是仓库”。

Workspace list/get 使用 schema 版本 6；目录版本 97 仍包含 91 份文档。其他查询版本
保留既有契约。验证覆盖主／链接检出、检出子目录中的 Workspace、目录缺失、Git 不可用、
来源事实不匹配、总期限、安全文本，以及拓扑和 Client 视图不变的真实 Windows 读取。

## Panel 终端观察

Windows `panel list/get/current` 还返回可空的 `cwd`、`cwdSource`、`foregroundCwd`、
`terminalTitle`、`terminalTitleStripped`，以及必需的 `terminalObservation`。
观察状态为 `unbound`、带 `reason: terminal-unavailable` 的 `unavailable`，或带有效帧和
已观察/不可用进程样本的 `observed`。终端停止不会让 Panel 从列表消失。只有有类型的
终端/进程服务不可用才转为不可用数据；协议、权限、传输、取消和期限错误令查询失败，
不输出部分结果。

每个主终端的单个只读 viewer 提供两类观察。帧事实保留 Host epoch、attachment、sequence、
generation、revision、几何、屏幕、scrollback 数量、原始标题及上报目录。在保留摘要前释放
完整单元格数据。进程样本复用 Windows Job 读取及其明确的 root/Agent-chain/ambiguous
选择策略，不声称 OS 前台权威。已观察样本保留独立来源及标准 `panel process-info` 结果。
进程采样不可用时仍保留已成功观察的帧元数据。

`cwd` 优先采用从帧上报目录解码的绝对 Windows 路径，回退到采样的根进程目录。
file URI 按严格 UTF-8 解码一次百分号转义（盘符路径或 UNC authority），不接受 query、fragment、
控制字符，不访问文件系统；非法或非 Windows 上报仅原样保留在帧证据中，不作为文件系统目录。
`cwdSource` 为 `terminal-frame`、
`root-process` 或 null。`foregroundCwd` 来自公开进程策略选中的进程，不可用或歧义时为 null。
不代入调用 CLI 的目录或猜测的启动目录。原始标题保持不变；简化标题移除精确的 Windows
`Administrator: ` 前缀、两端空白，以及至多一个用空白分隔的前导 Braille 或已知活动字符
（`·✢✳✶✻✽◐◓◑◒`），空结果为 null。文本输出转义全部终端/进程控制的字符串。

既有五秒总期限覆盖所有 Panel 观察、Agent 关联和最终规范/Client 检查。所有结果路径都
释放 viewer，准入会话销毁作为兜底。不产生输入、resize、焦点、history-demand 或 Client 确认效果。
独立的帧/进程观察不是原子快照。新 viewer 的实时帧不代表所选 Client 的滚动偏移，后者
由下述单独的 Client viewport 观察提供。Windows Panel list/get schema 为版本9，目录为版本97，共91份。

## Panel Client 滚动查询

Windows Panel list/get/current 返回可空的 `scroll` 及必需的 `scrollObservation`。
未选定 Client 时观察为 `no-client`，新 Host viewer 无法确定用户滚动位置。
指定精确 Client 后，有界只读视口批次返回其已安装的实时/历史指标，或明确的不可用/待处理
状态，见 [Client 视口观察](windows-client-control.zh-CN.md#client-视口观察)。
根 Client 来源保留各操作 ID，发布前检查请求的 Panel ID、主绑定、规范修订号及所选视图一致。

Panel 读取接纳当前 Ready 或 Busy Client，包括历史浏览。直接对规范快照验证选择成员，
不请求无关的活动 Tab 记忆；Workspace/Tab 查询保留原有准入限制。
全部观察共用原有已准入连接和五秒总期限，随后执行最终规范及 Client 修订检查。
查询不改变视口、焦点、选区、输入租约或已读确认。独立的 Host/进程与 Client 观察不是
原子快照。Windows Panel list/get 使用 schema 8，保留的视口控制使用 `orchbee.client.control` 3，
catalog 88 包含 91 份 schema。

## Windows 命名 Session 选择

全局 `--session <name>` / `--session=<name>` 为整个调用选择本地 Windows Session，包括
`server start`、`server stop`、查询和 `tui`，不改变 Agent `--agent-session` 选择器。
名称区分大小写，包含 1–64 个 ASCII 字母、数字、`.` / `_` / `-`，不能是 `.` 或 `..`。
`default` 选择现有默认 Session。先验证完整参数语法，再应用选择；离线 help、completion、
version 和 schema 操作不会因为提供名称而发现运行时。

选择优先级为显式 `--session`、托管终端的 `ORCHBEE_RUNTIME_SCOPE`、`ORCHBEE_SESSION`、旧的 `ORCHBEE_SITE`，最后
为 `default`。`ORCHBEE_RUNTIME_BASE` 仍选择已存在的绝对基目录。Worker 将准入后的基目录和
scope 传给子终端，使内部 CLI 命令保持所属 Session，即使 Server 继承了其他名称。
显式 `--session default` 覆盖继承的 scope。选中的环境变量值无效时，发现失败，不静默回退默认 Session。

命名运行时叶目录由当前 SID 和精确名称散列得到，名称不作为文件系统路径组件。
激活在现有 activation lock 下注册有界私有身份，再提交启动标记；只读发现验证该身份且不创建它。
检查点存储已按准入后的运行时路径隔离。
`session list [--format text|json]` 列出所选运行时基目录中的 default，再按精确名称排序列出命名会话。
它在一个五秒总期限内读取最多 256 个会话，不创建运行时状态，根据认证元数据及准入后的生命周期事实
报告 running、stopping、stopped、absent、starting、deleting、recoverable 或 recovery-required。未知／不安全清单明确失败。

检查与列表的文本视图先给出结论，原始原因结构只保留在 JSON 中。
`config check` 输出一行结论、每项检查一行对齐的路径／语言区域／失败原因、设置警告、正在使用的文件及
`--format json` 指引。`agent doctor` 先输出就绪汇总和下一条命令，再输出提供方表格。`session list`
带表头和原生路径，并解释 `not created` 会话。已关闭的 `workspace close-status` 把成员显示为已关闭。
空的 `session list`、`command list` 或 `launch-profile list` 输出一行说明如何创建第一项；启动配置目录
修订号仅出现在 JSON 中。

`session attach <name> [--mouse on|off]` 激活或重连真实 TUI。`session stop <name> --confirm`
复用 `server stop`，包括其 timeout、detach、request-id、retry 参数及现有结果 schema。
`session delete <name> --confirm` 只移除该已停止命名会话的保留检查点和自有运行时文件，拒绝 default、
运行中或不确定状态，绝不删除 Project／worktree／用户目录。名称位置参数必须与显式 `--session` 一致。
状态缺席时删除幂等成功；部分移除保留删除意图阻止启动，并明确失败，重复 delete 完成清理，不能用 start 修复。
删除在每个文件系统阶段前检查 Ctrl+C，中断退出 130。已完成移除不会回滚；保留的意图支持继续清理。`interrupted` 结果报告是否仍有待完成意图，也覆盖清理完成后才收到中断的情况。

列表输出 `orchbee.session.list` v2；v1 保持原有封闭状态集合。`recoverable` 表示受门保护的当次观察
确认 v2/v3 登记完整、所有确切身份消失且资源空闲；其 `running=false`、`pid=null`，不承诺稍后的启动。
删除仍使用 `orchbee.session.delete` v1；内嵌 schema 清单为 v148。

持久清单在 LocalAppData 保留精确的默认/命名身份和停止回执。使用相同的已准入运行时基目录时，
即使运行时叶目录被移除，`session list` 仍能发现已停止会话；`session attach`/`server start` 会重建
路由并恢复检查点。`server start` 和 TUI 启动会自动恢复登记完整且证明全部死亡的 v2/v3 Active 代次，
也支持运行时叶目录丢失，并保留最近检查点。第一阶段保留数据、拒绝活 Host、未知／不完整身份和旧版 Active；
不终止旧进程，不伪造正常停止回执。使用相同 `--session` 选择执行 `orchbee server diagnose` 检查拒绝原因，
或执行 `orchbee --session recovery-work server start` 另开命名会话。新激活会在提交标记、拉起 Server 或发布
检查点存储前先发布 catalog v3。Stopped v1/v2 要求主检查点；Stopped v3 准入固定槽位与精确的自有
损坏原件名。只支持 v1/v2 的读取器即使面对正常停止的会话，也会在产生效果前拒绝 v3。详见
[恢复合同](../../../crates/local-site-runtime/docs/windows-crash-recovery.zh-CN.md)。待删除状态也会在临时
元数据消失后保留，可用相同删除命令继续处理。列表仍只读，不隐式重建运行时基目录本身。

后台及正常停止检查点会保留布局、Launch Profile、采样所得的逐终端 cwd 和唯一 provider 恢复引用。
启动选择最新有效副本，保留损坏字节，并在回退或打开空工作区时发出警告。不保存存活 Agent 状态、
终端历史或屏幕内容。活 Host 接管仍是后续工作。

## Agent 会话名称

`agent names` 列出运行中 Session 的所有存活别名。`agent resolve <target>` 返回 canonical 会话 ID 或精确终端目标，
优先检查精确 ID 是否存在，再查别名。`agent rename <target> <new-name>` 赋名或原子替换
名称；`agent rename <target> --clear` 只释放名称，不停止会话。可把目标位置参数替换成
`--agent-session <id>`、`--name <alias>` 或 `--panel <id|@panel:suffix>` 明确选择；必须且只能使用一种选择器。
名称格式为 `[a-z][a-z0-9_-]{0,31}`，区分大小写且唯一，canonical 名称、终端名称与待完成启动预留共用 512 条上限。
会话结束、移除或 Session 运行时 dispose 后释放名称；Session 重启后仅可恢复已接纳的已完成托管名称。

Windows 终端名称要求真实的上报、进程识别或保留 provider 身份；普通未命名 shell 不符合条件。
目标携带 presentation ID、创建 generation、Host epoch 和终端 ID。首次 provider reference
补齐归属；provider 或已知 reference 改变、确认释放／退出、移除或物理重建会清除名称。
报告提交时同步核对归属，因此 A → B → A 不会复活旧别名。单独缺少进程观察不会清除名称。
后续 Hook 到达不会悄悄把终端别名转为 canonical 会话别名。

终端目标 resolve/list 使用 version 3，rename/clear 使用 version 2，混合目标重名失败使用
version 4 的 `conflictTarget`；既有 canonical 回执保持原版本。统一投影之前，Windows `agent get` 不带
`--client` 时曾返回 version 15 `terminal-found`，包含精确目标、实际帧／进程／主 Panel 报告
字段及独立的 management 来源／状态读取。状态为 unmanaged 或带恢复 provider 的 active，
不代表当前就绪；当前这些终端详情保留在 v19 `inspection` 中。不伪造会话 ID。子终端读取不会借用主 Panel 的报告。按名 `read`、
`send-keys`、`attach` 经精确物理绑定交给既有终端所有者。物理 `explain` 返回 v2 新鲜屏幕诊断或携带生效规则来源的 v3；
语义 `wait`／`prompt`、focus 和 `get --client` 使用各自的物理观察／Client 契约，不伪造规范会话；启用 provider 恢复后，已完成托管名称绑定到新物理终端。
普通手工名称和待完成预留不在冷启动恢复。同 provider 换引用或改名保留恢复后的托管身份；
清名仅清除其仍匹配的自有持久恢复引用。

三个命令均接受 `--format text|json`。内嵌成功 schema 为 `orchbee.agent.names`、
`orchbee.agent.resolve` 与 `orchbee.agent.rename`；names 为 version 2，独立列出预留，另两者为 version 1；接纳后的失败使用
`orchbee.cli.failure.agent-names` version 1。presentation 解析与 ambiguous-target 失败使用 version 2；launch-pending 失败使用 version 3；原有结果仍为 version 1。相关语义结果携带 `agent-names` source
和 OBAP request ID。这四份文档与提示契约一起纳入 Schema 目录 version 64。非法名称或冲突选择器在
任何运行时访问前以退出码 2 失败。重名以退出码 3 拒绝并标明占用者，绝不覆盖其名称。

每个命令的单一五秒截止覆盖 cohort 校验、仅查已有 Session、接纳、可选 Panel 解析和最终 OBAP 交换。
信号参与传输等待；会话在信号仲裁与输出前关闭。确认回复与信号竞态时，已确认回复仍为
权威结果。已提交修改若没有可信匹配回复，包括取消时，返回 outcome-unknown（退出码 8），
绝不重放。已知在提交前取消返回 130。不引入别名缓存、不调用 provider、不创建 Session，
也不修改 canonical 会话文档。

`get`、`explain` 和 `wait` 接受相同的位置目标或显式 `--agent-session`/`--name`/`--panel` 选择器。
精确的保留会话 ID 优先于名称；显式 ID 与不可能是名称或 Panel 的不透明 token 直接读取。其余目标在
同一个已接纳的 Client Session 上先发 Resolve。解析与读取共享原来的截止和信号所有者，
不二次接纳或续期。wait 的首次解析受 `min(remaining_ms, 5000)` 约束，固定解析结果后再轮询；
释放、重命名或复用别名不会改变等待目标。成功结果只使用实际读取的 `agent-read` source；
解析拒绝使用自身的 `agent-names` source 与失败 schema，绝不伪造未执行读取的来源。
Windows 进程证据覆盖真实管道接纳、名称解析后读取、固定目标轮询、共享五秒截止、
显式 ID 直读、schema 合规失败、丢失或不匹配的修改回复，以及效果前解析。

历史 Windows 物理 Agent Get v15 在封闭 v14 形状上加入 `semantic: {source, observation}`。
其来源记录 Session、Workbench／报告修订和请求 ID；观察包含精确绑定、有效报告视图、终端局部
序号及空变化列表。主展示与子展示读取各自报告资源；主面板元数据／报告投影须与语义报告
修订一致。文本按所选语言呈现有效报告。带 `--client` 时，v16 还要求 `clientView`：所选
Client／操作／视图、报告修订、可空 sequence／seen／status 和外层焦点。只有完整绑定和
语义序号匹配，且 Server 报告修订覆盖 Client 样本时，才核实 seen。状态独立保持 idle/working/blocked/done/error/unknown；确认缺失或过期时只有 seen 未知。不虚构规范 Session、轮次或就绪；旧变体保持
封闭。`agent attention` 将物理目标路由到同一 Client 所有者，输出 v4 Client control 回执，
且不会确认已看过。

### 物理 Agent 等待与提示提交

Windows 终端名称使用报告所有者的精确物理绑定和语义历史。CLI 在准入、各次有界读取与
可中断退避间持有同一个 Client Session，并在信号裁决前释放。每轮刷新 Workbench 成员关系，
读取连续变化，再验证原始选择器和 provider。替换、历史丢失、未来游标、传输失败和 provider
ABA 明确失败；证据丢失后不重连或退化轮询。总期限不续期，迟到观察不能成功。

普通等待可匹配保留的瞬态，当前观察单独报告。此目标的 `--after-revision` 指报告来源修订，不是规范 Agent 修订；未来门槛在首次观察时拒绝。此后消费的历史必晚于该检查点报告修订。working、idle、blocked、unknown 及 needs-attention 别名使用报告事实。显式 done/error 报告支持 succeeded/failed；既有 done 等待别名匹配任一已结束结果。新鲜度、取消和中断条件仍要求规范观察。`--client` 可选固定一个 Client 并暴露其单独核实的已读确认；仅 done 的请求不再隐式选择 Client。缺失／过期确认不会阻止状态匹配，但显式所选 Client 丢失或读取失败仍结束观察。

提示预检拒绝 blocked，通过真实前台进程链验证已知 provider，并在提交前重读精确语义／物理
目标。已知存活 provider 即使报告状态 unknown 也可接收输入，与固定 Herdr 的准入一致。
既有 send 独占 paste／Enter、Windows Codex 边界、输入清理和交付证据，且只调用一次。
提示等待以提交前序号建立活动门槛：原先 working 或后续 working／blocked 在五秒内
（或更短总期限）开启门槛；消息／引用变化和原先 idle 均不能开启。短暂开启门槛的
working／blocked 可匹配这些条件；静止条件要求活动后的当前状态。历史丢失失败且不重发。

Wait v5 返回 `terminal-matched`、物理观察、可空匹配变化、条件与统计。Prompt v5 返回终端
目标、原始提交及可空语义等待。选定 Client 的匹配使用 Wait／Prompt v6，匹配中要求
`clientView`；Prompt v6 要求已完成等待。预检在输入前检查所选 Client，全部读取共享既有
五秒评估及绝对总期限，不确认已看过、不重发。Agent 读取失败 v4 和提示失败 v2 携带物理失败详情，旧版
保持封闭。输入接纳后的取消／失败保留提交回执；部分／不确定交付保留原 send 结果。
测试覆盖状态门槛、静默更新、瞬态与当前匹配、provider／物理变化、修订门槛、有界超时、
回执保留和 Windows 实际输入。生成测试含 32 个种子、6,400 个状态页，并在失败时缩减为可重放序列；它只检验保留状态与活动门槛，不证明退出事件或 Client 注意力。范围参照固定 Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 wait 与 prompt 处理器；Client 注意力、
真实登录 provider 行为和最终全量验收仍需独立证据。

## Agent 终端操作

`agent read <target>` 和 `agent send-keys <target> <key>...` 接受与 `agent get`
相同的 Session ID、运行时名称或 Panel 选择器，包括显式 `--agent-session`、`--name`、`--panel` 和 `--terminal`。
读取支持 `--source` 和 `--lines <n>`，边界和含义与
`terminal read` 相同。发送最多接受 64 个语义按键或修饰键组合，产生任何效果前验证整个序列。

Windows 上，这些命令使用[Windows 统一 Agent 投影](#windows-统一-agent-投影)中描述的共享解析器，
也接受 `@agent:<kind>` 和完整列表中的 `@agent:<n>`。裸名称仍优先匹配 Server 别名；
物理目标在整个操作中保留并复核完整绑定。下方规范会话流程描述该类目标路径及其他平台。

CLI 组合既有责任方：在一个仅连接既有 Session 的 Client Session 中读取 Workbench
快照，按需解析别名，再读取权威 Agent 会话。只有活动会话中可导航且存活的 presentation
附件才能选中终端。关联时比较快照中已绑定终端的完整 presentation ID；不会将 Panel ID
当成 presentation ID。显式或回退得到的 Panel 将关联限制在其主 presentation。
没有匹配返回 not-found，多个匹配返回 ambiguous，两种情况
均不附加 viewer 或提交输入。所有交换共享一个五秒期限。操作固定解析到的目标；输入准入时
Server 仍验证终端当前绑定。

| 状态             | 允许的下一操作                  | 失败效果                       |
| ---------------- | ------------------------------- | ------------------------------ |
| 已解析           | 快照准入和读取                  | 无终端效果                     |
| 已持有快照       | 在同一会话中 Resolve 和权威 Get | 无终端效果                     |
| 唯一绑定终端     | 既有终端读取或发送交换          | 沿用 viewer/输入清理规则       |
| 已收到回复或失败 | 释放会话，投影输出              | 不重放；不确定发送仍为退出码 8 |

成功复用 `orchbee.terminal.read`、`orchbee.terminal.history` 或
`orchbee.terminal.send` schema 及实际终端来源。Agent 解析或读取失败保留自身关联的来源和
失败 schema。不引入 provider 命令、Agent 生命周期转换、新 Session、别名缓存、输入所有权
或逐轮因果保证。信号沿用既有终端操作行为；此组合不宣称新增可取消的 viewer 或输入传输。
证据须覆盖互异的 presentation/Panel ID、非活动及不可导航附件、歧义绑定、效果前解析、
单次会话准入以及发送失败不重放。

嵌入的发送 schema 验证现有序列回执：载荷种类、记录数、已确认前缀、停止位置、编码
字节数、部分接受和清理状态。通用歧义失败包含 Panel 与终端候选项。这些修正过时的 schema
投影，不改变线路协议或回执版本。

## Agent 提示提交

`agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms>]`
与其他 Agent 命令使用相同 ID/名称选择器。Windows 也通过上述共享投影解析器接受
`@agent:<kind>` 和完整列表中的 `@agent:<n>`。文字为非空 UTF-8，受 256 KiB paste 上限和
命令 64 KiB argv 封套限制。`--until` 和 `--timeout` 要求 `--wait`；Windows 默认 idle/done/blocked 且无总超时，
显式有限超时最长一小时。不等待时总预算为五秒。

一个仅连接既有 Session 的 Client Session 负责快照查找、一次 Agent 身份解析、权威预检 Get、
一次 paste 加 Enter 发送以及后续 Get。预检要求会话存活、新鲜、状态已知且唯一绑定可导航
终端。有待处理请求/needs-attention 时，在发送任何输入前以 `agent-blocked` 拒绝。这是
采样预检，不会锁定 provider 后续状态或前台所有权。Server 在输入准入时仍验证终端绑定。

| 状态     | 转换与证据                                                            | 保留效果                     |
| -------- | --------------------------------------------------------------------- | ---------------------------- |
| 已解析   | 在 min(总期限, 五秒) 内准入与解析                                     | 无                           |
| 预检     | 新鲜、存活、已知、未阻塞且唯一绑定                                    | 无                           |
| 发送     | 一次有序 paste 和 Enter 序列，不重放                                  | 精确终端回执                 |
| 活动检查 | 预检已 working，或接受后五秒内观察到更新的 working/attention/轮次结果 | 后续任何错误都保留已接受回执 |
| 条件等待 | 在原总期限内采样固定会话的状态/新鲜度条件                             | 不承诺轮次因果               |
| 完成     | 释放会话，裁定取消，一次输出                                          | 不重放输入或取消 provider    |

从非 working 状态开始等待时，先要求活动证据。若轮询之间错过短暂 working，更新的轮次
结果也可作为证据。旧 idle 或不变的完成结果不能通过活动检查。活动期限耗尽为
`prompt-stalled`；调用方期限先到则为 `wait-deadline`。后续读取错误、结束、超时或取消
不会抹去已确认提交。已在 working 的 Agent 可能完成既有轮次；匹配结果不标识本次提示
自己的轮次。轮询可能漏掉转换，不宣称支持 Herdr 的事件重放。

Windows 中，canonical provider 为 `codex` 时，单次输入序列按字面 paste、Right、Enter 排列。Right 作为非字符边界，促使 Codex 刷出 paste-burst 缓冲并清除 Enter 抑制窗口；不向提示文本拼接转义码，也不以固定延时猜测投递速度。其他 provider 保持 paste、Enter，普通 `terminal send --paste --enter` 不增加按键。回执准确报告三条或两条记录；若在 Right 或 Enter 前停止，只报告实际接受的前缀，绝不补发或开始等待。该边界不证明输入框为空、焦点归属或 provider 已就绪。

Schema 目录 version 64 共 74 份文档，包含提示成功与失败契约。

`orchbee.agent.prompt` 返回会话 ID、完整终端提交回执，以及可选的匹配观察/等待事实。
发送拒绝、部分接受或不确定时沿用既有终端发送/失败契约，不开始等待。预检及提交后失败
使用 `orchbee.cli.failure.agent-prompt`；提交后失败附回执，避免将等待失败误认为尚未提交。
不带 `--wait` 的确认发送在取消与回复竞争时仍为权威结果。等待中的取消为退出码 130，
附已接受回执。已提交输入存在歧义时，即使取消也保持退出码 8。所有会话均先释放，再完成
信号处理和输出。回执不包含提示文字。

证据覆盖阻塞/未就绪拒绝且无终端效果、精确字面 paste 加 Enter、单次准入和不重放、
working/idle 活动检查、更新的完成结果、固定会话读取、总期限/活动期限、部分及不确定发送、
取消裁定和 schema 校验。原生权威观察仍是唯一 Agent 状态来源，不新增屏幕推断或 provider API。

## Agent 等待策略

Windows `agent wait` 和 `agent prompt --wait` 默认无总超时，按顺序采用 `idle`、`done`、`blocked` 条件。`--forever` 仍为显式同义选项，与 `--timeout` 互斥；prompt 要求 `--wait`。有限超时仍为 1–3600000 毫秒。准入、解析及每次读取仍有五秒上限；中断只停止等待，不重新连接。现有 Linux 轮询默认值不变。Prompt 保留非工作状态下的五秒活动门槛。

Windows wait 在首次规范 Get 前建立经认证的变更检查点，随后在后续 Get 前消费连续保留页。全部步骤共用一个已准入会话、可选的单调总期限和取消归属。每次评估最多排空 512 条变更，每页 100 条，最多保留一条命中变更。不会静默恢复游标重置或跨启动周期：历史丢失明确拒绝。部分回包或读取失败不能成为命中观测。

Windows wait 版本 3 新增可空的 `matchedChange`。空表示当前样本命中；非空时历史变更是匹配证据，`revision` 和 `session` 仍为独立的最新 Get 样本。历史记录不会覆盖当前状态。`source.requestId` 标识命中的 Get 或变更读取。保留请求条件顺序。`--after-revision` 仍为全局修订门槛，不代表回合因果。Linux wait 和 prompt 成功封套保持版本 2；Windows prompt 使用版本 3。有限／无限失败保持既有版本及空超时区别。

`blocked` 匹配 fresh needs-attention；`done` 目前匹配 fresh succeeded/failed/canceled/interrupted。Client 相对的未看见状态合并属于独立后续消费者，变更流本身不代表它已实现。证据覆盖瞬态、其他会话、分页顺序、重置拒绝、固定身份、期限和取消清理。

对于规范会话目标，Windows `agent wait --client <id>` 同时固定一个精确 Client 和已解析 Agent。全部条件独立于 Client 已读确认使用规范当前／变更页匹配。缺失／过期 Client 版本暴露 null seen，不阻止 idle 或明确结果。历史规范事件不会虚构历史 Client 确认。所选 Client 丢失时结束等待，不改选其他 Client。

所选 Client 等待使用 v4，要求 `client` 投影并保留可空 `matchedChange`；未选 Client 的 Windows 等待保持 v3。每次 Client 读取、游标证明和规范样本共用五秒评估上限，并受原始总期限约束。Busy 时仍可读取，不确认已看见、不导航。有限超时和取消保持既有等待失败／收尾契约。即使当前 Client 状态不同，显式规范状态条件也可以返回历史证据。

所选 Client 组合查询对缺失或过期 Client 保留 `orchbee.cli.failure.client-control` v1 选择错误。其他 Client 读取错误使用同封套 v2，必须包含 `stage: client-agent-read`；kind 区分拒绝访问、操作拒绝、传输丢失、无效应答、截止和取消。既有 Agent 和准入错误保留原契约。

## 输入与信任边界

- `argv`：有界敌意输入。全部参数的 UTF-8 总字节不超过 64 KiB（65,536）。任一非 UTF-8 参数或
  超限即 `invalid-usage`、stdout 为空——即使另一个参数写着 `--format=json`。不隐式读取 stdin
  或文件输入——唯一例外是显式声明的 `terminal send --file`/`--stdin` 源（见
  [Send 输入源](#send-输入源)），在任何运行时副作用之前一次性物化。
- 环境：`ORCHBEE_RUNTIME_BASE`（可选，默认 `/tmp`）选择运行基座；`LC_ALL`/`LC_MESSAGES`/`LANG`
  选择诊断 locale。`agent doctor` 另外读取 `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`
  （观察端点 override，其 endpoint 事实原样报告）、供有界可执行文件扫描的继承 `PATH`，以及
  Linux 上 provider 目标前缀所用的 `HOME`。Windows 使用原生用户配置目录，
  并读取可选非空 `CODEX_HOME` 选择 Codex 配置。调用者模式的 `panel current` 还读取下文说明的
  `ORCHBEE_TERMINAL_ID` 与 `ORCHBEE_WORKER_ID`。不读取其他任何环境变量。
- 文件系统：解析出的运行基座、Session scope 目录 `orchbee-native-<euid>` 及其
  `client.sock`/`worker.sock`、activation 标记、当前可执行文件同目录的 `cohort.json` 与四个
  artifact 只被观察、绝不写入。清单还声明 `agent-observation-adapters` sidecar 摘要——
  启动绝不打开或遍历该目录。所有目录遍历使用既有 no-follow、属主/模式校验的 rendezvous 归属
  方（`ModeCheckedLocalSiteRendezvous::validate_mode_checked`，仅 `ExistingOnly` 策略）。
  `agent doctor` 检查一个不相交的声明集合——解析出的 home 下的十个 provider 钩子目标、
  提供的 `--asset-root` 路径、这些 provider 文件自身声明的 recorded root、观察 socket 路径的
  末段组件，以及每个 PATH 目录的候选可执行文件——全部在其自身的单文件/计数/聚合字节与条目
  上限下以 no-follow `lstat` 准入。它绝不枚举环境用户目录。
- Socket 对端：`client.sock` 可能属于真实 Server、残留 socket 或脚本 peer。CLI 信任类型化
  transport 与协议层（OBLC 准入、OBWP 相关性与契约校验），绝不从 `Display` 文本、`ErrorKind`
  猜测或字节计数重新推导结论。
- 快照数据：用户可控的 label 与目录路径是任意 Unicode，终端输出按敌意文本处理（见转义规则）；
  JSON 输出无损保留。

## 公共语法

选项扫描在 `--` 之前自左向右进行；`--` 仅终止选项识别。

- `--format text|json` 或 `--format=text|json`：至多一次选择，`--` 前任意位置。
  `--format` 缺值、空值、未知值或重复选择都是格式失败：`invalid-usage`、退出 2、stdout 为空。
  完整的 `--` 前 format envelope 先于普通语法校验整体扫描：envelope 内任何位置的格式缺陷都以
  空 stdout 作答；唯一合法的选择无论位于何处都决定其后一切语法失败的 envelope。
- `--project <target>`、`--workspace <target>`、`--tab <target>`：带值的具名过滤器与上下文
  操作数，各只能用一次。下一个 token 即作为值被消费，即使以减号开头；裸 `--` 永不被当作值
  （缺值 → `invalid-usage`）。被消费的过滤值不再充当 format 选择器。每个 flag 只在声明它
  的命令上合法（`--project` 仅 `workspace list`/`workspace create`/`worktree list`/
  `workspace close-status`/`workspace close-resume`，
  `--workspace` 仅
  `tab list`/`tab create`/`workspace close-status`/`workspace close-resume`，`--tab`
  仅 `panel list`），出现在任何其他位置（包括 `get`
  命令）都是 `invalid-usage`。过滤器绝不静默丢弃：出现在未声明的命令上即在任何运行时访问
  之前报 `invalid-usage`，故 `workspace list` 拒绝 `--workspace`、`--tab` 与
  `--standalone`。在 `workspace close-status` 与 `workspace close-resume` 上，两个 flag
  都是严格稳定 ID 而非 target：只接受完整的非零 32 位小写十六进制 Workbench 标识符——
  绝不接受 label 或 selector——因为保留的 close 记录比 Workspace 的 active-projection
  membership 活得更久，必须按稳定身份重新证明。
- `--directory <directory-id>`：带值 flag，单次使用，用于 `worktree list` 的注册目录形式，其他命令上均非法。Windows 另有不带此 flag 的来源形式；参见 [Windows Worktree](windows-worktrees.zh-CN.md#按来源发现-worktree)。值仅为 `WorkbenchDirectoryId`——严格非零 32 位小写十六进制标识符，
  绝非文件系统路径、标签或前缀。缺失、畸形、大写、全零或宽度错误的值在任何运行时访问
  之前即为 `invalid-usage`；CLI 不做目录 membership 预检——membership 由运行中 Session
  权威判定，well-formed 但不存在的目录以 worktree 失败 envelope 下的
  `not-found` `{resource:"directory"}` 作答。
- `--standalone`：无值布尔 flag，单次使用，仅 `tab list` 与 `tab create`；两处均与
  `--workspace` 互斥。
- `--label <label>`：带值 flag，单次使用；`project create`、`project rename`、非 Windows `workspace create`、`workspace rename` 与 `tab rename` 上必填。Windows `tab create` 与 `panel split` 可省略，采用创建前相应 Tab／Panel 数加一。显式值为非空且在协议上限内的 Workbench label；空串或超长均为 `invalid-usage`。
- `--path <path>`：带值 flag，单次使用。在 `workspace accept-directory` 上必填，接受 `WorkbenchDirectoryPath`：字面本机绝对路径（POSIX `/` 或 Windows 盘符绝对路径），至多
  4,096 UTF-8 字节且不含 NUL，不展开 `~` 或变量。缺失、相对、含 NUL 或超长的值均为 `invalid-usage`。
  Windows Worktree Create/Open 则按 [Worktree 路径策略](windows-worktrees.zh-CN.md#命令与默认值)接受字面绝对、相对及前导 `~` 操作数；完整语法校验与机器路由后，由最终执行的 CLI 在访问配置或运行时前解析。两种形式均不创建、修复或探测目录；文件系统事实仍由 Server 在提交后权威判定。
- `--direction left|right|up|down` 是四向词表。`tab move` 使用
  `--order previous|next|first|last`，与 `--insert-index` 互斥；`terminal copy-search`
  使用 `--search-direction forward|backward`。未知值在运行时访问前失败。
- `--cols <n>`、`--rows <n>`、`--cell-width <n>`、`--cell-height <n>`：`tab create` 与
  `panel split` 上的终端几何组，各取一个十进制 `u16`。该组为全有或全无：四个全部省略
  时命令请求文档化默认值（80x24 逻辑网格、成对零的未知单元像素）；提供任何一个就必须
  提供全部四个。组合后的格数与像素范围必须满足协议 `WorkbenchTerminalGeometry` 界限；
  缺项、重复、非数值或越界均在任何运行时访问之前报 `invalid-usage`。
- `--exec <path>` / `--arg <value>`：临时启动选择，仅 `tab create`、`panel split` 与
  `launch-profile create` 接受（后者必填，前两条 launch-bearing 修改命令上可选）。
  `--exec` 单次使用，取不含 NUL 的非空绝对 UTF-8 可执行文件路径。`--arg` 可重复，
  每个值按顺序原样追加为 `argv[1..]`——空值与 dash 前导值按标准取值规则被消费为值，
  `argv[0]` 由可执行文件路径合成。至多 128 个 `--arg` 值，且可执行文件加全部参数值合计
  至多 30,720 UTF-8 字节；无 `--exec` 而带 `--arg`、重复 `--exec`、或越界均在任何运行时
  访问之前报 `invalid-usage`。省略 `--exec` 保持解析默认 shell；选择属于请求身份，绝不
  持久化为 profile。
- `--name <name>`：带值 flag，单次使用，`launch-profile create` 上必填、其他任何
  命令上非法。值为 `LaunchProfileName`——非空 UTF-8 字面量、至多 512 字节；不要求
  唯一（唯一定位条目的是铸造的 profile ID，而非名称）。缺失、空或超长值均为
  `invalid-usage`。
- `--profile <selector>`：带值 flag，单次使用，仅 `tab create` 与 `panel split`
  受理，与显式 `--exec`/`--arg` 选择互斥——任一组合均为点名冲突选项的
  `invalid-usage`。值为 `LaunchProfileSelector`：非空 UTF-8 字面量、至多 512 字节，
  逐字传给 Server，由其在目录锁下按"先 ID 后名称基数"规则解析——CLI 绝不读目录或
  猜测匹配。解析出的 profile 仅为该次请求提供启动，不在任何其他地方持久化。
- `--request <close-id>`：带值 flag，单次使用，`workspace close-resume` 上必填、其他任何
  命令上均非法。值为该 close 操作自身的 `WorkbenchRequestId`——严格非零 32 位小写十六
  进制标识符，由 Server 重新校验的操作身份，绝非 resume 请求的新鲜身份。缺失、畸形、
  大写或全零的值在任何运行时访问之前即为 `invalid-usage`。
- `--generation <n>`：带值 flag，单次使用，`workspace close-resume` 上必填、其他任何
  命令上均非法。值为显式 attempt-generation 栅栏：不带前导零的非零十进制 `u64`——协议
  `WorkbenchTabCloseAttemptGeneration` 的拼写。陈旧但合法的 generation 是语义请求，由
  Server 以 `stale` 作答，绝非语法问题；缺失、空、零、非数值或溢出值在任何运行时访问
  之前均为 `invalid-usage`。
- `--confirm`：无值布尔 flag，`tab close`、`panel close`、`workspace close` 与
  `workspace close-resume` 上必填；该处缺失或出现在任何
  其他命令上均为 `invalid-usage`。带 `--dry-run` 的 `tab close` 或 `workspace close`
  无需 `--confirm`。
- `--match <text>`：带值 flag，单次使用，`terminal wait-output` 的两个搜索输入之一
  ——`--match` 与 `--regex` 必须且只能提供一个。非空 UTF-8 字面量，最长 4096 字节，
  匹配时区分大小写。缺失、空、非 UTF-8 或超长值——或两个搜索 flag 同时出现——均为
  `invalid-usage`。
- `--regex <pattern>`：带值 flag，单次使用，`terminal wait-output` 的另一搜索输入；
  非空 UTF-8 正则表达式，至多 1024 字节，在任何运行时访问之前以 1 MiB 程序大小上限
  与 1 MiB 惰性 DFA 缓存上限编译一次。缺失、空、超长或编译失败的值——或与 `--match`
  并存——均为 `invalid-usage`。
- `--timeout <ms>`：带值 flag，单次使用，`terminal wait-output`（所有平台默认 `30000`）与
  `agent wait`（Windows 默认无总超时，Linux 默认 `600000`）上可选；取一个 `1..=3600000` 的十进制 `u32`。
  重复、非数值或越界值均为 `invalid-usage`。

- `--target-panel <target>`：带值 flag，单次使用，既有同 Tab `panel move` 上必填，`--into-tab` 形式可选；按 `panel` 的
  target 规则解析（仅完整 typed ID）；既有移动在来源 Tab 内解析，`--into-tab` 形式在目标 Tab 内解析。
- `--source visible|recent|recent-unwrapped|detection` 与 `--lines <n>` 选择终端／Agent
  读取及终端等待的输出。默认 recent、80 个物理行；行数为无符号 32 位，钳制到 1000，允许零。
  读取／等待拒绝旧 `--history`、`--rows` 和 `--unwrap`；几何命令的 `--rows` 仍表示尺寸。
- `--text <utf8>`、`--key <name>`、`--file <path>`、`--stdin`：`terminal send`
  四个互斥输入源，在其他命令上均非法。`--text` 与 `--file` 按标准
  取值规则消费下一个 token 作为值（以减号开头的 token 也会被消费，裸 `--`
  不会）；每次 `--key` 取一个封闭键名，最多重复 64 次；其他源只能使用一次。`--stdin` 无值。`--text` 必须是非空 UTF-8
  字面量，其字节长度加上可选折叠 CR 不越过 4,096 字节记录上限；`--file`
  原样存储其操作数——解析器不做任何文件系统校验，一切路径、类型与内容
  检查都归 [Send 输入源](#send-输入源) 的物化器。`--enter` 是文本类源的
  修饰符：`--text`、`--file`、`--stdin` 上合法，`--key` 上或单独出现均非法。
  `--paste` 仅能与 `--file` 或 `--stdin` 同用，生成一条粘贴记录及可选的末尾 Enter 记录。
  零个或多于一种源均为 `invalid-usage`；冲突按固定次序
  `--text`、`--key`、`--file`、`--stdin` 命名，与 argv 次序无关。
- `--mouse off|on`：带值 flag，单次使用，在显式 `tui`、`terminal attach` 和
  `agent attach` 路由受理；宿主机指针捕获默认开启。attach 遵循
  [Windows 流契约](windows-terminal-streams.zh-CN.md)，包括任一全局格式下均输出交互画面。两种声明写法（`--mouse on` 与 `--mouse=on`）取值同为
  封闭的 `off|on` 词表；缺失或未知值、任一写法下的重复、出现在其他命令路由
  或裸调用，均为 `invalid-usage`。
- `--include-ended`：无值布尔 flag，单次使用，仅 `agent list` 受理；让请求页包含
  lifecycle 为 `ended` 的 session。
- `--limit <n>`：带值 flag，单次使用，仅 `agent list` 受理；页大小为一个
  `1..=100` 的十进制 `u64`，默认取 wire 契约的 50。重复、非数值或越界值均为
  `invalid-usage`。
- `--cursor <c>`：带值 flag，单次使用，仅 `agent list` 受理。Windows 使用不超过
  1024 个字符的结构化 `u1.<base64url>` 游标，绑定 Site、来源修订、筛选条件、Client
  选择和页位置／指纹。解析器识别出的旧格式游标会在读取实时来源前返回
  `cursor-restart`。判断 `u1` 游标是否过期需要读取当前来源，因此只能在这些读取后
  拒绝过期的结构化游标。其他平台保留规范 Server 游标契约。
- `--until <condition>`：可重复带值 flag，由 `agent wait` 和 `agent prompt --wait` 受理；每次出现取
  十四名条件词表中的一个：`blocked`、`done`、`needs-attention`、`failed`、`succeeded`、
  `canceled`、`interrupted`、`working`、`idle`、`unknown`、`ended`、`fresh`、
  `stale`、`disconnected`。重复的值按请求顺序保留用于匹配与输出回显；重复
  同一值或未知名称均为 `invalid-usage`。完全省略 `--until` 时，Windows 依次使用
  `idle`、`done`、`blocked`；Linux 依次保持 `needs-attention`、`failed`、`succeeded`、
  `canceled`、`interrupted`、`idle`、`ended`。
- `--after-revision <n>`：带值 flag，单次使用，仅 `agent wait` 受理；可选的
  全局修订栅栏，取 `0..=9007199254740991` 安全整数范围内的一个十进制 `u64`。
  重复、非数值或越界值均为 `invalid-usage`。
- `--provider <id>`：带值 flag，可重复，仅 `agent doctor` 受理；每个值是一个规范
  provider id（`claude-code`、`codex`、`gemini`、`github-copilot`、`grok`、`cline`、
  `opencode`、`pi`、`omp`、`qwen-code`），按 flag 次序选取并在固定规范行序下去重——
  未知 id 在任何运行时访问之前即 `invalid-usage`。不带该 flag 即选取全部十个。
- `--asset-root <path>`：带值 flag，可重复，仅 `agent doctor` 受理；每个值是声明为
  capsule 候选根的非空绝对无 NUL 路径，按 flag 次序保留。相对拼写、`~`/`$` 形式、空值
  与第五个值在任何运行时访问之前即 `invalid-usage`——至多受理四个 supplied root，
  任何一个都绝不展开、绝不猜测。
- `--provider <id>`：带值 flag，单次使用，仅 `agent integration` 命令受理；
  封闭的受支持 provider id 之一（十个成员，`agent integration list` 报告）。
  在 `explain`/`enable`/`disable` 上必填，在 `list` 上为可选过滤。其他取值均为
  `invalid-usage`。
- `--adapter-source <abs>`：带值 flag，单次使用，仅 `agent integration` 命令受理；
  一个字面绝对路径，指向开发/测试用的 adapter 负载，替换本次调用内打包的 cohort
  来源。相对路径或重复使用均为 `invalid-usage`。
- `-v`、`-V`、`--version` 是等价版本选项，仅在独占根调用时合法；均离线输出相同的
  `orchbee <version>` 行并以 `0` 退出。组合使用或添加命令、帮助开关、输出格式均是 usage。
  选项值与 `--` 后的 token 保持数据含义。
- `-h`/`--help`：`--` 前任意位置的单个开关，选择精确族路径或最长的已识别命令路径，
  忽略操作数和业务必需条件且不产生效果。
- `--` 之后的位置参数：`--` 之后的 token 只作位置参数，故 `orchbee -- status` 查询 status，
  `orchbee project get -- --help` 解析的是字面 label `--help`。

命令路径（固定词，无缩写、无别名）：

```
orchbee                         # 裸调用 → TUI（不变）
orchbee tui [--mouse off|on]    # TUI
orchbee -v|-V|--version         # 版本行
orchbee -h|--help | orchbee help   # 根帮助
orchbee status
orchbee status server | status client
orchbee project list | project get <target>
orchbee project create --label <label>
orchbee project rename <target> --label <label>
orchbee workspace list [--project <target>] | workspace get <target>
orchbee workspace create --project <target> --label <label>
orchbee workspace rename <target> --label <label>
orchbee workspace accept-directory <target> --path <path>
orchbee workspace close <target> (--confirm [--detach | --timeout <ms>] | --dry-run)
orchbee workspace close-status --workspace <id>
orchbee workspace close-resume --workspace <id> --request <close-id> --generation <n> --confirm
orchbee worktree list --directory <directory-id>
orchbee tab list [--workspace <target> | --standalone] | tab get <target>
orchbee tab create [--workspace <target> | --standalone] [--label <label>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee tab rename <target> --label <label>
orchbee tab move <target> (--order previous|next|first|last | --insert-index N)
orchbee tab close <target> (--confirm [--detach | --timeout <ms>] | --dry-run)
orchbee panel list [--tab <target> | --workspace <target>] | panel get <target>
orchbee panel rename <target> (--label <text> | --clear)
orchbee panel split [<target> | --current] --direction left|right|up|down [--label <label>] [--ratio <number>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee panel close <target> --confirm
orchbee panel move <target> --target-panel <target> --direction left|right|up|down
orchbee terminal list [--workspace <target> | --tab <target> | --panel <target>] | terminal get <target> | terminal read <target> [--source visible|recent|recent-unwrapped|detection] [--lines <n>]
orchbee terminal wait-output <target> (--match <text> | --regex <pattern>) [--source <source>] [--lines <n>] [--forever] [--timeout <1-3600000>]
orchbee terminal send <target> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>
orchbee launch-profile list | launch-profile get <selector>
orchbee launch-profile create --name <name> --exec <path> [--arg <value>]...
orchbee launch-profile delete <selector>
orchbee agent list [--include-ended] [--limit <1-100>] [--cursor <c>]
orchbee agent get <target> | agent explain <target>
orchbee agent read <target> [--source visible|recent|recent-unwrapped|detection] [--lines <n>]
orchbee agent send-keys <target> <key>...
orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms>] [--forever]
orchbee agent wait <target> [--until <condition>]... [--after-revision <0-9007199254740991>] [--timeout <1-3600000>] [--forever]
orchbee agent doctor [--provider <id>]... [--asset-root <path>]...
orchbee agent integration list [--provider <id>]
orchbee agent integration explain --provider <id>
orchbee agent integration enable --provider <id> [--adapter-source <abs>]
orchbee agent integration disable --provider <id> [--adapter-source <abs>]
orchbee client list | client get <client-id> | client control-status <operation-id>
orchbee workspace focus <target> [--client <id>]
orchbee tab focus <target> [--client <id>]
orchbee panel focus [<target> | --current] [--client <id>]
orchbee panel zoom [<target> | --current] [--toggle | --on | --off] [--client <id>]
orchbee api snapshot
orchbee api schema [<schema-id>] [--output <path>]
orchbee server start
orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>]
orchbee server stop-status <request-id>
orchbee server diagnose
orchbee config check
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --default-config  # Windows
orchbee --skill
orchbee help <recognized path>
orchbee <recognized path> -h|--help
```

帮助主题包含根级、构建接纳的每个命令族和完整命令路径（包括 `tui`），以及 `agent integration`、
`terminal session` 和 Windows `panel graphics` 等嵌套命令族。
`help` 只接受已识别的路径（不带过滤器、不带 format）；`help -h` 与 `help --help` 显示根帮助，
`help <path> -h|--help` 保留该精确路径及选项范围。
`--` 前的单个 `-h` 或 `--help` 是同一路径上的同一请求：`orchbee --help`、
`project --help`、`project get --help` 输出的内容与 `help`、`help project`、`help project get`
完全相同。开关 help 也接受已提供的操作数和已识别的选项（`project get x --format json --help`
以文本描述 `project get`），绝不应用这些选项或读取运行时状态。未知选项、缺值、重复选项和
未知族动词仍是 usage。未列出的
目标如 `help help`、`help version`、`help --version` 按未知路径 → usage。Help、`-v`/`-V`/`--version`、
`--skill`、`completion`、不带 `--output` 的 `api schema` 与 Windows `--default-config` 绝不触碰平台、运行、cohort 或 Session 状态。显式 Windows schema 导出仅操作调用者选择的文件目标及自有同目录临时文件。
帮助在 `en-US` 和 `zh-CN` 中均分为三个信息层级：

- 根帮助介绍 Orchbee，展示少量常用示例，将每个已接纳的顶层命令归入一个分组，
  并说明全局选项及如何进入下一级帮助。
- 命令族帮助说明该资源，并列出每个直接子命令及其用途。`agent view`、`panel graphics`
  等嵌套命令族有独立目录。“语法概要”一节重复每个直接子命令自身的用法行，省略开头的
  `orchbee` 和共用的 `--format` 选择，因此随语法同步；完整的选项说明仍在子命令专页。`completion` 列出受支持的 Shell。
  每个命令族与命令页面均显示 `-h, --help`；`server` 目录包含 `stop` 与 `stop-status`，
  并在 `stop` 上标明必须提供 `--confirm`。
- 具体命令帮助包含完整语法、位置参数、已接纳的选项、示例，以及该命令的默认值、
  效果和限制。`status` 还会列出其子命令。

`src/invocation/help.rs` 中的帮助路径目录同时驱动路径准入和命令目录。
业务选项归属使用补全的命令表；根模式选项遵循其既有语法。本地化语法与限制分别保存，
由一个纯渲染器负责间距和换行。
输出为无样式的 UTF-8，以换行结束；按显示单元格对齐，在 78 格处换行且不拆分字素簇。
长标签独占一行。示例标明所用 Shell，保留完整的带引号操作数，并在跨行时使用 PowerShell 或 POSIX Shell 续行符。终端和重定向输出使用同一确定宽度：帮助不查询终端，不读取 `COLUMNS`，
也不读取配置或运行时状态。窄于 78 列的宿主可能自行再次折行。
测试覆盖两种语言、所有已接纳路径每个位置上的两种帮助开关、补全与帮助命令集合一致性、
命令目录覆盖、40／78／120 格的渲染输入，并验证示例可被解析而不执行其效果。
`cli_windows_site_stop` 还用不可用的运行时根目录，运行真实二进制，检查版本别名、嵌套帮助
及停止缺少确认的拒绝行为；既有停止回执测试继续保护生命周期安全性。

在 Windows 上，`--default-config` 必须是根命令的唯一参数，直接从构建输出规范 UTF-8 TOML，不读取用户配置或运行时选择环境。
`--skill` 必须是根命令的唯一参数，输出内嵌且本地化的 CLI 使用指南。PowerShell
补全是静态语法投影：绝不执行用户表达式或查询 Session；需显式加载，不修改 shell profile。
PowerShell 7 会对独立的 `-` 调用补全并提供根级 `-v`/`-V` 别名；Windows PowerShell 5.1
不会为这个未完成的词调用原生补全器。Windows 见证在两个引擎中用未改动的回调检查全部共享
语法用例，并检查真实补全管道；其中 5.1 的两个独立短横线短别名用例明确记录为引擎未调用
边界。两个 Shell 都支持版本参数解析。

Elvish 补全与 PowerShell 使用同一份由 Rust 维护的 JSON 语法。回调接收交互编辑器提供的
字面词语（命令在首位，未完成词在末位）。每次调用的纯解析器记录已完成的位置参数、已见
flag、字面值和待消费值的所属 flag。`--` 与终止选项停止补全；非法作用域、重复或冲突使可行
命令集合为空。可行路径决定下一个命令词、未使用且兼容的 flag、操作数预算和封闭枚举值；
连写值保留 `--flag=` 前缀。自由形式的值没有候选。回调去重输出候选，匹配与引用由 Elvish
负责。它不求值任何传入词语，也不启动进程。只注册 `orchbee` 与 `orchbee.exe`，不修改
profile 或其他补全器。生成脚本遵循共享的 8 MiB 输出上限。独立的固定输出测试要求每个
补全脚本和帮助正文小于 512 KiB（实际输出上限的 1/16），用于提示意外的语法体积增长。在交互式 Elvish 0.21 中显式
执行 `eval (orchbee completion elvish | slurp)` 加载。

共享补全用例检查可接受与失效前缀、帮助深度、目标冲突、枚举所属参数及连写形式。Elvish
解释器测试在可移植验收中明确忽略；手动执行时必须用 `ORCHBEE_TEST_ELVISH` 指定解释器，
缺少解释器会使该次测试失败。测试提供注册命名空间，在无编辑器环境执行未经修改的脚本。
Windows ConPTY 证据单独检查实际交互注册与加载。这证明静态补全，不涵盖运行时目标发现
或其他操作系统。

`[<geometry>]` 代表可选的全有或全无组 `--cols <n> --rows <n> --cell-width <n>
--cell-height <n>`；省略时命令请求默认的 80x24 网格与未知单元像素 (0,0)。

`help`、`completion`、`tui`、`-v`/`-V`/`--version`、`--skill`、裸调用与 Windows `--default-config` 是固定文本输出，不接受 `--format`；一个合法
`--format=json` 会把随后产生的语法失败变成 JSON `invalid-usage` 失败文档，而 text/默认失败仍
保持 stdout 为空。数据命令（`status`、全部 `list`/`get`、`workspace close-status`、`agent explain`、`agent doctor`、`agent wait`、
`api snapshot`、`api schema`、`server start`、`server stop`、`server stop-status`、`server diagnose`、`config check`，以及全部四条 `agent integration` 命令）与全部 mutation 命令接受 `--format`。`api` 命令是机器面：`api snapshot` 或 `api schema` 成功时在
两种 format 下输出同一份 JSON 文档；`api` 失败则与其他命令一样跟随所选 format——text/默认下
只写 stderr、stdout 为空，只有 `--format json` 才产出失败 envelope。

`orchbee completion` 打印本语法的静态投影：动词、完整帮助路径、每条命令已声明的 flag
（分写与 `--flag=value` 连写两种拼写）、`--format` 的两种拼写及其 `text`/`json` 值、
封闭词表值在两种拼写下的补全（`--mouse`、`--direction`、`--key`、`--until`、`--provider`）、
`--` 终止与 flag 值消费上下文。不枚举任何 Session 状态、资源名或运行时数据，也绝不猜测
自由形式的值。

### 目标与过滤器

`<target>` 接受稳定紧凑身份 `@<kind>:0x<hex>`：1–32 位小写十六进制、非零、无前导零，
补足前导零后作为精确 32 位类型化身份。列表和文本诊断打印该句柄，JSON 回执保留规范 ID。
补全把它当作不透明操作数接受，不枚举运行时身份。PowerShell 中给句柄加引号，例如
`panel get '@panel:0x1'`。其作用域和寿命与完整 ID 相同，Server 重启后重新发现运行时终端 ID。
其他资源变化不会使它改指对象。既有三种输入形态也继续支持。第一种是完整 typed ID（恰好 32 位小写十六进制、非零），
只按身份匹配。第二种是显式 selector `@<kind>:<suffix>`——`<kind>` 为 `project`、
`workspace`、`tab`、`panel`、`terminal` 之一，`<suffix>` 为一至三十一个小写十六进制
字符——在已读快照上解析到该 kind 中标识符以该后缀结尾的唯一成员。已识别 selector
前缀但后缀为空、含大写、为 32 位数字，或 kind 与操作数 resource 不符，在任何运行时访问
之前即 `invalid-identifier`（退出 2）；其余 `@` token 保持 label 解读——只有五个前缀是
selector 语法。其他 token 在 resource 允许 label 时视为精确、区分大小写的 label 候选，
要求 1–512 个 Unicode scalar；空或过长的 token 与全零 ID 形值都是 `invalid-identifier`
（退出 2）。本身恰为合法 ID 或 selector 形状的 label 永远无法按 label 或 selector
命中——该实体仍可通过其真实 ID 访问。`panel` 与 `terminal` 只接受完整 typed ID 及各自的
`@panel:`/`@terminal:` selector，其余一律 `invalid-identifier`。过滤器
（`--project`、`--workspace`、`--tab`）使用各自资源相同的 target 规则；所有 mutation 位置
操作数与 `--target-panel` 同样遵守（`panel` 规则，并在被移动 panel 自身所在
tab 内解析）。`api schema` 接受非空
schema ID；空串是 `invalid-identifier`（`resource="schema"`），无匹配是 `not-found`。
`agent get`、`agent explain` 与 `agent wait` 的位置目标是 `[A-Za-z0-9._:-]` 的
1–128 字符 token（也支持显式 Panel 前缀）；按上述 ID 优先规则解析，或用 `--agent-session`/`--name`/`--panel` 明确选择。
畸形 token 在任何运行时访问前即 `invalid-usage`。
`launch-profile get` 与 `launch-profile delete` 接受一个位置 `<selector>`，
`--profile` 携带同一 token：`LaunchProfileSelector`——非空 UTF-8 字面量、至多 512
字节（即名称上限，使每个受理的 selector 都能触达任一规则）。空或超长 token 为
`invalid-usage`；CLI 绝不把它拆成 id/label 分派——由 Server 在目录锁下解析：先精确
profile ID，再按名称、匹配基数必须恰为一。零匹配应答 `not-found`；名称匹配多于一个
条目应答 `ambiguous`，报告至多前 8 个候选 profile ID 加真实 `totalCount`——两者均走
专用 launch-profile 失败 envelope。

读取成功后的解析：零匹配 → `not-found`（3）；多个精确 label 或 selector 后缀匹配 →
`ambiguous`（3），按 canonical 遍历次序报告至多前 8 个候选 typed ID 与真实 `totalCount`。
selector 候选是命令本已读取的那份快照中可见的成员——对每个操作数声明的作用域都是完整
集合，绝不追加抓取——且旧后缀是瞬时选择器而非稳定句柄：删除再创建可能让同一后缀落到
不同成员上，需要稳定身份的命令使用打印的紧凑句柄或完整 typed ID。命名不存在实体的过滤器是
`not-found`；实体存在但结果为空是成功的空表。

## 查询状态机与预算

每个网络查询（`status`、`list`/`get`、`api snapshot`）在 Linux 上严格按下列顺序执行；第一个
失败阶段即决定失败：

1. **Cohort** —— `NativeCohort::current` 在一个 checked 5 秒单调 deadline 下运行
   （`Instant::now().checked_add(5s)`；`checked_add` 失败为 `internal-failure`、退出 1）。
   任何 cohort 错误（缺失、非法、不安全、散列不符、协作式 deadline）都是 `check-failed`、
   stage `cohort`。本地化 stderr 提供修复建议，不输出原始 I/O 详情。只有同级文件的
   open 操作返回 `NotFound` 时才会点名该缺失可执行文件；否则不会猜测哪个文件缺失。
   Windows `status client --format json` v3 携带有界 `ioKind`／`osError` 详情，并在证据
   确凿时包含精确缺失的同级文件。失败封套和退出码不变。
2. **Discovery** —— 解析 `ORCHBEE_RUNTIME_BASE`/默认值，然后
   `ModeCheckedLocalSiteRendezvous::validate_mode_checked`（`ExistingOnly`，不 mkdir、不取锁）。
   分类使用 `RendezvousError` 的 typed path 与已解析 `runtime_base`/`runtime_base.join(scope)`
   比较：scope 目录上 source 为 `NotFound` 的 `System` 错误是 `service-unavailable`、stage
   `discovery`；其余一切 rendezvous 或文件系统校验失败都是 `check-failed`——typed path 属于
   基座或基座组件时 stage `runtime-base`，属于 scope 目录时 stage `runtime-directory`。
   前置观测不宣称与后续 open 原子。
3. **Socket** —— `client_socket_exists()` 复用既有 类型+属主+`0600`+no-follow 校验：不存在 →
   `service-unavailable`、stage `discovery`；无效/出错 → `check-failed`、stage `socket`。
4. **Admission** —— 紧邻 `LocalClient::connect_before` 之前建立一个新的 checked 5 秒单调
   deadline；同一 `Instant` 原样沿用到快照读取，绝不续期，绝不回退到 50 秒请求默认值。
5. **Read** —— 恰好一次 `submit_workbench_snapshot_read_before`，使用全新
   `WorkbenchRequestId`，受同一 deadline 约束。一次 OBLC 准入加一次 OBWP 请求；无重连、重试、
   publication 追赶或第二次读取。
6. **Release** —— 在任何映射、格式化、输出之前 RAII 释放全部自有 `LocalClient`/socket 句柄；
   慢 stdout 因此绝不持有 Session。不存在也不宣称 shutdown 交换；Server 可异步观察到 EOF。
7. **Map** —— 把已验证快照纯函数式地投影为命令结果。

`config check` 纯本地：不 connect socket、不取锁、不 mkdir/chmod；其 cohort 检查使用单独的
checked 5 秒预算。`server diagnose` 同属纯本地命令——只做描述符相对的路径与锁观察；绝不
connect、绝不创建锁或标记、绝不修复——共享一个在有界文件系统操作之间检查的协作式 5 秒
预算。Linux 和 Windows 以外的平台对网络查询与 mutation 产出 `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}`，对 `config check` 产出同 details 的
`check-failed`；不存在向无界连接 API 的回退。`server start` 在 Linux/Windows 以外、
`server diagnose` 在 Linux/Windows 以外的平台以同 details 的 `service-unavailable` 应答。

`agent doctor` 是离线 provider 钩子诊断：仅凭有界文件系统事实解释 Agent 观察为何可能无法
到达。它不执行 provider 命令、不 connect socket、不发现 Session、不取锁、不做任何写入——检查集
即上文声明的集合，每次 lstat/read/list/PATH 扫描都运行在同一个 5 秒协作预算下，至多 64 个探测
项、聚合 16 MiB 读取与单文件/计数上限，项间设取消检查点。Linux 和 Windows 以外的平台以
`check-failed` `{stage:"platform",reason:"unsupported-platform",command:"agent doctor"}`
应答。触及上限的运行仍产出文档并标 `complete:false`——部分事实是显式的，绝不静默丢弃——
`providers[]` 始终按规范次序报告每个所选 provider。`hints[]` 仅作观察：每个
`{id,evidence,text}` 指明其引用的具体事实行，不含因果、修复或信任断言。

### Windows Agent 诊断细化

Windows `agent doctor` 拥有相同的有界离线观察流程，输出 `orchbee.agent.doctor`
版本 3；Linux 保持版本 1，schema 继续保留原版本 2 词汇。Capsule 0.4.16 支持
原生 Windows Codex hook 传输，但 doctor 不连接命名管道，也不写 Console 关联 hint。
端点保持 `not-inspected`，原因改为 `windows-endpoint-not-inspected`；配置和胶囊
完整性不证明事件投递或 Session 存活。`complete` 描述探测覆盖率。
Windows 托管安装由 [Windows 集成契约](windows-agent-integrations.zh-CN.md) 负责；Windows Agent 启动见 [Windows Agent 启动](windows-agent-start.zh-CN.md)；prompt 组合契约见上文。

| 字段         | 契约                                                                                                                                                                                                                                                                               |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 输入与所有者 | 原生 doctor 独占负责只读文件系统、PATH 和环境观测。Windows home 使用原生用户配置目录；非空绝对 `CODEX_HOME` 指向现存非重解析目录时选择该 Codex 配置。空 `CODEX_HOME` 使用默认目录；无效非空根报告不可检查，不静默替换。该只读准入比 Codex 对相对路径或符号链接覆盖做规范化更严格。 |
| 准入         | 打开普通文件时不跟随末级重解析点，在有界读取前根据已打开句柄检查元数据。拒绝设备命名空间和相对路径；重解析条目不检查。祖先路径分量与同步文件系统延迟不属于硬截止时间保证。                                                                                                         |
| 决策         | provider 配置缺失、禁用、冲突或不可读，原生可执行文件或脚本可见或缺失，Node 可见或缺失，胶囊自洽、损坏或预算不足，以及 Windows 端点尚未检查，分别报告。PATH 搜索不执行条目，也不推断插件命令支持。                                                                                 |
| 兼容性       | Windows 接受原生绝对路径及 Windows 路径使用的单行 TOML 字面量字符串。既有 Linux 扫描语义和输出版本不变。嵌入 schema 将新增 Windows endpoint 词汇限定到版本 3；版本 2 保留原有封闭词汇。                                                                                            |
| 副作用与边界 | 不执行 provider、不连接 socket、不激活 Session、不加锁、不写入。沿用 5 秒协作预算、64 项、16 MiB 总读取量及各探测上限。取消返回 130；生成完整或部分诊断文档均返回 0。                                                                                                              |
| 证据与限制   | Windows 隔离进程测试覆盖本地化及 schema 合法输出、路径和配置隔离、缺失及已安装声明、损坏资源与零安装副作用。文件系统测试覆盖拒绝重解析点及读取预算。doctor 测试不证明 hook 投递；未完成独立评审；按用户要求不执行 TLA。                                                            |

`terminal read`、`agent read` 及 `terminal wait-output` 共享[原生终端输出](native-terminal-output.zh-CN.md)
拥有的当前来源选择和只读 viewer 生命周期。默认 recent（80 个物理行），`--lines` 控制有界范围，
`--source recent-unwrapped` 拼接已验证软换行。拒绝旧 history／rows／unwrap 参数。
历史 read／history 信封仍可按 Schema 读取，但不再是当前 CLI 模式。有限等待默认 30000 ms，
显式 `--forever` 才取消期限；`--after` 固定新输出边界，并拒绝重置／逐出。
中断、已知副作用保留及 detach 证据沿用[等待契约](native-cli-wait-model.zh-CN.md)。

`terminal send` 在命令入口采样一个绝对的 5 秒 deadline 并贯穿以下全部——声明型
`--file`/`--stdin` 的源物化（第一个有副作用的步骤，先于 cohort 检查）、cohort 检查在
自身预算与该上限的较早者之下运行、discovery、admission 与快照读取只消耗剩余额度——然后
在同一 deadline 内用同一已准入 session 执行恰好一次 `terminal.send` 交换。
`--text`/`--key` 调用完全没有物化步骤：

0. **物化**（仅声明型源）—— `--file`/`--stdin` 在任何运行时访问之前被完整读入
   下文定义的有界字节或粘贴内容（较小的 UTF-8 输入沿用文本记录路径）；见 [Send 输入源](#send-输入源)。源失败不拥有
   请求身份，也绝不抵达 Server。
1. **解析** —— 在已验证快照的 presentation 绑定中查找该 typed terminal ID；缺失 →
   `not-found`（3），details 为 `{resource:"terminal",target}`，session 随之 drop。
   绝不查询 label。
2. **发送** —— 一条 `terminal.send` 请求携带有序记录序列，在新的请求身份下使用
   快照中的 Session、Worker 与终端事实。`--text` 生成一条 UTF-8 记录（含可选 CR 共
   4096 字节）；`--file`/`--stdin` 生成一条不透明字节记录（含可选 CR 共 1 MiB），
   较小的 UTF-8 内容沿用文本路径。`--paste` 改为生成一条粘贴记录（256 KiB）及
   可选的独立 Enter 记录。重复 `--key` 按参数顺序生成最多 64 条语义键记录；键
   使用协议名称，不是预制转义字符串。在映射类型化结果或准备输出之前完整释放
   Client Session。包括部分接受在内，请求绝不重试。
3. **应答** —— Server 的 `terminal.send.result` 拆分为 envelope 逐字报告的两条轴：
   `inputStatus`（`accepted` 表示 Host 已把输入记录入队——绝不表示子进程已读取或执行；
   `rejected`/`not-submitted` 表示该记录可证明从未到达队列；`unknown` 表示其可能
   已经落地）与 `cleanupStatus`（`not-needed`/`confirmed`/`unconfirmed`，其中
   `unconfirmed` 表示临时 input-only attachment 可能存续至 carrier 拆除或 Host-epoch
   结束——绝不宣称已释放）。发送经由共享的 Worker↔Host 连接：当该连接失效、或其
   清理无法确认时，同一 Host 上的其它 viewer 可能被中断——这是底层连接的
   transport 级副作用，并非正常 send 夺取控制权。结果的物理 `target`（`workerGeneration`、`hostEpoch`、
   `hostTerminalId`）是 Server 实际准入的 incarnation，逐字投影；准入前拒绝不携带。
   退出映射：`accepted`+`confirmed` → 0；`accepted`+`unconfirmed` → 5 并附勿重发警告
   （输入可能已生效）；`rejected` → 3；`not-submitted` → 4，唯协议故障 reason
   `host-protocol-fault` 属 transport 类缺陷 → 5；`unknown` → 8 并附
   绝不重试警告。每个非零应答都保持 `inputStatus` 诚实——已接受的输入绝不改写为未发送。

`worktree list` 共享网络前导的第 1–4 步外加一次快照读取，全部处于命令入口一次性采样
的同一个绝对 5 秒 deadline 内（`snapshot_session_until`——cohort、discovery、socket、
admission、快照与 worktree-list 交换各自只消耗剩余额度；不做按阶段续期）。随后：

1. **解析** —— `--directory` 是严格的全局 Directory ID，无需 Project；不做本地文件系统探测。
2. **读取** —— 在同一准入 session 和总 deadline 内提交一次 `workbench.worktree-list.read {directoryId}`。Server 验证全局 Directory，再调度一个有界 Worker Git 发现任务。该任务共享五秒 TTL 和 64 KiB stdout 预算；CLI 不重试、不重放、不重连。
3. **映射** —— 先释放 session，再投影 `orchbee.worktree.list` v2：`{directoryId,path,entries}`，不含 Project 身份。条目保留原始 Git 字段及字节替代形式。交换失败使用 `orchbee.cli.failure.worktree`，不存在的 Directory 返回 `not-found`，不输出部分条目。

`launch-profile list` 与 `launch-profile get <selector>` 共享网络前导的第 1–4 步外加
一次快照读取，全部处于同一个 5 秒 deadline 下，然后：

1. **读取** —— 在同一已 admitted session 上、同一 deadline 内、一个全新
   `WorkbenchRequestId` 下发出恰好一次 `workbench.launch-profile-catalog.read` 请求
   `{selector|null}`。selector 逐字透传；Server 在目录锁下解析（先精确 profile ID，
   再按名称基数）。该读取绝不重试、重放或重连；不可判定的交换按失败矩阵映射。
2. **映射** —— session 在类型化 `launch-profile-catalog.result` 投影之前完全释放。
   `Catalog` 产出 `orchbee.launch-profile.list` 文档
   `{catalogRevision,profiles:[{profileId,name,launch}]}`（至多 64 条，逐字）；
   `Profile` 产出 `orchbee.launch-profile.get` 文档
   `{catalogRevision,profile:{profileId,name,launch}}`。该交换产出的每个失败——含
   transport 与 protocol 缺陷——都携带专用的
   `orchbee.cli.failure.launch-profile` envelope；
   `not-found{resource:"launch-profile"}` 是 selector 未命中的应答，`ambiguous`
   携带受限候选 ID。两个成功 envelope 都携带
   `source:{kind:"launch-profile-catalog",siteId, catalogRevision}`——目录私有
   revision，与 Workbench 导航 revision 是不同的计数。

### Send 输入源

Windows 管道读取拥有一个隐藏、非交互的 Windows PowerShell 读取进程，
从操作系统系统目录解析。仅传入继承的 stdin 句柄，来源字节不会变成脚本
或参数文本。固定脚本最多向私有输出管道复制来源上限加一个探测字节。
CLI 在现有期限和中断守卫下轮询，并在成功及已处理的失败路径终止和回收子进程。
不修改调用者的管道模式，也不遗留阻塞的读取线程。普通文件 stdin 保留
直接读取方式。进程启动和终止属于操作系统调度边界，不提供硬实时保证。

适配器使用已准入的 `winsafe` 系统目录和 `winapi-util` 句柄类型绑定，
沿用锁定版本，不引入自编写的不安全代码。状态为已接纳 -> 读取中 ->
完成/失败/超时/取消 -> 子进程已回收，仅完成可进入提交。
`source/input.rs` 拥有原始字节/paste 路由，文件/stdin 不限定为 UTF-8。
Windows 测试用真实读取子进程验证 EOF、上限和超时，并注入信号观察验证取消；
这些测试不模拟物理 Ctrl+C。此流程采用实现测试。

`--file` 与 `--stdin` 是声明型本地源。语法校验先于源读取；只有完整有界来源
存在后才进行 cohort 发现、准入、快照和提交。较小的合法 UTF-8 内容保留文本
记录路径，其他内容成为原始字节。Paste 内容成为一条 paste 记录，搭配
`--enter` 时再附加独立的尾随 Enter 记录。

- **纯输入边界。** 四种来源互斥，只有 `--key` 可重复，最多 64 条记录。
  `--enter` 不能搭配按键，`--paste` 必须搭配文件/stdin。
  准入前源失败不提交任何内容，也不拥有请求身份。
- **单一共享预算。** 命令入口采样的绝对 5 秒 deadline 覆盖源 setup、物化与整个 send
  生命周期；绝不重置或续期。每次源等待与每次读取之前都检查 deadline——包括恰在
  deadline 处结束的等待之后的读取——成功边界本身同样受仲裁：在预算关闭之后才
  观察到的输入结束是 `timeout`，绝非干净记录；过期之后不再发生任何运行时准入。
- **Linux 文件准入。** `--file` 以 `O_RDONLY|O_NONBLOCK|O_CLOEXEC` 打开——open 本身不会在
  FIFO 或设备上阻塞——随后在已打开 fd 上的 `fstat` 必须报告 regular file，这同时闭合
  路径替换竞态：被换掉的路径仍报告其真实类型。打不开/读不了的路径、非 regular 结果
  （目录、FIFO、设备或被替换的项）、空文件或读错误都是源阶段 `invalid-usage`。
- **Linux stdin 准入。** `--stdin` 拒绝终端描述符，再按继承描述符的精确 `S_IFMT` 类型分派：
  regular 文件在借用描述符上原位、无缓冲读取——共享文件位置恰好按返回字节数推进，
  至多 cap+1 探测字节（这正是继承位置契约）；
  FIFO/管道经 `/proc/self/fd/0` 重开为私有非阻塞文件描述，因此绝不改动调用方描述符
  旗标、也无需恢复；重开结果必须验证为 FIFO，其他一切类型（socket、设备等）一律拒绝。
- **有界记录。** 文件/stdin 最多读取 1 MiB 字节上限加一个探测字节，或
  256 KiB paste 内容上限加一个探测字节。原始字节（含 NUL 和非 UTF-8 数据）
  原样保留，不裁剪、不去 BOM、不归一 CRLF。空输入被拒绝。可选附加回车
  计入非 paste 上限，paste 则使用独立 Enter 记录。字面 `--text` 保留连同
  回车 4096 个 UTF-8 字节的上限。载荷不会出现在诊断中。
- **源信号 guard。** 在 Linux 上源阶段在其存续期内拥有进程信号捕获——单线程、无其他
  信号 owner、交接前不 spawn 子进程。合格集为 {SIGHUP, SIGINT, SIGQUIT, SIGTERM}
  减去进程在源入口已阻塞或忽略的全部信号：忽略集取自严格有界的
  `/proc/self/status` `SigIgn` 字段（最多读 64 KiB，须恰好一个合法十六进制字段——
  缺失、畸形、重复或超限均在任何源 I/O 之前报 internal failure），阻塞集取自当前
  线程 mask。Setup 先为合格集创建 `NONBLOCK|CLOEXEC` signalfd——此时继承 disposition
  仍然生效，故该处 setup 故障无需恢复任何东西——随后原子交换线程 mask 并捕获旧
  mask；捕获区间自交换起、先于任何源读取。绝不触碰 disposition，继承的 blocked/
  ignored 状态原样保留——空合格集是真实的 armed 策略（无可捕获），不是 no-op
  回落；继承的 pending 信号在原 mask 下保持 pending。
- **仲裁与最终化。** 读取循环把信号描述符与源描述符并列 poll，在处理就绪数据之前、
  判定超时之前、以及完成边界再各 drain 一次；首个被消费的合格信号选定源取消，且
  优先于 deadline 与源 I/O 错误。物化收尾以最后一次非阻塞信号读为线性化点：读到即
  取消；`EAGAIN` 表示源不再拥有取消——信号描述符关闭、旧 mask 在运行时准入之前被
  精确恢复。该读之后到达的信号保持 pending，在 unmask 时按原 disposition 交付——
  guard 绝不吞掉它——故迟到信号仍可能在准入前以默认行为终止进程。每个受监视
  syscall 都被检查：描述符创建、mask 交换、每次信号读与恢复；出错即 internal
  failure，绝不当作"无信号"；恢复失败是终态且无运行时准入。取消路径上合格 mask
  保持阻塞直到公共 `main` emit 完成输出尝试，再在同一路径恢复旧 mask——之后的
  信号仍可能在输出完成后终止进程，不对此承诺退出码。
- **失败兼容。** 校验与本地原因留在标准 `invalid-usage` 路径（无 details）。源超时与
  取消携带 `orchbee.cli.failure.source` v1 envelope——`{stage:"input",
inputSource:"file"|"stdin"}`，仅超时另带 `timeoutMs`（配置的共享预算，绝非实测
  时长）——无权威 `source`、无请求身份，因为尚无任何准入。源取消经与其他输出
  相同的公共 emit finalizer 按惯例 `128 + signo` 退出（129/130/131/143）——状态
  覆写仅存在于该 envelope——源超时仍为退出 7。
- **已声明的 OS 边界。** deadline 是每个 syscall 周围的逻辑界限，不进入其内部：内核
  卡死的 regular-file 读不可抢占，故不承诺该处及时终止。mask 交换前的 pre-arm 窗口
  在继承 disposition 下运行——该处默认死亡是诚实且无副作用的，不是 guard 窗口
  漏洞，且任一种 arming 次序都不能闭合全部信号到达竞态。继承的 `SIG_IGN`
  disposition 被尊重而非覆盖。以上均为依托 procfs 的 Linux-only 声明，与 procfd
  输入准入一致。

`agent list`、`agent get` 与 `agent explain` 沿用与每个网络查询相同的
cohort → discovery → socket → admission 前导，但共享一个在调用入口采样一次的绝对
5 秒命令预算：前导、准入、可选名称解析与 OBAP 读取共用同一个 ceiling，阶段之间绝不刷新——
预算耗尽的后续阶段不会启动，在 ceiling 上或其后完成的步骤结算为命令自身的
timeout。它们完全跳过 Workbench 快照读取。它们在唯一已准入的 Client Session 上
必要时先完成上述名称解析，再执行一次 Agent Observation Protocol（OBAP）读取交换：

Apple Silicon macOS 开发构建目前仅通过此路径提供不带 `--client` 的 `agent list`；`get`、
`explain`、`wait`、`changes` 与 Client 投影仍遵循各自文档中的平台范围。

1. **请求**——一个 OBAP 帧（`OBAP` magic，version 1），使用全新铸造的
   `ObapRequestId`：`agent.session.list.request`（kind 1）携带 `includeEnded`、
   `limit` 与可选 `cursor`（首页省略——绝不序列化为 `null`，线上显式 `null`
   亦被拒绝）；`agent.session.get.request`（kind 3）或
   `agent.session.explain.request`（kind 5）携带已解析的 `sessionId`。编码后的
   请求以 16 KiB 为界。
2. **结果**——恰好一个匹配的结果帧（kind 2/4/6），list 以 1 MiB 为界、
   get/explain 以 128 KiB 为界；小型拒答走 ≤1 KiB 界限。帧的 `requestId` 必须
   等于所发身份，结果 kind 必须匹配所发 operation——任何失配均为
   `protocol-invalid`、stage `agent-read`。
3. **释放**——Client Session 在任何结果映射或输出准备之前完全 drop。提交后
   不可判定的结果会 retire 该 session 且绝不重放：不重连、不二次准入、不重试。

在 Linux 上，Agent 读取命令为整个 armed span 拥有一套独立、更窄的信号
watch——仅 {SIGINT, SIGTERM}，在任何运行时接触之前通过源阶段所用的同一套
私有 arming 机制 arm：合格集排除进程已 block 或 ignore 的每个信号，借用的
信号描述符并入每次准入与交换等待，精确的旧 mask 在输出期间保持屏蔽、随后
由同一个不可克隆的 finalization 令牌恢复。arming 运行之前，进程信号行为
保持继承；SIGHUP 与 SIGQUIT 在整个调用期间绝不 mask、绝不捕获——完全保留
其继承 disposition，那里的默认原始死亡是诚实结果、CLI 不予上报；继承为
ignored 或 blocked 的成员只是离开合格集（空集仍是真实的 armed 策略）。
arming 本身就确立了取消所有权——与准入竞速无关——因此被捕获的信号无论在
准入期还是读取期落地，都将本次调用结算为 `orchbee.cli.failure.agent` 文档
上的 `cancelled`：`{stage:"agent-read", operation}`、无关联 source，任一
成员的退出状态均为 130。130 对 SIGINT 与 SIGTERM 是刻意统一的——不是
`128 + signo`，信号身份不予暴露。在唯一的 finish 仲裁上信号优先于
deadline、transport 故障与准入结果，且该仲裁仍只在已准入 session drop
之后运行。在 clear 仲裁下幸存的临时 wake 观测判为 `internal-failure`，
绝不判为 transport 或 timeout。

语义应答映射到 `orchbee.cli.failure.agent` 文档下的固定失败类别：
`listed`/`found` 是仅有的成功 outcome；`rejected{read-denied}` → `denied`；
`rejected{not-found}` → `not-found`（仅 get/explain，指名该 session）；
`rejected{cursor-invalid|cursor-stale}` → `rejected`（仅 list）；`unavailable` →
`service-unavailable` 且 `reason:"runtime-unavailable"`。交换内 deadline 耗尽为
`timeout` 且 `timeoutMs:5000`；transport 故障为 `transport-loss`；畸形、失配或
correlation 不符的帧为 `protocol-invalid`。准入后的全部失败携带
`{stage:"agent-read",operation:"list"|"get"|"explain"}`（wait 命令各臂上为 `"wait"`）；相关联的 `agent-read`
source 只附在可信语义应答上——transport、timeout、protocol-invalid 与
cancelled 失败均不携带。Agent 读取只观察 Server 的 canonical Agent session；`ended` session
被如实报告、绝不复活，`--include-ended` 放宽的是页过滤器而非生命周期本身。

`agent wait` 持有一个已准入的 Client Session。Windows 按上述 Agent 等待策略读取规范
变更及当前明细；Linux 保留 Get 轮询。每次交换都有新的请求身份，历史字段绝不覆盖
当前明细。在命令开始时采样的可选绝对单调 deadline 覆盖准入、每次读取
与每次退避；`--forever` 只省略该总期限；每轮在 `min(5 s, remaining total)` 下运行。两次未命中观察之间，循环睡一个
可中断的私有退避：起始 50ms、每次静默轮翻倍、封顶 500ms 且受剩余预算约束——
信号 wake 描述符并入该睡眠，持有的 session 在唯一的 finish 仲裁之前 drop，
与单次读取完全一致。retire 或结果不可判定的交换之后绝不重新准入、重放或新开
session。

每份返回的文档在客户端按请求顺序求值。三个新鲜度条件（`fresh`、`stale`、
`disconnected`）直接匹配观察的 freshness；每个状态条件还要求 `fresh` 新鲜度
；原生状态条件匹配 status `kind`，`blocked` 和 `done` 使用上述组合。多个条件可命中同一份文档——`matchedConditions`
按 `--until` 的请求顺序回显。没有基线或 poll-0 变更语义：第一次读取即可立即
命中。`--after-revision` 是调用方给出的全局修订栅栏——不是回合级或事件因果
身份。栅栏大于首个返回 revision 时，命令在恰好一次观察后以退出 3 拒绝
（`rejected`、`revisionAheadDetails`）；revision 单调，故该拒绝不可能在更晚
触发。任何一轮返回 `rejected{not-found}`——首轮或等待中——都是 `not-found`
（3）；`read-denied` 是 `denied`；`unavailable` 是 `service-unavailable`；
各自携带其应答轮的相关联 `agent-read` source。

两类不同的 deadline 都以退出 7 的 `timeout` 落在
`{stage:"agent-read", operation:"wait"}` 下。`wait-deadline` 是共享绝对预算
耗尽——步骤后结算优先于任何迟到的命中，预算之后到达的条件绝不发布。
`read-deadline` 是某一轮自身 `min(5 s, remaining)` 的界限在总量仍有余量时触发：
持有的 session 被该交换 retire 且绝不重新准入，details 记录实际准入的
`readBudgetMs`——文档不声称所求条件在整个等待期间缺席。timeout details 携带
配置的 `timeoutMs`、单调 `elapsedMs`、`completedObservations` 以及最后一次
完成观察的 `lastStatusKind`/`lastFreshness`/`lastRevision`——零次完成观察时
全部省略。armed span 内的取消同样是 `cancelled` 130 且无 source；准入失败
保持通用失败族。

现有 Linux 等待在每次完成的轮询处采样当前投影状态——它不是订阅，也不提供变更通知。
在两次已完成观察之间开始又结束的跃迁会被错过：待处理请求可能已清空，turn
outcome 可能被后来的 turn 替换，freshness 在轮询之间移动。错过窗口不只由
轮询间隔决定——Server 读取时长与调度同样会拉宽它——即便对一旦达成就保持
为真的条件，也不存在 sticky-state 保证。命中时发出的 `orchbee.agent.wait`
envelope（version 2）携带 `matchedConditions`、命中的 `revision`、完整的
`session` 明细逐字段，以及 `wait` 回显 `{until, afterRevision?, timeoutMs, observations, elapsedMs}`；其 `source` 仅为决定性轮询的 `agent-read`
requestId——失败、取消与不可信交换均不发布。text 输出为单行：
`session <id> matched <conditions> at revision <n> after <ms>ms`。

## Session 激活

`server start` 是 TUI 本地启动的无头形态：两者调用同一个共享 connect-or-activate 核
（`src/local_activation.rs`）——序列完全一致，仅失败投影不同。一个 20 秒绝对 deadline 在
cohort 验证之前开始，且绝不按阶段重置：

1. **Cohort** —— 先执行 `NativeCohort::current(deadline)`，先于任何运行时目录工作。其
   typed io kind 分类：`InvalidData` → `incompatible`，`NotFound` → `service-unavailable`，
   `PermissionDenied` → `denied`，`TimedOut` → `timeout`，其余一切 kind →
   `internal-failure`——均带 `{stage:"cohort"}`。
2. **Prepare** —— 解析 `ORCHBEE_RUNTIME_BASE`/默认值，推导 `orchbee-native-<euid>`，然后
   以 create-if-missing 模式执行 `prepare_mode_checked`：即使调用稍后被拒，运行时目录也可能
   已被创建。Rendezvous 错误与查询的 discovery 阶段完全同规则分类。
3. **Socket 预算** —— 每个派生 socket 路径必须适配本平台 `sockaddr_un`；超预算路径在任何
   取锁或准入尝试之前即 `service-unavailable`
   `{stage:"runtime-directory",reason:"address-too-long"}`。
4. **Connect or activate** —— 尝试一个已准入 `LocalClient`（socket 缺失或拒连在 deadline
   内可重试；其他 connect 失败逐字沿用查询的 `admission` 映射）；取激活锁，复查 socket，
   读标记，取 Session 生存期锁；当标记或 socket 显示无存活属主时拒绝
   `rejected`/`{stage:"activation",reason:"recovery-required"}`；提交标记，按已验证 cohort
   spawn `orchbee-server --local-site <runtime-directory>`，随后轮询 connect 加 `try_wait`
   直至就绪。spawn 或 `try_wait` 失败同样是 `recovery-required` 拒绝——标记可能已提交，故
   答案绝不是干净超时声明。
5. **Probe** —— 已准入的探针 `LocalClient` 在输出前释放；成功不意味持有 session、grant 或
   Session 身份收据。

`result.outcome` 报告本次调用实际走的分支：`started` 仅当本次调用拉起了 Server 子进程，
`already-running` 当既有 Session 准入了其探针。Linux 的 `timeout` 答案意为就绪未能确认——标记提交之前
无遗留；提交与 spawn 之间可能残留无存活属主的标记（后续调用随即应答
`recovery-required`）；spawn 之后 Server 可能仍在运行。该命令绝不声称超时证明没有 Server
启动，绝不重试 `recovery-required`，不强制清理不确定的运行时。Windows 可能在标记提交前已发布新代；
下一次调用复查确切进程身份、登记和全部恢复 guard，只有证明死亡的代才能自动对账，并保留 checkpoint。信号保持继承：不存在捕获
阶段，中断即进程死亡，按惯例 `128 + signo` 退出，绝不产出 `cancelled` envelope。下文说明 Windows 显式停止和后续启动；没有修复动词。

## Session 诊断

Windows `server diagnose` 输出独立闭合的 `orchbee.site.diagnose.windows` v3，保留
Linux v1 文档。当前目录及版本见上文。复用已有运行时路径／SID scope，不进行 cohort
准入、激活、端点连接或进程发现。有序行描述选定基目录、已准入运行时目录、标记、
激活／生命周期锁及五个命名管道地址。Windows 不输出文件系统 Host socket scope 行，
也不虚构 POSIX mode／UID。`private` 只表示已检查当前用户归属和运行时 DACL 白名单，
不是持续有效的安全保证。

版本 2 区分现有标记的 `activation: active|stopped`；Stopped 另带 `stopRequestId`，
不记作缺失。只有 Active 附加 activation-recorded 通告。这是保留完成记录的观察，
不是新的进程健康检查；后续启动会消费旧停止回执。

每行给出 known/absent/uninspectable，生命周期锁另有 free/held/unsafe。父目录被拒绝、
无效或缺失时，不检查所有后代。绝不获取激活锁；生命周期探针只打开已有且通过准入的
文件，短暂尝试独占锁，可能与启动争用。每个端点最多枚举 2048 个命名空间条目；
管道名称未经身份认证，预算耗尽为 `observation-limit`，不记作不存在。五秒协作式期限
可产生部分覆盖，文件系统停顿除外。`complete` 只表示每行已尝试或由父状态导出，
不表示健康。所有句柄由 RAII 持有，Ctrl+C 终止进程时由系统释放。不修复、创建／删除
文件、启动／停止进程或重试。标记和管道通告只描述当前恢复策略。Windows 公共进程
测试覆盖 absent/held/free、非法标记、中文路径、schema、本地化文本和零创建；
runtime／IPC 测试覆盖准入、真实监听器和有界观察。

`server diagnose` 是激活的只读对偶面：它报告隐式 per-euid 本地 Session 运行时的有界事实，
不 connect 任何东西、不创建任何东西、不修复任何东西。运行时基座与 `server start`
完全同规则解析（`ORCHBEE_RUNTIME_BASE` 或 `/tmp`，scope `orchbee-native-<euid>`），
随后命令经由 rendezvous 自身已验证的目录描述符检查一组有限具名路径——描述符相对的
`fstatat` 加 `AT_SYMLINK_NOFOLLOW`；仅对 Session 生存期锁追加一次"仅存性打开"探针，
其 `try_lock` 结果在同一描述符上立即释放。激活锁仅为元数据（存在性/类别/属主/模式/
链接数），命令绝不获取它。任何端点都不 connect——`client.sock` 尤其如此，一次
connect 就会消耗一个准入槽——也不枚举、发信号或散列任何进程。

每一行都是封闭状态的事实文档，绝非健康判定：`runtimeBase` 与 `runtimeDirectory`
携带 `path`/`pathBytesHex`；`marker`、`locks:{activation,siteLifetime}`、
`endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}` 与
`hostScope` 各自报告 `state`（`known|absent|unsafe|uninspectable`；生存期锁另有
`free` 与 `held`），以及不完全已知时的具体 `reason`——`symlink`、`invalid-kind`、
`foreign-owner`、`invalid-mode`、`invalid-size`、`invalid-link-count`、
`invalid-lock`、`denied`、`io-error`、`invalid-path`、`identity-changed`、
`unsafe-parent`、`parent-unavailable` 或 `deadline`。`Present` 行携带判定所依据的
原始事实（`kind`、`ownerMatch`、`mode`、`size`、`linkCount`）。`held` 生存期锁
仅意为某属主在该观察瞬间持有锁——绝不意为 Session、Worker 或任何进程存活——`absent`
是路径名事实，绝不证明执行已结束。Host 目录先获准入，然后才检查 Host 端点名；父级
不可准入时端点行不穿透遍历地关闭为 `uninspectable`/`unsafe-parent`。`complete` 是
"已尝试或已分类"的覆盖度事实——仅当协作式 5 秒预算在操作之间耗尽时为 `false`——
绝非运行时健康声明。

`result.advisories` 携带现行激活谓词自身事实所支持的条件性通告：观察到完全有效的
标记时 `activation-recorded`；观察到完全有效的 client 或 worker socket 叶时
`socket-residue`——文本中分别注释为 "activation recorded" 或 "socket residue
observed; if no existing Session can be joined, fresh activation is blocked by current
recovery policy"。该通告在已观察事实上引述现行策略；它绝不预测非原子未来
`server start` 的结果——后者在读任何残留之前先尝试认证接入。

命令以完整或部分事实文档退出 0——unsafe 与 uninspectable 行是事实而非命令失败。
调用、引擎与输出准备失败经正常失败面应答（`invalid-usage`、非 Linux 平台的
`service-unavailable` `{stage:"platform",reason:"unsupported-platform"}`、
`output-too-large`、`output-failed`）；中断即按惯例 `128 + signo` 的进程死亡
（SIGINT 为 `130`）。一次临时诊断性生存期锁探针可能与并发激活自身的取锁竞争——
这是调用方应预期的有界瞬时效应；命令记录该效应而非声称零副作用。它绝不获取激活锁、
绝不创建或删除路径、绝不修复、绝不制造存活或终止见证。

## Canonical 数据与结果

Project 查询遵循 `navigation.projects` 顺序。Workspace 查询遵循 `workspaceOrder`，
缺省或空列表时回退为全局 `navigation.workspaces` 顺序。其他遍历遵循
`terminalContexts` 顺序接 `tabs` 顺序；panels 深度优先且 `children[0]` 先于 `children[1]`；
terminal 绑定按该 panel 顺序且 `primaryPresentation` 先于 `childPresentations`。不新增排序；
过滤后的列表与 `ambiguous` 候选都保持该次序。

快照与单效果命令封套使用 `{schema, version, source, result}`；分阶段流程有自己的内嵌 schema。每份 schema 定义发出的版本及严格形状。Workbench 查询版本见上文；`orchbee.panel.current` 在 Windows 为 v9（其他平台保持 v3），`orchbee.api.snapshot` 因全局导航、可空 Workspace 分组及可选分隔比例为 v7。`orchbee.terminal.get` 因保留实际启动程序／参数为 v4。旧 `orchbee.terminal.read/history` v1/v2 Schema 描述历史回执。当前读取输出 `orchbee.terminal.output` v3/v4，来源等待输出 v3/v4。各命令与失败的完整版本契约见 `api schema <id>`。目录版本及文档数量见上文：注册 ID 会改变闭合字典形状，与业务文档版本独立。来源保留各所有者的含义：

- `source.kind="workbench-snapshot"`：带同一份已验证快照的 `siteId` 与十进制字符串
  `revision`——仅在完整有效读取之后出现。
- `source.kind="workbench-command"`：带 `siteId`、`requestId` 与十进制字符串
  `expectedRevision`——已产出的 mutation 结果，以及有效读取之后 command 阶段的失败。
- `source.kind="embedded"`：随二进制编译内嵌的 schema 字典（`api schema` 结果及其
  `not-found` 失败）。
- `source.kind="local-runtime"`：已产出的 `config check` 结果、`agent doctor` 结果、`server start` 的每个
  应答，以及每份 `server diagnose` 文档——本地运行时是激活 outcome、拒绝、离线诊断与
  有界运行时观察的应答权威。
- `source.kind="terminal.send"`：`siteId`、send `requestId`，以及解析出该绑定的十进制
  字符串 `snapshotRevision`——出现在类型化 `terminal send` 应答上。
- `source.kind="terminal-launch-read"`：`siteId` 与十进制字符串 `revision`——Server 在
  coordinator 锁内 join 出的修订——出现在 `terminal get` 应答上。
- `source.kind="worktree-list"`：`siteId` 与 Server 权威 membership 核验时刻已提交
  Workbench 的十进制字符串 `revision`——出现在 `worktree list` 结果及其快照后失败上。
- `source.kind="agent-read"`：语义应答带回的相关联 `requestId`——出现在已产出的
  agent 读取结果与准入后语义拒答上。
- `source.kind="launch-profile-catalog"`：`siteId` 与已解析目录的十进制字符串
  `catalogRevision`——出现在 `launch-profile list`/`get` 结果及其会话后失败上。
- `source.kind="workspace-close-resume"`：`siteId`、resume `requestId`、所指
  `closeRequestId`，以及请求携带的十进制字符串 `expectedAttemptGeneration` 栅栏——
  出现在 `workspace close-resume` 应答上：保留 lifecycle 的 resume 没有
  `workbench.command`，resume 请求自身的回显即诚实来源。
- 不存在 freshness 字段；一次一致读取不等于持续活性保证。

结果载荷（schema ID 为稳定机器名）：

- `orchbee.status` → `{site:{reachable:true,siteId,revision},
counts:{projects,workspaces,tabs,panels,boundTerminals},
worker:{status:"unknown",reason:"not-in-workbench-snapshot"}}`。`boundTerminals` 统计带
  `binding` 的 presentation；快照校验已保证 terminalId 全局唯一，故每个绑定恰命名一个不同
  终端。`reachable` 只证明本次读取成功。文本视图仅在快照成功读取后报告 Session 正在运行，
  随后列出本地化的资源计数。它省略内部 Site ID、快照修订号和 Worker 观察详情，也不声称
  Worker 或 Agent 已就绪。JSON 文档保持不变。
- `orchbee.project.list` → `{projects:[{projectId,label,directoryCount,workspaceCount}]}`；
  `orchbee.project.get` → `{project:{projectId,label,directories:[{directoryId,workerId,path}],
workspaces:[{workspaceId,label}]}}`。
- `orchbee.workspace.list` → `{workspaces:[{workspaceId,label,number,tabCount,panelCount,projectId,
primaryDirectoryId|null,terminalContextId}]}`，来自全局导航，`projectId` 必填可空；`workspace get` 增加经
  `terminalContextId` 反查的 `tabs:[{tabId,label}]`。
- `orchbee.workspace.close-status` → 逐字的保留 `WorkbenchWorkspaceCloseReadOutcome`：
  无覆盖该 scope 的保留操作时为 `{kind:"not-closing"}`，否则
  `{kind:"available",closeRequestId,workspaceId,terminalContextId,
attemptGeneration,status,counts:{closing,cleanupBlocked,outcomeUnknown,
disposedAwaitingPublication},members:[{tabId,panelId,presentationId,status}…]}`。`status`
  是 scope lifecycle（`kind` 为 `closing`|`cleanup-blocked`|`outcome-unknown`|
  `disposed-awaiting-publication`|`closed`，后者带 tombstone `revision`），每个 member
  的 `status` 是捕获时的逐成员 lifecycle 事实、不含 `closed`（closed tombstone 仅属
  scope 级），`attemptGeneration` 是十进制字符串的尝试栅栏；Workspace 离开 active
  projection 后该记录仍可应答。
- `orchbee.tab.list` → `{tabs:[{tabId,label,number,panelCount,pinned,groupId|null,
context:{kind:"standalone",terminalContextId}|{kind:"workspace",terminalContextId,
workspaceId}}]}`；standalone 上 `workspaceId` 键不存在而非 null。`tab get` 增加 `root`——
  typed `PanelNode` 子树的语义化重编码，不丢字段。
- `orchbee.panel.list` → `{panels:[{panelId,label,customLabel|null,displayLabel,workspaceId|null,tabId,terminalContextId,primaryPresentationId,
presentationCount,primaryTerminalId|null}]}`，`primaryTerminalId` 仅反映
  `primaryPresentation.binding`；`panel get` → `{panel:{panelId,tabId,context,slotPath,
primaryPresentation,childPresentations}}`，`slotPath` 为自 root 至该叶子的 `PanelSlot` ID
  链（root 本身即 panel 时为 `[]`），每个 presentation 为 `{presentationId,label,
binding:{terminalId,workerId}|null}`。
- `orchbee.terminal.list` → `{terminals:[{terminalId,workerId,boundTo:{presentationId,panelId,
tabId,terminalContextId}}]}`；JSON 行只包含身份，不含 lifecycle、process 或 Agent 成员。
  `--workspace`、`--tab` 与 `--panel` 至多选一个，只保留绑定在该已解析成员内的终端，并遵循标准
  筛选规则：成员不存在时为未找到，成员存在但没有终端时为空列表。
  Windows 文本输出为对齐的 `HANDLE`、`TITLE`、`AGENT`、`STATE`、`WORKSPACE`、`PANEL`、`CWD` 表格。
  其他平台保留相同身份表格，无法观察的运行时列显示为 `-`。
  `HANDLE`、`WORKSPACE` 与 `PANEL` 使用稳定的 `@<kind>:0x<hex>` 精确身份，
  筛选和其他成员变化不影响解析，全部有效数字保持可复制。
  独立终端的 `WORKSPACE` 列显示本地化的独立上下文名称。仅文本的补充数据在快照、精确
  Presentation 报告批次和最多 64 次精确绑定的 Host viewer 读取之间共用一个五秒期限与取消
  监视；其余行的补充字段保留为 `-`。`TITLE` 是已清理的 Host 帧标题；若标题只是指向
  `.exe`、`.com`、`.cmd`、`.bat` 或 `.ps1` 程序的绝对路径，则显示该程序名。只有 `TITLE`、
  `AGENT` 与 `STATE` 单元格会为适应 80 列而缩短；句柄和 `CWD` 绝不截断。`CWD` 使用经校验的
  终端帧目录，并在其缺失时回退至观察到的根进程目录；不会用 Workbench Workspace 路径代替。
  `AGENT` 优先使用精确 Presentation 报告，其次使用有界前台进程识别；`STATE` 保留已知的
  Presentation 报告状态。已识别 Agent 但没有已知报告状态时显示 `unknown`（未知）。否则，在
  观察到 Host 帧时显示 `observed`（已观察），这不表示 Shell 已准备好接收输入；两项事实都没有时
  显示 `-`。缺失的可选
  观察显示为 `-`；拒绝、传输、超时、取消和协议错误仍会使查询失败。每个已附加 viewer 在观察
  成功或失败后都会尝试 best-effort detach。文本保留可打印的路径字符并转义终端控制字符。
  `--format json` 继续使用原有的纯快照路径，结果形状和 schema 版本不变。`terminal get` 在
  `terminal` 下返回同一身份事实，其中嵌套一个
  `launch` 成员——文档形状为 `{terminal:{terminalId,workerId,boundTo,launch}}`，绝不出现兄弟
  `result.launch`。只有被 presentation 绑定的终端可见；绑定存在不证明进程存活。与
  `terminal list` 不同，`terminal get` 不从 Client
  持有的快照作答：它在已准入 session 上发送一次 `terminal.launch.read` 请求，由 Server 在
  coordinator 锁内解析当前绑定、路由与 retained launch fact——快照只提供类型化目标 id。
  其 envelope 为 `version:4`，`source:{kind:"terminal-launch-read",siteId,revision}` 的
  `revision` 是 join 所得 workbench 修订的十进制字符串；旧的 `version:1` 校验器会拒绝它，
  这是刻意的兼容性边界。`launch` 为 `{state,workerGeneration,createOutcome,geometry}`：
  `state` 逐字呈现 retained phase——`creating`、`awaiting-start`、`starting`、`started`、
  `unknown`、`unavailable`——或在可见绑定没有可唯一归属且一致的记录时为 `not-tracked`；
  `workerGeneration` 为正的十进制字符串，仅在 `not-tracked` 时为 `null`；`geometry` 仅在
  `started` 时非空；`createOutcome` 照录 retained create outcome，在 `creating` 与
  `not-tracked` 时为 `null`。当启动命名了一个目录 profile 时，终端还多一个兄弟成员
  `launchProfile:{profileId,catalogRevision}`——逐字呈现的已解析 provenance，显式
  `--exec` 与默认 shell 启动下缺席（绝非 `null`）；envelope 为 `version:4`。state
  是 retained 事实，绝非进程存活：`creating` 只记录
  phase、不证明 dispatch 已发生；deferred-start 失败如实读作 `unavailable`，不虚构 start
  原因。退出码保持查询契约——`not-found` 为 3、`server-busy` unavailable 为
  4——`found` 应答即使 `launch.state` 为 `unavailable` 且 `createOutcome.kind` 为
  `failed` 仍退出 0，因为查询本身成功。该交换承载于已准入 session，与准入及快照读取共享
  同一份 5 秒网络预算：断开或耗尽的交换是 `transport-loss` 5，correlation 失配或无效结果是
  `protocol-invalid` 5——launch read 绝不报告 `timeout`。
  显式程序或命名 profile 启动还返回可选同级 `launchSelection:{exec,args}`，保留创建时冻结的实际程序和参数；profile 删除或 Panel 移动不会重写它。默认 shell 与 `not-tracked` 省略该成员；不暴露环境变量。文本输出用本地化标签显示程序及 JSON 参数数组。此查询不保证跨重启保存启动命令。
- `orchbee.terminal.read` → `{terminal,frame,content}`：`terminal` 携带同样的
  `{terminalId,workerId,boundTo}` 身份事实；`frame` 携带已应用投影的
  `{revision,generation,sequence}`（十进制字符串，u64 无损）、`alternateScreen`、`geometry`
  `{columns,rows}` 与 `cursor` `{row,column,visible}`；`content` 携带 `rows`（发出的网格行，
  形如 `{index,text,continuation:{fromPrevious,intoNext}}`——`text` 拼接该行各 cell，
  丢弃宽字符占位尾并裁剪网格填充）、`rowCount`（已应用帧持有的全部行数）、
  `truncated`/`omittedRows` 与 `limits` `{rows:4096,textBytes:4194304}`。`--unwrap` 下
  `rows` 成员被 `lines` 替代——每条为 `{firstRow,lastRow,text,startsMidLine,endsMidLine}`
  的拼接逻辑行或带标记的窗口边缘片段——并附 `lineCount`；`--lines` 下附 `tail` 成员报告
  `{requested,omitted,entries:"rows"|"lines"}`。发出的行受累计 4 MiB
  JSON 编码文本预算约束；超出预算的行计入 `omittedRows` 且 `truncated` 为 true。text 输出下每
  个发出的行（或逻辑行）渲染为一行转义文本，`tail.omitted>0` 时追加本地化尾部标记，
  `omittedRows>0` 时末尾追加本地化截断标记。本次读取是单一
  时间点帧：`revision`/`generation`/`sequence` 是宿主在该观察时刻的计数器，不是 CLI 的
  freshness 声明。
- `orchbee.terminal.history` → `{terminal,history,content}`：`history` 携带已定价窗口的
  事实——`revision`/`generation`、`alternateScreen`、`columns`、`screenRows`、十进制字符串
  `scrollbackRows`/`totalRows`/`startRow`、`requestedRows` 与 `windowRows`（Host 窗口的完整
  行数）；`content` 携带 `rows`（`{index,text,continuation}`，`index` 为"保留区+屏幕"绝对行
  坐标的十进制字符串）、`rowCount`（发出的行数）、`truncated`/`omittedRows` 与 `limits`
  `{rows:1000,textBytes:4194304}`——并做与 `terminal read` 相同的 `--unwrap`/`--lines`
  替换（`lines` 的 `firstRow`/`lastRow` 为十进制字符串，附 `lineCount` 与 `tail`）。
  `omittedRows` 诚实计为 `min(requestedRows,scrollbackRows)` 减去已发出行数——即被 `MAXIMUM_FRAME_CELLS/columns` 上限或文本预算截掉的
  行——而早于 `startRow` 的行从未被请求，不算遗漏。text 输出与 `terminal read` 一致：每个
  发出的行渲染为一行转义文本加标记。
- `orchbee.terminal.send` → `{action:"terminal.send",requestId,siteId,workerId,terminalId,
payloadKind,records,recordBytes?,recordsAccepted,firstStoppedRecord,acceptedEncodedBytes,
inputStatus,cleanupStatus,reason?,target?}`. `payloadKind` 为 text、key、bytes、paste 或 sequence；
  `records` 为提交记录总数。`recordBytes` 仅表示单条内容记录的字节数（text/bytes 可含折入的 CR），
  不猜测后续语义按键的编码字节数。`recordsAccepted`、可空的零基 `firstStoppedRecord` 和
  `acceptedEncodedBytes` 原样报告已确认进度；回复丢失的失败 envelope 不伪造这些字段。
  `inputStatus` 包括 accepted、rejected、partial-accepted、not-submitted、unknown；
  `cleanupStatus` 独立报告清理结果。`reason` 为类型化拒绝/不可用原因。可选 `target` 是 Server
  实际准入的物理实例（`workerGeneration`、`hostEpoch`、`hostTerminalId`）。回执及诊断绝不回显
  提交内容。旧 `key` 和 `acceptedBytes` 字段不是当前序列回执的一部分。
- 在非 Windows 规范路由上，`orchbee.agent.list` v1 逐字返回 `listed` outcome：
  `{kind:"listed",items:[<session summary>…],nextCursor|null,revision}`，每个 summary
  即 wire `AgentSessionSummary`（session/provider/lifecycle/语义状态/freshness/观察
  计数/执行位置/pending 请求计数 ≤128/presentation 附件），`nextCursor` 为 Server
  铸造的续页 token，供 `--cursor` 使用。
- 在非 Windows 规范路由上，`orchbee.agent.get` 逐字返回 `found` outcome：
  `{kind:"found",revision,session}`，`session` 即 wire `AgentSessionDetail`——
  在 summary 字段之外增加 `capabilities`（≤10 个去重 token）、`lastOutcome` 与
  `pendingRequests`。
- `orchbee.agent.explain` → 逐字的 `found` outcome：
  `{kind:"found",revision,session,explanation}`，`explanation` 为 canonical 11 字段
  对象（`status`、`statusBasis`、`freshnessMeaning`、`capabilityCeiling`、
  `fidelity`、`freshness`、`generation`、`sequence`、`source`、`actionPath`、
  `contentAvailable`）——Server 的派生解读，绝非 CLI 推断。
- `orchbee.agent.wait` → 命中结果 `{matchedConditions,revision,session,wait}`；`matchedConditions` 是
  有效 `until` 的命中子集，按请求顺序返回（1–14 个唯一条件）。`revision` 与 `session`
  是当前 Get 样本的修订及原始 `AgentSessionDetail`。Windows v3 另带可空 `matchedChange`，
  将历史匹配证据与该样本分开；`wait` 回显有效的
  `{until,afterRevision?,timeoutMs,observations,elapsedMs}`，仅在提供时包含 `afterRevision`。
- `orchbee.api.snapshot` → `{snapshot}`：完整 `WorkbenchProtocolDocument::Snapshot` 的语义化
  重编码，含 `kind:"workbench.snapshot"`；envelope 的 `siteId`/`revision` 与内部文档一致。
- `orchbee.api.schema` → 带参 `{schema:{schemaId,document}}`；无参
  `{schemas:{schemaId:document,…}}` 列出全部已交付字典（draft 2020-12）。
  目录保持封闭：`result.schemas` 恰好命名已注册的 id，且每个内嵌文档的 `$id`
  与其键一致。按旧目录校验的 introspection 客户端从同一构建中选择内嵌 v2 文档。
- `orchbee.worktree.list` → `{directoryId,path,worktrees:[<entry>…]}`：已注册
  目录身份（`path` 为已注册 Workbench 目录——与每条目观察到的 `worktreePath` 相区分）
  加 `git worktree list --porcelain -z` 的逐字投影。每条目恰好携带 `worktreePath`
  （UTF-8）或 `worktreePathBytesHex`（小写十六进制原始字节）之一；`head` 为 40 或 64
  位小写十六进制对象名，bare 仓库为 `null`（全零 HEAD 是 unborn 仓库的真实 Git 值，
  原样保留）；`branch`（逐字完整 ref，如 `refs/heads/main`，绝不剥前缀）、
  `branchBytesHex` 或 `null`（无分支适用时）三者取一；外加 `bare`/`detached`/`locked`/
  `prunable` 标志。
- `orchbee.cli.failure.worktree` → `{schema:"orchbee.cli.failure.worktree",version:1,
failure:{kind,details?},source?}`：`worktree list` 越过快照边界后一切失败的专用
  envelope（边界前的语法、前导与 project 解析失败保持 `orchbee.cli.failure`）。其
  `details` 词汇封闭：`not-found` 为 `{resource:"directory",target}`；
  `denied`/`service-unavailable`（`server-busy`|`worker-unavailable`|`job-dead`|
  `job-unknown`）/`transport-loss`/`protocol-invalid`（`reason:"correlation-mismatch"`）
  为 `{stage:"worktree-list"[,reason]}`；`check-failed` 为 `{stage:"probe"|"list",status}`
  （`status` 为整数或 `null`）或 `{stage:"parse"}`；`timeout` 为 `{stage:"worktree-list", timeoutMs:5000}`；
  `output-too-large` 为 `{stage:"worktree-list",origin,limitBytes}`（`origin`
  `git-probe`|`git-list` → 65536，`worker-result` → 7936，`workbench-result` → 32768）。
- `orchbee.launch-profile.list` → `{catalogRevision,profiles:[<entry>…]}`：十进制字符串
  的目录私有 revision 加目录内全部条目（至多 64 条），按目录次序——
  `{profileId,name,launch:{exec,args}}`，全部逐字。
- `orchbee.launch-profile.get` → `{catalogRevision,profile:<entry>}`：Server 解析出的
  单个 profile，形状与列表条目相同。
- `orchbee.cli.failure.launch-profile` → `{schema:"orchbee.cli.failure.launch-profile",
version:1,failure:{kind,details?},source?}`：`launch-profile` 查询越过 session 边界后
  一切失败、以及 `launch-profile delete`、`tab create --profile`、`panel split
--profile` 上 profile 解析失败的专用 envelope（边界前的语法与前导失败保持
  `orchbee.cli.failure`）。其 `details` 词汇封闭：`not-found` 为
  `{resource:"launch-profile",target}`；`ambiguous` 为
  `{resource:"launch-profile",target,candidates≤8,totalCount}`（候选 ID 遵循
  profile-id 语法）；`denied`/`transport-loss` 为
  `{stage:"launch-profile-catalog"}`；`service-unavailable` 同形加
  `reason:"service-unavailable"`；correlation `protocol-invalid` 同形加
  `reason:"correlation-mismatch"`；`timeout` 为
  `{stage:"launch-profile-catalog",timeoutMs:5000}`。`source` 在读取失败时是
  `workbench-snapshot` 源、在 mutation 侧失败时是 `workbench-command` 源。
- `orchbee.mutation.result` → `{action,outcome,revision?,acceptedRevision?,closeRequestId?,
attemptGeneration?,<resolved typed IDs>,direction?,label?,terminal?,catalogRevision?,
profileId?,name?}`，
  按上文 mutation 矩阵：`committed`/
  `unchanged` 带 `revision`，`accepted` 带 `acceptedRevision`+`closeRequestId`，
  `resumed` 带 `closeRequestId`+`attemptGeneration`，
  `tab create`/`panel split` 带 `terminal:{outcome,reason?,geometry?}`。launch-profile
  动作在两份收据上都追加 `catalogRevision` 与 `profileId`，`launch-profile.create`
  另带 `name`；envelope 为 `version:4`。该收据及 Windows Workspace 与终端创建收据的文本输出共用
  一种本地化形状：先用一句话说明发生了什么，再以对齐行列出每个被创建或受影响的 Project、
  Workspace、Tab、Panel 与启动配置 ID，以及 label、目录和终端结果。修订、请求 ID，以及
  上下文、目录、split、slot 与 presentation ID 只出现在 JSON 中。终端已启动或正在启动的收据
  以针对新 Panel 的 `terminal list --panel` 提示结尾，因为拉起结果不携带终端 ID。`panel current`
  文本首行为当前 Panel ID。`workspace get`、`terminal get` 与 `panel process-info` 的文本同样
  先用一句话开头，再列出对齐的标签：Workspace 视图在任何 Agent 或 worktree 补充信息之前显示其
  Project 和主目录路径；终端视图显示所在 Panel、Tab 与启动状态；进程视图给出前台进程及其 PID、
  UTC 启动时间、目录和命令，并省略 Orchbee 为交互式 PowerShell 追加的 shell 初始化，随后列出
  所有采样进程。Worker、上下文、presentation 与目录 ID、原始启动结构以及 FILETIME 值仅出现在
  JSON 中。
- `orchbee.cli.failure.close` → `{schema:"orchbee.cli.failure.close",version:1,
failure:{kind,details?},source?}`：`workspace close-status` 与 `workspace close-resume`
  越过 admitted session 后一切失败的专用 envelope（边界前的语法、前导失败与
  `workspace close` 命令自身的失败保持 `orchbee.cli.failure`）。其 `details` 词汇
  封闭：`denied`/`transport-loss` 与裸 `protocol-invalid` 为
  `{stage:"close-status"|"close-resume"}`；`service-unavailable` 同形加
  `reason:"server-busy"`；`timeout` 为 `{stage:"close-status",timeoutMs:5000}`；
  已分类的 `protocol-invalid` 为 `{stage,reason}`、`reason` 取
  `correlation-mismatch`|`scope-mismatch`；`rejected` 为 `{stage:"close-resume",reason}`、
  `reason` 为类型化 resume 判定（`stale-generation`|`operation-unavailable`|
  `operation-mismatch`|`request-identity-conflict`|`generation-mismatch`|
  `not-resumable`|`generation-exhausted`）；`outcome-unknown` 为
  `{stage:"close-resume",requestId,reason}`。`source` 在 close-status 失败上是
  `workbench-snapshot` 源、在 resume 失败上是 `workspace-close-resume` 源。
- `orchbee.project.rename` → `{action:"project.rename",outcome,revision,projectId,label}`，
  `outcome` 取值 `committed|unchanged`：`project rename` 收据的专用 v1 文档，
  共享 `orchbee.mutation.result` 的 action 枚举保持封闭、不改动。
- `orchbee.config.check` → `{ok,checks:[{name,status,details}]}`，固定次序
  `runtime-base`、`runtime-directory`、`cohort`、`locale`、`sockets`；五项永远齐全。存在失败
  项时仍产出该成功 schema（`ok:false`）并以 1 退出。
- `orchbee.agent.doctor` → `{complete,providers:[{id,executable,runtime:{node},hook}],
endpoint:{socket},environment:{observationSocketEnv},asset:{roots:[<root>…]},hints:
[{id,evidence,text}]}`。`providers` 按规范次序报告每个所选 provider；每个事实行是封闭
  state/reason 词汇上的 `{state[,reason][,detail]}`——`executable`/`node` 报告 PATH 扫描
  事实（钩子状态需要 provider 自身命令面的条目带 `manualCommand`），`hook` 报告托管钩子
  检查结果（`present`/`absent`/`disabled`/`conflict`/`unknown`/`update-required`/
  `unsupported`），`socket`/`observationSocketEnv` 报告观察端点事实，每个 root 携带
  `origin`（`supplied`/`codex`/`cline`/`opencode`/`pi`/`omp`/`gemini`/`qwen-code`）、
  `state` 与恰居其一的 `path`（UTF-8）或 `pathBytesHex`（非 UTF-8）。`hints[].text` 是规范
  `en-US` 观察句——机器文档保持 locale 中立，文本输出会按 `id` 与 `evidence` 重新本地化
  同一句子。`complete:false` 标记受预算限制的部分文档；命令仍以 0 退出。
- `orchbee.site.start` → `{outcome,scope,runtimeBase}`：`outcome` 取值 `started` 或
  `already-running`（本次调用实际走的分支，而非 Session 身份收据），`scope` 为
  `orchbee-native-<euid>`，`runtimeBase` 恰携带 `path`（UTF-8）或 `pathBytesHex`
  （非 UTF-8，绝非有损显示串）之一。
- `orchbee.site.diagnose` → `{complete,scope,runtimeBase,runtimeDirectory,marker,locks,
endpoints,hostScope,advisories}`：Session 诊断一节描述的有界观察文档——封闭行状态与
  reason、`locks:{activation,siteLifetime}`、
  `endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}`、两个目录行的
  `path`/`pathBytesHex`，以及条件性 `advisories` token 清单。
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`；不存在英文 `message` 字段。`source` 只取自已可信来源（读取后失败用已验证快照；
  `api schema` not-found 用 `embedded`；mutation command 阶段失败一律用
  `workbench-command`；`server start` 的每个应答含其拒绝一律用 `local-runtime`；`terminal get`
  在可信 join 之后的输出准备失败用 `terminal-launch-read`）；读取之前
  的失败与语法失败没有 source。
- `orchbee.cli.failure.source` → `{schema:"orchbee.cli.failure.source",version:1,
failure:{kind,details}}`——send 源阶段专用 envelope，仅 `timeout` 与 `cancelled`。
  `details` 为 `{stage:"input",inputSource:"file"|"stdin"}`，超时另带 `timeoutMs`；
  绝不含权威 `source`、请求身份、路径或载荷字节，因为该失败可证明先于任何运行时
  副作用。
- `orchbee.cli.failure.agent` → `{schema:"orchbee.cli.failure.agent",version:1,failure:
{kind,details?},source?}`——准入后 agent 读取失败的文档：`kind` ∈
  `{cancelled,denied,internal-failure,not-found,protocol-invalid,rejected,
service-unavailable,timeout,transport-loss}`，`details` 为按 kind 封闭的词汇
  （`{stage:"agent-read",operation[,reason|sessionId|timeoutMs]}`，其中
  `operation` ∈ `{explain,get,list,wait}`；`wait` 在 `rejected` 上增加
  `revisionAheadDetails` `{sessionId,afterRevision,revision}`、在 `timeout`
  上增加 `waitTimeoutDetails` `{timeoutMs:null|1-3600000,elapsedMs,reason:wait-deadline|read-deadline,readBudgetMs?, completedObservations,lastStatusKind?,lastFreshness?,lastRevision?}`，而三个单次读取动词保持
  `timeoutMs:5000`），`source` 仅出现在可信语义应答上。agent 命令的准入前失败
  仍使用通用 `orchbee.cli.failure` 文档。

### Config 检查

| 检查项              | 规则                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime-base`      | 解析并观测 `ORCHBEE_RUNTIME_BASE`/默认值：存在且为目录则 pass；路径非法、缺失、不安全项或 I/O 错误则 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `runtime-directory` | 基座失败、或合法基座下 scope 目录不存在时为 `not-applicable`（Session 未运行不是配置错误）；既有目录通过校验为 pass；属主/模式/类型不安全或 I/O 错误为 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `cohort`            | `NativeCohort::current` 在其 5 秒预算内验证通过则 pass（`verified:true`）；缺失、非法、散列不符（`invalid-cohort`）或超时（`deadline`）为 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `locale`            | 信息性 pass，携带 `locale`（`en-US`/`zh-CN`）与 `source`（`LC_ALL`/`LC_MESSAGES`/`LANG`/`default`）。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `sockets`           | `client`/`worker` 在运行目录可安全检查后报告 `presence` 为 `present`/`absent`/`unchecked`，沿用既有 类型+属主+`0600`+no-follow 规则。`clientPending`/`host` 是仅派生路径预算条目：`presence` 恒为 `unchecked`，地址可容纳时为 `not-applicable`/`presence-not-inspected`，超预算时为 `fail`/`address-too-long`；两者始终携带 `path`/`pathBytesHex` 与 `addressBytes`/`addressCapacityBytes`。任一条目 `fail` 则整体 fail；否则至少一个合规存在的 `client`/`worker` 为 pass；其余为 `not-applicable`。存在绝不宣称服务或 Worker 活性，预算可容纳也绝不宣称端点存在——此处的 `not-applicable` 指刻意未检查 presence，而非预算未知。派生 socket 路径的字节长度超出本平台 `sockaddr_un` 容量时，无论 presence 如何都以 `address-too-long` fail——该地址永远无法 bind 或 connect。 |

details 一律类型化：`runtime-base`/`runtime-directory` 携带 `path`（UTF-8）或 `pathBytesHex`
（非 UTF-8，绝不用替换字符）；`exists` 仅在路径状态已确立时出现——已确立的缺失为 `false`、
观测到条目为 `true`，检查本身失败（权限、路径或 I/O 错误）时该字段整体省略——另可加 `reason`
∈ `{invalid-path,absent,unsafe,io-error}`；`sockets` 的 reasons 为 `{unsafe,io-error,
runtime-unavailable,address-too-long,presence-not-inspected}`。`clientPending`/`host` 预算条目与
每个 `address-too-long` 条目都携带 `path`/`pathBytesHex`
以及 `addressBytes`（派生路径的编码字节长度）和 `addressCapacityBytes`（本平台 socket 地址
容量）。不作跨文件原子健康快照声明。

## Mutations

`terminal send` 修改的是 Host 输入状态而非 Workbench 文档：它走上文描述的已准入 session
上的 OBCS `terminal.send` 交换，不触碰 `expectedRevision`、Workbench 命令通道，或 TUI 的
controller/viewer/resize 所有权。

十五条命令经已 admitted 的 OBWP `workbench.command` 通道修改 Workbench 状态：
`project create`/`rename`、`workspace create`/`rename`/`accept-directory`/`close`、
`tab create`/`rename`/`move`/`close`、`panel split`/`close`/`move` 与
`launch-profile create`/`delete`。每次调用原样执行查询状态机的第 1–5 阶段——一次 cohort 检查、
一次 discovery 与 admission、一次有界快照读取——然后在同一 5 秒 deadline 下再执行两个阶段：

6. **Resolve** —— 把已验证快照纯函数式地投影为 wire action：每个 target 以查询 resolver
   完全相同的语义解析（可能 `not-found`/`ambiguous`，此时提交零条命令），快照提供 `siteId`
   与 `expectedRevision`，并生成一个全新的 `WorkbenchRequestId`。
7. **Command** —— 在已准入 session 上恰好提交一次 `workbench.command`。随后在任何结果
   映射与输出准备之前完全释放 Client Session。

`expectedRevision` 的选择是协议的乐观并发契约：Server 将命令的 base revision 与当前
revision 比对，不符即拒绝且不应用任何效果，故"读取一次并按该 revision 提交"就是被 admitted
的交互序列——CLI 绝不为追赶变动的 base 而重复读取，也绝不在无 base 的情况下提交。

安全契约：

- **至多一次 effect。** 每次调用恰好提交一条命令；全部参数与 target 校验在命令构造之前
  完成，故解析缺陷或未解析 target 不提交任何命令。
- **全新身份。** 每次调用生成一个全新的全局唯一 `WorkbenchRequestId`；receipt 与
  `outcome-unknown` 失败都会报告它以便操作员对账。
- **Server 持有 revision。** `expectedRevision` 永远来自解析出全部 target 的同一份已验证
  快照——绝不猜测、绝不由客户端自增、绝不复用。
- **绝不自动重放。** 提交后结果不可判定的情形——写丢失、响应丢失/畸形/correlation 不符、
  或答案未通过校验——会 retire 该 session 并以 `outcome-unknown`（退出 8）呈现。命令可能
  已应用也可能未应用；绝不重发。本地化诊断会明确说明这一点，并指示操作员先检查
  Workbench 状态（`status`、`tab list`、`panel list` 等）再决定是否重复该命令。
- **关闭确认。** `tab close`、`panel close` 与 `workspace close` 是破坏性操作；解析器
  要求显式 `--confirm` flag，故非交互调用绝不会默认执行关闭。`workspace close-resume`
  同样要求该 flag：为保留的 close 开启新的尝试窗口本身也是 effect。
- **关闭等待。** `tab close` 与 `workspace close` 默认在接纳后读取新快照，直到该 Tab 或
  Workspace 消失，总期限为 `--timeout`（1..3600000 毫秒，默认 30000）。文本收据随后报告已关闭的
  名称和终端数。超时以 `timeout` 失败（`details:{stage:"wait",timeoutMs,observed}`）退出，
  关闭在会话中继续；文本诊断给出 `tab list` 或 `workspace close-status` 命令。`--detach`
  保持接纳即返回的 `accepted` 收据，与 `--timeout` 冲突。`--dry-run` 不提交任何内容，返回
  按该 Tab 或 Workspace 筛选的 `terminal list` 结果，文本以一句试运行说明开头；它与
  `--detach`、`--timeout` 冲突。JSON 收据形状不变。

结果：`committed` 报告新 `revision`（base + 1）；`unchanged`——未造成变化的 rename 或
move——报告当前 revision；`accepted`——各条 close——报告 `acceptedRevision` 及 Server 为
异步关闭给出的 `closeRequestId`；`resumed`——`workspace close-resume`——报告
`closeRequestId` 与 Server 开启的新 `attemptGeneration`。`tab create` 与 `panel split`
另携带 `terminal`：Server
仍欠 spawn 时为 `pending`，否则为类型化 spawn 结果（`started` 带回显的 `geometry`，
`unavailable`/`failed` 带 reason，`unknown`）。

`project create` 是唯一无 target 操作数、无文件系统副作用的修改命令：动作只携带 label，
由 Server 分配新 `projectId` 并在 `committed` 收据中返回，新建 Project 无目录、无 Workspace。
label 允许重复——label 无法唯一标识新 Project；收据中的 `projectId` 可以。

`workspace accept-directory` 提交既有 `WorkspacePrimaryDirectoryAccept` 动作，以全局 Workspace ID 定位，不携带 Project。`--path` 保留原有绝对路径、4096 UTF-8 字节及无 NUL 准入；CLI 不创建、修复或探测目录。Server 权威探测并拒绝已有主 Directory 的 Workspace。Windows `workspace create` 在一次 launch 内完成新 Workspace 目录准入，无需先执行该命令。

几何在请求中永远显式：非 TTY 调用无法继承终端尺寸，故 `tab create` 与 `panel split`
把四个 flag 作为全有或全无组接受，整组省略时请求默认的 80x24 网格与成对零（未知）像素
对——这是物化进 wire 几何的 CLI 请求策略，而非实测显示事实。`--cols`/`--rows` 为格数，
`--cell-width`/`--cell-height` 为每格像素尺寸；每个维度都是 `u16`，并在任何运行时访问
之前经协议 `WorkbenchTerminalGeometry` 校验。

`tab create` 与 `panel split` 的启动经三种选择之一：解析出的默认 shell、显式
`--exec`/`--arg` 对、或 `--profile <selector>`。selector 在命令中逐字透传——Server 在
命令时刻的目录锁下解析（先精确 ID，再按恰一匹配的名称），解析出的 retained 启动成为
终端的 provenance。未命中的 selector 是零变更的已提交 outcome，CLI 映射为 `not-found`；
多匹配的名称映射为 `ambiguous`——两者均走 launch-profile 失败 envelope，带该命令的
`workbench-command` 源。

`launch-profile create` 与 `launch-profile delete` 经同一"一条命令"envelope 修改目录
本身。`create` 携带 `--name` 与显式 flag 构建的同一份已校验 `LaunchSelection`，committed
收据报告 `revision`（Workbench revision）、`catalogRevision`（目录私有计数）、铸造的
`profileId` 与逐字的 `name`。`delete` 携带未解析的 selector——Server 在目录锁下应用
"先 ID 后名称基数"规则；committed 收据报告 `revision`、`catalogRevision` 与被删除的
`profileId`。已 settle 的 `ProfileUnavailable` outcome 映射为
`not-found{resource:"launch-profile"}`，`ProfileAmbiguous` 映射为携带受限候选 ID 的
`ambiguous`——均走
launch-profile 失败 envelope；目录容量拒绝走通用 envelope 下的普通 command 阶段拒绝。

Workspace 与组织变更使用 `schema:"orchbee.mutation.result"`、`version:6`；其他既有动作保留 v4/v5，
`source:{kind:"workbench-command",siteId,requestId,expectedRevision}`，`result` 携带 `action`
（点分命令名）、`outcome`、该结果的 revision 字段、已解析 typed ID，以及命令具备时的
`direction`/`label`/`terminal`/`catalogRevision`/`profileId`/`name`。`workspace
close-resume` 是唯一的来源例外：保留 lifecycle 的 resume 没有 `workbench.command`，故其
`resumed` 收据与提交后失败报告 `source.kind="workspace-close-resume"`——resume 请求自身的
回显，带 `siteId`、`requestId`、`closeRequestId` 与 `expectedAttemptGeneration`。
文本输出按与查询相同的转义规则逐行渲染同样字段。
`project rename` 是唯一例外：其收据使用专用的 `schema:"orchbee.project.rename"`、
`version:1` 文档——`source` 仍为同一 `workbench-command` 形态，`result` 封闭于
`action:"project.rename"`、`outcome`（`committed`/`unchanged`）、`revision`、
`projectId` 与 `label`。

`workspace close` 提交 `WorkspaceClose` 动作——由 Server 权威地关闭该 Workspace 的
Client 使用的全部终端，在整个 context scope 上复用与 `tab close`/`panel close` 相同的
terminal-tab-close lifecycle 机械。终端清理完成后，Workspace 及其 terminal context
从 active projection 移除；checkout、branch 与磁盘目录保留。`accepted` 收据携带 `acceptedRevision`、该
操作的 `closeRequestId` 与已解析的 `workspaceId`/`terminalContextId`；Session
异步完成关闭，保留的操作记录比 Workspace 的 active projection 活得更久。重复运行
`workspace close` 绝不是 resume——可恢复的保留操作只能经 `workspace close-resume` 继续。

`workspace close-status` 仅凭严格稳定 ID 读取该保留记录：它共享查询状态机的 session
准入但绝不在本地解析 Workspace membership——记录比 projection 行活得更久，故已完成的
close 仍以 `available` 连同其 `closed` tombstone `revision` 作答，而从未发生 close 的
scope 以 `not-closing` 作答。`workspace close-resume` 是保留 lifecycle 请求而非
`workbench.command`：它执行同样的准入纪律并恰好提交一次 resume 交换，携带该 close 操作的
`closeRequestId` 与调用方最后观察到的 `expectedAttemptGeneration` 栅栏。只有可恢复的
lifecycle 状态可以 resume——未知结果不可恢复——陈旧栅栏以类型化 `stale` 应答落定，在
close 失败 envelope 下报告为 `rejected`、`reason:"stale-generation"`，绝不产生第二次尝试。

明确不交付：`project close`、`panel resize`、终端 open 操作、Session restart、disable、recovery 或
修复命令（Windows 已支持 stop/status），以及一切远端操作。

## 失败矩阵

退出码：0 成功（含空表）；1 本地配置/输出/内部确定失败；2 usage/非法 target；3
not-found/ambiguous/denied/rejected；4 service-unavailable（含 `session-capacity`）；5
transport-loss/protocol-invalid；6 incompatible；7 `timeout`（`terminal wait-output`
期限耗尽未命中，或 send 源物化期间共享 deadline 到期）；8 outcome-unknown；130
`cancelled`（被处理的 SIGINT 类中断——`terminal wait-output` 期间收到
HUP/INT/QUIT/TERM，或 `agent doctor` 探测项之间捕获合格信号；更一般地，被处理的
`terminal send` 源阶段信号按惯例
`128 + signo` 退出：合格集对应 129/130/131/143）。`agent` 读取期间被捕获的
SIGINT 或 SIGTERM 结算为类型化 `cancelled` agent 失败、状态 130——按契约两个
成员相同——未被 watch 的 SIGHUP/SIGQUIT 保留其继承 disposition。源 guard 未捕获的合格信号按原
disposition 交付——OS 默认终止遵循 shell 的 `128 + signal` 约定，不是 CLI 合成的结果。

| 来源                                                                                                                                | `failure.kind`                                                             | 退出      | `details`                                                                                     |
| ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | --------- | --------------------------------------------------------------------------------------------- |
| argv 封装/语法/format/target 形状错误                                                                                               | `invalid-usage`                                                            | 2         | 省略                                                                                          |
| 全零 ID/空/超长 label、非法 panel/terminal ID                                                                                       | `invalid-identifier`                                                       | 2         | `{resource,target}`                                                                           |
| 合法 target 或 schema ID 无匹配                                                                                                     | `not-found`                                                                | 3         | `{resource,target}`                                                                           |
| 多个精确 label 匹配                                                                                                                 | `ambiguous`                                                                | 3         | `{resource,target,candidates≤8,totalCount}`                                                   |
| `NativeCohort::current` 出错                                                                                                        | `check-failed`                                                             | 1         | `{stage:"cohort"}`                                                                            |
| 基座非法/缺失/非目录、不安全 runtime/socket、其他校验                                                                               | `check-failed`                                                             | 1         | `{stage:"runtime-base"                             \| "runtime-directory"   \| "socket"}`     |
| 合法基座下 scope 目录或 `client.sock` 不存在                                                                                        | `service-unavailable`                                                      | 4         | `{stage:"discovery"}`                                                                         |
| `ConnectClientError::ConnectFailed`                                                                                                 | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                         |
| `ConnectClientError::Io`                                                                                                            | `transport-loss`                                                           | 5         | `{stage:"admission"}`                                                                         |
| OBLC `Rejected(CapacityExceeded)`                                                                                                   | `session-capacity`                                                         | 4         | `{stage:"admission"}`                                                                         |
| OBLC `Rejected(HandshakeTimeout)`                                                                                                   | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                         |
| `Rejected(IncompatibleProtocol                                    \| IncompatibleCohort)`、`Protocol(IncompatibleVersion)`          | `incompatible`                                                             | 6         | `{stage:"admission"}`                                                                         |
| `Rejected(WrongMessage                                            \| MalformedFrame)`、`UnexpectedResponse`、其余 `Protocol`        | `protocol-invalid`                                                         | 5         | `{stage:"admission"}`                                                                         |
| `RequestIoConfiguration`、`SessionUnavailable`、`RequestEncoding`                                                                   | `internal-failure`                                                         | 1         | 省略                                                                                          |
| `SnapshotReadError::RequestNotSubmitted`（任何原因）                                                                                | `transport-loss`                                                           | 5         | `{stage:"snapshot"}`                                                                          |
| `Failed(Write                                                     \| ResponseDeadline)`、`Failed(Response(Io))`                     | `transport-loss`                                                           | 5         | `{stage:"snapshot"}`                                                                          |
| `Failed(Response(InvalidHeader                                    \| Contract))`、`UnexpectedResponse`                              | `protocol-invalid`                                                         | 5         | `{stage:"snapshot"}`                                                                          |
| `Failed(CorrelationMismatch)`                                                                                                       | `protocol-invalid`                                                         | 5         | `{stage:"snapshot",reason:"correlation-mismatch"}`                                            |
| outcome `Denied`                                                                                                                    | `denied`                                                                   | 3         | 省略                                                                                          |
| outcome `Unavailable`                                                                                                               | `service-unavailable`                                                      | 4         | `{stage:"snapshot"}`                                                                          |
| viewer `RequestNotSubmitted`、`OutcomeUnknown(Write\|Read)`                                                                         | `transport-loss`                                                           | 5         | `{stage:"viewer"}`                                                                            |
| viewer `OutcomeUnknown(InvalidResult\|UnexpectedResult)`、错误 outcome 类别                                                         | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                            |
| viewer `OutcomeUnknown(CorrelationMismatch)`                                                                                        | `protocol-invalid`                                                         | 5         | `{stage:"viewer",reason:"correlation-mismatch"}`                                              |
| viewer outcome `Rejected(AlreadyAttached\|TerminalUnavailable)` 或 `Unavailable`                                                    | `service-unavailable`                                                      | 4         | `{stage:"viewer"}`                                                                            |
| viewer outcome `Rejected(FeedbackMismatch)`、record 被投影拒绝                                                                      | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                            |
| `ReadRows` outcome `RowsRejected(LimitExceeded)`                                                                                    | `rejected`                                                                 | 3         | `{stage:"viewer",operation:"read-history",reason:"limit-exceeded"}`                           |
| `ReadRows` outcome `RowsRejected`（其他可产生 reason）                                                                              | `service-unavailable`                                                      | 4         | `{stage:"viewer",operation:"read-history",reason}`                                            |
| `ReadRows` outcome `RowsRejected(InvalidField\|InvalidAcknowledgement)`、行窗口 record 失配/畸形                                    | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                            |
| launch-read `RequestNotSubmitted`、`OutcomeUnknown(Write\|Read)`（含共享网络预算耗尽）                                              | `transport-loss`                                                           | 5         | `{stage:"launch-read"}`                                                                       |
| launch-read `OutcomeUnknown(InvalidResult\|UnexpectedResult)`、错误 outcome 类别                                                    | `protocol-invalid`                                                         | 5         | `{stage:"launch-read"}`                                                                       |
| launch-read `OutcomeUnknown(CorrelationMismatch)`                                                                                   | `protocol-invalid`                                                         | 5         | `{stage:"launch-read",reason:"correlation-mismatch"}`                                         |
| launch-read outcome `Unavailable(ServerBusy)`                                                                                       | `service-unavailable`                                                      | 4         | `{stage:"launch-read"}`                                                                       |
| `DefinitelyNotSubmitted::RequestNotSubmitted`（command）                                                                            | `transport-loss`                                                           | 5         | `{stage:"command"}`                                                                           |
| `DefinitelyNotSubmitted::SessionUnavailable`/`RequestEncoding`                                                                      | `internal-failure`                                                         | 1         | 省略                                                                                          |
| command `Rejected` precommit                                                                                                        | `rejected`                                                                 | 3         | `{stage:"command",reason,revision}`                                                           |
| command `Denied` precommit                                                                                                          | `denied`                                                                   | 3         | 省略                                                                                          |
| command `Unavailable` precommit                                                                                                     | `service-unavailable`                                                      | 4         | `{stage:"command",reason}`                                                                    |
| `OutcomeUnknown`（任意原因）                                                                                                        | `outcome-unknown`                                                          | 8         | `{stage:"command",requestId,reason}`                                                          |
| close-status `RequestNotSubmitted`、写/io 响应失败                                                                                  | `transport-loss`                                                           | 5         | `{stage:"close-status"}`，属 `orchbee.cli.failure.close`                                      |
| close-status 非法/意外响应或 scope 不符的应答                                                                                       | `protocol-invalid`                                                         | 5         | `{stage:"close-status"[,reason:"correlation-mismatch"\|"scope-mismatch"]}`，属 close envelope |
| close-status 响应 deadline 到期，或共享 5 秒命令 deadline 到期                                                                      | `timeout`                                                                  | 7         | `{stage:"close-status",timeoutMs:5000}`，属 close envelope                                    |
| close-status outcome `Denied`                                                                                                       | `denied`                                                                   | 3         | `{stage:"close-status"}`，属 close envelope                                                   |
| close-status outcome `Unavailable`                                                                                                  | `service-unavailable`                                                      | 4         | `{stage:"close-status",reason:"server-busy"}`，属 close envelope                              |
| close-resume `RequestNotSubmitted`、写/io 响应失败                                                                                  | `transport-loss`                                                           | 5         | `{stage:"close-resume"}`，属 close envelope                                                   |
| close-resume 非法/意外响应                                                                                                          | `protocol-invalid`                                                         | 5         | `{stage:"close-resume"}`，属 close envelope                                                   |
| close-resume outcome `Denied`                                                                                                       | `denied`                                                                   | 3         | `{stage:"close-resume"}`，属 close envelope                                                   |
| close-resume outcome `Unavailable`                                                                                                  | `service-unavailable`                                                      | 4         | `{stage:"close-resume",reason:"server-busy"}`，属 close envelope                              |
| close-resume outcome `Stale`                                                                                                        | `rejected`                                                                 | 3         | `{stage:"close-resume",reason:"stale-generation",attemptGeneration}`，属 close envelope       |
| close-resume outcome `Rejected`（类型化 reason）                                                                                    | `rejected`                                                                 | 3         | `{stage:"close-resume",reason}`，属 close envelope                                            |
| close-resume `OutcomeUnknown`（任意原因）                                                                                           | `outcome-unknown`                                                          | 8         | `{stage:"close-resume",requestId,reason}`，属 close envelope                                  |
| worktree-list `RequestNotSubmitted`、写/io 响应失败、或 `TransportLoss` outcome（投递后响应丢失）                                   | `transport-loss`                                                           | 5         | `{stage:"worktree-list"}`，属 `orchbee.cli.failure.worktree`                                  |
| worktree-list 非法/意外响应                                                                                                         | `protocol-invalid`                                                         | 5         | `{stage:"worktree-list"[,reason:"correlation-mismatch"]}`，属 worktree envelope               |
| worktree-list 响应 deadline 到期，或共享 5 秒命令 deadline 到期                                                                     | `timeout`                                                                  | 7         | `{stage:"worktree-list",timeoutMs:5000}`，属 worktree envelope                                |
| worktree-list outcome `NotFound`（Server 核验时全局导航中没有该目录）                                                               | `not-found`                                                                | 3         | `{resource:"directory",target}`，属 worktree envelope                                         |
| worktree-list outcome `Denied`                                                                                                      | `denied`                                                                   | 3         | `{stage:"worktree-list"}`，属 worktree envelope                                               |
| worktree-list `Unavailable`/`SpawnUnavailable`/`SpawnFailed`/`OwnerIo`                                                              | `service-unavailable`                                                      | 4         | `{stage:"worktree-list",reason:"server-busy"                                                  | "worker-unavailable"                                     | "job-dead" | "job-unknown"}` |
| worktree-list `GitFailed`（probe/list 带精确退出 status，被信号终止时为 `null`）或 `MalformedListing`                               | `check-failed`                                                             | 1         | `{stage:"probe"                                                                               | "list",status}`或`{stage:"parse"}`，属 worktree envelope |
| worktree-list `TooLarge`（共享 Git stdout、OBCN 结果或 Workbench 结果预算被越过）                                                   | `output-too-large`                                                         | 1         | `{stage:"worktree-list",origin,limitBytes}`，属 worktree envelope                             |
| launch-profile-catalog `RequestNotSubmitted`、写/io 响应失败                                                                        | `transport-loss`                                                           | 5         | `{stage:"launch-profile-catalog"}`，属 `orchbee.cli.failure.launch-profile`                   |
| launch-profile-catalog 非法/意外响应                                                                                                | `protocol-invalid`                                                         | 5         | `{stage:"launch-profile-catalog"[,reason:"correlation-mismatch"]}`，属 profile envelope       |
| launch-profile-catalog 响应 deadline 到期                                                                                           | `timeout`                                                                  | 7         | `{stage:"launch-profile-catalog",timeoutMs:5000}`，属 profile envelope                        |
| launch-profile-catalog outcome `NotFound`                                                                                           | `not-found`                                                                | 3         | `{resource:"launch-profile",target}`，属 profile envelope                                     |
| launch-profile-catalog outcome `Ambiguous`                                                                                          | `ambiguous`                                                                | 3         | `{resource:"launch-profile",target,candidates≤8,totalCount}`，属 profile envelope             |
| launch-profile-catalog outcome `Denied`                                                                                             | `denied`                                                                   | 3         | `{stage:"launch-profile-catalog"}`，属 profile envelope                                       |
| launch-profile-catalog outcome `Unavailable`                                                                                        | `service-unavailable`                                                      | 4         | `{stage:"launch-profile-catalog",reason:"service-unavailable"}`，属 profile envelope          |
| launch-profile mutation `ProfileUnavailable`（`delete`、`tab create --profile`、`panel split --profile` 上零变更已 settle outcome） | `not-found`                                                                | 3         | `{resource:"launch-profile",target}`，属 profile envelope                                     |
| launch-profile mutation `ProfileAmbiguous`（零变更已 settle outcome，同批命令）                                                     | `ambiguous`                                                                | 3         | `{resource:"launch-profile",target,candidates≤8,totalCount}`，属 profile envelope             |
| send `RequestNotSubmitted`（请求可证明从未到达线缆）                                                                                | `transport-loss`                                                           | 5         | `{stage:"send"}`                                                                              |
| send `SessionUnavailable`/`RequestEncoding`                                                                                         | `internal-failure`                                                         | 1         | 省略                                                                                          |
| send `OutcomeUnknown`（写/读/无效结果/correlation）                                                                                 | `outcome-unknown`                                                          | 8         | `{stage:"send",requestId,reason}`                                                             |
| `terminal.send.result` `Rejected`/`Unavailable`                                                                                     | 类型化应答，非失败 envelope                                                | 3/4       | send 结果 `reason`、`cleanupStatus`；`host-protocol-fault` 退出 5                             |
| `terminal.send.result` `Accepted` + `Unconfirmed` cleanup                                                                           | 类型化应答，非失败 envelope                                                | 5         | send 结果 `recordsAccepted`、`acceptedEncodedBytes`、`cleanupStatus`                          |
| agent `RequestNotSubmitted`/`Write`/`ResponseDeadline`/`Response(Io)`（deadline 未耗尽）                                            | `transport-loss`                                                           | 5         | `{stage:"agent-read",operation}`                                                              |
| agent 交换或准入后步骤内共享总 deadline 耗尽                                                                                        | `timeout`                                                                  | 7         | `{stage:"agent-read",operation,timeoutMs:5000}`                                               |
| agent armed span 捕获合格 SIGINT 或 SIGTERM（任意阶段；HUP/QUIT 保持原始 disposition）                                              | `cancelled`                                                                | 130       | `{stage:"agent-read",operation}`                                                              |
| agent `Response(wire fault)`/`UnexpectedResponse`/`CorrelationMismatch`、错误结果 kind                                              | `protocol-invalid`                                                         | 5         | `{stage:"agent-read",operation}`                                                              |
| agent outcome `Rejected(NotFound)`（get/explain）                                                                                   | `not-found`                                                                | 3         | `{stage:"agent-read",operation,sessionId}`                                                    |
| agent outcome `Rejected(ReadDenied)`                                                                                                | `denied`                                                                   | 3         | `{stage:"agent-read",operation,reason:"read-denied"}`                                         |
| agent list outcome `Rejected(CursorInvalid\|CursorStale)`                                                                           | `rejected`                                                                 | 3         | `{stage:"agent-read",operation:"list",reason:"cursor-invalid"\|"cursor-stale"}`               |
| agent outcome `Unavailable`                                                                                                         | `service-unavailable`                                                      | 4         | `{stage:"agent-read",operation,reason:"runtime-unavailable"}`                                 |
| agent `SessionUnavailable`/`RequestEncoding`                                                                                        | `internal-failure`                                                         | 1         | 省略                                                                                          |
| send 源校验：打不开/非 regular 路径、终端或不支持的 stdin、空、超限、非 UTF-8                                                       | `invalid-usage`                                                            | 2         | 省略（本地化 stderr 指明所声明选项与具体原因）                                                |
| send 源物化期间共享 deadline 到期                                                                                                   | `timeout`，属 `orchbee.cli.failure.source`                                 | 7         | `{stage:"input",inputSource,timeoutMs}`                                                       |
| send 源物化期间捕获合格信号                                                                                                         | `cancelled`，属 `orchbee.cli.failure.source`                               | 128+signo | `{stage:"input",inputSource}`                                                                 |
| send 源信号 setup/读取/恢复失败                                                                                                     | `internal-failure`                                                         | 1         | 省略                                                                                          |
| 预备输出超 8 MiB                                                                                                                    | `output-too-large`                                                         | 1         | `{limitBytes:8388608}`                                                                        |
| stdout 写/flush 失败                                                                                                                | `output-failed`                                                            | 1         | 不追加第二份 stdout 文档                                                                      |
| `config check` 完成但存在失败项                                                                                                     | 成功 schema `ok:false`                                                     | 1         | 五项全部保留                                                                                  |
| `server start` cohort `InvalidData`/`NotFound`/`PermissionDenied`/`TimedOut`/其他 io kind                                           | `incompatible`/`service-unavailable`/`denied`/`timeout`/`internal-failure` | 6/4/3/7/1 | `{stage:"cohort"}`                                                                            |
| `server start` socket 路径超 `sockaddr_un` 预算                                                                                     | `service-unavailable`                                                      | 4         | `{stage:"runtime-directory",reason:"address-too-long"}`                                       |
| `server start` connect/admission 失败                                                                                               | 同上方 `ConnectClientError` 各行，逐字一致                                 | 4/5/6     | `{stage:"admission"}`                                                                         |
| `server start` 标记或 socket 已提交但无存活属主；Server spawn 或 `try_wait` io 失败                                                 | `rejected`                                                                 | 3         | `{stage:"activation",reason:"recovery-required"}`                                             |
| `server start` 激活 deadline 到期；取锁 `TimedOut`                                                                                  | `timeout`                                                                  | 7         | `{stage:"activation"}`                                                                        |
| `server start` 标记/探针/锁 io 失败（非超时）                                                                                       | `internal-failure`                                                         | 1         | `{stage:"activation"}`                                                                        |
| `server diagnose` 完整或部分事实文档——unsafe/uninspectable 行是事实而非失败                                                         | 类型化 `orchbee.site.diagnose` 文档                                        | 0         | 行 `state`/`reason` 字段                                                                      |
| `server diagnose` 在 Linux/Windows 以外；网络查询、mutation、`server start`、`config check` 或 `agent doctor` 在 Linux/Windows 以外 | `service-unavailable`/`check-failed`                                       | 4/1       | `{stage:"platform",reason:"unsupported-platform"[,command]}`                                  |
| `agent doctor` 在探测项之间捕获合格信号                                                                                             | `cancelled`                                                                | 130       | `{stage:"agent-doctor"}`——部分文档仅由预算耗尽产生                                            |
| `terminal wait-output` 期限耗尽未命中                                                                                               | `timeout`                                                                  | 7         | `{stage:"wait",timeoutMs,observed}`                                                           |
| `terminal wait-output` 期间收到 HUP/INT/QUIT/TERM                                                                                   | `cancelled`                                                                | 130       | `{stage:"wait",cleanup}`                                                                      |
| `terminal wait-output` 命中但 `Detach` 未确认                                                                                       | 类型化 viewer-stage 失败                                                   | *         | `{stage:"viewer",matchObserved:true,cleanup:"unconfirmed"}`                                   |

`resource` ∈ `project|workspace|tab|panel|terminal|schema`；`stage` ∈
`cohort|runtime-base|runtime-directory|socket|discovery|admission|activation|snapshot|viewer|command|send|agent-read|agent-doctor|platform|wait`。
专用 `orchbee.cli.failure.worktree` envelope 接纳 `resource:"directory"` 与上文随其
schema 记录的 `worktree-list|probe|list|parse` stage 词汇——这些值仅属该 envelope，
不进入通用 `orchbee.cli.failure` 词汇。
专用 `orchbee.cli.failure.close` envelope 接纳上文随其 schema 记录的
`close-status|close-resume` stage 词汇——这些值同样仅属该 envelope。
源阶段的 `input` stage 只存在于 `orchbee.cli.failure.source` envelope——不进入标准
`orchbee.cli.failure` 的 stage 词汇。
stage 与 reason 是有限 schema 值；任何异常文本都不进入机器字段。`details` 是按 kind 封闭的词汇——`{resource,target}`、
`{resource,target,candidates≤8,totalCount}`、`{stage[,reason]}`、
`{stage:"command",reason,revision}`、`{stage:"command",requestId,reason}`、
`{stage:"send",requestId,reason}`、
`{stage:"viewer",operation:"read-history"|"wait-output",reason}`、
`{stage:"wait",timeoutMs,observed}`、`{stage:"wait",cleanup}`、
`{stage:"viewer",matchObserved,cleanup}`、`{limitBytes}`、
`{stage:"platform",reason,command?}`（`command` 成员命名拒绝的检查命令，
`"agent doctor"`），或 agent-read 族——
`{stage:"agent-read",operation}`、`{stage:"agent-read",operation,sessionId}`、
`{stage:"agent-read",operation,reason}`、`{stage:"agent-read",operation,timeoutMs}`——
内嵌 schema 拒绝超出这些形状的文档。读路径错误永不归类为
outcome-unknown/failed-effect：只有 command 阶段可能产生不可判定效果；
`rejected`/`denied`/`unavailable` precommit 答案都是确定的——Server 未应用任何效果。
send 阶段对其单条输入记录遵循同一规则：`RequestNotSubmitted` 是确定的未提交，
`OutcomeUnknown` 表示记录可能已经落地，而类型化 `terminal.send.result` 应答是 Server
自己的判定，绝非客户端猜测。声明型 `--file`/`--stdin` 源更严格：其一切失败都发生在
任何运行时访问之前，故绝不可能产生 `outcome-unknown`；被捕获信号的取消或共享
deadline 到期在 wire 请求尚不存在时即退出。

## 输出契约与副作用

- stdout 只承载契约文档：`--format json` 下为一份带 2 空格缩进、以 `\n` 结尾的 JSON（`api`
  命令恒为 JSON），text/默认下为本地化人类文档。stderr 承载本地化人类诊断；查询路径不打印
  进度。
- `invalid-usage` 失败在 stderr 写一条本地化诊断，指明单一具体原因——未知命令或选项、
  该命令不接受的选项或参数、重复选项、冲突选项、缺少选项值、缺少必需选项或操作数、缺少
  命令、未知帮助路径、错位的 help/version 标志、非法选项值或几何参数组合、非 Unicode 或
  超限参数表——并以帮助提示结尾。提示对可识别的命令或族上下文给出 `orchbee <path>
--help`，否则回落到 `orchbee --help`；每条发出的提示都是无需 Session 即可退出 0 的真实
  路径。回显的不可信 token 复用下述带引号转义渲染，并先截到 256 个 Unicode 标量前缀、
  仍有剩余标量时显式追加省略号；非 Unicode 或超限参数表绝不回显。机器 envelope 不变——
  诊断只存在于 stderr。
- 文本把用户可控字符串渲染为带引号的转义值：反斜杠与引号用 `\\`/`\"`；C0/C1 控制字符（含
  ESC）、U+2028/U+2029 与双向格式控制符用无歧义 `\uXXXX` 转义；其余 Unicode 原样保留。列表
  表格只把 label 显示截到 80 显示列并加省略号，转义序列与收尾引号保持完整；ID 绝不截断；
  `get` 字段值完整呈现。JSON 用标准无损编码保留源数据——绝不用 TUI 的替换字符清洗器。
- 完整输出在首次 stdout 写入前于有界缓冲区内构造；预备超过 8 MiB 即 `output-too-large`，
  stdout 不被触碰。真实 OS 写失败可能留下部分字节，但绝不追加第二份文档；stderr 仍报告且
  退出码非零。
- 机器输出与 locale 无关且不含英文 `message`；stderr 诊断按 en-US/zh-CN 本地化。机器 `kind`、
  字段名与枚举值绝不本地化；用户 label/path 数据按数据原样保留。

## 不变量

- 语法非法的调用绝不触碰文件系统、cohort 或 socket。
- 离线命令（`help`、`-v`/`-V`/`--version`、`--skill`、`completion`、`api schema` 与 Windows
  `--default-config`）绝不执行平台、运行、cohort 或 Session 发现。
- 网络查询不创建任何事物：无目录、锁、标记、终端、viewer、控制授权或第二个请求——唯一
  例外是 `terminal read`/`terminal wait-output`，其只读能力 viewer attachment 是唯一被
  认可的读路径副作用，最小化持有并在输出前释放。至多持有一个短暂 Client Session，且绝不
  带入输出阶段。
- Mutation 每次调用在全新 `WorkbenchRequestId` 下至多提交一次 `workbench.command`；绝不
  重试或重放，结果不可判定时报告 `outcome-unknown` 而非猜测。`terminal send` 对其唯一
  一次 OBCS `terminal.send` 交换遵循同一纪律：每次调用在全新请求身份下提交一组有界记录序列，
  绝不重试——不可判定的应答是 `outcome-unknown`，且本地化诊断会说明输入可能已经落地，
  由操作员据此决定是否重复。
- 声明型 `terminal send` 源在任何运行时访问之前被完整物化：一组有界输入序列（文本 4096 字节、
  不透明字节 1 MiB、粘贴 256 KiB）喂给恰好一条 `terminal.send` 请求——绝不切分、重放或
  部分提交。源超时、取消或校验失败产生零运行时副作用且无请求身份；物化之后普通 send
  生命周期权威接管，源阶段结果不能在语义上取消已被准入的 send。
- Linux 上的源信号 guard 绝不改动 disposition，并把继承的线程 mask 精确恢复——被捕获、继承
  阻塞与继承忽略的信号各自保持原语义；没有任何信号 owner 活过源阶段，也没有任何
  合格信号在输出最终化路径之后仍被阻塞。
- 每次调用至多一次快照读取、一次准入，且读取与命令共用一个网络 deadline；`terminal read`
  的 viewer 交换与 `terminal send` 的唯一 send 复用同一 session 与同一 deadline——绝无
  第二次准入或续期预算。Agent 读取的可选名称解析及读取交换遵循同一纪律：每次请求均用
  全新 `ObapRequestId`、一个相关联结果，任何不可判定结果即 retire session 且绝不
  重放——且完全不取 Workbench 快照。
- Agent 观察绝不虚构：结果是逐字的 wire outcome，`found` 的 `explanation` 是 Server
  的 canonical 11 字段解读，成功绝不从终端内容或截图推断。`agent-read` source 只在
  可信的相关联应答之后存在；transport、timeout 与 protocol-invalid 失败均不携带。
  `terminal wait-output` 保持一次准入，每次 I/O 同时受五秒上限及总 `--timeout`
  剩余预算约束。来源模式的 `--forever` 仅取消总时限；每附件 `requestSequence`
  在全部 `read-rows` 中从 1 严格递增。
- match 只证明该文本在本次观察的搜索窗口内被找到——绝不证明命令完成、新 turn 或没有
  其他输出；轮询间隙滚过 Host 保留区的输出对任何消费者都不可见。
- 没有可信来源的字段不存在；`null` 表示来源确定地表达"无"，`"unknown"` 表示来源无法回答——
  两者绝不混用。
- 失败 envelope 绝不虚构 `source`：siteId/revision 只来自已验证快照。
- Ambiguous 绝不猜测：报告有界候选与真实总数。
- `api schema`、help 与 completion 只枚举同一次构建中真实交付的命令。
- `server start` 是唯一的激活面，也是唯一会创建本地运行时状态的命令：它可能创建运行时目录、
  持有激活/生存期锁、提交标记并拉起 Server——只有语法失败保证零副作用。其 20 秒绝对
  deadline 绝不按阶段重置；已返回的 `recovery-required` 拒绝不重试或强制清理；Windows 冷恢复在开始新代前
  复查独立文档中的死亡证明 guard。超时绝不声称
  没有 Server 启动。`server diagnose` 是观察面，除其有界生存期锁探针外无任何效应：不
  connect、不创建、不取锁、不修复，也不作存活或重启声明。
- 各 CLI 自有 5 秒预算都是 checked deadline；其构造失败是 `internal-failure`，而 transport
  内部 `RequestNotSubmitted` 一律保持 `transport-loss`。

## 场景矩阵与证据

| 场景                                                                                                                                                                     | 预期                                                                             | 证据                                                              |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 穷举语法：format、位置、`--`、help 形式、重复、非法                                                                                                                      | 精确 parse/失败类别                                                              | `src/invocation` 测试                                             |
| format envelope 次序：任何位置的缺陷即空 stdout；任何位置的合法 json 决定其后失败的 envelope                                                                             | envelope 优先于语法错误                                                          | `src/invocation` 测试 + `tests/cli_query.rs`                      |
| 过滤器范围：`workspace list` 只接受 `--project`；越界过滤器在任何运行时访问之前即 usage                                                                                  | 完整矩阵，过滤器绝不静默丢弃                                                     | `src/invocation` 测试 + `tests/cli_query.rs`                      |
| 目标规则：ID/label/全零/超长/歧义优先                                                                                                                                    | resolver 判定                                                                    | `src/cli` resolver 测试                                           |
| 已验证快照 → 各结果形状、空集合、null                                                                                                                                    | 精确 JSON/text fixture                                                           | `src/cli` 输出测试 + JSON Schema 校验（dev `jsonschema`）         |
| Schema 字典 ↔ 实际 envelope                                                                                                                                              | 每个成功/失败 fixture 通过校验；被拒变体不通过                                   | schema 对比测试                                                   |
| API 失败格式：text/默认失败保持 stdout 为空，`--format json` 产出 envelope                                                                                               | `api schema`/`api snapshot` 失败遵守调用方 format                                | `tests/cli_query.rs` + `src/cli` 测试                             |
| 无 Session 磁盘状态                                                                                                                                                      | `service-unavailable`，零 fs/进程/锁变更                                         | `tests/cli_query.rs`                                              |
| 真实 staged-cohort Server：status/list/get/api snapshot、8 个准入加第 9 个被拒加释放、活跃 TUI                                                                           | 成功/`session-capacity`/结构不变                                                 | `tests/local_startup_process`                                     |
| 脚本化 OBLC/OBWP peer：denied、unavailable、畸形、错误 correlation、超时、断连                                                                                           | 按矩阵 typed kind；绝不冒称真实 Server 证据                                      | `tests/cli_query.rs`                                              |
| 脚本化 OBCS viewer peer：attach-binding 字段、`Empty`→`record` 轮询、detach 释放、reject/unavailable/畸形/correlation/断连                                               | `terminal read` 成功 + typed viewer-stage 失败                                   | `tests/cli_query.rs`                                              |
| 脚本化 OBCS viewer peer + 手工 OBRW record：滚动定价、容量/滚动钳制、typed `rows-rejected`、过期重钉、窗口失配、超大 record                                              | 保留行投影成功 + typed 失败                                                      | `tests/cli_query.rs`                                              |
| 脚本化 OBCS send peer：UTF-8 文本与可选折叠回车、语义键均以一条请求到达，target/请求身份，accepted/rejected/unavailable/unknown 应答、停滞应答、无重试                   | `terminal send` 退出 0/3/4/5/8 且两轴结果字段齐全                                | `tests/cli_send.rs`                                               |
| send 源语法：四源互斥、固定次序冲突命名、`--enter` 适用范围、flag 范围                                                                                                   | 任何运行时访问前的精确 parse/`invalid-usage`                                     | `src/invocation` 测试                                             |
| 文件/stdin 物化：非阻塞 open + `fstat` 类型闸、regular 文件共享 offset、私有 FIFO 重开、cap+1、不透明字节、空/非 regular/不支持类型的拒绝                                | 有界记录或类型化源 `invalid-usage`；零运行时副作用                               | `src/cli/source` 单元测试 + `tests/cli_send.rs`                   |
| 源 deadline/取消：停滞管道在共享 deadline 退出 7、被捕获合格信号按 `128+signo` 携带 `orchbee.cli.failure.source`、继承 IGN/BLK 原样保留、恢复失败即 internal             | 源 envelope 事实、无 wire 请求、精确 mask 恢复                                   | `src/cli/source` 测试 + 标注的 OS/进程证据                        |
| 脚本化 OBCS send peer + 声明型源：物化后的文件/stdin 以一条 `terminal.send` 文本请求到达                                                                                 | 与 `--text` 完全相同的 envelope；恰好一条请求                                    | `tests/cli_send.rs`                                               |
| Windows 自有管道读取器：原始字节、EOF、空输入、字节/粘贴上限、期限、注入取消、子进程回收                                                                                 | 精确载荷或类型化拒绝；无残留管道读取器                                           | `src/cli/source_windows/pipe.rs` 测试                             |
| Windows 公开进程与 PowerShell 5.1/7 管道源校验                                                                                                                           | 源完成后抵达与文本对照相同的源后拒绝；非法源先失败                               | `tests/cli_windows_input.rs`                                      |
| PowerShell 5.1/7 补全与离线本地化 skill                                                                                                                                  | 限定候选、重复键、粘贴互斥、根独占 skill 且不访问运行时                          | `src/cli/completion/powershell/tests.rs` + `tests/process.rs`     |
| 脚本化 OBCS viewer peer + 手工 OBTF/OBRW record：递增 `requestSequence`、可见/历史边界窗口、每帧真实 `FrameApplied`、stale 重钉、行尾 excerpt 命中、含 preamble 的总期限 | `terminal wait-output` 成功 + typed 失败                                         | `tests/cli_query.rs`                                              |
| 脚本化 OBLC/OBAP peer：list/get/explain 逐字 outcome、选项→wire 回显、denied/not-found/cursor/unavailable 应答、畸形/错 kind/correlation 不符帧、无重试                  | agent 读取成功 + typed `agent-read` 失败                                         | `tests/cli_agent.rs`                                              |
| Agent envelope ↔ 内嵌 schema：发出的成功/失败文档通过校验；wire 不可能变体（计数 129、重复 capabilities/ceiling）被拒                                                    | wire 契约的 schema 重述                                                          | `tests/cli_agent.rs`                                              |
| 真实 staged-cohort Server + 活跃 TUI 终端：`wait-output` 命中已有/后到/滚出屏/UTF-8 输出，短期限退出 7，取消后槽位可复用                                                 | 公共等待观察与 TUI 自身 viewer 共存                                              | `tests/local_startup_process`                                     |
| 真实 staged-cohort Server + 活跃 TUI 终端：`terminal read` 返回其网格并通过 schema 校验                                                                                  | 真实只读观察与 TUI 自身 viewer 共存                                              | `tests/local_startup_process`                                     |
| Mutation 语法：全部命令、target 规则、`--confirm`、几何界限、flag 范围                                                                                                   | 运行时访问前的精确 parse/失败类别                                                | `src/invocation` 测试                                             |
| Mutation 脚本化 peer：全部十五条命令、precommit 判定、不可判定与未提交结果                                                                                               | 每次调用一条命令；`outcome-unknown` 绝不重放                                     | `tests/cli_mutation.rs`                                           |
| Workspace close 语法：`close` target+`--confirm`；`close-status`/`close-resume` 严格稳定 ID、`--request`/`--generation` 栅栏、陈旧但合法的 generation 可解析             | 在任何运行时访问之前给出精确 parse/`invalid-usage`                               | `src/invocation` 测试                                             |
| Workspace close envelope：close-status 逐字 outcome（含 tombstone）、resume 源回显、封闭的 close-envelope details                                                        | 每个 fixture 通过校验；被拒变体失败                                              | schema 对照测试                                                   |
| Worktree list 语法：仅 `--directory`，拒绝 Project、严格目录 ID 值规则、flag 作用域                                                                                      | 运行时访问前即精确 parse/`invalid-usage`                                         | `src/invocation` 测试                                             |
| Launch-profile 语法：四条命令路径、逐字 selector、`--name`/`--exec` 必填对、`--profile` 互斥与作用域                                                                     | 运行时访问前即精确 parse/`invalid-usage`                                         | `src/invocation` 测试                                             |
| Launch-profile 文档/失败 envelope：条目成员名、封闭 details 词汇、双 `source` 形态                                                                                       | 每个 fixture 校验通过；被拒变体失败                                              | schema 对照测试                                                   |
| Worktree list 文档/失败 envelope：条目 XOR 字节字段、bare/detached/HEAD 不变量、封闭 details 词汇                                                                        | 每个 fixture 校验通过；拒绝变体失败                                              | `tests/cli_query.rs` schema 对照测试                              |
| Worker 发现 job：真实一次性 Git 仓库（init + commit + detached 链接 worktree，仅逐命令 `-c` 身份）、共享 TTL/stdout 预算归因                                             | 逐字 `-z` 字节、真实 HEAD/ref、类型化 outcome                                    | `apps/orchbee-worker` `worktree_discovery_job` 测试               |
| Server worktree-list 处理器：`-z` 解析接受/拒绝矩阵、membership 权威、outcome→失败映射                                                                                   | 字节精确投影；畸形 listing 即 `check-failed`                                     | `apps/orchbee-server` `worktree_list` 测试                        |
| 非 TTY；stdout 阻塞时 session 已释放；各 format 预备 >8 MiB；真实写失败                                                                                                  | 文档化退出；阻塞前 transport 句柄已 drop                                         | `tests/local_startup_process` + emit 单元测试                     |
| `config check` 矩阵：每项 pass/fail/not-applicable、`exists` 语义                                                                                                        | 五项运行时检查齐全；Windows 额外校验用户配置                                     | `tests/cli_query.rs` + `src/cli/config` 测试                      |
| `agent doctor` 语法：provider/asset-root 准入、去重、上限、帮助路径；合成 fixture → 全序判定（codex 阶梯、guarded/capsule/root/socket 探测）、封闭词汇                   | 确定性离线事实；产出文档通过 schema 校验；无 provider 执行、无 socket connect    | `src/invocation` 测试 + `src/agent_doctor` 测试                   |
| `server start` 真实 staged cohort：冷启动拉起+准入、第二次/并发启动、无属主标记拒止、持锁 deadline、cohort 故障、信号、非 UTF-8 base、zh-CN 文本                         | `started`/`already-running` 分支真实；被拒 wedge 后标记逐字节不变；deadline 有界 | `tests/local_startup_process`                                     |
| `server diagnose` 语法 + 观察矩阵：仅 format 选项、每行 state/reason、条件性通告、held/free/unsafe 生存期锁、未准入父级、耗尽预算、零创建                                | 精确 parse；封闭事实行；无 connect/创建/修复；无存活声明                         | `src/invocation` 测试 + `src/cli/site_diagnose` + rendezvous 测试 |
| 补全脚本投影语法（bash/zsh 实执行，fish 结构对齐）                                                                                                                       | 正/负上下文；无运行时枚举                                                        | `tests/cli_query.rs`                                              |
| 双语文本/失败输出                                                                                                                                                        | 精确 en-US/zh-CN 字符串                                                          | `tests/process.rs` + `tests/cli_query.rs`                         |

此切片不增加 Server 状态机、持久化或重试。`terminal read` 使用既有 `terminal.viewer` attach/poll/detach 生命周期；来源选择增加有界 `read-rows`/`rows-rejected` 交换，`terminal wait-output` 对每个已应用帧重复 poll 加一次有界 `read-rows`。命令结算义务——标志采样、清理来源与迟到采样——使用 [CLI 等待证据](native-cli-wait-model.zh-CN.md) 中的实现测试。共享协议变化需要受影响保留核心；inventory 输出本身不是执行证据。

## 单次启动 cwd 与环境

`tab create` 和 `panel split` 接受绝对 `--cwd` 及重复的字面量
`--env NAME=VALUE`，独立于默认 shell、`--exec` 或 `--profile` 选择。
显式 cwd 仅为该次启动覆盖已接纳的 Workspace 目录；二者都缺失时沿用 Worker
主目录默认值。Worker 放置、Workspace 成员关系及存储的 profile 不变。
Host 校验实际目录并报告既有启动失败结果，不创建目录，也不静默替换为回退路径。

环境覆盖继承/默认值：最多 32 组，名称和值合计最多 1024 个 UTF-8 字节；
名称是 1–148 字节 ASCII 标识符。禁止 NUL 和忽略大小写后重复的名称，允许
空值及值中的字面等号。`ORCHBEE_*`、`TERM`、`COLORTERM`、`TERM_PROGRAM`
按忽略 ASCII 大小写保留。不进行 shell 展开、profile 修改或全局环境修改。
用法诊断和 debug 投影不包含环境值。

选项参与请求/重放身份。待处理的 Tab 和分屏在提交前及完成时核对解析后的
cwd 和环境。空选项保留旧 JSON 和二进制编码。非空 OBCN 环境使用旧布局非法的
零可执行文件长度前缀，随后为扩展版本 1、可选显式程序及有界长度前缀键值组。
新解码器接受旧载荷；旧解码器拒绝扩展，不会丢弃。交付使用匹配 cohort 哈希。
最大 cwd、argv 和环境合计保持在40,960 字节帧上限内。

证据覆盖构造器/解码器拒绝、新旧编码往返、组合上限、CLI 作用域和脱敏、Server
组合及重放身份，以及公开 Tab 和分屏命令启动的 Windows 子进程观察真实 cwd/环境。
不新增 macOS/Linux 适配器或运行声明；此流程采用实现测试。

原生启动命令使用独立的 96 KiB JSON 请求预算，使 cwd、argv、label 和环境的最大转义编码可同时容纳。OBWP command kind 2 在分配前按此上限检查；非启动 intent 仍为 16 KiB，结果沿用既有上限。OBCS v2 帧允许 63 KiB；OBCN 允许 40 KiB。OBCS v2 schema/corpus 已覆盖这些启动值；共享 Workbench schema 和 Desktop binding 仍不构成对这些原生扩展的支持证据。

## 调用者 Panel 上下文

`panel current [<panel-id-or-selector> | --current] [--client <id>]` 按
[当前 Panel 契约](native-current-panel.zh-CN.md)读取 Windows Panel，与
`panel process-info` 共用目标解析。显式 Panel 选择器忽略调用者环境，无需 Client。
显式 `--client` 忽略调用者环境，与显式 Panel 选择器互斥。否则仅当两个调用者变量
均缺失时允许回退至唯一 current Client；部分缺失、格式错误或过期的调用者信息
会失败。当前 Panel 读取接受 Ready 或 Busy Client；process-info 仍只接受 Ready。
Panel 必须仍属于其报告的 Context 和 Tab。
Client 选择歧义时必须使用 `--client`。

Windows Worker 从已接纳的启动授权写入子进程的 `ORCHBEE_TERMINAL_ID` 与
`ORCHBEE_WORKER_ID`，覆盖继承的冲突值。这些产品身份是查询提示而非凭证；两者
均须为非零、完整的 32 位小写十六进制 ID。格式错误、空值、非 Unicode 或部分
缺失返回 `invalid-usage`（退出码 2），提供本地化指引且不回显变量值。
显式 Panel 或 Client 模式绕过这些元数据。

一份已验证快照负责 Panel 成员关系。有效 Terminal/Worker 绑定解析其所在 Panel，
包括子 Presentation 和已移至另一 Tab 或 Workspace 的 Panel。绑定缺失或不匹配
返回 `not-found`（退出码 3）；身份重复返回 `protocol-invalid`（退出码 5）。
无效调用者绑定绝不回退。快照与可选 Client 观察并非原子操作，答案受观察时点限定。

Windows 当前 Panel 使用 `orchbee.panel.current` v8，显式携带 `currentPresentation`：
显式 Panel 使用主项，调用者使用精确终端，Client 使用实际所选成员。Agent、终端标题／cwd／
进程、报告、元数据和滚动共用该成员，同时保留规范主项／子项结构。显式／调用者查询保持
无界面可用。原始 Client 视图／修订及本地成员标识经过最终校验；变化时不输出部分结果。
五秒期限、取消、只读操作、Viewer 清理和 Windows 证据由所链接
契约负责。目录 v100 含 92 个 schema。不产生激活、输入、控制权或焦点效果；
非 Windows current 保留 v3。此流程采用实现测试。

Windows Worker 身份在每次连接 Server 时由已有 OS 随机源重新生成；同一 Worker
的终端共享该 ID。不同 Session 可能复用 Terminal ID，因此精确匹配 Worker 可拒绝
来自上一轮或另一 Session 的调用者。不引入重连或接管。Tab close 接纳并非移除：
Server 发布移除前，查询仍反映成员状态；发布后才找不到旧绑定。

## 目录工作区创建

Windows `workspace create` 的[创建契约](windows-workspace-creation.zh-CN.md)拥有可选 Project、名称、来源、目录及聚焦。默认创建未分组 Workspace 并启动默认 shell，无需 Project/名称表单，不自动启动 AI。显式高级选项保持可用。`orchbee.workspace.creation` v2 记录 Workspace、目录来源和可选聚焦；无 Project 准备阶段。标识和终端事实位于 `workspace.result`，其中 `projectId` 必填但可空。

Worker 先探测请求目录，Server 再用一次修订原子登记 Workspace、目录、Terminal Context 及首个 Tab/Panel/Presentation。准入失败时不创建这些资源。提交成功保留嵌套 mutation v6 信封：action 为 `workspace.create`，包含 `projectId`、`workspaceId`、`directoryId`、`terminalContextId`、`tabId`、`panelId`、`presentationId`、字面 `path`、`label` 和 `terminal`。Project 元数据必填可空；目录 v143 发布此变更契约。终端可能在提交后仍待定或启动失败：保留返回 ID，查询 Workspace，不再需要时显式关闭。响应未知时绝不自动重放或回滚。

原生 `workspace.launch` 使用 tagged placement：`ungrouped`、`project`、`new-project` 或 `alongside`。CLI 普通创建选择 ungrouped 或显式已有 project，并携带名称、尺寸和必填 cwd；launch/profile 保持互斥。Server 回执包含精确结构标识及可空 Project 元数据；pending/launched 分别保留准入和终端结算事实。激活与回退使用同一完整匹配 cohort。

## Agent Panel 目标模型

面向单个 Agent 的命令接受 `--panel <id|@panel:suffix>`，与位置目标、`--agent-session`
和 `--name` 互斥。位置参数中的完整 Panel ID 优先按保留的精确 Session ID 或运行时
名称解析；只有明确未找到才回退到 Panel。显式 Session／名称不回退。带 `@panel:`
前缀的目标直接选择 Panel，不查询名称。Workbench 拥有 Panel 身份及其主 presentation；
Agent 状态拥有存活、可导航的 presentation 附件。隐藏附件仍可选。CLI 在持有的同一
Client Session 上读取一次 Workbench 快照，将主 presentation ID 交给现有 OBAP 名称族。
其 presentation 选择器在 Agent 状态折叠内完成解析、分配或释放：零个存活匹配为未找到，
一个为选中，多个会话为歧义。同一会话的重复附件只计一次。不分页、不取首个匹配，
不推断终端文本，也不维护客户端名称表。

工作流为解析 → 仅接入已有 Session → 可选的精确 ID／名称解析 → Panel 快照解析 →
OBAP presentation 解析／操作 → 固定 Session 的读取、等待或终端操作。全程共享调用者的
截止时间和取消唤醒；最终名称变更之前的失败属于读取失败，不是结果未知的变更。变更
回执丢失仍为 outcome-unknown，绝不重放。等待者不会跟随 Panel 后来的使用者。终端操作
保留选定的主 presentation，发送前拒绝缺失绑定或已改变的 Agent 附件；Server 仍拥有
终端授权。Workbench 和 Agent 读取是顺序采样，不承诺跨所有者原子事务。
Session／名称行为和已有回执来源不变。解析回执可报告 `resolvedBy: presentation`；
歧义是有界拒绝，不附候选列表。

原生语法与 Panel 投影测试覆盖选择器互斥及主／子 presentation 区分；Agent 折叠测试
覆盖零／一／多会话、已结束／不可导航／隐藏附件及重命名的原子拒绝。Windows 进程测试
覆盖共享接入、ID／名称优先级、固定目标等待、变更前读取失败和精确终端选择。
真实 Session／Worker／ConPTY 证据覆盖集成命令路径。这些检查不证明 provider 前台所有权
或就绪状态。此流程采用实现测试。

## Panel 进程信息

### Windows 解释与 Agent 输入预检

`src/cli/processes.rs` 负责对经过校验的 Host 样本作有界、无状态的解释；进程采集仍只归
Host 所有。每个进程增加可空 `argv` 和 `providerId`；`foreground` 返回 `kind`、可空
`pid` 与可空 `providerId`。进程结果为 version 2。通过现有安全 `winsafe` shell 绑定
调用 Windows `CommandLineToArgvW`，只解释非空、不含 NUL、至多 4096 个 UTF-16 单元
的命令行。空或不可用命令保持 null，不替换成 CLI 自身命令行，不做 Shell 展开、文件
查找或环境读取。这是 Windows 参数解析投影，不保证另一程序采用相同解析器。仅 Windows
的 feature 不增加 crate、版本、构建脚本或进程执行。

可执行映像必须直接标识 Agent，或者是已知运行时且其真实脚本入口标识 Agent。
eval／command 内容和任意后续参数都不能标识 Agent。已知包入口与可执行别名采用
显式列表。provider 匹配是诊断启发式，不是可执行文件认证；它不创建 canonical
Agent 状态，也不代表已就绪。

| 样本解释                        | 前台投影                            | 输入预检                        |
| ------------------------------- | ----------------------------------- | ------------------------------- |
| 一个 Agent 候选是所有候选的祖先 | `agent-chain`，选中 PID/provider    | 必须匹配预期 canonical provider |
| 多个无祖先包含关系的候选        | `ambiguous`，PID/provider 为 null   | 拒绝                            |
| 没有候选，保留的根进程仍存在    | `root`，根 PID，provider 为 null    | 拒绝                            |
| 根进程缺失且没有唯一候选        | `unavailable`，PID/provider 为 null | 拒绝                            |

父边只连接样本中创建时间不晚于子进程的父进程。遍历受样本大小限制并检测环；复用的
父 PID 不能伪造祖先关系。它采用 Herdr `e0507237` 的 Windows 祖先链选择意图，但范围
限制在 Orchbee 保留的 Job 内，不做全局扫描或 Git Bash 环境标记逃逸回退。歧义明确
返回，不伪装成可用 Shell；不声称存在 POSIX 前台进程组。

在规范会话目标上，Windows 的 `agent send-keys` 和 `agent prompt` 先解析 canonical 会话及其精确终端，
在同一已接纳会话上查询该终端、核对预期 canonical provider ID（非可执行别名），然后在发送前重读固定 Agent。
第二次读取必须保持 provider、live 生命周期及选中的可导航 presentation 和 generation；prompt 也
使用最新观察检查 freshness 与 blocker。全部步骤共享原有五秒输入截止；每个 viewer
在下一步前尝试 detach，会话 drop 作为清理兜底。预检失败不发送输入。低层 `terminal send`
继续用于显式终端输入。检查和后续输入不具备原子性，进程可能在观察后退出；原有发送
回执与未知结果规则仍适用，不重试，也不声称持有进程租约。

运行时不匹配使用 `orchbee.cli.failure.agent-runtime` v1，携带相关进程 source 和有界
`stage/reason/sessionId/expectedProvider/detectedProvider` 详情。未知 provider、Agent
缺失、候选歧义、provider 不匹配与 canonical 绑定变化分别拒绝；采集和传输失败保留
原类型。验证覆盖 Windows Unicode／引号、空命令保护、运行时 eval 诱饵、祖先／环／
复用父 PID、封闭结果 schema、真实 CLI／命名管道预检顺序及零输入拒绝，以及真实隔离
Windows Job。上述检查不证明 provider 任务完成、身份认证或原子前台所有权。

`panel process-info [<panel-id> | --current] [--client <id>]` 读取精确 Terminal 的 Windows Job。位置参数与显式 Panel 目标互斥；完整 ID 和 `@panel:<suffix>` 使用现有 Panel 解析器及主绑定。没有显式选择器时，有效的 `ORCHBEE_TERMINAL_ID` 和 `ORCHBEE_WORKER_ID` 保留调用者的精确主终端或子终端；部分或过期元数据失败。调用者元数据缺席时要求唯一当前 Client；`--client` 指定精确当前 Client 并忽略调用者元数据。Client 选择被动解析实际成员，并在发布前验证同一绑定及本地附着标识。[当前 Panel 契约](native-current-panel.zh-CN.md) 负责修订栅栏、期限和取消。成员不可用时不回退到主终端。

一次仅接入既有 Session 的 Client Session 与一个绝对五秒预算覆盖快照、临时只读 viewer 接入、一次进程读取及尽力 detach。会话 drop 回收剩余 viewer。Server 再验证 Worker generation 与物理终端绑定；Worker 和 Host 保留进程事实授权边界。成功时发布确切的类型化 Windows Job 样本、Panel/Terminal 路由上下文和相关联的读取来源。顺序获取的快照与样本不宣称跨所有者原子观察。Host 失败保持明确，不转为空进程成功，也不重试。文本中的字符串经过转义，机器输出的创建时间保留十进制字符串。响应描述归属终端的进程，不代表 Windows 前台进程组或 Agent 的交互就绪状态。

JSON 封套为 `orchbee.panel.process-info` v2，schema 目录及双语帮助、补全与命令共同交付。证据覆盖隐式和显式目标、I/O 前的冲突拒绝、主与子终端选择、主绑定缺失、读取拒绝、响应身份错误、Host 错误保留、输出限额、viewer 回收，以及真实 Windows Session/Worker/Host 进程路由。共享类型化读取也是后续 Agent 启动和就绪工作的实际依赖，本身不关闭这些义务。

Host 的预期拒绝使用 `orchbee.cli.failure.process-info` v1，保留有界的错误代码；非法响应与传输失败使用通用 CLI 失败契约。成功的 `source` 同时记录快照 revision、OBCS request ID 与 Host correlation、epoch、attachment 和物理 Terminal ID；它们不是同一个身份。目录 version 39 包含 74 份 schema。

## Windows Agent 启动

[Windows Agent 启动](windows-agent-start.zh-CN.md)拥有 `agent start`、名称预留与就绪流程、Windows 引用、预算和恢复契约。`agent names` v2 独立显示启动预留；名称失败对 launch-pending 使用 v3。Schema 目录为 v69，含 78 个 schema。

## Windows Session 停止

托管 Windows Server 在 Workbench 提交变化后以 500ms 防抖保存，每 30 秒补存采样所得的逐终端目录及 provider 恢复引用，并在正常停止释放终端前执行最新 final 保存。后续已准入启动恢复布局／Launch Profile，并启动新的默认 shell 或所选 provider 恢复。保存的终端 cwd 优先于工作区／home 默认值；保存目录不存在时回退到 Worker home。保存失败保留此前的有效存档、无效原件及运行终端，解决后可显式重试。启动会保留不可读存档并回退到较早有效副本；全部副本无效时会提示并打开空工作区。归属的[检查点契约](../../orchbee-server/docs/workbench-checkpoint.zh-CN.md)定义兼容性与恢复边界。不恢复旧进程、活跃 Agent 状态、Client 视图、终端屏幕或历史。

`server stop --confirm [--request-id <32-hex>] [--retry] [--detach] [--timeout <ms>]` 停止现有托管 Windows Session，绝不激活新 Session。标识为非零小写 128 位十六进制值，省略时仅生成一次；`--retry` 必须提供标识。CLI 仅发送一次开始或重试请求，之后仅查询状态。`--detach` 立即返回已准入尝试；否则一个绝对截止时间（默认 65000 毫秒，范围 1–3600000 毫秒）覆盖运行时/cohort 准入、请求和轮询。取消或超时仅停止等待，不回滚已准入停止。每个运行时答案均保留请求标识，供 `server stop-status <id>` 查询。状态是事实文档，不是第二次变更。`orchbee.site.stop` v1 封套区分 stopped、pending、failed、busy、not-recorded、unmanaged 及 unavailable/unknown 传输结果；pending 可携带 detached、timed-out 或 cancelled 等待状态。已知 Server 失败保留尝试、阶段、原因及本次释放数量。只有准确匹配的严格已停止激活记录加上空闲的既有生命周期锁才能证明完成；端点消失、非法记录及不同标识均不能证明。后续启动消费该回执，因此未记录不能否定此前停止。Linux/macOS 返回 unsupported-platform，不产生运行时效果。

## Windows 保留 Worktree 操作

[Worktree 契约](windows-worktrees.zh-CN.md) 拥有 `worktree create/open/remove/status/resume`、来源／Project 推导、各命令 flag、操作身份、确认、等待、输出与恢复规则。它扩展本记录早先仅 list 的语法：Create/Open 接受可选 --client、--focus／--no-focus、标签和终端尺寸、源 Workspace 或 cwd，以及路径／分支选择器；Create 另接受 base。Remove 接受 force，要求 Workspace 及确认；Resume 要求操作 ID、generation 及确认。五条均接受 text/JSON；只有修改命令按该契约接受请求身份、信任与等待。`orchbee.worktree.operation` v5 封套包含可空的原请求 Project 元数据、可选聚焦及 shell 恢复回执，无 Project 准备阶段。其 schema 为旧消费者保留封闭的 v1–v4 文档；当前目录见上文。

Windows `worktree list` 额外接受来源 Workspace/cwd 与可选命令局部信任，默认采用已验证调用者／所选 Client，均无时才使用 cwd，无需 Project。本记录前面的已注册目录语法及封套仅适用于注册 `--directory` 形式（v2，不含 Project）。[来源发现契约](windows-worktrees.zh-CN.md#按来源发现-worktree) 拥有独立的 `orchbee.worktree.query` v2 结果（目录 v143）、互斥规则、总 deadline、失败退出码及已知 Workspace 关联。

## Project 全部关闭投影

Windows `project close-all <target> --confirm` 显式关闭一个 Project 捕获的成员，支持 `--request-id`、`--detach` 和 `--timeout`（1–3600000 毫秒，默认 120000）。`project close-status --request <operation-id>` 使用五秒纯读取预算；`project close-resume --request <operation-id> --generation <1..64> --confirm` 使用相同 detach/timeout 选项。普通 `workspace close` 只关闭选中 Workspace；旧 `--group` 形式拒绝。两种关闭都保留 Git checkout/分支；Project 全部关闭仍保留 Project，dissolve 仅取消分组而保留 Workspace。

`orchbee.project.close-all` v1 将 Server 保留事实与可空等待状态分开。source 为 `local-workspace-group`，Session ID 必填可空。结果只有 Project scope；至多 256 个捕获成员仅携带 Workspace ID、label 和可空子 Close，空组有效。Start/Resume 只提交一次，后续仅读取相同 attempt/成员；未知不重放。退出码和保留部分事实见 [Project 全部关闭](windows-worktrees.zh-CN.md#project-全部关闭)。帮助、`--skill` 与补全都离线声明该命令。

## Windows Client 导航

[Windows Client 视图控制](windows-client-control.zh-CN.md) 拥有 `client list/get/control-status`、`workspace focus`、`tab focus`、`panel focus` 和 `panel zoom`。导航通过 `--client` 或无歧义且已校验的调用者上下文定位一个 current TUI。只提交一次，并等待绘制确认；超时或 Ctrl+C 仅停止等待。单 Panel 缩放有明确无操作回执。目录 v69 包含 78 个 schema，新增均为 v1 的 `orchbee.client.view`、`orchbee.client.control` 和 `orchbee.cli.failure.client-control`。未增加操作系统窗口激活或 Mac/Linux 控制运行时。

## Windows 离线 schema 导出

不带 ID 的文本 `api schema` 列出目录：每个 schema ID 一行，含其版本和标题，再说明如何读取单个 schema 或全部文档。`api schema <schema-id>`、`api schema --format json` 和文件导出输出 JSON 文档。`api schema [<schema-id>] --output <path>` 显式发布与 `api schema [<schema-id>] --format json` 写入 stdout 完全相同的完整 JSON 文档。`--format text|json` 控制失败呈现；成功时 stdout 与文件字节均为带末尾换行的相同 JSON。省略 `--output` 时保留现有只读行为。

解析器接受非空、不超过 4,096 UTF-8 字节的字面文件路径，在副作用前校验全部参数，不展开变量或前导波浪号。相对路径以调用进程目录为基准。拒绝 Windows 设备名、备用数据流和含糊的盘符相对路径。此命令不选择或启动 Session，不读取运行时配置，也不从网络获取 schema。本次不新增 Mac/Linux 文件导出运行时。

Schema 所有者先解析内嵌 ID，并在任何文件操作前准备完整的有界文档。未知 ID 保留现有 not-found 结果且不创建内容。Windows 导出器解析目标，拒绝已有的非普通文件或重解析目标，以独占方式创建随机同目录临时文件 `.orchbee-schema-<nonce>.tmp`，写入并同步完整文档，关闭句柄，再通过一次重命名发布。允许替换已有普通目标文件；不创建父目录，不回退为截断写入，不自动重试。

| 阶段                   | 结果与归属                                                          |
| ---------------------- | ------------------------------------------------------------------- |
| 语法或内嵌查询失败     | 无文件副作用；沿用现有类型化失败                                    |
| 目标或临时文件准备失败 | 目标不变；返回本地导出失败                                          |
| 写入、同步或发布失败   | 未成功发布；删除自有临时文件并报告具体失败阶段                      |
| 临时文件清理失败       | 保留并报告其路径，供调用者删除自有残留                              |
| 重命名成功             | 完整目标已发布；输出预先准备的 stdout，后续 stdout 失败不会回滚文件 |

专用 `orchbee.cli.failure.schema-export` 第 1 版文档包含 `write-failed`、封闭阶段值、字面目标以及可空的残留临时路径，退出码为 1。只有 `--format json` 会在 stdout 输出该文档，并始终在 stderr 给出本地化诊断。不改变 provider、终端、规范 Workbench 状态或运行时身份。进程崩溃可能遗留可识别、尚未发布的同目录临时文件；发布不声称父目录具有断电持久性。

维护中的 Windows 进程检查覆盖中文／空格和相对路径、完整及指定 schema、替换、语法／未知 ID 隔离、缺失父目录、目录目标、锁定目标时原字节保留、失败输出 schema 校验及正常清理无残留。选定文件导出能力以 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/cli/api.rs` 为参考；Orchbee 保留自身 schema 封套及完整文件发布约束。

## Windows Panel 布局查询

`panel layout`、`panel neighbor --direction left|right|up|down` 和 `panel edges` 检查精确的 Windows TUI Client。它们接受位置参数 Panel 或 `--current`，路由有歧义时使用 `--client`。读取保持焦点和终端尺寸，支持非活动或未访问目标。`orchbee.panel.layout` v1 保留单元格几何、可为 null 的隐藏矩形、未缩放邻居、物理边缘结果及操作回执。参见[几何契约](windows-client-layout.zh-CN.md)。目录 v69 包含 76 份文档；原生命令清单包含 94 条路径。

`panel focus [source] --direction left|right|up|down` 在指定 TUI 的未缩放单元格布局中查找一个邻居，保留缩放；无邻居返回 `direction-unchanged`，不切换到来源 Tab。找到邻居后固定目标，仅在绘制成功后返回 `direction-focused`。回执查询不重复移动。

`panel resize [panel] --direction left|right|up|down [--amount <number>]` 调整一个符合条件的分割边界，保留焦点和缩放。`layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` 设置精确二进制分割路径。两者都路由到一个 Windows TUI，并保留 `layout-adjusted` 回执。当前标签页的变化等待绘制，非活动标签页只保存、不导航。有限比例限制为 0.1–0.9；有限 resize 数量转为绝对值且最多 0.5，默认 0.05。比例归该 TUI 会话所有，重启后不恢复。参见[布局契约](windows-client-layout.zh-CN.md)。

## 共享窗格标题

`panel rename <panel-id> --label <text>` 设置共享主 Presentation 的自定义标题；`--clear`（或空白文本）恢复创建时的标签。接受精确 ID 或 `@panel:<suffix>`，不改变焦点或终端。[窗格标题契约](native-panel-labels.zh-CN.md) 负责规范化、修订与重放规则以及匹配 cohort 的兼容边界。`orchbee.panel.rename` v1 区分 `renamed` 与 `unchanged`，可空 `customLabel` 回显规范化后的请求元数据。`panel get` 与 `panel current` v3 提供不变的 `label`、可空 `customLabel` 及实际显示的 `displayLabel`。`api snapshot` v6 保留规范的可选自定义标题。目录 v69 包含 76 份 schema，原生命令清单包含 94 条路径。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。Windows CLI 通过原 Client 单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

Windows `tab create` 和 `panel split` 在创建前解析 `--cwd follow|home|current|workspace|<absolute-path>`；默认 follow 采样已解析来源的根进程目录，v3 回执记录实际目录来源／路径。它们接受可选目标及标签，默认不聚焦；`--focus` 请求创建后聚焦，分屏可带 Client 独立的 `--ratio`。显式目标无需 TUI 即可创建。`orchbee.creation.result` v3 分别报告已提交创建与后续比例／焦点结果；后续失败不能当作未创建而重试。参见[原生终端创建](native-terminal-creation.zh-CN.md)，其目标、总预算、并发保护和恢复规则为这些 Windows 命令的权威。

Windows `panel input (<panel>|--current) --right-click orchbee|panel` 设置共享右键策略，不移动焦点。`--current` 要求有效调用者元数据，显式目标绕过它。`panel split --right-click` 随创建提交新 Panel 策略。默认 `orchbee` 将右键保留给 Client；`panel` 允许无修饰键的右键在启用鼠标上报的终端内容内透传。已准入的手势在策略改变后仍保留匹配的松开事件。路由、排队输入复核及失败恢复见[Panel 输入契约](native-panel-input.zh-CN.md)。本功能不新增上下文菜单。`orchbee.panel.input` v1 返回 `configured` 或 `unchanged`；`panel list` v2 与 `panel get/current` v3 提供 `rightClick`，`tab get` v2 和 `api snapshot` v6 接受可选规范字段。目录 v69 含 78 个 schema，原生命令清单含 94 条路径。

## Tab 插入边界

Windows 上，`tab move <id|@tab:suffix|label> (--order previous|next|first|last | --insert-index N)`
要求恰好一个位置选项。`N` 是原始上下文顺序中的零基边界，不是最终序号：0 表示首位，Tab 总数
表示末尾，源位置边界或紧接其后的边界保持顺序不变。解析接受 0..256 的整数；Server 拒绝超过
当前数量的值，不作截断。精确目标解析在既有总修改预算内使用一次快照、一个命令，不选择 Client
或隐式聚焦。

索引移动输出 `orchbee.tab.move` v1，包含 `action: tab.move`、请求的 `insertIndex`、精确上下文/Tab、
请求/Session/基础 revision，以及提交或未变化的 revision。方向移动保留 `orchbee.mutation.result` v4。
目录 v69 包含 78 个 schema 和 94 个命令路径。确定的越界拒绝保留 Server 原因。提交结果不明时
退役载体、报告 outcome unknown，绝不重放或补偿。原方向行为保留已有平台支持；此新增 CLI 形式
仅支持 Windows。不引入磁盘持久化或迁移。

索引命令运行时失败使用 `orchbee.cli.failure.tab-move` v2，新增确定的 `tab-move-order-conflict`：精确请求位置会破坏固定前缀或打散连续分组时返回该原因。方向移动仅针对这个新原因使用专用 v2 信封，其余方向和解析失败保留既有通用信封。内嵌 schema 保持 v1 封闭，拒绝在 v1 下使用新原因。拒绝保留原顺序、元数据和可用权威。索引载体仍将上下文关闭的 `terminal-busy` 视为确定的提交前失败，不扩展方向载体的该项契约。

PowerShell 补全将纯 ASCII 十进制整数 AST 常量按原始文本读取，因此 `--insert-index 2` 之后
仍可补全。此修复不求值或接纳变量、splat、表达式和脚本块。

## 按来源读取终端输出

Linux／Windows 读取使用 `--source` 和 `--lines`，默认 recent80。纯文本 JSON 为
`orchbee.terminal.output` v3，ANSI 为 v4。`--plain` 直接导出 UTF-8，与 JSON 互斥。
`--ansi` 用 SGR 保留带样式空格及 Unicode，不重放 PTY 字节或图形。拒绝旧读取参数。
[输出契约](native-terminal-output.zh-CN.md)负责来源范围、拼接、输出限额、游标围栏及样本验证。

来源等待默认 30000 ms 和 recent80，显式 `--forever` 才无限等待。输出 v3，指定 `--after`
时为 v4；未指定游标时既有输出也可匹配。匹配本身不表示命令完成。读取及等待不获取终端
控制权，也不改变 Client 焦点。

Windows `status server` 在 Client Session 准入前读取实际 Server 构建／协议／cohort 元数据；`status client` 离线检查当前程序，其 v3 JSON 会在 cohort 准入不可用时包含有界 I/O 诊断。两者支持 `--format text|json`，不激活 Session。裸 `status` 仍查询 Workbench。显式选择的 Session 不存在时，会提示运行 `orchbee --session <name> server start`；未显式选择时提示 `orchbee server start`。[运行时状态契约](native-runtime-status.zh-CN.md) 拥有五秒时限、未知元数据、兼容性、schema 和失败语义。更新渠道管理仍未提供。

## Agent 导航

Windows `agent focus <target> [--client <id>]` 将规范 Session 或终端 Agent 解析为一个可导航主 Panel。一次 `agent-focus` 控制在 Client 导航和完成期间保留精确 Agent 与 Presentation 身份。Client 复核自己的观察；物理目标还要求匹配 Host 帧。显式聚焦成功后，将所选 Tab 的当前已知版本标记已读，包括目标已经聚焦及外层失焦的情况。普通绘制和只读 `agent attention` 保持既有规则。`--request-id`、`--detach`、默认 30000 ms 总期限及保留状态恢复不变。Client 未就绪时在 Agent 读取前拒绝；目标丢失／替换会拒绝或报告导航未完整完成，不改选目标。`orchbee.client.control` v5 携带此操作，不激活操作系统窗口。[Agent 导航契约](native-agent-navigation.zh-CN.md) 负责身份、确认及最终验收。

## Windows Agent 变更页

`agent changes [--cursor <c>] [--limit <n>] [--format text|json]` 在现有五秒 Agent 读取总期限内读取一页有界变更。省略 cursor 获得当前头部检查点；将返回的不透明 cursor 用于后续读取。Limit 为 1–100，默认 50。不筛选、不订阅、不长轮询，也不自动重放。命令准入一个 Client Session，在中断仲裁前关闭它，并输出一个结果。Ctrl-C 保持共享取消/收尾契约。

`orchbee.agent.changes` version-1 信封包含 canonical changes/current/reset-required 结果及关联 Agent source。Reset-required 明确表示连续性丢失，必须重建基线。无效/其他启动实例 cursor、禁止读取与运行时不可用使用现有 Agent 失败家族，operation 为 changes。返回页不得超过请求 limit。JSON 保留类型化事实与游标；本地化文本展示状态、revision 与续读信息。Server 拥有保留范围、认证及一致历史；CLI grammar/help/completion/schema/Skill 公开此边界。其他平台运行时暂不支持此新增命令。

对于规范会话目标，Windows `agent prompt --wait` 在运行时预检后、单次提交输入前为保留的 Agent 变更日志建立检查点；检查点失败则不提交，其修订必须覆盖预检修订。每个完整验证的后续页保留短暂活动；fresh 且更新的 provider 版本必须显示 working/blocked 或 turn-finished 跃迁，才能打开活动门槛。只有 working/blocked 条件可匹配这个门槛事件；稳定状态条件必须匹配当前样本，因此短暂完成后又 working 时继续等待。单纯元数据或新鲜度变化不构成活动。保留五秒活动门槛、原始总／读取期限、固定身份和取消归属。当前规范样本须覆盖日志头。Windows v3 将可空 `matchedChange` 及其来源与当前观察分开；Linux 保持 v2。历史丢失或跨启动周期游标明确失败，并保留已接受的输入回执。这证明检查点后的已观察活动，不证明所提交回合的因果关系或独占归属。

对于规范会话目标，Windows `agent prompt <target> <text> --wait --client <id>` 在发送前预检一个精确 Client，之后每次采样读取其 attention。规范活动门槛仍适用。Idle/done 只匹配其当前经核实的静止状态投影；缺失／过期版本继续等待。Client 丢失或读取失败时停止等待，保留已接受的输入回执。Client 读取组合最多两秒，并受既有五秒评估及原始总期限限制。读取不确认已看见、不聚焦、不重发。所选 Client 成功封套为 v4，要求 `client`；普通 Windows prompt 保持 v3，Linux 为 v2。`--client` 要求 `--wait`，不新增 Mac/Linux 实现。

## Windows 统一 Agent 投影

Windows `agent list` 将规范 Server 会话和物理终端 Agent 合并为 `orchbee.agent.list` v2。`source.kind` 为 `agent-projection`；source 携带 `siteId`（32 位小写十六进制）、十进制字符串 `workbenchRevision` 和 `reportRevision`，以及安全整数 `canonicalRevision`。`result` 为 `{kind:"listed",revision,nextCursor,items,clientId?}`；`nextCursor` 为 null 或 `u1` 游标。每行包含一基 `ordinal`（1–4096）、`provider`、语义 `status`（`unknown`、`idle`、`working`、`blocked`、`done`、`error`、`ended`）、身份 `source`（`canonical`、`terminal`、`both`）、既有且可为空的 canonical `AgentSessionSummary`，以及包含既有 launch binding、非空 provider、`PanelAgentState` 和十进制字符串 sequence 的可空物理终端记录。`source` 控制分面是否存在。物理行和 `both` 行始终有 `placement`，其中 Panel、Presentation、Terminal、Terminal Context 和 Tab ID 为 32 位十六进制；`workspaceId` 可为空。canonical 行也可能有 placement，表示对应终端 incarnation 当前观测为没有 Agent。`cwd` 为可空字符串。使用 `--include-ended` 时，已结束会话排在所有当前 live 行之后，因此 live 序号保持稳定；ended 行按 canonical ID 排序，并可通过精确的 `--agent-session` 检查。

使用 `--client` 时，封套要求 `clientId`，每行也要求 `client`；canonical 和 terminal Client 分面保持独立。Client 未读状态不会提供 Agent 语义状态。每页最多为 64 行添加可选 cwd 信息；不可用时为 null。每页 1–100 行，默认 50 行；合并输出上限为 4096 行和 8 MiB。来源准入可能更早失败：canonical 来源限制为 512 个会话／六页各 100 行，物理来源注册上限为 1024 条。因此不能据 4096 行输出上限承诺同等数量的物理来源。

Windows 游标使用 `u1.<base64url>`，最多 1024 个字符。载荷为编码成 base64url 的规范 JSON。游标绑定 Site、来源修订、`includeEnded`／Client 筛选、页偏移和指纹；不保留文件。解析器识别出的旧格式游标会在读取实时来源前返回 `cursor-restart`。CLI 必须读取当前来源才能确定 `u1` 游标是否过期，因此只能在这些读取完成后拒绝过期的 `u1` 游标；调用方应从第一页重新开始。

Windows `agent get` 无论是否提供 `--client` 都输出 `orchbee.agent.get` v19，并使用相同的投影 `source`。`result` 为 `{kind:"projected",agent,inspection}`。`agent` 包含投影行中的 provider、status、source、canonical／terminal 分面、placement 和 cwd，但不包含 `ordinal` 或 `client`；`inspection` 是当前既有的 canonical 或 terminal inspection。详细进程、终端元数据、管理状态和 Client 证据仍保留在各自分面 inspection 中。

Windows Get、Read、Prompt、Send Keys、Wait 和 Focus 共用一个解析器。只有完整当前投影证明唯一时，`@agent:<kind>` 才选择提供方。`@agent:<n>` 选择完整列表顺序中的一基序号，不是页内序号；解析器只接受 live 行，因此检查 ended 会话时应使用精确的 `--agent-session`。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。部分读取不能证明提供方匹配唯一。修改效果保留完整物理绑定，并在实际写入前立即复核该绑定。

投影失败使用 `orchbee.cli.failure.agent-projection` v1。封闭的 `failure.details` 包含 `stage:"agent-projection"`、`unavailable`、`denied`、`changed`、`invalid`、`too-large`、`timeout`、`cancelled`、`cursor-restart`、`target-not-found` 或 `target-ambiguous` 之一作为 reason，并可包含 `selector`／`candidates`。Candidate 项携带 `selector:"@agent:<n>"`、可空 `sessionId`、可空 `panelId` 和可空 `AgentLaunchBinding`；最多返回 16 个候选。可选 envelope `source` 使用 Agent list/get 的同一类型化投影来源；失败时没有 `result`。

原生 provider 注册表现有 20 项，包含 Windows Cursor、Qoder 和 Antigravity JSON Hook 接入和离线诊断。Agent 投影文档已注册在 API 目录版本 148 中。

Mastracode 新增 Windows 扁平 Hook 接入、不完整安装修复、离线诊断及带运行标识的生命周期观察。

Droid 新增针对当前所选用户配置来源的 Windows JSON Hook 接入和诊断。

Kimi 新增 Windows TOML 会话 hook 接入和诊断；尚未声称完整支持工作中／阻塞／空闲观察。

Letta 新增 Windows 会话接入和离线诊断。提供方会将 stdout 作为上下文，因此 hook 输出保持静默。

Devin 新增六组身份 hook 的 Windows 接入和离线诊断。所有事件保留轮次状态；APPDATA 缺失或无效时拒绝设置。

Hermes 新增 Windows 插件激活及不含内容的会话观察。其离线 doctor 报告需要提供方命令检查。

## Windows 持久创建默认值

[Windows CLI 配置](windows-cli-configuration.zh-CN.md) 拥有有界 TOML 读取器、`ORCHBEE_CONFIG_FILE`、`[terminal] new_cwd`、`[worktrees] directory` 及创建副作用前的优先级。Windows `config check` v2 保留五项运行时检查并增加 `user-config`；v1 仍是五项检查。`orchbee.cli.failure.configuration` v1 报告有界准备错误。目录 v69 包含 78 个 schema 和 94 条命令路径。

当前 Windows `config check` v13 仍把致命文件和环境接纳错误报告为 `fail`／退出 1，并把已接纳的字段回退报告为 `warning`，同时保持 `ok:true`／退出 0。有界 `diagnostics` 与精确的 `diagnosticsOmitted` 数量不会放宽 v1-v12。`server config`／`server reload-config` v8 在每个成功快照中携带同一份已接纳警告记录，v1-v7 保持封闭。内嵌 schema 目录为 v148。

## Windows 终端观察流

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` 从一个已有绑定输出 NDJSON，不获取控制权，也不改变 Host 尺寸。[流契约](windows-terminal-streams.zh-CN.md) 拥有完整安全 ANSI 帧、重置、先刷新后确认、期限和清理规则。默认视口为 120×40，默认不限时。两种输出格式均产生 `orchbee.terminal.stream` v2 记录。目录 v69 含 78 个 schema、94 条原生命令路径。机器控制现通过 `terminal session control` 接收 NDJSON 输入；交互式 terminal/Agent attach 已实现，仍有保真与验收义务。

### Panel 迁移事实回执

Panel 迁移使用共享变更封装 v5；Workspace 与组织操作使用 v6，其他共享操作保留 v4。迁移可选提供 `transfer` 事实：`targetTerminalContextId`、
`closedSourceTab`，以及可选 `closedSourceWorkspaceId` 和 `createdWorkspaceId`。移入现有 Tab
也保留返回的 Split 和 slot 身份。移走 Tab 最后叶子时移除该 Tab，但不释放终端，结果明确报告移除。
本地传输在确认提交前检查目标种类／上下文、新 Workspace 字段、源移除的一致性及结果关联。
目录 v71 在此契约修订时包含 80 份内嵌 schema。

### Windows Panel 迁移流程

```text
panel move <panel> --into-tab <tab> --direction left|right|up|down [--target-panel <panel>] [--ratio <number>]
panel move <panel> --new-tab [--workspace <workspace> | --standalone] [--label <text>]
panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>]
```

各形式均接受 `--focus|--no-focus`、`--client` 和 `--timeout`。焦点默认为 true，
与固定 Herdr CLI `e0507237` 一致；其 JSON 方法另行默认为 false。
既有同 Tab `--target-panel --direction` 形式仍为单次变更。
比例必须有限并钳制到 0.1–0.9。新 Tab 标签可省略。

解析一次快照，以及显式、关联调用者或唯一当前 Client。歧义在产生效果前拒绝。
没有 Client 时，仅允许不带显式比例的 `--no-focus`：选择目标首个叶子和原生默认比例，
不声称已知 Client 几何或缩放状态。有 Client 时，只读来源／目标布局决定缩放状态及默认目标叶子。
来源／目标 Tab 已缩放及迁入同 Tab 返回未改变，不提交变更。显式目标 Panel 必须属于目标 Tab。

新 Workspace 只有显式 `--project` 才加入已有 Project；省略时未分组，不继承来源 Project，也不创建默认 Project。目录跟随已观测终端根 cwd；cwd 缺省时使用调用进程 cwd，非法观测拒绝。副作用前验证路径及名称。

阶段为预检 → canonical transfer（含新 Workspace 目录）→ 可选比例 → 聚焦 → 布局观测，共享绝对截止时间和取消。Server 原子提交拓扑，不重启终端；Worker 验证后 Directory 与拓扑共享提交。`orchbee.panel.move` v2 移除 Project 准备回执/阶段，仍独立保留 canonical 与 Client 结果。失败、取消或未知后不重放，不将部分成功报告为完成。

快照刷新后，比例针对返回的 Split／槽位身份。Client 后续操作等待权威状态收敛，
并保留未受影响的原选择；无关导航拒绝后续操作。所选来源被移除时允许普通选择修复。
其他 Client 保留独立导航。回执布局仅描述所选 Client。
未改变的空 Ready 视图可直接进入既有最低修订约束的控制，不等待无关导航心跳；
已观察修订不得倒退。未激活目标布局返回该 Client 记忆中的有效 Panel，无记忆时采用首个成员。
语法／决策测试、回执校验、可取消传输检查及真实 Windows 多 Client 活终端／重启验收覆盖这些边界。
通用事件及既有 child、分组、固定结构限制仍是独立义务。

目录 v72 包含 81 份 schema，命令路径保持 96 条。

## 可复用布局导出

Windows `layout export` 输出带比例、手工标签、观测目录及冻结启动命令的可复用 JSON 树。[布局契约](windows-layouts.zh-CN.md)拥有导出目标、Client 选择、边界、取消、无副作用与一致性规则，以及已实现的 `layout apply` 创建／替换流程和效果回执。`orchbee.layout.export` v1 和 `orchbee.cli.failure.layout` v1 在目录 v97（91 份文档）中引入。

## 程序化语义按键

`terminal send --key` 与 `agent send-keys` 共用解析器。除既有具名键外，接受无修饰的 Space、Insert、F1..F25、单个非控制 Unicode 标量、标点名称（`plus`、`semicolon` 等），以及 Control、Alt、Shift、Super 组合。别名包括 `control`、`option`、`cmd`、`command`、`return`、`esc`、`bs` 和 `C-c`。键名和修饰键名不区分大小写；ASCII 大写字符隐含 Shift。重复修饰键、多个基键、空分段和不支持的功能键编号在准入前报用法错误。字面 `+` 表示一个键；组合中用 `plus`。Shell 元字符请加引号。

旧拼写保留既有 wire 记录，扩展拼写通过两段原生协议的已校验 stroke 记录传递。Worker 提供物理身份、逻辑文本、未修饰标量和修饰键；Shift 生成的可打印文本消费文本生成的 Shift，同时为协商后的完整按键报告保留该修饰键。Host 按当前终端模式编码，旧模式的 Super 字符组合保留为 CSI-u。既有可打印组合也携带文本，避免仅 Shift 的字母和 Space 消失。序列顺序、最多 64 键、共享期限、仅输入附件、前缀回执，以及部分或未知结果不重放均不变。子进程未启用相应键盘协议时，新增功能键可能被当前模式忽略；回执保留实际编码字节数。

同作者回归覆盖解析拒绝、独立 wire golden、非法标量与标志边界、混合记录边界、Worker 语义投影及 Host 旧模式/Kitty 编码。针对性验证不替代最终 Windows cohort 验收或全目标回归。

## Windows Panel 元数据查询

[Panel 元数据契约](windows-panel-metadata.zh-CN.md#查询观察模型)负责分批读取主项及两类最终修订号校验。[当前 Panel](native-current-panel.zh-CN.md#当前成员信息增强)则校验精确解析成员的 ID、Panel 与绑定。Panel list/get v9 和 current v8 直接暴露其元数据；Windows Agent Get v19 在 terminal `inspection` 中保留元数据及带来源的 `reportedAgent`。实际 Agent／权威上下文选择带守卫展示，同时保留原始来源记录与全局 tokens。规范 OSC 标题、名称、状态及就绪性仍独立。

启动选择另将最坏 JSON 字符串内容限制为 30 KiB：ASCII 控制字符计六字节，引号和反斜杠计两字节，其他 UTF-8 字节计一字节。原有 2 KiB 输入域全部保留，同时约束更大的恢复命令。扩大上限后必须使用匹配 cohort。

Windows Session 重启默认恢复保留的官方 provider 引用，每个不同的来源／Agent／引用种类／值仅尝试一次。激活前设置 ORCHBEE_RESUME_AGENTS_ON_RESTORE=false 可启动普通 shell 并保留引用；仅接受精确 true／false。已运行的 Session 不受影响。Server 读取 SystemRoot 解析系统 Windows PowerShell，并继承 PATH 发现 provider 程序。shell 启动不证明 provider 就绪。失败、去重及其余恢复边界参见 Server 的[重启模型](../../orchbee-server/docs/agent-restart-reference.zh-CN.md)。

## 已安装的 Agent 检测清单

Windows `server agent-manifests` 读取运行中 Session 已安装的规则、修订、引擎版本、配置目录及每个
provider 的实际来源、版本、路径、缓存和警告，也显示启动时冻结的更新目录源、来源错误、最近
一次更新事实及持久化状态错误。
`server reload-agent-manifests [--request-id <operation-id>]` 重读启动配置目录及已有状态，
重置屏幕稳定状态，并同时切换采样和 Explain。
`server update-agent-manifests [--request-id <operation-id>]` 通过 Server 持有的 Windows curl
工作线程下载并校验规则，提交符合条件的缓存版本，再启用当前本地覆盖、缓存或内置规则的选择结果。
本地覆盖继续优先。目录源由 `ORCHBEE_AGENT_DETECTION_MANIFEST_CATALOG_URL` 指定，默认
`https://herdr.dev/agent-detection/index.toml`；无效覆盖值停用 Update，但本地规则仍可使用。
来源在 Session 启动时冻结。`update.manifest_check` 为 true（默认）时，托管发布版 Session 在启动时及
每三十分钟自动检查；调试／测试及独立端点不调度。规则 Reload 也从冻结配置路径刷新此开关，
配置无效保留旧策略与检测器。自动结果使用独立的最近结果槽，不消耗手动回执。
`server agent-manifests-status <operation-id>` 读取重载或更新回执，不重新提交。
这些命令要求 Session 已运行，不会聚焦 Client 或发送终端输入。

每次调用共用一个五秒接纳及交换预算。Reload 和 Update 只提交一次，之后仅查询状态。
待处理或效果未知时保留操作 ID 并退出 8；只读传输结果不确定时退出 4。
拒绝、忙碌、未记录、容量耗尽、操作类型冲突、更新已停用时退出 3；不可用退出 4；
已取消操作退出 7；加载、修订耗尽或内部错误退出 1。只有全部 provider、状态持久化及规则启用
均成功时，更新才退出 0。即使部分缓存已写入，部分失败或启用失败、未启用仍退出 1。
晚到中断保留最后一份已接纳回执。ID 为非零的 32 位小写十六进制值；省略时自动生成。
相同类型复用已接纳 ID 只读原回执；跨类型复用失败。Reload 与 Update 共用一个活动槽，
每个 Session 代次最多保留 1,024 条回执，不淘汰。重启结束回执范围；最近缓存报告仍保留在磁盘。

`orchbee.site.agent-manifests` v1 对旧结果保持封闭。v2 增加 Update、维护信息、更新报告及
效果未知回执。v3 增加自动策略门控、设置来源及最近自动操作；v1/v2 保持封闭。报告将尝试版本、缓存版本、逐 provider 失败与启用修订、失败原因分开呈现。
文本本地化且转义。目录 v97 包含 91 个 schema；原生命令语法包含 108 个命令路径。
Server [已安装规则维护契约](../../orchbee-server/docs/agent-screen-model.zh-CN.md#已安装规则维护)
拥有缓存安全、安装、取消及回执上限的规则。

五条 Windows Panel 报告命令另接受 `--presentation <full-id>`，指定位置参数 Panel 内精确的主项或子项成员。省略时保留主项目标及 v1 回执；显式成员使用 v2 并必需携带 `presentationId`。[Agent 上报](windows-panel-agent-reports.zh-CN.md#精确成员目标)和[元数据上报](windows-panel-metadata.zh-CN.md#精确成员目标)拥有身份校验、字节预检与未知结果处理。目录 v97 仍含 91 个 schema 和 108 条命令路径。

## 来源所属 Agent 视图命令

Windows `agent view get`、`agent view set --spec <json>` 和
`agent view clear [--source <name>]` 使用所选 Session，包括 `--machine` 路由。
[来源所属视图契约](machines.zh-CN.md#来源所属的-agent-视图)拥有归一化、有界
过滤／多字段排序语义、临时来源替换、插件资格、Client 关注状态及 TUI 投影。
`orchbee.agent.view` v1 回执区分服务端失败、交换失败和未知修改；目录 v121 包含 105 个 schema
和 141 条原生语法路径。

## Agent 侧栏配置发布

Windows `config check` v10 与 `server config`/`server reload-config` v5 暴露必需的有界 `agentSidebarProfile`；配置失败 v7 增加侧栏接纳诊断。旧公开版本保持封闭。[Windows 配置所有者](windows-cli-configuration.zh-CN.md#windows-agent-侧栏布局)定义启动、致命来源边界、字段级准入、规范发布及 Client 原子应用。目录 v122 仍含 105 个 schema 和 141 条命令路径。

## Windows CLI 进程验证

Agent focus／Workbench 进程用例在每个测试 binary 内最多接纳四个存活测试 cohort。
复制／哈希隔离的原生程序前获取 slot，保留到进程输出、peer join 和私有目录清理结束。
等待夹具资源发生在命令计时前；产品与测试截止时间不变。该上限保护 Windows 冷进程准入
及可执行文件 I/O，避免同套用例中的其他场景争用，不串行化应用状态，也不减少检查场景。

viewport-timeout peer 可能在 CLI 发出最后一次状态轮询之后、收到 queued 响应之前到期时
观察到 `BrokenPipe`。此时停止写入；消费者仍须返回准确超时失败，且没有部分结果。
其他写入错误及畸形请求仍属于失败，连接关闭后不编造回执。

## 统一保留操作路径

[保留操作状态](native-operation-status.zh-CN.md) 定义推荐的 `<family> status <request-id>` 及支持的 `<family> resume <request-id> --generation <n> --confirm` 语法。本参考中较早的命令族专属路径保留为兼容入口，新调用者使用统一路径。Tab 和 Panel 现有显式状态及恢复入口。
