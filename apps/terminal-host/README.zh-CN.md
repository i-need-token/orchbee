# Terminal Host

Terminal Host是Orchbee的本地权威终端执行服务。它拥有本机终端身份、PTYs 和进程树、规范的 Ghostty 状态、保留的生命周期以及一个受信任的桌面代理的附件范围的终端功能。

该应用程序是私有的并且是实验性版本。它安装一个可执行文件；它不导出可重用的本机库。

## 契约权威

- [运行时契约和正确性模型](docs/runtime-model.md)拥有主机生命周期、状态、交付、重试、失败、并发和证据要求。
- [终端协议参考](../../packages/terminal-protocol/references/protocol.md) 拥有线路记录、编解码器、消息模式、格式版本和协议安全限制。
- [桌面版](../desktop/README.md)拥有产品拓扑、恢复策略、展示、平台效果。

已接受的 [Desktop 更新连续性提案](../../proposals/terminal-host/desktop-terminal-host-update-continuity.md)记录未来实现设计；它不是当前运行时权威。

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

- 一个端点至多有一个活动的物理桌面代理会话。
- 桌面断开连接会破坏该代理的附件，但不会终止或处置保留的终端。
- 具有保留终端的主机仍可用于重新连接。没有代理且没有保留终端的主机可以退出。
- 当 terminal 保留在一个 Host epoch 中时，terminal creation 按 creation identity 与 durable launch fingerprint 保持原子和幂等。单独的非持久化 environment overlay 只应用于该 creation generation，并在 retained state 中表示为经目标规则规范化、与顺序无关的 digest。非空重放必须匹配该 digest；空重放可以在不重建 capability 的情况下校验已经保留的创建。显式 disposal 会释放 identity 与 overlay generation。
- 具有空几何图形的启动会在 `awaiting-start` 中保留身份、配置和准备好的启动所有权，而不会生成子项或 PTY。 `StartTerminal` 需要一个完整的几何形状； success 将其用于子项 PTY、规范状态、核心观察和第一帧。精确重播是幂等的，冲突的几何体被拒绝，失败的生成仍然可以重试。
- 立即、无头和测试启动可在创建时提供完整的显式几何图形。主机永远不会替代视觉启动超时或 80x24 后备。
- 每个终端都保留一个完整的运行时主题状态：操作标识、独立主题修订、配色方案、默认前景/背景、可为空的光标颜色以及恰好 256 个调色板条目。 Apply 使用预期修订比较和设置，并在一次语义提交之前更改延迟的启动准备或启动的引擎。精确重放是幂等的；陈旧、冲突、无效、耗尽或失败的操作会保留先前的完整状态，并且恢复清单会返回该状态，而无需实时附件。
- 启动的运行时主题替换会更新 Ghostty 突变边界下的默认值和活动颜色，从而清除先前的 OSC 动态颜色覆盖。稍后的 OSC 4/10/11/12 更改可能会覆盖它； OSC 重置表单、RIS 和 Host 重置返回到最新的运行时主题。物理颜色变化会在不改变生成的情况下推进内容修订一次，因此每个附件都会投射相同的连贯修订。
- 仅在子进程完成、正常 PTY 耗尽以及 PTY 编写器完全静止后，终端输出才变为正常退出。
- 不可恢复的 PTY 读取、PTY 写入、终端回复、调整大小或工作完整性故障将成为显式类型化失败结果。主机会隔离不安全的 I/O，包含所拥有的进程树，并且即使在没有连接代理的情况下也会继续有界清理重试。
- 优雅终止、强制终止和处置是不同的。桌面拥有升级时机；处置是明确的。
- 边界和调度程序背压拒绝或推迟当前操作，而不会破坏保留状态。完整的传出批次返回其提交的前缀，并且不是连接失败。
- 大型投影和历史记录一次跨越引擎到出口所有权：生产者写入保留的响应存储或传输拥有的信封就绪分配，而不是将完整的有效负载复制到第二个缓冲区中。
- 进入后回复丢失的输入具有未知的结果，并且不得自动重播。
- 没有客户端收到原始 PTY 输出。客户端接收从规范终端状态派生的有界协议投影、历史行、选择状态、导出和生命周期事件。
- Kitty 图形保留为有界本机状态。Ghostty 拥有 direct、普通文件、临时文件和受支持的 POSIX 共享内存媒体，以及 animation、virtual/relative 放置、裁剪、查询/删除与有符号 z 合成。Host 只通过 OBKG `0.0.2` 投影自有解码像素和已解析放置；render-detail OBRW `0.0.3` 包含图形快照，text detail 排除图形。macOS 与受支持、链接 libc 的 Linux 目标启用全部媒体；Windows 启用 direct、普通文件和临时文件媒体，但不支持 POSIX 共享内存。
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
- 以准确 private prefix `orchbee.agent.v3;` 开始的 unknown APC 可以发布一个经 base64url decode 的有界 application signal。Reader-batch admission 是事务性的，最多 64 个已准入 signal 会保持 FIFO 顺序，outgoing backpressure 会保留精确 queue head 直至 commit。Host 不认证 emitter，不解释 Agent 或 provider state，也不请求产品或平台 notification。Protocol record 中的 literal channel `orchbee.agent.v1` 只是历史 Host-to-Desktop discriminator，不是 child wire sequence。OSC 9 与 OSC 777 等公开 notification channel 会被该 transport 忽略。
- Ghostty进度报告项目仅是终端当前限定的本地样本；他们不断言流程完成或请求系统通知。
- OSC 133 `C` 和 `D` 操作在事务上仅保留一个终端本地完成序列、单调持续时间、挂钟完成时间戳和标准化退出代码。主机从不保留或投影命令文本、参数、目录、环境或提醒输出；新鲜度、焦点、许可、速率限制和通知效果仍然是客户政策。
- Ghostty pin 仍有效时，成功的网格 reflow 会保留跟随选择与已提交历史锚点，同时取消过时的行在途交付与已编码导出字节。已提交视口读取会在请求窗口可容纳时保留该锚点；否则仅把起始位置向上移动到足以返回完整窗口。屏幕替换和保留行淘汰仍是明确的选择淘汰边界。
- 公共选择导出请求和响应仅接受文字 `plain` 格式。 Ghostty 对 HTML 和原始 VT 的内部格式化程序支持不是主机协议功能，客户端无法选择。

