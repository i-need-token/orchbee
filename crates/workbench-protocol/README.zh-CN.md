# Workbench 协议

## 共享用户偏好

[通用偏好契约](docs/user-preferences.zh-CN.md) 拥有 GUI/TUI 清单、默认值、校验、
作用域、生效时机和能力边界。原生设置与匹配的 Desktop 辅助程序使用相同的
类型化记录和源检查补丁。JSON Schema 与跨语言语料位于 `schemas/workbench/v1`。

## Project 组织与稳定 Workspace 身份

Navigation 持有全局按身份排序的 `directories`、`projects` 和 `workspaces`，以及
独立的 `projectOrder`、`workspaceOrder`。每个非空顺序必须是其
注册表的完整排列；省略或空顺序使用规范 ID 顺序。Project 只包含 `projectId` 与 `label`。Workspace 的 `projectId` 和
`primaryDirectoryId` 必须存在且可为 null；其 ID、Terminal Context 与物理 Directory 独立于分组。
引用必须能解析，每个 Terminal Context 恰有一个 Workspace 所有者；零 Workspace／零 Context 状态合法。容量为 256 个 Project、65,536 个
全局 Directory、每个 Project 最多 256 个 Workspace、全局最多 1,024 个 Workspace。现有 1 MiB
投影字节预算是独立于集合容量的额外上限。

`workspace.create` 必须携带 `projectId`，允许 null。纯命令 `workspace.set-project` 改变一个已有
Workspace 的成员关系；`workspace.create-project` 原子创建 Project 并将该 Workspace 加入其中；
`project.dissolve` 仅移除组，使其 Workspace 成为未分组状态。结果使用带必填可空 `projectId` 的
`workspace-project-changed` 或 `project-dissolved`。重新分组保留 Workspace ID、目录、终端身份
及显示顺序。权威成员准入、revision CAS 与原子发布仍由 Server 负责。

Workspace 创建、启动和重命名命令要求名称至少包含一个非空白 Unicode 标量，
有效文本在既有 512 标量上限内精确保留。Rust、TypeScript 和 JSON Schema 共用此边界及命令
一致性案例；历史快照标签仍可解码，Client 可显示后备名称并重命名修复。此规则不改变
独立归属的 Project、Tab 或 Tab 分组命名规则。

