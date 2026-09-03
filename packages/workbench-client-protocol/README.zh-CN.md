# @orchbee/workbench-client-protocol

适用于一个 Orchbee Workbench 页面客户端的严格、运行时中立的数据协定。

## 适用范围

该包拥有共享 Workbench 客户端会话、终端投影控制以及为一个可见终端视口打开的投影附件流的经过验证的消息。 Electron、普通 Browser 页面和未来的移动客户端可以通过不同的传输方式携带这些消息，而不改变其 Workbench 客户端含义。

该 package 还提供增量 version-1 只读 Workbench projection kernel 的独立 TypeScript binding。
语言中立权威与 canonical conformance corpus 位于
[`schemas/workbench`](../../schemas/workbench/README.zh-CN.md)。Binding 解析一个严格的
`workbench.snapshot.read` intent、完整有界 snapshot 与 revision-chained aggregate delta；
`applyWorkbenchDelta` 原子应用 delta，或返回一个 typed recovery reason。该 contract 包含
Project、Workspace、Tab、Panel 与 Terminal Presentation 产品语义，但排除 Environment、
Directory、Worktree、Agent、Launch Profile、terminal content、persistence、transport 与所有
Client-local view fact。当前 client-session consumer 尚未使用这份新 contract。

每个`terminal-projection-open-requested`命令携带一个完整的正值`TerminalGeometry`。它是调用者测量的视觉表面的安装几何形状，而不是可选的调整大小提示或后备。因此，请求身份包括表示身份和几何结构：可以重放精确的副本，而不同几何结构的重用会与控制流量发生冲突。传输合同不提供超时或 80x24 默认值。

投影的附件流携带语义铃声事件，而无需选择声音、视觉闪光或通知策略。每个事件都保留 Host 附件身份和终端拥有的序列，因此预计的终端可以抑制过时或重复的传递。

同一流还携带完整的附件作用域调整大小租约所有权变化。渲染器在动态启用或禁用 Host 调整大小请求前，会校验附件与终端标识；所有权变化时，不会改写不可变的附件 bootstrap。

实时输入权限使用独立的 `attachment-input-capability-changed` 事件，其中包含 Host 已提交的完整附件能力状态与正数修订。传输层不能请求或合成该变更。客户端校验附件与终端标识，并且只应用精确重放，或精确下一修订且输入值已变化的事件；过时交付为空操作，冲突或修订缺口则无法恢复。降级会同步禁用 PTY Action，而历史、搜索、选择与复制仍保持本地输出交互。升级不携带排队输入，只授予带新修订标记的未来输入。

功能故障在 Electron 和 Gateway 传输中保留其语义操作和可重试性。 Host 响应还保留其确切的相关性 ID，而客户端拥有的复制和链接效果保留其请求序列。这些值确定后果和诊断；他们从不授权自动重放输入或其他用户效果。

本地文件系统效果是一项可选的附件能力，默认不存在。启用时，`link-open` 携带有判别字段的 URL 或文件目标，而不是授予通用 opener。URL 目标继续遵循共享 scheme 策略；文件目标只对拥有该附件的本地 Desktop broker 有意义。Browser、Gateway、archive 与来源中立的附件不会因为终端文本而获得本地应用启动或文件系统读取权限。

图片预览是独立且绑定请求的流程。具备能力的 renderer 可以请求一个检测到的本地图片路径，并取消精确的请求序列。Desktop 只会返回精确有界的 `ArrayBuffer` 与一种已准入 raster media type，或不泄露文件细节的 unavailable 结果。协议把预览内容限制在 8 MiB，且不传输文件句柄、目录列表、cache key 或持久化指令。请求标识让 renderer 能在 hover replacement 或附件退役后拒绝迟到内容；它不授权重试或远程文件传输。

附件流关闭后，Projection release 仍保持相关联。客户端会保留请求标识，直到 Main 返回 `terminal-projection-released` 或确切的类型化清理失败。成功确认不会传输任何端口；因此，迟到的清理结果只会结算其已释放请求，不会被误认为会使无关投影退役的未知消息。
由于命令交付与异步 open 完成是独立调度的，相关联的 `terminal-projection-opened` 可能在客户端请求 release 后才到达。客户端只关闭被转移的 attachment 端口，保留已释放请求标识直到清理结算，并让共享控制传输与无关请求继续可用。缺失或多出的转移端口、冲突标识或重复 open 仍属于无效控制流量，并会退役受影响的控制传输。

它不包含 Electron、DOM、UI、代理、协调器、传输生命周期、网络成帧、Gateway、身份验证、PTY 或 VT 实现。当前 shared-document 与 legacy TerminalPresentation 语义在已接受 migration 前仍属于 `@orchbee/workbench`；增量 version-1 projection 语义来自语言中立 schema 权威。Terminal Host wire 语义仍属于 `@orchbee/terminal-protocol`。

应用程序在第一个可信边界处使用这些模式解析每个不可信的 IPC 或网络值。传输所有者仍然负责成帧、身份验证、二进制传输、背压和连接退役。

每个客户端会话引导程序都带有 `terminalHostName`，由拥有投影终端计算机的应用程序运行时提供。共享的 Web 客户端使用该可信值来验证 OSC 7 当前目录元数据；它永远不会替换 Browser URL 主机名、Gateway 对等名称或渲染器输入。 Projected 帧携带完整的可为空的自然标题和当前目录观察。协议在不选择 Tab 标题优先级、持久性或 URI 信任策略的情况下传输这些值； `@orchbee/workbench` 拥有这些决定。

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
