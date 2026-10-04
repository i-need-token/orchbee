# Workbench 协议

## 语义事件契约

封闭 `workbench.events` 请求／结果 kind 55／56 各自上限 256 KiB。Subscribe、read、有界 wait
与 close 携带准确的连接持有订阅身份。日志 cursor 固定启动 epoch 与全宽十进制序号；
探测位置使用独立订阅序号。过滤条件、事件／数据 kind、来源版本、有序分页及明确 gap
在解释前验证。`matches_semantic_filter` 为运行时发布者与原生 CLI 回执检查共享纯语义匹配。
[Windows 事件模型](../../apps/orchbee-server/docs/windows-events.zh-CN.md) 管理真实发布与观察副作用。
可选 Client `splitRevision` 是投影缓存的完整宽度规范 Workbench 修订；旧观察省略该字段不能证明比例变化。

本 crate 管理原生 Workbench 客户端与 Server 共用的请求、响应、校验和帧传输契约。它不打开套接字或执行操作；运行时所有者负责处理已受理请求。

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