新增 `workspace.close-if-idle {workspaceId}` 与 `project.dissolve-if-idle {projectId}` 动作
保留普通命令信封及既有接受回执，使用独立完整指纹，不接受调用方空闲标志。只有 Server 拥有
的完整范围准入才能接受；确定的 `rejected` 原因 `confirmation-required` 无效果，且不授权
自动重发。普通已确认动作不变。
[Server 条件契约](../../apps/orchbee-server/docs/terminal-tab-close.zh-CN.md#条件式空闲准入)
拥有待处理预留、精确从未创建证据、隔离栅栏及组织保留语义。

`navigation.configure` 携带封闭的 `NavigationConfiguration`：`move-project` 要求 Project ID 和
可空 `beforeProjectId`；`move-workspace` 要求 Workspace ID、可空 `projectId` 和可空
`beforeWorkspaceId`。null 锚点表示追加到目标分组末尾；非空 Workspace 锚点必须已属于该组。
`configure_navigation` 为权威规划与原生回执验证共享同一确定性转换。它保留按身份排序的注册表、
全部物理资源及其他分组的相对顺序，原子改变成员关系和显示顺序。身份缺失、锚点无效或分组已满
时拒绝整个候选。无变化返回 `unchanged`，不增加 revision；变化返回 `navigation-configured`。
原生手势契约由[侧栏布局](../../apps/orchbee/docs/tui-shell-layout.zh-CN.md)管理。

`workspace.launch` 的 `placement` 为 `ungrouped`、`project { projectId }`、`new-project { label }`
或 `alongside { workspaceId, projectLabel }`。Alongside 在权威提交时解析源的当前分组。未分组的
源会原子创建一个 Project 并同时纳入源与新启动的 Workspace；已有分组则复用该组。Launch 要求
绝对路径 `options.cwd`，禁止 Plugin Pane 选项及相冲突的 launch/profile 选择器，并使用 96 KiB
启动命令上限。待完成和已完成回执都必须携带可空 `projectId`。`panel.relocate` 的
`new-workspace` 目标同样要求必填可空 `projectId`。

Rename、Close 与主 Directory 接受通过稳定 Workspace 身份定位，不携带 Project。
Close 接受、读取、恢复及其结果同样不含 `projectId`；Close 读取结果的精确最大编码长度为
1,000,007 字节。Workspace 事件数据只包含 `kind` 和 `workspace`，其中必填可空成员关系是
唯一的 Project 字段。

Worktree Start 要求必填可空 `projectId`：只有 Directory 来源可提供非空目标。Workspace 来源及
Remove 必须提供 null；其分组在最终提交时由稳定 Workspace 解析。Available 回执保留原始请求的
可空目标作为关联元数据，不声明当前成员关系。精确请求相等、保留结果及未知交付规则保持不变。
Worktree 查询关联的 Project 成员关系可为 null。旧 `workbench.worktree-list.read` 与其 `found`
结果仅通过全局 Directory 定位，不带 Project 作用域；结果维持 32 KiB 上限。

Project 组 Close 的 Start 仅含 `operationId` 和 `projectId`。Available 结果含 `projectId`、
`attempt`、`status` 和冻结的 `members`；每个成员仅含 `workspaceId`、`label` 与必填可空 `close`。
准入捕获该 Project 内全部 Workspace，无论 Git 仓库或目录，最多 256 个成员。不存在源必须最后
的规则。仍约束唯一 Workspace／子请求身份、按序已关闭子项前缀与明确恢复事实。空 Project
允许 Pending 和空集成立的 Closed 结果。Closed 要求所有捕获子项都已观测为 Closed。结果帧预算
为 1 MiB，容纳 256 个最大转义标签；请求仍为 16 KiB。驱动器负责从准入推进到完成。

这是未发布 OBWP v1 的同批升级：Rust、TypeScript、schema 与共享语料共同启用及回滚。
协议测试覆盖精确编码上限、封闭形状、可空字段、全局引用／顺序／容量、启动放置及保留的子项事实。
Checkpoint 迁移和原生／前端效果由各运行时所有者负责。

## Tab 配置

`tabs.configure` 携带准确的 context 及一个封闭 `TabConfiguration`；`tabs-configured` 返回已提交修订，
除创建外 group ID 必须为 null。纯 `configure_tabs` 转换保留终端成员，并约束固定前缀与连续分组顺序。
权威准入、revision CAS 与发布仍归 Server。Rust 和 GUI `TabModel` 消费同一份
`tab-configuration-conformance-v1.json` 语料；wire 形状同时使用共享命令语料。
该扩展属于未发布的 OBWP v1 匹配 cohort；运行时与回执语义见
[原生分组契约](../../apps/orchbee/docs/native-tab-groups.zh-CN.md)。

`WorkbenchTabIdSet` 为 `create-group-for-tabs`、`set-tabs-group` 与 `set-tabs-pinned` 接入
1–256 个唯一 ID；缺失任一成员即拒绝整个候选。位置由规范顺序决定。`move-tab` 遵循 GUI
分组边界，并消费临时的相邻组折叠提示。它不引入共享选择或折叠状态。

`move-tabs-by-drag` 将精确所选集合原子放置于原顺序插入边界（0–256），
目的组字段必填但可为空。它保留固定分区、完整选中组、未移动组的连续性及全部终端树／绑定。
混合固定状态的选择、缺失目标及超出 context 的边界均被拒绝。

## 终端 Tab 放置

`TabCreationPlacement` 为 `tab.create` 增加可选 `in-group`、`new-group`、`after-tab` 和精确单 Tab `duplicate`，
省略时保持追加。`validate_tab_creation` 在效果前检查目标及清理后的容量。`after-tab` 解析精确目标
组及 index + 1，拒绝在固定前缀内部插入未固定 Tab，允许在其最后边界插入。`create_terminal_tab`
依据调用方持有的全新身份和 binding 构造完整候选，保留旧树、复制源元数据并创建单个新 Panel。
Server 接纳／发布和终端生命周期仍归既有 owner。原生精确回执验证共享此纯转换。GUI 与 Rust
消费 `tab-creation-conformance-v1.json`。`WorkbenchTabGroupLabel` 允许空组名（0–512 个
Unicode 标量值），包括 `CreateGroup`、`CreateGroupForTabs` 和 `UpdateGroup`；`WorkbenchLabel` 仍须非空。详见
[原生分组契约](../../apps/orchbee/docs/native-tab-groups.zh-CN.md#终端创建与复制)。

## 语义事件契约

封闭 `workbench.events` 请求／结果 kind 55／56 各自上限 256 KiB。Subscribe、read、有界 wait
与 close 携带准确的连接持有订阅身份。日志 cursor 固定启动 epoch 与全宽十进制序号；
探测位置使用独立订阅序号。过滤条件、事件／数据 kind、来源版本、有序分页及明确 gap
在解释前验证。`matches_semantic_filter` 为运行时发布者与原生 CLI 回执检查共享纯语义匹配。
[Windows 事件模型](../../apps/orchbee-server/docs/windows-events.zh-CN.md) 管理真实发布与观察副作用。
可选 Client `splitRevision` 是投影缓存的完整宽度规范 Workbench 修订；旧观察省略该字段不能证明比例变化。

本 crate 管理原生 Workbench 客户端与 Server 共用的请求、响应、校验和帧传输契约。它不打开套接字或执行操作；运行时所有者负责处理已受理请求。

`WorkbenchDirectoryPath` 保留已准入操作数的字面拼写。`same_windows_path_spelling`
在 Server 与 Client 间共享现有 Worktree 关联比较规则：分隔符、结尾分隔符和
ASCII 大小写可以不同。它不检查文件系统身份、不规范化路径，也不解析 verbatim
前缀或点路径成分。wire 语法及运行时执行所有权保持不变。

## 配置式命令调用

同批 cohort 使用封闭的 `workbench.configured-command` 请求／结果 kind 53/54，
上限为 256 KiB。已安装按键 profile 持有目录 ID 和全部四种动作。
调用携带预期规范修订号及可选精确 Client／来源围栏，不携带可执行程序。
回执区分后台作业受理、规范 Pane 创建、插件执行和保留的 Client popup 控制；
嵌套回执保留原有不确定性。

配置 popup 控制携带独立类型的选择器、来源、展示上下文和配置尺寸。
Open/read/close 由已认证 Client 连接持有，使用既有独立终端生命周期。
目录、选择、成员关系或来源过期时，在派发前拒绝。配置命令不冒充已安装插件。
[原生命令行为](../../apps/orchbee/docs/windows-custom-commands.zh-CN.md)
管理配置、派发和 Windows 生命周期。

## 原生插件运行时契约

封闭的插件调用/log/get/cancel 请求及结果使用同批 cohort 的 OBWP 45/46，分别限制为 32 KiB 和 8 MiB。上下文只包含展示数据；受理 Server 解析执行意图，Worker 拥有进程。日志游标固定 Worker 身份/generation 和递减序号；修改投递结果未知时显式返回。Rust、TypeScript 和 JSON 共用插件一致性语料。[原生插件行为](../../apps/orchbee/docs/windows-plugins.zh-CN.md) 管理运行时生命周期和剩余集成。

`tab.create` 和 `panel.split` 的启动选项可包含 `pluginPane: { pluginId, entrypointId }`。两者均为有界 ASCII 目录名称（120 字节，入口名不含点）。选择器与程序／profile 互斥，Workspace 或子终端创建拒绝使用。缺省和 null 可选选择器规范化为省略，旧请求序列化保持不变。Server 解析与受保护的执行环境归终端受理所有，不由此不含展示数据的选择器决定。

插件 PaneGet 为只读操作，PaneFound 返回非零规范修订号、封闭选择器、精确 context／Tab／Panel／Presentation 及绑定。响应关联必须匹配请求的 Panel；结果不是 action Job 回执或调用者展示上下文。

## Workspace Git 状态读取

`WorkspaceGitRequest`／`WorkspaceGitResult` 增加封闭的 `workbench.workspace-git`
与 `workbench.workspace-git.result` 方向，OBWP kind 49/50，分别在分配前限制为 8 KiB。
必需 nullable branch/divergence 区分缺失与零；分支为无控制字符、最多 1024 字节的 UTF-8，
计数为 uint32，游离分支不能携带计数。必须精确关联请求／Site／Workspace。
[Schema 与共享语料](../../schemas/workbench/README.zh-CN.md#workspace-git-状态读取)
覆盖 Rust 及 TypeScript。不授予变更或 Desktop 命令。
配置发布还携带独立必需的 32 KiB `spacesSidebarProfile`；匹配 cohort 配置帧预算为 256 KiB。

## 插件覆盖层观察与 Client 返回身份

只读 `PaneObserve` 携带精确 `PluginPaneInfo`；`PaneObserved` 回显该对象、非零规范 revision 及布尔终止事实。相关性包含 selector、context／Tab／Panel／Presentation 和 binding，不授予生命周期变更权限。私有 Client `Overlay` 控制携带新建已绑定成员、精确可空返回成员及先前 zoom。验证拒绝返回自身的身份；Applied 要求所请求焦点成员、zoom 和多个叶节点。必填可空与语义失败复用 Rust／TypeScript 语料。[Windows 插件](../../apps/orchbee/docs/windows-plugins.zh-CN.md#覆盖层生命周期模型)负责运行时记录、观察清理和规范 Close。这些扩展要求匹配原生 cohort，不新增 Desktop 命令。

Pane 启动选项增加可选、有界的 `pluginContext`，仅允许与 Tab／分屏创建的 `pluginPane` 同时使用。共享命令语料覆盖 null 归一化、未知字段、UTF-8／整体上下文边界和不支持的创建类型。上下文是展示数据：Server 校验来源成员、提供规范标签，并在冻结受保护插件环境前固定调用来源与关联 ID。它不选择可执行程序、启动目录或 Worker 权限。省略字段保留旧准入。Rust 的启动 Debug 隐藏上下文；TypeScript 共享上下文 schema，避免启动／插件循环导入并保留已有插件导出。运行语义与验收范围归 [Pane 启动来源上下文](../../apps/orchbee/docs/windows-plugins.zh-CN.md#pane-启动来源上下文) 所有。

## 独立插件 popup 契约

私有插件 PopupOpen／PopupRead／PopupClose 使用既有匹配 cohort 插件传输，返回明确关联的 PopupOpened／PopupState／PopupClosing／PopupClosed／PopupUnknown。PopupOpen 携带封闭目录选择器、精确 context／Tab／Panel 及完整 Client Presentation selection、有界显示上下文、必填可空 cwd／尺寸，复用受保护名称启动环境准入。已绑定 attachment generation／incarnation 和非零未绑定 selection generation 属于来源身份。这些请求不创建规范 Pane 成员。Client PopupOpen／PopupClose 要求明确匹配的完成结果，通用 Applied 不能完成它们。hadPopup=false 的关闭可保留其他 Busy 对话框。Rust、TypeScript 与 JSON 共享有效／无效语料。[独立 popup 行为](../../apps/orchbee/docs/windows-plugins.zh-CN.md#独立终端-popup)负责物理生命周期、UI 输入及验收。对端需成套部署；不授予 Desktop 操作或新增 Host 能力。

## 私有 scrollback 编辑器 popup 契约

`workbench.terminal-editor` 及结果使用配套版本 OBWP51／52，不改变旧编号或公开
CLI 定义。闭合 Open／Read／Close 消息携带 Site／request／popup 关联。Open
指定规范已绑定 Client popup 来源、同连接只读 viewer、十进制字符串正 generation
及完整 u64 revision，以及可绘制尺寸；不接纳可执行程序、环境、文档路径或终端
内容。结果区分 opened／state／closing／closed／unknown 及闭合失败，并核对
动作允许的结果和准确关联。8KiB 请求／结果限制按整条消息计算，严格核对未知
字段及非零身份。[Server popup 模型](../../apps/orchbee-server/README.zh-CN.md#windows-scrollback-编辑器-popup-模型)
拥有来源授权、延迟创建／启动和清理。私有契约不证明 Client 焦点或公开编辑器
完整对齐。

匹配 Client 视图契约也接受私有 `editor-open`，只携带已绑定的
`ScrollbackEditorSpec.source`。明确类型的插件／编辑器 actor 共用准确来源的
`popup-opened` 完成和通用 `popup-close`。
[Client popup 模型](../../apps/orchbee/docs/windows-client-control.zh-CN.md#scrollback-编辑器-popup-模型)
负责同连接只读捕获、完整内容区几何、输入、取消和清理。公开编辑器操作回执使用控制 v10；
v1–v9 拒绝此意图。公开 Windows `terminal edit-scrollback` 路径由
[CLI 工作流程](../../apps/orchbee/docs/cli-workflows.zh-CN.md#编辑当前聚焦终端的滚动记录)
定义。定向管道／actor 证据不等于最终 GUI／cohort 验收。

## Panel 图片协议

`PaneGraphicsRequest` 和 `PaneGraphicsResult` 使用 OBWP 57/58，并在分配前独立限制方向（请求 24 MiB、结果 512 KiB）。它们覆盖完整 PNG／RGB／RGBA／BGRA 上传、静态图层、元数据目录、按资源版本读取的像素块以及连接所属的生产者。在没有渲染或文件副作用的情况下验证结构、规范 base64、通道长度、损伤范围、完整宽度生产者序号及响应关联。运行时所有者提供 PNG 解码、来源约束、文件租约和真实 Client 能力。共享案例位于 `schemas/workbench/v1/workbench-pane-graphics-conformance-v1.json`。

规范 `PanelNode::Split.ratio` 默认为 0.5，准入有限的 0.1–0.9 数值。有身份的 CAS 命令 `layout.set-split-ratio` 返回 `split-ratio-configured`；[共享比例归属](../../apps/orchbee-server/docs/shared-split-ratios.zh-CN.md) 定义其运行时效果。

运行时 JSON 依赖显式启用 `float_roundtrip`，让数值比例经过传输及 checkpoint
后保持精确的 `f64` 身份。例如 0.54 减 0.05 得到 0.49000000000000005，若解码
成 0.49，会在调整成功后破坏精确的 CAS／发布比较。JSON 形状和准入范围不变。
`tests/split_ratios.rs` 覆盖全部 648 个方向及粗／细调整的数值往返。开发依赖
也可能隐式启用此特性，因此测试构建本身不能证明交付构建已覆盖；
[键盘调整模型](../../apps/orchbee/docs/native-keyboard-resize.zh-CN.md)还要求普通
Windows cohort 的整链见证。

## 原生设置契约

匹配的 Windows cohort 使用闭合 `workbench.settings` 消息 59/60，请求独立限制为 4 KiB，响应为 256 KiB。Open 观测完整配置快照和精确 SHA-256 内容／缺失令牌。Save 携带预期发布 revision 及三个必需但可空的脏字段；空补丁和操作中的额外字段被拒绝。关联结果区分无变化、已保存并应用、已保存但未应用和写入前拒绝。协议不传任意路径或 TOML，不重放结果不明的交换。Rust、TypeScript 与 JSON Schema 使用[同一一致性语料](../../schemas/workbench/v1/workbench-settings-conformance-v1.json)；[原生设置所有者](../../apps/orchbee/docs/native-settings.zh-CN.md) 负责效果规则。

## 保留 Close 身份查询

`CloseOperationRead` 与 `CloseOperationResult` 在既有 intent 大小上限内使用 OBWP 61/62。它们按原始请求 ID 读取保留 Close，精确关联 Site／request／operation，并使用封闭的 Workspace／Tab／Panel 目标。此读取不创建状态，并保留墓碑。Rust、TypeScript 和 JSON Schema 共享命令语料；[CLI 操作状态](../../apps/orchbee/docs/native-operation-status.zh-CN.md) 负责调用及恢复。

<!-- documentation-pairing: english-sha256=42cb7657b535efd03e44a452fb47679fee74f0a9cc713a2af4e6d532cbdff40c zh-cn-sha256=8571973bde57509e90ad8a01aee23f36840cdf266b8512bdf81f7fddc0b2a1e5 -->
