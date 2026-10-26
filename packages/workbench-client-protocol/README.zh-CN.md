# @orchbee/workbench-client-protocol

私有包入口只导出仓库内有消费者的符号，Knip 的 `includeEntryExports` 检查该边界。内部契约、生成资产与独立一致性证据按其所有权保留；它们的存在不自动扩大公共入口。新增客户端应按已准入用途添加所需导出。

`@orchbee/agent` 是独立 Client-view 一致性绑定的开发依赖。
发布的 JavaScript 入口及其声明不依赖 Agent。为该绑定添加已准入的运行时消费者时，
也必须同步更新依赖分类。

独立 Workbench 绑定位于 `src/__tests__/conformance/`，由现有 schema／语料套件到达。产品 session、projection、preference 和 launch schema 仍位于 `src/`。完整依赖分析包含这些一致性源码并拒绝孤儿 helper；生产分析仅排除该开发目录。

## Project 与 Workspace 组织

同批升级投影使用全局 Directory、Project 与 Workspace 注册表。Project 只是可选分组：Workspace 的
`projectId` 与 `primaryDirectoryId` 必须存在且可为 null，Workspace／Terminal Context 身份稳定。
身份数组始终按 ID 排序，独立于显示顺序。Navigation 校验引用、唯一 Context 所有权、65,536 个
全局 Directory、每个 Project 最多 256 个 Workspace 与全局最多 1,024 个 Workspace。非空
`projectOrder` 和 `workspaceOrder` 必须分别是对应注册表的完整排列；省略／空值使用规范 ID 顺序。

