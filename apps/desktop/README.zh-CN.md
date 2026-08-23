# 桌面

沙盒化 Electron 产品客户端和 Orchbee 的本机 Terminal Host 的本地代理。

## 当前范围

桌面与主机支持的投影终端组成共享工作台。 Workbench拥有Project、Workspace、Tab、Panel、`TerminalPresentationId`状态； Electron Main拥有持久的技术绑定和Electron独有的特权效果； `apps/terminal-host` 拥有 PTYs、进程、Ghostty VT 状态、历史记录和真实终端身份。浏览器客户端在其自己的页面中提交剪贴板和白名单链接效果。共享工作台文档均不存储在主机中。

Main 启动时无需创建固定终端、打开附件或启动 Web 侦听器。它将持久的 `TerminalPresentationId` 恢复为 `TerminalBindingId` 注册表，拥有一个进程范围的规范 Workbench 共享文档协调器，并通过沙盒预加载传输一个原子允许列表功能包。它的七个端口有一个固定的顺序：

- 客户端会话控制提供一个原子文档/终端观察引导程序，接受完整的精确下一个共享文档提案，并发布一个有序的规范事件流；
- 投影控件仅为显示的终端视口打开并释放范围内的主机附件；
- 应用程序生命周期控制仅包含退出原因、用户选择、静默确认、共享文档修订、聚合进度、类型化故障以及恢复正在进行的开发主机重启所需的工作台表示标识；
- 浏览器访问控制仅包含显式启动、模式切换、打开、复制、停止、净化的原始状态和有限故障。它从不携带能力承载URL；
- 项目文件夹获取请求一个本机现有目录选择，并仅返回相关的选定路径、取消或有限失败；
- Project Worktree 控制只从当前规范 Project Directory 发现 repository，并根据 repository、base、branch 与 Workspace 选项创建一个托管 Worktree；renderer 命令绝不携带目标路径；
- Terminal-launch-profile 控制项目并改变系统 shell 或显式可执行启动选择的一个有界设备本地目录。它不携带环境、CWD、机密、项目有效负载或通用文件系统功能。

客户端会话和投影有效负载是来自 `@orchbee/workbench-client-protocol` 的平台中立合约。 `@orchbee/desktop-client-protocol` 拥有原子 Electron 渲染器引导程序、Electron 应用程序生命周期合约、窄浏览器访问命令合约、类型化 Project Folder 获取与 Project Worktree 合约，以及终端启动配置文件命令合约。 Desktop Main 和 Web 渲染器各自将通用合约调整到 `MessagePort` 的末端；传输细节都不会进入共享工作台客户端运行时。

渲染器没有 Node 集成、通用 Electron IPC、filesystem read capability、Host inventory 或原始 terminal-lifetime 权限。它不能直接调用 provision、restore、close 或 storage CAS effect。Preload 只接受一个 bootstrap，同时验证全部七个 port，保留固定的 client-session/projection/application-lifetime/Browser-access/Project-Folder-acquisition/Project-Worktree/terminal-launch-profiles 顺序，并证明 context isolation 与 sandboxing。Project Folder port 不接受 renderer path，也不公开 enumeration 或 mutation primitive。Project Worktree port 不接受源或目标 path：discovery 从规范 Project state 推导 repository，创建只接受返回的不透明 identity 与 Git 值。单独的 context-isolated file-drop adapter 只公开当前 platform 与 Electron 的 `File`-scoped backing-path lookup；它不能接受 arbitrary path string、读取 file content、枚举 directory 或执行 IPC request。一个拖入的 disk `File` 可以携带 file 或 directory backing path，但不会授予 child enumeration。Web renderer 只为 user drop 准入该 adapter，应用有界且兼容 Ghostty 的 POSIX live-buffer escaping，直接发布 prepared text，并且绝不追加 Enter。Partial 或 malformed bootstrap 不会启动 renderer runtime。

OSC 52 剪贴板写入使用现有投影端口，而不是通用 preload 剪贴板 API。Main 只记住转发到附件的第一个 Host 请求；渲染器自动提交决定必须精确重复其附件、序列与有界文本。匹配的肯定决定会执行 Desktop 拥有的纯文本写入并消费待处理请求；不匹配、重复、Broker 背压、页面退役或附件处置均不写入。Gateway 浏览器附件会尝试通过本地剪贴板 API 执行请求，并发送同一个精确决定，仅用于消费 Main 的待处理记录，避免 Electron 重复该客户端本地效果。

Main 在允许客户端之前加载并验证共享文档一次。可用的文档可恢复项目、工作区、选项卡、组、持久后备/自定义选项卡标题、面板/拆分结构、Presentation 布局和生命周期意图；每个页面都会在内存中派生一个新的 Client View。Desktop Web 组合可以从该 Electron 页面会话存储的一个精确且仍存活的 `TerminalPresentationId` 中，只恢复焦点 Panel 与显示的子 Presentation。主动导航、披露、焦点、最大化、主动子项、自然终端标题、当前 CWD 和 Split 权重不进入共享文件。丢失的文档将每个恢复的绑定投影为独立的 `retain` 选项卡，并为每个加入页面提供相同的 revision-`0` 初始化种子。第一个接受的提案会在 Main 执行恢复/配置效果之前持久化该种子。不可用的加载或失败的首次提交不会创建替换文档或终端效果。未引用的技术绑定仍然保留，但不会默默加入共享文档。

协调器是传输中立的，支持独立的页面会话，没有产品数量上限。 Electron 使用直接 `MessagePort` 适配器。显式浏览器访问启动 `apps/server` 和 `apps/gateway` 作为按需实用程序进程； Desktop 拥有这两个生命周期，并为每个经过身份验证的页面分配一对客户端会话/投影端口。服务器拥有静态渲染器交付，而网关拥有 exact-Origin 准入、能力认证、成帧和环回上的传输背压或显式选择的可信 LAN。网关永远不会接收应用程序生命周期端口或主机连接。 `apps/web` 拥有平台中立的 Workbench 客户端运行时和普通浏览器中使用的单个渲染器应用程序、网关浏览器客户端和 Electron。开发Electron是从环回Web来源上的User-Agent发现的。 Packaged Desktop 在 Electron 准备就绪之前注册标准安全 `orchbee:` 方案，为 `orchbee://app` 的 Web 构建提供服务，并回退到 `index.html` 来处理浏览器路由器文档请求。桌面拥有渲染器资源交付，而不是另一个渲染器源树。请参见 [`docs/multi-client-session-coordinator.md`](docs/multi-client-session-coordinator.md) 和 [`../web/docs/navigation-routing.md`](../web/docs/navigation-routing.md)。

Desktop 严格要求唯一的当前 Host 协议 `0.0.19`；预发布版本既不协商，也不记录历史协议变体。Electron 使用 `42.8.0`，即本仓库所选当前 Electron 主版本中的最新次版本。

`HostConnection` 在每个代理调用者上拥有一个连接全局出站窗口：最多 64 个未应答请求记录和 4 个 MiB 编码请求字节。它会保留该许可，直到相关响应或连接失败为止。已返回 `false` 的完整窗口或节点可写在切换之前拒绝工作，并可重试 `host-transport-backpressure`；在 `drain` 之前没有请求写入恢复。如果移交的请求达到其截止日期，Desktop 将终止连接，并使用不可重试的 `host-request-outcome-unknown` 拒绝所有待处理和未来的请求，因此在出现不明确的结果后，永远不会重播输入。