完整的状态模型、重试类、提交边界、不变量和证据矩阵位于[运行时合约](docs/runtime-model.md)中。

## 信任和连续性边界

运行时目录和端点是操作系统用户私有的。 POSIX 使用可信的每用户集合点、无跟随目录描述符、effective-UID 所有权、私有模式、扩展和可继承 ACLs 的删除和验证、终身锁以及实际 Unix 域套接字的后绑定验证。主机和桌面在每个连接上都需要通过内核凭据连接的对等方的有效 UID。 Windows 使用拥有的非重分析运行时目录、受保护的 DACLs、单例互斥体、本地命名管道和 peer-SID 验证。

这些控制建立了同一用户的信任边界。端点标识符不是秘密，并且 Terminal Host 不区分一个相同用户应用程序与另一个。它不是多租户服务。

当 Host 存活时，支持 Desktop 重新连接。在 POSIX 上，Desktop 的进程内本机端点适配器拥有客户端套接字，并在协议工作前验证 Host；Terminal Host 独立验证每个已接受客户端。关闭 Desktop 连接不会终止或以其他方式拥有 Host。不承诺在 Host 或操作系统崩溃后迁移或恢复 PTY。客户端模型见 Desktop 的 [POSIX Host 端点合同](../desktop/docs/posix-host-endpoint.zh-CN.md)。

进程树所有权遵循启动时创建的遏制域：专用的 POSIX 会话或 Windows 关闭时终止作业。仅当该域没有实时成员时清理才完成。在 Windows 上，Host 会等待 Job 的空信号后再关闭 HPCON；Desktop 拥有的强制升级会终止 Job 并唤醒该等待。防止受信任的同一用户子进程故意逃离 POSIX 会话并不是一个隔离承诺。

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

`--scrollback-bytes` 设置新创建终端的启动默认值。当前默认为 10,000,000 个十进制字节；可接受的值范围从 `0` 到 `1,000,000,000`。代理可以替换后续终端的流程默认值，或者在创建终端时提供显式配置。零禁用保留的回滚。

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

`build.zig.zon` 是依赖权限。该构建拒绝另一个 Zig 版本或 Ghostty 存档内容，并且不读取同级 Ghostty 签出。

## 开发

仓库级 `pnpm desktop` 开发运行时为此包运行 `zig build --watch -fincremental -Doptimize=ReleaseSafe`，并通过内容摘要观察已完成的 Host 二进制文件和源码输入。构建失败或同源二进制重建不会影响最后一个可运行的 Host 进程和二进制代次。Zig 代码不会修补进正在运行的 Host 进程：源码变更成功构建后，Desktop 会持久停止并重新暂存当前终端 presentation，退役旧 Host，再把捕获的二进制文件作为新进程启动。因此，PTY 进程状态、VT 历史记录和 Host 标识不会跨越 Zig 源更新。打包 Host 的生命周期和协议行为保持不变。

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