[Rust 组织契约](../../crates/workbench-protocol/README.zh-CN.md#project-组织与稳定-workspace-身份)
管理原子的 `navigation.configure` 成员／顺序变化、`workspace.set-project`、原子的
`workspace.create-project`、`project.dissolve` 和四种
`workspace.launch` 放置的共享语义。Launch 必须提供绝对 cwd，使用 96 KiB 启动类别；回执必须携带
可空 Project 成员关系。重新分组和解散保留物理资源与显示顺序。稳定 Workspace 的 rename／Close／
primary-directory 命令及全部 Close 身份省略 Project。Close 读取结果的精确上限为 1,000,007 字节。
Workspace 事件仅在 Workspace 内携带成员关系。所有绑定、schema 与原始／规范一致性语料在未发布
OBWP v1 内共同升级。

Workspace 创建、启动和重命名命令的标签必须包含非空白 Unicode 标量，与 Rust 和 JSON Schema
保持一致。校验精确保留有效文本，包括两端空格，并通过共享命令语料覆盖 ASCII 和 Unicode
空白。快照标签仍可读取，以便修复已有空白名称；其他标签类别维持各自规则。

严格命令绑定还接受 `workspace.close-if-idle {workspaceId}` 和
`project.dissolve-if-idle {projectId}`，拒绝调用方提供的空闲断言，并解码确定无效果的
`confirmation-required` 拒绝。既有已确认动作和接受回执的形状不变；条件与已确认 kind
属于不同请求指纹。
[Server 条件所有者](../../apps/orchbee-server/docs/terminal-tab-close.zh-CN.md#条件式空闲准入)
定义效果准入。绑定不能证明空闲或授权自动重试。

## 语义事件绑定

独立 `workbench.events` 绑定验证 28 种事件、封闭过滤器、物理／Client 载荷、全宽 cursor、
独立探测所有权、有序有界分页与历史 gap。它共用 Rust／JSON 事件语料及 256 KiB 配对 cohort
carrier 上限。可选 Client `splitRevision` 报告真实比例所有者计数，并要求已选择 context；
旧观察保留既有形状。本包增加契约，不增加 Desktop 事件消费者。
[Windows 事件模型](../../apps/orchbee-server/docs/windows-events.zh-CN.md) 管理运行时行为。

适用于一个 Orchbee Workbench 页面客户端的严格、运行时中立的数据协定。

## 适用范围

该包拥有共享 Workbench 客户端会话、终端投影控制以及为一个可见终端视口打开的投影附件流的经过验证的消息。 Electron、普通 Browser 页面和未来的移动客户端可以通过不同的传输方式携带这些消息，而不改变其 Workbench 客户端含义。

该 package 还提供增量 version-1 只读 Workbench projection kernel 的独立 TypeScript binding。
语言中立权威与 canonical conformance corpus 位于
[`schemas/workbench`](../../schemas/workbench/README.zh-CN.md)。Binding 解析一个严格的
`workbench.snapshot.read` intent、完整有界 snapshot 与 revision-chained aggregate delta；
`applyWorkbenchDelta` 原子应用 delta，或返回一个 typed recovery reason。该 contract 包含
Project、Directory、Workspace、Tab、Panel 与 Terminal Presentation 产品语义，包括 exact bounded
本地平台 path 与必填且可为空的 primary Directory reference，但排除 Environment、Directory discovery
或 mutation policy、Worktree、Agent、Launch Profile、terminal content、persistence、transport 与
所有 Client-local view fact。当前 client-session consumer 尚未使用这份新 contract。

独立 command binding 校验 `workbench.command` 及相关联的 command/snapshot result。它还覆盖携带
revision 游标的 `workbench.publication.read` intent 及其 `workbench.publication.result`；后者的
`published` outcome 在共享 snapshot-result 字节上限内携带 1 至 64 条有序的
`workbench.snapshot`/`workbench.delta` 文档。它包括 Project
与 Workspace 变更、`workspace.primary-directory.accept`、canonical `tab.rename`、Workspace 上下文内的 `tab.move` 与 `tabs.configure`，以及 已配置 Workspace 上下文内的 `tab.create`
及其已提交的 terminal result variant。Move 仅携带 `terminalContextId`、`tabId` 和
`direction: previous | next | first | last`；`tab-moved` 仅携带 `revision`、`terminalContextId`
和 `tabId`。
`tabs.configure` 校验封闭的分组／固定操作，以及 `tabs-configured` 中可空的分组身份；
[原生分组契约](../../apps/orchbee/docs/native-tab-groups.zh-CN.md)拥有运行时准入与回执规则。
选中集合操作要求 1–256 个唯一 Tab ID；`move-tab` 携带方向和临时的 `adjacentGroupCollapsed`
提示。Binding 不持久化 Client 的选择或折叠状态。
`move-tabs-by-drag` 携带同样的有界集合、0 到 256 的整数 `insertIndex`，
以及必填可空的 `groupId`，表示一次原子排列。
Server 拥有排序与 no-op 语义；消费 adapter 拥有请求/结果关联及 action 兼容性校验。
Binding 与两个 runtime binary 作为同一个本地未发布 OBWP v1 cohort 一起启用和回滚；旧的封闭
decoder 会拒绝 Move。它复用 projection identity、label、revision 和 snapshot invariant，拒绝未知字段，
并检查编码请求/结果上限。原生 Server 通过 OBWP 消费独立 Rust binding；本 package 拥有已解码值，
不实现 OBWP transport、duplicate-key decoding、重试、共享状态 mutation 或授权策略。

`tab.create` 和 `panel.split` 使用与 `panel.create-terminal` 相同的可选 launch／profile／options
字段、封闭嵌套结构、规范化和 96 KiB 命令信封。三者均保留字面参数和环境变量值；
同时提供非 null 的 launch 和 profile 无效。Tab 创建的默认 `automaticLabel:false` 与 Split
的默认 `rightClick:orchbee` 规范化为省略，与 Rust 序列化一致。null `options` 和 null 环境数组
无效；null cwd 和空环境数组省略。共享语料跨 Rust、TypeScript 和 JSON Schema 检查这些
既有原生启动能力，包括超过普通意图限额的扩展请求。本扩展不增加 Desktop 命令，
也不改变 Server 启动、授权、重放或回滚归属。Workspace 启动共享这些字段，并要求 cwd 与显式放置；
仅原生提供的 profile 目录操作仍在本绑定范围之外。

Workspace primary Directory 接受只携带 Workspace 与确切 path 输入。其 accepted result
携带 revision，以及 Server 选择的 Directory 与 Worker identity。Binding 将
`workspace-directory-unavailable` 与 `workspace-directory-conflict` 保留在 rejection domain，
将 `directory-unavailable` 保留在 command-unavailable domain。它校验封闭 wire 与 projection
值；文件系统观察、Worker 选择、授权、set-once 准入、提交与重放仍属于 Server 职责。

分阶段的 Close binding 增加 `tab.close`、不可变 `tab-close-accepted`、无副作用的
`workbench.tab-close.read` 与显式 `workbench.tab-close.resume` 及其严格结果。
`WorkbenchTabCloseAttemptGenerationSchema` 验证独立于修订的非零规范十进制 u64 值域。
封闭状态/阶段/原因联合将已知销毁、受阻（blocked）的清理、未知效果及最终移除保留为不同事实。
Resume 失败目标回显仅重复已提交身份，Denied 亦如此；不确认既有操作。
[Close 线协议契约](../../schemas/workbench/README.zh-CN.md#已接受的-tab-close-线协议契约)
拥有形状及共享 oracle（裁决基准）。这些已解码值解析器不激活生命周期效果，也不证明 Server/Worker/原生
集成；该批次必须一起推进。

每个`terminal-projection-open-requested`命令携带一个完整的正值`TerminalGeometry`。它是调用者测量的视觉表面的安装几何形状，而不是可选的调整大小提示或后备。因此，请求身份包括表示身份和几何结构：可以重放精确的副本，而不同几何结构的重用会与控制流量发生冲突。传输合同不提供超时或 80x24 默认值。

投影的附件流携带语义铃声事件，而无需选择声音、视觉闪光或通知策略。每个事件都保留 Host 附件身份和终端拥有的序列，因此预计的终端可以抑制过时或重复的传递。

附件 bootstrap 可以携带 `hostDefaultColors`，即所属运行时配置的颜色方案、前景色与背景色。
这些有界值与当前主题分离，使客户端在重开终端后可恢复宿主默认值，而无需从之前的显示覆盖推断。
它们不替换当前调色板、光标、主题修订或终端输出颜色。

同一流还携带完整的附件作用域调整大小租约所有权变化。渲染器在动态启用或禁用 Host 调整大小请求前，会校验附件与终端标识；所有权变化时，不会改写不可变的附件 bootstrap。

实时输入权限使用独立的 `attachment-input-capability-changed` 事件，其中包含 Host 已提交的完整附件能力状态与正数修订。传输层不能请求或合成该变更。客户端校验附件与终端标识，并且只应用精确重放，或精确下一修订且输入值已变化的事件；过时交付是惰性（inert）空操作；冲突或 revision 缺口按失败关闭（fail closed）处理。降级会同步禁用 PTY Action，而历史、搜索、选择与复制仍保持本地输出交互。升级不携带排队输入，只授予带新修订标记的未来输入。

功能故障在应用传输中保留其语义操作和可重试性。 Host 响应还保留其确切的相关性 ID，而客户端拥有的复制和链接效果保留其请求序列。这些值确定后果和诊断；他们从不授权自动重放输入或其他用户效果。

本地文件系统效果是一项可选的附件能力，默认不存在。启用时，`link-open` 携带有判别字段的 URL 或文件目标，而不是授予通用 opener。URL 目标继续遵循共享 scheme 策略；文件目标只对拥有该附件的本地 Desktop broker 有意义。Browser、archive 与来源中立的附件不会因为终端文本而获得本地应用启动或文件系统读取权限。

图片预览是独立且绑定请求的流程。具备能力的 renderer 可以请求一个检测到的本地图片路径，并取消精确的请求序列。Desktop 只会返回精确有界的 `ArrayBuffer` 与一种已准入 raster media type，或不泄露文件细节的 unavailable 结果。协议把预览内容限制在 8 MiB，且不传输文件句柄、目录列表、cache key 或持久化指令。请求标识让 renderer 能在 hover replacement 或附件退役后拒绝迟到内容；它不授权重试或远程文件传输。

附件流关闭后，Projection release 仍保持相关联。客户端会保留请求标识，直到 Main 返回 `terminal-projection-released` 或确切的类型化清理失败。成功确认不会传输任何端口；因此，迟到的清理结果只会结算其已释放请求，不会被误认为会使无关投影退役的未知消息。
由于命令交付与异步 open 完成是独立调度的，相关联的 `terminal-projection-opened` 可能在客户端请求 release 后才到达。客户端只关闭被转移的 attachment 端口，保留已释放请求标识直到清理结算，并让共享控制传输与无关请求继续可用。缺失或多出的转移端口、冲突标识或重复 open 仍属于无效控制流量，并会退役受影响的控制传输。

它不包含 Electron、DOM、UI、代理、协调器、传输生命周期、网络成帧、身份验证、PTY 或 VT 实现。当前 shared-document 与 legacy TerminalPresentation 语义在已接受 migration 前仍属于 `@orchbee/workbench`；增量 version-1 projection 语义来自语言中立 schema 权威。Terminal Host 线协议语义属于 [`schemas/terminal`](../../schemas/terminal/README.zh-CN.md)；`@orchbee/terminal-protocol` 是 TypeScript 绑定。

应用程序在第一个可信边界处使用这些模式解析每个不可信的 IPC 或网络值。传输所有者仍然负责成帧、身份验证、二进制传输、背压和连接退役。

每个客户端会话引导程序都带有 `terminalHostName`，由拥有投影终端计算机的应用程序运行时提供。共享的 Web 客户端使用该可信值来验证 OSC 7 当前目录元数据；它永远不会替换 Browser URL 主机名、传输对等名称或渲染器输入。 Projected 帧携带完整的可为空的自然标题和当前目录观察。协议在不选择 Tab 标题优先级、持久性或 URI 信任策略的情况下传输这些值； `@orchbee/workbench` 拥有这些决定。

Projected 帧和行窗口事件接受精确的 `ArrayBuffer` 或有界的 `Uint8Array` 视图，因此进程内适配器可以保留经过验证的有效负载视图。帧的可见载荷受 `maxFrameBytes` 限制，行窗口的可见载荷受 `maxRowWindowBytes` 限制；视图无关的后备字节不计入该限制。在将视图结构化克隆到不太受信任的功能之前，传输所有者必须将其可见字节复制到精确大小的 `ArrayBuffer` 中；结构化克隆携带完整的后备缓冲区，包括视图之外的字节。接收方仍然遵守声明的字节偏移量和长度，而传输信任和终端协议验证仍然是独立的责任。

选择导出仍然是附件范围且仅限纯文本。投影的命令没有格式选择器。 Main提交剪贴板效果后，Desktop效果附件接收`selection-copied`；客户端效果附件接收带有有限 UTF-8 字节的 `selection-copy-content` 并在报告本地完成之前通过其自己的平台剪贴板提交这些字节。这两个事件均不授予 HTML、原始 VT 或通用剪贴板功能。

Host 发起的剪贴板写入提交是一个单独的、请求绑定的流程。 Main 对每个附件最多转发一个待处理的有界请求，渲染器仅自动返回一个精确的 `clipboard-write-decision`，其中包含原始附件、序列和文本以及提交结果。 Desktop Main 拒绝不匹配和重复的决策，并仅针对积极的 Desktop 效应决策执行特权写入。客户端效果附件首先执行其本地 Browser 写入，并使用相同的精确决策仅消耗 Main 的挂起请求，因此 Main 无法复制该效果。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```

## Worktree 操作绑定

`workbench-worktree-contract.ts` 实现[保留 Worktree 线契约](../../schemas/workbench/v1/workbench-worktree.schema.json)。请求／响应联合纳入 Start/Read/Resume 及保留的部分结果，并拥有独立 96 KiB 编码预算。严格对象、UTF-8 引用限制、尝试上限、检出／路径相等、Git 状态／诊断约束、复用／启动结果一致性均与独立 Rust 绑定对齐。双方消费[共享语料](../../schemas/workbench/v1/workbench-worktree-conformance-v1.json)。共享叶模块 `workbench-terminal-launch-contract.ts` 拥有终端尺寸与启动结果；command 模块保持既有导出且无循环依赖。可选 Worktree shell 恢复回执保留 total/attempted/started/failed/unknown 计数（0–4096）和独立的可空恢复失败。计数描述串行启动前缀；最终回执不能有未完成启动，未知启动禁止 Resume。精确绑定及 Create 结果仍由既有查询所有者负责。Close 发布还接纳不可恢复的 `workspace-retained`，表示已静止的 Worktree Close 取消了规范删除。该绑定不增加 Desktop 运行时命令。

Start 和 Available 回执要求必填可空 `projectId`，保留原始请求目标。只有 Directory 来源允许 Start 的目标非空；Workspace 来源和 Remove 必须为 null，并在权威提交时解析分组。这些字段不授权自动重放或重新解释未知交付。

## Worktree 来源查询绑定

`workbench-worktree-query-contract.ts` 将独立读取请求／结果加入共享联合，与 Rust 采用相同的 32 KiB 结果上限、严格可空字段、闭合失败及跨条目不变量。它结合 [JSON Schema](../../schemas/workbench/v1/workbench-worktree-query.schema.json) 消费[查询一致性语料](../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json)。该绑定不增加 Desktop 命令或变更权限。

旧 `workbench-worktree-list-contract.ts` 将 `workbench.worktree-list.read` 及其 `found` 结果仅绑定到
全局 Directory 身份，不带 Project 作用域。它保留 UTF-8／原始字节 Git 条目的互斥形状与独立
32 KiB 结果上限。命令 schema 和共享命令语料覆盖该方向。Worktree 来源查询关联要求可空 Project 成员关系。

## Workspace 组关闭绑定

`workbench-workspace-group-close-contract.ts` 实现[组合同](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json)，扩展请求/结果联合，复用现有子 Close 状态/代次 schema。绑定约束 16 KiB 请求、1 MiB 结果、单个 Project 内最多 256 个唯一捕获 Workspace、1–64 次尝试及已关闭前缀顺序、身份观测唯一性和真实的聚合关闭/恢复。Start 仅指定 Project 与操作；结果不含源 Workspace 身份，成员不含重复 Project 身份。成员在准入时冻结，独立于 Git 仓库。空组的 Pending／Closed 合法；Closed 要求所有子项已关闭。必须存在、可为 null 的子操作观测区分尚未提交与已接纳但未观测。[共享样例](../../schemas/workbench/v1/workbench-workspace-group-close-conformance-v1.json) 同时检查独立 Rust 绑定和结构 schema。运行时接纳与单调发布仍由 Server 负责；此绑定不增加 Desktop CLI 动词。

## Client 视图绑定

`workbench-client-presentation-contract.ts` 验证标量精确当前成员快照：必须存在但可为 null
的选择；已绑定成员携带精确 Presentation／绑定、uint64 附着代次、uint32 窗格实例编号及已安装视口，
限制为 4 KiB。Client 视图绑定验证意图、Panel、Ready/Busy 视图与最低修订；共享语料
覆盖 null、子项、边界和不匹配回执。已有 Client 选择及主终端视口形状保持封闭。

`workbench-client-viewport-contract.ts` 校验有序的 1–16 个唯一 Panel 视口观察、十进制 uint64
位置、来源身份/几何及偏移公式。未知、待处理和实时/历史状态保持独立；接收回执前检查主绑定、
请求成员及 Ready/Busy 视图约束。快照在现有 Client 控制帧内限制为 12 KiB。
共享语料覆盖两种绑定及 JSON Schema，不引入 Desktop 运行时。

`workbench-client-view-contract.ts` 使用严格 Zod 结构和语义检查独立绑定 [Client 视图契约](../../schemas/workbench/README.zh-CN.md#易失-client-视图控制)。Rust 与 JSON Schema 证据也消费[共享语料](../../schemas/workbench/v1/workbench-client-view-conformance-v1.json)。该绑定不将 Client 本地选择放入规范 Workbench 快照，也不暴露 Desktop CLI 命令。

`workbench-client-layout-contract.ts` 校验有界单元格矩形、可为 null 的可见性／焦点、唯一 split／slot／Panel 身份、有限比例、物理包含关系及独立重新计算的方向邻居。Client 视图校验把布局读取绑定到精确目标及最低 revision；选中观测还必须与已发布 Client 视图匹配。共享语料覆盖有效的非活动／隐藏目标，以及被拒绝的跨字段、边界和引用违规。

方向控制新增 `focus-direction` 意图、`direction-focused` 和 `direction-unchanged` 回执，沿用普通 25/26 帧。成功聚焦校验区别于来源的精确目标和已观测视图；无邻居回执校验来源布局版本，支持当前选择位于其他上下文。共享语料覆盖闭合形状、版本、目标、意图不匹配和无效方向。

无邻居的 `direction-unchanged` 可以保留 Busy 视图，因为它不关闭当前模式或导航。
Starting 仍无效，源版本栅栏继续适用；成功的 `direction-focused` 仍要求视图已稳定。
Panel 和方向聚焦准入把可取消模式的校验交给原生 Client，在产生退出效果前执行。

分割调整在 25/26 帧新增严格的 `resize`、`set-split-ratio` 意图及 `layout-adjusted` 回执。数量为有限的 0–0.5，比例为有限的 0.1–0.9，二进制分割路径最多 16 个索引。绑定独立校验目标／版本／视图关系、互不相同的有序 slot、精确 setter 数值或夹取后的有符号 resize 增量，以及仅 resize 可用的 no-split 结果。普通 Applied 不能确认这两种意图。共享语料包含 64 个有效、115 个无效文档；结构 JSON Schema 检查补充独立语义校验器。运行时生命周期、绘制顺序和非活动保存归 Windows 布局所有者管理。

## 窗格自定义标题

[窗格标题契约](../../apps/orchbee/docs/native-panel-labels.zh-CN.md) 要求必填可空的规范 `Presentation.customLabel` 及携带精确 context/Tab/Panel ID 的 `panel.rename`。非 null 自定义标题必须非空、已裁剪且最多 512 个 Unicode 码点；没有覆盖项必须显式为 null。省略字段和纯空白回退标签均被拒绝。更新的 `label` 可省略或为 null，先限制原文长度，再按 Unicode White_Space 裁剪，空白输入规范化为 null。Rust 与 TypeScript 独立实现该规则（NEL 是空白，BOM 不是）。创建时的 `label` 及所有身份和绑定保持不变。此扩展属于统一升级的实验性 OBWP v1 cohort，旧的严格读取器可能拒绝新字段或动作。共享命令一致性语料覆盖规范设置、清除及规范化输入。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。Windows CLI 通过原 Client 单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](../../apps/orchbee/docs/native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

`set-split-ratio` 接受可选 `expectedSplit:{splitId,firstSlotId,secondSlotId}`。提供时三者均必填，两侧槽位必须不同；显式 null 非法。Client 在写入前比较精确身份，`layout-adjusted` 回执也必须匹配三者。省略时保留原路径设置行为。共享语料覆盖替换身份、缺字段及槽位重复；该附加字段要求匹配 cohort，未增加 Desktop CLI 命令。

[Panel 输入契约](../../apps/orchbee/docs/native-panel-input.zh-CN.md) 新增归 Panel 所有的可选 `rightClick`（默认 `orchbee`，或 `panel`）、`panel.input.set` 的必填策略、`panel.split` 的可选策略及 `panel-input-configured` 回执。null 与未知值无效。纯修改保持成员关系；分屏只为新 Panel 设置策略。Rust、TypeScript 和 JSON Schema 消费同一组一致性样例。不增加 Desktop CLI 命令，要求匹配的开发 cohort。

## Tab 插入边界契约

`tab.move-index { terminalContextId, tabId, insertIndex }` 扩展共享规范 Tab 重排；原 `tab.move`
方向契约不变。`insertIndex` 是 0..256 内的整数，在原始顺序中表示零基插入边界。Server 在精确
上下文/目标和树成员组合验证后拒绝超过当前 Tab 数量的值，原因是
`tab-move-index-out-of-bounds`。源边界及其后一边界均无变化；总数量边界表示末尾。
方向和索引移动均保留固定／分组元数据。精确位置若破坏固定前缀或打散连续分组，则返回保留的
`tab-move-order-conflict`，不修改状态、退役或隐式重新分组；合法位置保留完整 Tab 记录。
复用 `tab-moved` 和 `unchanged`、同一 revision、保留结果及原子提交 owner，不引入终端或焦点效果。
请求指纹包含 action 类型和插入边界。语料覆盖上下界、整数小数表示、缺失/多余/冲突字段、
负数、分数、字符串、布尔和空值拒绝。匹配实验批次一同发布和回退；不支持混合旧严格解码器，
无持久化迁移。可执行证据见拥有该行为的 Server/原生 CLI 测试，TLA 模型本次不执行。

## 自动 Tab 命名

[Tab 标签契约](../../apps/orchbee/docs/native-tab-labels.zh-CN.md) 拥有可选 `automaticLabel` 元数据。省略标签的新建 Tab 按当前上下文位置编号；显式数字名称及旧 Tab 保留固定名称。移动或关闭后 CLI 查询、标签选择器和 TUI 显示保持一致，重命名切换为固定名称。原生 `api snapshot` v4 暴露规范元数据；目录 v34 包含 68 份 schema。共享协议需要匹配 cohort。

Client 提示状态快照复用 `@orchbee/agent` 会话身份校验。严格形状与语义保留可空焦点/缺失状态、有序唯一批次、有界版本及精确意图对应。这些线 DTO 描述 Client 观察，不把确认转移到规范 Workbench 或 Agent 状态。

物理 `terminal-agent-attention` 读取指定 1–100 个唯一完整展示／创建／Host 绑定；
`terminal-attention-read` 保留有序的可空序号／seen 条目、报告修订及可空外层焦点。
它们复用类型 29／64 KiB 的提交／完成封套及读取生命周期。意图／结果关联要求精确绑定
和顺序，不合成规范 Session。[物理确认所有权](../../apps/orchbee/docs/native-client-attention.zh-CN.md#物理终端观察与绘制)
定义 Server／Client 合并及绘制门槛。

## Workspace 元数据契约

`workbench-workspace-metadata-contract.ts` 为共享请求／结果联合增加严格的上报／读取信封，请求上限 16 KiB、结果上限 128 KiB。[共享语料](../../schemas/workbench/v1/workbench-workspace-metadata-conformance-v1.json) 和 [JSON Schema](../../schemas/workbench/v1/workbench-workspace-metadata.schema.json) 覆盖 token 规范化、来源／序号／TTL 边界、读取批次及封闭失败类型。Rust 还拒绝原始 JSON 重复键；TypeScript 对象绑定保留合法的 `__proto__` 和 `constructor` token 键。此绑定不暴露 Desktop 命令或事件订阅。运行时归属由 [Workspace 元数据模型](../../apps/orchbee/docs/windows-workspace-metadata.zh-CN.md) 管理。

Panel 元数据新增严格的请求／结果联合成员，请求 16 KiB、结果 512 KiB。[共享用例](../../schemas/workbench/v1/workbench-panel-metadata-conformance-v1.json) 和 [schema](../../schemas/workbench/v1/workbench-panel-metadata.schema.json) 覆盖规范上报及有界来源记录。绑定还检查行／来源唯一性及逐字段写入顺序边界／归属。运行语义见 [Panel 元数据契约](../../apps/orchbee/docs/windows-panel-metadata.zh-CN.md)。

Panel Agent 上报新增 `workbench.panel-agent`／`workbench.panel-agent.result`，请求上限为
16 KiB，结果上限为 512 KiB。[共享语料](../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json)
和 [schema](../../schemas/workbench/v1/workbench-panel-agent.schema.json) 覆盖四种显式上报、四种
上报状态、完整 u64 十进制序号、有序读取及封闭回执。Rust／TypeScript 还校验 UTF-8 引用
字节上限、来源／引用资格和视图一致性。[Server 上报模型](../../apps/orchbee-server/docs/agent-report-model.zh-CN.md)
拥有物理代际、序号、抑制和原子元数据退出清理。此解码绑定不新增 Desktop 命令或进程生产者，
也不能提交进程存在声明。

物理 `observe` 读取复用 Panel Agent 承载和读权限，要求精确展示／Host 绑定，以及必填但可为
null 的 `afterSequence`；`observed` 返回当前视图、终端局部序号和最多 128 条连续语义变化。
Rust／TypeScript 校验最后变化与当前头部／状态一致，Rust 关联还证明请求游标覆盖。
这些是瞬态报告状态，与规范 Agent 修订及 Client `done` 分离。

物理 `explain` 使用相同的精确绑定和读取权限。`explained` 结果分别保留当前上报视图与原始
屏幕规则判断，包含必填的物理样本，以及必填但可为空的清单、胜出规则和发布跳过/回退原因。Rust/TypeScript
校验最高优先级首条胜出规则、状态/标记一致性与证据上限。
未知 provider 拒绝；没有匹配状态证据的已知 provider 明确回退为未知。沿用 512 KiB 结果上限。
这份未发布 cohort 一起变更；规范 OBAP 解释保持独立。

生效规则来源为物理 Explain 增加可选 `manifest` 对象：来源（`bundled`、`local-override`、
`remote-cache`）、必填且可空的实际路径／缓存版本、遮蔽情况及有界类型化告警。
省略信息保留旧形状，显式 null 拒绝。两个 binding 都校验远程来源／版本及告警来源唯一性。
CLI 携带来源信息时输出 v3，并保留封闭的 v1/v2 封套。

精简物理 `states` 读取接受 0–64 个唯一展示 ID，按请求顺序返回条目、Workbench／报告修订，
以及必填可空的完整绑定／序号／Agent／状态快照。成员不存在则拒绝整批；没有当前物理观察
则返回 null。空读取用于报告修订门槛。Rust／TypeScript 共用严格形状、唯一性／顺序检查和
一致性语料；Server 继续负责当前 Worker 及物理身份准入。

`agent-focus` ClientView 意图保留精确 context／Tab／Panel／Presentation，以及规范 Session
或完整物理 Agent 目标。绑定验证非零物理身份及展示对应；应用回执要求精确 Ready Client
选择。它复用既有导航帧及保留操作生命周期。[Agent 导航](../../apps/orchbee/docs/native-agent-navigation.zh-CN.md)
负责显式确认及身份丢失后的拒绝。

## 已安装的 Agent 规则操作

`workbench.agent-manifests` / `.result` 使用 OBWP 37/38，结果上限 512 KiB。严格的 Rust、
TypeScript 和 JSON 契约覆盖已安装快照、可选维护信息、保留的 Reload／Update 操作，
以及分开的缓存报告与启用事实。当前快照及状态查询只读。修改使用明确的 Site 生命周期操作 ID；
相同类型复用 ID 读取保留回执，跨类型复用拒绝。更新报告保留逐 provider 的尝试版本、缓存版本
和失败原因，以及目录源、时间和持久化事实；效果未知绝不声称成功。
共享 `workbench-agent-manifests-conformance-v1.json` 语料覆盖接纳／拒绝的形状、可空字段、
数字版本、来源、修订、重复项和跨字段一致性。语义字节／版本和跨字段校验补充 JSON Schema。
Server 屏幕模型拥有来源选择、下载、安装、取消和回执上限。

可选 `maintenance.automatic` 暴露配置开关及来源、后台／发布版／可用性门控、实际启用状态和最近一次自动操作。必需可空字段 `lastOperation` 保留 pending、updated、failed 或 unknown 事实，拒绝仅用于重载的 Applied。Rust／TypeScript 校验门控一致性与默认来源。自动任务不消耗手动回执 ID；调度由 Server 拥有。

两个 Panel 载体还支持 `read-presentations` 和 `presentations-read`，用于精确主项或子项报告／元数据观察：四个有序唯一 Presentation ID、空读取修订校验、必需的所属 `panel` 行，以及不同成员共享 Panel ID。Rust／TypeScript 与主项读取同样校验载荷不变量及元数据字段顺序归属。既有主项读写形状不变。共享用例覆盖精确成员数量界限、严格包装、同 Panel 成员和未绑定载荷；原生调用者还关联 Presentation、Panel 和 binding。协议应与其 Server 及原生 TUI 消费者作为匹配 cohort 交付。

两类 Panel 载体另接纳封闭的 `report-presentation` / `presentation-reported` 变体，必需
携带 Panel、Presentation 及期望／实际绑定身份。旧 `report` / `reported` 保持封闭且只
针对主项。共享 Rust／TypeScript／JSON 向量覆盖精确请求／回复、必需 ID 及旧形状拒绝；
原生关联还要求精确请求成员。[Agent 报告目标契约](../../apps/orchbee/docs/windows-panel-agent-reports.zh-CN.md#精确成员目标)
与[元数据目标契约](../../apps/orchbee/docs/windows-panel-metadata.zh-CN.md#精确成员目标)拥有
运行时语义、提交回执及配套 cohort 兼容规则。

精确 Presentation 选择也接受严格的未绑定形状 `{presentationId, selectionGeneration}`，
代次为非零十进制 uint64。它不携带挂接、绑定或 viewport，且与 null 本地选择不同。
已绑定选择保留原有形状。Rust、TypeScript 和 JSON Schema 共享缺失、混合、多余及无效代次
字段的正反一致性向量。原生 current-Panel 所有者验证规范绑定缺席，并用既有选择代次围住
关联读取；读取不分配代次或 viewer。此私有扩展与匹配的原生 cohort 一起发布。

## 原生插件运行时契约

导出的插件请求/结果 schema 校验同批 cohort 的 OBWP 45/46、封闭上下文/日志结构、UTF-8 字节上限、进程结果和 Worker/序号游标一致性。共享插件一致性用例同时对照 Rust 和 JSON Schema。[原生插件行为](../../apps/orchbee/docs/windows-plugins.zh-CN.md) 管理执行权限、输出上限和剩余集成。

启动选项中的插件 Pane 选择器是封闭、有界的目录标识，仅由 Tab／分屏创建受理。规范化保留非 null 选择器，移除 null／缺省值。共享命令语料覆盖标识边界、未知执行字段及程序／profile 冲突，不代表已支持所有插件布局。

插件契约还承载只读 Pane 成员查询及精确修订号／绑定响应。共享语料拒绝零修订号、无绑定成员与带插件前缀的本地入口名称。

## Workspace Git 状态读取

`workbench-workspace-git-contract.ts` 加入主请求／响应联合，保留封闭身份／修订／nullable／失败语义，
使用独立 8 KiB 帧、1024 字节分支文字及 uint32 ahead/behind 计数。
它与独立 Rust 绑定共同消费[共享语料](../../schemas/workbench/v1/workbench-workspace-git-conformance-v1.json)
及 [Schema](../../schemas/workbench/v1/workbench-workspace-git.schema.json)；
语义字节／跨字段检查补充 JSON Schema。
配置快照在 Agent profile 之外必需携带独立 32 KiB `spacesSidebarProfile`，编码预算为 256 KiB。
运行时安装由 Client 拥有并保持原子性。不增加 Desktop 命令或变更权限。

## 插件覆盖层契约

封闭插件联合还验证精确只读 `pane-observe`／`pane-observed` 成员和布尔终止事实。Client `overlay` intent 固定新建与返回成员身份、预期 binding、必填可空返回 binding 及先前 zoom。返回自身或 Applied 缺少 zoom／多个叶节点会在语义验证失败。共享 Rust／JSON Schema 语料覆盖这些新增形状；运行时所有权仍由 [Windows 原生 Client 与 Server](../../apps/orchbee/docs/windows-plugins.zh-CN.md#覆盖层生命周期模型)负责，不增加 Desktop 布局或变更权限。使用匹配的原生 cohort。

Pane 启动选项增加可选、有界的 `pluginContext`，仅允许与 Tab／分屏创建的 `pluginPane` 同时使用。共享命令语料覆盖 null 归一化、未知字段、UTF-8／整体上下文边界和不支持的创建类型。上下文是展示数据：Server 校验来源成员、提供规范标签，并在冻结受保护插件环境前固定调用来源与关联 ID。它不选择可执行程序、启动目录或 Worker 权限。省略字段保留旧准入。Rust 的启动 Debug 隐藏上下文；TypeScript 共享上下文 schema，避免启动／插件循环导入并保留已有插件导出。运行语义与验收范围归 [Pane 启动来源上下文](../../apps/orchbee/docs/windows-plugins.zh-CN.md#pane-启动来源上下文) 所有。

## 私有 scrollback 编辑器 popup 契约

Rust 与 TypeScript 共用闭合 `workbench.terminal-editor` Open／Read／Close 请求
和动作关联结果。规范已绑定 Client 来源及同连接 viewer／内容版本是选择输入，
不接纳可执行程序、文件路径或终端内容。规范十进制完整 u64 字段、非零身份、
可绘制尺寸、严格字段及整条消息8KiB限制一致。
[Rust 契约](../../crates/workbench-protocol/README.zh-CN.md#私有-scrollback-编辑器-popup-契约)
及 [Server 模型](../../apps/orchbee-server/README.zh-CN.md#windows-scrollback-编辑器-popup-模型)
拥有授权与物理生命周期；这是私有原生入口，不是新 Desktop 操作或已完成公开
CLI 能力。

匹配 Client 视图契约也接受私有 `editor-open`，只携带已绑定的
`ScrollbackEditorSpec.source`。明确类型的插件／编辑器 actor 共用准确来源的
`popup-opened` 完成和通用 `popup-close`。
[Client popup 模型](../../apps/orchbee/docs/windows-client-control.zh-CN.md#scrollback-编辑器-popup-模型)
负责同连接只读捕获、完整内容区几何、输入、取消和清理。公开编辑器操作回执使用控制 v10；
v1–v9 拒绝此意图。公开 Windows `terminal edit-scrollback` 路径由
[CLI 工作流程](../../apps/orchbee/docs/cli-workflows.zh-CN.md#编辑当前聚焦终端的滚动记录)
定义。定向管道／actor 证据不等于最终 GUI／cohort 验收。

## Windows配置命令契约

configured-command请求／结果绑定与Client的 `command-popup-open` 意图共享严格popup规格。原生已安装keys profile拥有不透明ID，解码值从不携带可执行命令文本。四种动作分别保留Worker启动、规范Pane创建、plugin调用及Client popup控制回执。可选plugin选区固定两个行列端点和准确终端内容版本，要求已绑定当前来源，最多捕获4096行及8192 UTF-8字节。请求／结果上限256 KiB。Client control v11接受配置popup意图，较旧原生输出版本保持封闭。Server解析、临时观察器回收、物理作业及TUI执行见 [Windows自定义命令](../../apps/orchbee/docs/windows-custom-commands.zh-CN.md)。

## Panel 图片绑定

pane graphics 绑定与 Rust 共用封闭的图片、图层、像素块和生产者协议。它检查编码及解码字节上限、规范 base64、资源排序、损伤矩形和完整宽度序号字符串。元数据目录不包含像素。渲染能力和可见性字段来自真实 Client 观察；空字段不表示已有渲染支持。共享图形案例检查 TypeScript、Rust 和 JSON Schema 的边界。

语义与命令边界独立校验规范分割比例及有身份的 `layout.set-split-ratio` CAS。原生[共享比例行为](../../apps/orchbee-server/docs/shared-split-ratios.zh-CN.md) 负责提交、持久化及绘制确认；Client 几何仍按各自视口计算。

## 原生设置绑定

`workbench-settings-contract.ts` 把闭合 Open/Save 请求和结果 Schema 加入共享命令联合类型，复用完整 `ConfigurationSnapshotSchema`，校验规范内容／缺失令牌、必需但可空的脏字段与非空补丁，并提供独立的 4 KiB／256 KiB 编码限制。严格 Zod、Rust 与 JSON Schema 使用[共享语料](../../schemas/workbench/v1/workbench-settings-conformance-v1.json)。此绑定不授予 Desktop 命令、文件写权限、传输重试或在线应用能力；这些效果仍由[原生设置所有者](../../apps/orchbee/docs/native-settings.zh-CN.md) 负责。

## 保留操作身份

封闭的 `CloseOperationReadSchema` 与 `CloseOperationResultSchema` 加入请求／结果联合及命令一致性语料。它们对应[保留 Close 身份查询](../../crates/workbench-protocol/README.zh-CN.md#保留-close-身份查询)，不引入 Client 所有的生命周期状态。