每个投影终端 Broker 最多保留 64 个已交给 Host 的命令，其中输入专用子窗口为 16，避免一个终端的滚轮或按键突发在通常情况下耗尽连接全局窗口。它通过一个按到达顺序排列的待发窗口吸收渲染器突发；该窗口最多容纳 1,024 个已验证的非 resize 命令及其总计 4 MiB 的序列化命令数据。请求完成后会立即交付最早的可调度待发命令，并且不允许后来的命令越过受输入子窗口阻挡的队首；resize 继续使用独立的最新值合并槽。在这些边界内获准的命令既不会报告渲染器背压，也不会废弃附件。真正填满的待发窗口仍会以关闭方式失败，丢弃从未交给 Host 的工作，并且只废弃该附件。`ProjectedTerminalBrokerAdmission` 检查输入活动与待发限制的有限状态细化、准入顺序保持、有界溢出，以及在 Host 完成公平性下待发窗口最终排空。

入站 Host 记录跨 JavaScript Promise 结算保持线协议顺序。`HostConnection` 在解码时认领每个相关响应、完成该响应，并将后续记录延迟到一个事件循环屏障之后，使响应延续先于稍后的 Host 事件运行。先于响应到达的事件仍保持在前；连续事件/帧交付保持同步；每条记录解码后，传输尾部都会跟进，而无需重新开放请求准入。

在 POSIX 上构造 `HostConnection` 之前，Desktop 会要求打包的本机主机帮助程序验证运行时目录和实际套接字、连接并根据内核对等凭据验证服务器的有效 UID。只有经过身份验证的 same-UID 连接才能抑制主机启动，并且每次重试或稍后桌面重新连接都会重复检查。助手是该连接的有界 stdio 代理；它的退出不能拥有或终止保留的主机进程，并且其传输保留 `HostConnection` 准入和未知结果行为不变。

Desktop 将一个 Terminal Host 端点扩展到其产品 `userData` 身份，该身份将 OS 用户、产品配置文件和发布通道数据根分开。短 POSIX 运行时路径是操作系统受信任的每用户运行时位置（Linux 上的 `/run/user/<effective-uid>` 和 Electron 上 macOS 上的 Electron 的每用户临时位置）的稳定产品范围子代，从来都不是确定性的共享 `/tmp` 的子级。 Windows 命名管道标识源自相同的产品范围。显式端点和运行时覆盖仅适用于受控开发和测试； POSIX 覆盖仍然通过本机无跟随所有者、私有模式和 ACL 准备和验证。每次成功握手后，Desktop 都会通过 `SetTerminalDefaults` 发送当前终端默认值，因此重新连接到已运行的单例与新启动 CLI 具有相同的结果。新终端使用与工作台画布对齐的灯光轮廓；保留的终端保留与其现有主机会话一起存储的配置文件。设备本地产品设置由共享 Web 渲染器拥有，并且永远不会成为主机默认设置。

Desktop Main 还拥有单个本地 Electron 窗口生命周期。它恢复严格的设备本地正常边界和最大化记录，将保存的几何图形重新投影到当前显示工作区域，并通过应用程序菜单及其加速器使用的 Main 拥有的语义操作公开最大化/恢复和全屏。全屏保持短暂状态，窗口状态永远不会进入共享工作台文档或终端恢复。请参阅[`docs/window-lifecycle.md`](docs/window-lifecycle.md)。

在 macOS 上，本机应用程序菜单还显示仅会话的 **安全键盘输入** 复选框。 Main拥有的一个语义Action控制Electron的特权应用程序能力；沙盒渲染器不接收安全输入 IPC 或状态。默认关闭。窗口模糊、应用程序清理、退出禁用功能和清除勾选状态；返回焦点不会默默地重新启用它。此切片故意不推断密码提示、保留选择或在停用后自动重新获取安全键盘输入。

## 渲染器文档生命周期

Electron Main 拥有渲染器文档生成及其客户端会话、投影和应用程序生命周期控件。 Electron `did-start-navigation` 详细信息是可信的导航输入； Main 在改变这一代之前用一个确定性策略对它们进行分类：

| 主机架 | 相同的文件 | 结果               |
| ------ | ---------- | ------------------ |
| 是的   | 否         | 开始生成渲染器文档 |
| 是的   | 是的       | 无操作             |
| 否     | 否         | 无操作             |
| 否     | 是的       | 无操作             |

渲染器文档开始加载时，会停用上一页面的控制对象并记录待处理生成。`did-finish-load` 只为最新生成发布一个新的原子 bootstrap；退役失败会封闭该生成。Main 只允许在配置的渲染器源内进行顶层导航，使开发重新加载与打包后的浏览器 Router 文档请求能够完成；跨源导航和新窗口仍被拒绝。同文档浏览器 Router 更新与子帧导航保留当前生成及全部活动控制对象。渲染器进程丢失与窗口关闭仍是显式退役路径；Host 终端与规范共享文档在页面退役后继续存在。同一窗口重载时，替换后的 Web 组合会在确认精确 Presentation 仍属于 bootstrap 文档后，独立恢复其有界会话焦点提示；Main 不接收 Client View 或浏览器存储能力。该模型不新增投影重试、控制端口超时或 IPC 契约。

`startsDesktopRendererDocument` 拥有纯决策权。 `DesktopApplication` 拥有生成转换和 Electron 效果。单元测试涵盖所有四个决策行和完整的导航/重新加载排序； `DesktopLifecycle` 的 `renderer-document` 配置检查忽略导航保留、当前一代控制所有权、替换前的退役以及通过一代 `2` 的有界挂起一代进度。 Electron 42.8.0 提供导航分类和事件顺序； Electron 发出事件之前操作系统终止仍然不属于此声明范围。

## 原生项目文件夹获取

Desktop 是当前唯一注入 Workbench existing-Folder 采集功能的组合。一个相关渲染器请求要求 Electron Main 打开所属窗口的 `dialog.showOpenDialog(..., { properties: ['openDirectory'] })`。 Main 最多接受该渲染器的一个活动请求，只需要一个选定的路径，将其标准化为绝对路径，并检查它是否仍命名为可读、可遍历的目录。验证遵循符号链接，同时保留用户选择的路径拼写。渲染器无法提交验证路径或请求目录内容。

选中结果会进入现有的 Workbench 项目创建或项目目录添加 Action。取消，以及有界对话框、类型、存在性、权限或校验失败，都不会改变 Project 状态。Workbench 会在变更前重新检查最新添加目标与精确重复路径。渲染器文档退役、重新加载、卸载、端口关闭、窗口关闭和应用处置都会撤销交付，防止延迟完成的原生请求改变替代页面。本切片不创建、删除或编辑任何文件系统条目；`New Folder…` 保持禁用。受维护的行为、证据、限制与回滚契约见 [`docs/project-folder-acquisition.md`](docs/project-folder-acquisition.md)。

## 托管 Project Worktree 创建

Desktop 是当前唯一注入托管 Worktree 创建能力的组合。Main 从目标 Project 的当前可用 Directory
发现 Git repository，按规范 Git common-directory identity 去重，并返回可编辑的 base 与新分支
默认值。Renderer 只读展示固定托管位置，可以请求可选 Workspace，但不能选择或提交文件系统 path。