`test` 会运行当前平台的原生测试、真实本地 IPC/PTY 黑盒跟踪、跨语言协议一致性测试，以及固定 Ghostty 版本的表面夹具比较。生成的跨语言选择案例只允许 `plain`，并要求 HTML/raw VT 请求严格解码失败；`terminal-parity/HS-07-selection-export` 从所属应用系统测试套件补充 real-PTY/Electron 剪贴板结果。`test:black-box` 只运行外部跟踪：启动构建后的 Host，通过本地 IPC 创建并附加真实终端，断开、重连并重放创建标识，重新附加同一附件标识，通过 PTY 输入驱动子进程，观察 PTY 输出排空、最终修订及成功退出事件，处置保留终端，并要求 Host 进程彻底停止。`test:protocol` 单独运行协议一致性测试；`test:surface-fixtures` 在临时目录重新生成并逐字节比较三个由 Host 拥有的原生夹具。主/备用 OBTF 夹具对来自同一 Ghostty 终端和 Projection，并跨越真实的备用屏幕转换。另一个 Ghostty 终端会重放丰富的主语料，加入 DECSCA 受保护与未受保护哨兵，并且只通过 `row_encoding.encode` 和 `projection_buffer.encodeRenderSnapshot` 到达 render-detail OBRW 夹具。协议包负责已跟踪字节及其导出；`@orchbee/terminal` 负责严格视图、可信视图和打包存储的精确语义判定。该证据覆盖确定性的原生投影映射，不覆盖 PTY 交付、历史锚定，也不授权其他信任路径。有意修改语料库或编码器并完成审核后，运行 `pnpm --filter @orchbee/terminal-host surface-fixtures:generate`，并审核全部生产者与消费者断言。`test:cross` 为 Linux、FreeBSD、macOS 与 Windows 编译包括黑盒运行器在内的完整测试图；交叉编译不构成这些平台的运行时证据。仓库 CI 执行包的 `test` 命令，因此外部 Host 跟踪会在 Linux、macOS 和 Windows 上原生运行。FreeBSD 仍只有编译证据，不会报告为原生 PTY 或 IPC 一致性证据。

`format:check` 还强制执行包边界规则，即 `src/` 下的每个手写生产 Zig 文件保持在或低于 500 物理行。 `__tests__/` 和 `*_test.zig` 文件下的场景目录除外。

较窄的 POSIX 端点安全证据可以在没有完整主机套件的情况下运行：

```sh
(cd apps/terminal-host && zig build test-cli)
(cd apps/terminal-host && zig build test-transport)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-linux-gnu)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-freebsd)
```

原生macOS运行练习真正的libc ACL继承和`getpeereid`；跨目标步骤编译 Linux `SO_PEERCRED` 和 FreeBSD `getpeereid` 分支，但不是这些系统的运行时证据。

`test:tla` 通过仓库运行器对 Host 拥有的注册表、单终端生命周期、多运行时关闭、交付与请求结果抽象执行模型检查。它属于变更触发的正确性证据，刻意不纳入常规 `test` 门。运行器默认使用 `PATH` 中的 `java`；可将 `ORCHBEE_TLA_JAVA` 设为明确的 Java 可执行文件，或将 `ORCHBEE_TLA_TIMEOUT_MILLISECONDS` 设为正数的单项检查超时覆盖，以适应较慢的验证环境。

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

## 原生输入法组合边界

`src/main.zig` 是可执行组合根。 `src/terminal_host.zig` 是该可执行文件使用的狭窄私有外观。每个域测试图直接导入其所属的命名模块；外观不会重新导出内部结构以进行测试。构建图是可见性权威：没有其他工作区导入这些 Zig 源文件。

源模块应遵循运行时合约中的语义所有者：本地 IPC 传输记录，本机协议适配器转换记录，主机注册表拥有终端成员身份，每个保留的终端记录将一个语义会话绑定到其延迟启动准备或已启动的引擎，每个会话拥有其生命周期，每个代理会话拥有其附件。仅当文件分割使这些所有权边界之一更加清晰时才有用。

包本地[模块边界契约](docs/module-boundaries.md) 将这些所有者映射到源目录，命名为 Zig 模块，允许导入和测试放置。
