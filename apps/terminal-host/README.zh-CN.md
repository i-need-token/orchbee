# Terminal Host

Terminal Host 是 Orchbee 的本地权威终端执行服务。它拥有本机终端身份、PTY 与进程树、规范的 Ghostty 状态、保留的生命周期，以及一个受信任本地 Broker 的附件范围终端能力。

该应用程序是私有的并且是实验性版本。它安装一个可执行文件；它不导出可重用的本机库。

## 契约权威

- [运行时契约和正确性模型](docs/runtime-model.md)拥有主机生命周期、状态、交付、重试、失败、并发和证据要求。
- [终端协议参考](../../schemas/terminal/references/protocol.md) 拥有线路记录、编解码器、消息模式、格式版本和协议安全限制。
- [桌面版](../desktop/README.md)拥有产品拓扑、恢复策略、展示、平台效果。
- [Worker](../orchbee-worker/README.zh-CN.md) 拥有其尚未发布、Server-first 的本地 Broker 连接与对端存活耦合；它不拥有终端语义。

[原生运行时拓扑提案](../../proposals/platform/native-runtime-topology.zh-CN.md#desktop-main-目标处置) 将 Host 生命周期归 Worker，并取代旧 Desktop 更新连续性设计；目标提案不证明现行 Desktop 路径已经迁移。

运行时契约已经接受，但这不表示当前所有实现路径都已符合契约。只有受影响的契约声明具备所需可执行证据时，行为变更才算完成。

## 责任

Terminal Host 拥有：

- 每个 `TerminalId` 的一条保留的终端记录将一个语义会话绑定到一个延迟启动准备或一个已启动的本机引擎；
- PTY 和进程树创建、观察、终止、遏制和清理；
- 输入编码、投影、历史和选择使用的单一规范 Ghostty 状态；
- 有效的终端配置、完整修订的运行时主题、有序的有界输入、修订和生成；
- Broker 作用域的附件、需求、在途交付、响铃游标、历史锚点、选择和导出；
- 有界 OSC 52 纯文本写入意图、有界 private-APC application-signal transport、当前终端进度，以及源自 Ghostty 输出的最新语义命令完成事实；
- 受操作系统用户身份保护的私有本地 IPC 端点；和
- 共享终端协议的类型化本机适配。

它不拥有 Project、Workspace、Workbench、Tab、Panel、Agent、provider 或产品 session 模型，也绝不从 terminal signal 派生 Agent 语义。它还不拥有 Electron 生命周期、产品恢复选择、渲染、平台剪贴板内容、应用程序提交策略、剪贴板效果、搜索查询、外部导航、更新、网络侦听器、远程身份验证、加密或共享。

## 运行时承诺

接受的合同需要这些外部可见的属性：

- 一个端点至多有一个活动的物理本地 Broker 会话。
- Broker 断开连接会破坏该 Broker 的附件，但不会终止或处置保留的终端。
- 具有保留终端的主机仍可用于重新连接。没有 Broker 且没有保留终端的主机可以退出。
- 当 terminal 保留在一个 Host epoch 中时，terminal creation 按 creation identity 与 durable launch fingerprint 保持原子和幂等。单独的非持久化 environment overlay 只应用于该 creation generation，并在 retained state 中表示为经目标规则规范化、与顺序无关的 digest。非空重放必须匹配该 digest；空重放可以在不重建 capability 的情况下校验已经保留的创建。显式 disposal 会释放 identity 与 overlay generation。
- 具有空几何图形的启动会在 `awaiting-start` 中保留身份、配置和准备好的启动所有权，而不会生成子项或 PTY。 `StartTerminal` 需要一个完整的几何形状； success 将其用于子项 PTY、规范状态、核心观察和第一帧。精确重播是幂等的，冲突的几何体被拒绝，失败的生成仍然可以重试。
- 立即、无头和测试启动可在创建时提供完整的显式几何图形。主机永远不会替代视觉启动超时或 80x24 后备。
- 每个终端都保留一个完整的运行时主题状态：操作标识、独立主题修订、配色方案、默认前景/背景、可为空的光标颜色以及恰好 256 个调色板条目。 Apply 使用预期修订比较和设置，并在一次语义提交之前更改延迟的启动准备或启动的引擎。精确重放是幂等的；陈旧、冲突、无效、耗尽或失败的操作会保留先前的完整状态，并且恢复清单会返回该状态，而无需实时附件。
- 启动的运行时主题替换会更新 Ghostty 突变边界下的默认值和活动颜色，从而清除先前的 OSC 动态颜色覆盖。稍后的 OSC 4/10/11/12 更改可能会覆盖它； OSC 重置表单、RIS 和 Host 重置返回到最新的运行时主题。物理颜色变化会在不改变生成的情况下推进内容修订一次，因此每个附件都会投射相同的连贯修订。
- 仅在子进程完成、正常 PTY 耗尽以及 PTY 编写器完全静止后，终端输出才变为正常退出。
- 不可恢复的 PTY 读取、PTY 写入、终端回复、调整大小或工作完整性故障将成为显式类型化失败结果。主机会隔离不安全的 I/O，包含所拥有的进程树，并且即使在没有连接 Broker 的情况下也会继续有界清理重试。
- 优雅终止、强制终止和处置是不同的。桌面拥有升级时机；处置是明确的。
- 边界和调度程序背压拒绝或推迟当前操作，而不会破坏保留状态。完整的传出批次返回其提交的前缀，并且不是连接失败。
- 大型投影和历史记录一次跨越引擎到出口所有权：生产者写入保留的响应存储或传输拥有的信封就绪分配，而不是将完整的有效负载复制到第二个缓冲区中。
- 进入后回复丢失的输入具有未知的结果，并且不得自动重播。
- 没有客户端收到原始 PTY 输出。客户端接收从规范终端状态派生的有界协议投影、历史行、选择状态、导出和生命周期事件。
- Kitty 图形保留为有界本机状态。Ghostty 拥有 direct、普通文件、临时文件和受支持的 POSIX 共享内存媒体，以及 animation、virtual/relative 放置、裁剪、查询/删除与有符号 z 合成。Host 只通过 OBKG `0.1.0` 投影自有解码像素和已解析放置；render-detail OBRW `0.1.0` 包含图形快照，text detail 排除图形。macOS 与受支持、链接 libc 的 Linux 目标启用全部媒体；Windows 启用 direct、普通文件和临时文件媒体，但不支持 POSIX 共享内存。
- 终端控制查询（包括 DECRQSS）读取相同的规范 Ghostty 状态，并通过主机拥有的有界终端回复路径将其回复排队；附件永远不会模拟第二个 VT 回复表面。
- 附件大小调整和终端驱动的 80/132 列请求共享一个本机几何提交所有者。在客户端观察到变化之前，主机将 OS PTY 大小、Ghostty 尺寸、单元像素、修订、生成、核心观察和投影快照关联起来。
- 正在运行的 terminal 最多有一个 attachment resize lease。具备 resize 能力的 observer
  可以显式 claim；Host 先发布旧 owner 的 revoke，再发布新 owner 的 grant，随后确认 claim。
  Owner detach 会晋升 successful claim 最新的剩余 attachment，从未获 grant 的 observer 以最早
  admission 打破平局；observer 无需重新打开即可成为 owner。
- 每个附件均从功能修订版 `1` 开始。仅在准确的下一个修订版中接受实时输入功能更新，并且仅对于准确的当前值而言是幂等的。原始和语义 PTY 输入必须在准备和最终队列提交时携带当前修订。降级仅修复目标附件的保留键、鼠标和焦点所有权，然后撤销输入；升级会创建一个新的空所有权分类账，并且永远不会重播输入。
- 焦点是终端范围的 OR 聚合，其输入来自已附加、具备输入能力且投影需求处于活动状态的附件焦点声明。只有第一项声明发出 `FocusIn`，只有最后一项声明释放时发出 `FocusOut`；需求暂停、分离、Broker 丢失与能力降级会先修复目标附件的按键和鼠标所有权，再清除其声明；激活与升级则要求新的焦点输入。
- 应用程序清除和重置请求由具备输入能力的附件直接对 Host 所拥有的 Ghostty 状态执行。清除会擦除活动屏幕并将其原位复位，但不改变屏幕模式；重置会回到全新的主屏幕状态，恢复最新运行时主题的颜色，以及保留配置中明确指定的光标形状和闪烁默认值。终端转义序列可以覆盖这些值，直至对应的重置形式、RIS 或 Host 重置恢复它们。这两个操作都不会向子进程写入转义字节，也不要求投影客户端模拟 VT 行为。
- 语义键事件可以将一个 macOS 选项和弦标记为终结 Alt。主机将该意图应用于 Ghostty 的当前终端模式的权威编码器；它从不将渲染器的布局生成的特殊字符视为已编码的终端字节序列。
- 每个 Ghostty 铃声都会推进一个终端拥有的序列。具有读取能力的附件仅接收稍后的铃声，每个提交的事件记录一次；重新连接不会重播之前的响铃意图。
- OSC 52 可以将一个有界纯文本写入意图发布到一个可读取的附件，但从不读取用户的剪贴板或执行平台效果。传出的背压重试相同的意图，而成功的切换和重新连接不能复制它。
- 以准确 private prefix `orchbee.agent.v1;` 开始的 unknown APC 可以发布一个经 base64url decode 的有界 application signal。Reader-batch admission 是事务性的，最多 64 个已准入 signal 会保持 FIFO 顺序，outgoing backpressure 会保留精确 queue head 直至 commit。Host 不认证 emitter，不解释 Agent 或 provider state，也不请求产品或平台 notification。Protocol record 中的 literal channel `orchbee.agent.v1` 只是历史 Host-to-Desktop discriminator，不是 child wire sequence。OSC 9 与 OSC 777 等公开 notification channel 会被该 transport 忽略。
- Ghostty 进度报告只投影（project）终端当前有界的本地采样；它们不断言进程完成，也不请求系统通知。
- OSC 133 `C` 和 `D` 操作在事务上仅保留一个终端本地完成序列、单调持续时间、挂钟完成时间戳和标准化退出代码。主机从不保留或投影命令文本、参数、目录、环境或提醒输出；新鲜度、焦点、许可、速率限制和通知效果仍然是客户政策。
- Ghostty pin 仍有效时，成功的网格 reflow 保留被跟踪的（tracked）选择与已提交历史锚点，同时取消过时的行在途交付与已编码导出字节。已提交视口读取会在请求窗口可容纳时保留该锚点；否则仅把起始位置向上移动到足以返回完整窗口。屏幕替换和保留行淘汰仍是明确的选择淘汰边界。
- 公共选择导出请求和响应仅接受文字 `plain` 格式。 Ghostty 对 HTML 和原始 VT 的内部格式化程序支持不是主机协议功能，客户端无法选择。

完整的状态模型、重试类、提交边界、不变量和证据矩阵位于[运行时合约](docs/runtime-model.md)中。

## 信任和连续性边界

运行时目录和端点是操作系统用户私有的。 POSIX 使用可信的每用户集合点、无跟随目录描述符、effective-UID 所有权、私有模式、扩展和可继承 ACLs 的删除和验证、终身锁以及实际 Unix 域套接字的后绑定验证。Host 与每个当前本地 Broker 在每次连接时都通过内核凭据要求对端具有相同 effective UID。 Windows 使用拥有的非重分析运行时目录、受保护的 DACLs、单例互斥体、本地命名管道和 peer-SID 验证。

用于 ACL 检查或修改的 POSIX 目录句柄必须可读。Zig 打开目录时同时指定 `.iterate = true` 和不跟随符号链接，让 Linux 返回支持这些操作的描述符，而非 `O_PATH`；所有权、权限模式和 ACL 拒绝检查仍然必需。

Linux 可执行文件启动时，Host 会在获取自身资源或启动工作线程之前，关闭标准输入、输出和错误之外继承的文件描述符。启动方通过参数和环境传递配置，不传递文件描述符。这避免长期运行的 Host 保留 Electron 调试套接字和存储锁。原生黑盒跟踪会注入一个属于启动方的描述符，并在 Host 提供服务时验证它已经释放。

非 Darwin 构建使用 `src/platform/darwin-cross-libraries/libbsm.tbd` 链接 `process_session.c` 声明的两个 audit-token 访问器。描述文件指向系统的 `/usr/lib/libbsm.0.dylib`，与 [Apple 的 OpenBSM 构建配置](https://github.com/apple-oss-distributions/OpenBSM/blob/bb0681420e99d447f466ba70d38589213bb69fea/OpenBSM.xcodeproj/project.pbxproj) 一致，不包含实现。其符号必须与原生访问器声明保持一致；macOS 本机构建使用本机 SDK，并继续负责证明这些调用的运行时行为。

这些控制建立了同一用户的信任边界。端点标识符不是秘密，并且 Terminal Host 不区分一个相同用户应用程序与另一个。它不是多租户服务。

当 Host 存活时，支持 Desktop 重新连接。在 POSIX 上，Desktop 的进程内本机端点适配器拥有客户端套接字，并在协议工作前验证 Host；Terminal Host 独立验证每个已接受客户端。关闭 Desktop 连接不会终止或以其他方式拥有 Host。不承诺在 Host 或操作系统崩溃后迁移或恢复 PTY。客户端模型见 Desktop 的 [POSIX Host 端点合同](../desktop/docs/posix-host-endpoint.zh-CN.md)。

尚未发布的 Rust Worker 是新增的第二种 Broker 实现，而不是可并发的第二个 session。它只在
Server admission 之后验证同用户 Host 并完成相同的精确版本 `Hello` exchange。其当前切片不发送
terminal command，也不创建 PTY；Server 仍是 launch-authorization owner，Host 仍是 terminal
authority。

进程树所有权遵循启动时创建的遏制域：Linux 和 macOS 上的专用会话、FreeBSD 上保留的直接子进程 process-reaper 子树，或 Windows 关闭时终止作业。仅当该域没有实时成员时清理才完成。在 Windows 上，该域是持有终端控制台的 Job 成员：关闭时结束这些成员，并把其余成员（例如从 shell 启动的 GUI 程序）移出终端所有权；任何失败或 Host 崩溃仍会结束整个 Job（[运行时模型](docs/runtime-model.zh-CN.md)）。Host 按带有等待间隔的单调时间节奏观察 Job 统计，仅在没有成员持有该控制台后关闭 HPCON；Desktop 拥有的强制升级会终止控制台持有者，失败时终止整个 Job，使该观察能够完成。进程自然退出不会使 Job 句柄变为有信号状态。在 Linux 和 macOS 上，防止受信任的同一用户子进程故意逃离 POSIX 会话并不是一个隔离承诺。

## 可执行接口

安装的二进制文件是 POSIX 上的 `orchbee-terminal-host` 和 Windows 上的 `orchbee-terminal-host.exe`。其私有启动器ABI是：

```text
orchbee-terminal-host serve \
  --runtime-directory <absolute-path> \
  --endpoint-id <32-lowercase-hex> \
  [--scrollback-bytes <decimal-bytes>]
```

Desktop 从其产品用户数据范围派生运行时目录和端点标识符；主机将两者视为基础设施值，并且不会从中推断出产品拓扑。

该二进制不公开客户端连接或运行时准备命令。Desktop 在其平台适配器内拥有这些操作，而 Terminal Host 继续作为独立保留的服务器进程。

`--scrollback-bytes` 设置新创建终端的启动默认值。当前默认为 10,000,000 个十进制字节；可接受的值范围从 `0` 到 `1,000,000,000`。Broker 可以替换后续终端的进程默认值，或在创建终端时提供显式配置。零禁用保留的回滚。

`orchbee-terminal-host --version` 打印可执行文件名称及其嵌入的语义版本。二进制版本标识打包的进程，并且独立于协议和二进制格式版本。

## SSH Environment release 组件

已接受的 schema-version-2 SSH Environment release 原样复用这个 executable 作为其
`terminal-host` component。本 workspace 拥有 component build recipe；仓库级 release automation
拥有跨 application assembly、signing input 与 final bundle。精确 remote target 为
`aarch64-macos` 与 `x86_64-linux-gnu`，使用 Zig `0.16.0`、`ReleaseSafe`、stripping 与
`-Dhost-version=<semver>` 构建。

Desktop 显式打开已接受的 `terminal-host-v1` service 后，远程 SSH Runtime 会连接此 executable
保持不变的 same-user private endpoint，或启动 cohort 精确锁定的 version，然后通过现有 SSH
attachment relay opaque Terminal Host protocol byte。Terminal Host 仍拥有 native terminal identity、
retained terminal state、PTY 与 process tree。Component 自身不获得 network listener、SSH parsing 或
generic command authority；remote lifecycle 与 relay policy 归 Desktop 和 SSH Runtime 所有。

## 固定本机输入

| 输入           | 价值                                                             |
| -------------- | ---------------------------------------------------------------- |
| Zig            | 正是`0.16.0`                                                     |
| Ghostty 修订版 | `9c3ec931d64561a8407dde7ac984ce156ae91539`                       |
| Ghostty 包哈希 | `ghostty-1.3.2-dev-5UdBCzw0UwUUuHE16pHQg0d9Bplq2ccuQ2cmOJRYkd-y` |

`build.zig.zon` 是源码依赖权威；`windows-conpty.json` 是 Windows 二进制运行时的固定值权威。该构建拒绝另一个 Zig 版本或 Ghostty 存档内容，并且不读取同级 Ghostty 签出。

### Windows 控制信号继承

Windows Host 在发布端点或启动服务工作者之前，通过 `SetConsoleCtrlHandler(NULL, FALSE)`
清除继承的忽略 Ctrl+C 属性。这是一次进程启动操作，失败则拒绝服务准入；仅查询版本时
不修改信号策略。分离运行的 会话 所创建的新进程组，不能让所有终端子进程都忽略 Ctrl+C。
Host 仍保持分离运行，终端 Job 包含关系不变。不附着调用者控制台、不广播信号、不向 shell
注入命令、不在每次创建进程时切换全局状态，也不根据输入字节直接终止进程。

子进程仍拥有自己的控制台模式与处理器：processed input 可以中断前台命令，raw input
接收字节 3。Host 不覆盖子进程主动选择的信号策略。Windows 黑盒流程从忽略 Ctrl+C 的
启动器创建真实 Host，等待子进程回调就绪，经终端协议发送该字节，并要求实际处理信号后
子进程成功退出。仅有输入确认不能满足该判据。参见 Microsoft 的
[可继承处理器属性](https://learn.microsoft.com/windows/console/setconsolectrlhandler)和
[新进程组行为](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)。

### Windows 终端子进程环境

每次创建 Windows 子进程前，Host 都会立即从
`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` 读取 Machine 环境，并从
`HKCU\Environment` 读取 User 环境。它构建一份新 map，只先纳入继承的 Windows volatile runtime
与 identity 值、每驱动器 current-directory 项，再以大小写不敏感名称依次应用 Machine、User
注册表值。任意继承名称不会继续传入：从注册表删除的值会从下一个 terminal 消失；应用需要的
process-only 值必须由 launch profile 提供。Machine 的 `USERNAME` 会被排除，因为 HKLM 通常记录
`SYSTEM` 而非交互用户。注册表的 unnamed default value 会被忽略。缺失 key 不贡献值；不可读或
malformed 输入以 unavailable 拒绝启动，越界输入则以 too-large 拒绝，且都发生在创建伪 Console 前。

`REG_SZ` 值保持字面内容。`REG_EXPAND_SZ` 值先针对 User 优先后的同一份大小写不敏感 raw 注册表
快照解析 `%NAME%` 依赖，再使用已准入的继承 runtime 与 Orchbee private context。展开结果不依赖
注册表枚举顺序。Machine `PATH` 会先完成投影；可展开 User `PATH` 中的 `%PATH%` 指向该 Machine
投影值，绝不指向 Host 的过时 `PATH`。未知依赖或会闭合其他依赖环的 token 保持原文。其他注册表
值类型不作为环境输入。

有效 `PATH` 是展开后的 Machine 值再接 User 值。精确片段按大小写不敏感规则去重，同时保留首次
出现的位置与顺序；不会 trim 或 normalize。若两个范围都未提供 `PATH`，不会合成过时的继承
`PATH`。继承的 `ORCHBEE_*` 会在注册表后重新应用，使注册表数据无法替换 live Session context。
有序启动环境随后应用，其中已授权项与删除项先于每个 creation 私有的 Orchbee overlay，因此拥有
最终优先级。Deferred terminal 在实际启动时取得快照，每次 spawn 失败后的重试都会重新读取。
已启动 terminal 不受影响；非 Windows 启动行为继续使用继承进程环境及相同的有序启动输入。

两个注册表范围合计最多准入 256 个枚举值，以及 512 KiB 存储后的 WTF-8 name、value 与 entry
separator。单个已准入注册表值、单次展开输出及合并后的 `PATH` 各自最多准入 256 KiB。最终投影
map 最多准入 1,536 项，以及 1 MiB 存储后的 WTF-8 name、value 与 entry separator。单次投影最多
执行 65,536 个 expandable-value visit 与 token resolution。`PATH` 去重最多扫描 4,096 个输入片段，
空片段及重复片段均计数。真实注册表读取器与确定性快照投影执行这些 limit，以 typed failure 拒绝
越界输入，而不截断输入或继续无界工作。

### Windows observation 传输运行时

Windows x64 开发 cohort 随 Host 暂存 Microsoft ConPTY `1.25.260930003`
（Terminal `v1.25.2733.0`，MIT）。`windows-conpty.json` 拥有上游 URL、压缩包
及二进制 SHA-256 固定值，随二进制交付许可证。原生 stager 下载精确软件包，
仅发布通过验证的文件，不更新操作系统、已安装 Terminal、PATH 或用户配置。

原始字节对比中，系统 ConHost `10.0.26100.8875` 吞掉了私有 APC，而固定 ConPTY
保留相同字节。接纳此运行时用于原生 CLI 的 Windows Agent 关联路径，
不改变 v3 observation 协议。

固定的运行时还会在成功 resize 后启动有界的 Win32 游标同步。Console 子进程下一次查询
screen-buffer 信息时，ConPTY 会通过 device-status report 向托管终端查询游标位置；
上游等待回复的上限为 500 ms。在正常回复路径中，Host 既有 VT 回复接缝从 resize 后的
Ghostty screen 回答，因此后续子进程输出会采用与已渲染历史相同的行锚，不会覆盖较早的
PowerShell prompt。Host resize 顺序、Ghostty reflow、history generation 及 wire protocol
均保持不变。

原生游标回归先观察 resize 后的尺寸，再发起一次新的 Win32 screen-buffer 查询。ConPTY 在
锁定并读取尺寸前检查是否需要游标同步，因此 resize 前已开始的查询可能返回新尺寸和未同步的游标。
新的查询建立所需的 resize 后观察；精确游标与历史内容断言保持不变，并继续搭配确定性的
Host reader／resize 顺序测试。

| 字段       | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 归属与输入 | Windows PTY 适配器拥有一个可选 DLL 引用及两个不可变文件的读取 handle。运行时根只能是执行中 Host 旁的 `conpty`；文件名和 SHA-256 来自构建 manifest。不接纳环境／PATH／cwd DLL 搜索。                                                                                                                                                                                                                                                 |
| 准入       | 缺少同级目录时保留独立开发 Host 的既有系统 API 路径，不承诺私有 APC 投递。目录存在时必须非重解析点，且含两个精确普通文件。打开时不跟随最终重解析点，也不允许写入／删除共享；检查磁盘文件元数据与有界 SHA-256 后，只通过 DLL 所在目录及 System32 依赖搜索加载绝对 DLL。存在但无效的 bundle 使 PTY 创建失败，不回退系统版。                                                                                                           |
| 状态与归属 | 未打开 → 验证并持有文件 → 加载 DLL 和解析导出 → 拥有伪 Console → 关闭伪 Console → 释放 DLL／文件 handle。失败时释放已获取前缀。resize/close 使用创建伪 Console 的同一个 API 实例。既有 Job 包含域和读取线程收尾保持权威。                                                                                                                                                                                                           |
| 发布与恢复 | Windows 原生 stager 始终包含固定 bundle。旧 cohort 保持既有行为，不隐式修复或删除不完整 bundle。选择此前完整 cohort 可回退。本次增加 Windows 二进制依赖；Linux/macOS 运行时适配器和 Agent wire 契约不变。                                                                                                                                                                                                                           |
| 验证       | loader 准入／篡改用例、既有 Windows PTY 生命周期测试、真实 bundled-ConPTY resize／游标回归、固定版与系统版原始 APC 对比，以及真实原生 CLI Agent 关联。`zig build test-conpty-resize` 会把 manifest 验证过的 bundle 暂存到隔离测试可执行文件旁，覆盖历史 grow／shrink／regrow，并把该原生判据与确定性的 Host reader／resize gate 检查配对。仅命名管道 ACK 不能证明成功。不声称 ARM64 部署、已安装 provider 任务完成或物理 TUI 验收。 |

## 开发

仓库级 `pnpm desktop` 开发运行时为此包运行 `zig build --watch -fincremental -Doptimize=ReleaseSafe`，并通过内容摘要观察已完成的 Host 二进制文件和源码输入。构建失败或同源二进制重建不会影响最后一个可运行的 Host 进程和二进制代次。Zig 代码不会修补进正在运行的 Host 进程：源码变更成功构建后，Desktop 会持久停止并重新暂存当前终端 presentation，退役旧 Host，再把捕获的二进制文件作为新进程启动。因此，PTY 进程状态、VT 历史记录和 Host 标识不会跨越 Zig 源更新。打包 Host 的生命周期和协议行为保持不变。

在原生 Windows 上运行 `node scripts/stage-local-runtime.mjs --output <absolute-directory>` 暂存时，当前进程必须有创建文件符号链接的权限。暂存脚本会在启动 Cargo 或 Zig 前检查此权限；请启用 Windows 开发者模式，或在以管理员身份运行的终端中启动暂存命令，这样 Zig 才能解包固定的 Ghostty 源码依赖。

构建需要仓库中的两个 `crates/local-runtime/src/directory/windows/*.ps1` 启动资产。
已登记输入的复制步骤嵌入其精确连接，因此只修改资产也会唤醒 Zig watch 并改变 Host 模块。
Desktop 的 Host 输入摘要包含同样的两个文件；编码命令帮助函数只消费公共初始化资产。

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host format:check
pnpm --filter @orchbee/terminal-host build
pnpm --filter @orchbee/terminal-host test
pnpm --filter @orchbee/terminal-host test:black-box
pnpm --filter @orchbee/terminal-host test:protocol
pnpm --filter @orchbee/terminal-host test:surface-fixtures
pnpm --filter @orchbee/terminal-host test:tla
pnpm --filter @orchbee/terminal-host test:cross
pnpm verify
```

`test` 会运行当前平台的原生测试、真实本地 IPC/PTY 黑盒跟踪、跨语言协议一致性测试，以及固定 Ghostty 版本的表面夹具比较。默认聚合命令将 Zig 构建任务数限制为一个（`-j1`），因为已确认 Zig 0.16 在 Windows 上存在静态风险：子进程可能继承广泛的句柄，并共享标准输入输出管道写入端。这只改变编译／任务调度；目标、测试、断言和期限均保持不变，运行时协议也不变。这约束了管道继承风险，但不证明每次运行器超时的原因。当固定版本的进程启动实现能够限制子进程句柄继承时，应重新评估这一限制。`scripts/run-zig-tests.mjs` 按[验证](../../docs/zh-CN/VERIFICATION.md#必需任务)中的单测试期限和整次运行期限执行该 Zig 构建。超时时，它列出仍在运行的测试二进制，只终止本次运行启动的进程，并使命令失败。生成的跨语言选择案例只允许 `plain`，并要求 HTML/raw VT 请求严格解码失败；`terminal-parity/HS-07-selection-export` 从所属应用系统测试套件补充 real-PTY/Electron 剪贴板结果。`test:black-box` 先运行事件判定器的确定性检查，再运行外部跟踪：启动构建后的 Host，通过本地 IPC 创建并附加真实终端，断开、重连并重放创建标识，重新附加同一附件标识，通过 PTY 输入驱动子进程，观察 PTY 输出排空、最终修订及成功退出事件，处置保留终端，并要求 Host 进程彻底停止。跟踪还会校验可能穿插于生命周期记录之间的标识作用域元数据事件，但不会把它们当作退出证据；未知事件或畸形元数据仍会失败。`test:protocol` 单独运行协议一致性测试；`test:surface-fixtures` 在临时目录重新生成并逐字节比较三个由 Host 拥有的原生夹具。主/备用 OBTF 夹具对来自同一 Ghostty 终端和 Projection，并跨越真实的备用屏幕转换。另一个 Ghostty 终端会重放丰富的主语料，加入 DECSCA 受保护与未受保护哨兵，并且只通过 `row_encoding.encode` 和 `projection_buffer.encodeRenderSnapshot` 到达 render-detail OBRW 夹具。`schemas/terminal` 负责已跟踪字节；`@orchbee/terminal` 负责严格视图、可信视图和打包存储的精确语义判定。该证据覆盖确定性的原生投影映射，不覆盖 PTY 交付、历史锚定，也不授权其他信任路径。有意修改语料库或编码器并完成审核后，运行 `pnpm --filter @orchbee/terminal-host surface-fixtures:generate`，并审核全部生产者与消费者断言。`test:cross` 为 Linux、FreeBSD、macOS 与 Windows 编译包括黑盒运行器在内的完整测试图；交叉编译不构成这些平台的运行时证据。仓库 CI 执行包的 `test` 命令，因此外部 Host 跟踪会在 Linux、macOS 和 Windows 上原生运行。FreeBSD 仍只有编译证据，不会报告为原生 PTY 或 IPC 一致性证据。

跨平台运行器每次只构建一个目标，并使用 `-j2` 限制 Zig 并发构建任务，避免大型 Ghostty 测试入口的内存开销随平台并发叠加。运行器通过仓库共享检查工具报告各目标耗时和失败日志；发生失败后，不再启动下一个目标。

Windows service focus test 使用同一 target toolchain 构建原生 console child，并启用 VT input，
通过 ConPTY 观察精确 focus-report byte。有界读取与显式 stream terminator 验证完整 O/I/O interval；
cooked console input 可能丢弃 focus report，因此不作为该 byte-stream oracle。原生 projection/selection
test 允许 scheduler 先交付独立的 startup metadata 与 application-feedback batch，再检查原有精确
event、cancellation 与 export 断言。

在原生 Windows 上，常规 `zig build test` gate 还会运行 `test-conpty-resize`。其 console fixture
先以 80×24 写入历史，再通过 `GetConsoleScreenBufferInfo` 观察每次 resize 后的窗口，并在
grow、shrink、regrow 后继续输出。Runner 会下载并验证声明的 ConPTY 压缩包，把 DLL 和
`OpenConsole.exe` 放到复制出的测试可执行文件旁，并要求该可执行文件在创建 PTY 前确认 bundle
准入。同一条原生路径还要求准确的 private-APC correlation payload 到达 Host application-signal
queue。确定性的 engine 检查会在 platform effect 返回后暂停 attachment resize，并证明并发游标
查询只能在新的 Ghostty geometry 提交后产生回复。既有 history-generation 与 selection-reflow 测试
继续负责 retained selection 语义。
同一个 bundled-runtime 测试套件会启动不加载用户 profile 的 Windows PowerShell，以及在标准
位置安装时的 PowerShell 7。它要求初始光标和输入草稿始终紧跟提示符，包括 geometry 改变之后；
语义 reader 标记不得插入新行。
Windows 默认在 resize 时保留提示符单元格；RIS 与 ResetTerminal 恢复该默认值，显式 OSC 133
重画选项在 reset 之前保持权威。

在原生 Windows x64 上，`zig build test-conpty-orchbee-pointer` 会针对同一份经 manifest 验证的
ConPTY bundle 和固定 Ghostty parser 运行受控的 pointer-admission 见证。复制出的 Orchbee Rust
测试二进制通过其继承 console、原生 reader、input decoder 和 terminal-mode guard 进入。Host 注入
语义化的无按键 motion：button-event tracking 必须不发出任何内容；mode 1003 的 reset report 必须
先于 DA1 sentinel，从而准入 any-motion tracking；之后同一个语义 motion 必须作为 typed no-button
mouse event 到达。在该全局探测前，子进程还会请求菜单范围策略（先 `1002h` 再 `1003h`），
不依靠能力回复通过继承 reader 接收语义移动；返回 button-only 后再次抑制空闲移动。
完整 pointer release 必须再次抑制 motion，子进程退出必须恢复原始原生 console
mode、primary screen 和 cursor。该证据只适用于固定的 Windows x64 harness，不覆盖完整 UI input
dispatcher，也不证明物理 mouse record、其他 terminal emulator 的 query 行为、palette 或 font
质量，也不证明交互 latency。

`format:check` 还强制执行包边界规则，即 `src/` 下的每个手写生产 Zig 文件保持在或低于 500 物理行。 `__tests__/` 和 `*_test.zig` 文件下的场景目录除外。

较窄的 POSIX 端点安全证据可以在没有完整主机套件的情况下运行：

```sh
(cd apps/terminal-host && zig build test-cli)
(cd apps/terminal-host && zig build test-platform -Dplatform-test-filter="Windows executable separator")
(cd apps/terminal-host && zig build test-transport)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-linux-gnu)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-freebsd)
```

原生macOS运行练习真正的libc ACL继承和`getpeereid`；跨目标步骤编译 Linux `SO_PEERCRED` 和 FreeBSD `getpeereid` 分支，但不是这些系统的运行时证据。

`test:tla` 通过共享固定运行器检查保留的 Host 交付与进程身份协议。注册表、生命周期、退出、请求结果、图形、输入及历史流程使用各自原生单元/属性/集成套件。[运行时证据映射](docs/runtime-model.zh-CN.md#所需的正确性证据) 列出声明、具体证据与限制。这些实现轨迹不提供已移除模型的全部交错穷举保证。

重点吞吐量线束贯穿真实的 PTY：

```sh
pnpm --filter @orchbee/terminal-host benchmark -- \
  --bytes 67108864 --columns 120 --rows 40 --mode no-demand

pnpm --filter @orchbee/terminal-host benchmark -- \
  --bytes 67108864 --columns 120 --rows 40 --mode active

pnpm --filter @orchbee/terminal-host benchmark -- \
  --file /absolute/path/to/output.txt \
  --columns 113 --rows 38 \
  --cell-pixel-width 18 --cell-pixel-height 36 \
  --mode active
```

文件固定模式以 `time -p cat -n` 运行仓库固定工作负载，通过子进程有效 `PATH` 解析这两个工具，而不依赖固定 FHS 路径；Desktop 工具采用同一同步策略。受支持的 POSIX 环境必须能从 `PATH` 访问 `time` 与 `cat`，否则基准会报告明确错误。

`ORCHBEE_TERMINAL_HOST_TRACE=1` 为受控桌面基准测试工具启用有界交互计时记录。这是测试诊断，而不是产品遥测。

构建输出保留在被忽略的 `.zig-cache/` 和 `zig-out/` 目录中。

## 原生组合边界

`src/main.zig` 是可执行组合根。 `src/terminal_host.zig` 是该可执行文件使用的狭窄私有外观。每个域测试图直接导入其所属的命名模块；外观不会重新导出内部结构以进行测试。构建图是可见性权威：没有其他工作区导入这些 Zig 源文件。

源模块应遵循运行时合约中的语义所有者：本地 IPC 传输记录，本机协议适配器转换记录，主机注册表拥有终端成员身份，每个保留的终端记录将一个语义会话绑定到其延迟启动准备或已启动的引擎，每个会话拥有其生命周期，每个 Broker 会话拥有其附件。仅当文件分割使这些所有权边界之一更加清晰时才有用。

包本地[模块边界契约](docs/module-boundaries.md) 把这些 owner 映射到源目录、具名（named）Zig 模块、允许的导入与测试放置。

## 程序化按键编码检查

用 `zig build test-input-encoding` 运行旧模式/Kitty 语义按键的针对性回归。Host 从终端当前模式派生编码。旧模式的 Super 字符按下使用既有 CSI-u 编码器保留组合，不改变协商模式，也不报告旧模式松开事件。Shift 可打印文本与 Kitty 完整报告的按键身份分别保留；Worker send 适配器负责逻辑字符规范化。

## 只读 PNG 图片服务

`DecodeGraphicsPng` 解析请求连接的可读附件，复用现有 Wuffs PNG 解码器，不写入终端输入或修改终端图形。服务先预留受限输出空间，验证完整图片，返回带关联标识及真实终端身份的 RGBA 像素。无效 PNG、尺寸不匹配和其他连接的附件返回明确失败。输入最多 16 MiB、解码像素最多 32 MiB，每个尺寸轴最多 4096。

`zig build test-graphics` 检查 PNG 服务、附件授权、终端文本保持及已有直接图形投影。Windows 服务测试自行创建并释放 ConPTY 进程和运行时，不打开 Desktop 窗口。

<!-- documentation-pairing: english-sha256=4649b7f5f0b79f1a93c07d104aced89fa356c71c551ccefd57b1208d9c5e1b89 zh-cn-sha256=d168db55f82f9bd842ff9a75ff5a09e165a441c1af370bd068300beb0d0c0da0 -->