打包版 Desktop 使用 `~/.orchbee`；每个开发 checkout 使用
`~/.orchbee-dev-<scope-digest>`，稳定 digest 从该 checkout 的规范源码路径推导。托管 Worktree 位于
`<data-root>/worktrees/<opaque-id>`。本期没有应用级或 Project 级位置设置。Main 在 Git mutation 前
记录恢复意图，原子提交 Workbench Directory 与可选 Workspace；对已变化或所有权不明确的结果，
会保守保留而不递归删除用户数据。参见 [`docs/data-root.md`](docs/data-root.md) 与
[`docs/project-worktree-creation.md`](docs/project-worktree-creation.md)。

## 浏览器访问预览

Desktop 拥有针对相同规范工作台和终端会话的选择加入浏览器访问工作流程。浏览器访问在进程启动时停止。选择 **此计算机** 在临时 `127.0.0.1` 端口上启动受信任的 Web 呈现器服务器和经过身份验证的网关。选择 **本地网络预览** 是一个单独的显式操作，它将两个侦听器绑定到 `0.0.0.0`，同时发布仅检测到的 RFC 1918 IPv4 源。未指定的侦听器地址永远不是客户端 URL。公共接口地址、IPv6、主机名、发现、Internet 入口和后台服务安装不在此预览范围内。

完整的访问URL在其片段中包含一种进程生命周期的高熵能力。 Desktop Main 拥有该秘密并自行执行复制和 system-Browser 效果。沙盒渲染器仅接收选定的模式、生命周期阶段和非秘密 HTTP 来源；它永远不会接收片段、网关端口、功能、子进程句柄或任意剪贴板和 URL 功能。获得复制的预览链接的任何人都可以观察和控制共享的工作台和终端，直到浏览器访问停止。 HTTP 流量未加密。 UI 标记并警告接受的预览边界； TLS、登录、持久凭证、比进程生命周期更精细的撤销以及生产远程客户端协议都不是目标。

该工作流程有一个权威的桌面主所有者和以下序列化转换：

| 当前阶段           | 意图或结果             | 下一阶段                                          |
| ------------------ | ---------------------- | ------------------------------------------------- |
| `stopped`          | 启动一种选定的模式     | `server-starting`                                 |
| `server-starting`  | 服务器拥有一个临时端口 | `gateway-starting`，或 `stopping`（如果意图改变） |
| `gateway-starting` | 网关拥有一个临时端口   | `ready`，或 `stopping`（如果意图改变）            |
| `ready`            | 同模式启动             | 无操作；重用功能和监听器                          |
| `ready`            | 停止或选择其他模式     | `stopping`                                        |
| 任何起始阶段       | 分配或验证失败         | `stopping`                                        |
| `stopping`         | 两个拥有的资源均已停用 | `stopped`，剩余1个时启动排队模式                  |
| 任何直播阶段       | 桌面关闭               | `stopping`，接终端`disposed`                      |

一个Desktop进程最多有一台Server和一台Gateway。一个准备好的网关总是有其匹配的服务器。失败或被取代的启动会在下次启动之前清理部分资源，并且即使渲染器页面消失，桌面关闭也会撤销该功能。服务器仅拥有静态 HTTP 传递。网关拥有exact-Origin准入、能力认证、成帧和页面套接字。 Desktop Main 仍然是共享产品状态的唯一写入者，并且仅在身份验证后才授予新的页面范围的客户端会话/投影对。

`browser-access` 的 `DesktopLifecycle` 配置在公平完成拥有的流程效果的情况下检查资源所有权、单模式准备情况、清理和最终能力撤销。确定性单元测试涵盖地址策略、同模式合并、模式替换、部分启动补偿、复制/开放限制和关闭。真实服务器/网关测试涵盖两种绑定模式、exact-Origin 拒绝、错误能力拒绝和远程主机 WebSocket 构造。打包的系统路径验证四个所需资源和一个 Electron 以及浏览器共享会话跟踪。

## 生命周期和身份所有权

维护的身份链是：

```text
Workbench TerminalPresentationId
        │ committed shared-document intent
        ▼
Desktop TerminalBindingId ── durable binding store
        │ recovery reconciliation
        ▼
HostEpoch + CreationId + TerminalId ── Terminal Host authority
        │ visible viewport demand
        ▼
AttachmentId ── one disposable projection capability
```

`TerminalPresentationId` 由 Workbench 创建并停用。 Main 保持对 `TerminalBindingId` 的严格内存中双射；它从不通过生命周期控制公开绑定、创建、主机、终端或附件身份。主机仍然是该过程和 VT 的真实来源。

版本 3 绑定存储包含规范的 BindingId-ordered 集合。 每条记录都有一个唯一的 BindingId、PresentationId、CreationId、有界启动配置文件以及一个持久状态：`planned`、`create-requested`、`retained`、 `exited`、`closing` 或 `lost`。 保留的身份是唯一的，并且必须重复其所属 CreationId。 该文件被解析为不受信任的输入，通过仅限所有者的原子替换进行更新，并且在无效或不可读时不会自动重写。 启动环境值永远不会进入诊断。 每个默认启动都会声明`TERM=xterm-256color`, `COLORTERM=truecolor`， 和`TERM_PROGRAM=Orchbee`，并删除继承的`NO_COLOR`, `FORCE_COLOR`， 和`NODE_DISABLE_COLORS`主机创建之前的策略PTY过程。 新的 POSIX 终端还从代码解析的应用程序资源路径接收 `ORCHBEE_SHELL_INTEGRATION_DIR`：开发相对于构建的 Main 条目解析它，打包的应用程序在 `process.resourcesPath` 下解析它。 周围环境无法替代该路径。 Desktop 将其自己的 Zsh、Bash 和 Fish 脚本作为包资源提供，但从不获取它们或修改启动文件； 设置公开了确切的手动命令。 桌面保留显式的 UTF-8 语言环境； 当继承的 POSIX 语言环境优先级不会产生 UTF-8 时，Main 会删除冲突的 `LC_ALL`，修复无效的 `LANG`，并提供 UTF-8 `LC_CTYPE` 后备根据需要。 Darwin 使用 `en_US.UTF-8` 是因为它的 shell 环境不提供 Linux 的 `C.UTF-8` 语言环境。

## 本地终端启动配置文件

Electron Main 拥有 `terminal-launch-profiles-v1.json`，这是命名设备本地启动配置文件的严格有界目录。文件缺失时会选择内置系统 shell 配置文件：在 macOS 上启用登录模式，在其他所有平台上禁用。文件无效或不可读时会投影同一平台回退以及可恢复的问题，并且绝不会自动重写。显式创建、编辑、默认选择、删除和重置操作会被序列化，并使用仅限所有者的原子替换。已存储目录及其显式登录选择在替换提交前始终具有权威性；写入失败会保留先前的默认值和目录。

配置文件选择具有 POSIX 登录 shell 决策的受信任系统 shell 或具有文字有序参数向量的绝对自定义可执行文件。它仅覆盖可信库 `TerminalLaunch` 中的 `executable` 和 `arguments`；桌面继续拥有环境、终端演示配置文件、配置、几何结构和 CWD 策略。 Windows 报告登录 shell 支持不可用。 Main 在接受存储之前根据具体主机 `create-terminal` 编码解析并绑定每个目录配置文件。接受的自定义可执行文件后来无法启动，会报告现有的终端生命周期故障，并且永远不会回退到系统 shell。

当新终端首次配置时，在写入其持久绑定之前，默认配置文件将被解析一次。重试、渲染器重新加载、应用程序重新启动和恢复重用该绑定捕获的完整启动，因此编辑或更改当前默认值不会改变现有终端。仅本地Electron渲染器接收管理端口；经过身份验证的网关页面和普通浏览器客户端无法检查或更改目录。请参阅[`docs/terminal-launch-profiles.md`](docs/terminal-launch-profiles.md)。

普通远程 shell 仍然由用户在 PTY 内部调用的系统 SSH 客户端拥有。 Desktop 不会包装 `ssh`、保留远程凭据或主机记录、添加 OpenSSH 环境转发规则、安装 terminfo 或写入远程缓存。请求的SSH PTY可以携带标准`TERM=xterm-256color`；仅当用户的 OpenSSH 客户端配置和服务器都允许时，`COLORTERM`、`TERM_PROGRAM` 和任何其他额外变量才会跨连接。共享设置表面提供了一个常量只读远程诊断命令，该命令报告观察到的变量并区分未设置的 TERM、丢失的 `infocmp` 程序和丢失的 terminfo 条目。该命令不会执行网络、安装、文件系统或终端输入效果，直到用户明确选择在 shell 中运行它。

终端、绑定或附件都没有产品级数量上限。活动集合随共享产品结构和可见需求增长；分配、文件系统、Host 或内存资源耗尽会返回操作失败，而不会驱逐或静默截断另一个终端。Host 清单通过限定代次的协议分页读取，而不是一个有界全局列表。关联前的 `TerminalExited` 交付使用有界的按身份索引准入缓冲区：已准入身份永不驱逐；尚未获得绑定关联的身份会在适配器释放缓冲区前重试。Host 信封一经接受，Desktop Main 就会在一次性 Host 监听器返回前，把事件同步记录到所有者专用的持久待处理日志。独立的 64 项按身份索引内存队列只是调度器/缓存，而非崩溃恢复的权威所有者。待处理日志有独立的 4096 项容量；可安全跨重启的发布账本与持久拒绝日志也各有独立容量。日志已满或写入失败时会尽可能记录可观察的持久拒绝；若无法记录，准入路径会以失败关闭。未知身份保持持久待处理所有权，不进入定时循环；观察到确切身份后会唤醒晋升。队列已满、关机和已观察的待处理条目可由重试所有者晋升。只有持久运行时记录、适配器发布提交、发布账本准入、协调器接受与实时发布全部成功后，才会移除待处理条目；因此，压缩失败会保留持久恢复状态。关机在完成其余应用拆卸前，会把尚未解决的内存条目转移到待处理日志。

终端出口溢出存储使用有界事件数组迁移旧版 v1 文件，将每个身份规范为其最新版本，并原子地重写文件一次。旧的待处理记录仍保留 v2 待处理所有权。仅当已加载的持久发布分类账确认身份和修订后，旧发布记录才会被丢弃；如果没有该证据，启动迁移将无法关闭，并且 v1 文件保持不变。过大或格式错误的遗留输入也无法关闭。

`TerminalFailed` 是不同的结果，而不是合成退出。仅当绑定仍在获取其主机身份时，桌面才会保留身份键控关联缓冲区，然后在公开另一个投影之前记录该绑定的失败。发生故障的终端在幸存的主机中仍然保留并具有权威性，因此桌面重新启动可以从主机清单中恢复相同类型的故障；它不会复制到特定出口的溢出存储或发布分类帐中。这并不保证主机或操作系统崩溃后的恢复。

单独的 Workbench 共享文档存储仅通过一个 Main 拥有的序列化编写器接受规范版本 1 文档。缺失的存储从修订版 `0` 开始；以后的每次提交都必须准确命名当前修订版本并将其前进 1。冲突、非下一个修订、无效文件和读取失败会保留当前数据。写入使用仅限所有者的临时文件、文件同步、原子替换和父目录同步（如果支持）。客户端仅接收原子引导、类型化提案结果和有序发布；它永远不会接收路径、节点错误、存储功能或一般文件系统功能。请参阅[`docs/workbench-shared-document-storage.md`](docs/workbench-shared-document-storage.md)。商店使用`workbench-shared-document-v1.json`。对于未发布的开发拓扑文件，它没有读取器、别名或迁移；旧路径将被忽略并且不会自动删除。

在创建终端之前，配置会分配并保留新的绑定。恢复仅接受精确的 HostEpoch/TerminalId/CreationId 匹配，并且绝不会用新的 shell 来替换丢失的连续性。丢失的创建响应可能仅重播相同的 CreationId 并在记录的主机纪元中启动。孤儿状态、冲突状态、腐败账本状态和传输状态无法关闭。

保留协调是一项单独的仅恢复操作。仅当绑定已持久保留或退出，或在同一主生命周期中成功配置后可恢复时，它才会成功。丢失、计划、关闭或丢失的绑定永远不会通过恢复路径分配替换。恢复失败仍然保持持久保留意图。

每个 Presentation 的生命周期工作由一个操作尾部串行化；无关终端可独立推进。Main 协调器会先持久化并发布包含 provision、close 或 stop 意图的完整共享文档，再取得相应效果。FIFO 提案队列对内容不同的并发完整文档采用先写者胜出，并让等价竞态收敛。随后，close 会持久化技术状态 `closing`，只退役该 Presentation 的活动投影，并把 graceful/force/dispose 行为委托给现有绑定关闭生命周期。Panel 或 Tab 会留在 Workbench 中，直至每个精确终端关闭结果完成；随后提交清理。若清理失败，先前已存储的关闭意图仍可重放。渲染器重新加载、渲染器崩溃、正常 macOS 窗口关闭及不受控 Main 丢失都会释放该页面的投影附件，但保留 Host 终端。受控应用退出遵循以下策略。

## 应用生命周期

正常关闭 macOS 窗口只会释放渲染器页面。Command+Q、菜单退出，或关闭 Windows/Linux 上最后一个窗口时，会在渲染器仍可用期间进入统一的用户退出工作流。若一个或多个 Presentation 依据 Workbench 的 Panel 关闭谓词需要保护，应用级对话框会以“保留会话并退出”为默认选项，以“停止会话并退出”为破坏性选项，也可在执行任何停止效果前取消。若观察到每个运行中 Presentation 都处于空闲状态，或已无 Presentation，Desktop 会保留会话、刷新共享文档并直接退出，不显示提示。未来的 updater 会静默进入相同的 `update-restart` 保留准备；updater 安装、签名与发布分发不在本包范围内。

在保留或停止准备之前，Main 确认应用程序范围的协调器已进入静默状态。此确认对生命周期和客户端会话端口进行排序：其他页面无法将新的共享更改竞争到准备窗口中。在前退出所有者断开连接后，替换渲染器只能恢复相同的静止模式。

开发 Host 重启使用独立的非交互静默模式。执行任何停止效果前，渲染器只捕获当前可重启的 Presentation；Main 则跨渲染器文档替换持久化这一精确目标集合。即使共享文档已把它们记录为 stopping 或 stopped，替代渲染器也会从 Main 拥有的目标恢复。该工作流遵循相同的持久 stop 与 Host 排空屏障，以新 provision 标识替换每个已捕获的停止 Presentation，不把该 provision 派发给退役 Host，并刷新暂存文档。只有在精确的暂存修订已加载，且旧 Host 不再拥有任何绑定或未知终端后，Main 才写入 generation-bound ready plan。普通更新重启仍走保留路径，绝不会使用这种破坏性的开发模式。

stop 会封闭退出所有者页面上的终端创建，把每个可见或隐藏的 Workbench Presentation 投影为持久 `stop`，对完整共享文档执行 compare-and-swap，随后才释放关闭命令。成功的关闭结果会把 Presentation 变为 `stopped`，同时保留 Project、Workspace、Tab、Panel、Split、标题及启动 CWD。水合绝不会启动已停止的 Presentation。可见的“开始新会话”Action 会用新标识替换已停止的 PresentationId，并只 provision 所选 Presentation。

当持久 stop 到达最终 Host revision 时，Main 会在终端结果之后、Host Dispose 之前打开一个临时只读附件。它为该 Presentation 归档最新 5,000 行最终渲染行或 10 MiB，以先达到的边界为准。Desktop 本地存储使用带内容校验、私有权限的原子文件，并施加 250 MiB 的全局 least-recently-used 上限。有效归档会在本地 Electron 的 stopped surface 中供现有终端 renderer 使用，保留滚动、搜索、选择与复制，并永久拒绝输入和调整尺寸。归档缺失、损坏、超时、不可写或被淘汰时，会回退到空的 stopped 状态，且绝不阻塞 Host Dispose 或应用退出。归档不会进入共享文档、renderer bootstrap、Gateway、Browser storage 或 cloud。普通关闭会移除任何归档，共享文档协调会移除孤立条目，“开始新会话”会删除旧关联；归档行绝不会恢复到新 VT 或与其拼接。

被引用的 Presentation 停止后，Main 会加载精确的已提交共享文档修订，并关闭由它拥有但文档未引用的每项绑定。随后要求绑定注册表为空，并扫描 Host 清单。未知 Host 终端会被报告为未解决，绝不会静默终止。部分失败会让 Desktop 进入不可逆恢复对话框，并在受支持时重试，同时显示明确的“仍然退出”警告。“仍然退出”可能让进程继续运行；持久 `stop` 意图会保留到下次启动时的重放权限。本工作流不声明 OS 关机、注销或强制终止行为。

## 需求驱动的投影

Workbench 仅在显示的面板视口内安装应用程序拥有的终端表面。仅当表面的生命周期为 `running`、`exited` 或 `failed` 时才请求投影；卸载、切换选项卡、隐藏另一个终端上下文或输入 `closing` 会释放请求。因此，后台项目、工作区、选项卡、非活动子终端和其他不可见面板不包含渲染器、GPU 表面、帧流、历史缓存、搜索工作线程或主机附件。

默认可视启动携带空几何。provision 会持久化绑定 Presentation 并在 `awaiting-start` 中创建 Host 标识，但不生成子进程或 PTY。可见的共享 Web 表面会等待配置字体与正挂载的内容框，再依据该精确呈现配置、内边距和文档 DPR 计算完整网格与物理单元格几何。Electron 与 Gateway 的投影打开命令保持几何不变。Main 在 `AttachTerminal` 前串行化 `StartTerminal`，校验接受结果中的标识与几何，并且只有精确 Host 转换完成后才把绑定标为已启动。因此，子 PTY、Host Core、首个 Host 帧及渲染器固定的首次呈现均从相同几何开始。不存在超时或 80x24 可视回退。释放尚未挂载或仍在等待的 Presentation 会中止测量；若关闭的 Host 终端仍处于 `awaiting-start`，则直接将其处置，无需 graceful 或 force 终止。立即无头启动与测试组合仍在创建时提供完整几何。

在渲染器文档绘制之前，Electron 使用发送到 Terminal Host 的同一启动配置文件中的默认背景填充本机窗口。该值只是一个启动桥：提交的主机框架元数据拥有 Electron 和浏览器客户端中的实时终端背景，因此 Desktop Main 永远不会维护第二个运行时终端调色板。

每个投影引导程序都包含由生命周期适配器保留的完整的最新主机主题状态。投影代理将附件范围的 `apply-terminal-theme` 命令转发为一个主机比较和设置，并仅发布经过身份检查的完整结果。在渲染器可以使用稍后的引导程序之前，它将结果记录回生命周期恢复状态。协调读取主机清单，需要准确的创建/主机/终端身份，刷新相同的完整状态，并返回专用的协调事件。已知的未改变的故障和未知的传输结果使用特定于主题的结果通道；未知的结果必须在重试之前进行协调，并且永远不会演变成一般的投影失败。 Desktop Main 不渲染或独立选择终端颜色。

Desktop 隐藏本机标题栏，同时保留操作系统的窗口控件。桌面渲染器仅将侧边栏的顶部镶边和未占用的水平 Tab 轨道背景标记为本机窗口拖动区域；选项卡、组、侧边栏切换和 new-Tab 控件仍然是交互式非拖动区域。在 macOS 上，Main 禁用窗口控制覆盖，将本机交通信号灯定位在 `(8, 10)`，渲染器将侧边栏切换置于保留本机控制空间后的同一 `36px` 水平选项卡行中。切换位置本身就是一个非拖动区域，其保留的引导通道永远不会被标记为可拖动，并且可见的侧边栏拖动几何图形在该通道之后开始，因此完整的指针目标保持交互式。

Desktop 在受信任的主要组合边界处读取一次终端计算机的主机名，将其验证为运行时配置，并将其包含在每个 Electron 或网关客户端会话引导程序中。共享 Web 渲染器将该主机名与提交的主机标题/CWD 元数据相结合，并应用 Workbench 拥有的 Warp 范围策略。渲染器输入、浏览器位置和传输对等名称无法替换此主机名。自然标题和当前 CWD 保留每页瞬时观察结果；只有后备和显式自定义选项卡标题才会进入共享文档协调器。

每个成功请求都会获得新的 AttachmentId、完整能力 bootstrap、正数能力修订和私有 `MessagePort`。Desktop 通过 Workbench bootstrap 转发该修订；每条投影的原始或语义输入命令也会把它带回 Host。对于运行中的 Presentation，每个可写附件都获得 read/input/resize 能力；串行化 open 只有在不存在活动 owner 时才请求初始 resize lease。接纳的 `focused: true`、key press/repeat、已提交 text 或 paste、application mouse press 或 terminal-selection start 会把该可写 projection 记录为最新 desired owner；attachment、background state、viewport resize、pointer motion、release/repair input 与 `focused: false` 不会。只有在运行中 terminal 上仍具有 input 与 resize 能力的 attachment 才能产生租约 activity 或发起 claim；input 降级会使其排队的 ownership intent 失效。Desktop 合并并发 intent，一次串行一个显式 Host `ClaimResizeLease`，且不会延迟 input。已提交 transfer 先以旧 owner 的 revoke 到达 Main，再以新 owner 的 grant 到达；晋升的 renderer 立即测量当前 surface 并提交该 geometry。Owner detach 时，Host 晋升 successful claim 最新的剩余 resize-capable attachment，从未 claim 的候选以最早 admission 打破平局；若 Desktop 记录了更新的剩余 activity，则会再次 claim。open、输入能力变更、provision 与 exit 操作按 Presentation 顺序串行化。只有 Main 拥有的投影租约方能请求实时输入变更；Broker 会向 Host 提交精确的下一修订，校验返回的完整附件标识、不可变 read/resize 状态、resize 租约、新输入值与修订，然后才向下游发布状态。重复的目标状态为空操作。渲染器命令不能提升自身能力；无效或不可发布的 Host 结果会废弃附件，而非授予客户端权限。Main 只有在待处理能力工作和分离都完成后才返回对应释放确认。Host 请求传输仍可用时，清理失败会保留同一请求以便稍后精确重试，且不能使另一个投影请求失效。一旦确认连接终止，Host 连接 teardown 就构成该连接附件的清理证据：Desktop 不再发送 Host 清理请求，并结算连接作用域内的本地租约/分离所有者，而不改变保留的终端。已退出或失败的终端以只读方式附加保留最终修订及类型化结果。多个可见 Split Panel 与页面客户端可以分别持有独立附件，不设全局附件阈值。

每个 `ProjectedTerminalBroker` 还为其附件拥有一个有界调整大小命令序列化器。串行器处于空闲状态或只有一个主机调整大小请求处于活动状态；当处于活动状态时，每个较新的经过验证的渲染器测量都会替换单个待处理的测量，而不是启动竞争的 PTY 调整大小。活动请求的成功或类型化失败使串行器返回空闲状态，并在附件保持活动状态时立即接受最新的待处理测量。活动失败仍然通过现有的类型化渲染器失败路径发布，并且永远不会静默重播；只有在开始后到达的测量才能驱动下一个请求。处置或附件清理会删除挂起的测量，其他渲染器命令保留其现有并发性，并且挂起的调整大小在调度之前不占用主机请求窗口槽。因此，容器测量的突发使用恒定内存，一次最多提交一个调整大小，并使最新的挂起几何图形成为下一个主机请求，而不允许中间请求超越它。

`HostConnection` 读取并验证固定终端帧标头一次，然后通过 AttachmentId 直接路由二进制记录。 Main 不执行 VT 解析、行重建、Kitty 图像加载、JSON/base64 转换或每单元传递。在 Electron 功能边界，代理在结构化克隆之前仅将路由有效负载复制到精确大小的 `ArrayBuffer` 中，因此信封标头和合并的相邻记录永远不会穿过渲染器端口。渲染器严格解码 OBTF 内的有界 OBKG 原始像素，并且从不接收路径、URL、临时文件句柄或共享内存功能。 OBPB 格式 `0.0.2` 本地历史渲染缓冲区然后使用结构视图边界；网关交付仍然经过外部验证。历史记录、选择、复制、链接和辅助功能行为仍然属于附件范围。正常的底部跟踪使用不会创建历史缓存或搜索 Worker；这些资源只能通过其现有的交互合约来激活。

相关主机 `TypedFailure` 仍为类型化 `HostRequestError`； `HostConnection` 保留了响应相关性 ID，而 `ProjectedTerminalBroker` 添加了确切的渲染器命令操作，而无需替换主机代码、消息或可重试性。失败的行读取还会保留其渲染器请求序列，以便只有历史记录飞行才能解决。客户端拥有的复制和链接效果使用其请求序列。 Electron 和网关转发相同的有界故障合约，并且两个代理都不会重试该诊断身份的用户效果。

Electron 附件仅请求协议的文字 `plain` 选择导出并通过 Main 的剪贴板提交解码的 UTF-8 文本。网关附件接收相同的有界纯文本选择字节，并通过浏览器剪贴板 API 提交它们。投影命令和平台效果都不会公开 HTML 或原始 VT 格式选择器。链接仅通过 Electron 或本地浏览器手势打开列入白名单的 `http`、`https` 或 `mailto` 目标。浏览器永远不会收到通用 Electron 效果端口。附件传输背压仅使该附件退出；可见表面需要新的投影和完整的快照。

意外的共享主机连接关闭会在请求清理之前向每个仍处于活动状态的附件发布一个可重试的 `host-connection-unavailable` 故障。每个可见表面仅冻结其最后提交的投影并进入相同的附件本地恢复路径；它不会等待另一个用户命令来发现主机传输已经结束。

主机承认的 OSC 52 纯文本写入与用户启动的选择复制不同。 Main 路由有界附件事件而不解释其文本。共享渲染器会自动处理确切的请求。桌面效果附件将单一特权写入委托给主程序；客户端效果附件首先调用其已经限定范围的平台剪贴板效果并报告本地写入是否完成。附件停用、渲染器替换或不可用的客户端本地效果不会产生延迟写入或重复写入。 Main 和渲染器都不会保留请求文本或信任决策；主机传送到一个附件意味着另一桌面或网关页面在切换后无法接收相同的请求。在允许一个本地视觉表面提示之前，通过共享工作台生命周期中的不透明终端身份和主机序列来消除重复的响铃传送。 Desktop 目前既不注册单独的选择加入声音效果，也不注册单独的选择加入操作系统通知效果，因此默认情况下，Bell 既不请求窗口焦点、音频也不请求系统通知。 OBTF 进展仍然安静的演示元数据。语义命令完成是分开的：在用户启用默认关闭设备本地策略后，渲染器可以呈现本地化的终端状态并使用其页面本地通知功能。 PTY 输出无法请求通知权限，并且完成负载不包含命令文本。

已知主机 `TerminalExited` 事件进入 Main 拥有的重试/协调路径，然后通过权威运行时注册表进行解析，保留到确切的绑定，并由 PresentationId 发布到工作台。仍然无法解析的身份将保留为持久的、可观察的协调记录，并且仅当观察到其确切身份（或通过可重试的已知条目路径）时才会升级；它不会在无限的未知身份计时器循环中旋转。恢复的退出绑定只会恢复到 Workbench，直至其经过身份检查的 bootstrap 事件进入 managed 自然退出 removal 协议。在此期间最终修订仍可读取；随后产生的 close intent 会在不终止进程的情况下处置 Host 终端，再由 Workbench 按 Panel 拓扑移除或提升精确的 Presentation。协调器预留、适配器提交、发布分类账记录和可见事件是单独的步骤：只有在所有持久和表示所有权步骤成功后，引导程序才能看到事件。在第一个退出发布被消耗之前确认耐久性。

Workbench `terminal-presentation-event-published` 消息是 Main 拥有的单一生命周期发布。类型化主机故障变成不可重试的 `terminal-failed` 呈现结果；协调器保留它以供以后的客户端引导，而工作台使终端保持可投影和只读状态，直到显式关闭为止。 `ProjectedTerminalBroker` 还可以转发身份检查、附件本地原始 `terminal-exited` 或 `terminal-failed` 通知，以便实时投影可以与主机状态同步；该渲染器传输有自己的单调修订过滤器，位于协调器序列之外，并且不是第二个工作台发布。因此，这两个通道不提供跨通道的一次性声明。

## 正确性证据

重点跟踪范围包括：

- 严格的客户端会话和投影边界验证、格式错误的消息退出、精确的提案关联以及页面附件处理；
- 原子引导、FIFO 完整文档提案、第一作者获胜、使用 Client View 保存进行过时回滚、无效文件保存和一个有序发布流；
- 首次提交和每命令预写排序、Main 拥有的仅恢复恢复、失败存储效应抑制以及效应后托管容器清理；
- 新配置、多绑定存储保存、恢复、退出恢复到处置、孤儿和冲突处理、持久关闭和表示注册表重建；
- 不相关的终端并发、精确关闭定位、保留原生窗口的自然退出 Tab 移除、Tab 关闭结果排序和过时结果拒绝；
- 两个恢复的工作台选项卡，其中两个生命周期记录一致，但只有活动的可见选项卡具有附件，然后是选项卡切换上的精确释放/打开行为；
- 确切的 `terminal-parity/SU-16-startup-geometry` 真实 Electron/主机证据：受控 POSIX shell 在将控制权交给 `/bin/sh` 之前记录其 PTY 大小，并且第一个子几何体与独立派生的安装网格和唯一的第一个客户端框架匹配，无需中间`80x24` 框架；所有者本地跟踪涵盖无子延迟创建、附加前启动排序、重复/冲突启动、失败启动和待处理处理。系统见证仅是 POSIX，不会声明 Windows ConPTY 启动或以后的大小调整；
- 明确的块/非闪烁配置文件默认值、闪烁条 DECSCUSR 覆盖和 RIS 恢复的精确 `terminal-parity/SU-13-cursor-defaults` real-PTY 投影证据。主机运行时测试还涵盖应用程序重置和非默认配置文件；系统见证人观察规范的投影元数据，而不是将平台屏幕截图视为光标权限；
- 并发查看者只有一个调整大小所有者、last-meaningful-activity handoff、recency fallback、动态渲染器所有权以及序列化发布/开放排序；
- deferred-Host 调整轨迹大小，证明每个附件有一个活动请求，中间测量的恒定空间替换，以及结算后立即发送最新的几何图形；
- 精确的 `terminal-parity/IN-10-read-only` 能力证据，覆盖 Host 提交的实时降级/升级、即时客户端修复、只读本地输出交互、不重放陈旧输入，以及分离前串行化变更；
- 精确标题为 `terminal-parity/IN-14-shared-focus` 的证据，包含两个投影附件各自的焦点声明、一个共享终端聚合焦点区间、只修复目标附件的可见性，以及激活时不重放声明。该夹具控制应用层焦点，不要求操作系统多窗口焦点；
- 精确标题 `terminal-parity/PR-11-bell`、`terminal-parity/IN-07-pre-input`、`terminal-parity/IN-09-action-scope`、`terminal-parity/IN-11-context` 和 `terminal-parity/IN-12-actions` 提供 Canvas 证据，覆盖共享铃声协调器、无按键应用鼠标移动上报期间的历史与选择保留、窗口失焦折叠无障碍 DOM 镜像时的 Host 选择保留、containing-Workbench 操作范围、固定/重新验证的语义上下文、同步可用性、类型化执行和本地化反馈。捕获的效果不声明本机音频、通知、URL-handler 或剪贴板传送；
- 精确的 `terminal-parity/IN-08-drop` protected file-marker admission、Desktop backing-path
  resolution、有界 Ghostty-compatible POSIX escaping、直接插入 line middle，以及不打开 review
  或追加隐式 Enter；
- 精确的 `terminal-parity/FS-05-file-links` 和 `terminal-parity/FS-06-url-policy` 用于禁用文件系统激活和共享 `http`/`https`/`mailto` 检测、悬停、上下文和激活边界的浏览器组合见证；
- 用于每次操作安全上下文、瞬时用户激活和 API 检查的精确标题 `terminal-parity/LC-05-lan-capability` 浏览器组合证据；同步复制可用性；一位被录取的作家来电；拒绝而不重试；局部故障；并且没有特权 insecure-LAN 路径；
- 确切的 `terminal-parity/HS-07-selection-export` real-PTY/Electron 证据表明，样式化终端选择以精确可见的纯文本形式到达平台剪贴板，与不公开 HTML 或原始 VT 格式的协议、主机和投影命令回归配对；
- 固定标头帧仅路由到匹配的 AttachmentId，并具有精确大小的结构化克隆有效负载隔离；
- 完整的主题引导、身份范围的应用/协调转发、生命周期刷新、未知结果保存以及精确的 `terminal-parity/SU-09-theme` 持有/释放证据，不会暴露混合应用程序/终端修订；
- 精确标题为 `terminal-parity/CF-03-visual-environment` 的证据，使用仓库产品字体，在生产渲染器能力边界强制选择 Canvas 2D，并固定 `900x500` 视口、DPR `1`、`en-US`、UTC、浅色配色方案及减少动态效果。它断言精确语义状态，不执行像素容差标准化；它不是 WebGPU、跨浏览器、原生 DPI 或操作系统字体光栅化的黄金基准；
- 沙盒端到端输入、输出、历史记录、搜索、选择、复制、链接、退出、渲染器重新加载和 Electron Main 重新启动；
- 开发和打包的 Shell 资源解析、资源存在和隐私检查、真正的 Zsh OSC 133 输出以及一个 macOS Electron 从手动设置策略到长命令完成提醒的路径，无需替换预计的附件；
- 一个 Electron 渲染器和两个普通浏览器页面共享规范工作台和终端状态，同时保留新页面会话、Client View 和附件标识，包括浏览器复制、链接、断开连接、拒绝和重新连接行为；
- 用户退出保留/取消/停止、空闲运行终端的自动保留、面板关闭保护的精确重用、隐藏终端停止、failed-CAS 抑制、部分停止重试、未引用绑定耗尽、unknown-Host-orphan 拒绝、更新重新启动保留、开发主机停止/阶段/就绪排序以及显式启动停止会话后的新身份。

桌面终端生命周期适配器模型检查新的延迟绑定是否等待已安装的几何图形，启动失败返回到等待状态而不提交几何图形，启动成功提交精确观察到的几何图形，并且排队关闭遵循配置或退休前的启动。现有的 TLA+ 模型继续检查独立有界终端绑定恢复和退出事件恢复、生命周期适配器序列化、投影需求和工作台删除安全性。恢复检查分别执行主机纪元/创建/文档生成行为以及跨主要发布和重新获取的持久退出准入/发布；每个声明的恢复操作都必须有可联系的 TLC 见证人。 `DesktopLifecycle` 将应用程序生命周期、浏览器访问、开发运行时、项目文件夹获取、渲染器文档、终端绑定关闭和窗口状态保留为一个注册工件中的七个场景隔离配置；每个配置都保留其转换系统、不变量、进度属性和所需的动作见证，而无需乘以七个状态空间。其项目文件夹获取配置检查一个实时请求、仅选择的突变、取消和无突变的失败、过时或重复的工作台目标、渲染器退休和延迟本机完成。它的窗口状态配置检查有序的正常边界捕获、序列化持久性、瞬时全屏排除、刷新完成以及跨可写和不可用存储分支的处置。它的渲染器文档配置检查忽略了相同文档和子帧导航、生成范围的控件替换以及有界加载进度。其应用程序生命周期配置检查提示、保留、停止、更新和开发主机重启路径；收盘前停止订单；持久的重启准备和计划准备；未引用的绑定漏极；生效前取消；干净的退出；以及显式的强制退出转义。它的开发运行时配置检查失败构建保留、合并进程生成、就绪计划排序以及关闭而不重新启动。其末端结合关闭配置保留了精确目标突变和无关结合保留。Desktop resize-lease model 检查三个 attachment、串行 open/release/claim operation、latest-activity intent、当前 input/resize/running 资格、幂等与遭拒 claim、successful-claim recency promotion、仅 owner 可改变 canonical geometry、即使 canonical metadata 陈旧也会在 grant 边沿提交当前测量 geometry，以及 ownership、显式拒绝或 intent 失效三种收敛结果。桌面共享文档存储模型还检查下一个 CAS、冲突保留、序列化、不可用存储、生成处理以及修订版 `2` 的进度。桌面多客户端模型检查两个会话、缺少/可用的引导程序、等效和不同的竞争、影响发布排序、附件/会话所有权、断开连接、静默切换、不可用存储和有限的发布进度。桌面预写模型在提供、恢复、关闭或停止调度之前检查匹配的持久意图；提交失败保存；无需创建替换即可恢复；渲染器处理和重新加载重放；以及一个抽象演示的实时生成进度。 Workbench 的终端停止模型检查布局保留、无绑定停止、重试身份和新的 PresentationId 生成。`ProjectWorktreeCreation` 另行检查有界的逐 repository 创建、Git/产品一致性、恢复 ledger 覆盖、response 顺序，以及最终 commit 或保留 recovery 的结果。存储库没有为此集成切片强加新的吞吐量接受门；可选的基准仍然是一种诊断工具。

Desktop terminal-archive 模型会另外检查 stop/close 分离、Host disposal 前 capture 收敛、有界只读 archive projection、corruption/eviction fallback、cleanup 与 fresh-session non-merging。

调整大小租约工件还把 Broker 串行器隔离在独立场景中：最新测量会替换一个待处理调整大小；活动调整大小绝不会被超越；结算会提升最新待处理几何；清理会移除待处理工作；静止状态最终会排空串行器。

## 开发

`pnpm desktop` 拥有一个开发运行时，其中包含 Web 开发服务器 Electron 和受监督的 Terminal Host。运行时遵循以下更新策略：

| 成功的改变                            | 发展行动                                                                                      | 保存状态                                              |
| ------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Web 渲染器或源构建的前端包            | 通过Web开发服务器重新加载渲染器文档                                                           | Canonical Workbench 文档、桌面绑定、主机终端          |
| Electron Main、预加载或源构建的依赖项 | 刷新渲染器文档，重新启动Electron，并重新连接到同一主机                                        | Canonical Workbench 文档和主机终端                    |
| Terminal Host Zig 源码                | 将每个正在运行的演示文稿作为新的持久配置进行暂存，停止旧会话，更换主机，然后重新启动 Electron | 项目、工作区、选项卡、面板、拆分、标题和启动 CWD 拓扑 |

Electron 或 Host 编译失败不会影响最后运行的进程。只有新二进制文件完成，且旧渲染器已持久暂存新的终端 provision 后，才开始替换 Host；暂存路径不可用时会中止替换。重启后的 Presentation 会获得新的 Workbench Presentation、Desktop 绑定、Host、创建、终端和附件标识；其进程、shell 状态、瞬态元数据、选择与回滚缓冲会被刻意丢弃。该策略仅用于开发：打包更新后的重启仍会保留 Host 中的保留终端。

桌面开发运行时拥有一个序列化更新工作流程。替换过程中到达的较新的成功构建将被合并并随后应用；即使连续观察看到相同的源摘要，每个更改的主机二进制文件也会被接纳。捕获的二进制文件仅限于活动代、回滚代和挂起代，并且关闭 Rsbuild 服务器会阻止所有重新启动。 `DesktopLifecycle` 的 `development-runtime` 配置检查两个有界主机和 Electron 代、失败构建、合并成功构建、持久重启准备、进程替换和公平收敛。确定性运行时测试自己的命令顺序、适配器故障、中止准备、更新合并、工作区清理和捕获的二进制不变性。强制操作系统终止、编译器正确性和跨主机进程的 PTY 迁移仍然不在这些声明范围内。

开发运行时仅从受监督的 Terminal Host 环境中删除继承的 `PAGER` 和 `GIT_PAGER` 覆盖。因此，交互式 shell 选择其配置的或默认的寻呼机，而 Electron、构建过程、打包启动和调用开发人员环境保持不变。

一起启动规范的 Web 开发服务器和桌面：

```sh
pnpm desktop
```

`pnpm desktop` 构建浏览器访问服务器、网关和静态 Web 资源，然后运行 Web 开发服务器和桌面拥有的本机/Electron 开发生命周期。 Electron 默认为 `http://127.0.0.1:3000/`；可以使用 `ORCHBEE_DESKTOP_RENDERER_URL` 选择显式环回 Web 服务器。打开 **设置 → 浏览器访问** 以启动环回访问或显式启用 trusted-LAN 预览。 Desktop 永远不会打印承载能力的 URL 或将其返回到渲染器状态；它的主进程执行复制链接和打开浏览器效果。设置 `ORCHBEE_DESKTOP_GATEWAY=0` 以从开发启动中删除浏览器访问资源。

使用以下命令构建未签名的当前平台应用程序包：

```sh
pnpm desktop:package
```

该命令在打包之前构建 Terminal Host、Web 渲染器、浏览器服务器、网关和 Electron Main/预加载包。 Web 构建既是只读的 `orchbee://app` 资源，也是按需浏览器渲染器根；本机主机、服务器和网关条目在 `app.asar` 外部仍然可用。输出是在 `apps/desktop/out/` 下生成的，不是签名的安装程序或发布工件。

`ORCHBEE_TERMINAL_HOST_BINARY` 选择绝对的开发/测试主机二进制文件。 `ORCHBEE_TERMINAL_SCROLLBACK_BYTES` 从 `0` 到 `1000000000` 选择以十进制字节为单位的临时开发启动/默认回滚预算；内置默认值为 `10000000`。当不使用默认同级版本时，`ORCHBEE_GATEWAY_ENTRY_PATH`、`ORCHBEE_SERVER_ENTRY_PATH` 和 `ORCHBEE_WEB_RENDERER_ROOT` 选择显式开发浏览器访问资源。 `ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`、`ORCHBEE_DESKTOP_ENDPOINT_ID` 和 `ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` 隔离受控开发和系统测试状态。它们不是版本化的主机安装或发现机制。默认的 POSIX 运行时使用上述受信任的每用户运行时位置的短稳定产品范围子代；显式运行时目录仍由本机端点安全边界准备和验证。

`pnpm desktop` 在内部设置 `ORCHBEE_TERMINAL_HOST_SUPERVISED=1`，以便 Main 仅重试开发运行时拥有的主机进程，并为生成准备握手提供仅所有者的 `ORCHBEE_DESKTOP_DEVELOPMENT_RESTART_PLAN` 路径。这两个变量是私有开发组合控件，不支持产品配置或生产更新机制。

从仓库根目录运行包检查：

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/gateway-protocol test
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

### 手动本机窗口测试

`test:system` 从每个聚合、自动、CI 和代理启动的运行中排除 `@manual-native-window` 测试。该测试会改变操作系统焦点，同时验证最小化、激活、最大化、全屏和重新启动行为。仅在人类明确批准该精确测试后才运行它：

```sh
ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL=I_ACKNOWLEDGE_FOCUS_INTERRUPTION \
  pnpm --filter @orchbee/desktop test:system:manual:native-window
```

AI 代理不得调用命令或设置批准变量，除非当前人工请求明确授权此精确的焦点窃取测试。任何聚合或全套命令都不能调用手动脚本。

可选的吞吐量线束需要绝对大输出的夹具和至少五次运行：

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5
```

使用基准测试时保持 `pnpm web` 运行，或将 `ORCHBEE_DESKTOP_RENDERER_URL` 设置为另一个环回 Web 开发服务器。

它还可以收集受控的 Ghostty 和仅主机引用。结果是诊断证据；它们不是当前开发阶段的合并或切片完成门。
